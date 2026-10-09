//! The plan for installing or updating a core release package on a card: everything the install would do, decided
//! and listed before anything is written. Read-only; executing it is a separate step that must be handed the plan's
//! `id` (the same plan -> review -> confirm shape as [`crate::package`], [`crate::sync`] and [`crate::remove`]).
//!
//! The plan is the shared front door of scenarios 1a (first install onto a card with other cores) and 1c (update from
//! a zip, including one downloaded by the GitHub check). It combines:
//! - the update verdict and pairing check ([`crate::update::assess_update_with`]),
//! - the file plan ([`crate::package::plan_install`]): what is new, replaced, unchanged,
//! - **what to back up first**: the installed files that will be overwritten, and files a release manifest marks
//!   obsolete,
//! - **what to clean**: the five catalog caches (so the Pocket rescans and shows the core) and the `._` stubs Finder
//!   leaves beside files this install writes,
//! - **what is kept**: library index, theme file and media are never in a package, so never touched,
//! - whether it fits on the card, and whether the install is **refused** (a pair the Pocket would black-screen on, a
//!   core that does not match, a downgrade not explicitly allowed).

use crate::{
    TauError,
    compare::DifferenceState,
    compat::{CompatDoc, Role},
    package::{self, PackagePlan},
    storage::{self, CapacityCheck},
    update::{self, CATALOG_CACHES, UpdateReport, UpdateVerdict},
};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

