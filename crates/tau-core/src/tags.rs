//! Tag and duration readers for the files a Tau library indexes: ID3v2/ID3v1 (MP3) and FLAC Vorbis comments.
//! Split out of `lib.rs` unchanged; the scanner in `lib.rs` is the only caller.

use crate::TauError;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Seek, SeekFrom},
};

pub(crate) fn read_id3v2(f: &mut fs::File) -> Result<(BTreeMap<String, String>, u64), TauError> {
    let mut h = [0; 10];
    f.seek(SeekFrom::Start(0))?;
    if f.read_exact(&mut h).is_err() || &h[..3] != b"ID3" || !matches!(h[3], 3 | 4) {
        return Ok((BTreeMap::new(), 0));
    }
    let size = syncsafe(&h[6..10]) as u64;
    let end = 10 + size;
    let ver = h[3];
    let mut pos = 10;
    let mut tags = BTreeMap::new();
    if h[5] & 0x40 != 0 {
        let mut extra = [0; 4];
        f.read_exact(&mut extra)?;
        let n = if ver == 4 {
            syncsafe(&extra) as u64
        } else {
            u32::from_be_bytes(extra) as u64 + 4
        };
        pos = 10 + n;
    }
    while pos + 10 <= end {
        f.seek(SeekFrom::Start(pos))?;
        let mut fh = [0; 10];
        if f.read_exact(&mut fh).is_err() || fh[0] == 0 {
            break;
        }
        let len = if ver == 4 {
            syncsafe(&fh[4..8]) as u64
        } else {
            u32::from_be_bytes(fh[4..8].try_into().unwrap()) as u64
        };
        if len == 0 || pos + 10 + len > end {
            break;
        }
        let id = String::from_utf8_lossy(&fh[..4]);
        if matches!(
            id.as_ref(),
            "TIT2" | "TPE1" | "TPE2" | "TALB" | "TRCK" | "TPOS" | "TYER" | "TDRC"
        ) && len < 4096
        {
            let mut b = vec![0; len as usize];
            f.read_exact(&mut b)?;
            if let Some(value) = decode_id3_text(&b) {
                tags.entry(id.to_string()).or_insert(value);
            }
        }
        pos += 10 + len;
    }
    Ok((tags, end))
}
fn syncsafe(b: &[u8]) -> u32 {
    (u32::from(b[0]) << 21) | (u32::from(b[1]) << 14) | (u32::from(b[2]) << 7) | u32::from(b[3])
}
/// Decodes an ID3v2 text frame body (encoding byte, then text) the way the
/// reference does (`tau-alpha/tools/tau_library.py` `_dec`): **decode the whole
/// body first, then cut at the first NUL character.** Cutting the bytes at the
/// first zero byte before decoding (what this used to do) destroys every UTF-16
/// string, because each ASCII letter in UTF-16 has a zero byte: "Kind of Blue"
/// became just a byte-order mark, so 266 of 341 albums in a real library listed
/// with blank titles and artists, and the index written for them was blank too.
///
/// Encodings: 0 = Latin-1; 1 = UTF-16 where a BOM picks the byte order and no BOM
/// means little-endian; 2 = UTF-16 big-endian (a BOM is kept as U+FEFF, as in the
/// reference); anything else = UTF-8. Text that does not decode in its declared
/// encoding falls back to Latin-1, like the reference.
fn decode_id3_text(b: &[u8]) -> Option<String> {
    let (&enc, body) = b.split_first()?;
    let latin1 = |bytes: &[u8]| -> String { bytes.iter().map(|x| char::from(*x)).collect() };
    let utf16 = |bytes: &[u8], big: bool| -> Option<String> {
        if !bytes.len().is_multiple_of(2) {
            return None;
        }
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| {
                if big {
                    u16::from_be_bytes(*p)
                } else {
                    u16::from_le_bytes(*p)
                }
            })
            .collect();
        String::from_utf16(&units).ok()
    };
    let text = match enc {
        0 => latin1(body),
        1 => {
            let (rest, big) = match body {
                [0xff, 0xfe, rest @ ..] => (rest, false),
                [0xfe, 0xff, rest @ ..] => (rest, true),
                _ => (body, false),
            };
            utf16(rest, big).unwrap_or_else(|| latin1(body))
        }
        2 => utf16(body, true).unwrap_or_else(|| latin1(body)),
        _ => String::from_utf8(body.to_vec()).unwrap_or_else(|_| latin1(body)),
    };
    Some(
        text.split('\0')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string(),
    )
}
pub(crate) fn read_id3v1(
    f: &mut fs::File,
    size: u64,
) -> Result<BTreeMap<String, String>, TauError> {
    let mut out = BTreeMap::new();
    if size < 128 {
        return Ok(out);
    }
    f.seek(SeekFrom::End(-128))?;
    let mut b = [0; 128];
    f.read_exact(&mut b)?;
    if &b[..3] != b"TAG" {
        return Ok(out);
    }
    for (key, range) in [
        ("TIT2", 3..33),
        ("TPE1", 33..63),
        ("TALB", 63..93),
        ("TYER", 93..97),
    ] {
        let val = String::from_utf8_lossy(&b[range])
            .trim_matches(char::from(0))
            .trim()
            .to_string();
        if !val.is_empty() {
            out.insert(key.into(), val);
        }
    }
    if b[125] == 0 && b[126] != 0 {
        out.insert("TRCK".into(), b[126].to_string());
    }
    Ok(out)
}
pub(crate) fn mp3_seconds(
    f: &mut fs::File,
    start: u64,
    size: u64,
) -> Result<Option<u64>, TauError> {
    f.seek(SeekFrom::Start(start))?;
    let mut buf = vec![0; 8192];
    let n = f.read(&mut buf)?;
    buf.truncate(n);
    for i in 0..buf.len().saturating_sub(4) {
        if buf[i] == 0xff && (buf[i + 1] & 0xe0) == 0xe0 {
            let (b1, b2, b3) = (buf[i + 1], buf[i + 2], buf[i + 3]);
            let (ver, layer) = ((b1 >> 3) & 3, (b1 >> 1) & 3);
            if ver == 1 || layer != 1 || (b2 >> 4) == 0 || (b2 >> 4) == 15 || ((b2 >> 2) & 3) == 3 {
                continue;
            }
            let br_table = if ver == 3 {
                [
                    0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
                ]
            } else {
                [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160]
            };
            let sr_table = match ver {
                3 => [44100, 48000, 32000],
                2 => [22050, 24000, 16000],
                _ => [11025, 12000, 8000],
            };
            let br = br_table[(b2 >> 4) as usize] as u64 * 1000;
            let sr = sr_table[((b2 >> 2) & 3) as usize] as u64;
            let spf = if ver == 3 { 1152 } else { 576 };
            let side = if ver == 3 {
                if (b3 >> 6) == 3 { 17 } else { 32 }
            } else if (b3 >> 6) == 3 {
                9
            } else {
                17
            };
            let x = i + 4 + side;
            if buf
                .get(x..x + 4)
                .is_some_and(|v| v == b"Xing" || v == b"Info")
                && x + 12 <= buf.len()
                && buf[x + 7] & 1 != 0
            {
                let frames = u32::from_be_bytes(buf[x + 8..x + 12].try_into().unwrap()) as u64;
                return Ok(Some(frames * spf / sr));
            }
            return Ok((br > 0).then_some(size.saturating_sub(start) * 8 / br));
        }
    }
    Ok(Some(0))
}
pub(crate) fn read_flac(f: &mut fs::File) -> Result<(BTreeMap<String, String>, u64), TauError> {
    let mut magic = [0; 4];
    f.read_exact(&mut magic)?;
    if &magic != b"fLaC" {
        return Ok((BTreeMap::new(), 0));
    }
    let (mut tags, mut secs) = (BTreeMap::new(), 0);
    loop {
        let mut h = [0; 4];
        if f.read_exact(&mut h).is_err() {
            break;
        }
        let (last, kind, len) = (
            h[0] & 0x80 != 0,
            h[0] & 0x7f,
            u32::from_be_bytes([0, h[1], h[2], h[3]]) as usize,
        );
        let mut body = vec![0; len];
        f.read_exact(&mut body)?;
        if kind == 0 && len >= 18 {
            let sr =
                ((body[10] as u64) << 12) | ((body[11] as u64) << 4) | u64::from(body[12] >> 4);
            let total = ((u64::from(body[13] & 15)) << 32)
                | u64::from(u32::from_be_bytes(body[14..18].try_into().unwrap()));
            secs = total.checked_div(sr).unwrap_or(0);
        } else if kind == 4 && len >= 8 {
            let vendor = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
            if vendor + 8 <= len {
                let mut p = 4 + vendor;
                let n = u32::from_le_bytes(body[p..p + 4].try_into().unwrap()) as usize;
                p += 4;
                for _ in 0..n {
                    if p + 4 > len {
                        break;
                    }
                    let n = u32::from_le_bytes(body[p..p + 4].try_into().unwrap()) as usize;
                    p += 4;
                    if p + n > len {
                        break;
                    }
                    let s = String::from_utf8_lossy(&body[p..p + n]);
                    p += n;
                    if let Some((k, v)) = s.split_once('=') {
                        let id = match k.to_ascii_uppercase().as_str() {
                            "TITLE" => "TIT2",
                            "ARTIST" => "TPE1",
                            "ALBUMARTIST" => "TPE2",
                            "ALBUM" => "TALB",
                            "TRACKNUMBER" => "TRCK",
                            "DISCNUMBER" => "TPOS",
                            "DATE" => "TYER",
                            _ => "",
                        };
                        if !id.is_empty() {
                            tags.entry(id.into()).or_insert(v.trim().into());
                        }
                    }
                }
            }
        }
        if last {
            break;
        }
    }
    Ok((tags, secs))
}

