//! Per-game artwork matching, local cache key derivation, and override storage
//! (`libretro-thumbnails` by default, plus app-local user overrides).
//!
//! All storage stays on the host (app cache and app data directories); the SD
//! card is never modified. Keys are derived from canonical platform IDs and
//! game titles so future Library views reuse the same cache and overrides.

use crate::{ErrorCode, TauError, platforms};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

/// Base URL for Libretro's public CDN thumbnail repository.
pub const LIBRETRO_BASE_URL: &str = "https://thumbnails.libretro.com";

/// Maximum local custom image file size accepted for game artwork overrides (16 MiB).
pub const MAX_LOCAL_IMAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_OVERRIDE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum GameArtKind {
    #[default]
    Boxart,
    Title,
    Snap,
}

impl GameArtKind {
    pub fn libretro_dir(self) -> &'static str {
        match self {
            Self::Boxart => "Named_Boxarts",
            Self::Title => "Named_Titles",
            Self::Snap => "Named_Snaps",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Boxart => "Boxart",
            Self::Title => "Title screen",
            Self::Snap => "Screenshot",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Boxart => "boxart",
            Self::Title => "title",
            Self::Snap => "snap",
        }
    }

    pub fn parse_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "title" => Self::Title,
            "snap" | "screenshot" => Self::Snap,
            _ => Self::Boxart,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum GameArtSourceKind {
    OverrideLocal,
    OverrideLibretro,
    LibretroAuto,
}

impl GameArtSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OverrideLocal => "override_local",
            Self::OverrideLibretro => "override_libretro",
            Self::LibretroAuto => "libretro_auto",
        }
    }

    pub fn parse_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "override_local" => Self::OverrideLocal,
            "override_libretro" => Self::OverrideLibretro,
            _ => Self::LibretroAuto,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameArtResult {
    pub rom_name: String,
    pub platform_id: String,
    pub game_key: String,
    pub data_url: String,
    pub kind: GameArtKind,
    pub source: GameArtSourceKind,
    pub matched_name: Option<String>,
    pub libretro_system: Option<String>,
    pub has_override: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameArtCandidate {
    pub system: String,
    pub kind: GameArtKind,
    pub name: String,
    pub data_url: String,
}

/// Resolves any platform alias to its canonical platform ID (or lowercased input if custom).
pub fn canonical_platform_id(platform_id: &str) -> String {
    let trimmed = platform_id.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(spec) = platforms::find(trimmed) {
        return spec.id.to_string();
    }
    trimmed.to_ascii_lowercase()
}

/// Returns the ordered Libretro database/thumbnail system names for a canonical or aliased platform ID.
pub fn libretro_systems_for_platform(platform_id: &str) -> Vec<&'static str> {
    platforms::find(platform_id)
        .map(|p| p.libretro_systems.to_vec())
        .unwrap_or_default()
}

/// Returns the filename without directory components or a final short alphanumeric extension.
pub fn strip_rom_extension(rom_name: &str) -> &str {
    let base = rom_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(rom_name)
        .trim();
    if let Some((stem, ext)) = base.rsplit_once('.')
        && !stem.is_empty()
        && !ext.is_empty()
        && ext.len() <= 8
        && ext.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return stem;
    }
    base
}

/// Clean game title shown on grid cards and sheet headers (filename stem without extension).
pub fn display_title(rom_name: &str) -> String {
    strip_rom_extension(rom_name).trim().to_string()
}

/// Device-agnostic canonical cache/override key shared across Core Files and future Library views.
pub fn canonical_game_key(platform_id: &str, rom_name: &str) -> String {
    let plat = canonical_platform_id(platform_id);
    let stem = strip_rom_extension(rom_name).trim().to_ascii_lowercase();
    format!("{plat}:{stem}")
}

/// Collision-free, filesystem-safe 24-hex-character SHA-256 stem for any key string.
pub fn game_key_hash(key: &str) -> String {
    let digest = Sha256::digest(key.as_bytes());
    let mut out = String::with_capacity(24);
    for byte in &digest[..12] {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Replaces characters that `libretro-thumbnails` swaps for `_` in file names.
pub fn libretro_sanitize(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '&' | '*' | '/' | ':' | '`' | '<' | '>' | '?' | '\\' | '|' | '"' => '_',
            other => other,
        })
        .collect()
}

