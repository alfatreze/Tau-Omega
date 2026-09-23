//! Read-only inspection and install/update planning for a core release
//! package: a zip containing `Cores/<id>/*`, `Assets/<platform>/**`, and
//! `Platforms/*.json` -- the same shape [`crate::inspect_card`] already
//! reads from an installed card. Follows the same plan -> review -> confirm
//! -> execute shape as [`crate::sync`]/[`crate::playlist`]: [`plan_install`]
//! resolves and hashes everything up front and returns a content-hash `id`;
//! [`execute_install`] only proceeds when the caller's confirmation matches,
//! and re-verifies every entry against the same zip before writing so a zip
//! that changed on disk between planning and confirming is caught rather
//! than silently installed.
//!
//! Removing an installed core is [`crate::remove`], not this module: `Cores/<id>`
//! is safe to delete on its own, but more than one core can share an
//! `Assets/<platform>` folder, so a correct "remove" needs to check every
//! other installed core's `core.json` before touching shared media -- a
//! separate, smaller piece of work from install/update.

use crate::{compare::DifferenceState, ErrorCode, TauError};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PackageEntry {
    /// Path inside the zip, forward-slash separated (e.g.
    /// `"Cores/alfatreze.TAU/core.json"`), also the path relative to a card
    /// root this entry would be written to.
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

/// What a package zip declares, without touching any card.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PackageManifest {
    pub source: PathBuf,
    /// Core ids found as immediate subfolders of `Cores/` in the zip.
    pub core_ids: Vec<String>,
    pub entries: Vec<PackageEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PackageItem {
    pub path: String,
    pub state: DifferenceState,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PackagePlan {
    pub id: String,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub items: Vec<PackageItem>,
    pub new_files: usize,
    pub updated_files: usize,
    pub unchanged_files: usize,
    pub bytes_to_write: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PackageReport {
    pub written: usize,
    pub unchanged: usize,
    pub bytes_written: u64,
}

/// Reads every entry in a package zip and hashes its decompressed content,
/// without writing anything anywhere.
pub fn inspect(zip_path: &Path) -> Result<PackageManifest, TauError> {
    let entries = read_entries(zip_path)?;
    let mut core_ids: Vec<String> = entries
        .iter()
        .filter_map(|entry| entry.path.strip_prefix("Cores/"))
        .filter_map(|rest| rest.split('/').next())
        .map(str::to_string)
        .collect();
    core_ids.sort();
    core_ids.dedup();
    Ok(PackageManifest {
        source: zip_path.to_path_buf(),
        core_ids,
        entries,
    })
}

/// Plans installing or updating a package: every entry that is new or
/// different at `card_root` would be written; unchanged entries are left
/// alone. `card_root` need not exist yet -- a first-ever install commonly
/// starts from nothing.
pub fn plan_install(zip_path: &Path, card_root: &Path) -> Result<PackagePlan, TauError> {
    let entries = read_entries(zip_path)?;
    let mut items = Vec::with_capacity(entries.len());
    let (mut new_files, mut updated_files, mut unchanged_files) = (0, 0, 0);
    let mut bytes_to_write = 0u64;
    for entry in &entries {
        let destination = card_root.join(&entry.path);
        let state = match fs::read(&destination) {
            Ok(existing) if sha256_bytes(&existing) == entry.sha256 => {
                unchanged_files += 1;
                DifferenceState::Identical
            }
            Ok(_) => {
                updated_files += 1;
                bytes_to_write += entry.bytes;
                DifferenceState::Different
            }
            Err(_) => {
                new_files += 1;
                bytes_to_write += entry.bytes;
                DifferenceState::OnlyLeft
            }
        };
        items.push(PackageItem {
            path: entry.path.clone(),
            state,
            bytes: entry.bytes,
        });
    }
    Ok(PackagePlan {
        id: plan_id(zip_path, card_root, &entries),
        source: zip_path.to_path_buf(),
        destination: card_root.to_path_buf(),
        items,
        new_files,
        updated_files,
        unchanged_files,
        bytes_to_write,
    })
}

/// Writes a reviewed [`PackagePlan`]: extracts every new/updated entry from
/// the same zip to `card_root`, verifying each entry's hash against the zip
/// immediately before writing it (catches a zip that changed on disk since
/// the plan was made) and again after (catches a truncated/failed write).
/// Refuses unless `confirmation` matches `plan.id` exactly.
pub fn execute_install(
    zip_path: &Path,
    card_root: &Path,
    plan: &PackagePlan,
    confirmation: &str,
) -> Result<PackageReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    let entries: BTreeMap<String, PackageEntry> = read_entries(zip_path)?
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect();
    let mut written = 0;
    let mut bytes_written = 0u64;
    let mut unchanged = 0;
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| TauError::e(ErrorCode::Io, format!("cannot read package zip: {error}")))?;
    for item in &plan.items {
        if item.state == DifferenceState::Identical {
            unchanged += 1;
            continue;
        }
        let entry = entries.get(&item.path).ok_or_else(|| {
            TauError::e(
                ErrorCode::SourceChangedSincePlan,
                format!("{}: no longer in this package zip", item.path),
            )
        })?;
        let mut zip_entry = archive.by_name(&item.path).map_err(|error| {
            TauError::e(
                ErrorCode::SourceChangedSincePlan,
                format!("{}: {error}", item.path),
            )
        })?;
        let mut data = Vec::with_capacity(entry.bytes as usize);
        zip_entry
            .read_to_end(&mut data)
            .map_err(TauError::from)?;
        if sha256_bytes(&data) != entry.sha256 {
            return Err(TauError::e(
                ErrorCode::SourceChangedSincePlan,
                format!("{}: content changed since the plan was made", item.path),
            ));
        }
        let destination = card_root.join(&item.path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&destination, &data)?;
        let written_back = fs::read(&destination)?;
        if sha256_bytes(&written_back) != entry.sha256 {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                format!("{}: written file does not match the package", item.path),
            ));
        }
        written += 1;
        bytes_written += entry.bytes;
    }
    Ok(PackageReport {
        written,
        unchanged,
        bytes_written,
    })
}

