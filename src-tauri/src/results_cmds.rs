//! Tauri commands for the Results page (`docs/DEV_LOOP_SPEC.md`, D3): decode the card's screenshots as Check reports, compare runs, and remember when
//! the last install was (per card, in the app's prefs) so "since the last install" has a baseline. Reads the card only.

use super::{PREFS_FILE, Prefs, config_dir, read_prefs};
use serde::Serialize;
use std::path::Path;
use tau_core::{
    ErrorCode, TauError,
    results::{self, ResultShot, Run},
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
