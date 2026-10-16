//! Per-track technical details for the library index's optional ninth section ("TDET", tau-alpha's
//! `docs/features/TRACK_DETAILS_SPEC.md`, reference tool `tools/tau_details.py`). Read-only: opens the
//! audio file, never decodes it. One fixed 32-byte record per track; the index writer in `build.rs`
//! stores the section, the player's track details page reads it.
//!
//! The extraction follows the reference tool field for field (including its quirks), and is checked
//! against records the reference tool produced (`tests/details_golden.rs`).

use crate::error::{ErrorCode, TauError};
use std::fs;
use std::path::Path;

pub const RECORD: usize = 32;
pub const VERSION: u16 = 1;
/// Version 2 appends `lead_skip` and `tail_skip` (u32 each, source pairs) to every record. Written only when at
/// least one track has a skip, so an index without trimming stays byte for byte what version 1 produces.
pub const VERSION_SKIP: u16 = 2;
pub const RECORD_SKIP: usize = 40;
pub const F_VBR: u8 = 1;
pub const F_LAME: u8 = 2;
pub const F_SIDECAR: u8 = 4;
pub const F_COVER: u8 = 8;
pub const F_RG_TRACK_GAIN: u8 = 16;
pub const F_RG_ALBUM_GAIN: u8 = 32;
pub const F_RG_TRACK_PEAK: u8 = 64;
pub const F_RG_ALBUM_PEAK: u8 = 128;
/// The cover file the player looks for next to the tracks of an album.
pub const SIDECAR: &str = "tau-art/cover_128.pal256.timg";

const BITRATE: [[u16; 15]; 2] = [
    [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
    ],
    [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
];

/// One track's details. `encoder` is the encoder or vendor text (stored in the index string pool).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Details {
    pub codec: u8,
    pub channels: u8,
    pub bits: u8,
    pub flags: u8,
    pub rate: u32,
    pub size: u32,
    pub kbps: u16,
    pub cover_kib: u16,
    pub gain_track: i16,
    pub gain_album: i16,
    pub peak_track: u16,
    pub peak_album: u16,
    pub delay: u16,
    pub padding: u16,
    pub encoder: String,
}

impl Details {
    /// The 28 record bytes before the encoder string offset, as hex (carried in the scan's tag map).
    pub fn to_hex(&self) -> String {
        let mut out = Vec::with_capacity(28);
        out.extend([self.codec, self.channels, self.bits, self.flags]);
        out.extend(self.rate.to_le_bytes());
        out.extend(self.size.to_le_bytes());
        out.extend(self.kbps.to_le_bytes());
        out.extend(self.cover_kib.to_le_bytes());
        out.extend(self.gain_track.to_le_bytes());
        out.extend(self.gain_album.to_le_bytes());
        out.extend(self.peak_track.to_le_bytes());
        out.extend(self.peak_album.to_le_bytes());
        out.extend(self.delay.to_le_bytes());
        out.extend(self.padding.to_le_bytes());
        out.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn from_hex(hex: &str, encoder: &str) -> Option<Self> {
        if hex.len() != 56 || !hex.is_ascii() {
            return None;
        }
        let b: Vec<u8> = (0..28)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16))
            .collect::<Result<_, _>>()
            .ok()?;
        let u16at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
        let i16at = |i: usize| i16::from_le_bytes([b[i], b[i + 1]]);
        let u32at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        Some(Self {
            codec: b[0],
            channels: b[1],
            bits: b[2],
            flags: b[3],
            rate: u32at(4),
            size: u32at(8),
            kbps: u16at(12),
            cover_kib: u16at(14),
            gain_track: i16at(16),
            gain_album: i16at(18),
            peak_track: u16at(20),
            peak_album: u16at(22),
            delay: u16at(24),
            padding: u16at(26),
            encoder: encoder.to_string(),
        })
    }

    /// The 32-byte record, `encoder_offset` being the encoder text's offset in the index string pool.
    pub fn pack(&self, encoder_offset: u32) -> [u8; RECORD] {
        let mut out = [0u8; RECORD];
        let hex = self.to_hex();
        for i in 0..28 {
            out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or(0);
        }
        out[28..32].copy_from_slice(&encoder_offset.to_le_bytes());
        out
    }
}

