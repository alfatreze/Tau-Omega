//! Tauri commands for the Check page (`docs/DEV_LOOP_SPEC.md`): the core checker on a package zip, a package or core folder, an installed core (with the
//! "why is it not in the menu" checks) and every core on the card. Reads only.

use serde::Serialize;
use std::path::Path;
use tau_core::{
    ErrorCode, TauError,
    core_lint::{self, Finding, LintReport},
    menu_symptoms,
};

#[derive(Serialize)]
pub struct CheckResult {
    title: String,
    lint: LintReport,
    /// Why an installed core might be missing from the menu (installed cores only).
    symptoms: Vec<Finding>,
}

/// Checks a package zip, or a package or bare core folder (packed to a temporary zip first).
#[tauri::command(async)]
pub fn check_source(app: tauri::AppHandle, path: String) -> Result<CheckResult, TauError> {
    let p = Path::new(&path);
    let lint = if p.is_dir() {
        let tmp = tauri::Manager::path(&app).app_cache_dir().map_err(|e| TauError { code: ErrorCode::Io, message: e.to_string() })?.join("check");
        std::fs::create_dir_all(&tmp)?;
        core_lint::lint_folder(p, &tmp)?
    } else {
        core_lint::lint_zip(p)?
    };
    let title = p.file_name().map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());
    Ok(CheckResult { title, lint, symptoms: Vec::new() })
}

#[tauri::command(async)]
pub fn check_installed(card: String, core_id: String) -> Result<CheckResult, TauError> {
    let card = Path::new(&card);
    // a core that is not under Cores (disabled, or not installed) has no lint, only the symptom that says so
    let lint = core_lint::lint_card_core(card, &core_id).unwrap_or_default();
    Ok(CheckResult { title: core_id.clone(), lint, symptoms: menu_symptoms::symptoms(card, &core_id) })
}

#[derive(Serialize)]
pub struct CardCheckRow {
    id: String,
    shortname: String,
    errors: usize,
    warnings: usize,
    notes: usize,
}

/// The core check on every installed core, a line each.
#[tauri::command(async)]
pub fn check_card(card: String) -> Result<Vec<CardCheckRow>, TauError> {
    let root = Path::new(&card);
    let cores = tau_core::inspect_card(root)?.cores;
    Ok(cores
        .into_iter()
        .map(|c| {
            let l = core_lint::lint_card_core(root, &c.id).unwrap_or_default();
            CardCheckRow { id: c.id, shortname: c.shortname, errors: l.errors, warnings: l.warnings, notes: l.notes }
        })
        .collect())
}
