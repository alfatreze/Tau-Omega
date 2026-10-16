//! Adding files (ROMs, BIOS, tapes, disks) to a core: the Files tab's drop target. A **plan first**, shown and confirmed, then a
//! verified copy; nothing is deleted.
//!
//! Destinations are exactly the two asset folders of one of the core's platforms: `Assets/<platform>/common` (shared by every core
//! of the platform) or `Assets/<platform>/<core>` (this core only). Names are kept as they are; a name the card cannot hold is refused
//! with the reason. A file already there is `Identical` (skipped) or `Different` (skipped unless the caller chose to replace it, in
//! which case the old file is first copied to a backup folder outside the card and verified). Every copy goes to a temporary name,
//! is read back from the card and compared by SHA-256, and only then renamed into place. Sources are never changed.

use crate::{
    ErrorCode, TauError,
    core_files::{self, FolderRole},
    corehub,
    storage::check_capacity,
    sync::{evict_cache, sha256_file},
};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Most files one plan may hold (a dropped folder of ROMs can be big; a runaway is a mistake).
pub const MAX_ITEMS: usize = 20_000;
const MAX_NAME_BYTES: usize = 255;
const MARGIN_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Destination {
    /// `Assets/<platform>/common`.
    Shared,
    /// `Assets/<platform>/<core>`.
    CoreOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AddState {
    New,
    /// Already there with the same content: skipped.
    Identical,
    /// There with different content: skipped unless the plan was made with `replace`.
    Different,
    Refused(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AddItem {
    pub source: PathBuf,
    /// Relative to the card root, forward slashes.
    pub dest_rel: String,
    pub bytes: u64,
    pub sha256: String,
    pub state: AddState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AddPlan {
    pub id: String,
    pub core_id: String,
    pub platform: String,
    pub destination: Destination,
    /// The folder, relative to the card.
    pub folder_rel: String,
    pub replace: bool,
    pub items: Vec<AddItem>,
    pub new_files: usize,
    pub identical: usize,
    /// Different files that will be replaced (`replace`) or skipped (not `replace`).
    pub different: usize,
    pub refused: usize,
    /// What will be written to the card (new files and replacements).
    pub bytes_to_write: u64,
    /// Things worth knowing that do not stop the add (an extension no slot of this core reads).
    pub warnings: Vec<String>,
    /// `Some(reason)` when the plan must not run (nothing to add, the card is too full).
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AddReport {
    pub written: usize,
    pub replaced: usize,
    pub skipped_identical: usize,
    pub skipped_different: usize,
    pub refused: usize,
    pub bytes_written: u64,
    /// Where replaced files were copied first (outside the card); `None` when nothing was replaced.
    pub backup_dir: Option<PathBuf>,
}

fn name_problem(part: &str) -> Option<String> {
    let bad = |c: char| {
        c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\' | '/')
    };
    if part.is_empty() || part == "." || part == ".." {
        Some("the name is empty".into())
    } else if part.len() > MAX_NAME_BYTES {
        Some(format!(
            "the name is longer than the card allows ({MAX_NAME_BYTES} bytes)"
        ))
    } else if part.chars().any(bad) {
        Some("the name has a character the card cannot hold (< > : \" | ? * \\ / or a control character)".into())
    } else if part.ends_with('.') || part.ends_with(' ') {
        Some("the name ends with a dot or a space, which the card cannot hold".into())
    } else {
        None
    }
}

/// The folder a chosen destination means, checked against the core's own folder list (so nothing else is ever a target).
fn target_folder(
    card: &Path,
    core_id: &str,
    platform: &str,
    destination: &Destination,
) -> Result<String, TauError> {
    let detail = corehub::core_detail(card, core_id)?;
    let role = match destination {
        Destination::Shared => FolderRole::AssetsShared,
        Destination::CoreOnly => FolderRole::AssetsCore,
    };
    core_files::folders(&detail)
        .into_iter()
        .find(|f| f.role == role && f.platform == platform)
        .map(|f| f.rel_path)
        .ok_or_else(|| {
            TauError::e(
                ErrorCode::InvalidPathReference,
                format!("{core_id} has no {platform} folder to add files to."),
            )
        })
}

/// Files under `root` (a dropped folder), never following symlinks, hidden files skipped.
/// Files found (path, name under the dropped folder) and entries that were skipped (path, why).
type Expanded = (Vec<(PathBuf, String)>, Vec<(PathBuf, String)>);

fn expand(root: &Path, limit: usize) -> Expanded {
    let top = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    let mut stack = vec![(root.to_path_buf(), top, 0usize)];
    while let Some((dir, rel, depth)) = stack.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let child = format!("{rel}/{name}");
            match entry.file_type() {
                Ok(t) if t.is_symlink() => {
                    skipped.push((path, "a link is not followed".to_string()))
                }
                Ok(t) if t.is_dir() => {
                    if depth < 8 {
                        stack.push((path, child, depth + 1));
                    } else {
                        skipped.push((
                            path,
                            "folders nested more than 8 levels are not added".to_string(),
                        ));
                    }
                }
                Ok(t) if t.is_file() => {
                    if files.len() < limit {
                        files.push((path, child));
                    }
                }
                _ => skipped.push((path, "not a regular file".to_string())),
            }
        }
    }
    files.sort_by_key(|(_, rel)| rel.to_lowercase());
    (files, skipped)
}

/// Plans adding `sources` (files, or folders whose files keep their folder name) to `core_id` on `card`.
pub fn plan_add(
    card: &Path,
    core_id: &str,
    platform: &str,
    destination: Destination,
    sources: &[PathBuf],
    replace: bool,
) -> Result<AddPlan, TauError> {
    let folder_rel = target_folder(card, core_id, platform, &destination)?;
    let detail = corehub::core_detail(card, core_id)?;
    let folder = card.join(&folder_rel);
    let mut candidates: Vec<(PathBuf, String)> = Vec::new();
    let mut items: Vec<AddItem> = Vec::new();
    let refuse = |items: &mut Vec<AddItem>, source: PathBuf, why: String| {
        items.push(AddItem {
            source,
            dest_rel: String::new(),
            bytes: 0,
            sha256: String::new(),
            state: AddState::Refused(why),
        });
    };
    for src in sources {
        match fs::symlink_metadata(src) {
            Err(e) => refuse(&mut items, src.clone(), format!("cannot be read: {e}")),
            Ok(m) if m.file_type().is_symlink() => {
                refuse(&mut items, src.clone(), "a link is not followed".into())
            }
            Ok(m) if m.is_file() => {
                let name = src
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if name.starts_with('.') {
                    continue;
                }
                candidates.push((src.clone(), name));
            }
            Ok(m) if m.is_dir() => {
                let (files, skipped) = expand(src, MAX_ITEMS);
                for (p, why) in skipped {
                    refuse(&mut items, p, why);
                }
                candidates.extend(files);
            }
            Ok(_) => refuse(
                &mut items,
                src.clone(),
                "not a regular file or folder".into(),
            ),
        }
    }
    if candidates.len() > MAX_ITEMS {
        return Err(TauError::e(
            ErrorCode::InstallRefused,
            format!("That is more than {MAX_ITEMS} files at once; add them in parts."),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut ext_unknown = std::collections::BTreeSet::new();
    let accepted: Vec<&str> = detail
        .slots
        .iter()
        .flat_map(|s| s.extensions.iter().map(String::as_str))
        .collect();
    for (src, rel) in candidates {
        if let Some(why) = rel.split('/').find_map(name_problem) {
            refuse(&mut items, src, why);
            continue;
        }
        let dest_rel = format!("{folder_rel}/{rel}");
        // names that differ only by case collide on the card
        if !seen.insert(dest_rel.to_ascii_lowercase()) {
            refuse(
                &mut items,
                src,
                "another file in this add has the same name (the card ignores case)".into(),
            );
            continue;
        }
        let meta = fs::metadata(&src)?;
        let sha = sha256_file(&src)?;
        let dest = folder.join(&rel);
        let state = match fs::metadata(&dest) {
            Ok(m) if m.is_file() => {
                if m.len() == meta.len() && sha256_file(&dest)? == sha {
                    AddState::Identical
                } else {
                    AddState::Different
                }
            }
            Ok(_) => AddState::Refused("a folder with this name is already there".into()),
            Err(_) => AddState::New,
        };
        if matches!(state, AddState::New | AddState::Different) {
            let ext = rel
                .rsplit_once('.')
                .map(|(_, e)| e.to_ascii_lowercase())
                .unwrap_or_default();
            if !accepted.is_empty()
                && !accepted.contains(&ext.as_str())
                && detail.slots.iter().all(|s| s.filename != rel)
            {
                ext_unknown.insert(if ext.is_empty() {
                    "(no extension)".to_string()
                } else {
                    format!(".{ext}")
                });
            }
        }
        items.push(AddItem {
            source: src,
            dest_rel,
            bytes: meta.len(),
            sha256: sha,
            state,
        });
    }
    let count = |f: &dyn Fn(&AddState) -> bool| items.iter().filter(|i| f(&i.state)).count();
    let new_files = count(&|s| *s == AddState::New);
    let identical = count(&|s| *s == AddState::Identical);
    let different = count(&|s| *s == AddState::Different);
    let refused = count(&|s| matches!(s, AddState::Refused(_)));
    let bytes_to_write: u64 = items
        .iter()
        .filter(|i| i.state == AddState::New || (replace && i.state == AddState::Different))
        .map(|i| i.bytes)
        .sum();
    let mut warnings = Vec::new();
    if !ext_unknown.is_empty() {
        warnings.push(format!(
            "No file slot of this core reads {}; the core may ignore {}.",
            ext_unknown.iter().cloned().collect::<Vec<_>>().join(", "),
            if ext_unknown.len() == 1 { "it" } else { "them" }
        ));
    }
    let to_do = new_files + if replace { different } else { 0 };
    let mut blocked = if to_do == 0 {
        Some(if refused > 0 {
            "Nothing can be added: every file was refused or is already there.".to_string()
        } else {
            "Nothing to add: everything is already on the card.".to_string()
        })
    } else {
        None
    };
    if blocked.is_none()
        && let Ok(cap) = check_capacity(card, bytes_to_write, MARGIN_BYTES)
        && !cap.fits
    {
        blocked = Some(format!(
            "The card has {} MB free and this needs {} MB (plus a small margin).",
            cap.space.available_bytes >> 20,
            bytes_to_write >> 20
        ));
    }
    let mut h = Sha256::new();
    for part in [
        card.to_string_lossy().as_ref(),
        core_id,
        platform,
        &folder_rel,
        if replace { "replace" } else { "keep" },
    ] {
        h.update(part.as_bytes());
        h.update([0]);
    }
    for i in &items {
        h.update(i.source.to_string_lossy().as_bytes());
        h.update([0]);
        h.update(i.dest_rel.as_bytes());
        h.update([0]);
        h.update(i.sha256.as_bytes());
        h.update([0]);
        h.update(format!("{:?}", i.state).as_bytes());
        h.update([0]);
    }
    Ok(AddPlan {
        id: format!("{:x}", h.finalize()),
        core_id: core_id.to_string(),
        platform: platform.to_string(),
        destination,
        folder_rel,
        replace,
        items,
        new_files,
        identical,
        different,
        refused,
        bytes_to_write,
        warnings,
        blocked,
    })
}

/// Copies `source` to `<dest>.omega-part`, returning the SHA-256 of what was read.
fn copy_to_part(source: &Path, part: &Path) -> Result<String, TauError> {
    let mut input = fs::File::open(source)?;
    let mut output = fs::File::create(part)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = io::Read::read(&mut input, &mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        io::Write::write_all(&mut output, &buf[..n])?;
    }
    output.sync_all()?;
    Ok(format!("{:x}", hash.finalize()))
}

/// Executes a reviewed plan. The plan is rebuilt from the same inputs and must have the same id, so a source or a card file that changed
/// since it was shown stops the run before the first write. `backup_root` is required when the plan replaces anything.
#[allow(clippy::too_many_arguments)]
pub fn execute_add(
    card: &Path,
    core_id: &str,
    platform: &str,
    destination: Destination,
    sources: &[PathBuf],
    replace: bool,
    confirmation: &str,
    backup_root: Option<&Path>,
) -> Result<AddReport, TauError> {
    let plan = plan_add(card, core_id, platform, destination, sources, replace)?;
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "The confirmation does not match the current plan: something changed since it was shown. Review it again.",
        ));
    }
    if let Some(why) = &plan.blocked {
        return Err(TauError::e(ErrorCode::InstallRefused, why.clone()));
    }
    let backup_dir = if replace && plan.different > 0 {
        let root = backup_root.ok_or_else(|| {
            TauError::e(
                ErrorCode::InstallRefused,
                "Replacing files needs a backup folder outside the card.",
            )
        })?;
        if root.starts_with(card) {
            return Err(TauError::e(
                ErrorCode::InstallRefused,
                "The backup folder must be outside the card.",
            ));
        }
        Some(root.join(&plan.id[..16]))
    } else {
        None
    };
    let mut report = AddReport {
        written: 0,
        replaced: 0,
        skipped_identical: plan.identical,
        skipped_different: if replace { 0 } else { plan.different },
        refused: plan.refused,
        bytes_written: 0,
        backup_dir: None,
    };
    for item in &plan.items {
        let replacing = item.state == AddState::Different && replace;
        if item.state != AddState::New && !replacing {
            continue;
        }
        let dest = card.join(&item.dest_rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        if replacing {
            // The old file is copied away and verified before anything of it is overwritten.
            let backup = backup_dir
                .as_ref()
                .expect("checked above")
                .join(&item.dest_rel);
            if let Some(parent) = backup.parent() {
                fs::create_dir_all(parent)?;
            }
            let old_sha = sha256_file(&dest)?;
            fs::copy(&dest, &backup)?;
            if sha256_file(&backup)? != old_sha {
                return Err(TauError::e(
                    ErrorCode::VerificationFailed,
                    format!(
                        "The backup of {} did not verify; nothing was replaced.",
                        item.dest_rel
                    ),
                ));
            }
            report.backup_dir = backup_dir.clone();
        }
        let part = dest.with_extension(format!("omega-part-{}", std::process::id()));
        let result = (|| -> Result<(), TauError> {
            let copied = copy_to_part(&item.source, &part)?;
            if copied != item.sha256 {
                return Err(TauError::e(
                    ErrorCode::SourceChangedSincePlan,
                    format!(
                        "{} changed while it was being copied.",
                        item.source.display()
                    ),
                ));
            }
            evict_cache(&part);
            if sha256_file(&part)? != item.sha256 {
                return Err(TauError::e(
                    ErrorCode::VerificationFailed,
                    format!(
                        "{} did not verify after copying to the card.",
                        item.dest_rel
                    ),
                ));
            }
            fs::rename(&part, &dest)?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = fs::remove_file(&part);
            return Err(e);
        }
        report.bytes_written += item.bytes;
        if replacing {
            report.replaced += 1
        } else {
            report.written += 1
        }
    }
    // macOS leaves `._` companions beside files it writes on exFAT; remove the ones this run could have made.
    let folder = card.join(&plan.folder_rel);
    crate::sync::sweep_appledouble(&folder);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corehub::testcard::{card, write};

    fn src(name: &str, data: &[u8]) -> PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let d = std::env::temp_dir().join(format!(
            "tau-add-src-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        let p = d.join(name);
        write(&p, data);
        p
    }

    fn plan(
        card: &Path,
        id: &str,
        dest: Destination,
        sources: &[PathBuf],
        replace: bool,
    ) -> AddPlan {
        plan_add(card, id, "gba", dest, sources, replace).unwrap()
    }

    #[test]
    fn a_new_rom_is_planned_added_verified_and_nothing_else_changes() {
        let c = card("add1");
        let rom = src("Metroid Fusion (USA).gba", &[5u8; 1000]);
        let p = plan(
            &c,
            "Spiritualized.GBA",
            Destination::Shared,
            std::slice::from_ref(&rom),
            false,
        );
        assert_eq!(
            (p.new_files, p.identical, p.different, p.refused),
            (1, 0, 0, 0)
        );
        assert_eq!(
            p.items[0].dest_rel,
            "Assets/gba/common/Metroid Fusion (USA).gba"
        );
        assert!(p.warnings.is_empty() && p.blocked.is_none() && p.bytes_to_write == 1000);
        let before = std::fs::read(c.join("Assets/gba/common/Mother 3.gba")).unwrap();
        let r = execute_add(
            &c,
            "Spiritualized.GBA",
            "gba",
            Destination::Shared,
            std::slice::from_ref(&rom),
            false,
            &p.id,
            None,
        )
        .unwrap();
        assert_eq!((r.written, r.bytes_written, r.replaced), (1, 1000, 0));
        assert_eq!(
            std::fs::read(c.join("Assets/gba/common/Metroid Fusion (USA).gba")).unwrap(),
            vec![5u8; 1000]
        );
        assert_eq!(
            std::fs::read(c.join("Assets/gba/common/Mother 3.gba")).unwrap(),
            before
        );
        // the file is listed, with its slot, straight away
        let listing = crate::core_files::core_files(&c, "Spiritualized.GBA").unwrap();
        assert!(
            listing
                .folders
                .iter()
                .flat_map(|f| &f.entries)
                .any(|e| e.name == "Metroid Fusion (USA).gba"
                    && e.slot.as_deref() == Some("Cartridge"))
        );
        // adding the same file again is a no-op
        let again = plan(&c, "Spiritualized.GBA", Destination::Shared, &[rom], false);
        assert_eq!((again.identical, again.new_files), (1, 0));
        assert!(
            again
                .blocked
                .as_deref()
                .unwrap()
                .contains("already on the card")
        );
    }

    #[test]
    fn the_core_only_folder_is_created_on_demand() {
        let c = card("add2");
        let rom = src("Mine.gba", b"abc");
        let p = plan(
            &c,
            "Spiritualized.GBA",
            Destination::CoreOnly,
            std::slice::from_ref(&rom),
            false,
        );
        assert_eq!(p.folder_rel, "Assets/gba/Spiritualized.GBA");
        execute_add(
            &c,
            "Spiritualized.GBA",
            "gba",
            Destination::CoreOnly,
            &[rom],
            false,
            &p.id,
            None,
        )
        .unwrap();
        assert!(c.join("Assets/gba/Spiritualized.GBA/Mine.gba").is_file());
    }

    // The file names under test (a colon, a trailing dot) cannot be created on Windows, so the test cannot set itself up there.
    #[cfg(unix)]
    #[test]
    fn a_dropped_folder_keeps_its_name_and_bad_names_are_refused_with_reasons() {
        let c = card("add3");
        let root = src("Game One.gba", b"1").parent().unwrap().to_path_buf();
        write(&root.join("Hacks/Game Two.gba"), b"2");
        write(&root.join("Hacks/bad:name.gba"), b"3");
        write(&root.join("Hacks/trailing.gba."), b"4");
        write(&root.join("Hacks/.hidden.gba"), b"5");
        let folder = root.join("Hacks");
        let p = plan(
            &c,
            "Spiritualized.GBA",
            Destination::Shared,
            &[folder, root.join("Game One.gba")],
            false,
        );
        let dests: Vec<&str> = p
            .items
            .iter()
            .filter(|i| i.state == AddState::New)
            .map(|i| i.dest_rel.as_str())
            .collect();
        assert_eq!(
            dests,
            [
                "Assets/gba/common/Hacks/Game Two.gba",
                "Assets/gba/common/Game One.gba"
            ]
        );
        assert_eq!(p.refused, 2);
        let reasons: Vec<String> = p
            .items
            .iter()
            .filter_map(|i| {
                if let AddState::Refused(w) = &i.state {
                    Some(w.clone())
                } else {
                    None
                }
            })
            .collect();
        assert!(
            reasons.iter().any(|r| r.contains("cannot hold"))
                && reasons.iter().any(|r| r.contains("dot or a space"))
        );
    }

    #[test]
    fn different_files_are_skipped_unless_replacing_and_replacing_backs_up_first() {
        let c = card("add4");
        let newer = src("Mother 3.gba", &[9u8; 200]); // same name as the one on the card, different content
        let keep = plan(
            &c,
            "Spiritualized.GBA",
            Destination::Shared,
            std::slice::from_ref(&newer),
            false,
        );
        assert_eq!((keep.different, keep.new_files), (1, 0));
        assert!(keep.blocked.is_some());
        let rep = plan(
            &c,
            "Spiritualized.GBA",
            Destination::Shared,
            std::slice::from_ref(&newer),
            true,
        );
        assert_eq!(rep.bytes_to_write, 200);
        // replacing needs a backup folder, outside the card
        assert!(
            execute_add(
                &c,
                "Spiritualized.GBA",
                "gba",
                Destination::Shared,
                std::slice::from_ref(&newer),
                true,
                &rep.id,
                None
            )
            .is_err()
        );
        assert!(
            execute_add(
                &c,
                "Spiritualized.GBA",
                "gba",
                Destination::Shared,
                std::slice::from_ref(&newer),
                true,
                &rep.id,
                Some(&c.join("bk"))
            )
            .is_err()
        );
        let bk = std::env::temp_dir().join(format!("tau-add-bk-{}", std::process::id()));
        let r = execute_add(
            &c,
            "Spiritualized.GBA",
            "gba",
            Destination::Shared,
            &[newer],
            true,
            &rep.id,
            Some(&bk),
        )
        .unwrap();
        assert_eq!((r.replaced, r.written), (1, 0));
        assert_eq!(
            std::fs::read(c.join("Assets/gba/common/Mother 3.gba")).unwrap(),
            vec![9u8; 200]
        );
        assert_eq!(
            std::fs::read(r.backup_dir.unwrap().join("Assets/gba/common/Mother 3.gba")).unwrap(),
            vec![2u8; 200]
        );
    }

    #[test]
    fn a_stale_plan_a_wrong_token_and_other_targets_are_refused() {
        let c = card("add5");
        let rom = src("A.gba", b"a");
        let p = plan(
            &c,
            "Spiritualized.GBA",
            Destination::Shared,
            std::slice::from_ref(&rom),
            false,
        );
        assert!(
            execute_add(
                &c,
                "Spiritualized.GBA",
                "gba",
                Destination::Shared,
                std::slice::from_ref(&rom),
                false,
                "wrong",
                None
            )
            .is_err()
        );
        // the source changed after the plan was shown
        std::fs::write(&rom, b"changed").unwrap();
        assert!(
            execute_add(
                &c,
                "Spiritualized.GBA",
                "gba",
                Destination::Shared,
                std::slice::from_ref(&rom),
                false,
                &p.id,
                None
            )
            .is_err()
        );
        assert!(
            !c.join("Assets/gba/common/A.gba").exists(),
            "nothing was written"
        );
        // only the core's own platform folders are targets, and only real cores
        assert!(
            plan_add(
                &c,
                "Spiritualized.GBA",
                "snes",
                Destination::Shared,
                std::slice::from_ref(&rom),
                false
            )
            .is_err()
        );
        assert!(
            plan_add(
                &c,
                "Spiritualized.GBA",
                "../x",
                Destination::Shared,
                std::slice::from_ref(&rom),
                false
            )
            .is_err()
        );
        assert!(plan_add(&c, "../x", "gba", Destination::Shared, &[rom], false).is_err());
    }

    #[test]
    fn an_extension_no_slot_reads_is_a_warning_and_links_are_not_followed() {
        let c = card("add6");
        let odd = src("notes.xyz", b"x");
        let p = plan(
            &c,
            "Spiritualized.GBA",
            Destination::Shared,
            std::slice::from_ref(&odd),
            false,
        );
        assert!(p.warnings[0].contains(".xyz") && p.blocked.is_none());
        #[cfg(unix)]
        {
            let link = odd.parent().unwrap().join("link.gba");
            std::os::unix::fs::symlink(&odd, &link).unwrap();
            let p = plan(&c, "Spiritualized.GBA", Destination::Shared, &[link], false);
            assert!(matches!(&p.items[0].state, AddState::Refused(w) if w.contains("link")));
        }
    }

    #[test]
    fn a_bios_dropped_clears_the_missing_list() {
        let c = card("add7");
        assert_eq!(
            crate::core_files::core_files(&c, "Spiritualized.GBA")
                .unwrap()
                .missing
                .len(),
            1
        );
        let bios = src("gba_bios.bin", &[0u8; 16384]);
        let p = plan(
            &c,
            "Spiritualized.GBA",
            Destination::Shared,
            std::slice::from_ref(&bios),
            false,
        );
        assert!(p.warnings.is_empty());
        execute_add(
            &c,
            "Spiritualized.GBA",
            "gba",
            Destination::Shared,
            &[bios],
            false,
            &p.id,
            None,
        )
        .unwrap();
        assert!(
            crate::core_files::core_files(&c, "Spiritualized.GBA")
                .unwrap()
                .missing
                .is_empty()
        );
    }
}
