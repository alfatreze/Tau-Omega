//! Tauri commands for hiding cores from the Pocket (`docs/CORE_DISABLE_SPEC.md`): which cores to suggest, what is disabled, and the plan then
//! confirmed move under the card write lock. The setting and the per-card "keep enabled" list live in the app's prefs.

use super::{Prefs, read_prefs, config_dir, PREFS_FILE};
use serde::Serialize;
use std::path::Path;
use tau_core::{
    ErrorCode, TauError,
    core_health::{CoreHealth, check_card, suggestions},
    core_state::{DisabledCore, StatePlan, StateReport},
};

/// A card is known by its volume name, like the other per-card prefs.
fn card_key(card: &str) -> String {
    Path::new(card).file_name().map_or_else(|| card.to_string(), |n| n.to_string_lossy().into_owned())
}

#[derive(Serialize)]
pub struct Suggestions {
    /// `"ask"`, `"auto"` or `"never"`.
    mode: String,
    cores: Vec<CoreHealth>,
}

/// The cores to offer disabling on this card (empty when the setting is "never"; cores marked keep-enabled are left out).
#[tauri::command(async)]
pub fn core_suggestions(app: tauri::AppHandle, card: String) -> Result<Suggestions, TauError> {
    let prefs = read_prefs(&app)?;
    if prefs.core_disable == "never" {
        return Ok(Suggestions { mode: prefs.core_disable, cores: Vec::new() });
    }
    let keep = prefs.core_keep.get(&card_key(&card)).cloned().unwrap_or_default();
    let health = check_card(Path::new(&card))?;
    Ok(Suggestions { mode: prefs.core_disable, cores: suggestions(&health, &keep) })
}

#[tauri::command(async)]
pub fn disabled_cores(card: String) -> Vec<DisabledCore> {
    tau_core::core_state::list_disabled(Path::new(&card))
}

#[tauri::command(async)]
pub fn plan_core_state(card: String, ids: Vec<String>, enable: bool) -> Result<StatePlan, TauError> {
    tau_core::core_state::plan(Path::new(&card), &ids, enable)
}

#[tauri::command(async)]
pub fn execute_core_state(
    app: tauri::AppHandle,
    card: String,
    ids: Vec<String>,
    enable: bool,
    confirmation: String,
    reasons: Vec<(String, Vec<String>)>,
) -> Result<StateReport, TauError> {
    let _write = crate::begin_card_write(&app, &card)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    tau_core::core_state::execute(Path::new(&card), &ids, enable, &confirmation, &reasons, now)
}

/// Adds (or removes) cores on this card's keep-enabled list: they are never suggested again.
#[tauri::command(async)]
pub fn core_keep(app: tauri::AppHandle, card: String, ids: Vec<String>, keep: bool) -> Result<(), TauError> {
    let mut prefs: Prefs = read_prefs(&app)?;
    let list = prefs.core_keep.entry(card_key(&card)).or_default();
    for id in ids {
        list.retain(|x| x != &id);
        if keep {
            list.push(id);
        }
    }
    if list.is_empty() {
        prefs.core_keep.remove(&card_key(&card));
    }
    let dir = config_dir(&app)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join(PREFS_FILE),
        serde_json::to_vec_pretty(&prefs).map_err(|e| TauError { code: ErrorCode::Json, message: e.to_string() })?,
    )?;
    Ok(())
}
