//! Removing files from a core: the Files tab's delete. A **plan first**, shown and confirmed, then for each file a verified copy in a
//! backup folder **outside the card**, and only then the delete. Nothing is deleted if its backup did not verify.
//!
//! Only files inside the core's own asset folders (`Assets/<platform>/common` and `Assets/<platform>/<core>`) can be named; saves, save
//! states and everything else are refused. A file in the shared folder is flagged with the other cores of the platform that read it,
//! and a file with the exact name of one of this core's required slots is flagged as needed.

use crate::{
    ErrorCode, TauError,
    core_files::{self, FolderRole},
    corehub,
    sync::sha256_file,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

pub const MAX_ITEMS: usize = 5_000;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RemoveItem {
    /// Relative to the card root, forward slashes.
    pub rel_path: String,
    pub bytes: u64,
    /// `Shared` files only: the other cores of the platform, which also read this file.
    pub shared_with: Vec<String>,
    /// This core's required slot that expects exactly this file name.
    pub needed_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RemoveFilesPlan {
    pub id: String,
    pub core_id: String,
    pub items: Vec<RemoveItem>,
    pub total_bytes: u64,
    /// Worth knowing, does not stop the run.
    pub warnings: Vec<String>,
    /// `Some(reason)` when the plan must not run.
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RemoveFilesReport {
    pub removed: usize,
    pub bytes_removed: u64,
    /// Where the removed files were copied first (outside the card).
    pub backup_dir: PathBuf,
}

fn bad_path(rel: &str) -> bool {
    rel.is_empty()
        || rel.starts_with('/')
        || rel.contains('\\')
        || rel
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == ".." || p.starts_with('.'))
}

/// Plans removing `rels` (card-relative paths) from `core_id`'s asset folders. Reads the card; writes nothing.
pub fn plan_remove_files(
    card: &Path,
    core_id: &str,
    rels: &[String],
) -> Result<RemoveFilesPlan, TauError> {
    let detail = corehub::core_detail(card, core_id)?;
    let folders: Vec<_> = core_files::folders(&detail)
        .into_iter()
        .filter(|f| matches!(f.role, FolderRole::AssetsShared | FolderRole::AssetsCore))
        .collect();
    let others: Vec<(String, Vec<String>)> = corehub::platform_tree(card)?
        .into_iter()
        .flat_map(|n| n.cores)
        .filter(|c| c.id != core_id)
        .map(|c| (c.id, c.platforms))
        .collect();
    let required: Vec<(String, String)> = detail
        .slots
        .iter()
        .filter(|s| s.required && !s.filename.is_empty())
        .map(|s| (s.filename.clone(), s.name.clone()))
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut items = Vec::new();
    let mut blocked = None;
    let mut hasher = Sha256::new();
    hasher.update(card.to_string_lossy().as_bytes());
    hasher.update([0]);
    hasher.update(core_id.as_bytes());
    for rel in rels {
        if !seen.insert(rel.clone()) {
            continue;
        }
        if bad_path(rel) {
            return Err(TauError::e(
                ErrorCode::InstallRefused,
                format!("{rel} is not a path this can remove."),
            ));
        }
        let Some(folder) = folders
            .iter()
            .find(|f| rel.starts_with(&format!("{}/", f.rel_path)))
        else {
            return Err(TauError::e(
                ErrorCode::InstallRefused,
                format!("{rel} is not in this core's files, so it is not removed."),
            ));
        };
        let path = card.join(rel);
        let meta = fs::symlink_metadata(&path).map_err(|_| {
            TauError::e(
                ErrorCode::InstallRefused,
                format!("{rel} is not on the card any more."),
            )
        })?;
        if !meta.is_file() {
            return Err(TauError::e(
                ErrorCode::InstallRefused,
                format!("{rel} is not a file."),
            ));
        }
        let name = rel.rsplit('/').next().unwrap_or(rel);
        let shared_with = if folder.role == FolderRole::AssetsShared {
            others
                .iter()
                .filter(|(_, p)| p.contains(&folder.platform))
                .map(|(id, _)| id.clone())
                .collect()
        } else {
            Vec::new()
        };
        let needed_by = required
            .iter()
            .find(|(f, _)| f.eq_ignore_ascii_case(name))
            .map(|(_, slot)| slot.clone());
        let mtime = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        hasher.update([0]);
        hasher.update(rel.as_bytes());
        hasher.update(meta.len().to_le_bytes());
        hasher.update(mtime.to_le_bytes());
        items.push(RemoveItem {
            rel_path: rel.clone(),
            bytes: meta.len(),
            shared_with,
            needed_by,
        });
    }
    if items.is_empty() {
        blocked = Some("Nothing is selected.".to_string());
    } else if items.len() > MAX_ITEMS {
        blocked = Some(format!("More than {MAX_ITEMS} files at once is too many."));
    }
    let mut warnings = Vec::new();
    let needed: Vec<&str> = items
        .iter()
        .filter(|i| i.needed_by.is_some())
        .map(|i| i.rel_path.rsplit('/').next().unwrap_or(""))
        .collect();
    if !needed.is_empty() {
        warnings.push(format!(
            "{} is a file this core needs to start.",
            needed.join(", ")
        ));
    }
    let shared = items.iter().filter(|i| !i.shared_with.is_empty()).count();
    if shared > 0 {
        warnings.push(format!(
            "{shared} file(s) are in a shared folder that other cores of the platform also read."
        ));
    }
    let total_bytes = items.iter().map(|i| i.bytes).sum();
    Ok(RemoveFilesPlan {
        id: format!("{:x}", hasher.finalize()),
        core_id: core_id.to_string(),
        items,
        total_bytes,
        warnings,
        blocked,
    })
}

/// Executes a reviewed plan. The plan is rebuilt and must have the same id (a file that changed since it was shown stops the run). Each
/// file is copied to `backup_root/<plan id>/<path>`, verified by SHA-256, and only then deleted.
pub fn execute_remove_files(
    card: &Path,
    core_id: &str,
    rels: &[String],
    confirmation: &str,
    backup_root: &Path,
) -> Result<RemoveFilesReport, TauError> {
    let plan = plan_remove_files(card, core_id, rels)?;
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "The confirmation does not match the current plan: something changed since it was shown. Review it again.",
        ));
    }
    if let Some(why) = &plan.blocked {
        return Err(TauError::e(ErrorCode::InstallRefused, why.clone()));
    }
    if backup_root.starts_with(card) {
        return Err(TauError::e(
            ErrorCode::InstallRefused,
            "The backup folder must be outside the card.",
        ));
    }
    let backup_dir = backup_root.join(&plan.id[..16]);
    let mut report = RemoveFilesReport {
        removed: 0,
        bytes_removed: 0,
        backup_dir: backup_dir.clone(),
    };
    for item in &plan.items {
        let path = card.join(&item.rel_path);
        let backup = backup_dir.join(&item.rel_path);
        if let Some(parent) = backup.parent() {
            fs::create_dir_all(parent)?;
        }
        let sha = sha256_file(&path)?;
        fs::copy(&path, &backup)?;
        if sha256_file(&backup)? != sha {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                format!(
                    "The backup of {} did not verify; it was not removed.",
                    item.rel_path
                ),
            ));
        }
        fs::remove_file(&path)?;
        // the `._` companion macOS leaves beside files on exFAT goes with it
        if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
            let _ = fs::remove_file(parent.join(format!("._{}", name.to_string_lossy())));
            prune_empty(parent, card, &plan, &item.rel_path);
        }
        report.removed += 1;
        report.bytes_removed += item.bytes;
    }
    Ok(report)
}

