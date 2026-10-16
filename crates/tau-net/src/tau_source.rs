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
    let mut cards = logic::catalog_cores(&releases, &docs);
    for card in &mut cards {
        if let Some(meta) = package_meta(card, cache, manifest_cache, offline) {
            meta.apply(card);
        }
    }
    Ok((cards, offline))
}

/// What the card says about the core, read from the newest standard package itself (its `core.json` and the platform file
/// it ships), not typed in here: the category and year are the package's own, so they follow whatever the author sets.
#[derive(serde::Serialize, serde::Deserialize)]
struct PackageMeta {
    category: String,
    manufacturer: String,
    year: u32,
    firmware_required: String,
    dock_supported: Option<bool>,
    link_port: Option<bool>,
    sleep_supported: Option<bool>,
}

impl PackageMeta {
    fn apply(self, card: &mut CatalogCore) {
        card.category = self.category;
        card.manufacturer = self.manufacturer;
        card.year = self.year;
        card.firmware_required = self.firmware_required;
        card.dock_supported = self.dock_supported;
        card.link_port = self.link_port;
        card.sleep_supported = self.sleep_supported;
    }
}

/// The package metadata, read from the newest standard package that downloads and verifies (a release whose zip does not match its
/// own checksum file is skipped, never trusted): cached per release, so a package is downloaded once. Offline, or when none can be
/// read, the saved copy is used, else nothing and the card keeps its neutral defaults.
fn package_meta(
    card: &CatalogCore,
    cache: &Path,
    manifest_cache: &Path,
    offline: bool,
) -> Option<PackageMeta> {
    let mut candidates: Vec<&tau_core::catalog::ReleaseSummary> = card
        .options
        .iter()
        .filter(|o| !o.diagnostics && !o.history.is_empty())
        .flat_map(|o| o.history.iter().take(1))
        .collect();
    candidates.sort_by(|a, b| tau_core::compat::compare_tags(&b.version, &a.version));
    for release in candidates.into_iter().take(3) {
        let Some((tag, name)) = logic::asset_of_url(&release.download_url) else {
            continue;
        };
        let saved = cache.join(format!("meta-{tag}.json"));
        if let Some(meta) = std::fs::read(&saved)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
        {
            return Some(meta);
        }
        if offline {
            continue;
        }
        if let Some(meta) = read_meta(&tag, &name, cache, manifest_cache) {
            if let Ok(bytes) = serde_json::to_vec(&meta) {
                let _ = std::fs::write(&saved, bytes);
            }
            return Some(meta);
        }
    }
    None
}

fn read_meta(tag: &str, name: &str, cache: &Path, manifest_cache: &Path) -> Option<PackageMeta> {
    let done = super::download(
        tag,
        &[name.to_string()],
        cache.join("meta-zips"),
        manifest_cache,
    )
    .ok()?;
    let zip = PathBuf::from(&done.first()?.path);
    let result = (|| {
        let vetted = tau_core::core_package::vet_zip(
            &zip,
            &card_core_of(&zip)?,
            tau_core::core_package::Limits::default(),
        )
        .ok()?;
        let preview = tau_core::core_preview::preview(&zip, &vetted).ok()?;
        let platform = preview.platforms.first()?;
        for (suffix, png) in [
            (".icon.png", &preview.icon_png),
            (".banner.png", &preview.banner_png),
        ] {
            if let Some(png) = png {
                let _ = std::fs::write(cache.join(format!("meta-{tag}{suffix}")), png);
            }
        }
        Some(PackageMeta {
            category: platform.category.clone(),
            manufacturer: platform.manufacturer.clone(),
            year: platform.year,
            firmware_required: preview.firmware_required.clone(),
            dock_supported: preview.dock_supported,
            link_port: preview.link_port,
            sleep_supported: preview.sleep_supported,
        })
    })();
    let _ = std::fs::remove_file(&zip);
    result
}

/// The Tau card's picture from the newest package read so far: the author icon (`Cores/<id>/icon.bin`) or the platform banner,
/// as PNG bytes. `None` until a package has been read; the caller then falls back to the author's GitHub avatar.
pub fn art(cache: &Path, banner: bool) -> Option<Vec<u8>> {
    let suffix = if banner { ".banner.png" } else { ".icon.png" };
    std::fs::read_dir(cache)
        .ok()?
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with("meta-") && n.ends_with(suffix)
        })
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .and_then(|e| std::fs::read(e.path()).ok())
}

/// The core folder a downloaded Tau zip carries, from its own file name (`alfatreze.TAU_Preview_0.6...` is `alfatreze.TAU Preview`).
fn card_core_of(zip: &Path) -> Option<String> {
    let name = zip.file_name()?.to_string_lossy().into_owned();
    let release = release_check::GithubRelease {
        tag: String::new(),
        title: String::new(),
        prerelease: false,
        published: String::new(),
        assets: Vec::new(),
    };
    logic::core_id_of_zip(&release, &name, &[])
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

#[cfg(test)]
mod tests {
    /// Hits the real GitHub release list (and downloads the newest Tau package once): `cargo test -p tau-net -- --ignored real_tau`.
    #[test]
    #[ignore]
    fn real_tau_source_lists_one_card_with_the_package_metadata() {
        let dir = std::env::temp_dir().join(format!("tau-source-real-{}", std::process::id()));
        let (cards, _) = super::cores(&dir.join("c"), &dir.join("m")).unwrap();
        assert_eq!(cards.len(), 1);
        let card = &cards[0];
        eprintln!(
            "{} {} {} {} {} fw {}",
            card.name,
            card.version,
            card.category,
            card.manufacturer,
            card.year,
            card.firmware_required
        );
        for o in &card.options {
            eprintln!(
                "  {} {} diag={} {:?}",
                o.core_id,
                o.channel,
                o.diagnostics,
                o.history.iter().map(|h| &h.version).collect::<Vec<_>>()
            );
        }
        assert_eq!(card.category, "Media Players");
        let icon = super::art(&dir.join("c"), false).expect("the package icon is kept");
        assert!(icon.starts_with(b"\x89PNG"));
        assert!(super::art(&dir.join("c"), true).is_some_and(|b| b.starts_with(b"\x89PNG")));
        let avatar = crate::marketplace::fetch_avatar(
            &crate::marketplace::Http,
            &dir.join("a"),
            tau_core::tau_source::OWNER,
            1,
        )
        .unwrap();
        assert!(avatar.is_some_and(|b| b.starts_with(b"\x89PNG") || b.starts_with(&[0xFF, 0xD8])));
        let _ = std::fs::remove_dir_all(dir);
    }
}