#[cfg(test)]
mod id3_text_tests {
    use super::decode_id3_text;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// Every expected value below was produced by running the reference
    /// (`tau-alpha/tools/tau_library.py`, `_dec(...).split("\x00")[0].strip()`) on the same
    /// bytes, so this is a conformance check, not a restatement of the Rust code.
    #[test]
    fn id3_text_decodes_exactly_like_the_reference() {
        let cases: &[(&str, &str, &str)] = &[
            (
                "utf16le_bom",
                "01fffe4b0069006e00640020006f006600200042006c0075006500",
                "Kind of Blue",
            ),
            (
                "utf16be_bom",
                "01feff004700f60074007400650072006400e4006d006d006500720075006e0067",
                "Götterdämmerung",
            ),
            ("utf16le_nobom", "0150006c00610069006e00", "Plain"),
            (
                "utf16_nul_terminated",
                "01fffe41006200630000006a0075006e006b00",
                "Abc",
            ),
            ("utf16_empty_bom_only", "01fffe", ""),
            ("utf16_odd_length", "01fffe4800690041", "\u{ff}\u{fe}H"),
            ("enc2_be", "020042006500740061", "Beta"),
            ("enc2_be_bom_kept", "02feff0042006500740061", "\u{feff}Beta"),
            ("latin1", "00436166e92020", "Café"),
            ("latin1_nul", "0041626300646566", "Abc"),
            ("utf8", "03c39c6ec3af636f6465", "Ünïcode"),
            ("utf8_invalid", "036162fffe6364", "ab\u{ff}\u{fe}cd"),
            ("utf8_nul", "034f6e650054776f", "One"),
            ("utf16_cjk", "01fffee5652c679e8a", "日本語"),
            ("utf16_surrogate_pair", "01fffe41003dd800de4200", "A😀B"),
            ("utf16_lone_surrogate", "01fffe00d84100", "\u{ff}\u{fe}"),
        ];
        for (name, bytes, expected) in cases {
            assert_eq!(
                decode_id3_text(&hex(bytes)).as_deref(),
                Some(*expected),
                "{name}"
            );
        }
        assert_eq!(decode_id3_text(&[]), None);
    }

