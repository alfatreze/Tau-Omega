//! Tauri commands for Game Bub multi-device support (`D-026`, G0–G3): device detection, read-only card/core/BIOS
//! inspection, verified zip download, and reviewed plan/execute for install, update, and removal.

use crate::{ErrorCode, TauError, begin_card_write, install_backup_root};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tau_core::{
    device::{self, CardDeviceInfo, DeviceKind},
    gamebub::{self, GameBubCardView},
    gamebub_install::{
        self, GameBubInstallPlan, GameBubInstallReport, GameBubRemovePlan, GameBubRemoveReport,
    },
};
use tau_net::marketplace::{self, Http, Transport};
use tauri::Manager;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn cache(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    app.path()
        .app_cache_dir()
        .map_err(|e| TauError { code: ErrorCode::Io, message: e.to_string() })
}

/// Detects whether a card root is for Analogue Pocket, Game Bub, or blank/unknown. Read-only.
#[tauri::command(async)]
pub fn card_detect_device(card: String) -> CardDeviceInfo {
    device::detect_card(Path::new(&card))
}

/// Inspects a Game Bub card: installed cores under `/cores/` and built-in BIOS files under `/system/`. Read-only.
#[tauri::command(async)]
pub fn gamebub_inspect_card(card: String) -> GameBubCardView {
    gamebub::inspect_card(Path::new(&card))
}

#[derive(Serialize)]
pub struct GameBubDownloaded {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub bundled_core_ids: Vec<String>,
}

/// Downloads a Game Bub core zip from `cores.gamebub.net`'s catalog entry, checks `asset_sha256` and zip safety.
#[tauri::command(async)]
pub fn gamebub_download_core(
    app: tauri::AppHandle,
    core_id: String,
) -> Result<GameBubDownloaded, TauError> {
    let dir = cache(&app)?;
    let cat = marketplace::fetch_gamebub_catalog(&Http, &dir, now(), false)?.catalog;
    let core = cat
        .cores
        .iter()
        .find(|c| c.device == DeviceKind::GameBub && c.id == core_id)
        .ok_or_else(|| TauError {
            code: ErrorCode::NotFound,
            message: format!("Game Bub core '{core_id}' is not in the catalog."),
        })?;
    if core.download_url.is_empty() {
        return Err(TauError {
            code: ErrorCode::NotFound,
            message: format!("'{core_id}' has no published download URL."),
        });
    }
    let dl_dir = dir.join("marketplace").join("gamebub-downloads");
    fs::create_dir_all(&dl_dir)?;
    let safe_name = format!(
        "{}-{}.zip",
        core.id.replace(['/', '\\'], "_"),
        core.version.replace(['/', '\\'], "_")
    );
    let dest = dl_dir.join(safe_name);
    let (status, bytes, sha256) = Http.download(&core.download_url, 64 * 1024 * 1024, &dest)?;
    if status != 200 {
        let _ = fs::remove_file(&dest);
        return Err(TauError {
            code: ErrorCode::Io,
            message: format!("Download failed with HTTP status {status}."),
        });
    }
    let raw = fs::read(&dest)?;
    let expected = (!core.asset_sha256.is_empty()).then_some(core.asset_sha256.as_str());
    let vetted = match gamebub_install::inspect_zip(&raw, expected) {
        Ok(v) => v,
        Err(e) => {
            let _ = fs::remove_file(&dest);
            return Err(e);
        }
    };
    let bundled_core_ids = vetted.cores.iter().map(|c| c.core.id.clone()).collect();
    Ok(GameBubDownloaded {
        path: dest.to_string_lossy().into_owned(),
        sha256,
        bytes,
        bundled_core_ids,
    })
}

/// Plans installing one or all cores from a Game Bub zip onto `card`. Read-only.
#[tauri::command(async)]
pub fn gamebub_plan_install(
    card: String,
    zip_path: String,
    only_core: Option<String>,
    expected_sha256: Option<String>,
) -> Result<GameBubInstallPlan, TauError> {
    let raw = fs::read(&zip_path)?;
    gamebub_install::plan_install(
        Path::new(&card),
        &raw,
        only_core.as_deref(),
        expected_sha256.as_deref(),
    )
}

/// Executes a reviewed Game Bub core install/update onto `card`.
#[tauri::command(async)]
pub fn gamebub_execute_install(
    app: tauri::AppHandle,
    card: String,
    zip_path: String,
    only_core: Option<String>,
    expected_sha256: Option<String>,
    confirmation: String,
) -> Result<GameBubInstallReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let backup = install_backup_root(&app)?;
    let raw = fs::read(&zip_path)?;
    gamebub_install::execute_install(
        Path::new(&card),
        &raw,
        only_core.as_deref(),
        expected_sha256.as_deref(),
        &confirmation,
        &backup,
    )
}

/// Plans removing an installed Game Bub core from `card`. Read-only.
#[tauri::command(async)]
pub fn gamebub_plan_remove(card: String, core_id: String) -> Result<GameBubRemovePlan, TauError> {
    gamebub_install::plan_remove(Path::new(&card), &core_id)
}

/// Executes a reviewed Game Bub core removal after verified backup outside the card.
#[tauri::command(async)]
pub fn gamebub_execute_remove(
    app: tauri::AppHandle,
    card: String,
    core_id: String,
    confirmation: String,
) -> Result<GameBubRemoveReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let backup = install_backup_root(&app)?;
    gamebub_install::execute_remove(Path::new(&card), &core_id, &confirmation, &backup)
}
