//! Community Analogue Pocket image packs index and release zip cache.
//!
//! Reads the public `pupdate` image-pack directory from GitHub (`INDEX_URL`)
//! and fetches release `.zip` assets via GitHub's HTML `releases/latest` +
//! `releases/expanded_assets/<tag>` endpoints so unauthenticated users do not
//! burn the 60/hour REST API limit.

use crate::{ErrorCode, TauError};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use tau_core::artwork;

pub const INDEX_URL: &str =
    "https://raw.githubusercontent.com/mattpannella/pupdate/main/image_packs.json";

const INDEX_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const INDEX_LIMIT: u64 = 1024 * 1024;
const HTML_LIMIT: u64 = 2 * 1024 * 1024;
const PACK_ZIP_LIMIT: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
struct RawPackEntry {
    owner: String,
    repository: String,
    #[serde(default)]
    variant: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImagePackEntry {
    pub id: String,
    pub owner: String,
    pub repository: String,
    pub variant: Option<String>,
    pub name: String,
    pub cached: bool,
    pub cached_platforms: usize,
}

fn err(code: ErrorCode, message: impl Into<String>) -> TauError {
    TauError {
        code,
        message: message.into(),
    }
}

fn valid_slug(s: &str) -> bool {
    !s.is_empty()
        && !s.contains("..")
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

pub fn make_pack_id(owner: &str, repository: &str, variant: Option<&str>) -> String {
    match variant.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => format!("{owner}/{repository}#{v}"),
        None => format!("{owner}/{repository}"),
    }
}

fn friendly_pack_name(owner: &str, repository: &str, variant: Option<&str>) -> String {
    let base = repository.replace(['-', '_'], " ");
    match variant.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => format!("{owner} · {base} ({})", v.replace('_', " ")),
        None => format!("{owner} · {base}"),
    }
}

/// Returns the local cache path for a pack zip inside `<cache_dir>/packs/`.
pub fn pack_zip_path(cache_dir: &Path, pack_id: &str) -> PathBuf {
    let safe: String = pack_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    cache_dir.join("packs").join(format!("{safe}.zip"))
}

fn parse_index_bytes(bytes: &[u8], cache_dir: &Path) -> Result<Vec<ImagePackEntry>, TauError> {
    let raw: Vec<RawPackEntry> = serde_json::from_slice(bytes)
        .map_err(|e| err(ErrorCode::Json, format!("Invalid image packs index: {e}")))?;
    let mut out = Vec::with_capacity(raw.len());
    for item in raw {
        if !valid_slug(&item.owner) || !valid_slug(&item.repository) {
            continue;
        }
        let variant = item
            .variant
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        if let Some(v) = &variant
            && !valid_slug(v)
        {
            continue;
        }
        let id = make_pack_id(&item.owner, &item.repository, variant.as_deref());
        let zip_path = pack_zip_path(cache_dir, &id);
        let (cached, cached_platforms) = if zip_path.is_file() {
            let count = artwork::list_pack_platforms(&zip_path)
                .map(|v| v.len())
                .unwrap_or(0);
            (true, count)
        } else {
            (false, 0)
        };
        let name = friendly_pack_name(&item.owner, &item.repository, variant.as_deref());
        out.push(ImagePackEntry {
            id,
            owner: item.owner,
            repository: item.repository,
            variant,
            name,
            cached,
            cached_platforms,
        });
    }
    Ok(out)
}

fn is_fresh(path: &Path, ttl: Duration) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|age| age < ttl)
        .unwrap_or(false)
}

