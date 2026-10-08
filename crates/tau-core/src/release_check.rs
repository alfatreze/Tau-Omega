//! Deciding, from GitHub's release list, whether a newer Tau release exists, and verifying what is downloaded.
//! Pure logic: no network and no files. The host crate (`src-tauri`) fetches the bytes; this module parses and judges
//! them, so everything here is tested on real captured responses.
//!
//! Trust rules: only asset URLs under [`DOWNLOAD_PREFIX`] are ever returned, a download is refused unless
//! `SHA256SUMS.txt` lists it and the hash matches, and when a release manifest names the zip its `zip_sha256` must
//! match too ([`verify_download`]).

use crate::{ErrorCode, TauError, compat};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{cmp::Ordering, collections::BTreeMap};

/// The one place releases are listed from.
pub const RELEASES_URL: &str = "https://api.github.com/repos/alfatreze/Tau-Alpha/releases";
/// Every asset Omega downloads must live here.
pub const DOWNLOAD_PREFIX: &str = "https://github.com/alfatreze/Tau-Alpha/releases/download/";

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GithubRelease {
    pub tag: String,
    pub title: String,
    pub prerelease: bool,
    /// `YYYY-MM-DD` of the publication.
    pub published: String,
    pub assets: Vec<ReleaseAsset>,
}

/// Parses GitHub's `/releases` response. Drafts, entries whose tag is not `vX.Y.Z[-pre]`, and assets outside
/// [`DOWNLOAD_PREFIX`] are dropped; unknown keys are ignored.
pub fn parse_releases(bytes: &[u8]) -> Result<Vec<GithubRelease>, TauError> {
    let json: Value = serde_json::from_slice(bytes).map_err(|e| {
        TauError::e(
            ErrorCode::Json,
            format!("the release list is not valid JSON: {e}"),
        )
    })?;
    let list = json.as_array().ok_or_else(|| {
        TauError::e(
            ErrorCode::Json,
            "the release list is not a list (GitHub may have rate-limited the request)",
        )
    })?;
    let mut out = Vec::new();
    for item in list {
        let text = |key: &str| item.get(key).and_then(Value::as_str).unwrap_or_default();
        if item.get("draft").and_then(Value::as_bool).unwrap_or(false)
            || !compat::is_release_tag(text("tag_name"))
        {
            continue;
        }
        let assets = item
            .get("assets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|a| {
                let url = a.get("browser_download_url")?.as_str()?;
                let name = a.get("name")?.as_str()?;
                (url.starts_with(DOWNLOAD_PREFIX) && !name.contains('/') && !name.contains(".."))
                    .then(|| ReleaseAsset {
                        name: name.to_string(),
                        url: url.to_string(),
                        size: a.get("size").and_then(Value::as_u64).unwrap_or(0),
                    })
            })
            .collect();
        out.push(GithubRelease {
            tag: text("tag_name").to_string(),
            title: text("name").to_string(),
            prerelease: item
                .get("prerelease")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            published: text("published_at").chars().take(10).collect(),
            assets,
        });
    }
    Ok(out)
}

/// What the app knows about the installed Tau, for deciding "is there something newer".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed {
    /// The release tag, when a manifest identified the installed build by hash.
    pub release: Option<String>,
    /// `core.json`'s `date_release`, as a fallback.
    pub date_release: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UpdateCheck {
    pub latest: GithubRelease,
    /// `Some(true)`: alert the user. `None`: nothing is known about the installed build (no card chosen).
    pub newer: Option<bool>,
    pub installed_release: Option<String>,
    /// The core zips of the release (normal and Diagnostic), the recommended normal one first.
    pub zips: Vec<ReleaseAsset>,
    pub manifest: Option<ReleaseAsset>,
    pub sums: Option<ReleaseAsset>,
    /// One plain sentence for the alert.
    pub message: String,
}

/// The newest release by SemVer precedence (pre-releases count: every Tau release so far is one).
pub fn latest(releases: &[GithubRelease]) -> Option<&GithubRelease> {
    releases
        .iter()
        .max_by(|a, b| compat::compare_tags(&a.tag, &b.tag))
}

/// Judges the release list against the installed build.
pub fn evaluate(releases: &[GithubRelease], installed: &Installed) -> Option<UpdateCheck> {
    evaluate_with_sums(releases, installed, None)
}

