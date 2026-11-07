//! The name sidecar: original names and tags beside the card-safe ones.
//!
//! Port of Tau Alpha's `docs/features/NAME_SIDECAR_SPEC.md` (reference
//! `tools/name_sidecar.py`, writer `tau_library.write_names`). The card carries
//! safe names and the index carries Latin-1 display text; both lose Cyrillic,
//! CJK, emoji and the Latin letters code page 437 lacks. The sidecar keeps the
//! original UTF-8 next to them:
//!
//! - `tau-names.json`, the manifest (source of truth): per track the card path,
//!   the original path and the original tags;
//! - `tau-names.bin` (`TNMS`), the firmware's display table, bound to the index
//!   by its `build_id`.
//!
//! Both are generated files in the media root, like `tau-library.tdb`: written
//! together or not at all, rebuilt whenever the index is, removed when a
//! library has nothing the index cannot show.

use crate::{ErrorCode, TauError, parse};
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, fs, path::Path};
use unicode_normalization::{UnicodeNormalization, char::canonical_combining_class};

pub const JSON_NAME: &str = "tau-names.json";
pub const BIN_NAME: &str = "tau-names.bin";
const MAGIC: &[u8; 4] = b"TNMS";
const VERSION: u16 = 1;
/// The firmware reads at most this much.
pub const BIN_CAP: usize = 1 << 20;

fn limit(key: &str) -> usize {
    match key {
        "file" => 255,
        _ => 127,
    }
}

/// Spec 3: controls become a space, the text is NFC and cut at `limit`
/// characters without a dangling combining mark, joiner or variation selector.
/// `None` when nothing is left.
pub fn clean_text(s: Option<&str>, limit: usize) -> Option<String> {
    let s = s?;
    let cleaned: String = s
        .chars()
        .map(|c| {
            let o = c as u32;
            if o < 0x20 || (0x7F..=0x9F).contains(&o) {
                ' '
            } else {
                c
            }
        })
        .nfc()
        .collect();
    let mut t: Vec<char> = cleaned.chars().collect();
    if t.len() > limit {
        t.truncate(limit);
        while t.last().is_some_and(|&c| {
            canonical_combining_class(c) != 0 || matches!(c, '\u{200d}' | '\u{fe0e}' | '\u{fe0f}')
        }) {
            t.pop();
        }
    }
    let t: String = t.into_iter().collect();
    let t = t.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// The original text behind one track of the index (by index id).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackSide {
    pub rel: String,
    pub file: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub idx_title: String,
    pub idx_artist: String,
    pub idx_album: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlbumSide {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub idx_title: String,
    pub idx_artist: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtistSide {
    pub name: Option<String>,
    pub idx_name: String,
}
/// What the index builder collected: original text and index text of every
/// track, album and artist, by index id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameSide {
    pub tracks: Vec<TrackSide>,
    pub albums: Vec<AlbumSide>,
    pub artists: Vec<ArtistSide>,
}

/// One track the binary carries: `None` = nothing to add over the index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct BinTrack {
    file: Option<String>,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
}

fn align(n: usize) -> usize {
    (n + 15) & !15
}

