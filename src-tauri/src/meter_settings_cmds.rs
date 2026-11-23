//! Tauri commands for the per-meter settings file (`Saves/<platform>/<Tau core>/tau-meter-settings.bin`). Thin: every judgement is in
//! `tau_core::meter_settings`. The Pocket makes and writes this file; Omega only **looks** for one of the wrong size (which stops the core at
//! start, Tau Alpha 032) and, after a reviewed plan and a verified backup outside the card, deletes exactly that file. It never creates, edits or
//! overwrites a settings file.

use std::path::Path;
use tau_core::{
    TauError,
    meter_settings::{self, DeletePlan, DeleteReport, FoundFile},
};

/// The Tau cores' settings files on the card and what the Pocket will do with each. Reads only.
#[tauri::command(async)]
pub fn meter_settings_scan(card: String) -> Vec<FoundFile> {
    meter_settings::scan_card(Path::new(&card))
}

/// What deleting one wrong-sized settings file would do. Reads the card; deletes nothing. A file of the right size is blocked.
#[tauri::command(async)]
pub fn meter_settings_plan_delete(card: String, rel_path: String) -> Result<DeletePlan, TauError> {
    meter_settings::plan_delete(Path::new(&card), &rel_path)
}

/// Deletes the reviewed file: it is copied to the app's backup folder and verified before it is removed.
#[tauri::command(async)]
pub fn meter_settings_delete(app: tauri::AppHandle, card: String, rel_path: String, confirmation: String) -> Result<DeleteReport, TauError> {
    let _write = crate::begin_card_write(&app, &card)?;
    let backup = crate::install_backup_root(&app)?;
    meter_settings::execute_delete(Path::new(&card), &rel_path, &confirmation, &backup)
}
