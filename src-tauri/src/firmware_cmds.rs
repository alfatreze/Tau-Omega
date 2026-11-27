//! Tauri commands for global per-system profiles and official firmware detection, download,
//! and reviewed installation (`Analogue Pocket` + `Game Bub`).

use crate::{ErrorCode, TauError, begin_card_write, install_backup_root};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tau_core::{
    device::{self, DeviceKind, SystemProfile},
    firmware::{
        self as fw_core, FirmwareCardStatus, FirmwareInstallPlan, FirmwareInstallReport, Uf2Drive,
    },
};
use tau_net::firmware::{self as fw_net, FirmwareDownload, FirmwareRelease};
use tauri::Manager;

fn cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    app.path()
        .app_cache_dir()
        .map_err(|e| TauError { code: ErrorCode::Io, message: e.to_string() })
}

fn uf2_mount_roots() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        vec![PathBuf::from("/Volumes")]
    }
    #[cfg(target_os = "linux")]
    {
        let mut roots = vec![PathBuf::from("/media"), PathBuf::from("/run/media"), PathBuf::from("/mnt")];
        if let Ok(user) = std::env::var("USER") {
            roots.push(PathBuf::from("/media").join(&user));
            roots.push(PathBuf::from("/run/media").join(&user));
        }
        roots
    }
    #[cfg(target_os = "windows")]
    {
        Vec::new()
    }
}

fn detect_mounted_uf2_drives() -> Vec<Uf2Drive> {
    #[cfg(target_os = "windows")]
    {
        let mut out = Vec::new();
        for letter in b'D'..=b'Z' {
            let root = PathBuf::from(format!("{}:\\", letter as char));
            let info_path = root.join("INFO_UF2.TXT");
            if let Ok(text) = fs::read_to_string(&info_path) {
                let lower = text.to_ascii_lowercase();
                if lower.contains("game bub") || lower.contains("gamebub") {
                    out.push(Uf2Drive {
                        label: if lower.contains("dock") { "GameBubDock".into() } else { "GAME BUB".into() },
                        path: root.to_string_lossy().into_owned(),
                        info: Some(text.lines().take(3).collect::<Vec<_>>().join(" · ")),
                    });
                }
            }
        }
        out
    }
    #[cfg(not(target_os = "windows"))]
    {
        fw_core::detect_uf2_drives_in(&uf2_mount_roots())
    }
}

/// Lists all supported handheld systems and their capabilities.
#[tauri::command]
pub fn systems_list() -> Vec<SystemProfile> {
    device::all_systems()
}

/// Inspects a card (if open) and connected USB UF2 bootloader drives for `device`. Read-only.
#[tauri::command(async)]
pub fn firmware_status(card: Option<String>, device: DeviceKind) -> FirmwareCardStatus {
    let uf2 = if device == DeviceKind::GameBub {
        detect_mounted_uf2_drives()
    } else {
        Vec::new()
    };
    let card_path = card.as_deref().filter(|s| !s.trim().is_empty()).map(Path::new);
    fw_core::inspect_card_firmware(device, card_path, uf2)
}

/// Fetches the latest official firmware release metadata for `device` (cached 1h unless `force`).
#[tauri::command(async)]
pub fn firmware_latest(
    app: tauri::AppHandle,
    device: DeviceKind,
    force: bool,
) -> Result<FirmwareRelease, TauError> {
    let cache = cache_dir(&app)?;
    fw_net::fetch_latest_firmware(device, &cache, force)
}

/// Downloads and verifies the official firmware image into the app's cache folder.
#[tauri::command(async)]
pub fn firmware_download(
    app: tauri::AppHandle,
    device: DeviceKind,
    release: FirmwareRelease,
) -> Result<FirmwareDownload, TauError> {
    let cache = cache_dir(&app)?;
    fw_net::download_firmware(device, &release, &cache)
}

/// Plans installing a downloaded firmware image onto a Pocket SD card root or a Game Bub UF2 drive. Read-only.
#[tauri::command(async)]
pub fn firmware_plan_install(
    target_root: String,
    device: DeviceKind,
    downloaded_path: String,
    file_name: String,
    expected_md5: Option<String>,
) -> Result<FirmwareInstallPlan, TauError> {
    fw_core::plan_install(
        device,
        Path::new(&target_root),
        Path::new(&downloaded_path),
        &file_name,
        expected_md5.as_deref(),
    )
}

/// Executes a reviewed firmware install plan.
#[tauri::command(async)]
pub fn firmware_execute_install(
    app: tauri::AppHandle,
    target_root: String,
    device: DeviceKind,
    downloaded_path: String,
    file_name: String,
    expected_md5: Option<String>,
    confirmation: String,
) -> Result<FirmwareInstallReport, TauError> {
    let _write = begin_card_write(&app, &target_root)?;
    let backup = install_backup_root(&app)?;
    fw_core::execute_install(
        device,
        Path::new(&target_root),
        Path::new(&downloaded_path),
        &file_name,
        expected_md5.as_deref(),
        &confirmation,
        &backup,
    )
}

/// Saves a copy of a cached firmware download to a user-chosen file path (for manual flashing).
#[tauri::command(async)]
pub fn firmware_save_copy(
    app: tauri::AppHandle,
    downloaded_path: String,
    dest_path: String,
) -> Result<String, TauError> {
    let dl_root = fs::canonicalize(cache_dir(&app)?.join("firmware").join("downloads")).map_err(
        |_| TauError {
            code: ErrorCode::NotFound,
            message: "Downloaded firmware file is no longer in the cache.".into(),
        },
    )?;
    let src = fs::canonicalize(&downloaded_path).map_err(|_| TauError {
        code: ErrorCode::NotFound,
        message: "Downloaded firmware file is no longer in the cache.".into(),
    })?;
    if !src.starts_with(&dl_root) || !src.is_file() {
        return Err(TauError {
            code: ErrorCode::InvalidPathReference,
            message: "Only downloaded firmware files from the app cache can be saved.".into(),
        });
    }
    let dst = Path::new(&dest_path);
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p)?;
    }
    fs::copy(&src, dst)?;
    Ok(dst.to_string_lossy().into_owned())
}
