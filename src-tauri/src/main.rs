mod cacheflush;
mod device;
mod ledger_host;
mod updates;

use serde::{Deserialize, Serialize};
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
struct CoreView { id: String, author: String, shortname: String, version: String, platform: String, platforms: Vec<String>, media_platform: String, platform_category: Option<String>, library_capable: bool, index_status: String, tracks: Option<usize> }

/// One album's worth of `plan_sync`'s discovered `art_sidecars`: enough for
/// the Sync screen to list what would be written and to ask
/// `preview_art_sidecar` for a look at one of them, without sending every
/// track's own destination path across the boundary.
#[derive(Serialize, Clone)]
struct ArtSidecarPreviewView { folder: String, cover_source: String }
#[derive(Serialize)]
struct SyncPlanView { id: String, new_files: usize, updates: usize, unchanged: usize, bytes_to_write: u64, art_sidecars: usize, art_sidecar_previews: Vec<ArtSidecarPreviewView>, warnings: Vec<tau_core::Warning> }
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
/// The Playlists page's own scan view: unlike `PlaylistView`, it needs each
/// playlist's tracks (as media-relative paths, to target rename/reorder/
/// import at a specific entry and to reorder in place) and `file` -- real
/// work `tau_core::Playlist` can't do itself, since it only stores `rel_ids`
/// (indices into a scan's own `entries`, meaningless once that scan is gone).
#[derive(Serialize)]
struct PlaylistDetailView { name: String, file: String, tracks: Vec<String> }
#[derive(Serialize)]
struct MediaScanView { playlists: Vec<PlaylistDetailView>, warnings: Vec<tau_core::Warning> }
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
/// Commands now run off the window's main thread, so two could overlap. Card
/// writes must never: one at a time, and a second request is refused with a
/// clear message rather than queued silently behind a long copy.
static CARD_WRITE: Mutex<()> = Mutex::new(());

fn card_write_guard() -> Result<std::sync::MutexGuard<'static, ()>, TauError> {
    match CARD_WRITE.try_lock() {
        Ok(guard) => Ok(guard),
        Err(std::sync::TryLockError::Poisoned(poisoned)) => Ok(poisoned.into_inner()),
        Err(std::sync::TryLockError::WouldBlock) => Err(TauError {
            code: ErrorCode::Io,
            message: "another change to a card is already running; wait for it to finish".into(),
        }),
    }
}

/// Takes the card write lock and, if the user chose to keep Spotlight off Pocket cards, makes sure the card holding
/// `path` carries the `.metadata_never_index` marker (best effort: a failure here never blocks the write itself).
fn begin_card_write(app: &tauri::AppHandle, path: &str) -> Result<std::sync::MutexGuard<'static, ()>, TauError> {
    let guard = card_write_guard()?;
    if read_prefs(app).map(|p| p.card_marker == "on").unwrap_or(false) {
        let _ = tau_core::marker::ensure(Path::new(path));
    }
    Ok(guard)
}

#[derive(Serialize)]
struct MarkerStatus { setting: String, is_card: bool, present: bool }

/// The Spotlight setting and whether the card holding `path` has the marker. Read-only.
#[tauri::command(async)]
fn card_marker_status(app: tauri::AppHandle, path: String) -> Result<MarkerStatus, TauError> {
    let present = tau_core::marker::is_present(Path::new(&path));
    Ok(MarkerStatus { setting: read_prefs(&app)?.card_marker, is_card: present.is_some(), present: present.unwrap_or(false) })
}

/// Saves the user's answer (`"on"`, `"off"`) and, for `"on"`, adds the marker to the card holding `path` now (the
/// user just agreed to this write).
#[tauri::command(async)]
fn set_card_marker(app: tauri::AppHandle, setting: String, path: Option<String>) -> Result<MarkerStatus, TauError> {
    if !matches!(setting.as_str(), "on" | "off") {
        return Err(TauError { code: ErrorCode::InvalidPathReference, message: "card_marker must be on or off".into() });
    }
    let mut prefs = read_prefs(&app)?;
    prefs.card_marker = setting.clone();
    let dir = config_dir(&app)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(PREFS_FILE), serde_json::to_vec_pretty(&prefs).map_err(|e| TauError { code: ErrorCode::Json, message: e.to_string() })?)?;
    if setting == "on" {
        if let Some(path) = &path {
            let _write = card_write_guard()?;
            tau_core::marker::ensure(Path::new(path))?;
        }
    }
    card_marker_status(app, path.unwrap_or_default())
}

/// Removes the marker from the card holding `path` (the user's explicit action; the setting is not changed).
#[tauri::command(async)]
fn remove_card_marker(app: tauri::AppHandle, path: String) -> Result<MarkerStatus, TauError> {
    {
        let _write = card_write_guard()?;
        tau_core::marker::remove(Path::new(&path))?;
    }
    card_marker_status(app, path)
}

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
                    tau_core::Stage::Editing => "editing",
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

#[tauri::command(async)]
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

#[tauri::command(async)]
fn read_journal(path: String) -> Result<Value, TauError> {
    tau_core::journal::read_journal(path)
}

#[tauri::command(async)]
fn list_journals(dir: String) -> Result<Vec<tau_core::journal::JournalSummary>, TauError> {
    tau_core::journal::list_journals(dir)
}

const REPORTS_DIR_FILE: &str = "reports_dir.txt";

/// Where Tau Omega remembers the user's chosen reports directory: one small
/// text file in this app's own config directory (Tauri's per-OS location),
/// not a card or media root. Reading a config dir that does not exist yet
/// (a first run) is `Ok(None)`, not an error. See [`get_reports_dir`] for the
/// folder actually used.
fn configured_reports_dir(app: &tauri::AppHandle) -> Result<Option<String>, TauError> {
    let path = app
        .path()
        .app_config_dir()
        .map_err(|error| TauError { code: ErrorCode::Io, message: error.to_string() })?
        .join(REPORTS_DIR_FILE);
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents.trim().to_string()).filter(|c| !c.is_empty())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(TauError::from(error)),
    }
}