/// The whole section (header plus records) for `records` in index track order.
pub fn section(records: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + records.len());
    out.extend(b"TDET");
    out.extend(VERSION.to_le_bytes());
    out.extend((RECORD as u16).to_le_bytes());
    out.extend(((records.len() / RECORD) as u32).to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(records);
    out
}

/// The version 2 section: the 32-byte `records` of every track plus its `(lead_skip, tail_skip)`.
pub fn section_skip(records: &[u8], skips: &[(u32, u32)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + skips.len() * RECORD_SKIP);
    out.extend(b"TDET");
    out.extend(VERSION_SKIP.to_le_bytes());
    out.extend((RECORD_SKIP as u16).to_le_bytes());
    out.extend((skips.len() as u32).to_le_bytes());
    out.extend(0u32.to_le_bytes());
    for (rec, (lead, tail)) in records.as_chunks::<RECORD>().0.iter().zip(skips) {
        out.extend(rec);
        out.extend(lead.to_le_bytes());
        out.extend(tail.to_le_bytes());
    }
    out
}

fn bad(message: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::UnsupportedTags, message)
}

/// Rounds half to even like the reference tool's Python `round`.
fn round_even(x: f64) -> i64 {
    x.round_ties_even() as i64
}

fn parse_gain_cdb(text: &str) -> Option<i16> {
    let t = text.trim_start();
    let bytes = t.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let digits_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let whole = i > digits_start;
    let mut frac = false;
    if i < bytes.len() && bytes[i] == b'.' {
        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if whole {
            i = j; // "12." and "12.5" both match
            frac = true;
        } else if j > i + 1 {
            i = j; // ".5"
            frac = true;
        }
    }
    if !whole && !frac {
        return None;
    }
    let value: f64 = t[..i].parse().ok()?;
    Some(round_even(value * 100.0).clamp(-3000, 1800) as i16)
}

/// Same rule as the firmware's `rg_parse_peak_q15`: four decimals, rounded up, whole part at most 7.
fn parse_peak_q15(text: &str) -> Option<u16> {
    let t = text.trim_start();
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let whole_text = &t[..i];
    let mut frac_text = "";
    if i < bytes.len() && bytes[i] == b'.' {
        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        frac_text = &t[i + 1..j];
    }
    if whole_text.is_empty() && frac_text.is_empty() {
        return None;
    }
    let mut whole: u64 =
        whole_text
            .parse()
            .unwrap_or(if whole_text.is_empty() { 0 } else { u64::MAX });
    let mut frac = frac_text.to_string();
    if whole > 7 {
        whole = 7;
        frac = "9999".into();
    }
    let padded = format!("{frac}0000");
    let mut f4: u64 = padded[..4].parse().unwrap_or(0);
    if frac.len() > 4 && frac[4..].bytes().any(|c| c != b'0') {
        f4 += 1;
    }
    if f4 > 9999 {
        whole += 1;
        f4 = 0;
    }
    if whole > 7 {
        whole = 7;
        f4 = 9999;
    }
    let q15 = ((whole * 10000 + f4) * 32768).div_ceil(10000);
    Some(q15.min(65535) as u16)
}

