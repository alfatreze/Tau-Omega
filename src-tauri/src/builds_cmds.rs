//! Tauri commands for the Builds page (`docs/DEV_LOOP_SPEC.md`): the build folder and the developer's naming schemes (both in the app's prefs), the
//! scan of the folder against the open card, and the zip an installable folder is packed into. Nothing here writes to a card: installing goes
//! through the usual reviewed plan and removing older builds through the usual reviewed removal.

use super::{PREFS_FILE, Prefs, config_dir, read_prefs};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tau_core::{
    ErrorCode, TauError,
    build_series::{self, Field, Placement, Scheme, Subject},
    builds::{self, BuildEntry, Installed, Relation},
    core_package::Limits,
    core_pack,
};

fn err(code: ErrorCode, m: impl Into<String>) -> TauError {
    TauError { code, message: m.into() }
}

#[derive(Serialize)]
pub struct BuildsConfig {
    dir: Option<String>,
    builtin: Vec<Scheme>,
    custom: Vec<Scheme>,
}

#[tauri::command(async)]
pub fn builds_config(app: tauri::AppHandle) -> Result<BuildsConfig, TauError> {
    let p = read_prefs(&app)?;
    Ok(BuildsConfig { dir: p.builds_dir, builtin: build_series::builtin(), custom: p.build_schemes })
}

/// Saves the build folder and the developer's own schemes. A scheme with a bad template or no name is refused with the reason.
#[tauri::command(async)]
pub fn set_builds_config(app: tauri::AppHandle, dir: Option<String>, custom: Vec<Scheme>) -> Result<BuildsConfig, TauError> {
    for s in &custom {
        if s.name.trim().is_empty() {
            return Err(err(ErrorCode::InvalidPathReference, "Every scheme needs a name."));
        }
        build_series::validate(&s.template).map_err(|m| err(ErrorCode::InvalidPathReference, format!("{}: {m}", s.name)))?;
    }
    let mut prefs: Prefs = read_prefs(&app)?;
    prefs.builds_dir = dir.filter(|d| !d.trim().is_empty());
    prefs.build_schemes = custom;
    let cfg = config_dir(&app)?;
    std::fs::create_dir_all(&cfg)?;
    std::fs::write(cfg.join(PREFS_FILE), serde_json::to_vec_pretty(&prefs).map_err(|e| err(ErrorCode::Json, e.to_string()))?)?;
    builds_config(app)
}

#[derive(Serialize)]
pub struct SchemeTry {
    error: Option<String>,
    placement: Option<Placement>,
}

/// Tries a template on sample values so the developer sees what it reads before saving.
#[tauri::command(async)]
pub fn try_scheme(field: Field, template: String, core_id: String, shortname: String, version: String, file: String) -> SchemeTry {
    if let Err(e) = build_series::validate(&template) {
        return SchemeTry { error: Some(e), placement: None };
    }
    let scheme = Scheme { name: "try".into(), field, template };
    let placement = build_series::classify(&[scheme], &Subject { core_id: &core_id, shortname: &shortname, version: &version, file: &file });
    SchemeTry { error: None, placement }
}

#[derive(Serialize)]
pub struct BuildRow {
    entry: BuildEntry,
    relation: Relation,
    /// Installed cores of the same series that are older than this build.
    older: Vec<Installed>,
}

#[derive(Serialize)]
pub struct BuildList {
    dir: String,
    rows: Vec<BuildRow>,
}

fn installed(card: &str) -> Vec<Installed> {
    if card.trim().is_empty() {
        return Vec::new();
    }
    tau_core::inspect_card(Path::new(card))
        .map(|c| c.cores.into_iter().map(|c| Installed { id: c.id, shortname: c.shortname, version: c.version }).collect())
        .unwrap_or_default()
}

/// The builds in the saved folder, compared with the open card (empty `card` skips the comparison).
#[tauri::command(async)]
pub fn builds_list(app: tauri::AppHandle, card: String) -> Result<BuildList, TauError> {
    let p = read_prefs(&app)?;
    let dir = p.builds_dir.clone().ok_or_else(|| err(ErrorCode::InvalidPathReference, "Choose the folder your builds are in first."))?;
    let schemes = build_series::with_custom(p.build_schemes);
    let entries = builds::scan(Path::new(&dir), &schemes)?;
    let on_card = installed(&card);
    let rows = entries
        .into_iter()
        .map(|entry| {
            let relation = builds::relation(&entry, &on_card, &schemes);
            let older = builds::older_on_card(&entry, &on_card, &schemes);
            BuildRow { entry, relation, older }
        })
        .collect();
    Ok(BuildList { dir, rows })
}

/// The zip to install for a build: the zip itself, or a package folder packed into the app's cache.
#[tauri::command(async)]
pub fn build_zip(app: tauri::AppHandle, path: String) -> Result<String, TauError> {
    let p = PathBuf::from(&path);
    if p.is_file() {
        return Ok(path);
    }
    let out = tauri::Manager::path(&app).app_cache_dir().map_err(|e| err(ErrorCode::Io, e.to_string()))?.join("builds");
    std::fs::create_dir_all(&out)?;
    let packed = core_pack::pack_folder(&p, &out, Limits::default())?;
    Ok(packed.zip.to_string_lossy().into_owned())
}

/// The core checker's findings for a package zip (the gate before a developer install).
#[tauri::command(async)]
pub fn lint_package(zip: String) -> Result<tau_core::core_lint::LintReport, TauError> {
    tau_core::core_lint::lint_zip(Path::new(&zip))
}