fn build_bin(
    index_bid: u32,
    tracks: &[BinTrack],
    albums: &[(Option<String>, Option<String>)],
    artists: &[Option<String>],
) -> Result<Vec<u8>, String> {
    let mut pool: Vec<u8> = vec![0];
    let mut seen: BTreeMap<String, u32> = BTreeMap::new();
    let mut add = |s: &Option<String>, key: &str| -> u32 {
        let Some(s) = clean_text(s.as_deref(), limit(key)) else {
            return 0;
        };
        *seen.entry(s.clone()).or_insert_with(|| {
            let at = pool.len() as u32;
            pool.extend_from_slice(s.as_bytes());
            pool.push(0);
            at
        })
    };
    let mut trk = Vec::new();
    for t in tracks {
        for v in [
            add(&t.file, "file"),
            add(&t.title, "title"),
            add(&t.artist, "artist"),
            add(&t.album, "album"),
        ] {
            trk.extend_from_slice(&v.to_le_bytes());
        }
    }
    let mut alb = Vec::new();
    for (title, artist) in albums {
        alb.extend_from_slice(&add(title, "title").to_le_bytes());
        alb.extend_from_slice(&add(artist, "artist").to_le_bytes());
    }
    let mut art = Vec::new();
    for name in artists {
        art.extend_from_slice(&add(name, "artist").to_le_bytes());
    }
    let o_trk = 48;
    let o_alb = align(o_trk + trk.len());
    let o_art = align(o_alb + alb.len());
    let o_pool = align(o_art + art.len());
    let mut hdr = Vec::with_capacity(48);
    hdr.extend_from_slice(MAGIC);
    hdr.extend_from_slice(&VERSION.to_le_bytes());
    hdr.extend_from_slice(&1u16.to_le_bytes());
    for v in [
        index_bid,
        tracks.len() as u32,
        albums.len() as u32,
        artists.len() as u32,
        o_pool as u32,
        pool.len() as u32,
    ] {
        hdr.extend_from_slice(&v.to_le_bytes());
    }
    let crc = crc32fast::hash(&hdr[..32]);
    hdr.truncate(32);
    hdr.extend_from_slice(&crc.to_le_bytes());
    hdr.extend_from_slice(&[0; 12]);
    let mut out = hdr;
    for (off, blob) in [(o_trk, &trk), (o_alb, &alb), (o_art, &art), (o_pool, &pool)] {
        out.resize(off, 0);
        out.extend_from_slice(blob);
    }
    if out.len() > BIN_CAP {
        return Err(format!(
            "sidecar is {} bytes, over the {BIN_CAP} cap",
            out.len()
        ));
    }
    Ok(out)
}

/// What a reader finds in `tau-names.bin`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedBin {
    pub tracks: Vec<[Option<String>; 4]>,
    pub albums: Vec<[Option<String>; 2]>,
    pub artists: Vec<Option<String>>,
}

/// The reader the firmware implements (spec 5 reader checks). Any error means
/// "no sidecar".
pub fn parse_bin(
    data: &[u8],
    index_bid: Option<u32>,
    counts: Option<(usize, usize, usize)>,
) -> Result<ParsedBin, String> {
    let u32_at = |o: usize| u32::from_le_bytes(data[o..o + 4].try_into().unwrap());
    if data.len() < 48 || &data[..4] != MAGIC {
        return Err("bad magic".into());
    }
    let version = u16::from_le_bytes([data[4], data[5]]);
    if version > VERSION {
        return Err(format!("version {version}"));
    }
    if crc32fast::hash(&data[..32]) != u32_at(32) {
        return Err("header crc".into());
    }
    if index_bid.is_some_and(|b| b != u32_at(8)) {
        return Err("index crc differs: the library was rebuilt".into());
    }
    let (nt, na, nr) = (
        u32_at(12) as usize,
        u32_at(16) as usize,
        u32_at(20) as usize,
    );
    if counts.is_some_and(|c| c != (nt, na, nr)) {
        return Err("counts differ from the index".into());
    }
    let (o_pool, n_pool) = (u32_at(24) as usize, u32_at(28) as usize);
    let o_alb = align(48 + nt * 16);
    let o_art = align(o_alb + na * 8);
    if o_pool != align(o_art + nr * 4)
        || o_pool + n_pool != data.len()
        || n_pool < 1
        || data[o_pool] != 0
        || data[o_pool + n_pool - 1] != 0
    {
        return Err("layout".into());
    }
    let pool = &data[o_pool..o_pool + n_pool];
    let s = |off: u32| -> Result<Option<String>, String> {
        let off = off as usize;
        if off == 0 {
            return Ok(None);
        }
        if off >= n_pool || pool[off - 1] != 0 {
            return Err(format!("offset {off} is not the start of a string"));
        }
        let end = pool[off..]
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated string")?;
        String::from_utf8(pool[off..off + end].to_vec())
            .map(Some)
            .map_err(|_| "invalid UTF-8".to_string())
    };
    let mut out = ParsedBin::default();
    for i in 0..nt {
        let r = 48 + 16 * i;
        out.tracks.push([
            s(u32_at(r))?,
            s(u32_at(r + 4))?,
            s(u32_at(r + 8))?,
            s(u32_at(r + 12))?,
        ]);
    }
    for i in 0..na {
        let r = o_alb + 8 * i;
        out.albums.push([s(u32_at(r))?, s(u32_at(r + 4))?]);
    }
    for i in 0..nr {
        out.artists.push(s(u32_at(o_art + 4 * i))?);
    }
    Ok(out)
}