fn synchsafe(b: &[u8]) -> usize {
    ((b[0] as usize) << 21) | ((b[1] as usize) << 14) | ((b[2] as usize) << 7) | b[3] as usize
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn utf16(bytes: &[u8]) -> String {
    let (big, body) = match bytes {
        [0xFE, 0xFF, rest @ ..] => (true, rest),
        [0xFF, 0xFE, rest @ ..] => (false, rest),
        _ => (false, bytes),
    };
    let units: Vec<u16> = (0..body.len() / 2)
        .map(|i| {
            let pair = [body[2 * i], body[2 * i + 1]];
            if big {
                u16::from_be_bytes(pair)
            } else {
                u16::from_le_bytes(pair)
            }
        })
        .collect();
    String::from_utf16(&units).unwrap_or_default()
}

fn strip_nul_space(s: &str) -> &str {
    s.trim_matches(|c| c == '\0' || c == ' ')
}

#[derive(Default)]
struct Rg {
    track_gain: Option<String>,
    album_gain: Option<String>,
    track_peak: Option<String>,
    album_peak: Option<String>,
}

impl Rg {
    fn set(&mut self, key: &str, value: &str) {
        let slot = match key.to_uppercase().as_str() {
            "REPLAYGAIN_TRACK_GAIN" => &mut self.track_gain,
            "REPLAYGAIN_ALBUM_GAIN" => &mut self.album_gain,
            "REPLAYGAIN_TRACK_PEAK" => &mut self.track_peak,
            "REPLAYGAIN_ALBUM_PEAK" => &mut self.album_peak,
            _ => return,
        };
        *slot = Some(value.to_string());
    }

    fn into(self, d: &mut Details) {
        if let Some(v) = self
            .track_gain
            .as_deref()
            .filter(|v| !v.is_empty())
            .and_then(parse_gain_cdb)
        {
            d.gain_track = v;
            d.flags |= F_RG_TRACK_GAIN;
        }
        if let Some(v) = self
            .album_gain
            .as_deref()
            .filter(|v| !v.is_empty())
            .and_then(parse_gain_cdb)
        {
            d.gain_album = v;
            d.flags |= F_RG_ALBUM_GAIN;
        }
        if let Some(v) = self
            .track_peak
            .as_deref()
            .filter(|v| !v.is_empty())
            .and_then(parse_peak_q15)
        {
            d.peak_track = v;
            d.flags |= F_RG_TRACK_PEAK;
        }
        if let Some(v) = self
            .album_peak
            .as_deref()
            .filter(|v| !v.is_empty())
            .and_then(parse_peak_q15)
        {
            d.peak_album = v;
            d.flags |= F_RG_ALBUM_PEAK;
        }
    }
}

fn be32(b: &[u8], at: usize) -> Result<u32, TauError> {
    b.get(at..at + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| bad("truncated header"))
}

fn le32(b: &[u8], at: usize) -> Result<u32, TauError> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| bad("truncated tag"))
}

