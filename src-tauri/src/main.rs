use serde::Serialize;
use std::path::{Path, PathBuf};
use serde_json::Value;

#[derive(Serialize)]
struct CoreView { id: String, author: String, version: String, platform: String, library_capable: bool, index_status: String, tracks: Option<usize> }

#[derive(Serialize)]
struct SyncPlanView { id: String, new_files: usize, updates: usize, unchanged: usize, bytes_to_write: u64, warnings: Vec<String> }
#[derive(Serialize)]
struct SyncResultView { copied: usize, unchanged: usize, bytes_written: u64, index_path: String, warnings: Vec<String> }
#[derive(Serialize)]
struct DifferenceView { relative: String, state: String, left_bytes: Option<u64>, right_bytes: Option<u64> }
#[derive(Serialize)]
struct ComparisonView { left: String, right: String, only_left: usize, only_right: usize, different: usize, identical: usize, differences: Vec<DifferenceView> }
#[derive(Serialize)]
struct SettingView { id: u64, kind: String, value: Value }
#[derive(Serialize)]
struct PlaylistView { name: String, tracks: usize }
#[derive(Serialize)]
struct MediaScanView { playlists: Vec<PlaylistView>, warnings: Vec<String> }
#[derive(Serialize)]
struct DuplicateView { files: Vec<String> }
#[derive(Serialize)]
struct LibrarySummaryView { tracks: usize, playlists: usize, warnings: Vec<String> }

#[tauri::command]
fn summarize_library(path: String) -> Result<LibrarySummaryView, String> {
    let scan = tau_core::scan_dir(Path::new(&path), true).map_err(|error| error.to_string())?;
    Ok(LibrarySummaryView { tracks: scan.entries.len(), playlists: scan.playlists.len(), warnings: scan.warnings })
}

#[tauri::command]
fn read_journal(path: String) -> Result<Value, String> {
    tau_core::journal::read_journal(path).map_err(|error| error.to_string())
}

#[tauri::command]
fn scan_media(path: String) -> Result<MediaScanView, String> {
    let scan = tau_core::scan_dir(Path::new(&path), true).map_err(|error| error.to_string())?;
    Ok(MediaScanView { playlists: scan.playlists.into_iter().map(|playlist| PlaylistView { name: playlist.name, tracks: playlist.rel_ids.len() }).collect(), warnings: scan.warnings })
}

#[tauri::command]
fn export_playlist(media_root: String, playlist_name: String, output: String) -> Result<(), String> {
    let scan = tau_core::scan_dir(Path::new(&media_root), true).map_err(|error| error.to_string())?;
    let playlist = scan.playlists.iter().find(|playlist| playlist.name == playlist_name).ok_or("playlist not found")?;
    tau_core::playlist::export_m3u(Path::new(&output), playlist, &scan.entries).map_err(|error| error.to_string())
}

