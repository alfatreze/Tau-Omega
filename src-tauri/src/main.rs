use serde::Serialize;
use std::path::{Path, PathBuf};
use serde_json::Value;
use tau_core::{IndexStatus, TauError, Warning};

/// A boundary error: `code` is `TauError::code()`'s stable numeric identifier,
/// so the front-end can branch on it directly instead of matching English
/// text (P0-2). `message` is for display only.
#[derive(Serialize)]
struct ApiError {
    code: u16,
    message: String,
}
impl From<TauError> for ApiError {
    fn from(error: TauError) -> Self {
        Self {
            code: error.code().as_u16(),
            message: error.message,
        }
    }
}

#[derive(Serialize)]
struct WarningView {
    code: String,
    message: String,
}
impl From<Warning> for WarningView {
    fn from(warning: Warning) -> Self {
        Self {
            code: warning.code.as_str().into(),
            message: warning.message,
        }
    }
}
fn warning_views(warnings: Vec<Warning>) -> Vec<WarningView> {
    warnings.into_iter().map(WarningView::from).collect()
}

#[derive(Serialize)]
struct CoreView { id: String, author: String, version: String, platform: String, library_capable: bool, index_status: String, tracks: Option<usize> }

#[derive(Serialize)]
struct SyncPlanView { id: String, new_files: usize, updates: usize, unchanged: usize, bytes_to_write: u64, warnings: Vec<WarningView> }
#[derive(Serialize)]
struct SyncResultView { copied: usize, unchanged: usize, bytes_written: u64, index_path: String, warnings: Vec<WarningView> }
#[derive(Serialize)]
struct DifferenceView { relative: String, state: String, left_bytes: Option<u64>, right_bytes: Option<u64> }
#[derive(Serialize)]
struct ComparisonView { left: String, right: String, only_left: usize, only_right: usize, different: usize, identical: usize, differences: Vec<DifferenceView> }
#[derive(Serialize)]
struct SettingView { id: u64, kind: String, value: Value }
#[derive(Serialize)]
struct PlaylistView { name: String, tracks: usize }
#[derive(Serialize)]
struct MediaScanView { playlists: Vec<PlaylistView>, warnings: Vec<WarningView> }
#[derive(Serialize)]
struct DuplicateView { files: Vec<String> }
#[derive(Serialize)]
struct LibrarySummaryView { tracks: usize, playlists: usize, warnings: Vec<WarningView> }

#[tauri::command]
fn summarize_library(path: String) -> Result<LibrarySummaryView, ApiError> {
    let scan = tau_core::scan_dir(Path::new(&path), true)?;
    Ok(LibrarySummaryView { tracks: scan.entries.len(), playlists: scan.playlists.len(), warnings: warning_views(scan.warnings) })
}

#[tauri::command]
fn read_journal(path: String) -> Result<Value, ApiError> {
    Ok(tau_core::journal::read_journal(path)?)
}

#[tauri::command]
fn scan_media(path: String) -> Result<MediaScanView, ApiError> {
    let scan = tau_core::scan_dir(Path::new(&path), true)?;
    Ok(MediaScanView { playlists: scan.playlists.into_iter().map(|playlist| PlaylistView { name: playlist.name, tracks: playlist.rel_ids.len() }).collect(), warnings: warning_views(scan.warnings) })
}

#[tauri::command]
fn export_playlist(media_root: String, playlist_name: String, output: String) -> Result<(), ApiError> {
    let scan = tau_core::scan_dir(Path::new(&media_root), true)?;
    let playlist = scan.playlists.iter().find(|playlist| playlist.name == playlist_name).ok_or(ApiError { code: 0, message: "playlist not found".into() })?;
    Ok(tau_core::playlist::export_m3u(Path::new(&output), playlist, &scan.entries)?)
}

#[tauri::command]
fn find_duplicates(path: String) -> Result<Vec<DuplicateView>, ApiError> {
    let scan = tau_core::scan_dir(Path::new(&path), false)?;
    Ok(tau_core::duplicates::find_duplicates(&path, &scan.entries)?
        .into_iter().map(|files| DuplicateView { files }).collect())
}

#[tauri::command]
fn read_persisted_settings(path: String) -> Result<Vec<SettingView>, ApiError> {
    Ok(tau_core::diag::read_persisted_settings(path)?.into_iter().map(|setting| SettingView { id: setting.id, kind: setting.kind, value: setting.value }).collect())
}

#[tauri::command]
fn inspect_card(path: String) -> Result<Vec<CoreView>, ApiError> {
    Ok(tau_core::inspect_card(path)?.cores.into_iter().map(|core| {
        let (index_status, tracks) = match core.index_status {
            IndexStatus::NoIndex => ("No index".to_string(), None),
            IndexStatus::Ready { tracks } => ("Index ready".to_string(), Some(tracks as usize)),
            IndexStatus::NeedsRepair => ("Index needs repair".to_string(), None),
        };
        CoreView { id: core.id, author: core.author, version: core.version, platform: core.platform, library_capable: core.library_capable, index_status, tracks }
    }).collect())
}

