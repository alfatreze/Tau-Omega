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
fn manifests(releases: &[release_check::GithubRelease]) -> Vec<CompatDoc> {
    let mut newest: Vec<_> = releases.iter().collect();
    newest.sort_by(|a, b| compat::compare_tags(&b.tag, &a.tag));
    newest
        .into_iter()
        .take(MANIFEST_RELEASES)
        .filter_map(|r| r.assets.iter().find(|a| a.name == "tau-compat.json"))
        .filter_map(|a| get(&a.url, MANIFEST_LIMIT).ok())
        .filter_map(|bytes| compat::parse_compat(&bytes).ok())
        .collect()
}

/// The check. `card` (optional) is a card folder whose Tau core is compared with the newest release.
pub fn check(card: Option<&str>) -> Result<Option<UpdateCheck>, TauError> {
    let releases = release_check::parse_releases(&get(RELEASES_URL, LIST_LIMIT)?)?;
    let installed = card
        .and_then(|card| installed_tau(std::path::Path::new(card)))
        .map(|identity| {
            let release = update::release_of(&identity, &manifests(&releases));
            Installed { release, date_release: Some(identity.date_release) }
        })
        .unwrap_or_default();
    // Offer only zips the release's own checksum file lists (a missing or unreadable file leaves them all, and the
    // download then refuses what it cannot verify).
    let sums = release_check::latest(&releases)
        .and_then(|r| r.assets.iter().find(|a| a.name == "SHA256SUMS.txt"))
        .and_then(|a| get(&a.url, MANIFEST_LIMIT).ok())
        .map(|bytes| release_check::parse_sums(&String::from_utf8_lossy(&bytes)));
    Ok(release_check::evaluate_with_sums(&releases, &installed, sums.as_ref()))
}

/// The normal Tau core on a card (the first library-capable core named TAU), if any.
fn installed_tau(card: &std::path::Path) -> Option<update::BuildIdentity> {
    let cores = tau_core::inspect_card(card).ok()?.cores;
    let core = cores.iter().find(|c| c.shortname == "TAU")?;
    update::installed_identity(card, &core.id)
}

#[derive(serde::Serialize)]
pub struct Downloaded {
    pub name: String,
    pub path: String,
    pub bytes: u64,
}

/// Downloads the named zips of release `tag` into `cache_dir/<tag>/`, each verified against `SHA256SUMS.txt` (and
/// the release manifest when there is one). Asset URLs come from a fresh release list, never from the caller.
pub fn download(tag: &str, names: &[String], cache_dir: PathBuf) -> Result<Vec<Downloaded>, TauError> {
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
    let doc = release
        .assets
        .iter()
        .find(|a| a.name == "tau-compat.json")
        .and_then(|a| get(&a.url, MANIFEST_LIMIT).ok())
        .and_then(|bytes| compat::parse_compat(&bytes).ok());
    let folder = cache_dir.join(tag);
    std::fs::create_dir_all(&folder)?;
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
        assert!(download("v0.0.0", &["tau.rom".into()], dir).is_err());
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// Talks to GitHub: `cargo test -- --ignored live_check --nocapture`.
    #[test]
    #[ignore]
    fn live_check_finds_the_latest_release_and_downloads_one_verified_zip() {
        let found = check(None).unwrap().expect("a release");
        println!("{} | newer={:?} | zips={:?} | manifest={}", found.message, found.newer, found.zips.iter().map(|z| &z.name).collect::<Vec<_>>(), found.manifest.is_some());
        let dir = std::env::temp_dir().join("tau-updates-live");
        let name = found.zips[0].name.clone();
        let got = download(&found.latest.tag, &[name], dir.clone()).unwrap();
        println!("downloaded {} bytes to {}", got[0].bytes, got[0].path);
        assert!(tau_core::package::inspect(std::path::Path::new(&got[0].path)).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }
}
