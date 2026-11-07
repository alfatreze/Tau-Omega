//! Putting meter preset sets on a card (`<media root>/tau-presets/<meter id>/`): the reviewed plan, the write and the undo.
//! The files themselves are `preset_files`. Same shape as every other card write (plan -> review -> confirm -> verify):
//! the plan lists what is new, changed, unchanged and removed and carries a token; the write refuses a token that is not the
//! plan's, takes a backup **outside the card**, writes each file next to its place, reads it back, swaps it in keeping the old
//! file until the new one is there, reads the folder back, and puts everything back if any step fails.
//!
//! What a write does in a meter folder: preset files first, the index last (so the index never names a file that is not
//! there), then the files of presets that are gone. Only `index.tpi` and `p<number>.tpr` are Omega's: anything else in the folder
//! is left alone. A meter is always left with an index, even an empty one: with no index at all an update would seed the factory
//! set again (`package::kept_for_user`), with an empty one the owner's choice of "DEFAULT only" stays.

use crate::{
    ErrorCode, TauError,
    preset_files::{self, DIR, INDEX_NAME},
    sync,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// One meter's folder as it should be: file name (`index.tpi`, `p3.tpr`) to bytes.
pub type Folder = BTreeMap<String, Vec<u8>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FileState {
    New,
    Changed,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PresetWrite {
    pub meter_id: u16,
    pub name: String,
    pub state: FileState,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PresetInstallPlan {
    /// Confirmation token: changes when the card's files or the new files change.
    pub id: String,
    pub media_root: PathBuf,
    pub meters: Vec<u16>,
    pub writes: Vec<PresetWrite>,
    /// Files of presets that are gone (`(meter id, file name)`), removed after the backup.
    pub removals: Vec<(u16, String)>,
    /// Files of the card that are backed up first: every one that is changed or removed.
    pub backup: Vec<(u16, String)>,
    pub unchanged: usize,
    /// A previous write was interrupted and left files to put back; running a write first does that.
    pub interrupted: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PresetInstallReport {
    pub written: usize,
    pub removed: usize,
    pub backup: Option<PathBuf>,
}

fn bad(code: ErrorCode, msg: impl Into<String>) -> TauError {
    TauError::e(code, msg)
}

/// Whether `name` is a file Omega manages in a meter folder.
pub fn is_managed(name: &str) -> bool {
    name == INDEX_NAME
        || name
            .strip_prefix('p')
            .and_then(|n| n.strip_suffix(".tpr"))
            .is_some_and(|n| !n.is_empty() && n.len() <= 3 && n.bytes().all(|b| b.is_ascii_digit()))
}

fn presets_dir(media_root: &Path) -> PathBuf {
    media_root.join(DIR)
}

fn meter_dir(media_root: &Path, meter_id: u16) -> PathBuf {
    presets_dir(media_root).join(meter_id.to_string())
}

/// The managed files of one meter folder on the card (empty when the folder is not there).
pub fn read_folder(media_root: &Path, meter_id: u16) -> Folder {
    let mut out = Folder::new();
    let mut old = Folder::new();
    if let Ok(entries) = fs::read_dir(meter_dir(media_root, meter_id)) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !e.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            // A `.old` beside a missing file is that file (an interrupted swap): read the folder as it will be once recovered.
            let (key, target) = match name.strip_suffix(".old") {
                Some(base) if is_managed(base) => (base.to_string(), &mut old),
                _ if is_managed(&name) => (name.clone(), &mut out),
                _ => continue,
            };
            if let Ok(bytes) = sync::read_back_bytes(&e.path()) {
                target.insert(key, bytes);
            }
        }
    }
    for (name, bytes) in old {
        out.entry(name).or_insert(bytes);
    }
    out
}

/// The meter ids that have a folder on the card.
pub fn meters_on_card(media_root: &Path) -> Vec<u16> {
    let mut v: Vec<u16> = fs::read_dir(presets_dir(media_root))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().to_string_lossy().parse().ok())
        .collect();
    v.sort_unstable();
    v
}

/// Refuses a folder the firmware would refuse: no index, an unreadable index, a listed preset with no file, a file that is not
/// that preset or not for that meter. Files of presets the index does not list are refused too (they would be orphans).
fn validate(meter_id: u16, files: &Folder) -> Result<(), TauError> {
    let index = files.get(INDEX_NAME).ok_or_else(|| {
        bad(
            ErrorCode::InvalidPathReference,
            format!("meter {meter_id}: no index.tpi"),
        )
    })?;
    let (idx_meter, entries) = preset_files::parse_index(index)?;
    if idx_meter != meter_id {
        return Err(bad(
            ErrorCode::InvalidPathReference,
            format!("meter {meter_id}: the index is for meter {idx_meter}"),
        ));
    }
    let mut want: Vec<String> = Vec::new();
    for e in &entries {
        let name = preset_files::preset_file_name(e.id);
        let bytes = files.get(&name).ok_or_else(|| {
            bad(
                ErrorCode::InvalidPathReference,
                format!("meter {meter_id}: {name} is listed but not given"),
            )
        })?;
        let file = preset_files::parse_preset(bytes)?;
        if file.id != e.id || file.meter_id != meter_id {
            return Err(bad(
                ErrorCode::InvalidPathReference,
                format!(
                    "meter {meter_id}: {name} is not preset {} of this meter",
                    e.id
                ),
            ));
        }
        want.push(name);
    }
    for name in files.keys() {
        if name != INDEX_NAME && !want.contains(name) {
            return Err(bad(
                ErrorCode::InvalidPathReference,
                format!("meter {meter_id}: {name} is not listed in the index"),
            ));
        }
        if !is_managed(name) {
            return Err(bad(
                ErrorCode::InvalidPathReference,
                format!("{name} is not a preset file name"),
            ));
        }
    }
    Ok(())
}

fn temp_name(name: &str) -> String {
    format!("{name}.part")
}
fn previous_name(name: &str) -> String {
    format!("{name}.old")
}

/// Puts back what an interrupted write left: a `.old` whose file is gone is the file; a `.part` is a half write.
fn recover(media_root: &Path) -> Result<(), TauError> {
    for meter in meters_on_card(media_root) {
        let dir = meter_dir(media_root, meter);
        for e in fs::read_dir(&dir)?.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(base) = name.strip_suffix(".old") {
                let live = dir.join(base);
                if live.is_file() {
                    let _ = fs::remove_file(e.path());
                } else if is_managed(base) {
                    fs::rename(e.path(), live)?;
                }
            } else if name.strip_suffix(".part").is_some_and(is_managed) {
                let _ = fs::remove_file(e.path());
            }
        }
    }
    Ok(())
}

