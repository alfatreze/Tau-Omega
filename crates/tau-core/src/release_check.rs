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
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

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
    /// The installed core's folder name (`alfatreze.TAU`, `alfatreze.TAU Preview`, ...). With it the check offers only
    /// releases that carry this core (or replace it); without it, any Tau zip.
    pub core_id: Option<String>,
    /// `core.json`'s full version (`0.7.0-preview.1`).
    pub version: Option<String>,
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
    /// The installed core's channel, when known.
    pub channel: Option<Channel>,
    /// Releases of other channels that are newer than what is installed: information, never an Update.
    pub others: Vec<String>,
}

/// Tau's three release channels. A channel is a different core (own id, own platform), not a version range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Channel {
    Stable,
    Preview,
    Dev,
}

impl Channel {
    pub fn label(self) -> &'static str {
        match self {
            Channel::Stable => "Stable",
            Channel::Preview => "Preview",
            Channel::Dev => "Dev",
        }
    }
}

/// The channel of a core id or a zip name (`alfatreze.TAU Preview`, `alfatreze.TAU_Preview_0.7.0-...zip`).
pub fn channel_of_name(name: &str) -> Channel {
    let lower = name.to_ascii_lowercase();
    if lower.contains("preview") {
        Channel::Preview
    } else if lower.contains("dev") && !lower.contains("diagnostic") || lower.contains("tau dev") {
        Channel::Dev
    } else {
        Channel::Stable
    }
}

/// The channel a `core.json` version names: `X.Y.Z` Stable, `-dev.` Dev, any other suffix Preview.
pub fn channel_of_version(version: &str) -> Channel {
    match version.split_once('-') {
        None => Channel::Stable,
        Some((_, pre)) if pre.starts_with("dev") => Channel::Dev,
        Some(_) => Channel::Preview,
    }
}

/// Whether a zip asset name is a Diagnostics core (new `TAU_Diagnostics_` and old `TAU_DIAGNOSTIC_`).
pub fn is_diagnostics_zip(name: &str) -> bool {
    name.to_ascii_lowercase().contains("diagnostic")
}

/// A core id as it appears in a zip name: spaces become underscores.
fn zip_stem(core_id: &str) -> String {
    core_id.replace(' ', "_")
}

/// The zips of `release` that carry `core_id`. A manifest answers exactly (the package's `core_id`, or `replaces`
/// naming it); without one the zip name must be `<core id>_<version>...` (so `alfatreze.TAU_` does not match
/// `alfatreze.TAU_Diagnostics_`).
pub fn zips_for_core<'a>(
    release: &'a GithubRelease,
    core_id: &str,
    docs: &[compat::CompatDoc],
) -> Vec<&'a ReleaseAsset> {
    if let Some(doc) = docs.iter().find(|d| d.release == release.tag) {
        return doc
            .packages
            .iter()
            .filter(|p| p.core_id == core_id || p.replaces.iter().any(|r| r == core_id))
            .filter_map(|p| release.assets.iter().find(|a| a.name == p.zip))
            .collect();
    }
    let stem = zip_stem(core_id);
    release
        .assets
        .iter()
        .filter(|a| a.name.ends_with(".zip"))
        .filter(|a| {
            a.name
                .strip_prefix(&stem)
                .and_then(|rest| rest.strip_prefix('_'))
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        })
        .collect()
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
    evaluate_for_core(releases, installed, sums, &[])
}