/// Space left free on the card after the install, so it never fills the card completely.
const MARGIN_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BackupFile {
    /// Card-relative path of an installed file that will be overwritten or removed.
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InstallPlan {
    /// The confirmation token: a hash of everything below.
    pub id: String,
    /// The verdict, reasons and pairing result per core in the package.
    pub update: UpdateReport,
    /// The files the install writes (the same plan the package module executes).
    pub files: PackagePlan,
    /// Installed files to copy to a backup folder (outside the card) before they are replaced or removed.
    pub backup: Vec<BackupFile>,
    pub backup_bytes: u64,
    /// Files a release manifest marks obsolete and that are present on the card: removed after the backup.
    pub obsolete_to_remove: Vec<String>,
    /// Catalog caches present on the card, to be backed up and deleted so the Pocket rescans.
    pub caches_to_clear: Vec<String>,
    /// `._` stubs beside files this install writes, to be removed.
    pub stubs_to_sweep: Vec<String>,
    /// Library index and theme file found on the card: never touched.
    pub user_files_kept: Vec<String>,
    /// Other Tau test cores on the card that look superseded (numbered builds): offered for removal, never part of
    /// this plan's writes.
    pub superseded_candidates: Vec<String>,
    /// Free space on the card against what the install needs; `None` if the card could not be measured.
    pub capacity: Option<CapacityCheck>,
    /// Nothing differs: installing would change nothing.
    pub nothing_to_do: bool,
    /// `Some(reason)` when the install must not run as planned.
    pub refused: Option<String>,
    /// Things the user has to accept knowingly (downgrade, two builds of one day, unverifiable pairing).
    pub cautions: Vec<String>,
    pub allow_downgrade: bool,
}

/// Plans installing `zip_path` onto `card_root`. `docs` are the release manifests known to the caller (may be empty).
/// `allow_downgrade` is the user's explicit choice to install an older build; without it a downgrade is refused.
/// Writes nothing.
pub fn plan(
    zip_path: &Path,
    card_root: &Path,
    docs: &[CompatDoc],
    allow_downgrade: bool,
) -> Result<InstallPlan, TauError> {
    let update = update::assess_update_with(zip_path, card_root, docs)?;
    let files = update.plan.clone();
    let mut cautions = Vec::new();
    let mut refused = None;
    for core in &update.cores {
        match core.verdict {
            UpdateVerdict::Mismatch => {
                refused.get_or_insert_with(|| core.reasons.join(" "));
            }
            UpdateVerdict::Older if !allow_downgrade => {
                refused.get_or_insert_with(|| {
                    format!(
                        "{} The older build is only installed if you choose to downgrade.",
                        core.reasons.join(" ")
                    )
                });
            }
            UpdateVerdict::Older => cautions.push(format!("Downgrade: {}", core.reasons.join(" "))),
            UpdateVerdict::SameDateDifferentBuild => cautions.push(core.reasons.join(" ")),
            _ => {}
        }
        if matches!(
            core.pair,
            update::PairStatus::NoMarker | update::PairStatus::CannotVerify
        ) {
            cautions.push(format!(
                "{}: the firmware's match with the bitstream could not be checked.",
                core.package.shortname
            ));
        }
    }

    let nothing_to_do = update
        .cores
        .iter()
        .all(|c| c.verdict == UpdateVerdict::SameBuild)
        && files.new_files == 0
        && files.updated_files == 0;

    // Back up what will be overwritten, and what a manifest marks obsolete.
    let size_on_card = |path: &str| {
        fs::metadata(card_root.join(path))
            .map(|m| m.len())
            .unwrap_or(0)
    };
    let mut backup: Vec<BackupFile> = files
        .items
        .iter()
        .filter(|item| item.state == DifferenceState::Different)
        .map(|item| BackupFile {
            path: item.path.clone(),
            bytes: size_on_card(&item.path),
        })
        .collect();
    let mut obsolete_to_remove = Vec::new();
    for core in &update.cores {
        let Some(doc) = core
            .package_release
            .as_deref()
            .and_then(|tag| docs.iter().find(|d| d.release == tag))
        else {
            continue;
        };
        for package in doc
            .packages
            .iter()
            .filter(|p| p.core_id == core.package.core_id)
        {
            let owned: Vec<String> = package
                .layout
                .iter()
                .filter(|e| e.role == Role::Owned)
                .map(|e| e.path.clone())
                .collect();
            for entry in package
                .layout
                .iter()
                .filter(|e| e.role == Role::Obsolete && !e.pattern)
            {
                if !crate::cardlayout::obsolete_path_allowed(
                    card_root,
                    &core.package.core_id,
                    &core.package.platforms,
                    &owned,
                    &entry.path,
                ) {
                    if card_root.join(&entry.path).exists() {
                        cautions.push(format!(
                            "{} is marked obsolete but is outside {}'s own files, so it was left alone.",
                            entry.path, core.package.core_id
                        ));
                    }
                    continue;
                }
                if card_root.join(&entry.path).is_file()
                    && !obsolete_to_remove.contains(&entry.path)
                {
                    backup.push(BackupFile {
                        path: entry.path.clone(),
                        bytes: size_on_card(&entry.path),
                    });
                    obsolete_to_remove.push(entry.path.clone());
                }
            }
        }
    }
    let caches_to_clear: Vec<String> = CATALOG_CACHES
        .iter()
        .filter(|name| card_root.join(name).is_file())
        .map(|name| name.to_string())
        .collect();
    for cache in &caches_to_clear {
        backup.push(BackupFile {
            path: cache.clone(),
            bytes: size_on_card(cache),
        });
    }
    backup.sort_by(|a, b| a.path.cmp(&b.path));
    let backup_bytes = backup.iter().map(|b| b.bytes).sum();

    // `._` stubs beside the files this install writes.
    let mut stubs_to_sweep: Vec<String> = files
        .items
        .iter()
        .filter_map(|item| {
            let (dir, name) = item.path.rsplit_once('/')?;
            let stub = format!("{dir}/._{name}");
            card_root.join(&stub).is_file().then_some(stub)
        })
        .collect();
    stubs_to_sweep.sort();

    let capacity = storage::check_capacity(card_root, files.bytes_to_write, MARGIN_BYTES).ok();
    if let Some(check) = capacity.filter(|c| !c.fits) {
        refused.get_or_insert_with(|| {
            format!(
                "The card has {} bytes free and this install needs {} (plus a small margin).",
                check.space.available_bytes, check.bytes_needed
            )
        });
    }

    let kept = update.user_files_kept.clone();
    let superseded_candidates = superseded(card_root, &update);

    let mut hasher = Sha256::new();
    hasher.update(files.id.as_bytes());
    hasher.update([allow_downgrade as u8]);
    for list in [&obsolete_to_remove, &caches_to_clear, &stubs_to_sweep] {
        for item in list {
            hasher.update(item.as_bytes());
            hasher.update([0]);
        }
        hasher.update([1]);
    }
    for file in &backup {
        hasher.update(file.path.as_bytes());
        hasher.update(file.bytes.to_le_bytes());
    }
    let id = format!("{:x}", hasher.finalize());

    Ok(InstallPlan {
        id,
        update,
        files,
        backup,
        backup_bytes,
        obsolete_to_remove,
        caches_to_clear,
        stubs_to_sweep,
        user_files_kept: kept,
        superseded_candidates,
        capacity,
        nothing_to_do,
        refused,
        cautions,
        allow_downgrade,
    })
}

/// Numbered Tau test builds (`alfatreze.TAU_DEV_*`, `alfatreze.TAU_0_6_0_A_*`) other than the cores in this package.
fn superseded(card_root: &Path, update: &UpdateReport) -> Vec<String> {
    let ours: Vec<&str> = update
        .cores
        .iter()
        .map(|c| c.package.core_id.as_str())
        .collect();
    let Ok(read) = fs::read_dir(card_root.join("Cores")) else {
        return Vec::new();
    };
    let mut out: Vec<String> = read
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|id| !ours.contains(&id.as_str()))
        .filter(|id| {
            id.strip_prefix("alfatreze.TAU_").is_some_and(|rest| {
                rest.starts_with("DEV_") || rest.starts_with(|c: char| c.is_ascii_digit())
            })
        })
        .collect();
    out.sort();
    out
}

