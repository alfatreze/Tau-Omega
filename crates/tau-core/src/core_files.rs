//! What a core reads and writes on the card, listed per folder: its ROMs and BIOS (`Assets/<platform>/common` shared by every
//! core of the platform, `Assets/<platform>/<core>` for this core only), its saves (`Saves/...`, same rule) and its save states
//! (`Memories/Save States/<core>`). Read-only; the add-files flow is `core_add`.
//!
//! Every entry carries a `kind` and, when a data slot accepts it, the `slot`; a parser can fill more in later without the UI
//! changing. Hidden files and macOS `._` companions are never listed.

use crate::{
    TauError,
    corehub::{self, CoreDetail, SlotInfo},
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

/// Entries listed per folder before the list is cut (the total is still counted).
pub const ENTRY_LIMIT: usize = 4000;
const MAX_DEPTH: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FolderRole {
    AssetsShared,
    AssetsCore,
    SavesShared,
    SavesCore,
    SaveStates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EntryKind {
    Rom,
    Bios,
    Tape,
    Disk,
    Save,
    State,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FolderRef {
    pub role: FolderRole,
    pub platform: String,
    /// Plain words for the UI and the CLI.
    pub label: String,
    /// Relative to the card root, forward slashes.
    pub rel_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Entry {
    /// Relative to the folder, forward slashes (folders dropped in keep their name).
    pub rel_path: String,
    pub name: String,
    pub bytes: u64,
    /// Unix seconds, 0 when unknown.
    pub modified: i64,
    pub kind: EntryKind,
    /// The data slot that accepts this file, when one does.
    pub slot: Option<String>,
    /// For saves and states: the game name taken from the file name.
    pub game: Option<String>,
    /// For states: when the Pocket made it (`YYYY-MM-DD HH:MM:SS`, from the file name).
    pub taken: Option<String>,
    /// What the file is for (see [`crate::file_purpose`]): its group, a plain explanation, where that came from, and whether it is a file the
    /// core needs to run (BIOS, palettes, configuration) rather than a game.
    #[cfg_attr(feature = "serde", serde(default))]
    pub group: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub purpose: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub purpose_source: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub support: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FolderListing {
    pub folder: FolderRef,
    pub exists: bool,
    pub entries: Vec<Entry>,
    pub total_files: usize,
    pub total_bytes: u64,
    /// `entries` was cut at [`ENTRY_LIMIT`].
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MissingFile {
    pub slot: String,
    pub filename: String,
    /// The folder it must be in, relative to the card.
    pub folder_rel: String,
    pub size_exact: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BackupZip {
    pub name: String,
    pub bytes: u64,
    pub modified: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreFiles {
    pub folders: Vec<FolderListing>,
    pub missing: Vec<MissingFile>,
    /// Pocket Sync's save backups sit in `Saves/`; they are counted, never listed as saves.
    pub save_backups: Vec<BackupZip>,
}

/// Every folder the core uses, in display order (assets, saves, save states).
pub fn folders(d: &CoreDetail) -> Vec<FolderRef> {
    let main = d
        .platforms
        .first()
        .map(|p| p.id.clone())
        .unwrap_or_default();
    let name_of = |id: &str| {
        d.platforms
            .iter()
            .find(|p| p.id == id)
            .map_or_else(|| id.to_string(), |p| p.name.clone())
    };
    let mut shared = BTreeSet::new();
    let mut own = BTreeSet::new();
    for s in &d.slots {
        if s.platform.is_empty() {
            continue;
        }
        if s.core_specific {
            own.insert(s.platform.clone())
        } else {
            shared.insert(s.platform.clone())
        };
    }
    if !main.is_empty() {
        // The platform's shared folder is always where a ROM can go; the core's own folder always exists as a place to put files.
        shared.insert(main.clone());
        own.insert(main.clone());
    }
    let mut out = Vec::new();
    for p in &shared {
        out.push(FolderRef {
            role: FolderRole::AssetsShared,
            platform: p.clone(),
            label: format!("Shared by every {} core", name_of(p)),
            rel_path: format!("Assets/{p}/common"),
        });
    }
    for p in &own {
        out.push(FolderRef {
            role: FolderRole::AssetsCore,
            platform: p.clone(),
            label: format!(
                "Only {}",
                if d.shortname.is_empty() {
                    &d.id
                } else {
                    &d.shortname
                }
            ),
            rel_path: format!("Assets/{p}/{}", d.id),
        });
    }
    for p in &shared {
        out.push(FolderRef {
            role: FolderRole::SavesShared,
            platform: p.clone(),
            label: format!("Saves shared by every {} core", name_of(p)),
            rel_path: format!("Saves/{p}/common"),
        });
    }
    for p in &own {
        out.push(FolderRef {
            role: FolderRole::SavesCore,
            platform: p.clone(),
            label: format!(
                "Saves of {} only",
                if d.shortname.is_empty() {
                    &d.id
                } else {
                    &d.shortname
                }
            ),
            rel_path: format!("Saves/{p}/{}", d.id),
        });
    }
    out.push(FolderRef {
        role: FolderRole::SaveStates,
        platform: main,
        label: "Save states".into(),
        rel_path: format!("Memories/Save States/{}", d.id),
    });
    out
}

const TAPE: &[&str] = &["tap", "tzx", "cas", "t64", "wav", "cdt", "p", "o"];
const DISK: &[&str] = &[
    "dsk", "adf", "d64", "d71", "d81", "g64", "ipf", "st", "msa", "img", "fdi", "hdf", "vhd",
    "iso", "cue", "chd", "mdf",
];

fn ext_of(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

/// `YYYYMMDD_HHMMSS` at the start of a name -> `YYYY-MM-DD HH:MM:SS`.
pub fn stamp_text(name: &str) -> Option<String> {
    let b = name.as_bytes();
    (b.len() >= 15 && b[8] == b'_' && b[..8].iter().chain(&b[9..15]).all(u8::is_ascii_digit)).then(
        || {
            format!(
                "{}-{}-{} {}:{}:{}",
                &name[0..4],
                &name[4..6],
                &name[6..8],
                &name[9..11],
                &name[11..13],
                &name[13..15]
            )
        },
    )
}

/// A state file is `<stamp>_USR_<hash>_<game>.sta`; the game name is what follows the hash.
fn state_game(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".sta").unwrap_or(name);
    let rest = stem.get(16..)?;
    let rest = rest
        .strip_prefix("USR_")
        .or_else(|| rest.strip_prefix("AUTO_"))
        .unwrap_or(rest);
    let game = rest.split_once('_').map(|(_, g)| g).unwrap_or(rest);
    (!game.is_empty()).then(|| game.to_string())
}

pub fn classify(role: FolderRole, name: &str, slots: &[SlotInfo]) -> (EntryKind, Option<String>) {
    match role {
        FolderRole::SaveStates => return (EntryKind::State, None),
        FolderRole::SavesShared | FolderRole::SavesCore => {
            let ext = ext_of(name);
            return (
                if ext == "zip" {
                    EntryKind::Other
                } else {
                    EntryKind::Save
                },
                slots
                    .iter()
                    .find(|s| s.nonvolatile && s.extensions.contains(&ext))
                    .map(|s| s.name.clone()),
            );
        }
        _ => {}
    }
    let ext = ext_of(name);
    if let Some(slot) = slots
        .iter()
        .find(|s| !s.filename.is_empty() && s.filename.eq_ignore_ascii_case(name))
    {
        return (EntryKind::Bios, Some(slot.name.clone()));
    }
    if let Some(slot) = slots.iter().find(|s| s.extensions.contains(&ext)) {
        let kind = if slot.name.to_ascii_lowercase().contains("bios") {
            EntryKind::Bios
        } else if slot.nonvolatile {
            EntryKind::Save
        } else if TAPE.contains(&ext.as_str()) {
            EntryKind::Tape
        } else if DISK.contains(&ext.as_str()) {
            EntryKind::Disk
        } else {
            EntryKind::Rom
        };
        return (kind, Some(slot.name.clone()));
    }
    (
        if TAPE.contains(&ext.as_str()) {
            EntryKind::Tape
        } else if DISK.contains(&ext.as_str()) {
            EntryKind::Disk
        } else {
            EntryKind::Other
        },
        None,
    )
}

fn modified(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// Walks `dir` for files (hidden and `._` skipped), at most [`MAX_DEPTH`] levels, returning relative paths sorted by name.
fn walk(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), String::new(), 0usize)];
    while let Some((path, rel, depth)) = stack.pop() {
        let Ok(read) = fs::read_dir(&path) else {
            continue;
        };
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let Ok(ft) = entry.file_type() else { continue };
            let child_rel = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if ft.is_dir() {
                if depth + 1 < MAX_DEPTH {
                    stack.push((entry.path(), child_rel, depth + 1));
                }
            } else if ft.is_file() {
                out.push((child_rel, entry.path()));
            }
        }
    }
    out.sort_by_key(|(r, _)| r.to_lowercase());
    out
}

pub fn list_folder(card: &Path, folder: &FolderRef, slots: &[SlotInfo]) -> FolderListing {
    let dir = card.join(&folder.rel_path);
    let exists = dir.is_dir();
    let files = if exists { walk(&dir) } else { Vec::new() };
    let mut entries = Vec::new();
    let mut total_bytes = 0u64;
    for (rel, path) in &files {
        let meta = fs::metadata(path).ok();
        let bytes = meta.as_ref().map_or(0, fs::Metadata::len);
        total_bytes += bytes;
        if entries.len() >= ENTRY_LIMIT {
            continue;
        }
        let name = rel.rsplit('/').next().unwrap_or(rel).to_string();
        let (kind, slot) = classify(folder.role, &name, slots);
        let (game, taken) = match folder.role {
            FolderRole::SaveStates => (state_game(&name), stamp_text(&name)),
            FolderRole::SavesShared | FolderRole::SavesCore => (
                Some(
                    name.rsplit_once('.')
                        .map_or(name.clone(), |(s, _)| s.to_string()),
                ),
                None,
            ),
            _ => (None, None),
        };
        let (group, purpose, purpose_source, support) = match folder.role {
            FolderRole::AssetsShared | FolderRole::AssetsCore => {
                let p = crate::file_purpose::describe(folder.role, rel, slots);
                (p.group, p.summary, p.source, p.support)
            }
            _ => (String::new(), String::new(), String::new(), false),
        };
        entries.push(Entry {
            rel_path: rel.clone(),
            name,
            bytes,
            modified: meta.as_ref().map_or(0, modified),
            kind,
            slot,
            game,
            taken,
            group,
            purpose,
            purpose_source,
            support,
        });
    }
    FolderListing {
        folder: folder.clone(),
        exists,
        entries,
        total_files: files.len(),
        total_bytes,
        truncated: files.len() > ENTRY_LIMIT,
    }
}

/// Required slots with a fixed file name (a BIOS) that are not on the card.
pub fn missing(card: &Path, d: &CoreDetail) -> Vec<MissingFile> {
    d.slots
        .iter()
        .filter(|s| s.required && !s.filename.is_empty())
        .filter_map(|s| {
            let folder = if s.core_specific {
                format!("Assets/{}/{}", s.platform, d.id)
            } else {
                format!("Assets/{}/common", s.platform)
            };
            let present = card.join(&folder).join(&s.filename).is_file()
                || card.join("Cores").join(&d.id).join(&s.filename).is_file();
            (!present).then(|| MissingFile {
                slot: s.name.clone(),
                filename: s.filename.clone(),
                folder_rel: folder,
                size_exact: s.size_exact,
            })
        })
        .collect()
}

pub fn save_backups(card: &Path) -> Vec<BackupZip> {
    let mut out: Vec<BackupZip> = fs::read_dir(card.join("Saves"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            (name.starts_with("pocket-sync-save-backup") && name.ends_with(".zip")).then(|| {
                let meta = e.metadata().ok();
                BackupZip {
                    name,
                    bytes: meta.as_ref().map_or(0, fs::Metadata::len),
                    modified: meta.as_ref().map_or(0, modified),
                }
            })
        })
        .collect();
    out.sort_by_key(|b| std::cmp::Reverse(b.modified));
    out
}

pub fn core_files(card: &Path, core_id: &str) -> Result<CoreFiles, TauError> {
    let d = corehub::core_detail(card, core_id)?;
    let folders = folders(&d)
        .iter()
        .map(|f| list_folder(card, f, &d.slots))
        .collect();
    Ok(CoreFiles {
        folders,
        missing: missing(card, &d),
        save_backups: save_backups(card),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corehub::testcard::card;

    fn listing<'a>(f: &'a CoreFiles, role: FolderRole, platform: &str) -> &'a FolderListing {
        f.folders
            .iter()
            .find(|l| l.folder.role == role && l.folder.platform == platform)
            .unwrap()
    }

    #[test]
    fn folders_files_and_kinds_for_a_core_that_reads_the_shared_folder() {
        let card = card("files");
        let f = core_files(&card, "Spiritualized.GBA").unwrap();
        let shared = listing(&f, FolderRole::AssetsShared, "gba");
        assert_eq!(shared.folder.rel_path, "Assets/gba/common");
        assert_eq!(shared.folder.label, "Shared by every Game Boy Advance core");
        // `._` companions and hidden files are never listed
        assert_eq!(
            shared
                .entries
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            ["Boktai 2 (USA).gba", "Mother 3.gba", "readme.txt"]
        );
        assert_eq!((shared.total_files, shared.total_bytes), (3, 300 + 200 + 5));
        let rom = &shared.entries[0];
        assert_eq!(
            (rom.kind, rom.slot.as_deref()),
            (EntryKind::Rom, Some("Cartridge"))
        );
        assert_eq!(
            (shared.entries[2].kind, shared.entries[2].slot.clone()),
            (EntryKind::Other, None)
        );
        // the core's own folder does not exist yet, and is still offered as a place to put files
        let own = listing(&f, FolderRole::AssetsCore, "gba");
        assert_eq!(
            (own.folder.rel_path.as_str(), own.exists, own.entries.len()),
            ("Assets/gba/Spiritualized.GBA", false, 0)
        );
    }

    #[test]
    fn saves_states_and_pocket_sync_backups_are_kept_apart() {
        let card = card("saves");
        let f = core_files(&card, "Spiritualized.GBA").unwrap();
        let saves = listing(&f, FolderRole::SavesShared, "gba");
        assert_eq!(
            (
                saves.entries[0].name.as_str(),
                saves.entries[0].kind,
                saves.entries[0].game.as_deref(),
                saves.entries[0].slot.as_deref()
            ),
            (
                "Boktai 2 (USA).sav",
                EntryKind::Save,
                Some("Boktai 2 (USA)"),
                Some("Save RAM")
            )
        );
        let states = f
            .folders
            .iter()
            .find(|l| l.folder.role == FolderRole::SaveStates)
            .unwrap();
        let s = &states.entries[0];
        assert_eq!(
            (s.kind, s.game.as_deref(), s.taken.as_deref()),
            (
                EntryKind::State,
                Some("Boktai 2 (USA)"),
                Some("2026-01-01 12:00:00")
            )
        );
        // the Pocket Sync zip is counted and is not a save
        assert_eq!(f.save_backups.len(), 1);
        assert!(
            f.folders
                .iter()
                .filter(|l| matches!(
                    l.folder.role,
                    FolderRole::SavesShared | FolderRole::SavesCore
                ))
                .all(|l| l.entries.iter().all(|e| !e.name.ends_with(".zip")))
        );
    }

    #[test]
    fn a_core_specific_slot_reads_its_own_folder() {
        let card = card("own");
        let f = core_files(&card, "other.GBAlt").unwrap();
        let own = listing(&f, FolderRole::AssetsCore, "gba");
        assert_eq!(own.entries.len(), 1);
        assert_eq!(own.entries[0].name, "Only Mine.gba");
        assert_eq!(own.entries[0].slot.as_deref(), Some("ROM"));
    }

    #[test]
    fn a_required_bios_is_missing_until_it_is_on_the_card() {
        let card = card("bios");
        let before = core_files(&card, "Spiritualized.GBA").unwrap();
        assert_eq!(before.missing.len(), 1);
        let m = &before.missing[0];
        assert_eq!(
            (
                m.slot.as_str(),
                m.filename.as_str(),
                m.folder_rel.as_str(),
                m.size_exact
            ),
            ("BIOS", "gba_bios.bin", "Assets/gba/common", Some(16384))
        );
        crate::corehub::testcard::write(
            &card.join("Assets/gba/common/gba_bios.bin"),
            &[0u8; 16384],
        );
        let after = core_files(&card, "Spiritualized.GBA").unwrap();
        assert!(after.missing.is_empty());
        let bios = listing(&after, FolderRole::AssetsShared, "gba")
            .entries
            .iter()
            .find(|e| e.name == "gba_bios.bin")
            .unwrap()
            .clone();
        assert_eq!(
            (bios.kind, bios.slot.as_deref()),
            (EntryKind::Bios, Some("BIOS"))
        );
        // a ROM slot with no fixed name is never "missing": the user picks it
        assert!(core_files(&card, "other.GBAlt").unwrap().missing.is_empty());
    }

    #[test]
    fn long_folders_are_cut_but_still_counted() {
        let card = card("long");
        for i in 0..(ENTRY_LIMIT + 7) {
            crate::corehub::testcard::write(
                &card.join(format!("Assets/gba/common/g{i:05}.gba")),
                &[0u8; 1],
            );
        }
        let f = core_files(&card, "Spiritualized.GBA").unwrap();
        let shared = listing(&f, FolderRole::AssetsShared, "gba");
        assert!(
            shared.truncated
                && shared.entries.len() == ENTRY_LIMIT
                && shared.total_files == ENTRY_LIMIT + 7 + 3
        );
    }

    #[test]
    fn stamps_and_state_names() {
        assert_eq!(
            stamp_text("20260101_120000_USR_00000000_x.sta").as_deref(),
            Some("2026-01-01 12:00:00")
        );
        assert_eq!(stamp_text("notastamp"), None);
        assert_eq!(
            state_game("20250103_101116_USR_00000000_Tetris 2 (USA, Europe) (Rev 1) (SGB Enh.sta")
                .as_deref(),
            Some("Tetris 2 (USA, Europe) (Rev 1) (SGB Enh")
        );
    }
}
