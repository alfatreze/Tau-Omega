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
use tau_core::{compare::MediaComparison, ErrorCode, IndexStatus, ProgressObserver, TauError};
use tauri::{Emitter, Manager, State, Window};

// P1-1: with tau-core's optional `serde` feature enabled, most of this
// adapter's former hand-written DTOs are gone -- `TauError`, `Warning`,
// `PersistedSetting` and `MediaDifference` are returned to the front-end
// directly, since they now implement `Serialize` themselves. The views that
// remain (`CoreView`, `SyncPlanView`, `ComparisonView`, `PlaylistView`,
// `MediaScanView`, `TrackRow`/`LibraryScanView`) stay because they do real
// work a serde derive can't: `CoreView` turns `IndexStatus` into an English
// sentence and `TrackRow` picks friendly display fields out of `Entry::tags`
// (presentation, not a serde limitation); the others aggregate counts
// (`new_files`, `tracks`, `only_left`, ...) that are not fields stored on the
// engine type, only derivable from it. `find_problems` returns
// `tau_core::problems::Problem` directly -- it's already exactly the shape a
// front-end needs, no DTO layer earns its keep there.

#[derive(Serialize)]
struct CoreView { id: String, author: String, version: String, platform: String, library_capable: bool, index_status: String, tracks: Option<usize> }

#[derive(Serialize)]
struct SyncPlanView { id: String, new_files: usize, updates: usize, unchanged: usize, bytes_to_write: u64, warnings: Vec<tau_core::Warning> }
#[derive(Serialize)]
struct ComparisonView {
    #[serde(flatten)]
    comparison: MediaComparison,
    only_left: usize,
    only_right: usize,
    different: usize,
    identical: usize,
}
#[derive(Serialize)]
struct PlaylistView { name: String, tracks: usize }
#[derive(Serialize)]
struct MediaScanView { playlists: Vec<PlaylistView>, warnings: Vec<tau_core::Warning> }
/// One row for the Library screen's track table: friendly display fields
/// derived from `Entry::tags` (untouched original tag text, not the
/// ASCII-folded index encoding `build_index` produces) -- this is
/// presentation, same as `CoreView`'s status sentence, not a serde
/// limitation, so it stays a hand-written view.
#[derive(Serialize)]
struct TrackRow {
    rel: String,
    title: String,
    artist: String,
    album: String,
    secs: u16,
    format: &'static str,
}
fn track_row(entry: tau_core::Entry) -> TrackRow {
    let title = entry
        .tags
        .get("TIT2")
        .cloned()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            entry
                .file
                .rsplit_once('.')
                .map(|(stem, _)| stem.to_string())
                .unwrap_or_else(|| entry.file.clone())
        });
    let artist = entry
        .tags
        .get("TPE1")
        .or_else(|| entry.tags.get("TPE2"))
        .cloned()
        .unwrap_or_default();
    let album = entry.tags.get("TALB").cloned().unwrap_or_default();
    TrackRow {
        rel: entry.rel,
        title,
        artist,
        album,
        secs: entry.secs,
        format: if entry.fmt == 2 { "FLAC" } else { "MP3" },
    }
}
#[derive(Serialize)]
struct LibraryScanView { tracks: Vec<TrackRow>, playlists: Vec<PlaylistView>, warnings: Vec<tau_core::Warning> }

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
) -> Result<T, TauError> {
    let cancelled = Arc::new(AtomicBool::new(false));
    jobs.0.lock().unwrap().insert(job_id.clone(), cancelled.clone());
    let mut observer = JobObserver { window, job_id: job_id.clone(), cancelled };
    let mut observer: Option<&mut dyn ProgressObserver> = Some(&mut observer);
    let result = body(&mut observer);
    jobs.0.lock().unwrap().remove(&job_id);
    result
}

#[tauri::command]
fn scan_library(path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<LibraryScanView, TauError> {
    let scan = with_job(&window, &jobs, job_id, |progress| {
        tau_core::scan_dir_with_progress(Path::new(&path), true, progress)
    })?;
    Ok(LibraryScanView {
        tracks: scan.entries.into_iter().map(track_row).collect(),
        playlists: scan
            .playlists
            .into_iter()
            .map(|playlist| PlaylistView { name: playlist.name, tracks: playlist.rel_ids.len() })
            .collect(),
        warnings: scan.warnings,
    })
}

#[tauri::command]
fn read_journal(path: String) -> Result<Value, TauError> {
    tau_core::journal::read_journal(path)
}

#[tauri::command]
fn list_journals(dir: String) -> Result<Vec<tau_core::journal::JournalSummary>, TauError> {
    tau_core::journal::list_journals(dir)
}

const REPORTS_DIR_FILE: &str = "reports_dir.txt";

/// Where Tau Omega remembers the user's chosen reports directory: one small
/// text file in this app's own config directory (Tauri's per-OS location),
/// not a card or media root. Reading a config dir that does not exist yet
/// (a first run) is `Some(None)`, not an error.
#[tauri::command]
fn get_reports_dir(app: tauri::AppHandle) -> Result<Option<String>, TauError> {
    let path = app
        .path()
        .app_config_dir()
        .map_err(|error| TauError { code: ErrorCode::Io, message: error.to_string() })?
        .join(REPORTS_DIR_FILE);
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents.trim().to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(TauError::from(error)),
    }
}

#[tauri::command]
fn set_reports_dir(app: tauri::AppHandle, path: String) -> Result<(), TauError> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| TauError { code: ErrorCode::Io, message: error.to_string() })?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(REPORTS_DIR_FILE), path.trim())?;
    Ok(())
}

