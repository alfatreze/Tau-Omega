//! Tauri commands moved out of `main.rs` unchanged; shared helpers and views stay in `main.rs`.

use super::*;

pub fn make_playlist_write_plan(
    path: &str,
    file: &str,
    tracks: &[String],
) -> Result<tau_core::playlist::PlaylistPlan, TauError> {
    let common = Path::new(path);
    let scan = tau_core::scan_dir(common, false)?;
    tau_core::playlist::plan_write(common, file, tracks, &scan.entries)
}

/// Plans creating or reordering a playlist -- writing `tracks`, in order, to
/// `file`. Reordering an existing playlist is the same call with a permuted
/// `tracks`, so there is no separate "reorder" command.
#[tauri::command(async)]
pub fn plan_playlist_write(
    path: String,
    file: String,
    tracks: Vec<String>,
) -> Result<tau_core::playlist::PlaylistPlan, TauError> {
    make_playlist_write_plan(&path, &file, &tracks)
}

#[tauri::command(async)]
pub fn execute_playlist_write(app: tauri::AppHandle, 
    path: String,
    file: String,
    tracks: Vec<String>,
    confirmation: String,
) -> Result<(), TauError> {
    let _write = begin_card_write(&app, &path)?;
    let plan = make_playlist_write_plan(&path, &file, &tracks)?;
    tau_core::playlist::execute(Path::new(&path), &plan, &confirmation)
}

#[tauri::command(async)]
pub fn plan_playlist_rename(
    path: String,
    old_file: String,
    new_file: String,
) -> Result<tau_core::playlist::PlaylistPlan, TauError> {
    tau_core::playlist::plan_rename(Path::new(&path), &old_file, &new_file)
}

#[tauri::command(async)]
pub fn execute_playlist_rename(app: tauri::AppHandle, 
    path: String,
    old_file: String,
    new_file: String,
    confirmation: String,
) -> Result<(), TauError> {
    let _write = begin_card_write(&app, &path)?;
    let plan = tau_core::playlist::plan_rename(Path::new(&path), &old_file, &new_file)?;
    tau_core::playlist::execute(Path::new(&path), &plan, &confirmation)
}

pub fn make_playlist_import_plan(
    path: &str,
    source: &str,
    dest_file: &str,
) -> Result<tau_core::playlist::PlaylistPlan, TauError> {
    let common = Path::new(path);
    let scan = tau_core::scan_dir(common, false)?;
    tau_core::playlist::plan_import(common, Path::new(source), dest_file, &scan.entries)
}

#[tauri::command(async)]
pub fn plan_playlist_import(
    path: String,
    source: String,
    dest_file: String,
) -> Result<tau_core::playlist::PlaylistPlan, TauError> {
    make_playlist_import_plan(&path, &source, &dest_file)
}

#[tauri::command(async)]
pub fn execute_playlist_import(app: tauri::AppHandle, 
    path: String,
    source: String,
    dest_file: String,
    confirmation: String,
) -> Result<(), TauError> {
    let _write = begin_card_write(&app, &path)?;
    let plan = make_playlist_import_plan(&path, &source, &dest_file)?;
    tau_core::playlist::execute(Path::new(&path), &plan, &confirmation)
}

#[tauri::command(async)]
pub fn export_playlist(media_root: String, playlist_name: String, output: String) -> Result<(), TauError> {
    let scan = tau_core::scan_dir(Path::new(&media_root), true)?;
    let playlist = scan
        .playlists
        .iter()
        .find(|playlist| playlist.name == playlist_name)
        .ok_or(TauError { code: ErrorCode::NotFound, message: "playlist not found".into() })?;
    tau_core::playlist::export_m3u(Path::new(&output), playlist, &scan.entries)
}