    /// The end-to-end symptom: a file whose tags are UTF-16 (iTunes, Windows Media Player
    /// and many taggers write these) must show its real title and artist, never a BOM.
    #[test]
    fn a_utf16_tagged_file_lists_its_real_title_and_artist() {
        fn frame(id: &str, text: &str) -> Vec<u8> {
            let mut body = vec![1u8, 0xff, 0xfe];
            body.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
            let mut f = id.as_bytes().to_vec();
            f.extend((body.len() as u32).to_be_bytes());
            f.extend([0, 0]);
            f.extend(body);
            f
        }
        let mut frames = frame("TIT2", "So What");
        frames.extend(frame("TPE1", "Miles Davis"));
        frames.extend(frame("TALB", "Kind of Blue"));
        let size = frames.len();
        let mut file = b"ID3\x03\x00\x00".to_vec();
        file.extend([
            (size >> 21 & 0x7f) as u8,
            (size >> 14 & 0x7f) as u8,
            (size >> 7 & 0x7f) as u8,
            (size & 0x7f) as u8,
        ]);
        file.extend(frames);
        file.extend(vec![0u8; 2048]);
        let dir = std::env::temp_dir().join(format!(
            "tau-id3-utf16-{}",
            std::process::id() as u128 + crate::test_uniq()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("01 So What.mp3");
        std::fs::write(&path, file).unwrap();
        let (tags, _, _) = crate::read_tags(&path).unwrap();
        assert_eq!(tags.get("TIT2").map(String::as_str), Some("So What"));
        assert_eq!(tags.get("TPE1").map(String::as_str), Some("Miles Davis"));
        assert_eq!(tags.get("TALB").map(String::as_str), Some("Kind of Blue"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
