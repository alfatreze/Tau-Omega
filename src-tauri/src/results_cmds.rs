//! Tauri commands for the Results page (`docs/DEV_LOOP_SPEC.md`, D3): decode the card's screenshots as Check reports, compare runs, and remember when
//! the last install was (per card, in the app's prefs) so "since the last install" has a baseline. Reads the card only.

use super::{PREFS_FILE, Prefs, config_dir, data_dir, read_prefs};
use serde::Serialize;
use std::path::Path;
use tau_core::{
    ErrorCode, TauError,
    core_package::Limits,
    core_preview,
    results::{self, ResultShot, Run},
    run_notes::{self, RunNote},
};

fn card_key(card: &str) -> String {
    Path::new(card).file_name().map_or_else(|| card.to_string(), |n| n.to_string_lossy().into_owned())
}

#[derive(Serialize)]
pub struct ResultsView {
    /// The baseline stamp (`YYYY-MM-DDTHH:MM:SS`, the host's local clock) or `None` for "everything".
    since: Option<String>,
    shots: Vec<ResultShot>,
    runs: Vec<Run>,
}

#[tauri::command(async)]
pub fn results_scan(app: tauri::AppHandle, card: String) -> Result<ResultsView, TauError> {
    let since = read_prefs(&app)?.last_install.get(&card_key(&card)).cloned();
    let shots = results::scan(Path::new(&card), since.as_deref())?;
    let runs = results::runs(&shots);
    Ok(ResultsView { since, shots, runs })
}

/// Sets (or clears, with `None`) the "last install" baseline for this card. The stamp comes from the UI's local clock in the Pocket's own shape.
#[tauri::command(async)]
pub fn set_last_install(app: tauri::AppHandle, card: String, stamp: Option<String>) -> Result<(), TauError> {
    if let Some(s) = &stamp {
        let ok = s.len() == 19 && s.as_bytes().get(10) == Some(&b'T') && s.chars().filter(|c| c.is_ascii_digit()).count() == 14;
        if !ok {
            return Err(TauError { code: ErrorCode::InvalidPathReference, message: "The time must look like 2026-10-14T10:15:00.".into() });
        }
    }
    let mut prefs: Prefs = read_prefs(&app)?;
    match stamp {
        Some(s) => {
            prefs.last_install.insert(card_key(&card), s);
        }
        None => {
            prefs.last_install.remove(&card_key(&card));
        }
    }
    let dir = config_dir(&app)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(PREFS_FILE), serde_json::to_vec_pretty(&prefs).map_err(|e| TauError { code: ErrorCode::Json, message: e.to_string() })?)?;
    Ok(())
}

// ---- run notes (D4) --------------------------------------------------------------------------------------------------

fn store(app: &tauri::AppHandle) -> Result<std::path::PathBuf, TauError> {
    Ok(data_dir(app)?.join("run-notes.json"))
}

#[tauri::command(async)]
pub fn run_notes_list(app: tauri::AppHandle, card: String) -> Result<Vec<RunNote>, TauError> {
    Ok(run_notes::list(&store(&app)?, &card_key(&card)))
}

/// Records an install as a run note (hashes of the package and its bitstreams, the backup, the time). Called when an install finishes with developer
/// tools on; `stamp` is the host's local clock in the Pocket's stamp shape.
#[tauri::command(async)]
pub fn run_note_record(app: tauri::AppHandle, card: String, zip: String, backup_dir: String, files_written: u64, stamp: String, lint_overrides: Vec<String>) -> Result<RunNote, TauError> {
    let zip_path = Path::new(&zip);
    let (package_sha256, bitstreams) = run_notes::package_facts(zip_path)?;
    let (core_id, shortname, version) = core_preview::preview_zip(zip_path, Limits::default()).map_or_else(
        |_| (String::new(), String::new(), String::new()),
        |p| (p.core_id, p.shortname, p.version),
    );
    let package = zip_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let note = RunNote {
        id: String::new(),
        card: card_key(&card),
        stamp,
        core_id,
        shortname,
        version,
        package,
        package_sha256,
        bitstreams,
        backup_dir,
        files_written,
        text: String::new(),
        lint_overrides,
        results: Vec::new(),
        results_at: None,
    };
    run_notes::add(&store(&app)?, note)
}

#[tauri::command(async)]
pub fn run_note_text(app: tauri::AppHandle, id: String, text: String) -> Result<RunNote, TauError> {
    run_notes::set_text(&store(&app)?, &id, &text)
}

#[tauri::command(async)]
pub fn run_note_delete(app: tauri::AppHandle, id: String) -> Result<(), TauError> {
    run_notes::delete(&store(&app)?, &id)
}

/// Stores what the Pocket reported since this install (until the next install on the same card) inside the note.
#[tauri::command(async)]
pub fn run_note_attach(app: tauri::AppHandle, card: String, id: String, now: String) -> Result<RunNote, TauError> {
    let path = store(&app)?;
    let notes = run_notes::list(&path, &card_key(&card));
    let me = notes.iter().find(|n| n.id == id).ok_or_else(|| TauError { code: ErrorCode::InvalidPathReference, message: "That run note no longer exists.".into() })?;
    // the next install on this card closes the window
    let until = notes.iter().filter(|n| n.stamp > me.stamp).map(|n| n.stamp.clone()).min();
    let mut shots = results::scan(Path::new(&card), Some(&me.stamp))?;
    shots.retain(|s| match (&s.captured_at, &until) {
        (Some(c), Some(u)) => c < u,
        _ => true,
    });
    let runs = results::runs(&shots);
    let snap = run_notes::snapshot(&runs, |r| {
        let shot = shots.iter().find(|s| s.filename == r.latest)?;
        let rep = shot.report.as_ref()?;
        let failing = rep.tests.iter().filter(|t| t.result == "FAIL").map(|t| t.name.clone()).collect();
        let (fw, bs) = rep.entries.build.as_ref().map_or_else(Default::default, |b| (b.firmware.clone(), b.bitstream.clone()));
        Some((shot.captured_at.clone().unwrap_or_default(), rep.verdict.clone(), failing, fw, bs))
    });
    run_notes::attach_results(&path, &id, snap, &now)
}

#[tauri::command(async)]
pub fn run_note_report(app: tauri::AppHandle, id: String) -> Result<String, TauError> {
    let all = run_notes::list_all(&store(&app)?);
    all.iter().find(|n| n.id == id).map(run_notes::markdown).ok_or_else(|| TauError { code: ErrorCode::InvalidPathReference, message: "That run note no longer exists.".into() })
}

/// Writes the report to a folder the user chose and returns the file's path (an existing name gets a number added).
#[tauri::command(async)]
pub fn run_note_export(app: tauri::AppHandle, id: String, dest_dir: String) -> Result<String, TauError> {
    let all = run_notes::list_all(&store(&app)?);
    let n = all.iter().find(|n| n.id == id).ok_or_else(|| TauError { code: ErrorCode::InvalidPathReference, message: "That run note no longer exists.".into() })?;
    let dir = Path::new(&dest_dir);
    if !dir.is_dir() {
        return Err(TauError { code: ErrorCode::InvalidPathReference, message: "Choose an existing folder.".into() });
    }
    let name = run_notes::report_file_name(n);
    let mut target = dir.join(&name);
    let mut k = 2;
    while target.exists() {
        target = dir.join(name.replace(".md", &format!("-{k}.md")));
        k += 1;
    }
    std::fs::write(&target, run_notes::markdown(n))?;
    Ok(target.to_string_lossy().into_owned())
}
