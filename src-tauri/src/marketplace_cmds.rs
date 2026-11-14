//! Tauri commands for the core marketplace. Thin: every judgement is in `tau_core` (catalog, release choice, zip vetting,
//! installed-vs-catalog verdicts) and every request in `tau_net::marketplace`. Nothing here writes to a card: a vetted
//! download is installed through the existing plan/confirm/execute commands (`plan_package_install`).

use serde::Serialize;
use std::{path::Path, time::{SystemTime, UNIX_EPOCH}};
use tau_core::{
    ErrorCode, TauError,
    core_package::{self, Limits, Vetted},
    core_update::{self, CoreStatus},
};
use tau_net::marketplace::{self, ArtKind, CatalogResult, CoreDownloaded, Http, Source};
use tauri::Manager;

/// Most manually added GitHub cores looked up in one update check (GitHub allows 60 unauthenticated lookups an hour).
const MANUAL_LIMIT: usize = 8;

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn cache(app: &tauri::AppHandle) -> Result<std::path::PathBuf, TauError> {
    app.path().app_cache_dir().map_err(|e| TauError { code: ErrorCode::Io, message: e.to_string() })
}

/// The openFPGA Library plus the other sources, one card per core id (see `catalog::merge_sources`; the user's default marketplace
/// wins unless another lists a strictly newer version). A source that cannot be reached leaves a note, never an empty page.
fn full_catalog(app: &tauri::AppHandle, force: bool) -> Result<CatalogResult, TauError> {
    let dir = cache(app)?;
    let mut result = marketplace::fetch_catalog(&Http, &dir, now(), force)?;
    let mut cores = std::mem::take(&mut result.catalog.cores);
    match tau_net::tau_source::cores(&dir.join("tau-source"), &crate::manifest_cache(app)?) {
        Ok((tau, offline)) => {
            cores.extend(tau);
            if offline {
                result.notes.push("Tau Alpha: showing its saved list (offline).".into());
            }
        }
        Err(e) => result.notes.push(format!("Tau Alpha could not be listed: {}", e.message)),
    }
    let default = crate::read_prefs(app).map(|p| p.marketplace_default_source).unwrap_or_default();
    result.catalog.cores = tau_core::catalog::merge_sources(cores, if default.is_empty() { tau_core::catalog::LIBRARY_SOURCE } else { &default });
    Ok(result)
}

/// The whole catalog (about 330 cores; the page filters and sorts it locally). Cached; `force` is the Refresh button.
#[tauri::command(async)]
pub fn marketplace_catalog(app: tauri::AppHandle, force: bool) -> Result<CatalogResult, TauError> {
    full_catalog(&app, force)
}

/// The icon (36x36) or the platform banner (521x165) of one catalog core as a PNG data URL, or `None` when the library has no
/// picture for it. Cached for a week and revalidated, so a picture is downloaded again only when the library changes it.
#[tauri::command(async)]
pub fn marketplace_art(app: tauri::AppHandle, core_id: String, kind: ArtKind) -> Result<Option<String>, TauError> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let dir = cache(&app)?;
    if tau_core::tau_source::is_source_core(&core_id) {
        // The Tau Alpha source: the picture the package itself ships, else the publisher's GitHub profile picture (icon only).
        let banner = matches!(kind, ArtKind::Banner);
        let bytes = match tau_net::tau_source::art(&dir.join("tau-source"), banner) {
            Some(png) => Some((png, "image/png")),
            None if !banner => marketplace::fetch_avatar(&Http, &dir, tau_core::tau_source::OWNER, now())?.map(|b| {
                let mime = if b.starts_with(&[0xFF, 0xD8]) { "image/jpeg" } else { "image/png" };
                (b, mime)
            }),
            None => None,
        };
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        return Ok(bytes.map(|(b, mime)| format!("data:{mime};base64,{}", STANDARD.encode(b))));
    }
    let catalog = marketplace::fetch_catalog(&Http, &dir, now(), false)?.catalog;
    let core = catalog.find(&core_id).ok_or_else(|| TauError { code: ErrorCode::NotFound, message: format!("{core_id} is not in the catalog.") })?;
    Ok(marketplace::fetch_art(&Http, &dir, core, kind, now(), false)?.map(|png| format!("data:image/png;base64,{}", STANDARD.encode(png))))
}

