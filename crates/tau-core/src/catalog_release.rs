//! Choosing the download for a catalog core from the GitHub release of its repository (`/releases/latest`).
//! Pure logic on captured JSON; the host does the request.
//!
//! The catalog names a repository, not a file, so every URL is checked here: it must sit under that exact
//! repository's `releases/download/` path on github.com, and a filename may not carry a path.

use crate::{ErrorCode, TauError, catalog::CatalogCore};
use serde_json::Value;

/// The one request per opened core: the newest published release of its repository.
pub fn latest_release_url(core: &CatalogCore) -> String {
    format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        core.repository.owner, core.repository.name
    )
}

/// Whether `url` is a download of this core's own repository (the only place its package may come from).
pub fn url_allowed(core: &CatalogCore, url: &str) -> bool {
    url.starts_with(&download_prefix(core)) && !url[download_prefix(core).len()..].contains("..")
}

fn download_prefix(core: &CatalogCore) -> String {
    format!(
        "https://github.com/{}/{}/releases/download/",
        core.repository.owner, core.repository.name
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreDownload {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// Lowercase hex SHA-256 from GitHub's per-asset digest; `None` for releases that predate digests (the plan
    /// then shows the "no checksum published" notice).
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreRelease {
    pub tag: String,
    pub title: String,
    pub prerelease: bool,
    /// `YYYY-MM-DD`.
    pub published: String,
    /// The release notes, plain text, cut to [`NOTES_LIMIT`] characters.
    pub notes: String,
    pub download: CoreDownload,
}

pub const NOTES_LIMIT: usize = 600;

fn clean_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', '\0']) && !name.contains("..")
}

fn digest_hex(asset: &Value) -> Option<String> {
    let d = asset.get("digest")?.as_str()?.strip_prefix("sha256:")?;
    (d.len() == 64 && d.chars().all(|c| c.is_ascii_hexdigit())).then(|| d.to_ascii_lowercase())
}

/// Parses one release object. Errors name what is wrong in plain words (they are shown to the user).
pub fn parse_release(bytes: &[u8], core: &CatalogCore) -> Result<CoreRelease, TauError> {
    let json: Value = serde_json::from_slice(bytes).map_err(|e| {
        TauError::e(
            ErrorCode::Json,
            format!("the release is not valid JSON: {e}"),
        )
    })?;
    if json.get("tag_name").and_then(Value::as_str).is_none() {
        return Err(TauError::e(
            ErrorCode::Json,
            "GitHub did not return a release (the author may not have published one, or the lookup limit was reached)",
        ));
    }
    let text = |k: &str| {
        json.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let prefix = download_prefix(core);
    let id = core.id.to_ascii_lowercase();
    let zips: Vec<&Value> = json
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|a| {
            let name = a.get("name").and_then(Value::as_str).unwrap_or_default();
            let url = a
                .get("browser_download_url")
                .and_then(Value::as_str)
                .unwrap_or_default();
            name.to_ascii_lowercase().ends_with(".zip")
                && clean_name(name)
                && url.starts_with(&prefix)
                && url.ends_with(name)
                && a.get("state")
                    .and_then(Value::as_str)
                    .is_none_or(|s| s == "uploaded")
        })
        .collect();
    let chosen = zips
        .iter()
        .find(|a| a.get("name").and_then(Value::as_str).is_some_and(|n| n.to_ascii_lowercase().starts_with(&id)))
        .or(if zips.len() == 1 { zips.first() } else { None })
        .ok_or_else(|| {
            TauError::e(
                ErrorCode::Json,
                if zips.is_empty() {
                    "this release has no downloadable zip"
                } else {
                    "this release has several zips and none is named for the core, so Omega will not guess"
                },
            )
        })?;
    let published = text("published_at");
    Ok(CoreRelease {
        tag: text("tag_name"),
        title: text("name"),
        prerelease: json
            .get("prerelease")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        published: published.get(..10).unwrap_or_default().to_string(),
        notes: text("body").chars().take(NOTES_LIMIT).collect(),
        download: CoreDownload {
            name: chosen["name"].as_str().unwrap_or_default().to_string(),
            url: chosen["browser_download_url"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            size: chosen.get("size").and_then(Value::as_u64).unwrap_or(0),
            sha256: digest_hex(chosen),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    fn arduboy() -> CatalogCore {
        let c = catalog::parse(include_bytes!("../../../testdata/openfpga/cores.json")).unwrap();
        c.find("agg23.Arduboy").unwrap().clone()
    }

    #[test]
    fn real_release_resolves_and_has_no_checksum() {
        let core = arduboy();
        let r = parse_release(
            include_bytes!("../../../testdata/openfpga/arduboy_release.json"),
            &core,
        )
        .unwrap();
        assert_eq!(r.tag, "0.9.0");
        assert_eq!(r.published, "2022-09-03");
        assert_eq!(r.download.name, "agg23.Arduboy.0.9.0.zip");
        assert_eq!(r.download.size, 257_119);
        assert_eq!(r.download.sha256, None);
        assert_eq!(
            latest_release_url(&core),
            "https://api.github.com/repos/agg23/openfpga-arduboy/releases/latest"
        );
    }

    fn release_with(assets: &str) -> Vec<u8> {
        format!(r#"{{"tag_name":"1.0","name":"n","published_at":"2026-01-02T03:04:05Z","body":"b","assets":[{assets}]}}"#).into_bytes()
    }

    #[test]
    fn digest_is_read_and_validated() {
        let core = arduboy();
        let h = "ab".repeat(32);
        let a = format!(
            r#"{{"name":"agg23.Arduboy.1.0.zip","size":5,"digest":"sha256:{h}","browser_download_url":"https://github.com/agg23/openfpga-arduboy/releases/download/1.0/agg23.Arduboy.1.0.zip"}}"#
        );
        assert_eq!(
            parse_release(&release_with(&a), &core)
                .unwrap()
                .download
                .sha256,
            Some(h)
        );
        let bad = a.replace(&"ab".repeat(32), "zz");
        assert_eq!(
            parse_release(&release_with(&bad), &core)
                .unwrap()
                .download
                .sha256,
            None
        );
    }

    #[test]
    fn refuses_foreign_urls_paths_and_ambiguity() {
        let core = arduboy();
        let url =
            |u: &str, n: &str| format!(r#"{{"name":"{n}","size":1,"browser_download_url":"{u}"}}"#);
        // another repository's URL, a plain http URL, a traversing name: none is offered
        for a in [
            url(
                "https://github.com/evil/repo/releases/download/1.0/agg23.Arduboy.zip",
                "agg23.Arduboy.zip",
            ),
            url(
                "http://github.com/agg23/openfpga-arduboy/releases/download/1.0/x.zip",
                "x.zip",
            ),
            url(
                "https://github.com/agg23/openfpga-arduboy/releases/download/1.0/..%2Fx.zip",
                "../x.zip",
            ),
            url(
                "https://github.com/agg23/openfpga-arduboy/releases/download/1.0/x.tar.gz",
                "x.tar.gz",
            ),
        ] {
            assert!(parse_release(&release_with(&a), &core).is_err(), "{a}");
        }
        // two zips, none named for the core: no guess; one of them named for the core: chosen
        let p = "https://github.com/agg23/openfpga-arduboy/releases/download/1.0/";
        let two = format!(
            "{},{}",
            url(&format!("{p}a.zip"), "a.zip"),
            url(&format!("{p}b.zip"), "b.zip")
        );
        assert!(parse_release(&release_with(&two), &core).is_err());
        let named = format!(
            "{},{}",
            url(&format!("{p}a.zip"), "a.zip"),
            url(&format!("{p}agg23.arduboy-2.zip"), "agg23.arduboy-2.zip")
        );
        assert_eq!(
            parse_release(&release_with(&named), &core)
                .unwrap()
                .download
                .name,
            "agg23.arduboy-2.zip"
        );
        // a lone unnamed zip is accepted
        assert!(parse_release(&release_with(&url(&format!("{p}a.zip"), "a.zip")), &core).is_ok());
    }

    #[test]
    fn rate_limit_or_missing_release_is_explained() {
        let core = arduboy();
        let e = parse_release(br#"{"message":"API rate limit exceeded"}"#, &core).unwrap_err();
        assert!(e.to_string().contains("did not return a release"));
        assert!(parse_release(b"nope", &core).is_err());
    }

    #[test]
    fn notes_are_cut() {
        let core = arduboy();
        let body = "x".repeat(2000);
        let a = r#"{"name":"agg23.Arduboy.zip","size":1,"browser_download_url":"https://github.com/agg23/openfpga-arduboy/releases/download/1.0/agg23.Arduboy.zip"}"#;
        let j = format!(r#"{{"tag_name":"1.0","body":"{body}","assets":[{a}]}}"#);
        assert_eq!(
            parse_release(j.as_bytes(), &core)
                .unwrap()
                .notes
                .chars()
                .count(),
            NOTES_LIMIT
        );
    }
}
