//! Removing an installed core from a card. `Cores/<id>` is always safe to
//! delete on its own, but the platform-wide files under `Assets/<platform>/`
//! and `Platforms/` are shared by every core on that platform -- deleting
//! them out from under a sibling core (exactly the shape tau-alpha itself
//! ships, e.g. `alfatreze.TAU` and `alfatreze.TAU_DIAGNOSTIC` both on
//! platform `tau`, sharing `Assets/tau/common`) would break it. [`plan_remove`]
//! checks every other core already found by [`crate::inspect_card`] before
//! deciding what is actually safe to remove; the platform-wide files are only
//! included in the plan when no other installed core shares that platform.
//! Follows the same plan -> review -> confirm -> execute shape as
//! [`crate::package`]/[`crate::playlist`].

use crate::{Card, ErrorCode, TauError};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RemovePlan {
    pub id: String,
    pub card_root: PathBuf,
    pub core_id: String,
    pub platform: String,
    /// True when another installed core still shares `platform`, in which
    /// case only this core's own files are planned for removal.
    pub platform_shared: bool,
    /// Paths relative to `card_root`, forward-slash separated, that would be
    /// removed (each is a file or a whole folder).
    pub paths: Vec<String>,
    pub files_to_remove: usize,
    pub bytes_to_remove: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RemoveReport {
    pub removed_files: usize,
    pub bytes_removed: u64,
}

/// Plans removing `core_id` from an already-inspected `card`. Touches no
/// filesystem state beyond reading sizes; nothing is deleted until
/// [`execute_remove`] is called with a matching confirmation token.
pub fn plan_remove(card: &Card, core_id: &str) -> Result<RemovePlan, TauError> {
    let core = card.cores.iter().find(|core| core.id == core_id).ok_or_else(|| {
        TauError::e(
            ErrorCode::NotFound,
            format!("{core_id}: no such installed core"),
        )
    })?;
    let platform = core.platform.clone();
    let platform_shared = !platform.is_empty()
        && card
            .cores
            .iter()
            .any(|other| other.id != core_id && other.platform == platform);

    let mut candidates = vec![PathBuf::from("Cores").join(core_id)];
    if !platform.is_empty() {
        let per_core_assets = PathBuf::from("Assets").join(&platform).join(core_id);
        if !platform_shared {
            // This core's own asset folder and the shared `common` folder are
            // normally the only two entries under `Assets/<platform>`; when
            // that holds, remove the whole platform folder in one path
            // instead of leaving an empty `Assets/<platform>` behind. If
            // something else is in there too, fall back to only the two
            // recognised sub-paths and leave the rest untouched.
            let platform_dir = PathBuf::from("Assets").join(&platform);
            let platform_dir_absolute = card.root.join(&platform_dir);
            let only_known_entries = fs::read_dir(&platform_dir_absolute)
                .map(|entries| {
                    entries.filter_map(Result::ok).all(|entry| {
                        matches!(entry.file_name().to_str(), Some(name) if name == core_id || name == "common")
                    })
                })
                .unwrap_or(false);
            if only_known_entries {
                candidates.push(platform_dir);
            } else {
                candidates.push(per_core_assets);
                candidates.push(PathBuf::from("Assets").join(&platform).join("common"));
            }
            candidates.push(PathBuf::from("Platforms").join(format!("{platform}.json")));
            candidates.push(
                PathBuf::from("Platforms")
                    .join("_images")
                    .join(format!("{platform}.bin")),
            );
        } else {
            candidates.push(per_core_assets);
        }
    }

    let mut paths = Vec::new();
    let mut files_to_remove = 0usize;
    let mut bytes_to_remove = 0u64;
    for candidate in candidates {
        let absolute = card.root.join(&candidate);
        if !absolute.exists() {
            continue;
        }
        let (files, bytes) = walk_size(&absolute)?;
        files_to_remove += files;
        bytes_to_remove += bytes;
        paths.push(candidate.to_string_lossy().replace('\\', "/"));
    }

    Ok(RemovePlan {
        id: plan_id(&card.root, core_id, &paths),
        card_root: card.root.clone(),
        core_id: core_id.to_string(),
        platform,
        platform_shared,
        paths,
        files_to_remove,
        bytes_to_remove,
    })
}

/// Deletes every path in a reviewed [`RemovePlan`]. Refuses unless
/// `confirmation` matches `plan.id` exactly. A path already gone since the
/// plan was made (e.g. a repeated confirm) is skipped rather than treated as
/// an error.
pub fn execute_remove(plan: &RemovePlan, confirmation: &str) -> Result<RemoveReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    let mut removed_files = 0usize;
    let mut bytes_removed = 0u64;
    for relative in &plan.paths {
        let absolute = plan.card_root.join(relative);
        if !absolute.exists() {
            continue;
        }
        let (files, bytes) = walk_size(&absolute)?;
        if absolute.is_dir() {
            fs::remove_dir_all(&absolute)?;
        } else {
            fs::remove_file(&absolute)?;
        }
        removed_files += files;
        bytes_removed += bytes;
    }
    Ok(RemoveReport {
        removed_files,
        bytes_removed,
    })
}

fn walk_size(path: &Path) -> Result<(usize, u64), TauError> {
    if path.is_file() {
        return Ok((1, fs::metadata(path)?.len()));
    }
    let mut files = 0usize;
    let mut bytes = 0u64;
    for entry in fs::read_dir(path)? {
        let entry_path = entry?.path();
        if entry_path.is_dir() {
            let (f, b) = walk_size(&entry_path)?;
            files += f;
            bytes += b;
        } else {
            files += 1;
            bytes += fs::metadata(&entry_path)?.len();
        }
    }
    Ok((files, bytes))
}

