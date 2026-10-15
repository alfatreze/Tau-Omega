//! Choosing the download for a catalog core. The v3 catalog already carries each release's direct `download_url`, so
//! nothing needs the GitHub API to find a zip; the API is used only, and only best effort, to learn the SHA-256 and
//! size GitHub publishes for that one asset just before it is downloaded ([`asset_digest`]).
//!
//! Every address is checked here (see [`parse_download_url`]): only the two shapes the library really uses are accepted.

use crate::{ErrorCode, TauError, catalog::CatalogCore};
use serde_json::Value;

/// Where a catalog download may come from. The library lists two kinds (checked over all 6,441 releases of the 333
/// cores): a GitHub release asset, `https://github.com/<owner>/<repo>/releases/download/<tag>/<file>.zip`, and a zip in a
/// repository pinned to one commit, `https://raw.githubusercontent.com/<owner>/<repo>/<40-hex commit>/<path>.zip`
/// (immutable, used by the big arcade collections). The owner and repository need not equal the catalog's `repository`
/// field: authors rename and move repositories, and the zip is vetted (identity, paths, size) whatever its address.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DownloadAddress {
    pub owner: String,
    pub repo: String,
    /// The release tag, or the commit for a pinned file.
    pub reference: String,
    /// The zip's file name (the last path segment).
    pub file: String,
    /// A GitHub release asset (its digest can be looked up); false for a commit-pinned file.
    pub is_release: bool,
}

fn name_ok(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && !part.contains(['\\', '\0', ' ', '?', '#', '%'])
        && part.len() <= 200
}

