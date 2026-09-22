use serde::Serialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tau_core::{IndexStatus, ProgressObserver, TauError, Warning};
use tauri::{Emitter, State, Window};

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

/// A serialisable mirror of `tau_core::Progress`, emitted as a `"tau://progress"`
/// window event so the front-end can show a live scan/copy indicator (P0-3).
#[derive(Serialize, Clone)]
struct ProgressEvent {
    job_id: String,
    stage: &'static str,
    done: u64,
    total: u64,
    path: Option<String>,
}

/// Cancellation flags for running jobs, keyed by a caller-chosen job id. A
/// front-end starts a job with a job id, then calls `cancel_job` with the same
/// id to stop it; the flag is checked once per progress tick (P0-3).
#[derive(Default)]
struct JobRegistry(Mutex<HashMap<String, Arc<AtomicBool>>>);

#[tauri::command]
fn cancel_job(job_id: String, jobs: State<JobRegistry>) {
    if let Some(flag) = jobs.0.lock().unwrap().get(&job_id) {
        flag.store(true, Ordering::SeqCst);
    }
}

/// Builds a progress observer that emits `"tau://progress"` events on `window`
/// and asks the running call to stop once `cancelled` is set. Registers and
/// unregisters `job_id` in `jobs` so a matching `cancel_job` call can find it.
struct JobObserver<'a> {
    window: &'a Window,
    job_id: String,
    cancelled: Arc<AtomicBool>,
}
impl ProgressObserver for JobObserver<'_> {
    fn report(&mut self, progress: tau_core::Progress) -> bool {
        let _ = self.window.emit(
            "tau://progress",
            ProgressEvent {
                job_id: self.job_id.clone(),
                stage: match progress.stage {
                    tau_core::Stage::Scanning => "scanning",
                    tau_core::Stage::Hashing => "hashing",
                    tau_core::Stage::Copying => "copying",
                    tau_core::Stage::BuildingIndex => "building_index",
                    tau_core::Stage::Verifying => "verifying",
                    tau_core::Stage::Deleting => "deleting",
                },
                done: progress.done,
                total: progress.total,
                path: progress.path,
            },
        );
        !self.cancelled.load(Ordering::SeqCst)
    }
}

/// Registers a cancellable job under `job_id`, runs `body` with a progress
/// observer wired to `window`, then unregisters the job regardless of outcome.
fn with_job<T>(
    window: &Window,
    jobs: &JobRegistry,
    job_id: String,
    body: impl FnOnce(&mut Option<&mut dyn ProgressObserver>) -> Result<T, TauError>,
) -> Result<T, ApiError> {
    let cancelled = Arc::new(AtomicBool::new(false));
    jobs.0.lock().unwrap().insert(job_id.clone(), cancelled.clone());
    let mut observer = JobObserver { window, job_id: job_id.clone(), cancelled };
    let mut observer: Option<&mut dyn ProgressObserver> = Some(&mut observer);
    let result = body(&mut observer);
    jobs.0.lock().unwrap().remove(&job_id);
    result.map_err(ApiError::from)
}

#[tauri::command]
fn summarize_library(path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<LibrarySummaryView, ApiError> {
    let scan = with_job(&window, &jobs, job_id, |progress| {
        tau_core::scan_dir_with_progress(Path::new(&path), true, progress)
    })?;
    Ok(LibrarySummaryView { tracks: scan.entries.len(), playlists: scan.playlists.len(), warnings: warning_views(scan.warnings) })
}

#[tauri::command]
fn read_journal(path: String) -> Result<Value, ApiError> {
    Ok(tau_core::journal::read_journal(path)?)
}

#[tauri::command]
fn scan_media(path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<MediaScanView, ApiError> {
    let scan = with_job(&window, &jobs, job_id, |progress| {
        tau_core::scan_dir_with_progress(Path::new(&path), true, progress)
    })?;
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
    Ok(tau_core::sync::plan_with_features(&source_paths, &destination, &root_prefix, false, embed_covers, &mut None)?)
}
fn make_core_copy_plan(source: String, destination: String) -> Result<tau_core::sync::SyncPlan, ApiError> {
    let destination_path = PathBuf::from(&destination);
    let root_prefix = tau_core::root_prefix(&destination_path)?;
    Ok(tau_core::sync::plan_core_copy(Path::new(&source), &destination_path, &root_prefix, &mut None)?)
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
fn execute_sync(sources: Vec<String>, destination: String, confirmation: String, manifest_path: String, embed_covers: bool, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<SyncResultView, ApiError> {
    let plan = make_plan(sources, destination, embed_covers)?;
    let manifest = PathBuf::from(manifest_path);
    let result = with_job(&window, &jobs, job_id, |progress| {
        tau_core::journal::execute_to_journal(&plan, &confirmation, &manifest, progress)
    })?;
    Ok(SyncResultView { copied: result.copied, unchanged: result.unchanged, bytes_written: result.bytes_written, index_path: result.index_path.display().to_string(), warnings: warning_views(result.warnings) })
}

#[tauri::command]
fn execute_core_copy(source: String, destination: String, confirmation: String, manifest_path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<SyncResultView, ApiError> {
    let plan = make_core_copy_plan(source, destination)?;
    let result = with_job(&window, &jobs, job_id, |progress| {
        tau_core::journal::execute_to_journal(&plan, &confirmation, Path::new(&manifest_path), progress)
    })?;
    Ok(SyncResultView { copied: result.copied, unchanged: result.unchanged, bytes_written: result.bytes_written, index_path: result.index_path.display().to_string(), warnings: warning_views(result.warnings) })
}

#[tauri::command]
fn execute_core_move(source: String, destination: String, confirmation: String, delete_confirmation: String, backup_path: String, manifest_path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<SyncResultView, ApiError> {
    let plan = make_core_copy_plan(source.clone(), destination)?;
    let source_path = PathBuf::from(&source);
    let source_root_prefix = tau_core::root_prefix(&source_path)?;
    let result = with_job(&window, &jobs, job_id, |progress| {
        tau_core::journal::execute_core_move_to_journal(
            &plan, &confirmation, &delete_confirmation, &source_path, &source_root_prefix, Path::new(&backup_path), Path::new(&manifest_path), progress,
        )
    })?;
    Ok(SyncResultView { copied: result.copied, unchanged: result.unchanged, bytes_written: result.bytes_written, index_path: result.index_path.display().to_string(), warnings: warning_views(result.warnings) })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(JobRegistry::default())
        .invoke_handler(tauri::generate_handler![inspect_card, summarize_library, scan_media, export_playlist, find_duplicates, compare_media, read_journal, read_persisted_settings, plan_sync, plan_core_copy, execute_sync, execute_core_copy, execute_core_move, cancel_job])
        .run(tauri::generate_context!())
        .expect("Tau Omega failed to start");
}
