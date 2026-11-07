//! Tauri commands for meter presets as files on the card (`tau_core::preset_files`, `preset_install`): read what the card holds
//! next to the registry's factory sets, plan a write, write it. The registry (`lab-data.json` of the pinned lab drop) is passed in
//! by the page that loaded it, so the page and the engine always agree on parameters, ranges and defaults.

use crate::{ErrorCode, TauError, begin_card_write, data_dir};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use tau_core::{
    meter_presets::{MeterRegistry, RegMeter},
    preset_files::{self, PresetSet},
    preset_install::{self, Folder, PresetInstallPlan, PresetInstallReport},
};

const RETIRED_FILE: &str = "meter_preset_ids.json";

fn bad(message: impl Into<String>) -> TauError {
    TauError {
        code: ErrorCode::InvalidPathReference,
        message: message.into(),
    }
}

/// Ids that were once used by a preset that is now deleted, per meter. The Pocket remembers a chosen preset by id, and a saved
/// tuning is anchored to it, so an id is never given to a different preset.
fn retired_path(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    Ok(data_dir(app)?.join(RETIRED_FILE))
}
fn read_retired(app: &tauri::AppHandle) -> BTreeMap<String, BTreeSet<u8>> {
    retired_path(app)
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn write_retired(app: &tauri::AppHandle, map: &BTreeMap<String, BTreeSet<u8>>) {
    if let Ok(path) = retired_path(app) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(map) {
            let _ = std::fs::write(path, bytes);
        }
    }
}

fn registry(json: &str) -> Result<MeterRegistry, TauError> {
    MeterRegistry::from_json(json.as_bytes())
}

fn meter_of(reg: &MeterRegistry, id: u16) -> Result<&RegMeter, TauError> {
    reg.meters
        .iter()
        .find(|m| u16::from(m.id) == id)
        .ok_or_else(|| bad(format!("the registry has no meter with id {id}")))
}

fn desired(reg: &MeterRegistry, sets: &[PresetSet]) -> Result<BTreeMap<u16, Folder>, TauError> {
    let mut out = BTreeMap::new();
    for set in sets {
        let specs = preset_files::param_specs(meter_of(reg, set.meter_id)?)?;
        if out.insert(set.meter_id, set.folder(&specs)?).is_some() {
            return Err(bad(format!("meter {} appears twice", set.meter_id)));
        }
    }
    Ok(out)
}

/// One meter as the Presets page shows it.
#[derive(Serialize)]
pub struct MeterPresetsView {
    pub meter_id: u16,
    pub key: String,
    pub name: String,
    /// The registry's factory presets, ids are the positions (what `install_factory_presets.py` and the release write).
    pub factory: PresetSet,
    /// What the card holds for this meter; `None` when there is no folder (or it cannot be read: see `problem`).
    pub on_card: Option<PresetSet>,
    /// A folder on the card that Omega could not read, in plain words. The Pocket ignores it; saving replaces it (after a backup).
    pub problem: Option<String>,
    /// Ids that must not be given to another preset.
    pub retired: Vec<u8>,
}

/// Meter presets: every meter of the registry that has parameters, with the card's presets and the factory ones. Reads only.
#[tauri::command(async)]
pub fn meter_presets_read(
    app: tauri::AppHandle,
    registry_json: String,
    media_root: String,
) -> Result<Vec<MeterPresetsView>, TauError> {
    let reg = registry(&registry_json)?;
    let retired = read_retired(&app);
    let root = Path::new(&media_root);
    let mut out = Vec::new();
    for m in reg.meters.iter().filter(|m| !m.params.is_empty()) {
        let id = u16::from(m.id);
        let specs = preset_files::param_specs(m)?;
        let files = preset_install::read_folder(root, id);
        let (on_card, problem) = if files.is_empty() {
            (None, None)
        } else {
            match PresetSet::from_files(id, &files, &specs) {
                Ok(set) => (Some(set), None),
                Err(e) => (None, Some(e.message)),
            }
        };
        out.push(MeterPresetsView {
            meter_id: id,
            key: m.key.clone(),
            name: m.name.clone(),
            factory: preset_files::factory_from_registry(m)?,
            on_card,
            problem,
            retired: retired
                .get(&id.to_string())
                .map(|s| s.iter().copied().collect())
                .unwrap_or_default(),
        });
    }
    Ok(out)
}

/// Meter presets: checks every rule over the sets and lists what writing them to the card would do. Writes nothing.
#[tauri::command(async)]
pub fn meter_presets_plan(
    registry_json: String,
    media_root: String,
    sets: Vec<PresetSet>,
) -> Result<PresetInstallPlan, TauError> {
    let reg = registry(&registry_json)?;
    preset_install::plan(Path::new(&media_root), &desired(&reg, &sets)?)
}

/// Meter presets: writes a reviewed plan (card write lock, backup outside the card, read-back, undo on failure).
#[tauri::command(async)]
pub fn meter_presets_install(
    app: tauri::AppHandle,
    registry_json: String,
    media_root: String,
    sets: Vec<PresetSet>,
    confirmation: String,
    backup: Option<String>,
) -> Result<PresetInstallReport, TauError> {
    let _write = begin_card_write(&app, &media_root)?;
    let reg = registry(&registry_json)?;
    let want = desired(&reg, &sets)?;
    let root = Path::new(&media_root);
    let plan = preset_install::plan(root, &want)?;
    let report = preset_install::execute(
        root,
        &want,
        &plan,
        &confirmation,
        backup.as_deref().map(Path::new),
    )?;
    // The ids of the presets this write removed are never given out again.
    let mut retired = read_retired(&app);
    for (meter, name) in &plan.removals {
        if let Some(id) = name
            .strip_prefix('p')
            .and_then(|n| n.strip_suffix(".tpr"))
            .and_then(|n| n.parse::<u8>().ok())
        {
            retired.entry(meter.to_string()).or_default().insert(id);
        }
    }
    write_retired(&app, &retired);
    Ok(report)
}