/// Percent-encodes one URL path segment using RFC 3986 unreserved characters.
pub fn percent_encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.as_bytes() {
        let is_unreserved =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if is_unreserved {
            out.push(*byte as char);
        } else {
            use std::fmt::Write as _;
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// Decodes `%XX` sequences in UTF-8 URL segments, keeping malformed sequences literal.
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Directory listing URL on `thumbnails.libretro.com` for one system & artwork kind.
pub fn libretro_index_url(system: &str, kind: GameArtKind) -> String {
    format!(
        "{LIBRETRO_BASE_URL}/{}/{}/",
        percent_encode_segment(system),
        kind.libretro_dir()
    )
}

/// Direct PNG thumbnail URL on `thumbnails.libretro.com` for an entry name.
pub fn libretro_image_url(system: &str, kind: GameArtKind, entry_name: &str) -> String {
    let clean = libretro_sanitize(entry_name);
    format!(
        "{LIBRETRO_BASE_URL}/{}/{}/{}.png",
        percent_encode_segment(system),
        kind.libretro_dir(),
        percent_encode_segment(&clean)
    )
}

fn unescape_basic_html(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// Parses an nginx/Apache directory listing HTML page from `thumbnails.libretro.com`
/// and returns sorted, deduplicated entry names (without `.png`).
pub fn parse_directory_index_html(html: &str) -> Vec<String> {
    let mut set = BTreeSet::new();
    let lower = html.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut pos = 0;

    while let Some(rel) = lower[pos..].find("href=") {
        let mut start = pos + rel + 5;
        while start < bytes.len() && bytes[start].is_ascii_whitespace() {
            start += 1;
        }
        if start >= bytes.len() {
            break;
        }
        let quote = bytes[start];
        let (raw_href, next_pos) = if quote == b'"' || quote == b'\'' {
            let val_start = start + 1;
            let Some(end_rel) = html[val_start..].find(quote as char) else {
                break;
            };
            (
                &html[val_start..val_start + end_rel],
                val_start + end_rel + 1,
            )
        } else {
            let mut end = start;
            while end < bytes.len() && !bytes[end].is_ascii_whitespace() && bytes[end] != b'>' {
                end += 1;
            }
            (&html[start..end], end)
        };
        pos = next_pos;

        let href = raw_href.trim();
        if href.is_empty()
            || href.starts_with('#')
            || href.starts_with('?')
            || href.contains("..")
            || href.starts_with("http://")
            || href.starts_with("https://")
        {
            continue;
        }
        let segment = href.rsplit('/').next().unwrap_or(href);
        let segment_no_q = segment.split('?').next().unwrap_or(segment);
        if segment_no_q.len() <= 4 || !segment_no_q.to_ascii_lowercase().ends_with(".png") {
            continue;
        }
        let stem_raw = &segment_no_q[..segment_no_q.len() - 4];
        let decoded = unescape_basic_html(&percent_decode(stem_raw))
            .trim()
            .to_string();
        if !decoded.is_empty() {
            set.insert(decoded);
        }
    }

    set.into_iter().collect()
}

fn strip_leading_track_number(s: &str) -> &str {
    let trimmed = s.trim_start();
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i > 5 {
        return trimmed;
    }
    let rest = &trimmed[i..];
    if let Some(after) = rest.strip_prefix(" - ") {
        return after.trim_start();
    }
    if let Some(after) = rest.strip_prefix(". ") {
        return after.trim_start();
    }
    trimmed
}

fn strip_bracket_groups(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    for ch in s.chars() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ if paren_depth == 0 && bracket_depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Extracts normalized region and pre-release/variant tokens from `(...)` groups.
pub fn extract_region_tags(raw: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut pos = 0;
    while let Some(open) = raw[pos..].find('(') {
        let start = pos + open + 1;
        let Some(close_rel) = raw[start..].find(')') else {
            break;
        };
        let inner = &raw[start..start + close_rel];
        pos = start + close_rel + 1;
        for part in inner.split(',') {
            let t = part.trim().to_ascii_lowercase();
            if t.is_empty() {
                continue;
            }
            match t.as_str() {
                "u" => tags.push("usa".into()),
                "e" => tags.push("europe".into()),
                "j" => tags.push("japan".into()),
                "w" => tags.push("world".into()),
                _ => tags.push(t),
            }
        }
    }
    tags
}

/// Produces a normalized title key for matching ROM names against Libretro thumbnail entries.
pub fn normalize_title(raw: &str) -> String {
    let stem = strip_rom_extension(raw);
    let no_num = strip_leading_track_number(stem);
    let no_groups = strip_bracket_groups(no_num);
    let ascii = deunicode::deunicode(&no_groups);

    // Strip punctuation (including `&` and `_`, since Libretro filenames turn `&` into `_`) so
    // `"Mario & Luigi"` and Libretro's `"Mario _ Luigi"` normalize identically.
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in ascii.chars() {
        if ch.is_ascii_alphanumeric() {
            cur.push(ch.to_ascii_lowercase());
        } else if !cur.is_empty() {
            tokens.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }

    // Normalize leading/trailing articles (`"Legend of Zelda, The"` <-> `"The Legend of Zelda"`).
    if tokens.len() >= 2 && matches!(tokens.first().map(String::as_str), Some("the" | "a" | "an")) {
        tokens.remove(0);
    }
    if tokens.len() >= 2 && matches!(tokens.last().map(String::as_str), Some("the" | "a" | "an")) {
        tokens.pop();
    }

    tokens.join(" ")
}

fn score_candidate(rom_tags: &[String], entry: &str) -> i32 {
    let entry_tags = extract_region_tags(entry);
    let mut score: i32 = 0;

    // Reward region alignment with tags present on the user's ROM filename.
    for rtag in rom_tags {
        if matches!(
            rtag.as_str(),
            "usa" | "europe" | "japan" | "world" | "en" | "fr" | "de" | "es" | "it"
        ) && entry_tags.iter().any(|e| e == rtag)
        {
            score += 50;
        }
    }

    // Default clean release preference when ROM lacks region hints.
    if entry_tags.iter().any(|t| t == "usa") {
        score += 20;
    } else if entry_tags.iter().any(|t| t == "world") {
        score += 18;
    } else if entry_tags.iter().any(|t| t == "europe") {
        score += 15;
    } else if entry_tags.iter().any(|t| t == "japan") {
        score += 10;
    }

    if entry_tags.iter().any(|t| {
        t.contains("beta")
            || t.contains("proto")
            || t.contains("sample")
            || t.contains("demo")
            || t.contains("kiosk")
            || t == "unl"
    }) {
        score -= 40;
    }
    if entry_tags.iter().any(|t| {
        t.contains("virtual console")
            || t.contains("switch online")
            || t.contains("classic mini")
            || t.contains("evercade")
    }) {
        score -= 5;
    }

    score
}

/// Finds the best matching Libretro thumbnail entry name for `rom_name`.
pub fn match_in_index(rom_name: &str, entries: &[String]) -> Option<String> {
    if entries.is_empty() {
        return None;
    }
    let clean_stem = libretro_sanitize(strip_rom_extension(rom_name))
        .trim()
        .to_string();
    if clean_stem.is_empty() {
        return None;
    }

    // Tier 1: exact case-insensitive filename stem match.
    for entry in entries {
        if entry.eq_ignore_ascii_case(&clean_stem) {
            return Some(entry.clone());
        }
    }

    let query_norm = normalize_title(rom_name);
    if query_norm.is_empty() {
        return None;
    }
    let rom_tags = extract_region_tags(rom_name);

    // Tier 2: exact normalized title match, ranked by region/status tags.
    let mut exact_norm_hits: Vec<(i32, usize, &str)> = Vec::new();
    for entry in entries {
        if normalize_title(entry) == query_norm {
            let s = score_candidate(&rom_tags, entry);
            exact_norm_hits.push((s, entry.len(), entry.as_str()));
        }
    }
    if !exact_norm_hits.is_empty() {
        exact_norm_hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(b.2)));
        return Some(exact_norm_hits[0].2.to_string());
    }

    // Tier 3: high-confidence token Jaccard match (>= 0.80) for slightly varied punctuation/subtitles.
    let q_tokens: BTreeSet<&str> = query_norm.split_whitespace().collect();
    if q_tokens.len() >= 2 && query_norm.len() >= 6 {
        let mut best: Option<(u32, i32, usize, &str)> = None;
        for entry in entries {
            let e_norm = normalize_title(entry);
            if e_norm.is_empty() {
                continue;
            }
            let e_tokens: BTreeSet<&str> = e_norm.split_whitespace().collect();
            let inter = q_tokens.intersection(&e_tokens).count();
            if inter == 0 {
                continue;
            }
            let union = q_tokens.union(&e_tokens).count();
            let sim_milli = ((inter * 1000) / union) as u32;
            if sim_milli >= 800 {
                let tag_score = score_candidate(&rom_tags, entry);
                let cand = (sim_milli, tag_score, entry.len(), entry.as_str());
                let replace = match best {
                    None => true,
                    Some(cur) => {
                        cand.0 > cur.0
                            || (cand.0 == cur.0 && cand.1 > cur.1)
                            || (cand.0 == cur.0 && cand.1 == cur.1 && cand.2 < cur.2)
                    }
                };
                if replace {
                    best = Some(cand);
                }
            }
        }
        if let Some(hit) = best {
            return Some(hit.3.to_string());
        }
    }

    None
}

/// Searches a Libretro directory index for interactive picker candidates, ranked best-first.
pub fn search_index(query: &str, entries: &[String], limit: usize) -> Vec<String> {
    let q_norm = normalize_title(query);
    if q_norm.is_empty() || entries.is_empty() || limit == 0 {
        return Vec::new();
    }
    let q_tokens: Vec<&str> = q_norm.split_whitespace().collect();
    let rom_tags = extract_region_tags(query);

    let mut ranked: Vec<(i32, usize, &str)> = Vec::new();
    for entry in entries {
        let e_norm = normalize_title(entry);
        if e_norm.is_empty() {
            continue;
        }
        let match_score = if e_norm == q_norm {
            1000
        } else if e_norm.starts_with(&q_norm) {
            750
        } else if e_norm.contains(&q_norm) {
            550
        } else {
            let e_tokens: Vec<&str> = e_norm.split_whitespace().collect();
            let matched = q_tokens
                .iter()
                .filter(|qt| e_tokens.iter().any(|et| et == *qt || et.starts_with(*qt)))
                .count();
            if matched == q_tokens.len() {
                400
            } else if matched * 2 >= q_tokens.len() && matched > 0 {
                ((matched * 250) / q_tokens.len()) as i32
            } else {
                0
            }
        };
        if match_score == 0 {
            continue;
        }
        let total = match_score + score_candidate(&rom_tags, entry);
        ranked.push((total, entry.len(), entry.as_str()));
    }

    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(b.2)));
    ranked
        .into_iter()
        .take(limit)
        .map(|(_, _, name)| name.to_string())
        .collect()
}

/// Sniffs supported image MIME types from header magic bytes.
pub fn sniff_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else {
        None
    }
}