/// The folder every journal goes in: the folder the user chose in Settings, or
/// a default `reports` folder in the app's data directory. Always a real path,
/// so sync history works without any setup.
#[tauri::command(async)]
fn get_reports_dir(app: tauri::AppHandle) -> Result<Option<String>, TauError> {
    Ok(Some(resolved_reports_dir(&app)?.to_string_lossy().into_owned()))
}

#[tauri::command(async)]
fn set_reports_dir(app: tauri::AppHandle, path: String) -> Result<(), TauError> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| TauError { code: ErrorCode::Io, message: error.to_string() })?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(REPORTS_DIR_FILE), path.trim())?;
    Ok(())
}

const RECENT_CARDS_FILE: &str = "recent_cards.txt";
const RECENT_CARDS_MAX: usize = 8;

/// Cards this app has successfully opened before, most-recent-first, so the
/// Cards screen can offer them back on the next launch instead of the user
/// retyping a path -- one small text file in this app's config directory,
/// same convention as [`get_reports_dir`]. A card that no longer exists
/// (ejected, renamed) stays listed; the frontend decides how to handle that
/// when the user picks it, this command only remembers paths.
#[tauri::command(async)]
fn get_recent_cards(app: tauri::AppHandle) -> Result<Vec<String>, TauError> {
    let path = app
        .path()
        .app_config_dir()
        .map_err(|error| TauError { code: ErrorCode::Io, message: error.to_string() })?
        .join(RECENT_CARDS_FILE);
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(contents.lines().map(str::to_string).collect()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(TauError::from(error)),
    }
}

/// Records a successfully opened card: moves it to the front if already
/// remembered, otherwise prepends it, and caps the list at
/// [`RECENT_CARDS_MAX`] entries.
#[tauri::command(async)]
fn record_recent_card(app: tauri::AppHandle, path: String) -> Result<(), TauError> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| TauError { code: ErrorCode::Io, message: error.to_string() })?;
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(RECENT_CARDS_FILE);
    let path = path.trim().to_string();
    let mut recent: Vec<String> = match std::fs::read_to_string(&file) {
        Ok(contents) => contents.lines().map(str::to_string).collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(TauError::from(error)),
    };
    recent.retain(|existing| existing != &path);
    recent.insert(0, path);
    recent.truncate(RECENT_CARDS_MAX);
    std::fs::write(file, recent.join("\n"))?;
    Ok(())
}

/// Looks for mounted volumes that look like an Analogue Pocket card (a
/// top-level folder with both `Cores` and `Assets`). A cheap directory check
/// on the platform's usual mount locations (see `device::mounted_cards`), so
/// the front-end can poll it to notice a card or the Pocket being connected.
#[tauri::command(async)]
fn list_mounted_cards() -> Result<Vec<String>, TauError> {
    Ok(device::mounted_cards())
}

/// How a card is connected: the Pocket's own (slow) USB mode, a card reader,
/// or unknown when the OS will not say.
/// Where the space on a card goes: each Tau platform's media (one segment per
/// platform folder; cores sharing a platform share a segment) against everything
/// else. Names and sizes only, off the main thread, cancellable by job id.
#[tauri::command(async)]
fn card_breakdown(card: String, cores: Vec<tau_core::breakdown::CoreRef>, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<tau_core::breakdown::CardBreakdown, TauError> {
    with_job(&window, &jobs, job_id, |progress| tau_core::breakdown::card_breakdown(Path::new(&card), &cores, progress))
}

/// Unmounts and ejects a card so it is safe to unplug (flushes anything the OS
/// still holds). Refused while a card write is running, and holds the write
/// guard for its duration so none can start halfway through.
#[tauri::command(async)]
fn eject_card(path: String) -> Result<device::EjectResult, TauError> {
    let _write = card_write_guard()?;
    Ok(device::eject(Path::new(&path)))
}

/// Whether read-backs after a write are checked against the device itself (true
/// where a cache evictor is registered) and how many evictions have failed.
#[derive(Serialize)]
struct ReadbackStatus {
    checks_the_device: bool,
    failed_evictions: u64,
}

#[tauri::command]
fn readback_status() -> ReadbackStatus {
    let (checks_the_device, failed_evictions) = tau_core::sync::readback_report();
    ReadbackStatus { checks_the_device, failed_evictions }
}

/// Forgets what the app remembers about the card holding this media root. Always safe: the next scan
/// simply reads the files again.
#[tauri::command(async)]
fn ledger_forget(media_root: String) -> bool {
    tau_core::ledger::clear(Path::new(&media_root))
}

#[tauri::command(async)]
fn detect_connection(path: String) -> device::ConnectionInfo {
    device::detect_connection(Path::new(&path))
}

const PREFS_FILE: &str = "workbench_prefs.json";

/// Library-workbench preferences, kept as one small JSON file in the app's
/// config directory (same convention as the other settings files).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
struct Prefs {
    /// `"backup"` (copy removed files to `backup_dir` first), `"ask"` (ask
    /// each time) or `"none"` (just remove).
    remove_mode: String,
    /// User-chosen backup folder; `None` means the app's default folder.
    backup_dir: Option<String>,
    /// Whether the one-time "removals are staged and backed up" note was shown.
    remove_explained: bool,
    /// The user ticked "don't ask again" on the slow direct-connection alert.
    slow_alert_suppressed: bool,
    /// Last measured write speed per card name, in bytes per second.
    speeds: std::collections::BTreeMap<String, f64>,
    /// Manual "how is this card connected" override per card name.
    connections: std::collections::BTreeMap<String, String>,
    /// Sync history retention: keep at most this many entries (`0` = no limit).
    history_keep_last: usize,
    /// Sync history retention: delete entries older than this many days (`0` = no limit).
    history_keep_days: u64,
    /// Look for a newer Tau release on GitHub when the app starts (only the check; installing is always the user's action).
    check_updates: bool,
    /// The one-time note about what the update check sends was shown.
    update_notice_shown: bool,
    /// Keep macOS Spotlight from scanning Pocket cards: `"ask"` (not decided yet), `"on"` (add the marker file when
    /// writing to a card) or `"off"`.
    card_marker: String,
}
impl Default for Prefs {
    fn default() -> Self {
        Self {
            remove_mode: "backup".into(),
            backup_dir: None,
            remove_explained: false,
            slow_alert_suppressed: false,
            speeds: Default::default(),
            connections: Default::default(),
            history_keep_last: 100,
            history_keep_days: 365,
            check_updates: true,
            update_notice_shown: false,
            card_marker: "ask".into(),
        }
    }
}

/// `Prefs` as the front-end sees them: the same fields plus the resolved
/// default folders, so the UI can always show a real path.
#[derive(Serialize)]
struct PrefsView {
    #[serde(flatten)]
    prefs: Prefs,
    default_backup_dir: String,
    reports_dir: String,
}

fn config_dir(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    app.path().app_config_dir().map_err(|error| TauError { code: ErrorCode::Io, message: error.to_string() })
}
fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    app.path().app_data_dir().map_err(|error| TauError { code: ErrorCode::Io, message: error.to_string() })
}
fn read_prefs(app: &tauri::AppHandle) -> Result<Prefs, TauError> {
    match std::fs::read(config_dir(app)?.join(PREFS_FILE)) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Prefs::default()),
        Err(error) => Err(TauError::from(error)),
    }
}
/// The folder journals go in: the user's chosen reports directory if they set
/// one on the Jobs page, otherwise a folder in the app's own data directory.
fn resolved_reports_dir(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    if let Some(dir) = configured_reports_dir(app)? {
        return Ok(PathBuf::from(dir));
    }
    Ok(data_dir(app)?.join("reports"))
}
fn prefs_view(app: &tauri::AppHandle) -> Result<PrefsView, TauError> {
    Ok(PrefsView {
        prefs: read_prefs(app)?,
        default_backup_dir: data_dir(app)?.join("removed-backups").to_string_lossy().into_owned(),
        reports_dir: resolved_reports_dir(app)?.to_string_lossy().into_owned(),
    })
}

