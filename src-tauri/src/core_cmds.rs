//! Tauri commands for the Cores section. Thin: the platform tree, a core's details, files, saves, settings and screenshots are read by
//! `tau_core` (read-only); adding files is a plan then a confirmed, verified copy under the card write lock, with replaced files
//! backed up outside the card first.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::path::{Path, PathBuf};
use tau_core::{
    TauError,
    core_add::{AddPlan, AddReport, Destination},
    core_remove::{RemoveFilesPlan, RemoveFilesReport},
    core_files::CoreFiles,
    core_screens::ScreenshotView,
    core_settings::CoreSettings,
    corehub::{CoreDetail, PlatformNode},
};

fn paths(list: Vec<String>) -> Vec<PathBuf> {
    list.into_iter().map(PathBuf::from).collect()
}

/// Installed cores grouped by main platform (the Cores menu).
#[tauri::command(async)]
pub fn cores_tree(card: String) -> Result<Vec<PlatformNode>, TauError> {
    tau_core::corehub::platform_tree(Path::new(&card))
}

#[tauri::command(async)]
pub fn core_detail(card: String, core_id: String) -> Result<CoreDetail, TauError> {
    tau_core::corehub::core_detail(Path::new(&card), &core_id)
}

/// The folders a core uses with their files, the files it still needs, and Pocket Sync's save backups.
#[tauri::command(async)]
pub fn core_files(card: String, core_id: String) -> Result<CoreFiles, TauError> {
    tau_core::core_files::core_files(Path::new(&card), &core_id)
}

#[tauri::command(async)]
pub fn core_settings(card: String, core_id: String) -> Result<CoreSettings, TauError> {
    tau_core::core_settings::core_settings(Path::new(&card), &core_id)
}

#[tauri::command(async)]
pub fn core_screenshots(card: String, core_id: String) -> Result<ScreenshotView, TauError> {
    tau_core::core_screens::screenshots(Path::new(&card), &core_id)
}

/// A small PNG of one screenshot as a data URL (the file name must be a plain `.png` in `Memories/Screenshots`).
#[tauri::command(async)]
pub fn screenshot_thumbnail(card: String, file: String, long_side: u16) -> Result<String, TauError> {
    let path = tau_core::core_screens::screenshot_path(Path::new(&card), &file)?;
    let png = tau_core::image::thumbnail_png(&std::fs::read(path)?, long_side.clamp(48, 640))?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

/// Plans adding files or folders to one of the core's asset folders. Reads the sources and the card; writes nothing.
#[tauri::command(async)]
pub fn plan_core_add(card: String, core_id: String, platform: String, destination: Destination, sources: Vec<String>, replace: bool) -> Result<AddPlan, TauError> {
    tau_core::core_add::plan_add(Path::new(&card), &core_id, &platform, destination, &paths(sources), replace)
}

/// Adds the reviewed files. Every copy is verified; replaced files are copied to the app's backup folder first.
#[tauri::command(async)]
pub fn execute_core_add(
    app: tauri::AppHandle,
    card: String,
    core_id: String,
    platform: String,
    destination: Destination,
    sources: Vec<String>,
    replace: bool,
    confirmation: String,
) -> Result<AddReport, TauError> {
    let _write = crate::begin_card_write(&app, &card)?;
    let backup = crate::install_backup_root(&app)?;
    tau_core::core_add::execute_add(Path::new(&card), &core_id, &platform, destination, &paths(sources), replace, &confirmation, Some(&backup))
}

/// Plans removing files from one of the core's asset folders. Reads the card; deletes nothing.
#[tauri::command(async)]
pub fn plan_core_remove(card: String, core_id: String, files: Vec<String>) -> Result<RemoveFilesPlan, TauError> {
    tau_core::core_remove::plan_remove_files(Path::new(&card), &core_id, &files)
}

/// Removes the reviewed files. Each is copied to the app's backup folder and verified before it is deleted.
#[tauri::command(async)]
pub fn execute_core_remove(app: tauri::AppHandle, card: String, core_id: String, files: Vec<String>, confirmation: String) -> Result<RemoveFilesReport, TauError> {
    let _write = crate::begin_card_write(&app, &card)?;
    let backup = crate::install_backup_root(&app)?;
    tau_core::core_remove::execute_remove_files(Path::new(&card), &core_id, &files, &confirmation, &backup)
}