/// A plan is only as fresh as when it was made: whether every file would still be new, replaced or unchanged exactly
/// as planned. (The file plan's own `id` covers the zip and the destination, not the card's current contents, so the
/// per-file states are compared.)
pub fn still_current(
    plan: &InstallPlan,
    zip_path: &Path,
    card_root: &Path,
) -> Result<bool, TauError> {
    Ok(package::plan_install(zip_path, card_root)?.items == plan.files.items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn pkg(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/packages")
            .join(name)
    }
    fn alpha3() -> PathBuf {
        pkg("alfatreze.TAU_0.6.0_2026-10-04.zip")
    }
    fn alpha4() -> PathBuf {
        pkg("alfatreze.TAU_0.6.0_2026-10-07.zip")
    }
    fn old() -> PathBuf {
        pkg("alfatreze.TAU_0.4.0_2026-09-22.zip")
    }
    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tau-installplan-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                + crate::test_uniq()
        ));
        fs::create_dir_all(root.join("Cores")).unwrap();
        fs::create_dir_all(root.join("Assets")).unwrap();
        root
    }
    fn install(zip: &Path, card: &Path) {
        let p = package::plan_install(zip, card).unwrap();
        package::execute_install(zip, card, &p, &p.id).unwrap();
    }

    #[test]
    fn a_first_install_onto_a_card_with_other_cores_touches_only_what_is_new() {
        let card = scratch("first");
        // Someone else's core, and the stale catalog caches and stubs a real card carries.
        fs::create_dir_all(card.join("Cores/other.Core")).unwrap();
        fs::write(card.join("Cores/other.Core/core.json"), b"{}").unwrap();
        fs::create_dir_all(card.join("System")).unwrap();
        for cache in CATALOG_CACHES {
            fs::write(card.join(cache), b"old").unwrap();
        }
        fs::create_dir_all(card.join("Cores/alfatreze.TAU")).unwrap();
        fs::write(card.join("Cores/alfatreze.TAU/._core.json"), b"stub").unwrap();
        let plan = plan(&alpha4(), &card, &[], false).unwrap();
        assert_eq!(plan.update.cores[0].verdict, UpdateVerdict::NewInstall);
        assert!(plan.refused.is_none() && !plan.nothing_to_do);
        assert_eq!(plan.files.new_files, 15);
        assert!(
            plan.backup.iter().all(|b| b.path.starts_with("System/")),
            "nothing to overwrite, only caches: {:?}",
            plan.backup
        );
        assert_eq!(plan.caches_to_clear.len(), 5);
        assert_eq!(plan.stubs_to_sweep, vec!["Cores/alfatreze.TAU/._core.json"]);
        assert!(plan.capacity.is_some_and(|c| c.fits));
        assert!(plan.superseded_candidates.is_empty());
        assert!(
            !plan
                .files
                .items
                .iter()
                .any(|i| i.path.contains("other.Core"))
        );
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn an_update_backs_up_exactly_what_it_overwrites_and_keeps_the_user_files() {
        let card = scratch("update");
        install(&alpha3(), &card);
        let common = card.join("Assets/tau/common");
        fs::write(common.join("tau-library.tdb"), b"library").unwrap();
        fs::write(common.join("tau-assets.bin"), b"themes").unwrap();
        fs::create_dir_all(card.join("Cores/alfatreze.TAU_DEV_67")).unwrap();
        let plan = plan(&alpha4(), &card, &[], false).unwrap();
        assert_eq!(plan.update.cores[0].verdict, UpdateVerdict::Update);
        let backed: Vec<_> = plan.backup.iter().map(|b| b.path.as_str()).collect();
        assert!(
            backed.contains(&"Assets/tau/common/tau.rom")
                && backed.contains(&"Cores/alfatreze.TAU/core.json")
        );
        assert!(
            !backed
                .iter()
                .any(|p| p.contains("bitstream") || p.contains("tau-library")),
            "{backed:?}"
        );
        assert!(plan.backup.iter().all(|b| b.bytes > 0));
        assert_eq!(
            plan.backup_bytes,
            plan.backup.iter().map(|b| b.bytes).sum::<u64>()
        );
        assert_eq!(plan.user_files_kept.len(), 2);
        assert_eq!(plan.superseded_candidates, vec!["alfatreze.TAU_DEV_67"]);
        assert!(plan.cautions.is_empty());
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn the_same_build_has_nothing_to_do() {
        let card = scratch("same");
        install(&alpha4(), &card);
        let plan = plan(&alpha4(), &card, &[], false).unwrap();
        assert!(plan.nothing_to_do && plan.refused.is_none() && plan.backup.is_empty());
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_downgrade_is_refused_unless_the_user_chose_it() {
        let card = scratch("down");
        install(&alpha4(), &card);
        let refused = plan(&alpha3(), &card, &[], false).unwrap();
        assert!(
            refused
                .refused
                .as_ref()
                .is_some_and(|r| r.contains("downgrade")),
            "{:?}",
            refused.refused
        );
        let allowed = plan(&alpha3(), &card, &[], true).unwrap();
        assert!(allowed.refused.is_none());
        assert!(allowed.cautions.iter().any(|c| c.starts_with("Downgrade")));
        assert_ne!(
            refused.id, allowed.id,
            "the choice is part of what is confirmed"
        );
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_pair_the_pocket_would_refuse_is_refused() {
        let card = scratch("pair");
        let doc = crate::compat::parse_compat(
            serde_json::json!({"schema": 1, "release": "v0.6.0-alpha.4", "date_release": "2026-10-07",
                "packages": [{"zip": "x.zip", "zip_sha256": "00", "core_id": "alfatreze.TAU",
                    "bitstream_sha256": update::package_identity(&alpha4(), "alfatreze.TAU").unwrap().bitstream_sha256,
                    "bitstream_core_version": "4D503317", "rom_sha256": update::package_identity(&alpha4(), "alfatreze.TAU").unwrap().rom_sha256,
                    "cold_sha256": update::package_identity(&alpha4(), "alfatreze.TAU").unwrap().cold_sha256, "rom_accepts": ["4D50331A"], "rom_needs": []}],
                "requires_omega": {"min_omega": "0.4.0"}})
            .to_string()
            .as_bytes(),
        )
        .unwrap();
        let plan = plan(&alpha4(), &card, &[doc], false).unwrap();
        assert!(
            plan.refused
                .as_ref()
                .is_some_and(|r| r.contains("black screen")),
            "{:?}",
            plan.refused
        );
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_firmware_without_a_marker_installs_with_a_caution() {
        let card = scratch("nomarker");
        let plan = plan(&old(), &card, &[], false).unwrap();
        assert!(plan.refused.is_none());
        assert!(
            plan.cautions
                .iter()
                .any(|c| c.contains("could not be checked"))
        );
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_manifest_marks_obsolete_files_for_backup_and_removal() {
        let card = scratch("obsolete");
        install(&alpha3(), &card);
        fs::create_dir_all(card.join("Assets/tau/alfatreze.TAU")).unwrap();
        fs::write(
            card.join("Assets/tau/alfatreze.TAU/tau-old.bin"),
            b"left over",
        )
        .unwrap();
        fs::write(card.join("Assets/tau/common/song.mp3"), b"music").unwrap();
        fs::create_dir_all(card.join("Saves/tau")).unwrap();
        fs::write(card.join("Saves/tau/keep.sav"), b"save").unwrap();
        let reader = package::inspect(&alpha4()).unwrap();
        let layout: Vec<_> = reader.entries.iter()
            .map(|e| serde_json::json!({"path": e.path, "role": "owned", "sha256": e.sha256, "slot": null, "required": false}))
            .chain(["Assets/tau/alfatreze.TAU/tau-old.bin", "Assets/tau/common/song.mp3", "Saves/tau/keep.sav"].map(|p| serde_json::json!({"path": p, "role": "obsolete", "slot": null, "required": false})))
            .collect();
        let doc = crate::compat::parse_compat(
            serde_json::json!({"schema": 2, "release": "v0.6.0-alpha.4", "date_release": "2026-10-07",
                "packages": [{"zip": "x.zip", "zip_sha256": "00", "core_id": "alfatreze.TAU",
                    "bitstream_sha256": update::package_identity(&alpha4(), "alfatreze.TAU").unwrap().bitstream_sha256,
                    "bitstream_core_version": "4D50331A",
                    "rom_sha256": update::package_identity(&alpha4(), "alfatreze.TAU").unwrap().rom_sha256,
                    "cold_sha256": update::package_identity(&alpha4(), "alfatreze.TAU").unwrap().cold_sha256,
                    "rom_accepts": ["4D50331A"], "rom_needs": [], "layout": layout}],
                "requires_omega": {"min_omega": "0.4.0"}})
            .to_string()
            .as_bytes(),
        )
        .unwrap();
        let plan = plan(&alpha4(), &card, &[doc], false).unwrap();
        assert_eq!(
            plan.obsolete_to_remove,
            vec!["Assets/tau/alfatreze.TAU/tau-old.bin"],
            "only the core's own files are ever removed; media and saves are left alone"
        );
        assert!(
            plan.cautions
                .iter()
                .any(|c| c.contains("song.mp3") && c.contains("left alone"))
        );
        assert!(
            plan.backup
                .iter()
                .any(|b| b.path == "Assets/tau/alfatreze.TAU/tau-old.bin" && b.bytes == 9)
        );
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn the_plan_goes_stale_when_the_card_changes() {
        let card = scratch("stale");
        let first = plan(&alpha4(), &card, &[], false).unwrap();
        assert!(still_current(&first, &alpha4(), &card).unwrap());
        install(&alpha3(), &card);
        assert!(!still_current(&first, &alpha4(), &card).unwrap());
        fs::remove_dir_all(card).unwrap();
    }
}
