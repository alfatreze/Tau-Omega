//! Where a core's files live on a card. Tau's Preview and Dev cores build on
//! their own platform (`tau_preview`, `tau_dev`) but read the library, covers
//! and music from `Assets/tau/common` through data-slot parameter bits
//! [25:24] (the index into `platform_ids`). Every part of Omega that needs the
//! media folder asks here, so there is one rule.

use serde_json::Value;
use std::{fs, path::Path};

/// Data-slot parameter bit 1: the file lives in `Assets/<platform>/<core>/`.
pub const SLOT_CORE_SPECIFIC: u64 = 0x2;

pub fn slot_parameters(slot: &Value) -> u64 {
    match slot.get("parameters") {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(Value::String(s)) => {
            let t = s.trim();
            t.strip_prefix("0x")
                .or_else(|| t.strip_prefix("0X"))
                .map_or_else(|| t.parse().ok(), |h| u64::from_str_radix(h, 16).ok())
                .unwrap_or(0)
        }
        _ => 0,
    }
}

/// Index into `platform_ids` a slot reads from (parameter bits [25:24]).
pub fn slot_platform_index(slot: &Value) -> usize {
    ((slot_parameters(slot) >> 24) & 3) as usize
}

fn slots_of(data_json: &Value) -> Vec<&Value> {
    data_json
        .pointer("/data/data_slots")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect()
}

