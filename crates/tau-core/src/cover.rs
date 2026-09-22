//! Copy-only cover-art helpers.  These functions never open a source file for writing.

use crate::{ErrorCode, TauError};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_COVER_BYTES: usize = 2 * 1024 * 1024;
const COVER_NAMES: [&str; 6] = [
    "cover.jpg",
    "cover.jpeg",
    "folder.jpg",
    "front.jpg",
    "cover.png",
    "folder.png",
];

pub fn find_cover(folder: &Path) -> Option<PathBuf> {
    for name in COVER_NAMES {
        let cover = folder.join(name);
        if cover.is_file() {
            return Some(cover);
        }
    }
    let mut candidates = fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path.file_name().is_some_and(|name| {
                    name.to_string_lossy()
                        .to_ascii_lowercase()
                        .starts_with("cover-")
                })
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.into_iter().next()
}

/// Checks whether a discovered cover is safe for Tau's current JPEG decoder.
pub fn validate_jpeg_cover(cover: &Path) -> Result<(), TauError> {
    validate_jpeg(&fs::read(cover)?)
}

/// Embeds a baseline JPEG as the only MP3 APIC frame in a copied file.
/// Existing non-art ID3 frames and audio bytes are retained verbatim.
pub fn embed_mp3_copy(source: &Path, cover: &Path, output: &Path) -> Result<(), TauError> {
    let image = fs::read(cover)?;
    validate_jpeg(&image)?;
    let data = fs::read(source)?;
    let (major, frames, audio) = if data.starts_with(b"ID3") {
        if data.len() < 10 || !matches!(data[3], 3 | 4) {
            return Err(TauError::e(
                ErrorCode::UnsupportedCover,
                "unsupported ID3 version for cover embedding",
            ));
        }
        if data[5] & 0xd0 != 0 {
            return Err(TauError::e(
                ErrorCode::UnsupportedCover,
                "ID3 unsynchronisation, extended header, or footer is not safe to rewrite",
            ));
        }
        let major = data[3];
        let size = syncsafe(&data[6..10]) as usize;
        if 10 + size > data.len() {
            return Err(TauError::e(
                ErrorCode::UnsupportedCover,
                "malformed ID3 tag",
            ));
        }
        let mut kept = Vec::new();
        let mut position = 10;
        let end = 10 + size;
        while position + 10 <= end {
            let header = &data[position..position + 10];
            if header[0] == 0 {
                break;
            }
            let length = if major == 4 {
                syncsafe(&header[4..8]) as usize
            } else {
                u32::from_be_bytes(header[4..8].try_into().unwrap()) as usize
            };
            if length == 0 || position + 10 + length > end {
                return Err(TauError::e(
                    ErrorCode::UnsupportedCover,
                    "malformed ID3 frame",
                ));
            }
            if &header[..4] != b"APIC" {
                kept.extend_from_slice(&data[position..position + 10 + length]);
            }
            position += 10 + length;
        }
        (major, kept, data[end..].to_vec())
    } else {
        (3, Vec::new(), data)
    };
    let mut picture = Vec::new();
    picture.extend_from_slice(b"\0image/jpeg\0\x03\0");
    picture.extend_from_slice(&image);
    let mut tag = frames;
    tag.extend_from_slice(b"APIC");
    let length = if major == 4 {
        to_syncsafe(picture.len() as u32).to_vec()
    } else {
        (picture.len() as u32).to_be_bytes().to_vec()
    };
    tag.extend_from_slice(&length);
    tag.extend_from_slice(&[0, 0]);
    tag.extend_from_slice(&picture);
    let mut result = Vec::new();
    result.extend_from_slice(b"ID3");
    result.extend_from_slice(&[major, 0, 0]);
    result.extend_from_slice(&to_syncsafe(tag.len() as u32));
    result.extend_from_slice(&tag);
    result.extend_from_slice(&audio);
    fs::write(output, result)?;
    Ok(())
}
/// Writes a JPEG PICTURE block into a FLAC copy, retaining audio and all
/// metadata except old PICTURE and PADDING blocks.
pub fn embed_flac_copy(source: &Path, cover: &Path, output: &Path) -> Result<(), TauError> {
    let image = fs::read(cover)?;
    validate_jpeg(&image)?;
    let (width, height) = jpeg_dimensions(&image).ok_or_else(|| {
        TauError::e(
            ErrorCode::UnsupportedCover,
            "could not read JPEG dimensions",
        )
    })?;
    let data = fs::read(source)?;
    if !data.starts_with(b"fLaC") {
        return Err(TauError::e(ErrorCode::UnsupportedCover, "not a FLAC file"));
    }
    let mut position = 4;
    let mut blocks = Vec::new();
    loop {
        if position + 4 > data.len() {
            return Err(TauError::e(
                ErrorCode::UnsupportedCover,
                "malformed FLAC metadata",
            ));
        }
        let header = data[position];
        let kind = header & 0x7f;
        let length = ((data[position + 1] as usize) << 16)
            | ((data[position + 2] as usize) << 8)
            | data[position + 3] as usize;
        position += 4;
        if position + length > data.len() {
            return Err(TauError::e(
                ErrorCode::UnsupportedCover,
                "malformed FLAC metadata block",
            ));
        }
        if kind != 1 && kind != 6 {
            blocks.push((kind, data[position..position + length].to_vec()));
        }
        position += length;
        if header & 0x80 != 0 {
            break;
        }
    }
    let mut picture = Vec::new();
    picture.extend_from_slice(&3u32.to_be_bytes());
    picture.extend_from_slice(&10u32.to_be_bytes());
    picture.extend_from_slice(b"image/jpeg");
    picture.extend_from_slice(&0u32.to_be_bytes());
    picture.extend_from_slice(&width.to_be_bytes());
    picture.extend_from_slice(&height.to_be_bytes());
    picture.extend_from_slice(&24u32.to_be_bytes());
    picture.extend_from_slice(&0u32.to_be_bytes());
    picture.extend_from_slice(&(image.len() as u32).to_be_bytes());
    picture.extend_from_slice(&image);
    blocks.push((6, picture));
    let mut result = Vec::from(&b"fLaC"[..]);
    for (index, (kind, body)) in blocks.iter().enumerate() {
        result.push((if index + 1 == blocks.len() { 0x80 } else { 0 }) | kind);
        let length = body.len() as u32;
        result.extend_from_slice(&[(length >> 16) as u8, (length >> 8) as u8, length as u8]);
        result.extend_from_slice(body);
    }
    result.extend_from_slice(&data[position..]);
    fs::write(output, result)?;
    Ok(())
}
fn validate_jpeg(data: &[u8]) -> Result<(), TauError> {
    if data.len() > MAX_COVER_BYTES {
        return Err(TauError::e(
            ErrorCode::UnsupportedCover,
            "cover exceeds the 2 MiB firmware limit",
        ));
    }
    if !data.starts_with(&[0xff, 0xd8]) {
        return Err(TauError::e(
            ErrorCode::UnsupportedCover,
            "cover embedding currently accepts JPEG only; optimise PNG or progressive JPEG first",
        ));
    }
    if is_progressive_jpeg(data) {
        return Err(TauError::e(
            ErrorCode::UnsupportedCover,
            "progressive JPEG covers are not supported; export a baseline JPEG first",
        ));
    }
    Ok(())
}

