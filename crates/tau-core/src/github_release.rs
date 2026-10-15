//! Reading a GitHub repository's releases for a manual add (`docs/MARKETPLACE_SPEC.md` section 7.1): parse the API's JSON, choose the zip, find the
//! pictures in a README. Pure and tolerant (unknown fields ignored, a bad entry skipped); the requests live in `tau_net::github_source`.

use crate::{
    ErrorCode, TauError,
    catalog_release::{DownloadAddress, parse_download_url},
};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GhAsset {
    pub name: String,
    pub size: u64,
    pub url: String,
    /// Lowercase hex SHA-256 from GitHub's per-asset digest, when it publishes one.
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GhRelease {
    pub tag: String,
    pub name: String,
    /// `YYYY-MM-DD`.
    pub published: String,
    pub prerelease: bool,
    pub assets: Vec<GhAsset>,
}

fn text(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn one(v: &Value) -> Option<GhRelease> {
    if v.get("draft").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let tag = text(v, "tag_name");
    if tag.is_empty() {
        return None;
    }
    let assets = v
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let (name, url) = (text(a, "name"), text(a, "browser_download_url"));
            (!name.is_empty() && !url.is_empty()).then(|| GhAsset {
                name,
                size: a.get("size").and_then(Value::as_u64).unwrap_or(0),
                url,
                sha256: a
                    .get("digest")
                    .and_then(Value::as_str)
                    .and_then(|d| d.strip_prefix("sha256:"))
                    .filter(|d| d.len() == 64 && d.chars().all(|c| c.is_ascii_hexdigit()))
                    .map(str::to_ascii_lowercase),
            })
        })
        .collect();
    Some(GhRelease {
        name: if text(v, "name").is_empty() {
            tag.clone()
        } else {
            text(v, "name")
        },
        published: text(v, "published_at").chars().take(10).collect(),
        prerelease: v
            .get("prerelease")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        tag,
        assets,
    })
}

/// From `GET /repos/<o>/<r>/releases`: the non-draft releases, in the order GitHub lists them (newest first).
pub fn parse_releases(bytes: &[u8]) -> Result<Vec<GhRelease>, TauError> {
    let json: Value = serde_json::from_slice(bytes).map_err(|e| {
        TauError::e(
            ErrorCode::Json,
            format!("GitHub's release list could not be read: {e}"),
        )
    })?;
    Ok(json
        .as_array()
        .map(|a| a.iter().filter_map(one).collect())
        .unwrap_or_default())
}

/// From `GET /repos/<o>/<r>/releases/tags/<tag>`.
pub fn parse_release(bytes: &[u8]) -> Result<GhRelease, TauError> {
    let json: Value = serde_json::from_slice(bytes).map_err(|e| {
        TauError::e(
            ErrorCode::Json,
            format!("GitHub's release could not be read: {e}"),
        )
    })?;
    one(&json).ok_or_else(|| TauError::e(ErrorCode::NotFound, "That release could not be read."))
}