fn plan_id(card_root: &Path, core_id: &str, paths: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(card_root.to_string_lossy().as_bytes());
    hasher.update([0]);
    hasher.update(core_id.as_bytes());
    hasher.update([0]);
    for path in paths {
        hasher.update(path.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{inspect_card, package};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_zip() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/packages/alfatreze.TAU_0.4.0_2026-09-22.zip")
    }

    fn scratch_card(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tau-remove-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    /// Installs the real release zip, then duplicates its core folder and
    /// per-core asset folder under a second id on the same platform -- the
    /// same "two Tau cores share `Assets/tau/common`" shape tau-alpha's own
    /// audit trail records installing on real hardware (e.g.
    /// `alfatreze.TAU` + `alfatreze.TAU_DIAGNOSTIC`), not an invented layout.
    fn install_two_cores_sharing_a_platform(card: &Path) {
        let plan = package::plan_install(&fixture_zip(), card).unwrap();
        package::execute_install(&fixture_zip(), card, &plan, &plan.id).unwrap();

        let second_id = "alfatreze.TAU_SECOND";
        copy_dir(
            &card.join("Cores/alfatreze.TAU"),
            &card.join("Cores").join(second_id),
        );
        copy_dir(
            &card.join("Assets/tau/alfatreze.TAU"),
            &card.join("Assets/tau").join(second_id),
        );
    }

    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let dest = to.join(entry.file_name());
            if entry.path().is_dir() {
                copy_dir(&entry.path(), &dest);
            } else {
                fs::copy(entry.path(), dest).unwrap();
            }
        }
    }

    #[test]
    fn plans_removing_the_only_core_on_its_platform_as_not_shared() {
        let card = scratch_card("solo");
        let plan = package::plan_install(&fixture_zip(), &card).unwrap();
        package::execute_install(&fixture_zip(), &card, &plan, &plan.id).unwrap();

        let inspected = inspect_card(&card).unwrap();
        let remove_plan = plan_remove(&inspected, "alfatreze.TAU").unwrap();

        assert!(!remove_plan.platform_shared);
        assert!(remove_plan.paths.contains(&"Cores/alfatreze.TAU".to_string()));
        // `Assets/tau` holds only this core's own folder plus `common`, so
        // the whole platform folder collapses into one path rather than two.
        assert!(remove_plan.paths.contains(&"Assets/tau".to_string()));
        assert!(
            remove_plan
                .paths
                .contains(&"Platforms/tau.json".to_string())
        );
        assert!(
            remove_plan
                .paths
                .contains(&"Platforms/_images/tau.bin".to_string())
        );
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn removing_one_of_two_cores_on_a_shared_platform_keeps_the_shared_files() {
        let card = scratch_card("shared");
        install_two_cores_sharing_a_platform(&card);

        let inspected = inspect_card(&card).unwrap();
        assert_eq!(inspected.cores.len(), 2);
        let plan = plan_remove(&inspected, "alfatreze.TAU").unwrap();

        assert!(plan.platform_shared);
        assert!(plan.paths.contains(&"Cores/alfatreze.TAU".to_string()));
        assert!(
            plan.paths
                .contains(&"Assets/tau/alfatreze.TAU".to_string())
        );
        assert!(!plan.paths.contains(&"Assets/tau/common".to_string()));
        assert!(!plan.paths.contains(&"Platforms/tau.json".to_string()));

        let report = execute_remove(&plan, &plan.id).unwrap();
        assert!(report.removed_files > 0);
        assert!(!card.join("Cores/alfatreze.TAU").exists());
        assert!(!card.join("Assets/tau/alfatreze.TAU").exists());
        // The sibling core and the platform-wide files it still needs survive.
        assert!(card.join("Cores/alfatreze.TAU_SECOND").is_dir());
        assert!(card.join("Assets/tau/common").is_dir());
        assert!(card.join("Platforms/tau.json").is_file());

        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn removing_the_last_core_on_a_platform_removes_the_shared_files_too() {
        let card = scratch_card("last");
        let plan = package::plan_install(&fixture_zip(), &card).unwrap();
        package::execute_install(&fixture_zip(), &card, &plan, &plan.id).unwrap();

        let inspected = inspect_card(&card).unwrap();
        let remove_plan = plan_remove(&inspected, "alfatreze.TAU").unwrap();
        let report = execute_remove(&remove_plan, &remove_plan.id).unwrap();

        assert!(report.removed_files > 0);
        assert!(!card.join("Cores/alfatreze.TAU").exists());
        assert!(!card.join("Assets/tau").exists());
        assert!(!card.join("Platforms/tau.json").exists());
        assert!(!card.join("Platforms/_images/tau.bin").exists());

        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn plan_remove_refuses_an_unknown_core_id() {
        let card = scratch_card("missing");
        let plan = package::plan_install(&fixture_zip(), &card).unwrap();
        package::execute_install(&fixture_zip(), &card, &plan, &plan.id).unwrap();
        let inspected = inspect_card(&card).unwrap();

        let error = plan_remove(&inspected, "not.installed").unwrap_err();
        assert_eq!(error.code(), ErrorCode::NotFound);
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn execute_refuses_a_stale_confirmation_token() {
        let card = scratch_card("stale");
        let plan = package::plan_install(&fixture_zip(), &card).unwrap();
        package::execute_install(&fixture_zip(), &card, &plan, &plan.id).unwrap();
        let inspected = inspect_card(&card).unwrap();
        let remove_plan = plan_remove(&inspected, "alfatreze.TAU").unwrap();

        assert!(execute_remove(&remove_plan, "wrong").is_err());
        assert!(card.join("Cores/alfatreze.TAU").exists());
        fs::remove_dir_all(card).unwrap();
    }
}