/// Release manifests (`tau-compat.json`) seen so far, kept so an install plan knows them even when offline.
fn manifest_cache(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    Ok(app.path().app_cache_dir().map_err(|error| TauError { code: ErrorCode::Io, message: error.to_string() })?.join("manifests"))
}

/// Looks for a newer Tau release on GitHub, comparing with the Tau core on `card` when one is given. Does nothing
/// (returns `None`) when the user turned the check off. Never downloads or installs anything.
#[tauri::command(async)]
fn update_check(app: tauri::AppHandle, card: Option<String>) -> Result<Option<tau_core::release_check::UpdateCheck>, TauError> {
    if !read_prefs(&app)?.check_updates {
        return Ok(None);
    }
    updates::check(card.as_deref(), &manifest_cache(&app)?)
}

/// Downloads the chosen zips of one release into the app's cache, verified against the release's checksums. Only
/// called when the user presses Update.
#[tauri::command(async)]
fn update_download(app: tauri::AppHandle, tag: String, names: Vec<String>) -> Result<Vec<updates::Downloaded>, TauError> {
    let cache = app.path().app_cache_dir().map_err(|error| TauError { code: ErrorCode::Io, message: error.to_string() })?;
    updates::download(&tag, &names, cache.join("updates"), &manifest_cache(&app)?)
}

/// The library's health (cheap, no tag scan) for the badge: present, valid, rooted at this core, files all there.
#[tauri::command(async)]
fn library_health(media_root: String) -> tau_core::refresh::IndexState {
    tau_core::refresh::index_state(Path::new(&media_root))
}

/// Plans a library refresh of one core's media folder (`Assets/<platform>/common`). Reads tags; writes nothing.
#[tauri::command(async)]
fn plan_library_refresh(media_root: String) -> Result<tau_core::refresh::RefreshPlan, TauError> {
    tau_core::refresh::plan_refresh(Path::new(&media_root))
}

/// Runs a reviewed refresh: backs up what it replaces outside the card, renames, rewrites playlists, rebuilds and
/// verifies the index; rolls back by itself on any failure.
#[tauri::command(async)]
fn execute_library_refresh(app: tauri::AppHandle, media_root: String, confirmation: String) -> Result<tau_core::refresh::RefreshReport, TauError> {
    let _write = begin_card_write(&app, &media_root)?;
    let root = Path::new(&media_root);
    let plan = tau_core::refresh::plan_refresh(root)?;
    tau_core::refresh::execute_refresh(root, &plan, &confirmation, &refresh_backup_root(&app)?)
}

/// Undoes a refresh from its backup folder.
#[tauri::command(async)]
fn rollback_library_refresh(app: tauri::AppHandle, media_root: String, backup_dir: String) -> Result<(), TauError> {
    let _write = begin_card_write(&app, &media_root)?;
    tau_core::refresh::rollback_refresh(Path::new(&media_root), Path::new(&backup_dir))
}

fn refresh_backup_root(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    Ok(match read_prefs(app)?.backup_dir {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir).join("refreshes"),
        _ => data_dir(app)?.join("refresh-backups"),
    })
}

/// Where install backups go: the user's chosen backup folder, else a folder in the app's data directory.
fn install_backup_root(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    Ok(match read_prefs(app)?.backup_dir {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir).join("installs"),
        _ => data_dir(app)?.join("install-backups"),
    })
}

/// Plans installing or updating a core package on a card: verdict, backups, cleanup, space, refusals. Writes nothing.
#[tauri::command(async)]
fn plan_core_update(app: tauri::AppHandle, zip: String, card: String, allow_downgrade: bool) -> Result<tau_core::install_plan::InstallPlan, TauError> {
    let docs = tau_core::release_check::manifests_for_install(&manifest_cache(&app)?, Path::new(&zip));
    tau_core::install_plan::plan(Path::new(&zip), Path::new(&card), &docs, allow_downgrade)
}

