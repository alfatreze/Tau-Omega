//! Copy-only cover-art helpers.  These functions never open a source file for writing.

use crate::{ErrorCode, TauError};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
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

/// Whether a media file already carries embedded cover art: an MP3 APIC
/// frame (any ID3v2 major version) or a FLAC PICTURE block. Read-only, used
/// by the Problems screen to tell apart tracks that already have art from
/// ones that would need a folder cover. Returns `Ok(false)` rather than an
/// error for anything unreadable or malformed, since a broken tag is a
/// separate, already-reported problem.
pub fn has_embedded_cover(path: &Path) -> Result<bool, TauError> {
    let Some(data) = read_tag_region(path)? else {
        return Ok(false);
    };
    if data.starts_with(b"ID3") {
        return Ok(id3_has_apic(&data).unwrap_or(false));
    }
    if data.starts_with(b"fLaC") {
        return Ok(flac_has_picture(&data).unwrap_or(false));
    }
    Ok(false)
}

fn id3_has_apic(data: &[u8]) -> Option<bool> {
    let major = *data.get(3)?;
    if !matches!(major, 2..=4) {
        return Some(false);
    }
    let size = syncsafe(data.get(6..10)?) as usize;
    let end = 10 + size;
    if end > data.len() {
        return Some(false);
    }
    let mut position = 10;
    if major == 2 {
        // ID3v2.2 frames: 3-char id, 3-byte size, no flags.
        while position + 6 <= end {
            let id = data.get(position..position + 3)?;
            if id == b"\0\0\0" {
                break;
            }
            let length = ((data[position + 3] as usize) << 16)
                | ((data[position + 4] as usize) << 8)
                | data[position + 5] as usize;
            if id == b"PIC" {
                return Some(true);
            }
            position += 6 + length;
        }
        return Some(false);
    }
    while position + 10 <= end {
        let header = data.get(position..position + 10)?;
        if header[0] == 0 {
            break;
        }
        let length = if major == 4 {
            syncsafe(&header[4..8]) as usize
        } else {
            u32::from_be_bytes(header[4..8].try_into().ok()?) as usize
        };
        if length == 0 || position + 10 + length > end {
            break;
        }
        if &header[..4] == b"APIC" {
            return Some(true);
        }
        position += 10 + length;
    }
    Some(false)
}

fn flac_has_picture(data: &[u8]) -> Option<bool> {
    let mut position = 4;
    loop {
        let header = *data.get(position)?;
        let kind = header & 0x7f;
        let length = ((*data.get(position + 1)? as usize) << 16)
            | ((*data.get(position + 2)? as usize) << 8)
            | *data.get(position + 3)? as usize;
        position += 4;
        if position + length > data.len() {
            return Some(false);
        }
        if kind == 6 {
            return Some(true);
        }
        position += length;
        if header & 0x80 != 0 {
            return Some(false);
        }
    }
}

/// Returns the picture bytes (JPEG or PNG) embedded in an MP3 (ID3v2.2 to
/// v2.4 picture frame) or FLAC (PICTURE block), or `None` when there is none
/// or the tag is not one this can read. Read-only and forgiving: a malformed
/// tag is "no picture", never an error, because this only feeds a preview.
pub fn extract_embedded_cover(path: &Path) -> Option<Vec<u8>> {
    let data = read_tag_region(path).ok()??;
    if data.starts_with(b"ID3") {
        return id3_picture(&data);
    }
    if data.starts_with(b"fLaC") {
        return flac_picture(&data);
    }
    None
}

/// The most a tag/metadata region may be before it is treated as junk.
const MAX_TAG_REGION: u64 = 16 * 1024 * 1024;