/// The catalog's repository address for a core: `https://github.com/<owner>/<name>`, only when both parts are plain names.
fn repo_address(core: &tau_core::catalog::CatalogCore) -> Option<(String, String, String)> {
    let plain = |s: &str| !s.is_empty() && s.len() <= 100 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    let (o, n) = (&core.repository.owner, &core.repository.name);
    (plain(o) && plain(n)).then(|| (o.clone(), n.clone(), format!("https://github.com/{o}/{n}")))
}

/// What GitHub says about a core's repository (description, licence, download counts), for the details sheet. Cached for an hour.
#[tauri::command(async)]
pub fn marketplace_repo_details(app: tauri::AppHandle, core_id: String, force: bool) -> Result<marketplace::RepoDetails, TauError> {
    let dir = cache(&app)?;
    let catalog = full_catalog(&app, false)?.catalog;
    let core = catalog.find(&core_id).ok_or_else(|| TauError { code: ErrorCode::NotFound, message: format!("{core_id} is not in the catalog.") })?;
    let (owner, name, _) = repo_address(core).ok_or_else(|| TauError { code: ErrorCode::InvalidPathReference, message: "This core has no GitHub repository Omega can look up.".into() })?;
    marketplace::fetch_repo_details(&Http, &dir, &owner, &name, now(), force)
}

#[derive(Serialize)]
pub struct GalleryItem {
    pub alt: String,
    /// The picture as a `data:` address (checked to really be a PNG, JPEG, GIF or WebP).
    pub src: String,
}

/// The pictures a core's README shows on GitHub, for the gallery strip and lightbox in the details sheet. Best effort: no README, no
/// pictures or no connection is an empty list, never an error on the screen.
#[tauri::command(async)]
pub fn marketplace_gallery(app: tauri::AppHandle, core_id: String, force: bool) -> Result<Vec<GalleryItem>, TauError> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let dir = cache(&app)?;
    let catalog = full_catalog(&app, false)?.catalog;
    let core = catalog.find(&core_id).ok_or_else(|| TauError { code: ErrorCode::NotFound, message: format!("{core_id} is not in the catalog.") })?;
    let Some((owner, name, _)) = repo_address(core) else { return Ok(Vec::new()) };
    Ok(marketplace::fetch_gallery(&Http, &dir, &owner, &name, now(), force)
        .unwrap_or_default()
        .into_iter()
        .map(|p| GalleryItem { alt: p.alt, src: format!("data:{};base64,{}", p.mime, STANDARD.encode(p.bytes)) })
        .collect())
}

/// Opens a core's GitHub page in the computer's browser. The address is built here from the catalog entry (never taken from the screen),
/// and only plain repository names are accepted, so nothing but `https://github.com/<owner>/<name>` is ever opened.
#[tauri::command(async)]
pub fn marketplace_open_repo(app: tauri::AppHandle, core_id: String) -> Result<(), TauError> {
    let catalog = full_catalog(&app, false)?.catalog;
    let core = catalog.find(&core_id).ok_or_else(|| TauError { code: ErrorCode::NotFound, message: format!("{core_id} is not in the catalog.") })?;
    let (_, _, url) = repo_address(core).ok_or_else(|| TauError { code: ErrorCode::InvalidPathReference, message: "This core has no GitHub repository Omega can open.".into() })?;
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = { let mut c = std::process::Command::new("cmd"); c.args(["/C", "start", ""]); c };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(&url).spawn().map(|_| ()).map_err(|e| TauError { code: ErrorCode::Io, message: format!("Could not open the browser: {e}") })
}

#[derive(Serialize)]
pub struct CoreUpdates {
    pub rows: Vec<CoreStatus>,
    pub source: Source,
    pub fetched: u64,
}