fn nfc(s: &str) -> String {
    s.nfc().collect()
}

/// Spec 4: the manifest keeps a record only where the original path differs
/// from the card path or a tag says more than the index.
fn build_manifest(index_bid: u32, side: &NameSide, origins: &BTreeMap<String, String>) -> Value {
    let mut records = Vec::new();
    for (id, t) in side.tracks.iter().enumerate() {
        let orig = nfc(origins.get(&t.rel).unwrap_or(&t.rel));
        let mut rec = Map::new();
        rec.insert("id".into(), json!(id));
        rec.insert("card".into(), json!(t.rel));
        let mut extra = orig != t.rel;
        rec.insert("orig".into(), json!(orig));
        for (key, raw, idx) in [
            ("title", &t.title, &t.idx_title),
            ("artist", &t.artist, &t.idx_artist),
            ("album", &t.album, &t.idx_album),
        ] {
            let Some(v) = clean_text(raw.as_deref(), limit(key)) else {
                continue;
            };
            if &v != idx {
                extra = true;
            }
            rec.insert(key.into(), json!(v));
        }
        if extra {
            records.push(Value::Object(rec));
        }
    }
    json!({
        "format": "tau-names",
        "version": VERSION,
        "index_build_id": index_bid,
        "tracks": records,
    })
}

/// Deterministic UTF-8 JSON (sorted keys, one-space indent, trailing newline),
/// the bytes the Python reference writes.
pub fn manifest_bytes(manifest: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
    serde::Serialize::serialize(manifest, &mut ser).expect("a JSON value serialises");
    out.push(b'\n');
    out
}

/// What [`build_sidecar`] decided.
#[derive(Debug, Clone, PartialEq)]
pub struct Sidecar {
    pub manifest: Value,
    /// `None` when the binary has nothing to add (or does not fit): not written.
    pub binary: Option<Vec<u8>>,
}

impl Sidecar {
    pub fn records(&self) -> usize {
        self.manifest["tracks"].as_array().map_or(0, Vec::len)
    }
    pub fn is_empty(&self) -> bool {
        self.records() == 0 && self.binary.is_none()
    }
}

/// `origins` maps a card path (relative to the media root) to the original
/// path it was converted from. A field goes into the binary only when its
/// original differs from what the index shows. Over the 1 MiB cap the file
/// names are dropped first, then the binary is not written (the manifest
/// still is).
pub fn build_sidecar(
    index_bid: u32,
    side: &NameSide,
    origins: &BTreeMap<String, String>,
    counts: (usize, usize, usize),
) -> Result<Sidecar, TauError> {
    let keep = |raw: &Option<String>, idx: &str, key: &str| {
        clean_text(raw.as_deref(), limit(key)).filter(|v| v != idx)
    };
    let mut bt: Vec<BinTrack> = side
        .tracks
        .iter()
        .map(|t| {
            let orig = nfc(origins.get(&t.rel).unwrap_or(&t.rel));
            let ofile = orig.rsplit('/').next().unwrap_or("").to_string();
            BinTrack {
                file: (ofile != t.file).then_some(ofile),
                title: keep(&t.title, &t.idx_title, "title"),
                artist: keep(&t.artist, &t.idx_artist, "artist"),
                album: keep(&t.album, &t.idx_album, "album"),
            }
        })
        .collect();
    let manifest = build_manifest(index_bid, side, origins);
    let albums: Vec<_> = side
        .albums
        .iter()
        .map(|a| {
            (
                keep(&a.title, &a.idx_title, "title"),
                keep(&a.artist, &a.idx_artist, "artist"),
            )
        })
        .collect();
    let artists: Vec<_> = side
        .artists
        .iter()
        .map(|a| keep(&a.name, &a.idx_name, "artist"))
        .collect();
    if (bt.len(), albums.len(), artists.len()) != counts {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            "sidecar source does not match the index counts",
        ));
    }
    let nothing = bt
        .iter()
        .all(|t| t.file.is_none() && t.title.is_none() && t.artist.is_none() && t.album.is_none())
        && albums.iter().all(|a| a.0.is_none() && a.1.is_none())
        && artists.iter().all(Option::is_none);
    if nothing {
        return Ok(Sidecar {
            manifest,
            binary: None,
        });
    }
    let binary = build_bin(index_bid, &bt, &albums, &artists)
        .or_else(|_| {
            bt.iter_mut().for_each(|t| t.file = None);
            build_bin(index_bid, &bt, &albums, &artists)
        })
        .ok();
    Ok(Sidecar { manifest, binary })
}