/// The platform ids a core declares, in order (empty when core.json is unreadable).
pub fn platform_ids(core_json: &Value) -> Vec<String> {
    core_json
        .pointer("/core/metadata/platform_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

/// The platform whose `common/` holds the core's library: the platform picked
/// by the `tau-library.tdb` slot, else the first declared platform.
pub fn media_platform(platforms: &[String], data_json: Option<&Value>) -> String {
    let index = data_json
        .and_then(|json| {
            slots_of(json)
                .into_iter()
                .find(|s| s.get("filename").and_then(Value::as_str) == Some("tau-library.tdb"))
        })
        .map(slot_platform_index)
        .unwrap_or(0);
    platforms
        .get(index)
        .or_else(|| platforms.first())
        .cloned()
        .unwrap_or_default()
}

/// Reads a core's declared platforms and media platform from the card.
pub fn core_platforms(card_root: &Path, core_id: &str) -> (Vec<String>, String) {
    let dir = card_root.join("Cores").join(core_id);
    let read = |name: &str| {
        fs::read(dir.join(name))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
    };
    let platforms = read("core.json")
        .map(|j| platform_ids(&j))
        .unwrap_or_default();
    let media = media_platform(&platforms, read("data.json").as_ref());
    (platforms, media)
}

/// Another core (not `except`) that lists `platform` in any position.
pub fn platform_used_by_other(card_root: &Path, platform: &str, except: &str) -> bool {
    other_cores(card_root, except)
        .into_iter()
        .any(|(_, platforms, _)| platforms.iter().any(|p| p == platform))
}

/// True when another core still reads `filename` from the platform's `common/`
/// (a slot naming it without the core-specific bit, on that platform).
pub fn read_from_common_by_other(
    card_root: &Path,
    platform: &str,
    filename: &str,
    except: &str,
) -> bool {
    let cores_dir = card_root.join("Cores");
    let Ok(rd) = fs::read_dir(&cores_dir) else {
        return false;
    };
    for entry in rd.flatten() {
        let id = entry.file_name().to_string_lossy().to_string();
        if id == except || !entry.path().is_dir() {
            continue;
        }
        let read = |name: &str| {
            fs::read(entry.path().join(name))
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        };
        let platforms = read("core.json")
            .map(|j| platform_ids(&j))
            .unwrap_or_default();
        let Some(data) = read("data.json") else {
            continue;
        };
        for slot in slots_of(&data) {
            if slot.get("filename").and_then(Value::as_str) != Some(filename)
                || slot_parameters(slot) & SLOT_CORE_SPECIFIC != 0
            {
                continue;
            }
            let p = platforms
                .get(slot_platform_index(slot))
                .or_else(|| platforms.first());
            if p.map(String::as_str) == Some(platform) {
                return true;
            }
        }
    }
    false
}

fn other_cores(card_root: &Path, except: &str) -> Vec<(String, Vec<String>, String)> {
    let Ok(rd) = fs::read_dir(card_root.join("Cores")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let id = entry.file_name().to_string_lossy().to_string();
        if id == except || !entry.path().is_dir() {
            continue;
        }
        let (platforms, media) = core_platforms(card_root, &id);
        out.push((id, platforms, media));
    }
    out
}

/// Whether a manifest-declared obsolete path may ever be deleted for `core_id`.
/// Only the core's own areas qualify: its `Cores/<id>/` folder, its
/// `Assets/<platform>/<id>/` folders, platform files of a platform no other
/// core uses, and a build-bound file moved out of `common/` (decided by the
/// caller with [`read_from_common_by_other`]). User data is never allowed.
pub fn obsolete_path_allowed(
    card_root: &Path,
    core_id: &str,
    platforms: &[String],
    owned: &[String],
    path: &str,
) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == "..")
        || parts.len() < 2
    {
        return false;
    }
    match parts[0] {
        "Cores" => parts[1] == core_id,
        "Assets" => {
            if parts.len() == 4 && parts[2] == "common" && platforms.iter().any(|p| p == parts[1]) {
                // A build-bound file moved out of common/: only when the same
                // package now owns it in this core's own folder and no other
                // core still reads it from common/.
                let moved = format!("Assets/{}/{core_id}/{}", parts[1], parts[3]);
                return owned.contains(&moved)
                    && !read_from_common_by_other(card_root, parts[1], parts[3], core_id);
            }
            parts.len() >= 4 && platforms.iter().any(|p| p == parts[1]) && parts[2] == core_id
        }
        "Platforms" => {
            let name = parts[1..].join("/");
            let stem = name.split('.').next().unwrap_or("");
            let in_image_dir = parts.get(1) == Some(&"_images");
            let stem = if in_image_dir {
                parts
                    .get(2)
                    .map(|n| n.split('.').next().unwrap_or(""))
                    .unwrap_or("")
            } else {
                stem
            };
            platforms.iter().any(|p| p == stem) && !platform_used_by_other(card_root, stem, core_id)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn core(card: &Path, id: &str, platforms: &[&str], slots: Value) {
        let dir = card.join("Cores").join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("core.json"),
            json!({"core":{"metadata":{"platform_ids":platforms}}}).to_string(),
        )
        .unwrap();
        fs::write(
            dir.join("data.json"),
            json!({"data":{"data_slots":slots}}).to_string(),
        )
        .unwrap();
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("tau-cardlayout-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn media_platform_follows_the_library_slot() {
        let card = tmp("media");
        core(
            &card,
            "alfatreze.TAU Preview",
            &["tau_preview", "tau"],
            json!([
                {"id":1,"filename":"tau.rom","parameters":"0x2"},
                {"id":5,"filename":"tau-library.tdb","parameters":"0x1000000"}
            ]),
        );
        let (platforms, media) = core_platforms(&card, "alfatreze.TAU Preview");
        assert_eq!(platforms, ["tau_preview", "tau"]);
        assert_eq!(media, "tau");
        core(
            &card,
            "alfatreze.TAU",
            &["tau"],
            json!([{"id":5,"filename":"tau-library.tdb","parameters":"0x0"}]),
        );
        assert_eq!(core_platforms(&card, "alfatreze.TAU").1, "tau");
        let _ = fs::remove_dir_all(&card);
    }

    #[test]
    fn platform_use_counts_any_position() {
        let card = tmp("use");
        core(&card, "a.One", &["tau"], json!([]));
        core(&card, "a.Two", &["tau_dev", "tau"], json!([]));
        assert!(platform_used_by_other(&card, "tau", "a.One"));
        assert!(!platform_used_by_other(&card, "tau_dev", "a.Two"));
        assert!(platform_used_by_other(&card, "tau_dev", "a.One"));
        let _ = fs::remove_dir_all(&card);
    }

    #[test]
    fn common_read_by_other_needs_a_non_core_specific_slot() {
        let card = tmp("common");
        core(
            &card,
            "a.New",
            &["tau"],
            json!([{"id":1,"filename":"tau.rom","parameters":"0x2"}]),
        );
        core(
            &card,
            "a.Old",
            &["tau"],
            json!([{"id":1,"filename":"tau.rom","parameters":"0x0"}]),
        );
        assert!(read_from_common_by_other(&card, "tau", "tau.rom", "a.New"));
        assert!(!read_from_common_by_other(&card, "tau", "tau.rom", "a.Old"));
        let _ = fs::remove_dir_all(&card);
    }

    #[test]
    fn obsolete_filter_stays_inside_the_core() {
        let card = tmp("obs");
        core(&card, "a.One", &["tau"], json!([]));
        let p = ["tau".to_string()];
        for ok in [
            "Cores/a.One/old.json",
            "Assets/tau/a.One/tau.rom",
            "Platforms/tau.json",
            "Platforms/_images/tau.bin",
        ] {
            assert!(obsolete_path_allowed(&card, "a.One", &p, &[], ok), "{ok}");
        }
        for bad in [
            "Cores/other/core.json",
            "Assets/tau/common/song.mp3",
            "Assets/tau/common/tau-library.tdb",
            "Saves/tau/a.sav",
            "Settings/a.One/x",
            "Memories/Screenshots/a.png",
            "System/cores_cache.bin",
            "Cores/../Saves/x",
            "/etc/passwd",
            "Assets/other/a.One/x",
        ] {
            assert!(
                !obsolete_path_allowed(&card, "a.One", &p, &[], bad),
                "{bad}"
            );
        }
        let owned = ["Assets/tau/a.One/tau.rom".to_string()];
        core(
            &card,
            "a.One",
            &["tau"],
            json!([{"id":1,"filename":"tau.rom","parameters":"0x2"}]),
        );
        assert!(obsolete_path_allowed(
            &card,
            "a.One",
            &p,
            &owned,
            "Assets/tau/common/tau.rom"
        ));
        assert!(
            !obsolete_path_allowed(&card, "a.One", &p, &[], "Assets/tau/common/tau.rom"),
            "not owned by the package"
        );
        assert!(!obsolete_path_allowed(
            &card,
            "a.One",
            &p,
            &owned,
            "Assets/tau/common/song.mp3"
        ));
        core(
            &card,
            "a.Old",
            &["tau"],
            json!([{"id":1,"filename":"tau.rom","parameters":"0x0"}]),
        );
        assert!(
            !obsolete_path_allowed(&card, "a.One", &p, &owned, "Assets/tau/common/tau.rom"),
            "another core reads it"
        );
        core(&card, "a.Two", &["tau", "x"], json!([]));
        assert!(!obsolete_path_allowed(
            &card,
            "a.One",
            &p,
            &[],
            "Platforms/tau.json"
        ));
        let _ = fs::remove_dir_all(&card);
    }
}