fn leftovers(media_root: &Path) -> bool {
    meters_on_card(media_root).into_iter().any(|m| {
        fs::read_dir(meter_dir(media_root, m))
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.ends_with(".old") || n.ends_with(".part")
            })
    })
}

/// Plans writing `desired` (meter id to its folder) to the card. Reads the card only.
pub fn plan(
    media_root: &Path,
    desired: &BTreeMap<u16, Folder>,
) -> Result<PresetInstallPlan, TauError> {
    sync::validate_media_root(media_root)?;
    let mut writes = Vec::new();
    let mut removals = Vec::new();
    let mut backup = Vec::new();
    let mut unchanged = 0;
    let mut warnings = Vec::new();
    let mut token = String::from("presets|");
    token.push_str(&media_root.display().to_string());
    for (&meter, files) in desired {
        validate(meter, files)?;
        let card = read_folder(media_root, meter);
        if card.contains_key(INDEX_NAME) && preset_files::parse_index(&card[INDEX_NAME]).is_err() {
            warnings.push(format!(
                "Meter {meter}: the index on the card cannot be read (the Pocket ignores it); it will be backed up and replaced."
            ));
        }
        for (name, bytes) in files {
            let state = match card.get(name) {
                None => FileState::New,
                Some(old) if old == bytes => FileState::Unchanged,
                Some(_) => FileState::Changed,
            };
            token.push_str(&format!(
                "|{meter}/{name}:{}:{}",
                sync::sha256_bytes(bytes),
                card.get(name)
                    .map_or("none".into(), |o| sync::sha256_bytes(o))
            ));
            match state {
                FileState::Unchanged => unchanged += 1,
                FileState::Changed => backup.push((meter, name.clone())),
                FileState::New => {}
            }
            writes.push(PresetWrite {
                meter_id: meter,
                name: name.clone(),
                state,
                bytes: bytes.len() as u64,
            });
        }
        for (name, old) in &card {
            if !files.contains_key(name) {
                token.push_str(&format!("|-{meter}/{name}:{}", sync::sha256_bytes(old)));
                removals.push((meter, name.clone()));
                backup.push((meter, name.clone()));
            }
        }
    }
    let interrupted = leftovers(media_root);
    if interrupted {
        warnings.push("A previous write to the presets was interrupted; this one first puts back what it left.".into());
    }
    Ok(PresetInstallPlan {
        id: sync::sha256_bytes(token.as_bytes()),
        media_root: media_root.to_path_buf(),
        meters: desired.keys().copied().collect(),
        writes,
        removals,
        backup,
        unchanged,
        interrupted,
        warnings,
    })
}

enum Undo {
    Remove(PathBuf),
    Restore(PathBuf, Vec<u8>),
}

fn undo(steps: Vec<Undo>) {
    for step in steps.into_iter().rev() {
        match step {
            Undo::Remove(p) => {
                let _ = fs::remove_file(p);
            }
            Undo::Restore(p, bytes) => {
                let _ = fs::write(p, bytes);
            }
        }
    }
}

