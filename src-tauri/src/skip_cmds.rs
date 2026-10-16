//! Tauri commands for the skip-silence list and editor (`docs/SKIP_SILENCE.md`): the tracks with what would be skipped, one
//! track's waveform, and a hand edit (typed values or "never skip"). A hand edit is kept in the app's preferences per media
//! root and written, with the rest of the folder's table, into `tau-skip.bin` on the card.

use super::{PREFS_FILE, begin_card_write, config_dir, read_prefs};
use std::path::Path;
use tau_core::{
    ErrorCode, TauError,
    trim::{self, Override, SkipDetail, SkipTrack},
};

fn err(code: ErrorCode, m: impl Into<String>) -> TauError {
    TauError { code, message: m.into() }
}

/// Every track under the media root with its skip. Needs skip silence to be on: otherwise nothing is measured.
#[tauri::command(async)]
pub fn skip_tracks(app: tauri::AppHandle, media_root: String) -> Result<Vec<SkipTrack>, TauError> {
    read_prefs(&app)?;
    trim::skip_tracks(Path::new(&media_root), &trim::active())
}

/// One track decoded: its waveform and what the detector finds.
#[tauri::command(async)]
pub fn skip_detail(app: tauri::AppHandle, media_root: String, rel: String) -> Result<SkipDetail, TauError> {
    read_prefs(&app)?;
    trim::skip_detail(Path::new(&media_root), &rel, &trim::active())
}

/// Keeps (or, with `None`, forgets) a hand edit and rewrites the folder tables on the card.
#[tauri::command(async)]
pub fn skip_set_edit(
    app: tauri::AppHandle,
    card: String,
    media_root: String,
    rel: String,
    edit: Option<Override>,
) -> Result<(), TauError> {
    let settings = {
        read_prefs(&app)?;
        trim::active()
    };
    if !settings.enabled {
        return Err(err(ErrorCode::InvalidPathReference, "Turn on Skip silence in Settings first."));
    }
    if rel.split('/').any(|c| c == ".." || c.is_empty()) {
        return Err(err(ErrorCode::InvalidPathReference, "not a track path"));
    }
    let _write = begin_card_write(&app, &card)?;
    let mut prefs = read_prefs(&app)?;
    let map = prefs.skip_edits.entry(media_root.clone()).or_default();
    match edit.filter(|e| *e != Override::default()) {
        Some(e) => {
            map.insert(rel, e);
        }
        None => {
            map.remove(&rel);
        }
    }
    let now = map.clone();
    let dir = config_dir(&app)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join(PREFS_FILE),
        serde_json::to_vec_pretty(&prefs).map_err(|e| err(ErrorCode::Json, e.to_string()))?,
    )?;
    trim::set_overrides(Path::new(&media_root), now);
    trim::rewrite_skip_files(Path::new(&media_root), &settings)?;
    Ok(())
}