/// Reads only the leading tag region of an audio file: an ID3v2 tag (its own
/// declared size) or a FLAC file's metadata blocks (walked by header, bodies
/// skipped with a seek). The audio itself is never read, so a cover preview
/// costs kilobytes instead of a whole 5-40 MB track. This matters most on a
/// slow link: over the Pocket's USB mode (about 0.7 MB/s) reading whole files
/// for every album froze the window for over a minute. `Ok(None)` means the
/// file has no ID3 or FLAC tag region; an `Err` is a real read failure.
fn read_tag_region(path: &Path) -> Result<Option<Vec<u8>>, std::io::Error> {
    let mut file = fs::File::open(path)?;
    let mut head = [0u8; 10];
    if file.read_exact(&mut head).is_err() {
        return Ok(None);
    }
    if head.starts_with(b"ID3") {
        let total = (10 + u64::from(syncsafe(&head[6..10]))).min(MAX_TAG_REGION);
        let mut buffer = head.to_vec();
        file.take(total - 10).read_to_end(&mut buffer)?;
        return Ok(Some(buffer));
    }
    if head.starts_with(b"fLaC") {
        // Block headers start right after the 4-byte marker.
        let mut position = 4u64;
        file.seek(SeekFrom::Start(position))?;
        for _ in 0..1024 {
            let mut block = [0u8; 4];
            if file.read_exact(&mut block).is_err() {
                return Ok(None);
            }
            let length = (u64::from(block[1]) << 16) | (u64::from(block[2]) << 8) | u64::from(block[3]);
            position += 4 + length;
            if position > MAX_TAG_REGION {
                return Ok(None);
            }
            if block[0] & 0x80 != 0 {
                file.seek(SeekFrom::Start(0))?;
                let mut buffer = Vec::with_capacity(position as usize);
                file.take(position).read_to_end(&mut buffer)?;
                return Ok(Some(buffer));
            }
            file.seek(SeekFrom::Start(position))?;
        }
    }
    Ok(None)
}

/// Skips a text field ended by a terminator suitable for `encoding`, returning the index after it.
fn skip_text(data: &[u8], from: usize, encoding: u8) -> Option<usize> {
    if matches!(encoding, 1 | 2) {
        let mut i = from;
        while i + 1 < data.len() {
            if data[i] == 0 && data[i + 1] == 0 {
                return Some(i + 2);
            }
            i += 2;
        }
        None
    } else {
        data.get(from..)?.iter().position(|b| *b == 0).map(|n| from + n + 1)
    }
}

fn id3_picture(data: &[u8]) -> Option<Vec<u8>> {
    let major = *data.get(3)?;
    if !matches!(major, 2..=4) {
        return None;
    }
    let end = (10 + syncsafe(data.get(6..10)?) as usize).min(data.len());
    let mut position = 10;
    while position + if major == 2 { 6 } else { 10 } <= end {
        let (id, length, header) = if major == 2 {
            let id = data.get(position..position + 3)?;
            (id.to_vec(), ((data[position + 3] as usize) << 16) | ((data[position + 4] as usize) << 8) | data[position + 5] as usize, 6)
        } else {
            let header = data.get(position..position + 10)?;
            let length = if major == 4 { syncsafe(&header[4..8]) as usize } else { u32::from_be_bytes(header[4..8].try_into().ok()?) as usize };
            (header[..4].to_vec(), length, 10)
        };
        if id[0] == 0 || length == 0 || position + header + length > end {
            return None;
        }
        if id == b"APIC" || id == b"PIC" {
            let body = &data[position + header..position + header + length];
            let encoding = *body.first()?;
            let mut i = 1;
            if major == 2 {
                i += 3; // three-letter image format
            } else {
                i = skip_text(body, i, 0)?; // MIME type, always ISO-8859-1
            }
            i += 1; // picture type
            i = skip_text(body, i, encoding)?; // description
            return body.get(i..).filter(|b| !b.is_empty()).map(<[u8]>::to_vec);
        }
        position += header + length;
    }
    None
}

fn flac_picture(data: &[u8]) -> Option<Vec<u8>> {
    let mut position = 4;
    loop {
        let header = *data.get(position)?;
        let length = ((*data.get(position + 1)? as usize) << 16) | ((*data.get(position + 2)? as usize) << 8) | *data.get(position + 3)? as usize;
        position += 4;
        let body = data.get(position..position + length)?;
        if header & 0x7f == 6 {
            let read_u32 = |at: usize| -> Option<usize> { Some(u32::from_be_bytes(body.get(at..at + 4)?.try_into().ok()?) as usize) };
            let mime = read_u32(4)?;
            let desc_at = 8 + mime;
            let desc = read_u32(desc_at)?;
            let data_len_at = desc_at + 4 + desc + 16; // width, height, depth, colours
            let picture = read_u32(data_len_at)?;
            return body.get(data_len_at + 4..data_len_at + 4 + picture).map(<[u8]>::to_vec);
        }
        position += length;
        if header & 0x80 != 0 {
            return None;
        }
    }
}