/// The Tau decoder accepts baseline JPEG artwork.  We inspect markers rather
/// than trusting a filename, so a progressive JPEG renamed to `.jpg` cannot
/// reach the card unnoticed.
fn is_progressive_jpeg(data: &[u8]) -> bool {
    let mut position = 2;
    while position + 4 <= data.len() {
        if data[position] != 0xff {
            position += 1;
            continue;
        }
        while position < data.len() && data[position] == 0xff {
            position += 1;
        }
        let Some(&marker) = data.get(position) else {
            return false;
        };
        position += 1;
        if matches!(marker, 0xd8 | 0xd9 | 0x01 | 0xd0..=0xd7) {
            continue;
        }
        let Some(length) = data
            .get(position..position + 2)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u16::from_be_bytes)
        else {
            return false;
        };
        if length < 2 || position + length as usize > data.len() {
            return false;
        }
        if marker == 0xc2 {
            return true;
        }
        position += length as usize;
    }
    false
}
fn jpeg_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let mut position = 2;
    while position + 9 <= data.len() {
        if data[position] != 0xff {
            position += 1;
            continue;
        }
        while position < data.len() && data[position] == 0xff {
            position += 1;
        }
        let marker = *data.get(position)?;
        position += 1;
        if marker == 0xd8 || marker == 0xd9 {
            continue;
        }
        let length = u16::from_be_bytes([*data.get(position)?, *data.get(position + 1)?]) as usize;
        if length < 2 || position + length > data.len() {
            return None;
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            return Some((
                u16::from_be_bytes([data[position + 5], data[position + 6]]) as u32,
                u16::from_be_bytes([data[position + 3], data[position + 4]]) as u32,
            ));
        }
        position += length;
    }
    None
}
fn syncsafe(bytes: &[u8]) -> u32 {
    (u32::from(bytes[0]) << 21)
        | (u32::from(bytes[1]) << 14)
        | (u32::from(bytes[2]) << 7)
        | u32::from(bytes[3])
}
fn to_syncsafe(value: u32) -> [u8; 4] {
    [
        ((value >> 21) & 127) as u8,
        ((value >> 14) & 127) as u8,
        ((value >> 7) & 127) as u8,
        (value & 127) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    #[test]
    fn embeds_only_in_copy() {
        let root = std::env::temp_dir().join(format!(
            "tau-cover-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.mp3");
        let cover = root.join("cover.jpg");
        let output = root.join("copy.mp3");
        fs::write(&source, b"audio").unwrap();
        fs::write(&cover, [0xff, 0xd8, 0xff, 0xd9]).unwrap();
        embed_mp3_copy(&source, &cover, &output).unwrap();
        assert_eq!(fs::read(&source).unwrap(), b"audio");
        let copy = fs::read(output).unwrap();
        assert!(copy.starts_with(b"ID3"));
        assert!(copy.windows(4).any(|window| window == b"APIC"));
        assert!(copy.ends_with(b"audio"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn embeds_flac_picture_in_copy() {
        let root = std::env::temp_dir().join(format!(
            "tau-flac-cover-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.flac");
        let cover = root.join("cover.jpg");
        let output = root.join("copy.flac");
        let mut flac = b"fLaC".to_vec();
        flac.extend_from_slice(&[0x80, 0, 0, 18]);
        flac.extend_from_slice(&[0; 18]);
        flac.extend_from_slice(b"audio");
        fs::write(&source, &flac).unwrap();
        let jpeg = [
            0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, 0, 1, 0, 1, 3, 1, 17, 0, 2, 17, 0, 3, 17, 0, 0xff,
            0xd9,
        ];
        fs::write(&cover, jpeg).unwrap();
        embed_flac_copy(&source, &cover, &output).unwrap();
        assert_eq!(fs::read(&source).unwrap(), flac);
        let copy = fs::read(output).unwrap();
        assert!(copy.windows(10).any(|window| window == b"image/jpeg"));
        assert!(copy.ends_with(b"audio"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_progressive_jpeg_before_writing_a_copy() {
        let root = std::env::temp_dir().join(format!(
            "tau-progressive-cover-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.mp3");
        let cover = root.join("cover.jpg");
        let output = root.join("copy.mp3");
        fs::write(&source, b"audio").unwrap();
        // SOF2 is the progressive JPEG frame marker.
        fs::write(&cover, [0xff, 0xd8, 0xff, 0xc2, 0, 2, 0xff, 0xd9]).unwrap();
        assert!(embed_mp3_copy(&source, &cover, &output).is_err());
        assert!(!output.exists());
        assert_eq!(fs::read(&source).unwrap(), b"audio");
        fs::remove_dir_all(root).unwrap();
    }
}