/// [`evaluate`], keeping only the zips that `SHA256SUMS.txt` (when given) lists: an unlisted zip could not be verified
/// after download, so it is not offered.
pub fn evaluate_with_sums(
    releases: &[GithubRelease],
    installed: &Installed,
    sums: Option<&BTreeMap<String, String>>,
) -> Option<UpdateCheck> {
    let latest = latest(releases)?.clone();
    let newer = match (&installed.release, &installed.date_release) {
        (Some(tag), _) => Some(compat::compare_tags(&latest.tag, tag) == Ordering::Greater),
        // Published dates are never earlier than the build date, so a strictly later publication is newer.
        (None, Some(date)) => Some(latest.published.as_str() > date.as_str()),
        (None, None) => None,
    };
    let version = latest.tag.trim_start_matches('v').to_string();
    let mut zips: Vec<ReleaseAsset> = latest
        .assets
        .iter()
        .filter(|a| a.name.starts_with("alfatreze.TAU") && a.name.ends_with(".zip"))
        .filter(|a| sums.is_none_or(|s| s.contains_key(&a.name)))
        .cloned()
        .collect();
    // The zip named for the release itself ("...0.6.0-alpha.3...") and not the Diagnostic Build goes first.
    zips.sort_by_key(|a| {
        (
            a.name.contains("TAU_DIAGNOSTIC"),
            !a.name.contains(&version),
            a.name.clone(),
        )
    });
    let find = |name: &str| latest.assets.iter().find(|a| a.name == name).cloned();
    let message = match newer {
        Some(true) => format!(
            "Tau {} is available{}.",
            latest.tag,
            installed
                .release
                .as_ref()
                .map_or(String::new(), |t| format!(" (you have {t})"))
        ),
        Some(false) => format!("Tau is up to date ({}).", latest.tag),
        None => format!("The newest Tau release is {}.", latest.tag),
    };
    Some(UpdateCheck {
        manifest: find("tau-compat.json"),
        sums: find("SHA256SUMS.txt"),
        installed_release: installed.release.clone(),
        newer,
        zips,
        message,
        latest,
    })
}

/// `SHA256SUMS.txt`: `<64 hex>  <name>` per line (`*name` marks binary mode).
pub fn parse_sums(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let (hash, name) = line.trim().split_once(char::is_whitespace)?;
            let name = name.trim().trim_start_matches('*');
            (hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) && !name.is_empty())
                .then(|| (name.to_string(), hash.to_ascii_lowercase()))
        })
        .collect()
}

