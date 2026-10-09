//! The GitHub update check and download (roadmap item 1b). The only network code in Tau Omega.
//!
//! What leaves the machine: one unauthenticated GET of the public release list, plus (when a release has them) its
//! small `tau-compat.json`; and, only when the user presses Update, the zips and `SHA256SUMS.txt` of one release.
//! No identifier, card name or path is sent. The judgement (is it newer, is the download trustworthy) is
//! `tau_core::release_check`, tested on real captured responses; this file only moves bytes.

use std::{path::PathBuf, time::Duration};
use tau_core::{
    ErrorCode, TauError,
    compat::{self, CompatDoc},
    release_check::{self, DOWNLOAD_PREFIX, Installed, RELEASES_URL, UpdateCheck},
    update,
};

const LIST_LIMIT: u64 = 4 * 1024 * 1024;
const MANIFEST_LIMIT: u64 = 2 * 1024 * 1024;
const ZIP_LIMIT: u64 = 64 * 1024 * 1024;
/// Manifests read to identify an installed build: the newest few releases only.
const MANIFEST_RELEASES: usize = 5;

fn err(code: ErrorCode, message: impl Into<String>) -> TauError {
    TauError { code, message: message.into() }
}

/// Plain-words explanation of a failed request.
fn explain(error: &ureq::Error) -> String {
    match error {
        ureq::Error::StatusCode(403 | 429) => "GitHub is limiting requests from this connection right now. Try again later.".into(),
        ureq::Error::StatusCode(404) => "That release or file is no longer on GitHub.".into(),
        ureq::Error::StatusCode(code) => format!("GitHub answered with an error ({code})."),
        ureq::Error::Timeout(_) => "GitHub did not answer in time. Check the connection and try again.".into(),
        ureq::Error::Io(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => "Could not reach GitHub. Are you offline?".into(),
        other => format!("The request failed: {other}"),
    }
}

/// One GET, only to the Tau repository, capped at `limit` bytes.
fn get(url: &str, limit: u64) -> Result<Vec<u8>, TauError> {
    if !(url == RELEASES_URL || url.starts_with(DOWNLOAD_PREFIX)) {
        return Err(err(ErrorCode::InvalidPathReference, "Omega only fetches from the Tau release page."));
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(8)))
        .timeout_global(Some(Duration::from_secs(90)))
        .user_agent(concat!("Tau-Omega/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let mut response = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| err(ErrorCode::Io, explain(&e)))?;
    response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|e| err(ErrorCode::Io, explain(&e)))
}

/// Manifests of the newest releases that publish one; a malformed or unknown-schema file is skipped, not fatal.
fn manifests(releases: &[release_check::GithubRelease], cache: &std::path::Path) -> Vec<CompatDoc> {
    let mut newest: Vec<_> = releases.iter().collect();
    newest.sort_by(|a, b| compat::compare_tags(&b.tag, &a.tag));
    newest
        .into_iter()
        .take(MANIFEST_RELEASES)
        .filter_map(|r| r.assets.iter().find(|a| a.name == "tau-compat.json"))
        .filter_map(|a| get(&a.url, MANIFEST_LIMIT).ok())
        // Kept in the cache so an install plan made later (maybe offline) knows these releases.
        .filter_map(|bytes| release_check::save_cached(cache, &bytes).ok())
        .collect()
}

/// The check. `card` (optional) is a card folder whose Tau cores are compared with the releases of their own channel.
pub fn check(card: Option<&str>, manifest_cache: &std::path::Path) -> Result<Option<UpdateCheck>, TauError> {
    let releases = release_check::parse_releases(&get(RELEASES_URL, LIST_LIMIT)?)?;
    let docs = manifests(&releases, manifest_cache);
    // Offer only zips the chosen release's own checksum file lists (a missing or unreadable file leaves them all, and
    // the download then refuses what it cannot verify).
    let sums_of = |release: &release_check::GithubRelease| {
        release
            .assets
            .iter()
            .find(|a| a.name == "SHA256SUMS.txt")
            .and_then(|a| get(&a.url, MANIFEST_LIMIT).ok())
            .map(|bytes| release_check::parse_sums(&String::from_utf8_lossy(&bytes)))
    };
    let cores = card.map(|card| installed_tau_cores(std::path::Path::new(card))).unwrap_or_default();
    if cores.is_empty() {
        let sums = release_check::latest(&releases).and_then(sums_of);
        return Ok(release_check::evaluate_with_sums(&releases, &Installed::default(), sums.as_ref()));
    }
    // Every installed Tau core is judged against its own channel; an Update is reported for the first one that has
    // one, otherwise the first core's answer ("up to date").
    let mut checks: Vec<UpdateCheck> = cores
        .iter()
        .filter_map(|identity| {
            let installed = Installed {
                core_id: Some(identity.core_id.clone()),
                version: Some(identity.version.clone()).filter(|v| !v.is_empty()),
                release: update::release_of(identity, &docs),
                date_release: Some(identity.date_release.clone()),
            };
            release_check::evaluate_for_core(&releases, &installed, None, &docs)
        })
        .collect();
    let position = checks.iter().position(|c| c.newer == Some(true)).unwrap_or(0);
    if checks.is_empty() {
        return Ok(None);
    }
    let mut check = checks.swap_remove(position);
    if let Some(sums) = sums_of(&check.latest) {
        check.zips.retain(|z| sums.contains_key(&z.name));
    }
    Ok(Some(check))
}

/// Every Tau-family core on a card (folder id starts `alfatreze.TAU`, as the storage breakdown groups them), in
/// folder order.
fn installed_tau_cores(card: &std::path::Path) -> Vec<update::BuildIdentity> {
    let Ok(card) = tau_core::inspect_card(card) else { return Vec::new() };
    card.cores
        .iter()
        .filter(|c| c.id.to_ascii_lowercase().starts_with("alfatreze.tau"))
        .filter_map(|c| update::installed_identity(&card.root, &c.id))
        .collect()
}

#[derive(serde::Serialize)]
pub struct Downloaded {
    pub name: String,
    pub path: String,
    pub bytes: u64,
}

/// Downloads the named zips of release `tag` into `cache_dir/<tag>/`, each verified against `SHA256SUMS.txt` (and
/// the release manifest when there is one). Asset URLs come from a fresh release list, never from the caller.
pub fn download(tag: &str, names: &[String], cache_dir: PathBuf, manifest_cache: &std::path::Path) -> Result<Vec<Downloaded>, TauError> {
    let releases = release_check::parse_releases(&get(RELEASES_URL, LIST_LIMIT)?)?;
    let release = releases
        .iter()
        .find(|r| r.tag == tag)
        .ok_or_else(|| err(ErrorCode::NotFound, format!("{tag} is not on GitHub any more.")))?;
    let asset = |name: &str| {
        release
            .assets
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| err(ErrorCode::NotFound, format!("{name} is not part of {tag}.")))
    };
    let sums_text = get(&asset("SHA256SUMS.txt")?.url, MANIFEST_LIMIT)?;
    let sums = release_check::parse_sums(&String::from_utf8_lossy(&sums_text));
    let manifest_bytes = release
        .assets
        .iter()
        .find(|a| a.name == "tau-compat.json")
        .and_then(|a| get(&a.url, MANIFEST_LIMIT).ok());
    let doc = manifest_bytes.as_deref().and_then(|bytes| compat::parse_compat(bytes).ok());
    let folder = cache_dir.join(tag);
    std::fs::create_dir_all(&folder)?;
    if let (Some(bytes), Some(_)) = (&manifest_bytes, &doc) {
        // Beside the zips (the install plan finds it there) and in the shared cache (it knows this release afterwards).
        std::fs::write(folder.join("tau-compat.json"), bytes)?;
        let _ = release_check::save_cached(manifest_cache, bytes);
    }
    let mut out = Vec::new();
    for name in names {
        if !name.ends_with(".zip") {
            return Err(err(ErrorCode::InvalidPathReference, "Only the release zips are downloaded."));
        }
        let bytes = get(&asset(name)?.url, ZIP_LIMIT)?;
        release_check::verify_download(name, &bytes, &sums, doc.as_ref())?;
        let path = folder.join(name);
        let temp = folder.join(format!("{name}.part"));
        std::fs::write(&temp, &bytes)?;
        std::fs::rename(&temp, &path)?;
        out.push(Downloaded { name: name.clone(), path: path.to_string_lossy().into_owned(), bytes: bytes.len() as u64 });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_tau_release_page_is_ever_fetched() {
        for url in ["https://example.com/a.zip", "http://api.github.com/repos/alfatreze/Tau-Alpha/releases", "file:///etc/passwd",
                    "https://github.com/someone-else/Tau-Alpha/releases/download/v1/a.zip"] {
            assert_eq!(get(url, 10).unwrap_err().code, ErrorCode::InvalidPathReference, "{url}");
        }
    }

    #[test]
    fn download_refuses_anything_but_a_zip_before_touching_the_network_for_it() {
        // (The release list fetch comes first; with no network this fails earlier, which is also a refusal.)
        let dir = std::env::temp_dir().join("tau-updates-test");
        assert!(download("v0.0.0", &["tau.rom".into()], dir.clone(), &dir).is_err());
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// Talks to GitHub: `cargo test -- --ignored live_check --nocapture`.
    #[test]
    #[ignore]
    fn live_check_finds_the_latest_release_and_downloads_one_verified_zip() {
        let found = check(None, &std::env::temp_dir().join("tau-manifests-live")).unwrap().expect("a release");
        println!("{} | newer={:?} | zips={:?} | manifest={}", found.message, found.newer, found.zips.iter().map(|z| &z.name).collect::<Vec<_>>(), found.manifest.is_some());
        let dir = std::env::temp_dir().join("tau-updates-live");
        let name = found.zips[0].name.clone();
        let got = download(&found.latest.tag, &[name], dir.clone(), &dir.join("m")).unwrap();
        println!("downloaded {} bytes to {}", got[0].bytes, got[0].path);
        assert!(tau_core::package::inspect(std::path::Path::new(&got[0].path)).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }
}