fn read_entries(zip_path: &Path) -> Result<Vec<PackageEntry>, TauError> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| TauError::e(ErrorCode::Io, format!("cannot read package zip: {error}")))?;
    let mut entries = Vec::new();
    for i in 0..archive.len() {
        let mut zip_entry = archive
            .by_index(i)
            .map_err(|error| TauError::e(ErrorCode::Io, error.to_string()))?;
        if zip_entry.is_dir() {
            continue;
        }
        let path = zip_entry.name().replace('\\', "/");
        let mut data = Vec::with_capacity(zip_entry.size() as usize);
        zip_entry.read_to_end(&mut data).map_err(TauError::from)?;
        entries.push(PackageEntry {
            bytes: data.len() as u64,
            sha256: sha256_bytes(&data),
            path,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

fn sha256_bytes(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn plan_id(zip_path: &Path, card_root: &Path, entries: &[PackageEntry]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(zip_path.to_string_lossy().as_bytes());
    hasher.update([0]);
    hasher.update(card_root.to_string_lossy().as_bytes());
    hasher.update([0]);
    for entry in entries {
        hasher.update(entry.path.as_bytes());
        hasher.update([0]);
        hasher.update(entry.sha256.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/packages/alfatreze.TAU_0.4.0_2026-09-22.zip")
    }

    fn scratch_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tau-package-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn inspects_the_real_release_zip() {
        let manifest = inspect(&fixture()).unwrap();
        assert_eq!(manifest.core_ids, vec!["alfatreze.TAU"]);
        assert_eq!(manifest.entries.len(), 15);
        let core_json = manifest
            .entries
            .iter()
            .find(|entry| entry.path == "Cores/alfatreze.TAU/core.json")
            .unwrap();
        assert_eq!(core_json.bytes, 891);
    }

    #[test]
    fn plans_a_first_install_as_all_new() {
        let card = scratch_root("first-install");
        let plan = plan_install(&fixture(), &card).unwrap();
        assert_eq!(plan.new_files, 15);
        assert_eq!(plan.updated_files, 0);
        assert_eq!(plan.unchanged_files, 0);
        assert_eq!(
            plan.bytes_to_write,
            plan.items.iter().map(|item| item.bytes).sum::<u64>()
        );
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn install_then_plan_again_sees_everything_unchanged() {
        let card = scratch_root("install-then-replan");
        let plan = plan_install(&fixture(), &card).unwrap();
        let report = execute_install(&fixture(), &card, &plan, &plan.id).unwrap();
        assert_eq!(report.written, 15);
        assert_eq!(report.unchanged, 0);
        assert_eq!(report.bytes_written, plan.bytes_to_write);
        assert!(card.join("Cores/alfatreze.TAU/core.json").is_file());
        assert!(card.join("Platforms/_images/tau.bin").is_file());

        let replan = plan_install(&fixture(), &card).unwrap();
        assert_eq!(replan.new_files, 0);
        assert_eq!(replan.updated_files, 0);
        assert_eq!(replan.unchanged_files, 15);
        assert_eq!(replan.bytes_to_write, 0);
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn plans_an_update_when_one_file_differs() {
        let card = scratch_root("update");
        let plan = plan_install(&fixture(), &card).unwrap();
        execute_install(&fixture(), &card, &plan, &plan.id).unwrap();
        fs::write(card.join("Cores/alfatreze.TAU/core.json"), b"stale").unwrap();

        let replan = plan_install(&fixture(), &card).unwrap();
        assert_eq!(replan.updated_files, 1);
        assert_eq!(replan.unchanged_files, 14);
        let report = execute_install(&fixture(), &card, &replan, &replan.id).unwrap();
        assert_eq!(report.written, 1);
        assert_eq!(report.unchanged, 14);
        let restored = fs::read(card.join("Cores/alfatreze.TAU/core.json")).unwrap();
        assert_ne!(restored, b"stale");
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn execute_refuses_a_stale_confirmation_token() {
        let card = scratch_root("token");
        let plan = plan_install(&fixture(), &card).unwrap();
        assert!(execute_install(&fixture(), &card, &plan, "wrong").is_err());
        assert!(!card.join("Cores/alfatreze.TAU/core.json").exists());
        fs::remove_dir_all(card).unwrap();
    }
}