/// Validates raw image bytes and encodes them as a `data:<mime>;base64,...` URL.
pub fn image_data_url(bytes: &[u8]) -> Result<String, TauError> {
    let mime = sniff_image_mime(bytes).ok_or_else(|| {
        TauError::e(
            ErrorCode::UnsupportedCover,
            "Unsupported image file: choose a PNG, JPEG, GIF or WebP image.",
        )
    })?;
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

/// Reads and validates a user-picked local image file as a base64 data URL (no resizing/crop).
pub fn read_local_preview_data_url(path: &Path) -> Result<String, TauError> {
    let meta = fs::metadata(path).map_err(|_| {
        TauError::e(
            ErrorCode::SourceMissing,
            format!("Image file not found: {}", path.display()),
        )
    })?;
    if !meta.is_file() {
        return Err(TauError::e(
            ErrorCode::SourceMissing,
            format!("Not a regular file: {}", path.display()),
        ));
    }
    if meta.len() as usize > MAX_LOCAL_IMAGE_BYTES {
        return Err(TauError::e(
            ErrorCode::UnsupportedCover,
            "Image is larger than 16 MB; choose a smaller file.",
        ));
    }
    let bytes = fs::read(path)?;
    image_data_url(&bytes)
}

fn safe_platform_dir_segment(platform_id: &str) -> String {
    let canon = canonical_platform_id(platform_id);
    let clean: String = canon
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() {
        "_unknown".into()
    } else {
        clean
    }
}

fn override_paths(
    overrides_root: &Path,
    platform_id: &str,
    rom_name: &str,
) -> (PathBuf, PathBuf, String, String) {
    let plat = canonical_platform_id(platform_id);
    let key = canonical_game_key(&plat, rom_name);
    let hash = game_key_hash(&key);
    let dir = overrides_root.join(safe_platform_dir_segment(&plat));
    (
        dir.join(format!("{hash}.img")),
        dir.join(format!("{hash}.json")),
        plat,
        key,
    )
}

/// Reads a saved per-game local override if one exists.
pub fn read_override(
    overrides_root: &Path,
    platform_id: &str,
    rom_name: &str,
) -> Result<Option<GameArtResult>, TauError> {
    let (img_path, meta_path, plat, game_key) =
        override_paths(overrides_root, platform_id, rom_name);
    let meta = match fs::metadata(&img_path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(TauError::from(e)),
    };
    if !meta.is_file() || meta.len() > MAX_OVERRIDE_BYTES {
        return Ok(None);
    }
    let bytes = fs::read(&img_path)?;
    let Ok(data_url) = image_data_url(&bytes) else {
        return Ok(None);
    };

    let mut kind = GameArtKind::Boxart;
    let mut source = GameArtSourceKind::OverrideLocal;
    let mut matched_name = None;
    let mut libretro_system = None;

    if let Ok(raw_json) = fs::read_to_string(&meta_path)
        && let Ok(val) = serde_json::from_str::<Value>(&raw_json)
    {
        if let Some(k) = val.get("kind").and_then(Value::as_str) {
            kind = GameArtKind::parse_str(k);
        }
        if let Some(s) = val.get("source").and_then(Value::as_str) {
            source = GameArtSourceKind::parse_str(s);
        }
        matched_name = val
            .get("matched_name")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        libretro_system = val
            .get("libretro_system")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }

    Ok(Some(GameArtResult {
        rom_name: rom_name.to_string(),
        platform_id: plat,
        game_key,
        data_url,
        kind,
        source,
        matched_name,
        libretro_system,
        has_override: true,
    }))
}

/// Writes a per-game local override atomically inside `overrides_root`. Never touches SD cards.
#[allow(clippy::too_many_arguments)]
pub fn write_override_bytes(
    overrides_root: &Path,
    platform_id: &str,
    rom_name: &str,
    bytes: &[u8],
    kind: GameArtKind,
    source: GameArtSourceKind,
    matched_name: Option<&str>,
    libretro_system: Option<&str>,
) -> Result<GameArtResult, TauError> {
    if bytes.len() > MAX_LOCAL_IMAGE_BYTES {
        return Err(TauError::e(
            ErrorCode::UnsupportedCover,
            "Image is larger than 16 MB; choose a smaller file.",
        ));
    }
    let data_url = image_data_url(bytes)?;
    let (img_path, meta_path, plat, game_key) =
        override_paths(overrides_root, platform_id, rom_name);
    if let Some(parent) = img_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let img_part = img_path.with_extension("img.part");
    fs::write(&img_part, bytes)?;
    fs::rename(&img_part, &img_path)?;

    let sidecar = json!({
        "platform_id": plat,
        "rom_name": rom_name,
        "game_key": game_key,
        "kind": kind.as_str(),
        "source": source.as_str(),
        "matched_name": matched_name,
        "libretro_system": libretro_system,
    });
    let meta_bytes = serde_json::to_vec_pretty(&sidecar)
        .map_err(|e| TauError::e(ErrorCode::Json, e.to_string()))?;
    let meta_part = meta_path.with_extension("json.part");
    fs::write(&meta_part, meta_bytes)?;
    fs::rename(&meta_part, &meta_path)?;

    Ok(GameArtResult {
        rom_name: rom_name.to_string(),
        platform_id: plat,
        game_key,
        data_url,
        kind,
        source,
        matched_name: matched_name.map(ToString::to_string),
        libretro_system: libretro_system.map(ToString::to_string),
        has_override: true,
    })
}

/// Reads a local image file and saves it as a per-game override.
pub fn write_override_from_file(
    overrides_root: &Path,
    platform_id: &str,
    rom_name: &str,
    source_path: &Path,
) -> Result<GameArtResult, TauError> {
    let meta = fs::metadata(source_path).map_err(|_| {
        TauError::e(
            ErrorCode::SourceMissing,
            format!("Image file not found: {}", source_path.display()),
        )
    })?;
    if !meta.is_file() {
        return Err(TauError::e(
            ErrorCode::SourceMissing,
            format!("Not a regular file: {}", source_path.display()),
        ));
    }
    if meta.len() as usize > MAX_LOCAL_IMAGE_BYTES {
        return Err(TauError::e(
            ErrorCode::UnsupportedCover,
            "Image is larger than 16 MB; choose a smaller file.",
        ));
    }
    let bytes = fs::read(source_path)?;
    let file_label = source_path
        .file_name()
        .and_then(|s| s.to_str())
        .map(ToString::to_string);
    write_override_bytes(
        overrides_root,
        platform_id,
        rom_name,
        &bytes,
        GameArtKind::Boxart,
        GameArtSourceKind::OverrideLocal,
        file_label.as_deref(),
        None,
    )
}

/// Clears any per-game override files. Returns `true` if an override existed.
pub fn clear_override(
    overrides_root: &Path,
    platform_id: &str,
    rom_name: &str,
) -> Result<bool, TauError> {
    let (img_path, meta_path, _, _) = override_paths(overrides_root, platform_id, rom_name);
    let mut removed = false;
    for path in [&img_path, &meta_path] {
        match fs::remove_file(path) {
            Ok(()) => removed = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(TauError::from(e)),
        }
    }
    Ok(removed)
}

/// Groups multiple candidate keys by normalized canonical key (helper for batch tests/telemetry).
pub fn group_roms_by_key<'a>(
    platform_id: &str,
    rom_names: &'a [String],
) -> BTreeMap<String, &'a str> {
    let mut map = BTreeMap::new();
    for rom in rom_names {
        map.insert(canonical_game_key(platform_id, rom), rom.as_str());
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let uniq = crate::test_uniq();
        let dir = std::env::temp_dir().join(format!(
            "tau-game-art-{label}-{}-{nanos}-{uniq}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn tiny_png() -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, 2, 2);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .unwrap()
            .write_image_data(&[
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ])
            .unwrap();
        out
    }

    #[test]
    fn sanitize_and_urls_match_libretro_rules() {
        assert_eq!(
            libretro_sanitize("Mario & Luigi: Superstar Saga (USA)"),
            "Mario _ Luigi_ Superstar Saga (USA)"
        );
        assert_eq!(
            libretro_image_url(
                "Nintendo - Game Boy Advance",
                GameArtKind::Boxart,
                "Mario & Luigi: Superstar Saga (USA)"
            ),
            "https://thumbnails.libretro.com/Nintendo%20-%20Game%20Boy%20Advance/Named_Boxarts/Mario%20_%20Luigi_%20Superstar%20Saga%20%28USA%29.png"
        );
    }

    #[test]
    fn html_directory_listing_parses_decodes_and_skips_parents() {
        let html = r#"
        <html><body><pre>
        <a href="../">../</a>
        <a href="Advance%20Wars%20%28USA%29.png">Advance Wars (USA).png</a>
        <a href='Boktai%202%20-%20Solar%20Boy%20Django%20%28USA%29.png?v=1'>Boktai 2</a>
        <a href="Mario%20%26%20Luigi_%20Superstar%20Saga%20%28USA%29.png">M&amp;L</a>
        <a href="readme.txt">readme.txt</a>
        <a href="https://example.com/external.png">ext</a>
        </pre></body></html>
        "#;
        let entries = parse_directory_index_html(html);
        assert_eq!(
            entries,
            vec![
                "Advance Wars (USA)",
                "Boktai 2 - Solar Boy Django (USA)",
                "Mario & Luigi_ Superstar Saga (USA)",
            ]
        );
    }

    #[test]
    fn matches_nointro_clean_goodtools_numbered_the_and_regions() {
        let entries = vec![
            "Chrono Trigger (Beta)".into(),
            "Chrono Trigger (Japan)".into(),
            "Chrono Trigger (USA)".into(),
            "Legend of Zelda, The - A Link to the Past (Europe)".into(),
            "Legend of Zelda, The - A Link to the Past (USA)".into(),
            "Mario _ Luigi_ Superstar Saga (USA)".into(),
            "Mother 3 (Japan)".into(),
            "Pokemon - Red Version (USA, Europe)".into(),
            "Super Metroid (Japan, USA) (En,Ja)".into(),
            "Yoshi's Island - Super Mario Advance 3 (USA)".into(),
        ];

        assert_eq!(
            match_in_index("Super Metroid.sfc", &entries).as_deref(),
            Some("Super Metroid (Japan, USA) (En,Ja)")
        );
        assert_eq!(
            match_in_index("Mario & Luigi - Superstar Saga.gba", &entries).as_deref(),
            Some("Mario _ Luigi_ Superstar Saga (USA)")
        );
        assert_eq!(
            match_in_index("Chrono Trigger (U) [!].sfc", &entries).as_deref(),
            Some("Chrono Trigger (USA)")
        );
        assert_eq!(
            match_in_index("001 - Pokemon - Red Version.gb", &entries).as_deref(),
            Some("Pokemon - Red Version (USA, Europe)")
        );
        assert_eq!(
            match_in_index(
                "The Legend of Zelda - A Link to the Past (Europe).sfc",
                &entries
            )
            .as_deref(),
            Some("Legend of Zelda, The - A Link to the Past (Europe)")
        );
        assert_eq!(
            match_in_index("Mother 3.gba", &entries).as_deref(),
            Some("Mother 3 (Japan)")
        );
        assert_eq!(
            match_in_index("Definitely Not A Real Game 999.sfc", &entries),
            None
        );
    }

    #[test]
    fn search_index_ranks_exact_starts_and_partial_hits() {
        let entries = vec![
            "Advance Wars (USA)".into(),
            "Advance Wars 2 - Black Hole Rising (USA)".into(),
            "Mario & Luigi: Superstar Saga (USA)".into(),
            "Super Mario Advance 4 - Super Mario Bros. 3 (USA)".into(),
        ];
        let hits = search_index("Advance Wars", &entries, 10);
        assert_eq!(hits[0], "Advance Wars (USA)");
        assert_eq!(hits[1], "Advance Wars 2 - Black Hole Rising (USA)");
    }

    #[test]
    fn overrides_round_trip_and_clear() {
        let root = scratch_dir("ovr");
        let png_bytes = tiny_png();
        assert!(
            read_override(&root, "gameboyadvance", "Mother 3.gba")
                .unwrap()
                .is_none()
        );

        let written = write_override_bytes(
            &root,
            "gameboyadvance",
            "Mother 3.gba",
            &png_bytes,
            GameArtKind::Title,
            GameArtSourceKind::OverrideLibretro,
            Some("Mother 3 (Japan)"),
            Some("Nintendo - Game Boy Advance"),
        )
        .unwrap();
        assert_eq!(written.platform_id, "gba");
        assert_eq!(written.game_key, "gba:mother 3");
        assert!(written.has_override);
        assert!(written.data_url.starts_with("data:image/png;base64,"));

        let read_back = read_override(&root, "gba", "roms/Mother 3.gba")
            .unwrap()
            .unwrap();
        assert_eq!(read_back.kind, GameArtKind::Title);
        assert_eq!(read_back.source, GameArtSourceKind::OverrideLibretro);
        assert_eq!(read_back.matched_name.as_deref(), Some("Mother 3 (Japan)"));

        assert!(clear_override(&root, "gba", "Mother 3.gba").unwrap());
        assert!(!clear_override(&root, "gba", "Mother 3.gba").unwrap());
        assert!(
            read_override(&root, "gba", "Mother 3.gba")
                .unwrap()
                .is_none()
        );

        let _ = fs::remove_dir_all(&root);
    }
}