/// Parses a catalog download address; `None` for anything that is not one of the two accepted shapes.
pub fn parse_download_url(url: &str) -> Option<DownloadAddress> {
    if let Some(rest) = url.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        // owner / repo / releases / download / <tag, which may contain slashes> / <file>
        if parts.len() < 6
            || parts[2] != "releases"
            || parts[3] != "download"
            || !parts.iter().all(|p| name_ok(p))
        {
            return None;
        }
        let file = parts[parts.len() - 1];
        return file
            .to_ascii_lowercase()
            .ends_with(".zip")
            .then(|| DownloadAddress {
                owner: parts[0].into(),
                repo: parts[1].into(),
                reference: parts[4..parts.len() - 1].join("/"),
                file: file.into(),
                is_release: true,
            });
    }
    if let Some(rest) = url.strip_prefix("https://raw.githubusercontent.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        // owner / repo / <commit> / path... / <file>
        if parts.len() < 4
            || !parts.iter().all(|p| name_ok(p))
            || parts[2].len() != 40
            || !parts[2].chars().all(|c| c.is_ascii_hexdigit())
        {
            return None;
        }
        let file = parts[parts.len() - 1];
        return file
            .to_ascii_lowercase()
            .ends_with(".zip")
            .then(|| DownloadAddress {
                owner: parts[0].into(),
                repo: parts[1].into(),
                reference: parts[2].into(),
                file: file.into(),
                is_release: false,
            });
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreDownload {
    pub name: String,
    pub url: String,
    /// Bytes GitHub lists for the asset; 0 while unknown (the catalog does not carry it).
    pub size: u64,
    /// Lowercase hex SHA-256 from GitHub's per-asset digest, when it has been looked up and exists; `None` means the
    /// plan shows the "no checksum published" notice.
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreRelease {
    /// The GitHub release tag (or the commit, for a file pinned to one).
    pub tag: String,
    /// A GitHub release asset: its checksum can be looked up. False for a commit-pinned file (the notice always shows).
    pub is_release: bool,
    pub version: String,
    /// `YYYY-MM-DD`.
    pub published: String,
    pub download: CoreDownload,
}

/// The release to install: the newest by default, or `version` when given (older ones stay in the catalog's history).
pub fn release_from_catalog(
    core: &CatalogCore,
    version: Option<&str>,
) -> Result<CoreRelease, TauError> {
    let entry = match version {
        Some(v) => core.history.iter().find(|r| r.version == v),
        None => core.history.first(),
    }
    .ok_or_else(|| {
        TauError::e(
            ErrorCode::NotFound,
            if core.history.is_empty() {
                "The author has not published a download yet."
            } else {
                "That version is not in the library."
            },
        )
    })?;
    let address = parse_download_url(&entry.download_url).ok_or_else(|| {
        TauError::e(ErrorCode::InvalidPathReference, "The library lists a download address Omega does not recognise as a GitHub zip, so it will not use it.")
    })?;
    Ok(CoreRelease {
        tag: address.reference,
        is_release: address.is_release,
        version: entry.version.clone(),
        published: entry.release_date.clone(),
        download: CoreDownload {
            name: address.file,
            url: entry.download_url.clone(),
            size: 0,
            sha256: None,
        },
    })
}

/// The API address of the release that holds this download (for the digest lookup); `None` for a commit-pinned file or a tag
/// that would need escaping, in which case the lookup is skipped and the notice shows.
pub fn digest_lookup_url(download_url: &str) -> Option<String> {
    let a = parse_download_url(download_url).filter(|a| a.is_release)?;
    a.reference
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .then(|| {
            format!(
                "https://api.github.com/repos/{}/{}/releases/tags/{}",
                a.owner, a.repo, a.reference
            )
        })
}

/// From GitHub's release-by-tag JSON: the size and published SHA-256 of the asset named `asset_name`, if it is there.
pub fn asset_digest(bytes: &[u8], asset_name: &str) -> Option<(u64, Option<String>)> {
    let json: Value = serde_json::from_slice(bytes).ok()?;
    let asset = json
        .get("assets")?
        .as_array()?
        .iter()
        .find(|a| a.get("name").and_then(Value::as_str) == Some(asset_name))?;
    let digest = asset
        .get("digest")
        .and_then(Value::as_str)
        .and_then(|d| d.strip_prefix("sha256:"))
        .filter(|d| d.len() == 64 && d.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase);
    Some((
        asset.get("size").and_then(Value::as_u64).unwrap_or(0),
        digest,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    fn real() -> catalog::Catalog {
        catalog::parse(
            include_bytes!("../../../testdata/openfpga/cores_v3.json"),
            include_bytes!("../../../testdata/openfpga/platforms_v3.json"),
        )
        .unwrap()
    }

    #[test]
    fn the_newest_release_comes_straight_from_the_catalog() {
        let c = real();
        let core = c.find("agg23.Arduboy").unwrap();
        let r = release_from_catalog(core, None).unwrap();
        assert_eq!(
            (r.tag.as_str(), r.version.as_str(), r.published.as_str()),
            ("0.9.0", "0.9.0", "2022-09-03")
        );
        assert_eq!(r.download.name, "agg23.Arduboy.0.9.0.zip");
        assert_eq!((r.download.size, r.download.sha256), (0, None));
        // an older version can be chosen by its version string
        let b = c.find("HarpMudd.Berzerk").unwrap();
        assert_eq!(release_from_catalog(b, None).unwrap().version, "1.1.1");
        assert_eq!(
            release_from_catalog(b, Some("1.1.0")).unwrap().tag,
            "v1.1.0"
        );
        assert!(release_from_catalog(b, Some("0.0.1")).is_err());
    }

    #[test]
    fn every_release_in_the_real_sample_resolves() {
        let c = real();
        for core in c.cores.iter().filter(|c| !c.history.is_empty()) {
            for r in &core.history {
                let rel = release_from_catalog(core, Some(&r.version))
                    .unwrap_or_else(|e| panic!("{} {}: {e}", core.id, r.version));
                assert!(rel.download.name.ends_with(".zip"), "{}", core.id);
            }
        }
    }

    #[test]
    fn a_core_with_no_release_says_so() {
        let c = real();
        let e = release_from_catalog(c.find("Obsidian.PocketTyrian").unwrap(), None).unwrap_err();
        assert!(
            e.message.contains("not published a download"),
            "{}",
            e.message
        );
    }

    #[test]
    fn both_real_address_shapes_parse_and_everything_else_is_refused() {
        let rel = parse_download_url("https://github.com/agg23/openfpga-arduboy/releases/download/0.9.0/agg23.Arduboy.0.9.0.zip").unwrap();
        assert_eq!(
            (
                rel.owner.as_str(),
                rel.repo.as_str(),
                rel.reference.as_str(),
                rel.file.as_str(),
                rel.is_release
            ),
            (
                "agg23",
                "openfpga-arduboy",
                "0.9.0",
                "agg23.Arduboy.0.9.0.zip",
                true
            )
        );
        // a real tag that contains slashes
        let slashy = parse_download_url("https://github.com/Mazamars312/openfpga-pcengine-cd/releases/download/Mazamars312.PCECD.0.1.6-07/02/2023/x.zip").unwrap();
        assert_eq!(
            (slashy.reference.as_str(), slashy.file.as_str()),
            ("Mazamars312.PCECD.0.1.6-07/02/2023", "x.zip")
        );
        let sha = "49b4091fb8d71116eb5920a88aae72b910322fa4";
        let raw = parse_download_url(&format!("https://raw.githubusercontent.com/Coin-OpCollection/Distribution-OpenFPGA/{sha}/Releases/coc.bublbobl.zip")).unwrap();
        assert_eq!(
            (raw.reference.as_str(), raw.file.as_str(), raw.is_release),
            (sha, "coc.bublbobl.zip", false)
        );
        for bad in [
            "http://github.com/a/b/releases/download/1/x.zip",
            "https://evil.example/a/b/releases/download/1/x.zip",
            "https://github.com/a/b/releases/download/1/../../x.zip",
            "https://github.com/a/b/releases/download/1/x.zip?token=1",
            "https://github.com/a/b/releases/download/1/x.tar.gz",
            "https://github.com/a/b/releases/download/1/",
            "https://github.com/a/b/archive/main.zip",
            "https://github.com/a/b/releases/latest/download/x.zip",
            "https://raw.githubusercontent.com/a/b/main/x.zip",
            "https://raw.githubusercontent.com/a/b/zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz/x.zip",
            "https://raw.githubusercontent.com/a/b/49b4091fb8d71116eb5920a88aae72b910322fa4/../x.zip",
            "https://github.com/a/b/releases/download/1/a b.zip",
            "https://github.com/a/b/releases/download/1/a%2fb.zip",
            "",
        ] {
            assert!(parse_download_url(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn a_renamed_repository_still_resolves_from_its_listed_address() {
        // the catalog's repository field can be stale (authors rename and move repositories)
        let c = real();
        let mut core = c.find("agg23.Arduboy").unwrap().clone();
        core.history[0].download_url =
            "https://github.com/someone-else/new-name/releases/download/1.0/agg23.Arduboy.zip"
                .into();
        let r = release_from_catalog(&core, None).unwrap();
        assert_eq!(
            (r.tag.as_str(), r.download.name.as_str()),
            ("1.0", "agg23.Arduboy.zip")
        );
        core.history[0].download_url = "https://evil.example/x.zip".into();
        assert!(release_from_catalog(&core, None).is_err());
    }

    #[test]
    fn the_digest_lookup_reads_size_and_checksum_and_never_trusts_a_different_asset() {
        let c = real();
        let core = c.find("agg23.Arduboy").unwrap();
        assert_eq!(
            digest_lookup_url(&core.download_url).unwrap(),
            "https://api.github.com/repos/agg23/openfpga-arduboy/releases/tags/0.9.0"
        );
        assert_eq!(
            digest_lookup_url("https://github.com/a/b/releases/download/x/y/z.zip"),
            None
        );
        assert_eq!(
            digest_lookup_url(
                "https://raw.githubusercontent.com/a/b/49b4091fb8d71116eb5920a88aae72b910322fa4/z.zip"
            ),
            None
        );
        let real_release = include_bytes!("../../../testdata/openfpga/arduboy_release.json");
        assert_eq!(
            asset_digest(real_release, "agg23.Arduboy.0.9.0.zip"),
            Some((257_119, None))
        );
        assert_eq!(asset_digest(real_release, "other.zip"), None);
        let h = "ab".repeat(32);
        let with = format!(
            r#"{{"assets":[{{"name":"x.zip","size":9,"digest":"sha256:{h}"}},{{"name":"y.zip","size":1,"digest":"sha256:zz"}}]}}"#
        );
        assert_eq!(asset_digest(with.as_bytes(), "x.zip"), Some((9, Some(h))));
        assert_eq!(asset_digest(with.as_bytes(), "y.zip"), Some((1, None)));
        assert_eq!(asset_digest(b"not json", "x.zip"), None);
    }
}
