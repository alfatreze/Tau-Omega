//! What a file in a core's asset folders is for, as far as it can be known. Three sources, strongest first, and the entry says which one it
//! used so nothing is presented as more certain than it is:
//!
//! 1. **The core's own file list** (`data.json`): the data slot that names the file (or accepts its extension) gives its role name, whether it
//!    is required, its exact or maximum size and whether it is core-only. This is the card's own statement.
//! 2. **A known file**: a short built-in list of well-known names (BIOS images, palettes) with a one-line explanation, each checked against the
//!    core's README or the platform's documentation. Only names that are unambiguous are in it.
//! 3. **A guess** from the slot name or the extension (hard drive, floppy, palette...), worded as a guess.
//!
//! The group (BIOS and system, palettes, disks, memory and saves, game setup, game data) is inferred the same way. `support` marks the
//! files a core needs in order to run (BIOS, palettes, configuration), which the Settings tab lists; games stay in Files.

use crate::{core_files::FolderRole, corehub::SlotInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Purpose {
    /// "BIOS and system", "Palettes", "Disks", "Memory and saves", "Game setup", "Game data", "Other files".
    pub group: String,
    /// One or two plain sentences.
    pub summary: String,
    /// `core` (the core's file list), `known` (the built-in list) or `guess`.
    pub source: String,
    /// A file the core needs to run, not a game.
    pub support: bool,
}

pub const BIOS: &str = "BIOS and system";
pub const PALETTES: &str = "Palettes";
pub const DISKS: &str = "Disks";
pub const MEMORY: &str = "Memory and saves";
pub const SETUP: &str = "Game setup";
pub const DATA: &str = "Game data";
pub const OTHER: &str = "Other files";

/// Well-known file names: (lower-case file name, group, explanation).
const KNOWN: &[(&str, &str, &str)] = &[
    (
        "fds.bios",
        BIOS,
        "The Famicom Disk System BIOS. Needed only to play .fds disk games; without it those games report a missing file.",
    ),
    (
        "c2c03.pal",
        PALETTES,
        "Colour palette for Nintendo Vs. System arcade games (the RP2C03 picture chip). Only used when playing Vs. System games.",
    ),
    (
        "c2c03b.pal",
        PALETTES,
        "Colour palette for Nintendo Vs. System arcade games (the RP2C03B picture chip). Only used when playing Vs. System games.",
    ),
    (
        "p2c04_1.pal",
        PALETTES,
        "Colour palette for Nintendo Vs. System arcade games (RP2C04-0001). Only used when playing Vs. System games.",
    ),
    (
        "p2c04_2.pal",
        PALETTES,
        "Colour palette for Nintendo Vs. System arcade games (RP2C04-0002). Only used when playing Vs. System games.",
    ),
    (
        "p2c04_3.pal",
        PALETTES,
        "Colour palette for Nintendo Vs. System arcade games (RP2C04-0003). Only used when playing Vs. System games.",
    ),
    (
        "p2c04_4.pal",
        PALETTES,
        "Colour palette for Nintendo Vs. System arcade games (RP2C04-0004). Only used when playing Vs. System games.",
    ),
    (
        "gba_bios.bin",
        BIOS,
        "The Game Boy Advance boot ROM (16 KB). The core needs it to start any game.",
    ),
    (
        "uni-bios_4_0.rom",
        BIOS,
        "The Universe BIOS for Neo Geo: a community replacement for the original system ROM, with extra menus and region switching.",
    ),
    (
        "000-lo.lo",
        BIOS,
        "The Neo Geo zoom lookup table, part of the system ROMs every game uses for sprite scaling.",
    ),
    (
        "sfix.sfix",
        BIOS,
        "The Neo Geo fix-layer tiles (text and status bars) the system ROM draws with.",
    ),
    (
        "amigavision.rom",
        BIOS,
        "An Amiga Kickstart ROM, the computer's built-in system software.",
    ),
    (
        "megaags-kickstart.rom",
        BIOS,
        "An Amiga Kickstart ROM, the computer's built-in system software.",
    ),
    (
        "amiga_mpu_bios.bin",
        BIOS,
        "Firmware for the core's helper processor (MPU), which loads disks and drives the on-screen menu.",
    ),
];

fn lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

fn ext_of(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| lower(e))
        .unwrap_or_default()
}