/// Embeds a baseline JPEG as the only MP3 APIC frame in a copied file.
/// Existing non-art ID3 frames and audio bytes are retained verbatim.
pub fn embed_mp3_copy(source: &Path, cover: &Path, output: &Path) -> Result<(), TauError> {
    fs::write(output, embed_mp3_bytes(source, cover)?)?;
    Ok(())
}

/// The bytes `embed_mp3_copy` would write, so a caller can hash them first and
/// prove that what reached the card is exactly what was intended.
pub fn embed_mp3_bytes(source: &Path, cover: &Path) -> Result<Vec<u8>, TauError> {
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
    Ok(result)
}
/// Writes a JPEG PICTURE block into a FLAC copy, retaining audio and all
/// metadata except old PICTURE and PADDING blocks.
pub fn embed_flac_copy(source: &Path, cover: &Path, output: &Path) -> Result<(), TauError> {
    fs::write(output, embed_flac_bytes(source, cover)?)?;
    Ok(())
}

/// The bytes `embed_flac_copy` would write (see [`embed_mp3_bytes`]).
pub fn embed_flac_bytes(source: &Path, cover: &Path) -> Result<Vec<u8>, TauError> {
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
    Ok(result)
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
    /// A FLAC file: STREAMINFO, then a PICTURE block, then lots of "audio".
    fn flac_with_picture(picture: &[u8], audio_bytes: usize) -> Vec<u8> {
        let mut body = Vec::new();
        for value in [3u32, 9] {
            body.extend(value.to_be_bytes());
            if value == 9 {
                body.extend(b"image/jpg");
            }
        }
        body.extend(0u32.to_be_bytes()); // empty description
        for _ in 0..4 {
            body.extend(0u32.to_be_bytes()); // width, height, depth, colours
        }
        body.extend((picture.len() as u32).to_be_bytes());
        body.extend(picture);
        let mut file = b"fLaC".to_vec();
        file.extend([0x00, 0, 0, 34]);
        file.extend([0u8; 34]);
        file.extend([0x80 | 6, (body.len() >> 16) as u8, (body.len() >> 8) as u8, body.len() as u8]);
        file.extend(body);
        file.extend(vec![0xAA; audio_bytes]);
        file
    }
    #[test]
    fn extracts_an_embedded_cover_from_the_tag_region_only() {
        let root = std::env::temp_dir().join(format!("tau-tagregion-{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() + crate::test_uniq()));
        fs::create_dir_all(&root).unwrap();
        let picture = [0xff, 0xd8, 0xff, 0xd9];
        // MP3: a real embedded cover followed by 3 MB of audio.
        let (source, cover, output) = (root.join("s.mp3"), root.join("c.jpg"), root.join("o.mp3"));
        fs::write(&source, vec![0x55u8; 3 << 20]).unwrap();
        fs::write(&cover, picture).unwrap();
        embed_mp3_copy(&source, &cover, &output).unwrap();
        assert_eq!(extract_embedded_cover(&output).as_deref(), Some(&picture[..]));
        assert!(has_embedded_cover(&output).unwrap());
        // FLAC, same shape.
        let flac = root.join("t.flac");
        fs::write(&flac, flac_with_picture(&picture, 3 << 20)).unwrap();
        assert_eq!(extract_embedded_cover(&flac).as_deref(), Some(&picture[..]));
        assert!(has_embedded_cover(&flac).unwrap());
        // The tag region of both is tiny compared with the files.
        assert!(read_tag_region(&output).unwrap().unwrap().len() < 1024);
        assert!(read_tag_region(&flac).unwrap().unwrap().len() < 1024);
        // Plain audio without a tag, and a too-short file, are "no cover", not errors.
        let plain = root.join("plain.mp3");
        fs::write(&plain, vec![0u8; 4096]).unwrap();
        assert_eq!(extract_embedded_cover(&plain), None);
        assert!(!has_embedded_cover(&plain).unwrap());
        let tiny = root.join("tiny.mp3");
        fs::write(&tiny, b"ID3").unwrap();
        assert_eq!(extract_embedded_cover(&tiny), None);
        // A missing file is still a real error for has_embedded_cover.
        assert!(has_embedded_cover(&root.join("missing.mp3")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn embeds_only_in_copy() {
        let root = std::env::temp_dir().join(format!(
            "tau-cover-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() + crate::test_uniq()
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
                .as_nanos() + crate::test_uniq()
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
                .as_nanos() + crate::test_uniq()
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
