//! A core's saved settings, read from `Settings/<core>/{Interact,Input,Video}/_core/*_persist.json` and decoded against the core's
//! own `interact.json` (the persisted file holds ids and numbers; the names, list options and slider ranges are in the core).
//! Also counts the per-game presets under `Presets/<core>/`. Read-only.

use crate::{TauError, core_files, corehub};
use serde_json::Value;
use std::{fs, path::Path, time::UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SettingValue {
    pub id: i64,
    /// The menu entry's name from `interact.json`, or `Setting <id>` when the core no longer has it.
    pub name: String,
    /// `check`, `slider`, `list`, `radio`, or the raw type.
    pub kind: String,
    pub value: i64,
    /// What a person reads: On/Off, the list option's name, the number.
    pub display: String,
    pub default_display: String,
    pub known: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SavedFile {
    /// `Interact`, `Input` or `Video`.
    pub group: String,
    pub rel_path: String,
    pub bytes: u64,
    pub modified: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreSettings {
    pub values: Vec<SettingValue>,
    pub files: Vec<SavedFile>,
    pub presets: usize,
    pub presets_folder: String,
    /// The persisted interact file exists but cannot be read.
    pub unreadable: Option<String>,
}

fn json_at(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn display(var: Option<&Value>, kind: &str, value: i64) -> String {
    match kind {
        "check" => if value == 0 { "Off" } else { "On" }.to_string(),
        "list" | "radio" => var
            .and_then(|v| v.get("options"))
            .and_then(Value::as_array)
            .and_then(|o| {
                o.iter()
                    .find(|o| o.get("value").and_then(Value::as_i64) == Some(value))
            })
            .and_then(|o| o.get("name").and_then(Value::as_str))
            .map_or_else(|| value.to_string(), str::to_string),
        _ => value.to_string(),
    }
}

fn kind_of(t: &str) -> String {
    match t {
        "slider_u32" | "slider_s32" | "slider" => "slider".into(),
        other => other.to_string(),
    }
}

pub fn core_settings(card: &Path, core_id: &str) -> Result<CoreSettings, TauError> {
    let dir = corehub::core_dir(card, core_id)?;
    let interact = json_at(&dir.join("interact.json"));
    let variables: Vec<&Value> = interact
        .as_ref()
        .and_then(|j| j.pointer("/interact/variables"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect();
    let var = |id: i64| {
        variables
            .iter()
            .find(|v| v.get("id").and_then(Value::as_i64) == Some(id))
            .copied()
    };

    let persist = card
        .join("Settings")
        .join(core_id)
        .join("Interact/_core/interact_persist.json");
    let mut values = Vec::new();
    let mut unreadable = None;
    if persist.is_file() {
        match json_at(&persist)
            .as_ref()
            .and_then(|j| j.pointer("/interact_persist/variables"))
            .and_then(Value::as_array)
        {
            Some(list) => {
                for v in list {
                    let (Some(id), Some(val)) = (
                        v.get("id").and_then(Value::as_i64),
                        v.get("val").and_then(Value::as_i64),
                    ) else {
                        continue;
                    };
                    let def = var(id);
                    let kind = kind_of(
                        v.get("type")
                            .and_then(Value::as_str)
                            .or_else(|| def.and_then(|d| d.get("type")).and_then(Value::as_str))
                            .unwrap_or("value"),
                    );
                    values.push(SettingValue {
                        id,
                        name: def
                            .and_then(|d| d.get("name"))
                            .and_then(Value::as_str)
                            .map_or_else(|| format!("Setting {id}"), str::to_string),
                        known: def.is_some(),
                        value: val,
                        display: display(def, &kind, val),
                        default_display: def
                            .and_then(|d| d.get("defaultval"))
                            .and_then(Value::as_i64)
                            .map_or_else(String::new, |d| display(def, &kind, d)),
                        kind,
                    });
                }
            }
            None => unreadable = Some("interact_persist.json could not be read".to_string()),
        }
    }
    values.sort_by_key(|v| (!v.known, v.id));

    let mut files = Vec::new();
    for group in ["Interact", "Input", "Video"] {
        let folder = card.join("Settings").join(core_id).join(group);
        for (rel, path) in walk_files(&folder) {
            let meta = fs::metadata(&path).ok();
            files.push(SavedFile {
                group: group.into(),
                rel_path: format!("Settings/{core_id}/{group}/{rel}"),
                bytes: meta.as_ref().map_or(0, fs::Metadata::len),
                modified: meta
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0)),
            });
        }
    }
    let presets_folder = format!("Presets/{core_id}");
    let presets = walk_files(&card.join(&presets_folder))
        .iter()
        .filter(|(rel, _)| rel.to_ascii_lowercase().ends_with(".json"))
        .count();
    Ok(CoreSettings {
        values,
        files,
        presets,
        presets_folder,
        unreadable,
    })
}

fn walk_files(dir: &Path) -> Vec<(String, std::path::PathBuf)> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), String::new(), 0usize)];
    while let Some((path, rel, depth)) = stack.pop() {
        for e in fs::read_dir(&path).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let child = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            match e.file_type() {
                Ok(t) if t.is_dir() && depth < 8 => stack.push((e.path(), child, depth + 1)),
                Ok(t) if t.is_file() => out.push((child, e.path())),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// Re-exported so the host asks one module for the settings page: what the core still needs from the user.
pub fn needs_attention(
    card: &Path,
    core_id: &str,
) -> Result<Vec<core_files::MissingFile>, TauError> {
    let d = corehub::core_detail(card, core_id)?;
    Ok(core_files::missing(card, &d))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corehub::testcard::{card, write};

    #[test]
    fn saved_settings_are_decoded_against_the_core() {
        let card = card("settings");
        let s = core_settings(&card, "Spiritualized.GBA").unwrap();
        let by = |n: &str| s.values.iter().find(|v| v.name == n).unwrap();
        assert_eq!(
            (
                by("High Quality Audio").display.as_str(),
                by("High Quality Audio").default_display.as_str(),
                by("High Quality Audio").kind.as_str()
            ),
            ("Off", "On", "check")
        );
        assert_eq!(
            (
                by("Rumble Strength").display.as_str(),
                by("Rumble Strength").kind.as_str()
            ),
            ("6", "slider")
        );
        assert_eq!(by("Edge Masking").display, "Auto Masking");
        // an id the core no longer has is kept, last, and named by its id
        let unknown = s.values.last().unwrap();
        assert_eq!(
            (unknown.name.as_str(), unknown.known),
            ("Setting 99", false)
        );
        assert_eq!(
            s.files.iter().map(|f| f.group.as_str()).collect::<Vec<_>>(),
            ["Interact", "Input"]
        );
        assert_eq!(
            (s.presets, s.presets_folder.as_str()),
            (1, "Presets/Spiritualized.GBA")
        );
        assert!(s.unreadable.is_none());
    }

    #[test]
    fn nothing_saved_and_a_damaged_file_are_both_plain() {
        let card = card("settings2");
        let none = core_settings(&card, "other.GBAlt").unwrap();
        assert!(none.values.is_empty() && none.files.is_empty() && none.presets == 0);
        write(
            &card.join("Settings/other.GBAlt/Interact/_core/interact_persist.json"),
            b"not json",
        );
        assert!(
            core_settings(&card, "other.GBAlt")
                .unwrap()
                .unreadable
                .is_some()
        );
        assert!(core_settings(&card, "../x").is_err());
    }

    #[test]
    fn needs_attention_lists_the_missing_bios() {
        let card = card("attention");
        assert_eq!(
            needs_attention(&card, "Spiritualized.GBA").unwrap()[0].filename,
            "gba_bios.bin"
        );
    }
}
