//! Portable, UI-independent Tau Omega engine.
//!
//! This crate deliberately has no card-writing API in T0/T1. It can inspect a
//! card and build, parse and verify the Tau v1 index from a folder or fixture.

pub mod compare;
pub mod cover;
pub mod diag;
pub mod duplicates;
pub mod journal;
pub mod playlist;
pub mod sync;

use crc32fast::hash as crc32;
use serde_json::Value;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub const ROOT_PREFIX: &str = "/Assets/tau/common/";
const MAGIC: u32 = 0x4249_4c54;
const HEADER: usize = 128;
const SECTIONS: [&str; 8] = [
    "artists",
    "albums",
    "tracks",
    "strings",
    "album_by_title",
    "track_by_title",
    "letters",
    "playlists",
];
const MAX_TRACKS: usize = 16_384;
const MAX_ALBUMS: usize = 2_048;
const MAX_ARTISTS: usize = 1_024;
const MAX_PLAYLISTS: usize = 64;
const MAX_FILE: usize = 4 << 20;
const MAX_STRINGS: usize = 3 << 20;
const MAX_PATH: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TauError {
    Code(u8, String),
    Io(String),
    Json(String),
}

impl TauError {
    pub fn code(&self) -> Option<u8> {
        if let Self::Code(code, _) = self {
            Some(*code)
        } else {
            None
        }
    }
    fn e(code: u8, message: impl Into<String>) -> Self {
        Self::Code(code, message.into())
    }
}