/// The release's downloadable zips: named `.zip`, with a size, and at an address Omega accepts (a release asset).
pub fn zip_assets(r: &GhRelease) -> Vec<&GhAsset> {
    r.assets
        .iter()
        .filter(|a| {
            a.name.to_ascii_lowercase().ends_with(".zip")
                && a.size > 0
                && parse_download_url(&a.url).is_some_and(|d| d.is_release)
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick<'a> {
    One(&'a GhAsset),
    /// Several zips and nothing says which: the user chooses.
    Choose(Vec<&'a GhAsset>),
    None,
}

/// Which zip of a release: the only one; else the only one whose name starts with `hint` (a core id or a repository name); else the user chooses.
pub fn pick_asset<'a>(r: &'a GhRelease, hint: &str) -> Pick<'a> {
    let zips = zip_assets(r);
    match zips.len() {
        0 => Pick::None,
        1 => Pick::One(zips[0]),
        _ => {
            let h = hint.to_ascii_lowercase();
            let named: Vec<&GhAsset> = zips
                .iter()
                .copied()
                .filter(|a| !h.is_empty() && a.name.to_ascii_lowercase().starts_with(&h))
                .collect();
            if named.len() == 1 {
                Pick::One(named[0])
            } else {
                Pick::Choose(zips)
            }
        }
    }
}

const IMAGE_HOSTS: [&str; 2] = [
    "https://raw.githubusercontent.com/",
    "https://user-images.githubusercontent.com/",
];
const IMAGE_EXT: [&str; 5] = [".png", ".jpg", ".jpeg", ".gif", ".webp"];

fn image_ok(url: &str) -> bool {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    IMAGE_HOSTS.iter().any(|h| url.starts_with(h))
        && IMAGE_EXT.iter().any(|e| path.ends_with(e))
        && !url.contains(['\\', ' ', '@']) // no user info tricks
        && !url.contains("/../")
}

/// Up to `max` picture addresses a README refers to (`![](url)` and `<img src="url">`), only on GitHub's own image hosts; a relative path is
/// resolved against the repository's default branch. Nothing else is ever fetched.
pub fn readme_images(markdown: &str, owner: &str, repo: &str, max: usize) -> Vec<String> {
    // every picture target in document order
    let mut targets: Vec<String> = Vec::new();
    let mut i = 0;
    while i < markdown.len() {
        let rest = &markdown[i..];
        if rest.starts_with("![") {
            if let Some(close) = rest.find("](") {
                let start = close + 2;
                if let Some(end) = rest[start..].find(')') {
                    targets.push(
                        rest[start..start + end]
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_string(),
                    );
                    i += start + end;
                    continue;
                }
            }
        } else if rest.starts_with("<img")
            && let Some(end) = rest.find('>')
        {
            let tag = &rest[..end];
            if let Some(s) = tag.find("src=\"").map(|p| p + 5)
                && let Some(e) = tag[s..].find('"')
            {
                targets.push(tag[s..s + e].to_string());
            }
            i += end;
            continue;
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }
    let mut found: Vec<String> = Vec::new();
    for raw in targets {
        let raw = raw.trim().trim_matches(['<', '>']).to_string();
        let url = if raw.starts_with("https://") {
            raw
        } else if raw.contains("://")
            || raw.starts_with("//")
            || raw.contains("..")
            || raw.is_empty()
        {
            continue;
        } else {
            format!(
                "https://raw.githubusercontent.com/{owner}/{repo}/HEAD/{}",
                raw.trim_start_matches("./").trim_start_matches('/')
            )
        };
        if image_ok(&url) && !found.contains(&url) {
            found.push(url);
            if found.len() == max {
                break;
            }
        }
    }
    found
}

/// A release asset address as the download code wants it.
pub fn address_of(asset: &GhAsset) -> Option<DownloadAddress> {
    parse_download_url(&asset.url).filter(|a| a.is_release)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELEASES: &str = r#"[
      {"tag_name":"v2.0-rc1","name":"RC","draft":true,"assets":[]},
      {"tag_name":"v1.1","name":"One point one","published_at":"2024-05-02T10:00:00Z","prerelease":false,"assets":[
        {"name":"a.B.1.1.zip","size":3145728,"browser_download_url":"https://github.com/o/r/releases/download/v1.1/a.B.1.1.zip","digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"},
        {"name":"source.tar.gz","size":10,"browser_download_url":"https://github.com/o/r/releases/download/v1.1/source.tar.gz"}]},
      {"tag_name":"v1.0","published_at":"2024-01-01T00:00:00Z","prerelease":true,"assets":[
        {"name":"a.B.zip","size":10,"browser_download_url":"https://github.com/o/r/releases/download/v1.0/a.B.zip"},
        {"name":"other.zip","size":10,"browser_download_url":"https://github.com/o/r/releases/download/v1.0/other.zip"},
        {"name":"empty.zip","size":0,"browser_download_url":"https://github.com/o/r/releases/download/v1.0/empty.zip"},
        {"name":"odd.zip","size":10,"browser_download_url":"https://evil.example/odd.zip"}]}
    ]"#;

    #[test]
    fn releases_are_read_tolerantly_and_drafts_are_dropped() {
        let r = parse_releases(RELEASES.as_bytes()).unwrap();
        assert_eq!(
            r.iter().map(|x| x.tag.as_str()).collect::<Vec<_>>(),
            ["v1.1", "v1.0"]
        );
        assert_eq!(
            (r[0].name.as_str(), r[0].published.as_str(), r[0].prerelease),
            ("One point one", "2024-05-02", false)
        );
        assert_eq!(
            r[0].assets[0].sha256.as_deref(),
            Some("a".repeat(64).as_str())
        );
        assert_eq!(r[1].name, "v1.0");
        assert!(parse_releases(b"not json").is_err());
        assert!(parse_releases(b"{}").unwrap().is_empty());
    }

    #[test]
    fn the_zip_is_chosen_by_what_is_there() {
        let r = parse_releases(RELEASES.as_bytes()).unwrap();
        // one zip: that one (the tarball is not a zip)
        assert!(matches!(pick_asset(&r[0], "x"), Pick::One(a) if a.name == "a.B.1.1.zip"));
        // several valid zips: the hint picks one, else the user chooses; zero-size and off-host files never count
        assert!(matches!(pick_asset(&r[1], "a.B"), Pick::One(a) if a.name == "a.B.zip"));
        assert!(matches!(pick_asset(&r[1], ""), Pick::Choose(v) if v.len() == 2));
        let none = GhRelease {
            tag: "t".into(),
            name: "t".into(),
            published: String::new(),
            prerelease: false,
            assets: vec![],
        };
        assert_eq!(pick_asset(&none, "x"), Pick::None);
    }

    #[test]
    fn readme_pictures_only_from_githubs_own_hosts() {
        let md = "# Core\n![shot](https://raw.githubusercontent.com/o/r/main/a.png)\n![](docs/b.jpg \"title\")\n<img src=\"https://user-images.githubusercontent.com/1/c.gif\" width=300>\n![x](https://evil.example/d.png)\n![y](http://raw.githubusercontent.com/o/r/main/e.png)\n![z](../../etc/f.png)\n![s](https://raw.githubusercontent.com/o/r/main/g.svg)\n![dup](https://raw.githubusercontent.com/o/r/main/a.png)\n![four](docs/h.png)";
        let v = readme_images(md, "o", "r", 3);
        assert_eq!(
            v,
            [
                "https://raw.githubusercontent.com/o/r/main/a.png",
                "https://raw.githubusercontent.com/o/r/HEAD/docs/b.jpg",
                "https://user-images.githubusercontent.com/1/c.gif",
            ]
        );
        assert!(readme_images("no pictures here", "o", "r", 3).is_empty());
    }
}