/// A download is trusted only if `SHA256SUMS.txt` lists it with this hash, and, when a manifest names the zip, the
/// manifest agrees. Refuses otherwise.
pub fn verify_download(
    name: &str,
    bytes: &[u8],
    sums: &BTreeMap<String, String>,
    manifest: Option<&compat::CompatDoc>,
) -> Result<(), TauError> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    let fail = |msg: String| Err(TauError::e(ErrorCode::VerificationFailed, msg));
    match sums.get(name) {
        None => {
            return fail(format!(
                "{name} is not listed in SHA256SUMS.txt, so it cannot be verified."
            ));
        }
        Some(expected) if *expected != actual => {
            return fail(format!(
                "{name} does not match SHA256SUMS.txt; the download is damaged or was changed."
            ));
        }
        Some(_) => {}
    }
    if let Some(package) = manifest.and_then(|d| d.packages.iter().find(|p| p.zip == name))
        && package.zip_sha256 != actual
    {
        return fail(format!("{name} does not match the release manifest."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real() -> Vec<GithubRelease> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/github/releases.json");
        parse_releases(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn parses_the_real_release_list() {
        let releases = real();
        assert_eq!(releases.len(), 5);
        let newest = latest(&releases).unwrap();
        assert_eq!(newest.tag, "v0.6.0-alpha.3");
        assert!(newest.prerelease);
        assert_eq!(newest.published, "2026-10-04");
        assert!(
            newest
                .assets
                .iter()
                .all(|a| a.url.starts_with(DOWNLOAD_PREFIX))
        );
    }

    #[test]
    fn newer_means_a_later_release_than_the_installed_one() {
        let releases = real();
        let check = |release: Option<&str>, date: Option<&str>| {
            evaluate(
                &releases,
                &Installed {
                    release: release.map(str::to_string),
                    date_release: date.map(str::to_string),
                },
            )
            .unwrap()
        };
        assert_eq!(check(Some("v0.6.0-alpha.2"), None).newer, Some(true));
        assert_eq!(check(Some("v0.6.0-alpha.3"), None).newer, Some(false));
        assert_eq!(
            check(Some("v0.6.0-alpha.10"), None).newer,
            Some(false),
            "numeric, not text, order"
        );
        assert_eq!(check(None, Some("2026-09-30")).newer, Some(true));
        assert_eq!(
            check(None, Some("2026-10-07")).newer,
            Some(false),
            "a local build newer than the last release"
        );
        assert_eq!(check(None, None).newer, None);
        let alert = check(Some("v0.6.0-alpha.2"), None);
        assert_eq!(
            alert.message,
            "Tau v0.6.0-alpha.3 is available (you have v0.6.0-alpha.2)."
        );
    }

    #[test]
    fn the_core_zip_named_for_the_release_comes_first_and_diagnostic_last() {
        let check = evaluate(&real(), &Installed::default()).unwrap();
        let names: Vec<_> = check.zips.iter().map(|z| z.name.as_str()).collect();
        assert_eq!(names[0], "alfatreze.TAU_0.6.0-alpha.3_2026-10-04.zip");
        assert!(names.last().unwrap().contains("TAU_DIAGNOSTIC"));
        assert_eq!(check.sums.as_ref().unwrap().name, "SHA256SUMS.txt");
        assert!(
            check.manifest.is_none(),
            "these releases predate tau-compat.json"
        );
    }

    #[test]
    fn only_zips_the_checksum_file_lists_are_offered() {
        let sums = parse_sums(&format!(
            "{h}  alfatreze.TAU_0.6.0_2026-10-04.zip\n{h}  alfatreze.TAU_DIAGNOSTIC_0.6.0_2026-10-04.zip\n",
            h = "a".repeat(64)
        ));
        let check = evaluate_with_sums(&real(), &Installed::default(), Some(&sums)).unwrap();
        let names: Vec<_> = check.zips.iter().map(|z| z.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "alfatreze.TAU_0.6.0_2026-10-04.zip",
                "alfatreze.TAU_DIAGNOSTIC_0.6.0_2026-10-04.zip"
            ],
            "the labelled zip is not in the real SHA256SUMS.txt, so it cannot be verified and is not offered"
        );
    }

    #[test]
    fn untrusted_entries_are_dropped() {
        let json = br#"[
          {"tag_name":"v9.9.9","draft":true,"assets":[]},
          {"tag_name":"nightly","assets":[]},
          {"tag_name":"v0.7.0","prerelease":false,"published_at":"2026-11-01T10:00:00Z","assets":[
            {"name":"ok.zip","size":3,"browser_download_url":"https://github.com/alfatreze/Tau-Alpha/releases/download/v0.7.0/ok.zip"},
            {"name":"evil.zip","size":3,"browser_download_url":"https://example.com/evil.zip"},
            {"name":"../x.zip","size":3,"browser_download_url":"https://github.com/alfatreze/Tau-Alpha/releases/download/v0.7.0/x.zip"}]}]"#;
        let releases = parse_releases(json).unwrap();
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].assets.len(), 1);
        assert!(parse_releases(b"{\"message\":\"API rate limit exceeded\"}").is_err());
        assert!(parse_releases(b"[]").unwrap().is_empty());
        assert!(evaluate(&[], &Installed::default()).is_none());
    }

    #[test]
    fn downloads_are_trusted_only_with_a_matching_listed_hash() {
        let real_sums = "8e0b839ccd2cbc3703c8bae2e4e66739a2edb81f1cf6aa2ae522c0be246673f9  alfatreze.TAU_0.6.0_2026-10-07.zip\n\
                         e7f5efbdfe3cec09e9e2454ed2afd23e1aa8e8e84abfde866464c1e69660b130 *alfatreze.TAU_DIAGNOSTIC_0.6.0_2026-10-07.zip\nnot a line\n";
        let sums = parse_sums(real_sums);
        assert_eq!(sums.len(), 2);
        let zip = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../testdata/packages/alfatreze.TAU_0.6.0_2026-10-07.zip"),
        )
        .unwrap();
        verify_download("alfatreze.TAU_0.6.0_2026-10-07.zip", &zip, &sums, None).unwrap();
        // One flipped byte, an unlisted name and a wrong listed hash are all refused.
        let mut damaged = zip.clone();
        damaged[100] ^= 1;
        assert!(
            verify_download("alfatreze.TAU_0.6.0_2026-10-07.zip", &damaged, &sums, None).is_err()
        );
        assert!(verify_download("other.zip", &zip, &sums, None).is_err());
        let wrong = parse_sums(&format!("{}  a.zip", "0".repeat(64)));
        assert!(verify_download("a.zip", &zip, &wrong, None).is_err());
    }

    #[test]
    fn a_manifest_that_disagrees_with_the_listed_hash_is_refused() {
        let zip = b"zip bytes".to_vec();
        let hash = format!("{:x}", Sha256::digest(&zip));
        let sums = parse_sums(&format!("{hash}  a.zip"));
        let doc = |zip_sha: &str| {
            compat::parse_compat(
                serde_json::json!({"schema": 1, "release": "v0.7.0", "date_release": "2026-11-01",
                    "packages": [{"zip": "a.zip", "zip_sha256": zip_sha, "core_id": "alfatreze.TAU",
                        "bitstream_sha256": "00", "bitstream_core_version": "4D50331A",
                        "rom_sha256": "00", "cold_sha256": "00", "rom_accepts": [], "rom_needs": []}],
                    "requires_omega": {"min_omega": "0.4.0"}})
                .to_string()
                .as_bytes(),
            )
            .unwrap()
        };
        verify_download("a.zip", &zip, &sums, Some(&doc(&hash))).unwrap();
        assert!(verify_download("a.zip", &zip, &sums, Some(&doc(&"1".repeat(64)))).is_err());
    }
}