impl std::fmt::Display for TauError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Code(c, s) => write!(f, "E{c}: {s}"),
            Self::Io(s) | Self::Json(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for TauError {}
impl From<io::Error> for TauError {
    fn from(e: io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Core {
    pub id: String,
    pub author: String,
    pub shortname: String,
    pub version: String,
    pub platform: String,
    pub library_capable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub root: PathBuf,
    pub is_pocket_card: bool,
    pub cores: Vec<Core>,
    pub warnings: Vec<String>,
}

/// Reads card metadata only; no filesystem mutation is performed.
pub fn inspect_card(root: impl AsRef<Path>) -> Result<Card, TauError> {
    let root = root.as_ref().canonicalize().map_err(TauError::from)?;
    let is_pocket_card = root.join("Cores").is_dir() && root.join("Assets").is_dir();
    let mut warnings = Vec::new();
    if !is_pocket_card {
        warnings.push("This folder does not contain both Cores and Assets.".into());
    }
    let mut cores = Vec::new();
    let core_root = root.join("Cores");
    if core_root.is_dir() {
        let mut folders: Vec<_> = fs::read_dir(&core_root)?
            .filter_map(Result::ok)
            .filter(|p| p.path().is_dir())
            .collect();
        folders.sort_by_key(|p| p.file_name());
        for folder in folders {
            let id = folder.file_name().to_string_lossy().to_string();
            let core_json = folder.path().join("core.json");
            if !core_json.is_file() {
                warnings.push(format!("{id}: missing core.json"));
                continue;
            }
            let json: Value = serde_json::from_slice(&fs::read(&core_json)?)
                .map_err(|e| TauError::Json(format!("{id}/core.json: {e}")))?;
            let metadata = json
                .pointer("/core/metadata")
                .ok_or_else(|| TauError::Json(format!("{id}/core.json: missing core.metadata")))?;
            let string = |key: &str| {
                metadata
                    .get(key)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let platform = metadata
                .get("platform_ids")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let data_json = folder.path().join("data.json");
            let library_capable = fs::read(&data_json)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .map(|value| slots_have_library(&value))
                .unwrap_or(false);
            cores.push(Core {
                id,
                author: string("author"),
                shortname: string("shortname"),
                version: string("version"),
                platform,
                library_capable,
            });
        }
    }
    Ok(Card {
        root,
        is_pocket_card,
        cores,
        warnings,
    })
}

/// A core supports the media library if any data slot serves `tau-library.tdb`.
///
/// Matched by filename, never by slot id: the id is the core author's choice and
/// the shipped Tau core has already moved its other slots around. The real APF
/// layout is `{"data": {"data_slots": [...]}}`; the flatter shapes are accepted
/// because older hand-written cores use them.
fn slots_have_library(json: &Value) -> bool {
    let serves_library = |items: Option<&Vec<Value>>| {
        items
            .into_iter()
            .flatten()
            .any(|v| v.get("filename").and_then(Value::as_str) == Some("tau-library.tdb"))
    };
    let slots = |pointer: &str| serves_library(json.pointer(pointer).and_then(Value::as_array));
    slots("/data/data_slots") || slots("/core/data/data_slots") || slots("/data") || slots("/core/data")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub rel: String,
    pub dir: String,
    pub file: String,
    pub tags: BTreeMap<String, String>,
    pub secs: u16,
    pub fmt: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    pub name: String,
    pub rel_ids: Vec<usize>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    pub entries: Vec<Entry>,
    pub playlists: Vec<Playlist>,
    pub warnings: Vec<String>,
}

/// Returns printable device ASCII. Latin characters needed by common music tags
/// are folded explicitly; unsupported Unicode is dropped just as the reference does.
pub fn ascii_text(input: &str, limit: usize) -> String {
    let mut out = String::new();
    for c in input.chars() {
        let folded = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä'
            | 'Å' | 'Ā' | 'Ă' | 'Ą' => "a",
            'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' | 'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => "c",
            'ď' | 'đ' | 'Ð' | 'Ď' | 'Đ' => "d",
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' | 'È' | 'É' | 'Ê' | 'Ë' | 'Ē'
            | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "e",
            'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī'
            | 'Ĭ' | 'Į' => "i",
            'ñ' | 'ń' | 'ņ' | 'ň' | 'Ñ' | 'Ń' | 'Ņ' | 'Ň' => "n",
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö'
            | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => "o",
            'ß' => "ss",
            'ŕ' | 'ŗ' | 'ř' | 'Ŕ' | 'Ŗ' | 'Ř' => "r",
            'ś' | 'ŝ' | 'ş' | 'š' | 'Ś' | 'Ŝ' | 'Ş' | 'Š' => "s",
            'ţ' | 'ť' | 'ŧ' | 'Ţ' | 'Ť' | 'Ŧ' => "t",
            'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'Ù' | 'Ú' | 'Û' | 'Ü'
            | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => "u",
            'ý' | 'ÿ' | 'ŷ' | 'Ý' | 'Ÿ' | 'Ŷ' => "y",
            'ź' | 'ż' | 'ž' | 'Ź' | 'Ż' | 'Ž' => "z",
            x if x.is_ascii() && !x.is_ascii_control() => {
                out.push(x);
                continue;
            }
            x if x.is_whitespace() => " ",
            _ => "",
        };
        out.push_str(folded);
    }
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(limit)
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn ascii_name(input: &str) -> String {
    let mut name = ascii_text(input, usize::MAX);
    for bad in ['<', '>', ':', '"', '|', '?', '*', '\\'] {
        name = name.replace(bad, "_");
    }
    name.trim().to_string()
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum Token {
    Number(u64),
    Text(String),
}
impl Ord for Token {
    fn cmp(&self, o: &Self) -> Ordering {
        match (self, o) {
            (Self::Number(a), Self::Number(b)) => a.cmp(b),
            (Self::Text(a), Self::Text(b)) => a.cmp(b),
            (Self::Number(_), Self::Text(_)) => Ordering::Less,
            (Self::Text(_), Self::Number(_)) => Ordering::Greater,
        }
    }
}
impl PartialOrd for Token {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
fn tokens(input: &str) -> Vec<Token> {
    let mut parts = Vec::new();
    let mut buf = String::new();
    let mut digits: Option<bool> = None;
    for c in input.to_lowercase().chars() {
        let d = c.is_ascii_digit();
        if digits.is_some_and(|old| old != d) {
            parts.push(if digits == Some(true) {
                Token::Number(buf.parse().unwrap_or(0))
            } else {
                Token::Text(buf.clone())
            });
            buf.clear();
        }
        digits = Some(d);
        buf.push(c);
    }
    if !buf.is_empty() {
        parts.push(if digits == Some(true) {
            Token::Number(buf.parse().unwrap_or(0))
        } else {
            Token::Text(buf)
        });
    }
    parts
}
fn sort_key(input: &str) -> (u8, Vec<Token>) {
    let mut t = input.trim().to_lowercase();
    for article in ["the ", "a "] {
        if t.starts_with(article) && t.len() > article.len() {
            t = t[article.len()..].to_string();
            break;
        }
    }
    let class = t
        .as_bytes()
        .first()
        .filter(|b| b.is_ascii_lowercase())
        .map(|b| b - b'a' + 1)
        .unwrap_or(0);
    (class, tokens(&t))
}
fn natural(input: &str) -> Vec<Token> {
    tokens(input)
}

/// Scans a destination media root. Source paths remain read-only; this operation only reads tags.
pub fn scan_dir(common: &Path, playlists: bool) -> Result<Scan, TauError> {
    let mut files = Vec::new();
    collect_files(common, common, &mut files)?;
    files.sort();
    let mut output = Scan::default();
    for path in files {
        let extension = path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(extension.as_str(), "mp3" | "flac")
            || path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("._"))
        {
            continue;
        }
        let rel = path
            .strip_prefix(common)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let (tags, secs, fmt) = match read_tags(&path) {
            Ok(result) => result,
            Err(e) => {
                output
                    .warnings
                    .push(format!("{rel}: tags unreadable ({e})"));
                (BTreeMap::new(), 0, if extension == "flac" { 2 } else { 1 })
            }
        };
        let (dir, file) = rel
            .rsplit_once('/')
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .unwrap_or((String::new(), rel.clone()));
        output.entries.push(Entry {
            rel,
            dir,
            file,
            tags,
            secs,
            fmt,
        });
    }
    if playlists {
        output.playlists = scan_playlists(common, &output.entries, &mut output.warnings)?;
    }
    Ok(output)
}

fn collect_files(root: &Path, at: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for child in fs::read_dir(at)? {
        let child = child?.path();
        if child.is_dir() {
            collect_files(root, &child, out)?;
        } else if child.is_file()
            && !child
                .strip_prefix(root)
                .unwrap_or(&child)
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with("._"))
        {
            out.push(child);
        }
    }
    Ok(())
}

fn scan_playlists(
    common: &Path,
    entries: &[Entry],
    warnings: &mut Vec<String>,
) -> Result<Vec<Playlist>, TauError> {
    let mut found = Vec::new();
    collect_files(common, common, &mut found)?;
    found.sort();
    let by_rel: BTreeMap<_, _> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.rel.as_str(), i))
        .collect();
    let mut used = BTreeSet::new();
    let mut output = Vec::new();
    for file in found.into_iter().filter(|f| {
        f.extension().is_some_and(|x| x.eq_ignore_ascii_case("m3u"))
            && !f
                .file_name()
                .is_some_and(|x| x.to_string_lossy().starts_with("._"))
    }) {
        let base = file
            .parent()
            .unwrap()
            .strip_prefix(common)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let mut ids = Vec::new();
        let mut dropped = 0;
        for line in String::from_utf8_lossy(&fs::read(&file)?)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let rel = if let Some(rest) = line.strip_prefix('/') {
                rest
            } else if base.is_empty() {
                line
            } else {
                &format!("{base}/{line}")
            };
            if let Some(id) = by_rel.get(rel) {
                ids.push(*id);
            } else {
                dropped += 1;
            }
        }
        if dropped > 0 {
            warnings.push(format!(
                "{}: {dropped} line(s) not in the library, dropped",
                file.file_name().unwrap().to_string_lossy()
            ));
        }
        let mut own: Vec<_> = entries
            .iter()
            .filter(|e| e.dir == base)
            .map(|e| e.rel.as_str())
            .collect();
        own.sort_by_key(|r| natural(r.rsplit('/').next().unwrap_or(r)));
        if !base.is_empty()
            && ids
                .iter()
                .map(|id| entries[*id].rel.as_str())
                .eq(own.iter().copied())
        {
            continue;
        }
        if ids.is_empty() {
            continue;
        }
        if ids.len() > MAX_TRACKS {
            warnings.push(format!(
                "{}: {} entries, truncated to {MAX_TRACKS}",
                file.file_name().unwrap().to_string_lossy(),
                ids.len()
            ));
            ids.truncate(MAX_TRACKS);
        }
        let stem = file.file_stem().unwrap_or_default().to_string_lossy();
        let mut name = ascii_text(&stem, 31);
        if name.is_empty() || name.eq_ignore_ascii_case("playlist") {
            name = if base.is_empty() {
                "Playlist".into()
            } else {
                ascii_text(
                    file.parent()
                        .unwrap()
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .as_ref(),
                    31,
                )
            };
        }
        let basename = if name.is_empty() {
            "Playlist".into()
        } else {
            name.clone()
        };
        let mut n = 2;
        while used.contains(&name.to_lowercase()) {
            name = format!("{} {n}", basename.chars().take(28).collect::<String>());
            n += 1;
        }
        used.insert(name.to_lowercase());
        output.push(Playlist { name, rel_ids: ids });
    }
    Ok(output)
}

fn read_tags(path: &Path) -> Result<(BTreeMap<String, String>, u16, u8), TauError> {
    let mut f = fs::File::open(path)?;
    let size = f.metadata()?.len();
    if path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("flac"))
    {
        let (t, s) = read_flac(&mut f)?;
        return Ok((t, s.min(u16::MAX as u64) as u16, 2));
    }
    let (mut tags, start) = read_id3v2(&mut f)?;
    for (k, v) in read_id3v1(&mut f, size)? {
        tags.entry(k).or_insert(v);
    }
    let secs = mp3_seconds(&mut f, start, size)?
        .unwrap_or(0)
        .min(u16::MAX as u64) as u16;
    Ok((tags, secs, 1))
}