/// The full judgement. With `installed.core_id` set, only releases that carry that core (by manifest `core_id` or
/// `replaces` when `docs` knows the release, else by zip name) count, so a Stable user is never offered the Preview
/// core as an update; newer releases of other channels are reported in `others`.
pub fn evaluate_for_core(
    releases: &[GithubRelease],
    installed: &Installed,
    sums: Option<&BTreeMap<String, String>>,
    docs: &[compat::CompatDoc],
) -> Option<UpdateCheck> {
    let listed = |a: &ReleaseAsset| sums.is_none_or(|s| s.contains_key(&a.name));
    let own_zips = |r: &GithubRelease| -> Vec<ReleaseAsset> {
        match &installed.core_id {
            Some(id) => zips_for_core(r, id, docs)
                .into_iter()
                .filter(|a| listed(a))
                .cloned()
                .collect(),
            None => r
                .assets
                .iter()
                .filter(|a| a.name.starts_with("alfatreze.TAU") && a.name.ends_with(".zip"))
                .filter(|a| listed(a))
                .cloned()
                .collect(),
        }
    };
    let carrying = installed.core_id.as_ref().and_then(|id| {
        releases
            .iter()
            .filter(|r| !zips_for_core(r, id, docs).is_empty())
            .max_by(|a, b| compat::compare_tags(&a.tag, &b.tag))
    });
    // No release carries this core (for example a Stable user while only a Preview release exists): nothing is newer
    // for it, and the newest release is shown only as information.
    let nothing_for_core = installed.core_id.is_some() && carrying.is_none();
    let latest = match carrying {
        // The newest release that carries this core, listed or not (an unlisted zip then shows "no verifiable download").
        Some(r) => r.clone(),
        None => latest(releases)?.clone(),
    };
    let channel = installed
        .version
        .as_deref()
        .map(channel_of_version)
        .or_else(|| installed.core_id.as_deref().map(channel_of_name));
    let installed_tag = installed
        .release
        .clone()
        .or_else(|| installed.version.as_ref().map(|v| format!("v{v}")));
    let newer = match (&installed.release, &installed.date_release) {
        (Some(tag), _) => Some(compat::compare_tags(&latest.tag, tag) == Ordering::Greater),
        // Same-day builds: the full version in core.json orders them (`0.7.0-dev.385` before `.386`).
        (None, _)
            if installed
                .version
                .as_deref()
                .is_some_and(|v| v.contains('-')) =>
        {
            installed_tag
                .as_deref()
                .map(|t| compat::compare_tags(&latest.tag, t) == Ordering::Greater)
        }
        // Published dates are never earlier than the build date, so a strictly later publication is newer.
        (None, Some(date)) => Some(latest.published.as_str() > date.as_str()),
        (None, None) => None,
    };
    let newer = if nothing_for_core { Some(false) } else { newer };
    let version = latest.tag.trim_start_matches('v').to_string();
    let mut zips = if nothing_for_core {
        Vec::new()
    } else {
        own_zips(&latest)
    };
    // The zip named for the release itself and not the Diagnostics core goes first.
    zips.sort_by_key(|a| {
        (
            is_diagnostics_zip(&a.name),
            !a.name.contains(&version),
            a.name.clone(),
        )
    });
    let find = |name: &str| latest.assets.iter().find(|a| a.name == name).cloned();
    let what = match (&installed.core_id, channel) {
        (Some(_), Some(c)) if c != Channel::Stable => format!("Tau {}", c.label()),
        _ => "Tau".to_string(),
    };
    let message = match newer {
        _ if nothing_for_core => format!(
            "No published release carries {} yet.",
            installed.core_id.as_deref().unwrap_or("this core")
        ),
        Some(true) => format!(
            "{what} {} is available{}.",
            latest.tag,
            installed
                .release
                .as_ref()
                .map_or(String::new(), |t| format!(" (you have {t})"))
        ),
        Some(false) => format!("{what} is up to date ({}).", latest.tag),
        None => format!("The newest {what} release is {}.", latest.tag),
    };
    // Other channels: the newest release per other channel that is newer than what is installed.
    let mut others = Vec::new();
    if let (Some(own), Some(_)) = (channel, &installed.core_id) {
        for other in [Channel::Stable, Channel::Preview, Channel::Dev] {
            if other == own {
                continue;
            }
            let newest = releases
                .iter()
                .filter(|r| {
                    r.assets.iter().any(|a| {
                        a.name.starts_with("alfatreze.TAU")
                            && a.name.ends_with(".zip")
                            && !is_diagnostics_zip(&a.name)
                            && channel_of_name(&a.name) == other
                    })
                })
                .filter(|r| {
                    installed_tag
                        .as_deref()
                        .is_none_or(|t| compat::compare_tags(&r.tag, t) == Ordering::Greater)
                })
                .max_by(|a, b| compat::compare_tags(&a.tag, &b.tag));
            if let Some(r) = newest {
                others.push(format!(
                    "{} {} is also available. Installing it adds a separate core; it does not update this one.",
                    other.label(),
                    r.tag
                ));
            }
        }
    }
    Some(UpdateCheck {
        manifest: find("tau-compat.json"),
        sums: find("SHA256SUMS.txt"),
        installed_release: installed.release.clone(),
        newer,
        zips,
        message,
        channel,
        others,
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

// ---------------------------------------------------------------------------
// Manifests available to a plan: cached from GitHub, or beside a local zip
// ---------------------------------------------------------------------------

/// The manifest `bytes` if it parses **and one of its packages is exactly this zip** (by SHA-256). A manifest that does
/// not describe the zip in hand is never used to vouch for it.
pub fn manifest_matching_zip(zip_path: &Path, bytes: &[u8]) -> Option<compat::CompatDoc> {
    let doc = compat::parse_compat(bytes).ok()?;
    let hash = format!("{:x}", Sha256::digest(fs::read(zip_path).ok()?));
    doc.packages
        .iter()
        .any(|p| p.zip_sha256 == hash)
        .then_some(doc)
}

/// A dev package's own manifest: `tau-compat.json` in the zip's folder, used only if it describes that zip.
pub fn sibling_manifest(zip_path: &Path) -> Option<compat::CompatDoc> {
    let bytes = fs::read(zip_path.parent()?.join("tau-compat.json")).ok()?;
    manifest_matching_zip(zip_path, &bytes)
}

fn cache_file(dir: &Path, release: &str) -> PathBuf {
    let safe: String = release
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect();
    dir.join(format!("{safe}.json"))
}

/// Validates `bytes` as a manifest and stores it in `dir` under its release tag. An invalid file is not stored.
pub fn save_cached(dir: &Path, bytes: &[u8]) -> Result<compat::CompatDoc, TauError> {
    let doc = compat::parse_compat(bytes)?;
    fs::create_dir_all(dir)?;
    fs::write(cache_file(dir, &doc.release), bytes)?;
    Ok(doc)
}

/// Every valid manifest in `dir`; anything unreadable or unknown is skipped.
pub fn load_cached(dir: &Path) -> Vec<compat::CompatDoc> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut docs: Vec<_> = read
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| fs::read(e.path()).ok())
        .filter_map(|bytes| compat::parse_compat(&bytes).ok())
        .collect();
    docs.sort_by(|a, b| compat::compare_tags(&a.release, &b.release));
    docs
}

/// Persisted-setting names from the manifests' `persist_registry`: the newest release's name wins, older releases fill
/// ids the newest no longer lists. Empty when no known manifest carries a registry.
pub fn persist_names(docs: &[compat::CompatDoc]) -> BTreeMap<u64, String> {
    let mut sorted: Vec<&compat::CompatDoc> = docs.iter().collect();
    sorted.sort_by(|a, b| compat::compare_tags(&b.release, &a.release));
    let mut names = BTreeMap::new();
    for doc in sorted {
        for (id, meaning) in &doc.persist_registry {
            names.entry(*id).or_insert_with(|| meaning.name.clone());
        }
    }
    names
}

/// The manifests a plan for `zip_path` should know: those cached from GitHub plus the zip's own sibling manifest
/// (which wins over a cached one for the same release).
pub fn manifests_for_install(cache_dir: &Path, zip_path: &Path) -> Vec<compat::CompatDoc> {
    let mut docs = load_cached(cache_dir);
    if let Some(own) = sibling_manifest(zip_path) {
        docs.retain(|d| d.release != own.release);
        docs.push(own);
    }
    docs
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
                    ..Default::default()
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

    fn rel(tag: &str, names: &[&str]) -> GithubRelease {
        GithubRelease {
            tag: tag.into(),
            title: String::new(),
            prerelease: tag.contains('-'),
            published: "2026-11-01".into(),
            assets: names
                .iter()
                .map(|n| ReleaseAsset {
                    name: (*n).into(),
                    url: format!("{DOWNLOAD_PREFIX}{tag}/{n}"),
                    size: 1,
                })
                .collect(),
        }
    }

    fn channels() -> Vec<GithubRelease> {
        vec![
            rel(
                "v0.6.0",
                &[
                    "alfatreze.TAU_0.6.0_2026-10-20.zip",
                    "alfatreze.TAU_Diagnostics_0.6.0_2026-10-20.zip",
                ],
            ),
            rel(
                "v0.7.0-preview.1",
                &[
                    "alfatreze.TAU_Preview_0.7.0-preview.1_2026-11-02.zip",
                    "alfatreze.TAU_Preview_Diagnostics_0.7.0-preview.1_2026-11-02.zip",
                ],
            ),
        ]
    }

    fn core(id: &str, version: &str, release: Option<&str>) -> Installed {
        Installed {
            core_id: Some(id.into()),
            version: Some(version.into()),
            release: release.map(str::to_string),
            date_release: None,
        }
    }

    #[test]
    fn a_stable_user_is_never_offered_the_preview_core_as_an_update() {
        let check = evaluate_for_core(
            &channels(),
            &core("alfatreze.TAU", "0.6.0", Some("v0.6.0")),
            None,
            &[],
        )
        .unwrap();
        assert_eq!(check.latest.tag, "v0.6.0");
        assert_eq!(check.newer, Some(false));
        assert_eq!(check.channel, Some(Channel::Stable));
        let names: Vec<_> = check.zips.iter().map(|z| z.name.as_str()).collect();
        assert_eq!(names, ["alfatreze.TAU_0.6.0_2026-10-20.zip"]);
        assert_eq!(check.others.len(), 1);
        assert!(check.others[0].starts_with("Preview v0.7.0-preview.1 is also available"));
    }

    #[test]
    fn a_core_no_release_carries_gets_no_update_only_information() {
        let releases = vec![rel(
            "v0.6.0-preview.1",
            &["alfatreze.TAU_Preview_0.6.0-preview.1_2026-10-09.zip"],
        )];
        let check = evaluate_for_core(
            &releases,
            &core("alfatreze.TAU", "0.6.0", Some("v0.6.0-alpha.3")),
            None,
            &[],
        )
        .unwrap();
        assert_eq!(check.newer, Some(false));
        assert!(check.zips.is_empty());
        assert_eq!(
            check.message,
            "No published release carries alfatreze.TAU yet."
        );
        assert_eq!(check.others.len(), 1, "{:?}", check.others);
    }

    #[test]
    fn a_preview_user_is_checked_and_offered_the_preview_zip_only() {
        let installed = core(
            "alfatreze.TAU Preview",
            "0.7.0-preview.0",
            Some("v0.7.0-preview.0"),
        );
        let check = evaluate_for_core(&channels(), &installed, None, &[]).unwrap();
        assert_eq!(check.latest.tag, "v0.7.0-preview.1");
        assert_eq!(check.newer, Some(true));
        assert_eq!(check.channel, Some(Channel::Preview));
        assert!(
            check
                .message
                .starts_with("Tau Preview v0.7.0-preview.1 is available")
        );
        let names: Vec<_> = check.zips.iter().map(|z| z.name.as_str()).collect();
        assert_eq!(
            names,
            ["alfatreze.TAU_Preview_0.7.0-preview.1_2026-11-02.zip"]
        );
        // SemVer orders the Stable release above the preview of the same X.Y.Z only once it exists; v0.6.0 is older.
        assert!(check.others.is_empty(), "{:?}", check.others);
    }

    #[test]
    fn a_preview_user_is_told_when_the_stable_release_overtakes_the_preview() {
        let mut releases = channels();
        releases.push(rel("v0.7.0", &["alfatreze.TAU_0.7.0_2026-12-01.zip"]));
        let installed = core(
            "alfatreze.TAU Preview",
            "0.7.0-preview.1",
            Some("v0.7.0-preview.1"),
        );
        let check = evaluate_for_core(&releases, &installed, None, &[]).unwrap();
        assert_eq!(
            check.newer,
            Some(false),
            "Preview updates stay on the Preview channel"
        );
        assert!(check.others.iter().any(|o| o.starts_with("Stable v0.7.0")));
    }

    #[test]
    fn diagnostics_cores_match_their_own_zip_not_the_normal_one() {
        let diag = core("alfatreze.TAU Diagnostics", "0.6.0", Some("v0.6.0"));
        let check = evaluate_for_core(&channels(), &diag, None, &[]).unwrap();
        let names: Vec<_> = check.zips.iter().map(|z| z.name.as_str()).collect();
        assert_eq!(names, ["alfatreze.TAU_Diagnostics_0.6.0_2026-10-20.zip"]);
        let old = core("alfatreze.TAU_DIAGNOSTIC", "0.4.0", None);
        let releases = vec![rel(
            "v0.4.0",
            &[
                "alfatreze.TAU_0.4.0_2026-09-22.zip",
                "alfatreze.TAU_DIAGNOSTIC_0.4.0_2026-09-22.zip",
            ],
        )];
        let check = evaluate_for_core(&releases, &old, None, &[]).unwrap();
        assert_eq!(check.zips.len(), 1);
        assert!(check.zips[0].name.contains("TAU_DIAGNOSTIC"));
    }

    fn doc_replacing(tag: &str, zip: &str, core_id: &str, replaces: &[&str]) -> compat::CompatDoc {
        compat::CompatDoc {
            schema: 2,
            release: tag.into(),
            date_release: "2026-11-01".into(),
            prerelease: false,
            previous_release: None,
            packages: vec![compat::CompatPackage {
                zip: zip.into(),
                zip_sha256: String::new(),
                core_id: core_id.into(),
                bitstream_sha256: String::new(),
                bitstream_core_version: String::new(),
                bitstream_features: None,
                rom_sha256: String::new(),
                cold_sha256: String::new(),
                rom_accepts: vec![],
                rom_needs: vec![],
                replaces: replaces.iter().map(|s| s.to_string()).collect(),
                rom_version: None,
                layout: vec![],
            }],
            persist_ids_changed: vec![],
            persist_registry: Default::default(),
            min_omega: "0.3.0".into(),
            notes: String::new(),
            source_commit: None,
            source_dirty: false,
        }
    }

    #[test]
    fn the_manifest_replaces_list_routes_an_old_core_to_its_successor_zip() {
        let releases = channels();
        let doc = doc_replacing(
            "v0.6.0",
            "alfatreze.TAU_Diagnostics_0.6.0_2026-10-20.zip",
            "alfatreze.TAU Diagnostics",
            &["alfatreze.TAU_DIAGNOSTIC"],
        );
        let old = core("alfatreze.TAU_DIAGNOSTIC", "0.4.0", None);
        let check = evaluate_for_core(&releases, &old, None, std::slice::from_ref(&doc)).unwrap();
        let names: Vec<_> = check.zips.iter().map(|z| z.name.as_str()).collect();
        assert_eq!(names, ["alfatreze.TAU_Diagnostics_0.6.0_2026-10-20.zip"]);
        // Without the manifest the name rule cannot know the successor.
        let check = evaluate_for_core(&releases, &old, None, &[]).unwrap();
        assert!(check.zips.is_empty());
    }

    #[test]
    fn same_day_dev_builds_are_ordered_by_the_full_core_version() {
        let releases = vec![rel(
            "v0.7.0-dev.386",
            &["alfatreze.TAU_DEV_385_0.7.0-dev.386.zip"],
        )];
        let mut installed = core("alfatreze.TAU DEV 385", "0.7.0-dev.385", None);
        installed.date_release = Some("2026-11-01".into());
        let check = evaluate_for_core(&releases, &installed, None, &[]).unwrap();
        assert_eq!(check.newer, Some(true));
        assert_eq!(check.channel, Some(Channel::Dev));
    }

    #[test]
    fn persist_names_come_from_the_newest_registry() {
        let mut old = doc_replacing("v0.6.0", "a.zip", "alfatreze.TAU", &[]);
        old.persist_registry.insert(
            16,
            compat::PersistMeaning {
                name: "EQ".into(),
                meaning: 1,
                since: "v0.1.0".into(),
            },
        );
        old.persist_registry.insert(
            9,
            compat::PersistMeaning {
                name: "Old only".into(),
                meaning: 1,
                since: "v0.1.0".into(),
            },
        );
        let mut new = doc_replacing("v0.7.0", "a.zip", "alfatreze.TAU", &[]);
        new.persist_registry.insert(
            16,
            compat::PersistMeaning {
                name: "Halcyon EQ preset".into(),
                meaning: 2,
                since: "v0.6.0".into(),
            },
        );
        let names = persist_names(&[old, new]);
        assert_eq!(names[&16], "Halcyon EQ preset");
        assert_eq!(names[&9], "Old only");
        assert!(persist_names(&[]).is_empty());
    }

    #[test]
    fn channel_rules() {
        assert_eq!(channel_of_version("0.6.0"), Channel::Stable);
        assert_eq!(channel_of_version("0.7.0-preview.1"), Channel::Preview);
        assert_eq!(channel_of_version("0.7.0-rc.1"), Channel::Preview);
        assert_eq!(channel_of_version("0.7.0-dev.385"), Channel::Dev);
        assert_eq!(channel_of_name("alfatreze.TAU"), Channel::Stable);
        assert_eq!(
            channel_of_name("alfatreze.TAU Diagnostics"),
            Channel::Stable
        );
        assert_eq!(
            channel_of_name("alfatreze.TAU Preview Diagnostics"),
            Channel::Preview
        );
        assert_eq!(channel_of_name("alfatreze.TAU DEV 385"), Channel::Dev);
        assert!(is_diagnostics_zip("alfatreze.TAU_Diagnostics_0.6.0_x.zip"));
        assert!(is_diagnostics_zip("alfatreze.TAU_DIAGNOSTIC_0.4.0_x.zip"));
        assert!(!is_diagnostics_zip("alfatreze.TAU_0.6.0_x.zip"));
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

    fn manifest_for(zip: &Path, release: &str) -> Vec<u8> {
        let hash = format!("{:x}", Sha256::digest(fs::read(zip).unwrap()));
        serde_json::json!({"schema": 1, "release": release, "date_release": "2026-10-07",
            "packages": [{"zip": "z.zip", "zip_sha256": hash, "core_id": "alfatreze.TAU",
                "bitstream_sha256": "00", "bitstream_core_version": "4D50331A",
                "rom_sha256": "00", "cold_sha256": "00", "rom_accepts": [], "rom_needs": []}],
            "requires_omega": {"min_omega": "0.4.0"}})
        .to_string()
        .into_bytes()
    }

    fn zip() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/packages/alfatreze.TAU_0.6.0_2026-10-07.zip")
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tau-manifests-{name}-{}", crate::test_uniq()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_manifest_is_used_only_for_the_zip_it_describes() {
        let bytes = manifest_for(&zip(), "v0.6.0-alpha.4");
        assert!(manifest_matching_zip(&zip(), &bytes).is_some());
        let other = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/packages/alfatreze.TAU_0.6.0_2026-10-04.zip");
        assert!(
            manifest_matching_zip(&other, &bytes).is_none(),
            "another zip's manifest must not vouch for this one"
        );
        assert!(manifest_matching_zip(&zip(), b"not json").is_none());
    }

    #[test]
    fn a_dev_package_brings_its_own_manifest_beside_it() {
        let dir = temp("sibling");
        fs::copy(zip(), dir.join("dev.zip")).unwrap();
        assert!(sibling_manifest(&dir.join("dev.zip")).is_none());
        fs::write(
            dir.join("tau-compat.json"),
            manifest_for(&dir.join("dev.zip"), "v0.7.0-dev.1"),
        )
        .unwrap();
        assert_eq!(
            sibling_manifest(&dir.join("dev.zip")).unwrap().release,
            "v0.7.0-dev.1"
        );
        // A manifest left beside a different zip is ignored.
        fs::write(dir.join("other.zip"), b"other").unwrap();
        assert!(sibling_manifest(&dir.join("other.zip")).is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_cache_keeps_valid_manifests_and_skips_the_rest() {
        let dir = temp("cache");
        let a = save_cached(&dir, &manifest_for(&zip(), "v0.6.0-alpha.4")).unwrap();
        assert_eq!(a.release, "v0.6.0-alpha.4");
        assert!(
            save_cached(&dir, b"{\"schema\": 99}").is_err(),
            "an unknown schema is not stored"
        );
        fs::write(dir.join("junk.json"), b"junk").unwrap();
        fs::write(dir.join("notes.txt"), b"text").unwrap();
        let loaded = load_cached(&dir);
        assert_eq!(loaded.len(), 1);
        assert!(load_cached(&dir.join("missing")).is_empty());
        // The zip's own manifest wins over a cached one for the same release.
        let beside = temp("beside");
        fs::copy(zip(), beside.join("dev.zip")).unwrap();
        fs::write(
            beside.join("tau-compat.json"),
            manifest_for(&beside.join("dev.zip"), "v0.6.0-alpha.4"),
        )
        .unwrap();
        let docs = manifests_for_install(&dir, &beside.join("dev.zip"));
        assert_eq!(docs.len(), 1, "same release: one copy");
        fs::remove_dir_all(dir).unwrap();
        fs::remove_dir_all(beside).unwrap();
    }
}