/// Runs a reviewed install plan: verified backup outside the card, the writes, cleanup, the post-install check. Rolls
/// back by itself if a step fails.
#[tauri::command(async)]
fn execute_core_update(app: tauri::AppHandle, zip: String, card: String, allow_downgrade: bool, confirmation: String) -> Result<tau_core::install_exec::InstallReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let (zip, card) = (Path::new(&zip), Path::new(&card));
    // The same manifests the plan was made with (so the confirmation token matches), unless the cache changed meanwhile.
    let docs = tau_core::release_check::manifests_for_install(&manifest_cache(&app)?, zip);
    let plan = tau_core::install_plan::plan(zip, card, &docs, allow_downgrade)?;
    tau_core::install_exec::execute(zip, card, &plan, &confirmation, &install_backup_root(&app)?, &docs)
}

/// Puts a card back as it was before an install, from that install's backup folder.
#[tauri::command(async)]
fn rollback_core_update(app: tauri::AppHandle, card: String, backup_dir: String) -> Result<tau_core::install_exec::RollbackReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    tau_core::install_exec::rollback(Path::new(&card), Path::new(&backup_dir))
}

#[tauri::command(async)]
fn get_prefs(app: tauri::AppHandle) -> Result<PrefsView, TauError> {
    prefs_view(&app)
}

#[tauri::command(async)]
fn set_prefs(app: tauri::AppHandle, prefs: Prefs) -> Result<PrefsView, TauError> {
    if !matches!(prefs.remove_mode.as_str(), "backup" | "ask" | "none") {
        return Err(TauError { code: ErrorCode::InvalidPathReference, message: "remove_mode must be backup, ask or none".into() });
    }
    let dir = config_dir(&app)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(PREFS_FILE), serde_json::to_vec_pretty(&prefs).map_err(|e| TauError { code: ErrorCode::Json, message: e.to_string() })?)?;
    prefs_view(&app)
}

const MANUAL_PLAYERS_FILE: &str = "manual_players.txt";

/// Core ids the user has manually marked as a player (`Set as player`) even
/// though their platform's own `category` isn't `"Media Players"` (or is
/// unknown) -- an override list, not a replacement for the real signal.
/// Same one-file-in-the-config-dir convention as recent cards.
#[tauri::command(async)]
fn get_manual_players(app: tauri::AppHandle) -> Result<Vec<String>, TauError> {
    let path = app
        .path()
        .app_config_dir()
        .map_err(|error| TauError { code: ErrorCode::Io, message: error.to_string() })?
        .join(MANUAL_PLAYERS_FILE);
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(contents.lines().map(str::to_string).collect()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(TauError::from(error)),
    }
}

/// Adds or removes one core id from the manual-players override list.
#[tauri::command(async)]
fn set_manual_player(app: tauri::AppHandle, core_id: String, enabled: bool) -> Result<(), TauError> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| TauError { code: ErrorCode::Io, message: error.to_string() })?;
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(MANUAL_PLAYERS_FILE);
    let mut ids: Vec<String> = match std::fs::read_to_string(&file) {
        Ok(contents) => contents.lines().map(str::to_string).collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(TauError::from(error)),
    };
    ids.retain(|existing| existing != &core_id);
    if enabled {
        ids.push(core_id);
    }
    std::fs::write(file, ids.join("\n"))?;
    Ok(())
}

