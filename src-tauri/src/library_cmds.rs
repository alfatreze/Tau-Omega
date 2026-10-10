//! Tauri commands moved out of `main.rs` unchanged; shared helpers and views stay in `main.rs`.

use super::*;

/// The workbench's album/track/playlist listing of a media root (a local
/// library folder or a card's media root).
#[tauri::command(async)]
pub fn list_library(path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<tau_core::workbench::LibraryListing, TauError> {
    with_job(&window, &jobs, job_id, |progress| tau_core::workbench::list_library(Path::new(&path), progress))
}

/// What a reviewed change set will do, in counts (the full per-file plan
/// stays in the engine and is re-derived at execute time).
#[derive(Serialize)]
pub(super) struct ChangePlanView {
    id: String,
    new_files: usize,
    updated_files: usize,
    unchanged_files: usize,
    bytes_to_write: u64,
    removed_files: usize,
    bytes_to_remove: u64,
    edited_files: usize,
    playlists_updated: usize,
    warnings: Vec<tau_core::Warning>,
}

pub fn change_plan_view(plan: &tau_core::changes::ChangePlan) -> ChangePlanView {
    use tau_core::sync::CopyState;
    let count = |state: CopyState| plan.sync.as_ref().map_or(0, |s| s.items.iter().filter(|i| i.state == state).count());
    ChangePlanView {
        id: plan.id.clone(),
        new_files: count(CopyState::New),
        updated_files: count(CopyState::Update),
        unchanged_files: count(CopyState::Same),
        bytes_to_write: plan.bytes_to_write,
        removed_files: plan.removal.as_ref().map_or(0, |r| r.items.len()),
        bytes_to_remove: plan.bytes_to_remove,
        edited_files: plan.edit.as_ref().map_or(0, |e| e.items.len()),
        playlists_updated: plan.removal.as_ref().map_or(0, |r| r.playlist_updates.len()),
        warnings: plan.sync.as_ref().map(|s| s.warnings.clone()).unwrap_or_default(),
    }
}

#[tauri::command(async)]
pub fn plan_changes(request: tau_core::changes::ChangeRequest, destination: String) -> Result<ChangePlanView, TauError> {
    let plan = tau_core::changes::plan_changes(Path::new(&destination), &request, &mut None)?;
    Ok(change_plan_view(&plan))
}

/// Small cover pictures for a batch of album folders under `path` (a local
/// library or a card's media root). Albums with no readable picture come back
/// with `png_base64: null` so the UI can show a placeholder.
#[tauri::command(async)]
pub fn album_thumbnails(path: String, ids: Vec<String>) -> Result<Vec<tau_core::workbench::Thumbnail>, TauError> {
    tau_core::workbench::album_thumbnails(Path::new(&path), &ids, 96)
}

/// A proper thumbnail (decoded, resized, re-encoded as PNG) of an image file the
/// user picked, as a data URL; also proves the picture can be read at all.
#[tauri::command(async)]
pub fn image_thumbnail(path: String, long_side: u16) -> Result<String, TauError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = std::fs::read(&path)?;
    let png = tau_core::image::thumbnail_png(&bytes, long_side.clamp(16, 512))?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

/// The result of applying a change set: the engine's report plus where its
/// journal was written, so the UI can open that entry in the sync history.
#[derive(Serialize)]
pub(super) struct ChangeResult {
    #[serde(flatten)]
    report: tau_core::changes::ChangeReport,
    journal: String,
}

/// Best-effort retention: delete history beyond the user's limits. Never fails
/// the caller (a full disk or a permissions problem must not turn a good sync
/// into an error).
pub fn prune_history_best_effort(app: &tauri::AppHandle) {
    if let (Ok(prefs), Ok(dir)) = (read_prefs(app), resolved_reports_dir(app)) {
        let _ = tau_core::journal::prune_journals(&dir, prefs.history_keep_last, prefs.history_keep_days);
    }
}

/// Applies a reviewed change set. `backup` is the folder removed files are
/// copied to first (`None` = the user chose to remove without a backup).
/// `context` is human-readable detail (titles, card, connection) stored in the
/// journal so the sync history can describe the run in words. The journal is
/// written to the reports directory before anything changes.
#[tauri::command(async)]
pub fn execute_changes(app: tauri::AppHandle, request: tau_core::changes::ChangeRequest, destination: String, confirmation: String, backup: Option<String>, context: Option<Value>, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<ChangeResult, TauError> {
    let _write = begin_card_write(&app, &destination)?;
    let dest = PathBuf::from(&destination);
    let plan = tau_core::changes::plan_changes(&dest, &request, &mut None)?;
    let reports = resolved_reports_dir(&app)?;
    std::fs::create_dir_all(&reports)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default();
    let journal = reports.join(format!("{stamp}-library-changes.json"));
    let backup = backup.filter(|b| !b.trim().is_empty()).map(PathBuf::from);
    if let Some(dir) = &backup {
        std::fs::create_dir_all(dir)?;
    }
    let result = with_job(&window, &jobs, job_id, |progress| {
        tau_core::journal::execute_changes_to_journal(&plan, &confirmation, backup.as_deref(), &journal, context, progress)
    });
    prune_history_best_effort(&app);
    result.map(|report| ChangeResult { report, journal: journal.to_string_lossy().into_owned() })
}

/// Every journal in the sync history folder, newest first. An absent folder
/// (nothing synced yet) is an empty history, not an error.
#[tauri::command(async)]
pub fn list_history(app: tauri::AppHandle) -> Result<Vec<tau_core::journal::JournalSummary>, TauError> {
    let dir = resolved_reports_dir(&app)?;
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    tau_core::journal::list_journals(dir)
}

/// Applies the retention preferences now (also done after every sync).
#[tauri::command(async)]
pub fn prune_history(app: tauri::AppHandle) -> Result<usize, TauError> {
    let prefs = read_prefs(&app)?;
    let dir = resolved_reports_dir(&app)?;
    if !dir.is_dir() {
        return Ok(0);
    }
    tau_core::journal::prune_journals(dir, prefs.history_keep_last, prefs.history_keep_days)
}

/// Deletes the whole sync history (the user's explicit "clear history").
#[tauri::command(async)]
pub fn clear_history(app: tauri::AppHandle) -> Result<usize, TauError> {
    let dir = resolved_reports_dir(&app)?;
    if !dir.is_dir() {
        return Ok(0);
    }
    tau_core::journal::clear_journals(dir)
}
