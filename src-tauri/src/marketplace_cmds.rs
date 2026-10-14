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

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn cache(app: &tauri::AppHandle) -> Result<std::path::PathBuf, TauError> {
    app.path().app_cache_dir().map_err(|e| TauError { code: ErrorCode::Io, message: e.to_string() })
}

/// The whole catalog (about 330 cores; the page filters and sorts it locally). Cached; `force` is the Refresh button.
#[tauri::command(async)]
pub fn marketplace_catalog(app: tauri::AppHandle, force: bool) -> Result<CatalogResult, TauError> {
    marketplace::fetch_catalog(&Http, &cache(&app)?, now(), force)
}

/// The icon (36x36) or the platform banner (521x165) of one catalog core as a PNG data URL, or `None` when the library has no
/// picture for it. Cached for a week and revalidated, so a picture is downloaded again only when the library changes it.
#[tauri::command(async)]
pub fn marketplace_art(app: tauri::AppHandle, core_id: String, kind: ArtKind) -> Result<Option<String>, TauError> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let dir = cache(&app)?;
    let catalog = marketplace::fetch_catalog(&Http, &dir, now(), false)?.catalog;
    let core = catalog.find(&core_id).ok_or_else(|| TauError { code: ErrorCode::NotFound, message: format!("{core_id} is not in the catalog.") })?;
    Ok(marketplace::fetch_art(&Http, &dir, core, kind, now(), false)?.map(|png| format!("data:image/png;base64,{}", STANDARD.encode(png))))
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
    let cat = marketplace::fetch_catalog(&Http, &cache(&app)?, now(), force)?;
    let inspected = tau_core::inspect_card(Path::new(&card))?;
    Ok(CoreUpdates { rows: core_update::check(&inspected.cores, &cat.catalog), source: cat.source, fetched: cat.fetched })
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