fn agent_default() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(90)))
        .user_agent(concat!("Tau-Omega/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn explain(error: &ureq::Error) -> String {
    match error {
        ureq::Error::StatusCode(403 | 429) => {
            "GitHub is limiting requests from this connection right now. Try again later.".into()
        }
        ureq::Error::StatusCode(404) => "That image pack was not found on GitHub.".into(),
        ureq::Error::StatusCode(code) => format!("GitHub answered with an error ({code})."),
        ureq::Error::Timeout(_) => {
            "GitHub did not answer in time. Check your connection and try again.".into()
        }
        ureq::Error::Io(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => {
            "Could not reach GitHub. Are you offline?".into()
        }
        other => format!("The download failed: {other}"),
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), TauError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let part = path.with_extension("part");
    fs::write(&part, bytes)?;
    fs::rename(&part, path)?;
    Ok(())
}

/// Fetches (and caches) the community image pack list from `INDEX_URL`.
pub fn fetch_index(cache_dir: &Path, force: bool) -> Result<Vec<ImagePackEntry>, TauError> {
    let index_path = cache_dir.join("index.json");
    if !force
        && is_fresh(&index_path, INDEX_TTL)
        && let Ok(bytes) = fs::read(&index_path)
    {
        return parse_index_bytes(&bytes, cache_dir);
    }

    let agent = agent_default();
    let fetched = agent
        .get(INDEX_URL)
        .call()
        .map_err(|e| err(ErrorCode::Io, explain(&e)))
        .and_then(|mut resp| {
            resp.body_mut()
                .with_config()
                .limit(INDEX_LIMIT)
                .read_to_vec()
                .map_err(|e| err(ErrorCode::Io, explain(&e)))
        });

    match fetched {
        Ok(bytes) => {
            let parsed = parse_index_bytes(&bytes, cache_dir)?;
            let _ = write_atomic(&index_path, &bytes);
            Ok(parsed)
        }
        Err(e) => {
            if let Ok(cached_bytes) = fs::read(&index_path)
                && let Ok(parsed) = parse_index_bytes(&cached_bytes, cache_dir)
            {
                return Ok(parsed);
            }
            Err(e)
        }
    }
}

/// Resolves the latest release tag via `https://github.com/{owner}/{repo}/releases/latest` HTTP redirect
/// without touching the REST API rate limit.
fn resolve_latest_tag(owner: &str, repository: &str) -> Result<String, TauError> {
    if !valid_slug(owner) || !valid_slug(repository) {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "Invalid repository slug for image pack.",
        ));
    }
    let url = format!("https://github.com/{owner}/{repository}/releases/latest");
    let no_redirect: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(30)))
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent(concat!("Tau-Omega/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();

    let resp = no_redirect
        .get(&url)
        .call()
        .map_err(|e| err(ErrorCode::Io, explain(&e)))?;
    let status = resp.status().as_u16();
    if (300..400).contains(&status)
        && let Some(loc) = resp.headers().get("location").and_then(|v| v.to_str().ok())
        && let Some(tag) = tag_from_release_url(loc)
    {
        return Ok(tag);
    }
    Err(err(
        ErrorCode::NotFound,
        format!("No published release found for {owner}/{repository}."),
    ))
}

fn tag_from_release_url(url: &str) -> Option<String> {
    let idx = url.find("/releases/tag/")?;
    let rest = &url[idx + "/releases/tag/".len()..];
    let tag = rest.split(['?', '#']).next()?.trim_end_matches('/').trim();
    if tag.is_empty() {
        None
    } else {
        Some(tag.to_string())
    }
}

/// Extracts `.zip` download paths from GitHub's `releases/expanded_assets/<tag>` HTML fragment.
pub(crate) fn extract_zip_paths(
    html: &str,
    owner: &str,
    repository: &str,
    tag: &str,
) -> Vec<String> {
    let prefix = format!("/{owner}/{repository}/releases/download/{tag}/");
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(pos) = rest.find(&prefix) {
        let candidate = &rest[pos..];
        let end = candidate
            .find(['"', '\'', '<', '>', ' ', '\n', '\r'])
            .unwrap_or(candidate.len());
        let path = &candidate[..end];
        if path.to_ascii_lowercase().ends_with(".zip") && !out.iter().any(|p| p == path) {
            out.push(path.to_string());
        }
        rest = &candidate[end..];
    }
    out
}

/// Picks the best `.zip` asset path matching an optional `variant` (e.g. `"home"` never matches `"home_jp_alts"`).
pub(crate) fn select_zip_for_variant<'a>(
    paths: &'a [String],
    variant: Option<&str>,
) -> Option<&'a str> {
    let Some(v) = variant.map(str::trim).filter(|v| !v.is_empty()) else {
        return paths.first().map(String::as_str);
    };
    let v_lower = v.to_ascii_lowercase();

    // 1. Exact stem match (`Home_USA.zip` == `"Home_USA"`, `pocket-platform-overrides-home.zip` ends `-home`).
    for path in paths {
        let filename = path.rsplit('/').next().unwrap_or(path);
        let stem = filename
            .strip_suffix(".zip")
            .or_else(|| filename.strip_suffix(".ZIP"))
            .unwrap_or(filename)
            .to_ascii_lowercase();
        if stem == v_lower
            || stem.ends_with(&format!("-{v_lower}"))
            || stem.ends_with(&format!("_{v_lower}"))
        {
            return Some(path.as_str());
        }
    }

    // 2. Fallback: token boundary match split on common separators (`-`, `_`, `.`).
    let v_tokens: Vec<&str> = v_lower
        .split(['-', '_', '.'])
        .filter(|t| !t.is_empty())
        .collect();
    for path in paths {
        let filename = path.rsplit('/').next().unwrap_or(path);
        let stem = filename
            .strip_suffix(".zip")
            .or_else(|| filename.strip_suffix(".ZIP"))
            .unwrap_or(filename)
            .to_ascii_lowercase();
        let file_tokens: Vec<&str> = stem
            .split(['-', '_', '.'])
            .filter(|t| !t.is_empty())
            .collect();
        if !v_tokens.is_empty()
            && file_tokens.len() >= v_tokens.len()
            && file_tokens[file_tokens.len() - v_tokens.len()..] == v_tokens[..]
        {
            return Some(path.as_str());
        }
    }

    None
}

