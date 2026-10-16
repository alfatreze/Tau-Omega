//! The Tau Alpha marketplace source on the network: the release list of `alfatreze/Tau-Alpha` as catalog cores, and the
//! verified download of one of them. Judgement lives in `tau_core::tau_source`; the transfer, the checksum file and the
//! release manifest are the same code as the Tau update check (`super::download`). Nothing is installed here.

use super::{LIST_LIMIT, err, get, manifests};
use crate::marketplace::{Checksum, CoreDownloaded};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tau_core::{
    ErrorCode, TauError,
    catalog::CatalogCore,
    release_check::{self, RELEASES_URL},
    tau_source as logic,
};

/// The release list as last fetched, kept so the source still lists its cores offline.
const SAVED_LIST: &str = "tau-alpha-releases.json";

/// The Tau Alpha cores. A failed request falls back to the saved list (the second value says so); with none saved it is an error
/// that the caller shows as a note while the other sources still list.
pub fn cores(cache: &Path, manifest_cache: &Path) -> Result<(Vec<CatalogCore>, bool), TauError> {
    let saved = cache.join(SAVED_LIST);
    let (bytes, offline) = match get(RELEASES_URL, LIST_LIMIT) {
        Ok(bytes) => {
            let _ = std::fs::create_dir_all(cache).and_then(|()| std::fs::write(&saved, &bytes));
            (bytes, false)
        }
        Err(e) => match std::fs::read(&saved) {
            Ok(bytes) => (bytes, true),
            Err(_) => return Err(e),
        },
    };
    let releases = release_check::parse_releases(&bytes)?;
    let docs = if offline {
        release_check::load_cached(manifest_cache)
    } else {
        manifests(&releases, manifest_cache)
    };
    Ok((logic::catalog_cores(&releases, &docs), offline))
}

/// Downloads `version` (empty: the newest) of Tau core `core_id` into `dest`, checked against the release's `SHA256SUMS.txt`
/// and manifest. Returns the zip's path and facts in the marketplace's shape so the install review is the same.
pub fn download_core(
    core_id: &str,
    version: Option<&str>,
    dest: PathBuf,
    manifest_cache: &Path,
) -> Result<CoreDownloaded, TauError> {
    if core_id.eq_ignore_ascii_case(logic::DEV_CORE_ID) {
        return Err(err(
            ErrorCode::NotFound,
            "Dev builds are local and never published. Use Add a core manually with the build's zip or folder.",
        ));
    }
    let releases = release_check::parse_releases(&get(RELEASES_URL, LIST_LIMIT)?)?;
    let docs = manifests(&releases, manifest_cache);
    let cards = logic::catalog_cores(&releases, &docs);
    let option = logic::find_option(&cards, core_id).ok_or_else(|| {
        err(
            ErrorCode::NotFound,
            format!("{core_id} is not published on Tau Alpha."),
        )
    })?;
    let release = match version.filter(|v| !v.is_empty()) {
        Some(v) => option.history.iter().find(|h| h.version == v),
        None => option.history.first(),
    };
    let url = release.map(|h| h.download_url.clone()).ok_or_else(|| {
        err(
            ErrorCode::NotFound,
            format!("{core_id} has no such release to download."),
        )
    })?;
    let (tag, name) = logic::asset_of_url(&url).ok_or_else(|| {
        err(
            ErrorCode::NotFound,
            format!("{core_id} has nothing to download yet."),
        )
    })?;
    let done = super::download(&tag, &[name], dest, manifest_cache)?;
    let file = done
        .first()
        .ok_or_else(|| err(ErrorCode::NotFound, "The download returned no file."))?;
    let sha256 = Sha256::digest(std::fs::read(&file.path)?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(CoreDownloaded {
        path: file.path.clone(),
        bytes: file.bytes,
        sha256,
        // `super::download` refuses any zip its release's checksum file does not match.
        checksum: Checksum::Verified,
    })
}