#[tauri::command(async)]
fn scan_media(path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<MediaScanView, TauError> {
    let scan = with_job(&window, &jobs, job_id, |progress| {
        tau_core::scan_dir_with_progress(Path::new(&path), true, progress)
    })?;
    let playlists = scan
        .playlists
        .into_iter()
        .map(|playlist| PlaylistDetailView {
            name: playlist.name,
            file: playlist.file,
            tracks: playlist
                .rel_ids
                .iter()
                .filter_map(|&id| scan.entries.get(id).map(|entry| entry.rel.clone()))
                .collect(),
        })
        .collect();
    Ok(MediaScanView { playlists, warnings: scan.warnings })
}

fn make_playlist_write_plan(
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
fn plan_playlist_write(
    path: String,
    file: String,
    tracks: Vec<String>,
) -> Result<tau_core::playlist::PlaylistPlan, TauError> {
    make_playlist_write_plan(&path, &file, &tracks)
}

#[tauri::command(async)]
fn execute_playlist_write(app: tauri::AppHandle, 
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
fn plan_playlist_rename(
    path: String,
    old_file: String,
    new_file: String,
) -> Result<tau_core::playlist::PlaylistPlan, TauError> {
    tau_core::playlist::plan_rename(Path::new(&path), &old_file, &new_file)
}

#[tauri::command(async)]
fn execute_playlist_rename(app: tauri::AppHandle, 
    path: String,
    old_file: String,
    new_file: String,
    confirmation: String,
) -> Result<(), TauError> {
    let _write = begin_card_write(&app, &path)?;
    let plan = tau_core::playlist::plan_rename(Path::new(&path), &old_file, &new_file)?;
    tau_core::playlist::execute(Path::new(&path), &plan, &confirmation)
}

fn make_playlist_import_plan(
    path: &str,
    source: &str,
    dest_file: &str,
) -> Result<tau_core::playlist::PlaylistPlan, TauError> {
    let common = Path::new(path);
    let scan = tau_core::scan_dir(common, false)?;
    tau_core::playlist::plan_import(common, Path::new(source), dest_file, &scan.entries)
}

#[tauri::command(async)]
fn plan_playlist_import(
    path: String,
    source: String,
    dest_file: String,
) -> Result<tau_core::playlist::PlaylistPlan, TauError> {
    make_playlist_import_plan(&path, &source, &dest_file)
}

#[tauri::command(async)]
fn execute_playlist_import(app: tauri::AppHandle, 
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
fn export_playlist(media_root: String, playlist_name: String, output: String) -> Result<(), TauError> {
    let scan = tau_core::scan_dir(Path::new(&media_root), true)?;
    let playlist = scan
        .playlists
        .iter()
        .find(|playlist| playlist.name == playlist_name)
        .ok_or(TauError { code: ErrorCode::NotFound, message: "playlist not found".into() })?;
    tau_core::playlist::export_m3u(Path::new(&output), playlist, &scan.entries)
}

#[tauri::command(async)]
fn find_problems(path: String) -> Result<Vec<tau_core::problems::Problem>, TauError> {
    let scan = tau_core::scan_dir(Path::new(&path), false)?;
    tau_core::problems::find_problems(&path, &scan.entries)
}

#[tauri::command(async)]
fn read_persisted_settings(path: String) -> Result<Vec<tau_core::diag::PersistedSetting>, TauError> {
    tau_core::diag::read_persisted_settings(path)
}

/// `None` means these persist ids don't currently hold a Check summary --
/// the normal case (no Check has been run, or they're this core's legacy
/// playlist state, per `docs/FIRMWARE_SYNC.md`'s overloaded-ids trap), not
/// an error the front-end needs to display as one.
#[tauri::command(async)]
fn read_check_summary(path: String) -> Result<Option<tau_core::diag::CheckSummary>, TauError> {
    match tau_core::diag::read_check_summary(path) {
        Ok(summary) => Ok(Some(summary)),
        Err(error) if error.code() == ErrorCode::NotACheckSummary => Ok(None),
        Err(error) => Err(error),
    }
}

/// Decodes a screenshot's Check QR page into the full TAUD1 report. Unlike
/// `read_check_summary` (the tiny 4-word persisted summary), this needs a
/// real screenshot PNG, not a card path -- `None` when the image simply has
/// no QR code in it (a normal screenshot of something else), an error for
/// anything that looks like a QR but fails to decode as a valid report.
#[tauri::command(async)]
fn read_qr_report(path: String) -> Result<Option<tau_core::taud::TaudReport>, TauError> {
    match tau_core::taud::read_qr_report(Path::new(&path)) {
        Ok(report) => Ok(Some(report)),
        Err(error) if error.code() == ErrorCode::NoQrCodeFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Lists every screenshot the Pocket has saved on this card
/// (`Memories/Screenshots/`), newest first -- the "find it automatically"
/// complement to `read_qr_report`, which decodes one the caller already has
/// a path for.
#[tauri::command(async)]
fn list_screenshots(card: String) -> Result<Vec<tau_core::screenshots::ScreenshotEntry>, TauError> {
    tau_core::screenshots::list_screenshots(Path::new(&card))
}

/// Reads one screenshot's raw bytes and returns them as a `data:` URL, so
/// the UI can show the actual image inline (a real preview, not just its
/// filename) without granting the webview broader filesystem access via
/// Tauri's asset protocol -- this only ever serves a path `list_screenshots`
/// or the user's own file picker already produced.
#[tauri::command(async)]
fn read_image_data_url(path: String) -> Result<String, TauError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = std::fs::read(&path)?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

/// Appearance: checks a theme (names, ranges, contrast on every surface) and shows each colour as the Pocket will
/// show it. Writes nothing.
#[tauri::command(async)]
fn appearance_check(theme: tau_core::assets::ThemeInput) -> tau_core::assets::ThemeReport {
    tau_core::assets::check_theme(&theme)
}

/// Appearance: opens a `tau-assets.bin` for editing. Every checksum is verified before anything is shown.
#[tauri::command(async)]
fn appearance_open(path: String) -> Result<Vec<tau_core::assets::ThemeInput>, TauError> {
    tau_core::assets::parse_assets(&std::fs::read(&path)?)
}

/// Appearance: writes `tau-assets.bin` to a file the user chose (not to the card), then reads it back and checks it
/// parses to the same themes. Every non-theme section (meter presets, EQ presets) is kept: from `keep_from` (the file
/// the themes were opened from) when given, else from the file being overwritten. Returns the file's size in bytes.
#[tauri::command(async)]
fn appearance_export(themes: Vec<tau_core::assets::ThemeInput>, path: String, keep_from: Option<String>) -> Result<u64, TauError> {
    let source = match &keep_from {
        Some(k) => Some(std::fs::read(k)?),
        None => std::fs::read(&path).ok(),
    };
    let bytes = tau_core::assets::pack_assets_keeping(&themes, source.as_deref())?;
    std::fs::write(&path, &bytes)?;
    let written = std::fs::read(&path)?;
    let back = tau_core::assets::parse_assets(&written)?;
    let kept = source.as_deref().map(tau_core::assets::kept_sections).unwrap_or_default();
    if written != bytes || tau_core::assets::pack_assets_keeping(&back, source.as_deref())? != bytes || tau_core::assets::kept_sections(&written) != kept {
        return Err(TauError { code: ErrorCode::VerificationFailed, message: "The saved file did not read back the same, so it should not be used.".into() });
    }
    Ok(bytes.len() as u64)
}

/// Appearance: plans putting these themes on a card at `<media root>/tau-assets.bin`. Writes nothing.
#[tauri::command(async)]
fn appearance_plan_install(
    themes: Vec<tau_core::assets::ThemeInput>,
    media_root: String,
) -> Result<tau_core::assets::AssetsInstallPlan, TauError> {
    tau_core::assets::plan_install(&themes, Path::new(&media_root))
}

/// Appearance: writes a reviewed theme-file install. Takes the card write lock, backs up any file it replaces
/// (when `backup` is given), verifies by reading back from the device, and swaps the file in recoverably.
#[tauri::command(async)]
fn appearance_install(app: tauri::AppHandle, 
    themes: Vec<tau_core::assets::ThemeInput>,
    media_root: String,
    confirmation: String,
    backup: Option<String>,
) -> Result<tau_core::assets::AssetsInstallReport, TauError> {
    let _write = begin_card_write(&app, &media_root)?;
    let root = Path::new(&media_root);
    let plan = tau_core::assets::plan_install(&themes, root)?;
    tau_core::assets::execute_install(&themes, root, &plan, &confirmation, backup.as_deref().map(Path::new))
}

/// Send diagnostics: reads what a Check run left on a card (saved summaries, Check QR screenshots, core file
/// checksums). Read-only; also returns the readable summary for "Copy summary".
#[derive(Serialize)]
struct DiagView {
    reading: tau_core::diagnostics::DiagReading,
    summary: String,
}

#[tauri::command(async)]
fn diag_read(card: String) -> Result<DiagView, TauError> {
    let reading = tau_core::diagnostics::read(Path::new(&card), 0)?;
    let summary = tau_core::diagnostics::summary_markdown(&reading);
    Ok(DiagView { reading, summary })
}

/// Send diagnostics: writes the zip into a folder outside the card. Nothing is uploaded and nothing is written to the card.
#[tauri::command(async)]
fn diag_zip(card: String, dest_dir: String) -> Result<String, TauError> {
    let reading = tau_core::diagnostics::read(Path::new(&card), 0)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = tau_core::diagnostics::create_zip(&reading, Path::new(&card), Path::new(&dest_dir), now)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Previews what `art_sidecar_pal256` will encode for one album's cover
/// (`cover_source`, from a `plan_sync` result's `art_sidecar_previews`),
/// without writing anything: runs the exact same quantizer `execute_sync`
/// will use, so what's shown here matches what actually ends up on the card.
#[tauri::command(async)]
fn preview_art_sidecar(cover_source: String) -> Result<String, TauError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = std::fs::read(&cover_source)?;
    let png = tau_core::image::preview_pal256_png(&bytes, 128)?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

/// Decodes a core's `icon.bin` (`Cores/<core_id>/icon.bin`) into a PNG data
/// URL. `None`, not an error, when the file simply doesn't exist -- not
/// every core ships one.
#[tauri::command(async)]
fn read_core_icon(card: String, core_id: String) -> Result<Option<String>, TauError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let path = Path::new(&card).join("Cores").join(&core_id).join("icon.bin");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(TauError::from(error)),
    };
    let png_bytes = tau_core::icon::decode_icon_bin(&bytes)?;
    Ok(Some(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(png_bytes)
    )))
}

/// Decodes a platform's banner (`Platforms/_images/<platform>.bin`) into a
/// PNG data URL -- the real per-platform artwork (521x165, shared by every
/// core on that platform), distinct from a core's own small `icon.bin`.
/// `None`, not an error, when the file doesn't exist.
#[tauri::command(async)]
fn read_platform_image(card: String, platform: String) -> Result<Option<String>, TauError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let path = Path::new(&card)
        .join("Platforms")
        .join("_images")
        .join(format!("{platform}.bin"));
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(TauError::from(error)),
    };
    let png_bytes = tau_core::icon::decode_platform_image(&bytes)?;
    Ok(Some(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(png_bytes)
    )))
}

#[tauri::command(async)]
fn inspect_card(path: String) -> Result<Vec<CoreView>, TauError> {
    Ok(tau_core::inspect_card(path)?.cores.into_iter().map(|core| {
        let (index_status, tracks) = match core.index_status {
            IndexStatus::NoIndex => ("No index".to_string(), None),
            IndexStatus::Ready { tracks } => ("Index ready".to_string(), Some(tracks as usize)),
            IndexStatus::NeedsRepair => ("Index needs repair".to_string(), None),
        };
        CoreView { id: core.id, author: core.author, shortname: core.shortname, version: core.version, platform: core.platform, platforms: core.platforms, media_platform: core.media_platform, platform_category: core.platform_category, library_capable: core.library_capable, index_status, tracks }
    }).collect())
}

#[tauri::command(async)]
fn compare_media(left: String, right: String) -> Result<ComparisonView, TauError> {
    let comparison = tau_core::compare::media_roots(Path::new(&left), Path::new(&right))?;
    let only_left = comparison.count(tau_core::compare::DifferenceState::OnlyLeft);
    let only_right = comparison.count(tau_core::compare::DifferenceState::OnlyRight);
    let different = comparison.count(tau_core::compare::DifferenceState::Different);
    let identical = comparison.count(tau_core::compare::DifferenceState::Identical);
    Ok(ComparisonView { comparison, only_left, only_right, different, identical })
}

fn make_plan(sources: Vec<String>, destination: String, embed_covers: bool, art_sidecar_pal256: bool) -> Result<tau_core::sync::SyncPlan, TauError> {
    let destination = PathBuf::from(destination);
    let source_paths = sources.into_iter().filter(|source| !source.trim().is_empty()).map(PathBuf::from).collect::<Vec<_>>();
    let root_prefix = tau_core::root_prefix(&destination)?;
    tau_core::sync::plan(
        &source_paths,
        &destination,
        &root_prefix,
        tau_core::sync::PlanOptions { mirror: false, embed_covers, art_sidecar_pal256 },
        &mut None,
    )
}
fn make_core_copy_plan(source: String, destination: String) -> Result<tau_core::sync::SyncPlan, TauError> {
    let destination_path = PathBuf::from(&destination);
    let root_prefix = tau_core::root_prefix(&destination_path)?;
    tau_core::sync::plan_core_copy(Path::new(&source), &destination_path, &root_prefix, &mut None)
}

#[tauri::command(async)]
fn plan_sync(sources: Vec<String>, destination: String, embed_covers: bool, art_sidecar_pal256: bool) -> Result<SyncPlanView, TauError> {
    let plan = make_plan(sources, destination, embed_covers, art_sidecar_pal256)?;
    Ok(sync_plan_view(plan))
}

/// Shared by `plan_sync`/`plan_core_copy` so both surface the same counts and
/// the same `art_sidecar_previews` list, computed once instead of twice.
fn sync_plan_view(plan: tau_core::sync::SyncPlan) -> SyncPlanView {
    let art_sidecar_previews = plan
        .art_sidecars
        .iter()
        .map(|item| ArtSidecarPreviewView {
            folder: item
                .source_folder
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| item.source_folder.to_string_lossy().into_owned()),
            cover_source: item.cover_source.to_string_lossy().into_owned(),
        })
        .collect();
    SyncPlanView { id: plan.id, new_files: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::New).count(), updates: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Update).count(), unchanged: plan.items.iter().filter(|item| item.state == tau_core::sync::CopyState::Same).count(), bytes_to_write: plan.bytes_to_write, art_sidecars: plan.art_sidecars.len(), art_sidecar_previews, warnings: plan.warnings }
}