/// Ensures the release zip for `entry` is present in `<cache_dir>/packs/` and returns its path.
pub fn ensure_pack_zip(
    cache_dir: &Path,
    entry: &ImagePackEntry,
    force: bool,
) -> Result<PathBuf, TauError> {
    if !valid_slug(&entry.owner) || !valid_slug(&entry.repository) {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "Invalid repository slug for image pack.",
        ));
    }
    let zip_path = pack_zip_path(cache_dir, &entry.id);
    if !force && zip_path.is_file() {
        return Ok(zip_path);
    }

    let tag = resolve_latest_tag(&entry.owner, &entry.repository)?;
    let expanded_url = format!(
        "https://github.com/{}/{}/releases/expanded_assets/{tag}",
        entry.owner, entry.repository
    );
    let agent = agent_default();
    let html_bytes = agent
        .get(&expanded_url)
        .call()
        .map_err(|e| err(ErrorCode::Io, explain(&e)))?
        .body_mut()
        .with_config()
        .limit(HTML_LIMIT)
        .read_to_vec()
        .map_err(|e| err(ErrorCode::Io, explain(&e)))?;
    let html = String::from_utf8_lossy(&html_bytes);
    let zips = extract_zip_paths(&html, &entry.owner, &entry.repository, &tag);
    let rel_path = select_zip_for_variant(&zips, entry.variant.as_deref()).ok_or_else(|| {
        err(
            ErrorCode::NotFound,
            format!("Could not find a downloadable zip for “{}”.", entry.name),
        )
    })?;

    let expected_prefix = format!(
        "https://github.com/{}/{}/releases/download/",
        entry.owner, entry.repository
    );
    let download_url = format!("https://github.com{rel_path}");
    if !download_url.starts_with(&expected_prefix) {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "Refused non-release download URL for image pack.",
        ));
    }

    let zip_bytes = agent
        .get(&download_url)
        .call()
        .map_err(|e| err(ErrorCode::Io, explain(&e)))?
        .body_mut()
        .with_config()
        .limit(PACK_ZIP_LIMIT)
        .read_to_vec()
        .map_err(|e| err(ErrorCode::Io, explain(&e)))?;

    write_atomic(&zip_path, &zip_bytes)?;
    // Verify the downloaded file parses as a valid zip.
    let _ = artwork::list_pack_platforms(&zip_path)?;
    Ok(zip_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_matching_distinguishes_home_from_alts() {
        let paths = vec![
            "/dyreschlock/pocket-platform-images/releases/download/v5.2.0/pocket-platform-overrides-arcade.zip".into(),
            "/dyreschlock/pocket-platform-images/releases/download/v5.2.0/pocket-platform-overrides-home.zip".into(),
            "/dyreschlock/pocket-platform-images/releases/download/v5.2.0/pocket-platform-overrides-home_jp_alts.zip".into(),
            "/dyreschlock/pocket-platform-images/releases/download/v5.2.0/pocket-platform-overrides-home_pal_alts.zip".into(),
        ];

        assert_eq!(
            select_zip_for_variant(&paths, Some("home")),
            Some(paths[1].as_str())
        );
        assert_eq!(
            select_zip_for_variant(&paths, Some("home_jp_alts")),
            Some(paths[2].as_str())
        );
        assert_eq!(
            select_zip_for_variant(&paths, None),
            Some(paths[0].as_str())
        );

        let shissa = vec![
            "/Shissa43/Analogue-Pocket-Platform-Art/releases/download/v0.2.7/Arcade_Multi.zip"
                .into(),
            "/Shissa43/Analogue-Pocket-Platform-Art/releases/download/v0.2.7/Home_USA.zip".into(),
        ];
        assert_eq!(
            select_zip_for_variant(&shissa, Some("Home_USA")),
            Some(shissa[1].as_str())
        );
    }

    #[test]
    fn extracts_zip_links_from_expanded_assets_html() {
        let html = r#"
          <a href="/Shissa43/Analogue-Pocket-Platform-Art/releases/download/v0.2.7/Home_JP.zip">Home_JP.zip</a>
          <a href="/Shissa43/Analogue-Pocket-Platform-Art/releases/download/v0.2.7/Home_USA.zip">Home_USA.zip</a>
          <a href="/Shissa43/Analogue-Pocket-Platform-Art/releases/download/v0.2.7/notes.txt">notes</a>
        "#;
        let found = extract_zip_paths(html, "Shissa43", "Analogue-Pocket-Platform-Art", "v0.2.7");
        assert_eq!(found.len(), 2);
        assert!(found[0].ends_with("/Home_JP.zip"));
        assert!(found[1].ends_with("/Home_USA.zip"));
    }

    #[test]
    fn rejects_path_traversal_and_invalid_slugs() {
        assert!(!valid_slug("../evil"));
        assert!(!valid_slug("a/b"));
        assert!(!valid_slug(""));
        assert!(valid_slug("openFPGA-Platform-Art-Set"));

        let bad = ImagePackEntry {
            id: "bad".into(),
            owner: "../etc".into(),
            repository: "repo".into(),
            variant: None,
            name: "bad".into(),
            cached: false,
            cached_platforms: 0,
        };
        assert_eq!(
            ensure_pack_zip(Path::new("/tmp"), &bad, false)
                .unwrap_err()
                .code,
            ErrorCode::InvalidPathReference
        );
    }
}
