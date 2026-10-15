//! The Cores section's read side: the platform tree of the open card and one core's details. Pure reads of `Cores/`,
//! `Platforms/` and the core's own JSON; nothing is written and nothing is guessed that the card does not say.
//!
//! A core belongs to the platform it names first (`platform_ids[0]`); a core with no platform lands under `_none`
//! ("No platform"). Platform names come from `Platforms/<id>.json`; a platform folder with no such file is shown by its id.

use crate::{TauError, cardlayout, inspect_card};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlatformInfo {
    pub id: String,
    /// `Platforms/<id>.json` name, else the id.
    pub name: String,
    pub category: String,
    pub manufacturer: String,
    pub year: u32,
    pub has_image: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreRow {
    pub id: String,
    pub shortname: String,
    pub author: String,
    pub version: String,
    pub platform: String,
    pub platforms: Vec<String>,
    /// Required files with a fixed name (a BIOS) that are not on the card: the tree marks the core when this is not zero.
    pub missing: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlatformNode {
    pub platform: PlatformInfo,
    /// Sorted by shortname.
    pub cores: Vec<CoreRow>,
}

pub const NO_PLATFORM: &str = "_none";

fn json_at(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

pub fn platform_info(card: &Path, id: &str) -> PlatformInfo {
    if id == NO_PLATFORM || id.is_empty() {
        return PlatformInfo {
            id: NO_PLATFORM.into(),
            name: "No platform".into(),
            ..PlatformInfo::default()
        };
    }
    let json = json_at(&card.join("Platforms").join(format!("{id}.json")));
    let text = |k: &str| {
        json.as_ref()
            .and_then(|j| j.pointer(&format!("/platform/{k}")))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let name = text("name");
    PlatformInfo {
        id: id.to_string(),
        name: if name.is_empty() {
            id.to_string()
        } else {
            name
        },
        category: text("category"),
        manufacturer: text("manufacturer"),
        year: json
            .as_ref()
            .and_then(|j| j.pointer("/platform/year"))
            .and_then(Value::as_u64)
            .and_then(|y| u32::try_from(y).ok())
            .unwrap_or(0),
        has_image: card
            .join("Platforms/_images")
            .join(format!("{id}.bin"))
            .is_file(),
    }
}

/// Installed cores grouped by main platform, platforms sorted by name, cores by shortname.
pub fn platform_tree(card: &Path) -> Result<Vec<PlatformNode>, TauError> {
    let cores = inspect_card(card)?.cores;
    let mut groups: BTreeMap<String, Vec<CoreRow>> = BTreeMap::new();
    for c in cores {
        let key = if is_tau_family(&c) {
            // Tau is one system, like every GBA core is under `gba`: its stable, preview and dev cores share one row and one media folder.
            TAU_PLATFORM.to_string()
        } else if c.platform.is_empty() {
            NO_PLATFORM.to_string()
        } else {
            c.platform.clone()
        };
        let missing =
            core_detail(card, &c.id).map_or(0, |d| crate::core_files::missing(card, &d).len());
        groups.entry(key).or_default().push(CoreRow {
            missing,
            shortname: if c.shortname.is_empty() {
                c.id.clone()
            } else {
                c.shortname.clone()
            },
            id: c.id,
            author: c.author,
            version: c.version,
            platform: c.platform,
            platforms: c.platforms,
        });
    }
    let mut nodes: Vec<PlatformNode> = groups
        .into_iter()
        .map(|(id, mut cores)| {
            cores.sort_by_key(|c| c.shortname.to_lowercase());
            let mut platform = tree_platform_info(card, &id, &cores);
            let (category, manufacturer) =
                crate::taxonomy::resolve(&id, &platform.category, &platform.manufacturer);
            if id != NO_PLATFORM {
                platform.category = category;
                platform.manufacturer = manufacturer;
            }
            PlatformNode { platform, cores }
        })
        .collect();
    nodes.sort_by_key(|n| {
        (
            n.platform.id == NO_PLATFORM,
            crate::taxonomy::category_rank(&n.platform.category),
            n.platform.manufacturer.to_lowercase(),
            n.platform.name.to_lowercase(),
        )
    });
    Ok(nodes)
}

pub const TAU_PLATFORM: &str = "tau";

/// A core of the Tau family: it serves the library index, or its id starts with `alfatreze.TAU`.
fn is_tau_family(c: &crate::card::Core) -> bool {
    c.library_capable || c.id.to_ascii_lowercase().starts_with("alfatreze.tau")
}

/// The platform record of a tree row. The Tau row uses `tau`'s own platform file; a card that only has a dev build's platform file
/// (`tau_dev`) borrows the first Tau-family platform file it finds, so the row still shows the picture and the name.
fn tree_platform_info(card: &Path, id: &str, cores: &[CoreRow]) -> PlatformInfo {
    let own = platform_info(card, id);
    if id != TAU_PLATFORM || card.join("Platforms/tau.json").is_file() {
        return own;
    }
    let borrowed = cores
        .iter()
        .flat_map(|c| c.platforms.iter())
        .find(|p| card.join("Platforms").join(format!("{p}.json")).is_file());
    match borrowed {
        Some(p) => PlatformInfo {
            id: TAU_PLATFORM.into(),
            name: "TAU".into(),
            has_image: false,
            ..platform_info(card, p)
        },
        None => PlatformInfo {
            name: "TAU".into(),
            category: "Media Players".into(),
            manufacturer: "alfatreze".into(),
            ..own
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SlotInfo {
    pub id: String,
    pub name: String,
    pub required: bool,
    /// A fixed file name (a BIOS), else empty.
    pub filename: String,
    pub extensions: Vec<String>,
    pub nonvolatile: bool,
    /// Read from `Assets/<platform>/<core>/` instead of `common/`.
    pub core_specific: bool,
    /// The platform folder this slot reads from.
    pub platform: String,
    pub size_exact: Option<u64>,
    pub size_maximum: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreDetail {
    pub id: String,
    pub shortname: String,
    pub author: String,
    pub description: String,
    pub url: String,
    pub version: String,
    pub date_release: String,
    /// Minimum Pocket firmware, empty when not stated.
    pub firmware_required: String,
    pub dock_supported: Option<bool>,
    pub link_port: Option<bool>,
    pub sleep_supported: Option<bool>,
    pub platforms: Vec<PlatformInfo>,
    pub slots: Vec<SlotInfo>,
    pub has_icon: bool,
    pub has_interact: bool,
}

fn id_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// The slots of a core, with the folder rule applied (`core_specific_file`, `platform_index`).
pub fn slots(data_json: Option<&Value>, platform_ids: &[String]) -> Vec<SlotInfo> {
    data_json
        .and_then(|j| j.pointer("/data/data_slots"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|s| {
            let index = cardlayout::slot_platform_index(s);
            let text = |k: &str| {
                s.get(k)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            SlotInfo {
                id: id_text(s.get("id")),
                name: text("name"),
                required: s.get("required").and_then(Value::as_bool).unwrap_or(false),
                filename: text("filename"),
                extensions: s
                    .get("extensions")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.as_str().map(str::to_ascii_lowercase))
                    .collect(),
                nonvolatile: s
                    .get("nonvolatile")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                core_specific: cardlayout::slot_parameters(s) & cardlayout::SLOT_CORE_SPECIFIC != 0,
                platform: platform_ids
                    .get(index)
                    .or(platform_ids.first())
                    .cloned()
                    .unwrap_or_default(),
                size_exact: s
                    .get("size_exact")
                    .and_then(Value::as_u64)
                    .filter(|n| *n > 0),
                size_maximum: s
                    .get("size_maximum")
                    .and_then(Value::as_u64)
                    .filter(|n| *n > 0),
            }
        })
        .collect()
}

/// A core id that is a plain folder name under `Cores/` (the UI passes ids back to us).
pub fn core_dir(card: &Path, core_id: &str) -> Result<PathBuf, TauError> {
    if core_id.is_empty() || core_id.contains(['/', '\\', '\0']) || core_id.contains("..") {
        return Err(TauError::e(
            crate::ErrorCode::InvalidPathReference,
            "That is not a core id.",
        ));
    }
    let dir = card.join("Cores").join(core_id);
    if dir.join("core.json").is_file() {
        Ok(dir)
    } else {
        Err(TauError::e(
            crate::ErrorCode::NotFound,
            format!("{core_id} is not on this card."),
        ))
    }
}

pub fn core_detail(card: &Path, core_id: &str) -> Result<CoreDetail, TauError> {
    let dir = core_dir(card, core_id)?;
    let core = json_at(&dir.join("core.json")).ok_or_else(|| {
        TauError::e(
            crate::ErrorCode::Json,
            format!("{core_id}/core.json cannot be read."),
        )
    })?;
    let text = |k: &str| {
        core.pointer(&format!("/core/metadata/{k}"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let flag = |p: &str| core.pointer(p).and_then(Value::as_bool);
    let platform_ids = cardlayout::platform_ids(&core);
    let data = json_at(&dir.join("data.json"));
    Ok(CoreDetail {
        id: core_id.to_string(),
        shortname: text("shortname"),
        author: text("author"),
        description: text("description"),
        url: text("url"),
        version: text("version"),
        date_release: text("date_release"),
        firmware_required: core
            .pointer("/core/framework/version_required")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        dock_supported: flag("/core/framework/dock/supported"),
        link_port: flag("/core/framework/hardware/link_port"),
        sleep_supported: flag("/core/framework/sleep_supported"),
        platforms: platform_ids
            .iter()
            .map(|p| platform_info(card, p))
            .collect(),
        slots: slots(data.as_ref(), &platform_ids),
        has_icon: dir.join("icon.bin").is_file(),
        has_interact: dir.join("interact.json").is_file(),
    })
}

#[cfg(test)]
pub(crate) mod testcard {
    //! A synthetic Pocket card for the Cores tests: two platforms, three cores, files, saves, states, screenshots, settings.
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    pub fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    pub fn core_json(author: &str, short: &str, version: &str, platforms: &[&str]) -> String {
        let ids = platforms
            .iter()
            .map(|p| format!("\"{p}\""))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"core":{{"magic":"APF_VER_1","metadata":{{"platform_ids":[{ids}],"shortname":"{short}","description":"{short} core","author":"{author}","url":"https://example.org/{short}","version":"{version}","date_release":"2026-01-02"}},"framework":{{"version_required":"1.1","sleep_supported":false,"dock":{{"supported":true}},"hardware":{{"link_port":false}}}}}}}}"#
        )
    }

    pub fn card(name: &str) -> PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "tau-cores-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        fs::create_dir_all(root.join("Cores")).unwrap();
        fs::create_dir_all(root.join("Assets")).unwrap();
        // GBA: shared ROM folder, a required BIOS slot, a save slot
        let data = r#"{"data":{"magic":"APF_VER_1","data_slots":[
          {"name":"Cartridge","id":"0x100","required":true,"parameters":129,"extensions":["gba","rom"],"size_maximum":67108864},
          {"name":"Save RAM","id":"0x101","required":false,"parameters":36,"nonvolatile":true,"extensions":["sav"]},
          {"name":"BIOS","filename":"gba_bios.bin","id":"0x103","required":true,"parameters":137,"extensions":["bin"],"size_exact":16384}]}}"#;
        write(
            &root.join("Cores/Spiritualized.GBA/core.json"),
            core_json("Spiritualized", "GBA", "1.2.0", &["gba"]).as_bytes(),
        );
        write(
            &root.join("Cores/Spiritualized.GBA/data.json"),
            data.as_bytes(),
        );
        write(&root.join("Cores/Spiritualized.GBA/interact.json"), br#"{"interact":{"magic":"APF_VER_1","variables":[
          {"name":"High Quality Audio","id":1,"type":"check","defaultval":1},
          {"name":"Rumble Strength","id":2,"type":"slider_u32","defaultval":5,"graphical":{"min":0,"max":7}},
          {"name":"Edge Masking","id":3,"type":"list","defaultval":3,"options":[{"value":0,"name":"No Masking"},{"value":3,"name":"Auto Masking"}]},
          {"name":"Reset core","id":0,"type":"action"}]}}"#);
        write(&root.join("Cores/Spiritualized.GBA/icon.bin"), &[0u8; 2592]);
        // Second GBA core, core-specific ROM folder
        write(
            &root.join("Cores/other.GBAlt/core.json"),
            core_json("other", "GBAlt", "0.3", &["gba"]).as_bytes(),
        );
        write(&root.join("Cores/other.GBAlt/data.json"), br#"{"data":{"data_slots":[{"name":"ROM","id":1,"required":true,"parameters":3,"extensions":["gba"]}]}}"#);
        // A core with no platform and a core on a platform with no Platforms file
        write(
            &root.join("Cores/solo.Thing/core.json"),
            core_json("solo", "Thing", "1", &[]).as_bytes(),
        );
        write(
            &root.join("Cores/arc.Game/core.json"),
            core_json("arc", "Game", "2", &["arcgame"]).as_bytes(),
        );
        write(&root.join("Platforms/gba.json"), br#"{"platform":{"category":"Nintendo","name":"Game Boy Advance","year":2001,"manufacturer":"Nintendo"}}"#);
        write(&root.join("Platforms/_images/gba.bin"), &[0u8; 8]);
        // files
        write(
            &root.join("Assets/gba/common/Boktai 2 (USA).gba"),
            &[1u8; 300],
        );
        write(&root.join("Assets/gba/common/Mother 3.gba"), &[2u8; 200]);
        write(&root.join("Assets/gba/common/readme.txt"), b"hello");
        write(&root.join("Assets/gba/common/._Mother 3.gba"), b"junk");
        write(
            &root.join("Assets/gba/other.GBAlt/Only Mine.gba"),
            &[3u8; 50],
        );
        // saves, states
        write(
            &root.join("Saves/gba/common/Boktai 2 (USA).sav"),
            &[9u8; 64],
        );
        write(&root.join("Saves/pocket-sync-save-backup__1.zip"), b"zip");
        write(&root.join("Memories/Save States/Spiritualized.GBA/20260101_120000_USR_00000000_Boktai 2 (USA).sta"), &[7u8; 10]);
        // screenshots: two near the state, one far away
        write(
            &root.join("Memories/Screenshots/20260101_120300.png"),
            &[0u8; 4],
        );
        write(
            &root.join("Memories/Screenshots/20260101_121000.png"),
            &[0u8; 4],
        );
        write(
            &root.join("Memories/Screenshots/20260305_090000.png"),
            &[0u8; 4],
        );
        // settings and presets
        write(&root.join("Settings/Spiritualized.GBA/Interact/_core/interact_persist.json"), br#"{"interact_persist":{"magic":"APF_VER_1","variables":[{"id":1,"type":"check","val":0},{"id":2,"type":"slider_u32","val":6},{"id":3,"type":"list","val":3},{"id":99,"type":"check","val":1}]}}"#);
        write(
            &root.join("Settings/Spiritualized.GBA/Input/_core/input_persist.json"),
            b"{}",
        );
        write(
            &root.join(
                "Presets/Spiritualized.GBA/Interact/gba/Spiritualized.GBA/Boktai 2 (USA).json",
            ),
            b"{}",
        );
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use testcard::*;

    #[test]
    fn the_tree_groups_by_main_platform_with_names_from_the_card() {
        let card = card("tree");
        let tree = platform_tree(&card).unwrap();
        let ids: Vec<&str> = tree.iter().map(|n| n.platform.id.as_str()).collect();
        // sorted by category (the Library's grouping: GBA is a Handheld), then manufacturer, then name; "no platform" is last
        assert_eq!(ids, ["gba", "arcgame", "_none"]);
        let gba = tree.iter().find(|n| n.platform.id == "gba").unwrap();
        // the card's own file says category "Nintendo"; the Library's grouping wins: Handheld > Nintendo
        assert_eq!(
            (
                gba.platform.category.as_str(),
                gba.platform.manufacturer.as_str()
            ),
            ("Handheld", "Nintendo")
        );
        let arc = tree.iter().find(|n| n.platform.id == "arcgame").unwrap();
        // unknown to both the Library and the card: "Other"
        assert_eq!(arc.platform.category, "Other");
        let gba = tree.iter().find(|n| n.platform.id == "gba").unwrap();
        // the GBA core needs its BIOS and the tree says so; the other core does not
        assert_eq!(
            gba.cores.iter().map(|c| c.missing).collect::<Vec<_>>(),
            [1, 0]
        );
        assert_eq!(
            (
                gba.platform.name.as_str(),
                gba.platform.manufacturer.as_str(),
                gba.platform.year,
                gba.platform.has_image
            ),
            ("Game Boy Advance", "Nintendo", 2001, true)
        );
        assert_eq!(
            gba.cores.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["Spiritualized.GBA", "other.GBAlt"]
        );
        let arc = tree.iter().find(|n| n.platform.id == "arcgame").unwrap();
        assert_eq!(
            (arc.platform.name.as_str(), arc.platform.has_image),
            ("arcgame", false)
        );
        assert_eq!(tree.last().unwrap().platform.name, "No platform");
    }

    #[test]
    fn every_tau_core_sits_under_one_tau_row_in_media_players() {
        let card = card("taurow");
        let tau = |id: &str, short: &str, platforms: &[&str]| {
            write(
                &card.join("Cores").join(id).join("core.json"),
                core_json("alfatreze", short, "0.6.0", platforms).as_bytes(),
            );
        };
        tau("alfatreze.TAU", "TAU", &["tau"]);
        tau(
            "alfatreze.TAU Preview",
            "TAU Preview",
            &["tau_preview", "tau"],
        );
        tau("alfatreze.TAU DEV 122", "TAU DEV 122", &["tau_dev", "tau"]);
        tau("alfatreze.TAU_DEV_105", "TAU_DEV_105", &["tau_dev_105"]);
        write(&card.join("Platforms/tau.json"), br#"{"platform":{"category":"Media Players","name":"TAU","year":2026,"manufacturer":"alfatreze"}}"#);
        write(&card.join("Platforms/tau_dev_105.json"), br#"{"platform":{"category":"Media Players","name":"TAU DEV 105","year":2026,"manufacturer":"alfatreze"}}"#);
        let tree = platform_tree(&card).unwrap();
        let rows: Vec<&PlatformNode> = tree
            .iter()
            .filter(|n| n.cores.iter().any(|c| c.id.starts_with("alfatreze.TAU")))
            .collect();
        assert_eq!(rows.len(), 1, "one Tau row, not one per build platform");
        let t = rows[0];
        assert_eq!(
            (
                t.platform.id.as_str(),
                t.platform.name.as_str(),
                t.platform.category.as_str(),
                t.platform.manufacturer.as_str()
            ),
            ("tau", "TAU", "Media Players", "alfatreze")
        );
        assert_eq!(t.cores.len(), 4);
        // other platforms are not touched by the fold
        assert!(
            tree.iter()
                .any(|n| n.platform.id == "gba" && n.cores.len() == 2)
        );
    }

    #[test]
    fn a_card_with_only_a_dev_platform_file_still_shows_the_tau_row() {
        let card = card("taudev");
        write(
            &card.join("Cores/alfatreze.TAU DEV 9/core.json"),
            core_json("alfatreze", "TAU DEV 9", "0.6.0", &["tau_dev", "tau"]).as_bytes(),
        );
        write(&card.join("Platforms/tau_dev.json"), br#"{"platform":{"category":"Media Players","name":"TAU Dev","year":2026,"manufacturer":"alfatreze"}}"#);
        let tree = platform_tree(&card).unwrap();
        let t = tree.iter().find(|n| n.platform.id == "tau").unwrap();
        assert_eq!(
            (t.platform.name.as_str(), t.platform.category.as_str()),
            ("TAU", "Media Players")
        );
        assert!(!t.platform.has_image);
    }

    #[test]
    fn a_core_detail_carries_its_facts_and_slots_with_the_folder_rule() {
        let card = card("detail");
        let d = core_detail(&card, "Spiritualized.GBA").unwrap();
        assert_eq!(
            (
                d.shortname.as_str(),
                d.version.as_str(),
                d.firmware_required.as_str(),
                d.dock_supported
            ),
            ("GBA", "1.2.0", "1.1", Some(true))
        );
        assert_eq!(
            (d.has_icon, d.has_interact, d.platforms[0].name.as_str()),
            (true, true, "Game Boy Advance")
        );
        let by = |n: &str| d.slots.iter().find(|s| s.name == n).unwrap();
        assert!(
            by("Cartridge").required
                && !by("Cartridge").core_specific
                && by("Cartridge").extensions == ["gba", "rom"]
        );
        assert!(
            by("Save RAM").nonvolatile && !by("Save RAM").core_specific,
            "parameters 36 = shared saves, as on the real card"
        );
        let alt = core_detail(&card, "other.GBAlt").unwrap();
        assert!(
            alt.slots[0].core_specific,
            "parameters 3 sets the core-specific bit"
        );
        assert_eq!(
            (by("BIOS").filename.as_str(), by("BIOS").size_exact),
            ("gba_bios.bin", Some(16384))
        );
        assert!(core_detail(&card, "../x").is_err() && core_detail(&card, "nobody.Core").is_err());
    }
}