#[tauri::command]
fn compare_media(left: String, right: String) -> Result<ComparisonView, ApiError> {
    let comparison = tau_core::compare::media_roots(Path::new(&left), Path::new(&right))?;
    let only_left = comparison.count(tau_core::compare::DifferenceState::OnlyLeft);
    let only_right = comparison.count(tau_core::compare::DifferenceState::OnlyRight);
    let different = comparison.count(tau_core::compare::DifferenceState::Different);
    let identical = comparison.count(tau_core::compare::DifferenceState::Identical);
    let differences = comparison.differences.into_iter().map(|item| DifferenceView {
        relative: item.relative.display().to_string(),
        state: match item.state {
            tau_core::compare::DifferenceState::OnlyLeft => "only_left",
            tau_core::compare::DifferenceState::OnlyRight => "only_right",
            tau_core::compare::DifferenceState::Different => "different",
            tau_core::compare::DifferenceState::Identical => "identical",
        }.into(),
        left_bytes: item.left_bytes,
        right_bytes: item.right_bytes,
    }).collect();
    Ok(ComparisonView {
        left: comparison.left.display().to_string(), right: comparison.right.display().to_string(),
        only_left, only_right, different, identical, differences,
    })
}

fn make_plan(sources: Vec<String>, destination: String, embed_covers: bool) -> Result<tau_core::sync::SyncPlan, ApiError> {
    let destination = PathBuf::from(destination);
    let source_paths = sources.into_iter().filter(|source| !source.trim().is_empty()).map(PathBuf::from).collect::<Vec<_>>();
    let root_prefix = tau_core::root_prefix(&destination)?;
    Ok(tau_core::sync::plan_with_features(&source_paths, &destination, &root_prefix, false, embed_covers)?)
}
fn make_core_copy_plan(source: String, destination: String) -> Result<tau_core::sync::SyncPlan, ApiError> {
    let destination_path = PathBuf::from(&destination);
    let root_prefix = tau_core::root_prefix(&destination_path)?;
    Ok(tau_core::sync::plan_core_copy(Path::new(&source), &destination_path, &root_prefix)?)
}

#[tauri::command]
fn plan_sync(sources: Vec<String>, destination: String, embed_covers: bool) -> Result<SyncPlanView, ApiError> {
    let plan = make_plan(sources, destination, embed_covers)?;
    Ok(SyncPlanView { id: plan.id, new_files: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::New).count(), updates: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Update).count(), unchanged: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Same).count(), bytes_to_write: plan.bytes_to_write, warnings: warning_views(plan.warnings) })
}

#[tauri::command]
fn plan_core_copy(source: String, destination: String) -> Result<SyncPlanView, ApiError> {
    let plan = make_core_copy_plan(source, destination)?;
    Ok(SyncPlanView { id: plan.id, new_files: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::New).count(), updates: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Update).count(), unchanged: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Same).count(), bytes_to_write: plan.bytes_to_write, warnings: warning_views(plan.warnings) })
}

#[tauri::command]
fn execute_sync(sources: Vec<String>, destination: String, confirmation: String, manifest_path: String, embed_covers: bool) -> Result<SyncResultView, ApiError> {
    let plan = make_plan(sources, destination, embed_covers)?;
    let manifest = PathBuf::from(manifest_path);
    let result = tau_core::journal::execute_to_journal(&plan, &confirmation, &manifest)?;
    Ok(SyncResultView { copied: result.copied, unchanged: result.unchanged, bytes_written: result.bytes_written, index_path: result.index_path.display().to_string(), warnings: warning_views(result.warnings) })
}

#[tauri::command]
fn execute_core_copy(source: String, destination: String, confirmation: String, manifest_path: String) -> Result<SyncResultView, ApiError> {
    let plan = make_core_copy_plan(source, destination)?;
    let result = tau_core::journal::execute_to_journal(&plan, &confirmation, Path::new(&manifest_path))?;
    Ok(SyncResultView { copied: result.copied, unchanged: result.unchanged, bytes_written: result.bytes_written, index_path: result.index_path.display().to_string(), warnings: warning_views(result.warnings) })
}

#[tauri::command]
fn execute_core_move(source: String, destination: String, confirmation: String, delete_confirmation: String, backup_path: String, manifest_path: String) -> Result<SyncResultView, ApiError> {
    let plan = make_core_copy_plan(source.clone(), destination)?;
    let source_path = PathBuf::from(&source);
    let source_root_prefix = tau_core::root_prefix(&source_path)?;
    let result = tau_core::journal::execute_core_move_to_journal(
        &plan, &confirmation, &delete_confirmation, &source_path, &source_root_prefix, Path::new(&backup_path), Path::new(&manifest_path),
    )?;
    Ok(SyncResultView { copied: result.copied, unchanged: result.unchanged, bytes_written: result.bytes_written, index_path: result.index_path.display().to_string(), warnings: warning_views(result.warnings) })
}

fn main() {
    tauri::Builder::default().plugin(tauri_plugin_dialog::init()).invoke_handler(tauri::generate_handler![inspect_card, summarize_library, scan_media, export_playlist, find_duplicates, compare_media, read_journal, read_persisted_settings, plan_sync, plan_core_copy, execute_sync, execute_core_copy, execute_core_move]).run(tauri::generate_context!()).expect("Tau Omega failed to start");
}