#[tauri::command(async)]
fn plan_core_copy(source: String, destination: String) -> Result<SyncPlanView, TauError> {
    let plan = make_core_copy_plan(source, destination)?;
    Ok(sync_plan_view(plan))
}

/// Whether a plan's `bytes_needed` fits at `path`'s volume, with the
/// engine's own default safety margin. Read-only: this never reserves
/// space, so an executing plan still handles a full-disk failure itself.
#[tauri::command(async)]
fn check_storage_capacity(path: String, bytes_needed: u64) -> Result<tau_core::storage::CapacityCheck, TauError> {
    tau_core::storage::check_capacity(Path::new(&path), bytes_needed, tau_core::storage::DEFAULT_MARGIN_BYTES)
}

/// Plans backing up an arbitrary folder onto another -- a dry-run preview
/// only (`STATUS_HANDOFF.md` item 5); there is deliberately no
/// `execute_backup` yet.
#[tauri::command(async)]
fn plan_backup(source: String, destination: String) -> Result<tau_core::backup::BackupPlan, TauError> {
    tau_core::backup::plan(Path::new(&source), Path::new(&destination))
}

#[tauri::command(async)]
fn inspect_package(path: String) -> Result<tau_core::package::PackageManifest, TauError> {
    tau_core::package::inspect(Path::new(&path))
}