fn mp3(data: &[u8], size: u64) -> Result<Details, TauError> {
    let mut d = Details {
        codec: 1,
        channels: 2,
        size: size.min(u32::MAX as u64) as u32,
        ..Details::default()
    };
    let mut rg = Rg::default();
    let mut start = 0usize;
    if data.len() >= 10 && &data[..3] == b"ID3" && (data[3] == 3 || data[3] == 4) {
        let end = 10 + synchsafe(&data[6..10]);
        start = end;
        let version = data[3];
        let mut pos = 10usize;
        while pos + 10 <= end {
            let Some(fh) = data.get(pos..pos + 10) else {
                break;
            };
            if fh[0] == 0 {
                break;
            }
            let id = latin1(&fh[..4]);
            let fsz = if version == 4 {
                synchsafe(&fh[4..8])
            } else {
                be32(fh, 4)? as usize
            };
            if fsz == 0 || pos + 10 + fsz > end {
                break;
            }
            if id == "APIC" {
                d.cover_kib = fsz.div_ceil(1024).min(65535) as u16;
            } else if id == "TXXX" && fsz < 96 {
                let body = data
                    .get(pos + 10..(pos + 10 + fsz).min(data.len()))
                    .unwrap_or(&[]);
                if let Some((&enc, text)) = body.split_first() {
                    let text = match enc {
                        1 | 2 => utf16(text),
                        0 => latin1(text),
                        _ => String::from_utf8_lossy(text).into_owned(),
                    };
                    let (desc, value) = text.split_once('\0').unwrap_or((text.as_str(), ""));
                    rg.set(desc.trim(), strip_nul_space(value));
                }
            }
            pos += 10 + fsz;
        }
    }
    let buf = data.get(start.min(data.len())..).unwrap_or(&[]);
    let buf = &buf[..buf.len().min(16384)];
    for i in 0..buf.len().saturating_sub(4) {
        if buf[i] != 0xFF || buf[i + 1] & 0xE0 != 0xE0 {
            continue;
        }
        let (b1, b2, b3) = (buf[i + 1], buf[i + 2], buf[i + 3]);
        let version = (b1 >> 3) & 3;
        let layer = (b1 >> 1) & 3;
        if version == 1 || layer != 1 || matches!(b2 >> 4, 0 | 15) || (b2 >> 2) & 3 == 3 {
            continue;
        }
        let family = if version == 3 { 0 } else { 1 };
        let rate_index = ((b2 >> 2) & 3) as usize;
        let rate: u32 = match version {
            3 => [44100, 48000, 32000][rate_index],
            2 => [22050, 24000, 16000][rate_index],
            _ => [11025, 12000, 8000][rate_index],
        };
        let spf = if version == 3 { 1152.0 } else { 576.0 };
        let mono = b3 >> 6 == 3;
        d.channels = if mono { 1 } else { 2 };
        d.rate = rate;
        d.kbps = BITRATE[family][(b2 >> 4) as usize];
        let side = if version == 3 {
            if mono { 17 } else { 32 }
        } else if mono {
            9
        } else {
            17
        };
        let x = i + 4 + side;
        let tag = buf.get(x..x + 4);
        if (tag == Some(b"Xing") || tag == Some(b"Info")) && x + 12 <= buf.len() {
            if tag == Some(b"Xing") {
                d.flags |= F_VBR;
            }
            let fl = be32(buf, x + 4)?;
            let mut e = x + 8;
            let (mut frames, mut nbytes) = (0u32, 0u32);
            if fl & 1 != 0 {
                frames = be32(buf, e)?;
                e += 4;
            }
            if fl & 2 != 0 {
                nbytes = be32(buf, e)?;
                e += 4;
            }
            if fl & 4 != 0 {
                e += 100;
            }
            if fl & 8 != 0 {
                e += 4;
            }
            if frames != 0 && rate != 0 {
                let secs = frames as f64 * spf / rate as f64;
                let bytes = if nbytes != 0 {
                    nbytes as u64
                } else {
                    size.saturating_sub(start as u64)
                };
                d.kbps = round_even(bytes as f64 * 8.0 / secs / 1000.0).clamp(0, 65535) as u16;
            }
            let ext = buf.get(e..(e + 24).min(buf.len())).unwrap_or(&[]);
            if ext.len() >= 24 && ext[..4].iter().all(|c| (0x20..0x7F).contains(c)) {
                d.flags |= F_LAME;
                let name: String = ext[..9]
                    .iter()
                    .map(|&c| if c < 0x80 { c as char } else { '\u{FFFD}' })
                    .collect();
                d.encoder = strip_nul_space(&name).to_string();
                d.delay = ((ext[21] as u16) << 4) | (ext[22] as u16 >> 4);
                d.padding = (((ext[22] & 15) as u16) << 8) | ext[23] as u16;
            }
        }
        break;
    }
    rg.into(&mut d);
    Ok(d)
}