fn read_id3v2(f: &mut fs::File) -> Result<(BTreeMap<String, String>, u64), TauError> {
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
fn decode_id3_text(b: &[u8]) -> Option<String> {
    let (&enc, body) = b.split_first()?;
    let bytes = body.split(|x| *x == 0).next().unwrap_or_default();
    let text = match enc {
        0 => bytes.iter().map(|x| char::from(*x)).collect(),
        3 => String::from_utf8_lossy(bytes).to_string(),
        1 | 2 => {
            let big = enc == 2;
            let mut chars = Vec::new();
            for pair in bytes.as_chunks::<2>().0 {
                chars.push(if big {
                    u16::from_be_bytes([pair[0], pair[1]])
                } else {
                    u16::from_le_bytes([pair[0], pair[1]])
                });
            }
            String::from_utf16_lossy(&chars)
        }
        _ => String::from_utf8_lossy(bytes).to_string(),
    };
    Some(text.trim().to_string())
}
fn read_id3v1(f: &mut fs::File, size: u64) -> Result<BTreeMap<String, String>, TauError> {
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
fn mp3_seconds(f: &mut fs::File, start: u64, size: u64) -> Result<Option<u64>, TauError> {
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
fn read_flac(f: &mut fs::File) -> Result<(BTreeMap<String, String>, u64), TauError> {
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

fn num(value: Option<&String>, hi: u16) -> u16 {
    value
        .and_then(|v| v.trim().split('/').next()?.parse::<u16>().ok())
        .map(|v| v.min(hi))
        .unwrap_or(0)
}
fn year(tags: &BTreeMap<String, String>) -> u16 {
    tags.get("TDRC")
        .or_else(|| tags.get("TYER"))
        .and_then(|v| {
            v.as_bytes()
                .windows(4)
                .find_map(|w| std::str::from_utf8(w).ok()?.parse().ok())
        })
        .unwrap_or(0)
}
fn push_u16(out: &mut Vec<u8>, n: u16) {
    out.extend_from_slice(&n.to_le_bytes());
}
fn push_u32(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_le_bytes());
}
fn put_u16(out: &mut [u8], at: usize, n: u16) {
    out[at..at + 2].copy_from_slice(&n.to_le_bytes());
}
fn put_u32(out: &mut [u8], at: usize, n: u32) {
    out[at..at + 4].copy_from_slice(&n.to_le_bytes());
}
fn align(out: &mut Vec<u8>) {
    out.resize((out.len() + 15) & !15, 0);
}

#[derive(Default)]
struct Pool {
    buffer: Vec<u8>,
    offsets: BTreeMap<String, u32>,
}
impl Pool {
    fn new() -> Self {
        let mut out = Self::default();
        out.buffer.push(0);
        out.offsets.insert(String::new(), 0);
        out
    }
    fn add(&mut self, text: &str) -> u32 {
        if let Some(offset) = self.offsets.get(text) {
            return *offset;
        }
        let offset = self.buffer.len() as u32;
        self.buffer.extend_from_slice(text.as_bytes());
        self.buffer.push(0);
        self.offsets.insert(text.into(), offset);
        offset
    }
}

#[derive(Clone)]
struct Album {
    dir: String,
    ids: Vec<usize>,
    artist: String,
    title: String,
    year: u16,
}

/// Encodes exactly the Tau v1 index layout. The input describes files already
/// laid out under the destination common directory; it does not copy or rename them.
pub fn build_index(
    entries: &[Entry],
    playlists: &[Playlist],
    root_prefix: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<u8>, TauError> {
    let mut entries = entries.to_vec();
    let mut grouped: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, e) in entries.iter_mut().enumerate() {
        let title = ascii_text(e.tags.get("TIT2").map(String::as_str).unwrap_or(""), 63);
        e.tags.insert(
            "_title".into(),
            if title.is_empty() {
                let stem = e.file.rsplit_once('.').map(|x| x.0).unwrap_or(&e.file);
                let fallback = ascii_text(
                    stem.trim_start_matches(|c: char| {
                        c.is_ascii_digit()
                            || c.is_ascii_whitespace()
                            || matches!(c, '.' | '_' | '-')
                    }),
                    63,
                );
                if fallback.is_empty() {
                    "Track".into()
                } else {
                    fallback
                }
            } else {
                title
            },
        );
        let tno = (num(e.tags.get("TPOS"), 63) << 10) | num(e.tags.get("TRCK"), 1023);
        e.tags.insert("_tno".into(), tno.to_string());
        if (root_prefix.len() + e.rel.len()) > MAX_PATH {
            return Err(TauError::e(
                14,
                format!("path over {MAX_PATH} bytes: {}", e.rel),
            ));
        }
        if !(e.tags.contains_key("TIT2")
            && (e.tags.contains_key("TPE1") || e.tags.contains_key("TPE2")))
        {
            warnings.push(format!("{}: missing title/artist tag, using names", e.rel));
        }
        grouped.entry(e.dir.clone()).or_default().push(i);
    }
    let mut albums = Vec::new();
    for (dir, mut ids) in grouped {
        ids.sort_by_key(|&i| {
            (
                entries[i].tags["_tno"].parse::<u16>().unwrap() >> 10,
                entries[i].tags["_tno"].parse::<u16>().unwrap() & 1023,
                natural(&entries[i].file),
            )
        });
        let first = &entries[ids[0]];
        let artist = ids
            .iter()
            .find_map(|&i| {
                let t = ascii_text(
                    entries[i]
                        .tags
                        .get("TPE2")
                        .or_else(|| entries[i].tags.get("TPE1"))
                        .map(String::as_str)
                        .unwrap_or(""),
                    63,
                );
                (!t.is_empty()).then_some(t)
            })
            .unwrap_or_else(|| "Unknown Artist".into());
        let title = ascii_text(first.tags.get("TALB").map(String::as_str).unwrap_or(""), 63);
        let title = if title.is_empty() {
            let fallback = ascii_text(dir.rsplit('/').next().unwrap_or(""), 63);
            if fallback.is_empty() {
                "Unknown Album".into()
            } else {
                fallback
            }
        } else {
            title
        };
        let album_year = ids
            .iter()
            .map(|&i| year(&entries[i].tags))
            .find(|v| *v != 0)
            .unwrap_or(0);
        albums.push(Album {
            dir,
            ids,
            artist,
            title,
            year: album_year,
        });
    }
    let mut artist_names: Vec<_> = albums.iter().map(|a| a.artist.clone()).collect();
    artist_names.sort_by_key(|n| (sort_key(n), n.clone()));
    artist_names.dedup();
    let artist_ids: BTreeMap<_, _> = artist_names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.clone(), i))
        .collect();
    albums.sort_by_key(|a| {
        (
            artist_ids[&a.artist],
            a.year,
            sort_key(&a.title),
            a.dir.clone(),
        )
    });
    if entries.len() > MAX_TRACKS
        || albums.len() > MAX_ALBUMS
        || artist_names.len() > MAX_ARTISTS
        || playlists.len() > MAX_PLAYLISTS
    {
        return Err(TauError::e(14, "library exceeds a hard cap"));
    }
    let mut pool = Pool::new();
    let root = pool.add(root_prefix);
    let (mut tracks, mut albums_b, mut first_album, mut track_new) =
        (Vec::new(), Vec::new(), BTreeMap::new(), BTreeMap::new());
    let mut album_titles = Vec::new();
    let mut track_titles = Vec::new();
    for (ai, a) in albums.iter().enumerate() {
        let artist = artist_ids[&a.artist];
        let item = first_album.entry(artist).or_insert((ai, 0usize));
        item.1 += 1;
        push_u32(&mut albums_b, pool.add(&a.title));
        push_u32(&mut albums_b, pool.add(&a.dir));
        push_u16(&mut albums_b, artist as u16);
        push_u16(&mut albums_b, a.year);
        push_u16(&mut albums_b, (tracks.len() / 16) as u16);
        push_u16(&mut albums_b, a.ids.len() as u16);
        push_u16(&mut albums_b, u16::MAX);
        push_u16(&mut albums_b, 0);
        album_titles.push(a.title.clone());
        for &i in &a.ids {
            let e = &entries[i];
            track_new.insert(i, tracks.len() / 16);
            let title = e.tags["_title"].clone();
            track_titles.push(title.clone());
            push_u32(&mut tracks, pool.add(&title));
            push_u32(&mut tracks, pool.add(&e.file));
            push_u16(&mut tracks, e.secs);
            push_u16(&mut tracks, e.tags["_tno"].parse().unwrap());
            push_u16(&mut tracks, ai as u16);
            tracks.push(e.fmt);
            tracks.push(0);
        }
    }
    let mut artists_b = Vec::new();
    for (i, name) in artist_names.iter().enumerate() {
        let (first, count) = first_album[&i];
        push_u32(&mut artists_b, pool.add(name));
        push_u16(&mut artists_b, first as u16);
        push_u16(&mut artists_b, count as u16);
    }
    let mut albums_order: Vec<_> = (0..albums.len()).collect();
    albums_order.sort_by_key(|&i| (sort_key(&album_titles[i]), i));
    let mut tracks_order: Vec<_> = (0..entries.len()).collect();
    tracks_order.sort_by_key(|&i| (sort_key(&track_titles[i]), i));
    let make_letters = |names: Vec<String>| -> Vec<u16> {
        let classes: Vec<_> = names.iter().map(|n| sort_key(n).0).collect();
        (0..27)
            .map(|c| {
                classes
                    .iter()
                    .position(|&cls| cls >= c)
                    .unwrap_or(classes.len()) as u16
            })
            .collect()
    };
    let mut letter_values = make_letters(artist_names.clone());
    letter_values.extend(make_letters(
        albums_order
            .iter()
            .map(|&i| album_titles[i].clone())
            .collect(),
    ));
    letter_values.extend(make_letters(
        tracks_order
            .iter()
            .map(|&i| track_titles[i].clone())
            .collect(),
    ));
    let (mut playlists_b, mut items) = (Vec::new(), Vec::new());
    for playlist in playlists {
        push_u32(&mut playlists_b, pool.add(&playlist.name));
        push_u16(&mut playlists_b, (items.len() / 2) as u16);
        push_u16(&mut playlists_b, playlist.rel_ids.len() as u16);
        for old in &playlist.rel_ids {
            let new = track_new
                .get(old)
                .ok_or_else(|| TauError::e(17, "playlist references a missing track"))?;
            push_u16(&mut items, *new as u16);
        }
    }
    let mut bodies = vec![
        artists_b,
        albums_b,
        tracks,
        pool.buffer.clone(),
        albums_order
            .iter()
            .flat_map(|i| (*i as u16).to_le_bytes())
            .collect(),
        tracks_order
            .iter()
            .flat_map(|i| (*i as u16).to_le_bytes())
            .collect(),
        letter_values.iter().flat_map(|i| i.to_le_bytes()).collect(),
        [playlists_b, items].concat(),
    ];
    let mut out = vec![0; HEADER];
    let mut sections = Vec::new();
    for body in bodies.drain(..) {
        align(&mut out);
        sections.push((out.len(), body.len()));
        out.extend(body);
    }
    align(&mut out);
    if out.len() > MAX_FILE || pool.buffer.len() > MAX_STRINGS {
        return Err(TauError::e(14, "index exceeds a size cap"));
    }
    let body_crc = crc32(&out[HEADER..]);
    let file_size = out.len() as u32;
    put_u32(&mut out, 0, MAGIC);
    put_u16(&mut out, 4, 1);
    put_u16(&mut out, 6, 1);
    put_u32(&mut out, 8, HEADER as u32);
    put_u32(&mut out, 12, if playlists.is_empty() { 0 } else { 2 });
    put_u32(&mut out, 16, body_crc);
    put_u32(&mut out, 20, file_size);
    put_u32(&mut out, 24, body_crc);
    put_u32(&mut out, 28, 0);
    put_u16(&mut out, 32, artist_names.len() as u16);
    put_u16(&mut out, 34, albums.len() as u16);
    put_u16(&mut out, 36, entries.len() as u16);
    put_u16(&mut out, 38, playlists.len() as u16);
    put_u32(&mut out, 40, root);
    put_u32(&mut out, 44, 0);
    for (i, (off, len)) in sections.iter().enumerate() {
        put_u32(&mut out, 48 + i * 8, *off as u32);
        put_u32(&mut out, 52 + i * 8, *len as u32);
    }
    let header_crc = crc32(&out[..124]);
    put_u32(&mut out, 124, header_crc);
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct Index {
    pub data: Vec<u8>,
    pub flags: u32,
    pub build_id: u32,
    pub counts: Counts,
    pub sections: BTreeMap<String, (usize, usize)>,
    pub root: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub artists: u16,
    pub albums: u16,
    pub tracks: u16,
    pub playlists: u16,
}
fn u16_at(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes(data[off..off + 2].try_into().unwrap())
}
fn u32_at(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(data[off..off + 4].try_into().unwrap())
}

/// Parses using the same validation order and E-codes as the firmware loader.
pub fn parse(data: impl AsRef<[u8]>) -> Result<Index, TauError> {
    let data = data.as_ref();
    if data.len() < HEADER {
        return Err(TauError::e(11, "shorter than header"));
    }
    if u32_at(data, 0) != MAGIC
        || u16_at(data, 6) > 1
        || u32_at(data, 8) != HEADER as u32
        || crc32(&data[..124]) != u32_at(data, 124)
    {
        return Err(TauError::e(11, "bad header"));
    }
    if u32_at(data, 20) as usize != data.len() {
        return Err(TauError::e(12, "size differs from header"));
    }
    if crc32(&data[HEADER..]) != u32_at(data, 24) {
        return Err(TauError::e(13, "body CRC"));
    }
    let counts = Counts {
        artists: u16_at(data, 32),
        albums: u16_at(data, 34),
        tracks: u16_at(data, 36),
        playlists: u16_at(data, 38),
    };
    if counts.tracks as usize > MAX_TRACKS
        || counts.albums as usize > MAX_ALBUMS
        || counts.artists as usize > MAX_ARTISTS
        || counts.playlists as usize > MAX_PLAYLISTS
        || data.len() > MAX_FILE
    {
        return Err(TauError::e(14, "count above cap"));
    }
    let mut sections = BTreeMap::new();
    for (i, name) in SECTIONS.iter().enumerate() {
        let (off, len) = (
            u32_at(data, 48 + i * 8) as usize,
            u32_at(data, 52 + i * 8) as usize,
        );
        if off % 16 != 0 || off < HEADER || off.checked_add(len).is_none_or(|end| end > data.len())
        {
            return Err(TauError::e(15, format!("section {name} out of range")));
        }
        sections.insert((*name).to_string(), (off, len));
    }
    for (name, want) in [
        ("artists", counts.artists as usize * 8),
        ("albums", counts.albums as usize * 20),
        ("tracks", counts.tracks as usize * 16),
        ("album_by_title", counts.albums as usize * 2),
        ("track_by_title", counts.tracks as usize * 2),
        ("letters", 162),
    ] {
        if sections[name].1 != want {
            return Err(TauError::e(15, format!("section {name} length")));
        }
    }
    if sections["strings"].1 > MAX_STRINGS {
        return Err(TauError::e(14, "string cap"));
    }
    let ix = Index {
        data: data.to_vec(),
        flags: u32_at(data, 12),
        build_id: u32_at(data, 16),
        counts,
        sections,
        root: u32_at(data, 40),
    };
    walk(&ix)?;
    Ok(ix)
}
fn string_at(ix: &Index, offset: u32) -> Result<&str, TauError> {
    let (off, len) = ix.sections["strings"];
    let start = off + offset as usize;
    if offset as usize >= len {
        return Err(TauError::e(17, "string offset"));
    }
    let bytes = &ix.data[start..off + len];
    let end = bytes.iter().position(|x| *x == 0).unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..end]).map_err(|_| TauError::e(17, "non-ASCII string"))
}
fn walk(ix: &Index) -> Result<(), TauError> {
    let strings = ix.sections["strings"].1;
    if ix.root as usize >= strings {
        return Err(TauError::e(17, "root string"));
    }
    let (tracks, _) = ix.sections["tracks"];
    for i in 0..ix.counts.tracks as usize {
        if i >= 256 && i % 64 != 0 {
            continue;
        }
        let r = tracks + i * 16;
        if u32_at(&ix.data, r) as usize >= strings
            || u32_at(&ix.data, r + 4) as usize >= strings
            || u16_at(&ix.data, r + 12) >= ix.counts.albums
        {
            return Err(TauError::e(17, "track record"));
        }
    }
    let (albums, _) = ix.sections["albums"];
    for i in 0..ix.counts.albums as usize {
        let r = albums + i * 20;
        if u32_at(&ix.data, r) as usize >= strings
            || u32_at(&ix.data, r + 4) as usize >= strings
            || u16_at(&ix.data, r + 8) >= ix.counts.artists
            || usize::from(u16_at(&ix.data, r + 12)) + usize::from(u16_at(&ix.data, r + 14))
                > ix.counts.tracks as usize
        {
            return Err(TauError::e(17, "album record"));
        }
    }
    let (lists, len) = ix.sections["playlists"];
    if len < 8 * ix.counts.playlists as usize
        || !(len - 8 * ix.counts.playlists as usize).is_multiple_of(2)
    {
        return Err(TauError::e(15, "playlist range"));
    }
    let items = (len - 8 * ix.counts.playlists as usize) / 2;
    for i in 0..ix.counts.playlists as usize {
        let r = lists + i * 8;
        if u32_at(&ix.data, r) as usize >= strings
            || usize::from(u16_at(&ix.data, r + 4)) + usize::from(u16_at(&ix.data, r + 6)) > items
            || u16_at(&ix.data, r + 6) as usize > MAX_TRACKS
        {
            return Err(TauError::e(17, "playlist record"));
        }
    }
    for i in 0..items {
        if i >= 256 && i % 64 != 0 {
            continue;
        }
        if u16_at(&ix.data, lists + 8 * ix.counts.playlists as usize + i * 2) >= ix.counts.tracks {
            return Err(TauError::e(17, "playlist item"));
        }
    }
    Ok(())
}
pub fn track_path(ix: &Index, id: u16) -> Result<String, TauError> {
    if id >= ix.counts.tracks {
        return Err(TauError::e(17, "track id"));
    }
    let (t, _) = ix.sections["tracks"];
    let (a, _) = ix.sections["albums"];
    let r = t + id as usize * 16;
    let album = u16_at(&ix.data, r + 12) as usize;
    let ar = a + album * 20;
    let root = string_at(ix, ix.root)?;
    let dir = string_at(ix, u32_at(&ix.data, ar + 4))?;
    let file = string_at(ix, u32_at(&ix.data, r + 4))?;
    Ok(if dir.is_empty() {
        format!("{root}{file}")
    } else {
        format!("{root}{dir}/{file}")
    })
}
pub fn verify(data: impl AsRef<[u8]>, root: Option<&Path>) -> Result<Vec<String>, TauError> {
    let ix = parse(data)?;
    let mut issues = Vec::new();
    if let Some(root) = root {
        for id in 0..ix.counts.tracks {
            let path = track_path(&ix, id)?;
            let relative = path.strip_prefix(string_at(&ix, ix.root)?).unwrap_or(&path);
            if !root.join(relative).is_file() {
                issues.push(format!("file missing: {relative}"));
            }
        }
    }
    Ok(issues)
}

pub fn synth(tracks: usize, albums: usize, artists: usize) -> Vec<Entry> {
    (0..tracks)
        .map(|i| {
            let album = i * albums / tracks;
            let artist = album * artists / albums;
            let mut tags = BTreeMap::new();
            tags.insert("TIT2".into(), format!("Track {:05}", i + 1));
            tags.insert("TPE1".into(), format!("Artist {:04}", artist + 1));
            tags.insert("TALB".into(), format!("Album {:04}", album + 1));
            tags.insert("TRCK".into(), format!("{}", (i % 12) + 1));
            Entry {
                rel: format!(
                    "Artist {:04}/Album {:04}/{:02} Track {:05}.mp3",
                    artist + 1,
                    album + 1,
                    (i % 12) + 1,
                    i + 1
                ),
                dir: format!("Artist {:04}/Album {:04}", artist + 1, album + 1),
                file: format!("{:02} Track {:05}.mp3", (i % 12) + 1, i + 1),
                tags,
                secs: 180,
                fmt: 1,
            }
        })
        .collect()
}