fn size_phrase(s: &SlotInfo) -> String {
    let kb = |b: u64| {
        if b >= 1_048_576 {
            format!("{:.1} MB", b as f64 / 1_048_576.0)
        } else if b >= 1024 {
            format!("{} KB", b / 1024)
        } else {
            format!("{b} bytes")
        }
    };
    match (s.size_exact, s.size_maximum) {
        (Some(e), _) if e > 0 => format!(", exactly {}", kb(e)),
        (_, Some(m)) if m > 0 => format!(", up to {}", kb(m)),
        _ => String::new(),
    }
}

/// The group a slot belongs to, from its name and extensions.
fn slot_group(slot: &SlotInfo) -> &'static str {
    let n = lower(&slot.name);
    let has = |w: &[&str]| w.iter().any(|w| n.contains(w));
    let exts: Vec<&str> = slot.extensions.iter().map(String::as_str).collect();
    if has(&["bios", "firmware", "kickstart", "boot"]) {
        BIOS
    } else if has(&["pal"]) || exts.contains(&"pal") {
        PALETTES
    } else if has(&["hard drive", "floppy", "disk", "cd", "tape"])
        || exts
            .iter()
            .any(|e| ["hdf", "adf", "d64", "dsk", "img", "iso", "chd"].contains(e))
    {
        DISKS
    } else if slot.nonvolatile || has(&["save", "memory", "backup", "ram"]) {
        MEMORY
    } else if exts == ["json"] || has(&["setup", "config", "instance"]) {
        SETUP
    } else if slot.filename.is_empty() {
        DATA
    } else if slot.required {
        BIOS
    } else {
        OTHER
    }
}