/// Removes folders emptied by the delete, up to (never including) the core's asset folder.
fn prune_empty(mut dir: &Path, card: &Path, _plan: &RemoveFilesPlan, rel: &str) {
    let parts: Vec<&str> = rel.split('/').collect();
    // Assets/<platform>/<folder>/... : the first three parts are the asset folder itself
    let mut depth = parts.len() - 1;
    while depth > 3 {
        let empty = fs::read_dir(dir).is_ok_and(|mut r| {
            r.all(|e| e.is_ok_and(|e| e.file_name().to_string_lossy().starts_with("._")))
        });
        if !empty || fs::remove_dir_all(dir).is_err() {
            break;
        }
        match dir.parent() {
            Some(p) if p.starts_with(card) => dir = p,
            _ => break,
        }
        depth -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corehub::testcard::{card, write};

    fn scratch_backup(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "tau-rmfiles-{name}-{}",
            u128::from(std::process::id()) + crate::test_uniq()
        ));
        let _ = fs::remove_dir_all(&p);
        p
    }

    fn rel(s: &str) -> Vec<String> {
        vec![s.to_string()]
    }

    #[test]
    fn a_file_is_backed_up_verified_and_then_removed() {
        let c = card("rm1");
        let backup = scratch_backup("rm1");
        let r = rel("Assets/gba/common/Mother 3.gba");
        let plan = plan_remove_files(&c, "Spiritualized.GBA", &r).unwrap();
        assert_eq!((plan.items.len(), plan.total_bytes), (1, 200));
        assert_eq!(plan.items[0].shared_with, vec!["other.GBAlt".to_string()]);
        assert!(plan.warnings[0].contains("shared folder"));
        let rep = execute_remove_files(&c, "Spiritualized.GBA", &r, &plan.id, &backup).unwrap();
        assert_eq!((rep.removed, rep.bytes_removed), (1, 200));
        assert!(!c.join("Assets/gba/common/Mother 3.gba").exists());
        assert!(!c.join("Assets/gba/common/._Mother 3.gba").exists());
        assert_eq!(
            fs::read(rep.backup_dir.join("Assets/gba/common/Mother 3.gba")).unwrap(),
            vec![2u8; 200]
        );
        // the neighbours are untouched
        assert!(c.join("Assets/gba/common/Boktai 2 (USA).gba").is_file());
    }

    #[test]
    fn only_the_cores_own_asset_files_can_be_named() {
        let c = card("rm2");
        for bad in [
            "Saves/gba/common/Boktai 2 (USA).sav",
            "Cores/Spiritualized.GBA/core.json",
            "Assets/gba/common/../../../Cores/x",
            "Assets/gba/other.GBAlt/Only Mine.gba",
            "/etc/passwd",
            "Assets/gba/common",
            "Assets/gba/common/.hidden",
        ] {
            assert!(
                plan_remove_files(&c, "Spiritualized.GBA", &rel(bad)).is_err(),
                "{bad}"
            );
        }
        // the other core's own folder is fine for that core
        assert!(
            plan_remove_files(
                &c,
                "other.GBAlt",
                &rel("Assets/gba/other.GBAlt/Only Mine.gba")
            )
            .is_ok()
        );
        let empty = plan_remove_files(&c, "Spiritualized.GBA", &[]).unwrap();
        assert!(empty.blocked.is_some());
    }

    #[test]
    fn a_stale_or_wrong_confirmation_and_an_inside_backup_remove_nothing() {
        let c = card("rm3");
        let backup = scratch_backup("rm3");
        let r = rel("Assets/gba/common/Mother 3.gba");
        let plan = plan_remove_files(&c, "Spiritualized.GBA", &r).unwrap();
        assert!(execute_remove_files(&c, "Spiritualized.GBA", &r, "nope", &backup).is_err());
        assert!(
            execute_remove_files(&c, "Spiritualized.GBA", &r, &plan.id, &c.join("bk")).is_err()
        );
        // a file that changed after the plan was shown
        write(&c.join("Assets/gba/common/Mother 3.gba"), &[5u8; 999]);
        assert!(execute_remove_files(&c, "Spiritualized.GBA", &r, &plan.id, &backup).is_err());
        assert!(c.join("Assets/gba/common/Mother 3.gba").is_file());
        assert!(!backup.exists());
    }

    #[test]
    fn a_required_file_is_flagged_and_emptied_folders_go() {
        let c = card("rm4");
        write(&c.join("Assets/gba/common/gba_bios.bin"), &[1u8; 16]);
        write(&c.join("Assets/gba/common/Sub/Deep/x.gba"), &[1u8; 4]);
        let backup = scratch_backup("rm4");
        let bios = rel("Assets/gba/common/gba_bios.bin");
        let plan = plan_remove_files(&c, "Spiritualized.GBA", &bios).unwrap();
        assert_eq!(plan.items[0].needed_by.as_deref(), Some("BIOS"));
        assert!(plan.warnings.iter().any(|w| w.contains("needs to start")));
        let deep = rel("Assets/gba/common/Sub/Deep/x.gba");
        let plan = plan_remove_files(&c, "Spiritualized.GBA", &deep).unwrap();
        execute_remove_files(&c, "Spiritualized.GBA", &deep, &plan.id, &backup).unwrap();
        assert!(!c.join("Assets/gba/common/Sub").exists());
        assert!(c.join("Assets/gba/common").is_dir());
    }
}