/// After an index was written: the sidecar beside it. A failure here never undoes
/// the library; it becomes a warning (the Pocket then shows the index text).
pub(crate) fn write_after_build(
    folder: &Path,
    index: &[u8],
    side: &NameSide,
    origins: &BTreeMap<String, String>,
    warnings: &mut Vec<crate::Warning>,
) {
    if let Err(e) = write_sidecar(folder, index, side, origins) {
        warnings.push(crate::Warning::new(
            crate::WarningCode::NamesNotWritten,
            format!(
                "the library was written, but the name sidecar was not ({}); the Pocket shows the index text",
                e.message
            ),
        ));
    }
}

/// What [`write_sidecar`] did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NamesOutcome {
    /// Manifest records written (0 with no files written).
    pub records: usize,
    /// Size of `tau-names.bin`, when one was written.
    pub binary_bytes: Option<usize>,
    /// A stale sidecar was removed because the library has nothing to record.
    pub removed: bool,
}

/// Originals of an earlier manifest, kept for files that are still on the card.
fn earlier_origins(folder: &Path) -> BTreeMap<String, String> {
    let Ok(bytes) = fs::read(folder.join(JSON_NAME)) else {
        return BTreeMap::new();
    };
    let Ok(v) = serde_json::from_slice::<Value>(&bytes) else {
        return BTreeMap::new();
    };
    if v["format"] != "tau-names" || v["version"].as_u64().unwrap_or(0) > u64::from(VERSION) {
        return BTreeMap::new();
    }
    v["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            Some((
                r["card"].as_str()?.to_string(),
                r["orig"].as_str()?.to_string(),
            ))
        })
        .collect()
}

/// Writes `tau-names.json` and `tau-names.bin` beside the index just built
/// (`index` is its bytes, `side` what the builder collected), or removes stale
/// ones when the library has nothing the index cannot show. Each file goes
/// through a temp file, is read back and compared, then renamed in; the binary
/// is re-parsed against this index. `origins` are the originals this run knows;
/// earlier ones are kept for files still on the card.
pub fn write_sidecar(
    folder: &Path,
    index: &[u8],
    side: &NameSide,
    origins: &BTreeMap<String, String>,
) -> Result<NamesOutcome, TauError> {
    let ix = parse(index)?;
    let counts = (
        usize::from(ix.counts.tracks),
        usize::from(ix.counts.albums),
        usize::from(ix.counts.artists),
    );
    let mut known = earlier_origins(folder);
    known.extend(origins.iter().map(|(k, v)| (k.clone(), v.clone())));
    let sidecar = build_sidecar(ix.build_id, side, &known, counts)?;
    let (json_path, bin_path) = (folder.join(JSON_NAME), folder.join(BIN_NAME));
    if sidecar.is_empty() {
        let mut removed = false;
        for p in [&json_path, &bin_path] {
            if p.exists() {
                fs::remove_file(p)?;
                removed = true;
            }
        }
        return Ok(NamesOutcome {
            removed,
            ..NamesOutcome::default()
        });
    }
    let manifest = manifest_bytes(&sidecar.manifest);
    for (path, bytes) in [(&bin_path, &sidecar.binary), (&json_path, &Some(manifest))] {
        let Some(bytes) = bytes else {
            if path.exists() {
                fs::remove_file(path)?;
            }
            continue;
        };
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let temp = folder.join(format!(".{name}.tmp"));
        crate::sync::write_durable(&temp, bytes)?;
        if crate::sync::read_back_bytes(&temp)? != *bytes {
            let _ = fs::remove_file(&temp);
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                format!("verify failed: {name}"),
            ));
        }
        crate::sync::swap_in_file(&temp, path, &format!(".{name}.prev"))?;
    }
    if let Some(blob) = &sidecar.binary {
        let read = fs::read(&bin_path)?;
        if read != *blob || parse_bin(&read, Some(ix.build_id), Some(counts)).is_err() {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                "tau-names.bin does not bind to the index it was built for",
            ));
        }
    }
    Ok(NamesOutcome {
        records: sidecar.records(),
        binary_bytes: sidecar.binary.as_ref().map(Vec::len),
        removed: false,
    })
}

#[cfg(test)]
mod tests;