/// Each core on the card against the catalog. Reads the card, never writes it.
#[tauri::command(async)]
pub fn marketplace_core_updates(app: tauri::AppHandle, card: String, force: bool) -> Result<CoreUpdates, TauError> {
    let cat = full_catalog(&app, force)?;
    let inspected = tau_core::inspect_card(Path::new(&card))?;
    let mut rows = core_update::check(&inspected.cores, &cat.catalog);
    // Cores added manually from a GitHub repository that the Library does not list: their repository's newest release (one request each, at most
    // MANUAL_LIMIT per check; a failure leaves that core "not in the Library"). The source record is Omega's own, never read from the card.
    let reg = tau_core::manual_registry::load(&app.path().app_data_dir().map_err(|e| TauError { code: ErrorCode::Io, message: e.to_string() })?.join("manual_cores.json"));
    let mut found = Vec::new();
    for e in reg.github().take(MANUAL_LIMIT) {
        let unknown = rows.iter().any(|r| r.id.eq_ignore_ascii_case(&e.core_id) && r.status == core_update::Status::NotInCatalog);
        if !unknown {
            continue;
        }
        if let Ok(releases) = tau_net::github_source::list_releases(&Http, &e.owner, &e.repo, now())
            && let Some(r) = releases.iter().find(|r| !r.prerelease).or(releases.first())
        {
            found.push(core_update::ManualRelease {
                core_id: e.core_id.clone(),
                version: r.tag.clone(),
                date: r.published.clone(),
                url: format!("https://github.com/{}/{}/releases/tag/{}", e.owner, e.repo, r.tag),
            });
        }
    }
    core_update::apply_manual(&mut rows, &found);
    Ok(CoreUpdates { rows, source: cat.source, fetched: cat.fetched })
}

#[derive(Serialize)]
pub struct MarketplaceDownload {
    pub download: CoreDownloaded,
    pub vetted: Vetted,
}

/// Downloads the core's newest release zip (or the `version` asked for, from the catalog's history) into the app cache (size and published checksum checked), then vets it. A zip
/// that fails vetting is deleted and reported. Pass `path` to `plan_package_install` to review the install.
#[tauri::command(async)]
pub fn marketplace_download(app: tauri::AppHandle, core_id: String, version: Option<String>) -> Result<MarketplaceDownload, TauError> {
    let dir = cache(&app)?;
    if tau_core::tau_source::is_source_core(&core_id) {
        // The Tau Alpha source: its own release checksums and manifest, then the same vetting and review as any core.
        let download = tau_net::tau_source::download_core(&core_id, version.as_deref(), dir.join("marketplace").join("tau-downloads"), &crate::manifest_cache(&app)?)?;
        return match core_package::vet_zip(Path::new(&download.path), &core_id, Limits::default()) {
            Ok(vetted) => Ok(MarketplaceDownload { download, vetted }),
            Err(e) => {
                let _ = std::fs::remove_file(&download.path);
                Err(TauError { code: e.code, message: format!("The download was refused: {}", e.message) })
            }
        };
    }
    let catalog = marketplace::fetch_catalog(&Http, &dir, now(), false)?.catalog;
    let core = catalog.find(&core_id).ok_or_else(|| TauError { code: ErrorCode::NotFound, message: format!("{core_id} is not in the catalog.") })?;
    let release = marketplace::resolve_release(&Http, &dir, core, version.as_deref(), now(), true)?.release;
    let limits = Limits::default();
    let download = marketplace::download_core(&Http, core, &release, &dir.join("marketplace").join("downloads"), limits.max_zip_bytes)?;
    match core_package::vet_zip(Path::new(&download.path), &core.id, limits) {
        Ok(vetted) => Ok(MarketplaceDownload { download, vetted }),
        Err(e) => {
            let _ = std::fs::remove_file(&download.path);
            Err(TauError { code: e.code, message: format!("The download was refused: {}", e.message) })
        }
    }
}