#[tauri::command]
fn scan_media(path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<MediaScanView, TauError> {
    let scan = with_job(&window, &jobs, job_id, |progress| {
        tau_core::scan_dir_with_progress(Path::new(&path), true, progress)
    })?;
    Ok(MediaScanView { playlists: scan.playlists.into_iter().map(|playlist| PlaylistView { name: playlist.name, tracks: playlist.rel_ids.len() }).collect(), warnings: scan.warnings })
}

#[tauri::command]
fn export_playlist(media_root: String, playlist_name: String, output: String) -> Result<(), TauError> {
    let scan = tau_core::scan_dir(Path::new(&media_root), true)?;
    let playlist = scan
        .playlists
        .iter()
        .find(|playlist| playlist.name == playlist_name)
        .ok_or(TauError { code: ErrorCode::NotFound, message: "playlist not found".into() })?;
    tau_core::playlist::export_m3u(Path::new(&output), playlist, &scan.entries)
}

#[tauri::command]
fn find_problems(path: String) -> Result<Vec<tau_core::problems::Problem>, TauError> {
    let scan = tau_core::scan_dir(Path::new(&path), false)?;
    tau_core::problems::find_problems(&path, &scan.entries)
}

#[tauri::command]
fn read_persisted_settings(path: String) -> Result<Vec<tau_core::diag::PersistedSetting>, TauError> {
    tau_core::diag::read_persisted_settings(path)
}

#[tauri::command]
fn inspect_card(path: String) -> Result<Vec<CoreView>, TauError> {
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
fn compare_media(left: String, right: String) -> Result<ComparisonView, TauError> {
    let comparison = tau_core::compare::media_roots(Path::new(&left), Path::new(&right))?;
    let only_left = comparison.count(tau_core::compare::DifferenceState::OnlyLeft);
    let only_right = comparison.count(tau_core::compare::DifferenceState::OnlyRight);
    let different = comparison.count(tau_core::compare::DifferenceState::Different);
    let identical = comparison.count(tau_core::compare::DifferenceState::Identical);
    Ok(ComparisonView { comparison, only_left, only_right, different, identical })
}

fn make_plan(sources: Vec<String>, destination: String, embed_covers: bool) -> Result<tau_core::sync::SyncPlan, TauError> {
    let destination = PathBuf::from(destination);
    let source_paths = sources.into_iter().filter(|source| !source.trim().is_empty()).map(PathBuf::from).collect::<Vec<_>>();
    let root_prefix = tau_core::root_prefix(&destination)?;
    tau_core::sync::plan(
        &source_paths,
        &destination,
        &root_prefix,
        tau_core::sync::PlanOptions { mirror: false, embed_covers },
        &mut None,
    )
}
fn make_core_copy_plan(source: String, destination: String) -> Result<tau_core::sync::SyncPlan, TauError> {
    let destination_path = PathBuf::from(&destination);
    let root_prefix = tau_core::root_prefix(&destination_path)?;
    tau_core::sync::plan_core_copy(Path::new(&source), &destination_path, &root_prefix, &mut None)
}

#[tauri::command]
fn plan_sync(sources: Vec<String>, destination: String, embed_covers: bool) -> Result<SyncPlanView, TauError> {
    let plan = make_plan(sources, destination, embed_covers)?;
    Ok(SyncPlanView { id: plan.id, new_files: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::New).count(), updates: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Update).count(), unchanged: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Same).count(), bytes_to_write: plan.bytes_to_write, warnings: plan.warnings })
}

#[tauri::command]
fn plan_core_copy(source: String, destination: String) -> Result<SyncPlanView, TauError> {
    let plan = make_core_copy_plan(source, destination)?;
    Ok(SyncPlanView { id: plan.id, new_files: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::New).count(), updates: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Update).count(), unchanged: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Same).count(), bytes_to_write: plan.bytes_to_write, warnings: plan.warnings })
}

#[tauri::command]
fn execute_sync(sources: Vec<String>, destination: String, confirmation: String, manifest_path: String, embed_covers: bool, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<tau_core::sync::SyncReport, TauError> {
    let plan = make_plan(sources, destination, embed_covers)?;
    let manifest = PathBuf::from(manifest_path);
    with_job(&window, &jobs, job_id, |progress| {
        tau_core::journal::execute_to_journal(&plan, &confirmation, "sync", &manifest, progress)
    })
}

#[tauri::command]
fn execute_core_copy(source: String, destination: String, confirmation: String, manifest_path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<tau_core::sync::SyncReport, TauError> {
    let plan = make_core_copy_plan(source, destination)?;
    with_job(&window, &jobs, job_id, |progress| {
        tau_core::journal::execute_to_journal(&plan, &confirmation, "core_copy", Path::new(&manifest_path), progress)
    })
}

#[tauri::command]
fn execute_core_move(source: String, destination: String, confirmation: String, delete_confirmation: String, backup_path: String, manifest_path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<tau_core::sync::SyncReport, TauError> {
    let plan = make_core_copy_plan(source.clone(), destination)?;
    let source_path = PathBuf::from(&source);
    let source_root_prefix = tau_core::root_prefix(&source_path)?;
    with_job(&window, &jobs, job_id, |progress| {
        tau_core::journal::execute_core_move_to_journal(
            &plan, &confirmation, &delete_confirmation, &source_path, &source_root_prefix, Path::new(&backup_path), Path::new(&manifest_path), progress,
        )
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(JobRegistry::default())
        .invoke_handler(tauri::generate_handler![inspect_card, scan_library, scan_media, export_playlist, find_problems, compare_media, read_journal, list_journals, get_reports_dir, set_reports_dir, read_persisted_settings, plan_sync, plan_core_copy, execute_sync, execute_core_copy, execute_core_move, cancel_job])
        .run(tauri::generate_context!())
        .expect("Tau Omega failed to start");
}