#[tauri::command]
fn find_duplicates(path: String) -> Result<Vec<DuplicateView>, String> {
    let scan = tau_core::scan_dir(Path::new(&path), false).map_err(|error| error.to_string())?;
    tau_core::duplicates::find_duplicates(&path, &scan.entries)
        .map(|groups| groups.into_iter().map(|files| DuplicateView { files }).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn read_persisted_settings(path: String) -> Result<Vec<SettingView>, String> {
    tau_core::diag::read_persisted_settings(path).map(|settings| settings.into_iter().map(|setting| SettingView { id: setting.id, kind: setting.kind, value: setting.value }).collect())
}

#[tauri::command]
fn inspect_card(path: String) -> Result<Vec<CoreView>, String> {
    tau_core::inspect_card(path).map(|card| {
        let card_root = card.root;
        card.cores.into_iter().map(|core| {
        let media_root = card_root.join("Assets").join(&core.platform).join("common");
        let index = media_root.join("tau-library.tdb");
        let (index_status, tracks) = if !index.is_file() {
            ("No index".into(), None)
        } else {
            match std::fs::read(&index).ok().and_then(|bytes| tau_core::parse(bytes).ok()) {
                Some(parsed) => ("Index ready".into(), Some(parsed.counts.tracks as usize)),
                None => ("Index needs repair".into(), None),
            }
        };
        CoreView { id: core.id, author: core.author, version: core.version, platform: core.platform, library_capable: core.library_capable, index_status, tracks }
    }).collect()
    }).map_err(|e| e.to_string())
}

#[tauri::command]
fn compare_media(left: String, right: String) -> Result<ComparisonView, String> {
    let comparison = tau_core::compare::media_roots(Path::new(&left), Path::new(&right)).map_err(|error| error.to_string())?;
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

fn root_prefix(common: &Path) -> Result<String, String> {
    let platform = common.parent().and_then(Path::file_name).and_then(|name| name.to_str()).ok_or("Destination must be Assets/<platform>/common")?;
    Ok(format!("/Assets/{platform}/common/"))
}

fn make_plan(sources: Vec<String>, destination: String, embed_covers: bool) -> Result<tau_core::sync::SyncPlan, String> {
    let destination = PathBuf::from(destination);
    let source_paths = sources.into_iter().filter(|source| !source.trim().is_empty()).map(PathBuf::from).collect::<Vec<_>>();
    tau_core::sync::plan_with_features(&source_paths, &destination, &root_prefix(&destination)?, false, embed_covers).map_err(|error| error.to_string())
}
fn make_core_copy_plan(source: String, destination: String) -> Result<tau_core::sync::SyncPlan, String> {
    let destination_path = PathBuf::from(&destination);
    tau_core::sync::plan_core_copy(Path::new(&source), &destination_path, &root_prefix(&destination_path)?).map_err(|error| error.to_string())
}

#[tauri::command]
fn plan_sync(sources: Vec<String>, destination: String, embed_covers: bool) -> Result<SyncPlanView, String> {
    let plan = make_plan(sources, destination, embed_covers)?;
    Ok(SyncPlanView { id: plan.id, new_files: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::New).count(), updates: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Update).count(), unchanged: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Same).count(), bytes_to_write: plan.bytes_to_write, warnings: plan.warnings })
}

#[tauri::command]
fn plan_core_copy(source: String, destination: String) -> Result<SyncPlanView, String> {
    let plan = make_core_copy_plan(source, destination)?;
    Ok(SyncPlanView { id: plan.id, new_files: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::New).count(), updates: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Update).count(), unchanged: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Same).count(), bytes_to_write: plan.bytes_to_write, warnings: plan.warnings })
}

#[tauri::command]
fn execute_sync(sources: Vec<String>, destination: String, confirmation: String, manifest_path: String, embed_covers: bool) -> Result<SyncResultView, String> {
    let plan = make_plan(sources, destination, embed_covers)?;
    let manifest = PathBuf::from(manifest_path);
    let result = tau_core::journal::execute_to_journal(&plan, &confirmation, &manifest).map_err(|error| error.to_string())?;
    Ok(SyncResultView { copied: result.copied, unchanged: result.unchanged, bytes_written: result.bytes_written, index_path: result.index_path.display().to_string(), warnings: result.warnings })
}

#[tauri::command]
fn execute_core_copy(source: String, destination: String, confirmation: String, manifest_path: String) -> Result<SyncResultView, String> {
    let plan = make_core_copy_plan(source, destination)?;
    let result = tau_core::journal::execute_to_journal(&plan, &confirmation, Path::new(&manifest_path)).map_err(|error| error.to_string())?;
    Ok(SyncResultView { copied: result.copied, unchanged: result.unchanged, bytes_written: result.bytes_written, index_path: result.index_path.display().to_string(), warnings: result.warnings })
}

#[tauri::command]
fn execute_core_move(source: String, destination: String, confirmation: String, delete_confirmation: String, backup_path: String, manifest_path: String) -> Result<SyncResultView, String> {
    let plan = make_core_copy_plan(source.clone(), destination)?;
    let source_path = PathBuf::from(&source);
    let result = tau_core::journal::execute_core_move_to_journal(
        &plan, &confirmation, &delete_confirmation, &source_path, &root_prefix(&source_path)?, Path::new(&backup_path), Path::new(&manifest_path),
    ).map_err(|error| error.to_string())?;
    Ok(SyncResultView { copied: result.copied, unchanged: result.unchanged, bytes_written: result.bytes_written, index_path: result.index_path.display().to_string(), warnings: result.warnings })
}

fn main() {
    tauri::Builder::default().plugin(tauri_plugin_dialog::init()).invoke_handler(tauri::generate_handler![inspect_card, summarize_library, scan_media, export_playlist, find_duplicates, compare_media, read_journal, read_persisted_settings, plan_sync, plan_core_copy, execute_sync, execute_core_copy, execute_core_move]).run(tauri::generate_context!()).expect("Tau Omega failed to start");
}