#[tauri::command(async)]
fn plan_package_install(path: String, card: String) -> Result<tau_core::package::PackagePlan, TauError> {
    tau_core::package::plan_install(Path::new(&path), Path::new(&card))
}

#[tauri::command(async)]
fn execute_package_install(app: tauri::AppHandle, 
    path: String,
    card: String,
    confirmation: String,
) -> Result<tau_core::package::PackageReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let zip_path = Path::new(&path);
    let card_root = Path::new(&card);
    let plan = tau_core::package::plan_install(zip_path, card_root)?;
    tau_core::package::execute_install(zip_path, card_root, &plan, &confirmation)
}

/// Plans removing an installed core; re-inspects the card so the plan always
/// reflects every currently-installed core, not a snapshot the caller might
/// be holding stale (the same "safe to re-derive, cheap to re-check" choice
/// `execute_package_install` already makes for its own plan).
#[tauri::command(async)]
fn plan_remove_core(card: String, core_id: String) -> Result<tau_core::remove::RemovePlan, TauError> {
    let card = tau_core::inspect_card(Path::new(&card))?;
    tau_core::remove::plan_remove(&card, &core_id)
}

#[tauri::command(async)]
fn execute_remove_core(app: tauri::AppHandle, 
    card: String,
    core_id: String,
    confirmation: String,
) -> Result<tau_core::remove::RemoveReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let card = tau_core::inspect_card(Path::new(&card))?;
    let plan = tau_core::remove::plan_remove(&card, &core_id)?;
    tau_core::remove::execute_remove(&plan, &confirmation)
}