/// What `name` (a file in a core's asset folder of `role`) is for.
pub fn describe(role: FolderRole, name: &str, slots: &[SlotInfo]) -> Purpose {
    let lname = lower(name);
    let base = lname.rsplit('/').next().unwrap_or(&lname).to_string();
    let by_name = slots.iter().find(|s| {
        !s.filename.is_empty() && lower(&s.filename).rsplit('/').next() == Some(base.as_str())
    });
    let ext = ext_of(name);
    let by_ext = slots
        .iter()
        .find(|s| s.filename.is_empty() && s.extensions.contains(&ext));
    let known = KNOWN.iter().find(|(f, _, _)| *f == base);

    if let Some(s) = by_name {
        let (group, summary_known) = match known {
            Some((_, g, t)) => (*g, Some((*t).to_string())),
            None => (slot_group(s), None),
        };
        let facts = format!(
            "Slot \"{}\": {}{}{}.",
            s.name,
            if s.required { "required" } else { "optional" },
            size_phrase(s),
            if s.core_specific {
                ", only this core reads it"
            } else {
                ""
            }
        );
        let summary = match summary_known {
            Some(t) => format!("{t} {facts}"),
            None => facts,
        };
        let support = !matches!(
            role,
            FolderRole::SavesShared | FolderRole::SavesCore | FolderRole::SaveStates
        );
        return Purpose {
            group: group.into(),
            summary,
            source: "core".into(),
            support,
        };
    }
    if let Some((_, g, t)) = known {
        return Purpose {
            group: (*g).into(),
            summary: (*t).into(),
            source: "known".into(),
            support: true,
        };
    }
    if let Some(s) = by_ext {
        // A file the slot accepts by extension (a game, a disk image): data the core reads, not something it needs in order to start.
        return Purpose {
            group: slot_group(s).into(),
            summary: format!(
                "Read by the slot \"{}\" ({}{}).",
                s.name,
                if s.required { "required" } else { "optional" },
                size_phrase(s)
            ),
            source: "core".into(),
            support: false,
        };
    }
    // Nothing names it. A file inside the core's own folder that no slot mentions is most likely read by the core directly.
    if role == FolderRole::AssetsCore && ext == "json" {
        // A json file in the core's own folder that no slot accepts: the core's instance/configuration file. (A core whose slots accept json,
        // like Neo Geo's per-game setups, never gets here: those are matched by their slot and stay with the games.)
        return Purpose {
            group: SETUP.into(),
            summary: "A configuration file in the core's own folder (an instance file that tells the core which data files to load). Not named in the core's file list.".into(),
            source: "guess".into(),
            support: true,
        };
    }
    if role == FolderRole::AssetsCore {
        return Purpose {
            group: OTHER.into(),
            summary: "Not named in the core's file list. It is in the core's own folder, so the core or its helper firmware probably reads it directly.".into(),
            source: "guess".into(),
            support: true,
        };
    }
    Purpose {
        group: DATA.into(),
        summary: String::new(),
        source: "guess".into(),
        support: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(name: &str, file: &str, exts: &[&str], required: bool) -> SlotInfo {
        SlotInfo {
            id: "0x1".into(),
            name: name.into(),
            required,
            filename: file.into(),
            extensions: exts.iter().map(|s| (*s).into()).collect(),
            nonvolatile: false,
            core_specific: false,
            platform: "nes".into(),
            size_exact: None,
            size_maximum: None,
        }
    }

    #[test]
    fn a_file_named_by_a_slot_gets_the_slots_facts_and_a_known_explanation() {
        let mut vs = slot("VS Pal", "C2C03.PAL", &["pal"], false);
        vs.size_exact = Some(192);
        let p = describe(FolderRole::AssetsCore, "C2C03.PAL", &[vs]);
        assert_eq!(
            (p.group.as_str(), p.source.as_str(), p.support),
            (PALETTES, "core", true)
        );
        assert!(
            p.summary.contains("Vs. System")
                && p.summary
                    .contains("Slot \"VS Pal\": optional, exactly 192 bytes"),
            "{}",
            p.summary
        );
    }

    #[test]
    fn groups_come_from_the_slot_name_and_extension() {
        let s = [
            slot("Hard Drive - IDE 0", "AmigaVision.hdf", &["hdf"], false),
            slot("Floppy Drive 0", "", &["adf"], false),
            slot("Game JSON Setup", "", &["json"], true),
        ];
        assert_eq!(
            describe(FolderRole::AssetsShared, "AmigaVision.hdf", &s).group,
            DISKS
        );
        assert_eq!(
            describe(FolderRole::AssetsShared, "Zak.adf", &s).group,
            DISKS
        );
        assert_eq!(
            describe(FolderRole::AssetsCore, "3 Count Bout.json", &s).group,
            SETUP
        );
    }

    #[test]
    fn an_unlisted_file_in_the_cores_own_folder_is_a_labelled_guess() {
        let p = describe(
            FolderRole::AssetsCore,
            "mpu.bin",
            &[slot("MPU Bios", "amiga_mpu_bios.bin", &["bin"], true)],
        );
        assert_eq!((p.source.as_str(), p.support), ("guess", true));
        assert!(p.summary.contains("Not named in the core's file list"));
        // an unlisted json in the core's own folder is its configuration, not a game
        let c = describe(
            FolderRole::AssetsCore,
            "TAU.json",
            &[slot("MPU Bios", "amiga_mpu_bios.bin", &["bin"], true)],
        );
        assert_eq!(
            (c.group.as_str(), c.source.as_str(), c.support),
            (SETUP, "guess", true)
        );
        // a game in the shared folder with no slot says nothing it cannot know
        let g = describe(FolderRole::AssetsShared, "Some Game.nes", &[]);
        assert_eq!(
            (g.group.as_str(), g.summary.as_str(), g.support),
            (DATA, "", false)
        );
    }

    #[test]
    fn known_names_without_a_slot_still_explain_themselves() {
        let p = describe(FolderRole::AssetsShared, "fds.bios", &[]);
        assert_eq!((p.group.as_str(), p.source.as_str()), (BIOS, "known"));
        assert!(p.summary.contains("Famicom Disk System"));
    }
}