fn read_exact_or_none(f: &mut fs::File, n: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut buf = vec![0u8; n];
    let mut got = 0;
    while got < n {
        match f.read(&mut buf[got..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => got += k,
        }
    }
    buf.truncate(got);
    Some(buf)
}

/// FLAC: walks the metadata blocks, seeking past pictures (their size is all that is needed).
fn flac(f: &mut fs::File, size: u64) -> Result<Details, TauError> {
    use std::io::{Seek, SeekFrom};
    let mut d = Details {
        codec: 2,
        channels: 2,
        bits: 16,
        size: size.min(u32::MAX as u64) as u32,
        ..Details::default()
    };
    let mut rg = Rg::default();
    let mut secs = 0.0f64;
    if read_exact_or_none(f, 4).as_deref() != Some(b"fLaC") {
        return Ok(d);
    }
    let mut pic = 0usize;
    loop {
        let h = read_exact_or_none(f, 4).unwrap_or_default();
        if h.len() < 4 {
            break;
        }
        let last = h[0] >> 7;
        let kind = h[0] & 0x7F;
        let len = ((h[1] as usize) << 16) | ((h[2] as usize) << 8) | h[3] as usize;
        if kind == 0 && len >= 18 {
            let body = read_exact_or_none(f, len).unwrap_or_default();
            if body.len() < 18 {
                return Err(bad("truncated STREAMINFO"));
            }
            let sr = ((body[10] as u32) << 12) | ((body[11] as u32) << 4) | (body[12] as u32 >> 4);
            d.channels = ((body[12] >> 1) & 7) + 1;
            d.bits = ((((body[12] & 1) as u16) << 4) | (body[13] as u16 >> 4)) as u8 + 1;
            let total = (((body[13] & 15) as u64) << 32) | be32(&body, 14)? as u64;
            d.rate = sr;
            secs = if sr != 0 {
                total as f64 / sr as f64
            } else {
                0.0
            };
        } else if kind == 4 && len < 1 << 20 {
            let body = read_exact_or_none(f, len).unwrap_or_default();
            let vendor_len = le32(&body, 0)? as usize;
            let vendor = body.get(4..(4 + vendor_len).min(body.len())).unwrap_or(&[]);
            d.encoder = strip_nul_space(&String::from_utf8_lossy(vendor)).to_string();
            let mut p = 4 + vendor_len;
            let count = le32(&body, p)?;
            p += 4;
            for _ in 0..count {
                let cl = le32(&body, p)? as usize;
                let kv = String::from_utf8_lossy(
                    body.get(p + 4..(p + 4 + cl).min(body.len())).unwrap_or(&[]),
                )
                .into_owned();
                p += 4 + cl;
                if let Some((k, v)) = kv.split_once('=') {
                    rg.set(k, v);
                }
            }
        } else {
            if kind == 6 {
                pic += len;
            }
            f.seek(SeekFrom::Current(len as i64))?;
        }
        if last == 1 {
            break;
        }
    }
    d.cover_kib = if pic > 0 {
        pic.div_ceil(1024).min(65535) as u16
    } else {
        0
    };
    if secs > 0.0 {
        d.kbps = round_even(size as f64 * 8.0 / secs / 1000.0).clamp(0, 65535) as u16;
    }
    rg.into(&mut d);
    Ok(d)
}

/// Why the Pocket will not play this file, if it will not (the player refuses FLAC above 48 kHz, more
/// than two channels, or a bit depth the decoder lacks; MP3 plays at every rate it can carry).
pub fn unplayable_reason(d: &Details) -> Option<String> {
    if d.codec != 2 {
        return None;
    }
    if d.rate > 48_000 {
        return Some(format!(
            "{} kHz FLAC: the Pocket plays FLAC up to 48 kHz",
            d.rate as f64 / 1000.0
        ));
    }
    if d.channels > 2 {
        return Some(format!(
            "{} channels: the Pocket plays mono and stereo",
            d.channels
        ));
    }
    if !matches!(d.bits, 8 | 16 | 20 | 24) {
        return Some(format!(
            "{}-bit FLAC: the Pocket plays 8, 16, 20 and 24 bit",
            d.bits
        ));
    }
    None
}

/// Reads one track's details. Never decodes audio; reads the tag region and the first 16 KiB of audio.
pub fn extract(path: &Path) -> Result<Details, TauError> {
    let size = fs::metadata(path)?.len();
    let mut file = fs::File::open(path)?;
    let mut d = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("flac"))
    {
        flac(&mut file, size)?
    } else {
        mp3(&read_mp3_head(&mut file)?, size)?
    };
    if d.cover_kib > 0 {
        d.flags |= F_COVER;
    }
    if path.parent().is_some_and(|p| p.join(SIDECAR).is_file()) {
        d.flags |= F_SIDECAR;
    }
    Ok(d)
}

fn read_mp3_head(f: &mut fs::File) -> Result<Vec<u8>, TauError> {
    use std::io::Read;
    let mut head = [0u8; 10];
    let n = f.read(&mut head)?;
    let mut out = head[..n].to_vec();
    let want = if n == 10 && &head[..3] == b"ID3" && (head[3] == 3 || head[3] == 4) {
        // The tag, then 16 KiB of audio after it (the reference reads from the end of the tag).
        synchsafe(&head[6..10]) + 16384
    } else {
        16384 - n
    };
    let mut rest = Vec::new();
    f.take(want as u64).read_to_end(&mut rest)?;
    out.extend(rest);
    Ok(out)
}