#[tauri::command(async)]
fn execute_sync(app: tauri::AppHandle, sources: Vec<String>, destination: String, confirmation: String, manifest_path: String, embed_covers: bool, art_sidecar_pal256: bool, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<tau_core::sync::SyncReport, TauError> {
    let _write = begin_card_write(&app, &destination)?;
    let plan = make_plan(sources, destination, embed_covers, art_sidecar_pal256)?;
    let manifest = PathBuf::from(manifest_path);
    with_job(&window, &jobs, job_id, |progress| {
        tau_core::journal::execute_to_journal(&plan, &confirmation, "sync", &manifest, progress)
    })
}

#[tauri::command(async)]
fn execute_core_copy(app: tauri::AppHandle, source: String, destination: String, confirmation: String, manifest_path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<tau_core::sync::SyncReport, TauError> {
    let _write = begin_card_write(&app, &destination)?;
    let plan = make_core_copy_plan(source, destination)?;
    with_job(&window, &jobs, job_id, |progress| {
        tau_core::journal::execute_to_journal(&plan, &confirmation, "core_copy", Path::new(&manifest_path), progress)
    })
}

#[tauri::command(async)]
fn execute_core_move(app: tauri::AppHandle, source: String, destination: String, confirmation: String, delete_confirmation: String, backup_path: String, manifest_path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<tau_core::sync::SyncReport, TauError> {
    let _write = begin_card_write(&app, &destination)?;
    let plan = make_core_copy_plan(source.clone(), destination)?;
    let source_path = PathBuf::from(&source);
    let source_root_prefix = tau_core::root_prefix(&source_path)?;
    with_job(&window, &jobs, job_id, |progress| {
        tau_core::journal::execute_core_move_to_journal(
            &plan, &confirmation, &delete_confirmation, &source_path, &source_root_prefix, Path::new(&backup_path), Path::new(&manifest_path), progress,
        )
    })
}

/// The workbench's album/track/playlist listing of a media root (a local
/// library folder or a card's media root).
#[tauri::command(async)]
fn list_library(path: String, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<tau_core::workbench::LibraryListing, TauError> {
    with_job(&window, &jobs, job_id, |progress| tau_core::workbench::list_library(Path::new(&path), progress))
}

/// What a reviewed change set will do, in counts (the full per-file plan
/// stays in the engine and is re-derived at execute time).
#[derive(Serialize)]
struct ChangePlanView {
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

fn change_plan_view(plan: &tau_core::changes::ChangePlan) -> ChangePlanView {
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
fn plan_changes(request: tau_core::changes::ChangeRequest, destination: String) -> Result<ChangePlanView, TauError> {
    let plan = tau_core::changes::plan_changes(Path::new(&destination), &request, &mut None)?;
    Ok(change_plan_view(&plan))
}

/// Small cover pictures for a batch of album folders under `path` (a local
/// library or a card's media root). Albums with no readable picture come back
/// with `png_base64: null` so the UI can show a placeholder.
#[tauri::command(async)]
fn album_thumbnails(path: String, ids: Vec<String>) -> Result<Vec<tau_core::workbench::Thumbnail>, TauError> {
    tau_core::workbench::album_thumbnails(Path::new(&path), &ids, 96)
}

/// A proper thumbnail (decoded, resized, re-encoded as PNG) of an image file the
/// user picked, as a data URL; also proves the picture can be read at all.
#[tauri::command(async)]
fn image_thumbnail(path: String, long_side: u16) -> Result<String, TauError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = std::fs::read(&path)?;
    let png = tau_core::image::thumbnail_png(&bytes, long_side.clamp(16, 512))?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

/// The result of applying a change set: the engine's report plus where its
/// journal was written, so the UI can open that entry in the sync history.
#[derive(Serialize)]
struct ChangeResult {
    #[serde(flatten)]
    report: tau_core::changes::ChangeReport,
    journal: String,
}

/// Best-effort retention: delete history beyond the user's limits. Never fails
/// the caller (a full disk or a permissions problem must not turn a good sync
/// into an error).
fn prune_history_best_effort(app: &tauri::AppHandle) {
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
fn execute_changes(app: tauri::AppHandle, request: tau_core::changes::ChangeRequest, destination: String, confirmation: String, backup: Option<String>, context: Option<Value>, job_id: String, window: Window, jobs: State<JobRegistry>) -> Result<ChangeResult, TauError> {
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
fn list_history(app: tauri::AppHandle) -> Result<Vec<tau_core::journal::JournalSummary>, TauError> {
    let dir = resolved_reports_dir(&app)?;
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    tau_core::journal::list_journals(dir)
}

/// Applies the retention preferences now (also done after every sync).
#[tauri::command(async)]
fn prune_history(app: tauri::AppHandle) -> Result<usize, TauError> {
    let prefs = read_prefs(&app)?;
    let dir = resolved_reports_dir(&app)?;
    if !dir.is_dir() {
        return Ok(0);
    }
    tau_core::journal::prune_journals(dir, prefs.history_keep_last, prefs.history_keep_days)
}

/// Deletes the whole sync history (the user's explicit "clear history").
#[tauri::command(async)]
fn clear_history(app: tauri::AppHandle) -> Result<usize, TauError> {
    let dir = resolved_reports_dir(&app)?;
    if !dir.is_dir() {
        return Ok(0);
    }
    tau_core::journal::clear_journals(dir)
}

fn main() {
    cacheflush::register();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(JobRegistry::default())
        .setup(|app| {
            // What the app remembers about each card lives in its own cache folder, never on the card.
            if let Ok(dir) = app.path().app_cache_dir() {
                ledger_host::init(dir);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![inspect_card, scan_library, scan_media, export_playlist, find_problems, compare_media, read_journal, list_journals, get_reports_dir, set_reports_dir, get_recent_cards, record_recent_card, list_mounted_cards, get_manual_players, set_manual_player, read_persisted_settings, read_check_summary, plan_sync, plan_core_copy, execute_sync, execute_core_copy, execute_core_move, plan_playlist_write, execute_playlist_write, plan_playlist_rename, execute_playlist_rename, plan_playlist_import, execute_playlist_import, check_storage_capacity, plan_backup, inspect_package, plan_package_install, execute_package_install, plan_remove_core, execute_remove_core, read_qr_report, list_screenshots, read_image_data_url, read_core_icon, read_platform_image, preview_art_sidecar, cancel_job, detect_connection, eject_card, readback_status, card_breakdown, ledger_forget, appearance_check, appearance_open, appearance_export, appearance_plan_install, appearance_install, diag_read, diag_zip, get_prefs, set_prefs, update_check, update_download, card_marker_status, set_card_marker, remove_card_marker, plan_core_update, execute_core_update, rollback_core_update, library_health, plan_library_refresh, execute_library_refresh, rollback_library_refresh, list_library, plan_changes, execute_changes, list_history, prune_history, clear_history, album_thumbnails, image_thumbnail])
        .run(tauri::generate_context!())
        .expect("Tau Omega failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The UI reads `PrefsView` (prefs plus resolved folders) and sends the
    /// whole object back to `set_prefs`; the extra fields must be ignored and
    /// a missing field must fall back to its default.
    #[test]
    fn prefs_round_trip_through_the_view_the_ui_sends_back() {
        let view = PrefsView {
            prefs: Prefs { remove_mode: "ask".into(), ..Prefs::default() },
            default_backup_dir: "/data/removed-backups".into(),
            reports_dir: "/data/reports".into(),
        };
        let json = serde_json::to_string(&view).unwrap();
        assert!(json.contains("\"remove_mode\":\"ask\"") && json.contains("\"default_backup_dir\""));
        let back: Prefs = serde_json::from_str(&json).unwrap();
        assert_eq!(back.remove_mode, "ask");
        let sparse: Prefs = serde_json::from_str(r#"{"slow_alert_suppressed": true}"#).unwrap();
        assert_eq!(sparse.remove_mode, "backup");
        assert!(sparse.slow_alert_suppressed && !sparse.remove_explained);
        assert_eq!((sparse.history_keep_last, sparse.history_keep_days), (100, 365));
    }
}