/// Writes a reviewed plan. `backup_root` must be outside the card and is required when the write changes or removes anything.
pub fn execute(
    media_root: &Path,
    desired: &BTreeMap<u16, Folder>,
    plan_in: &PresetInstallPlan,
    confirmation: &str,
    backup_root: Option<&Path>,
) -> Result<PresetInstallReport, TauError> {
    if confirmation != plan_in.id {
        return Err(bad(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    let card = media_root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap_or(media_root);
    if let Some(b) = backup_root
        && (b.as_os_str().is_empty() || sync::backup_is_inside(b, card))
    {
        return Err(bad(
            ErrorCode::UnsafeBackupLocation,
            "backup folder must be outside the card",
        ));
    }
    recover(media_root)?;
    let fresh = plan(media_root, desired)?;
    if fresh.id != plan_in.id {
        return Err(bad(
            ErrorCode::SourceChangedSincePlan,
            "the presets on the card or the new presets changed since the plan was reviewed",
        ));
    }
    if !fresh.backup.is_empty() && backup_root.is_none() {
        return Err(bad(
            ErrorCode::UnsafeBackupLocation,
            "a backup folder outside the card is required: this changes files that are already there",
        ));
    }
    // Back up first: nothing on the card is touched until every old file reads back from the backup.
    let mut backup_dir = None;
    if let Some(root) = backup_root
        && !fresh.backup.is_empty()
    {
        let base = root.join(&fresh.id).join(DIR);
        for (meter, name) in &fresh.backup {
            let old = fs::read(meter_dir(media_root, *meter).join(name))?;
            let dest = base.join(meter.to_string()).join(name);
            fs::create_dir_all(dest.parent().unwrap_or(&base))?;
            sync::write_durable(&dest, &old)?;
            if sync::sha256_bytes(&fs::read(&dest)?) != sync::sha256_bytes(&old) {
                return Err(bad(
                    ErrorCode::VerificationFailed,
                    "a backup did not read back the same, so nothing was changed",
                ));
            }
        }
        backup_dir = Some(base);
    }
    let mut undo_steps: Vec<Undo> = Vec::new();
    let mut written = 0;
    let mut removed = 0;
    let result = (|| -> Result<(), TauError> {
        for (&meter, files) in desired {
            let dir = meter_dir(media_root, meter);
            fs::create_dir_all(&dir)?;
            let current = read_folder(media_root, meter);
            // Preset files first, the index last.
            let mut order: Vec<&String> =
                files.keys().filter(|n| n.as_str() != INDEX_NAME).collect();
            order.extend(files.keys().filter(|n| n.as_str() == INDEX_NAME));
            for name in order {
                let bytes = &files[name];
                if current.get(name) == Some(bytes) {
                    continue;
                }
                let live = dir.join(name);
                let temp = dir.join(temp_name(name));
                sync::write_durable(&temp, bytes)?;
                if sync::read_back_bytes(&temp)? != *bytes {
                    let _ = fs::remove_file(&temp);
                    return Err(bad(
                        ErrorCode::VerificationFailed,
                        format!("{name} did not read back the same"),
                    ));
                }
                match current.get(name) {
                    Some(old) => undo_steps.push(Undo::Restore(live.clone(), old.clone())),
                    None => undo_steps.push(Undo::Remove(live.clone())),
                }
                sync::swap_in_file(&temp, &live, &previous_name(name))?;
                if sync::read_back_bytes(&live)? != *bytes {
                    return Err(bad(
                        ErrorCode::VerificationFailed,
                        format!("{name} on the card did not read back the same"),
                    ));
                }
                written += 1;
            }
            for (name, old) in &current {
                if !files.contains_key(name) {
                    undo_steps.push(Undo::Restore(dir.join(name), old.clone()));
                    fs::remove_file(dir.join(name))?;
                    removed += 1;
                }
            }
            // The folder as the Pocket will read it: every file is the one asked for and the set is consistent.
            let back = read_folder(media_root, meter);
            if &back != files {
                return Err(bad(
                    ErrorCode::VerificationFailed,
                    format!("meter {meter}: the folder did not read back as written"),
                ));
            }
            validate(meter, &back)?;
        }
        Ok(())
    })();
    // macOS leaves `._` stubs beside what it renames on exFAT/FAT (SAFETY_RULES 7): remove those of the names used here.
    for (&meter, files) in desired {
        let dir = meter_dir(media_root, meter);
        for name in files.keys() {
            for n in [name.clone(), temp_name(name), previous_name(name)] {
                let _ = fs::remove_file(dir.join(format!("._{n}")));
            }
        }
        let _ = fs::remove_file(presets_dir(media_root).join(format!("._{meter}")));
    }
    if let Err(e) = result {
        undo(undo_steps);
        let _ = recover(media_root);
        return Err(TauError {
            code: e.code,
            message: format!(
                "{} The presets on the card were put back as they were.",
                e.message
            ),
        });
    }
    Ok(PresetInstallReport {
        written,
        removed,
        backup: backup_dir,
    })
}
