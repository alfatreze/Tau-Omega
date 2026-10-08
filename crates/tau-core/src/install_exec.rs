//! Carrying out a reviewed [`InstallPlan`]: verified backup outside the card, the file writes, removal of obsolete
//! files, catalog caches and `._` stubs, the post-install check, and a rollback from the backup.
//!
//! Order matters and every step is verified before the next begins:
//! 1. the confirmation token, the refusal flags, the backup location (a backup on the card is not a backup) and the
//!    plan's freshness are checked before anything is touched;
//! 2. everything the install will overwrite or remove is copied to `<backup_root>/<plan id>/` and **read back and
//!    hash-checked**, and a journal (`install-journal.json`) is written there *before* the first card write, listing
//!    the backed-up files and the files the install creates;
//! 3. the package files are written ([`package::execute_install`] verifies each against the zip before and after);
//! 4. obsolete files, catalog caches and `._` stubs are removed;
//! 5. [`update::post_install_check_with`] runs for each core and its report is returned.
//!
//! If step 3 or 4 fails the install is rolled back automatically from the backup. [`rollback`] can also be run later
//! from the journal alone: it restores every backed-up file (after re-verifying the backup's hash) and removes the
//! files the install created. The caller must hold the card write lock for the whole call.

use crate::{
    ErrorCode, TauError,
    compare::DifferenceState,
    compat::CompatDoc,
    install_plan::{self, InstallPlan},
    package, sync,
    update::{self, CheckStatus, PostInstallReport},
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

const JOURNAL: &str = "install-journal.json";

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InstallReport {
    /// Where the backup and journal are, for [`rollback`] and for the user.
    pub backup_dir: PathBuf,
    pub files_written: usize,
    pub bytes_written: u64,
    pub files_backed_up: usize,
    pub obsolete_removed: usize,
    pub caches_cleared: usize,
    pub stubs_swept: usize,
    /// The plan had nothing to change: nothing was written, no backup made.
    pub nothing_to_do: bool,
    /// One check report per core; look at each `verdict`.
    pub checks: Vec<PostInstallReport>,
}

impl InstallReport {
    /// True when no post-install check failed.
    pub fn ok(&self) -> bool {
        self.checks.iter().all(|c| c.verdict != CheckStatus::Fail)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RollbackReport {
    pub restored: usize,
    pub created_removed: usize,
}

fn refuse(message: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InstallRefused, message)
}

/// Runs `plan` against `card_root`. `backup_root` must be outside the card. See the module notes.
pub fn execute(
    zip_path: &Path,
    card_root: &Path,
    plan: &InstallPlan,
    confirmation: &str,
    backup_root: &Path,
    docs: &[CompatDoc],
) -> Result<InstallReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    if let Some(reason) = &plan.refused {
        return Err(refuse(reason.clone()));
    }
    if backup_root.as_os_str().is_empty() || sync::backup_is_inside(backup_root, card_root) {
        return Err(TauError::e(
            ErrorCode::UnsafeBackupLocation,
            "backup folder must be outside the card",
        ));
    }
    if !install_plan::still_current(plan, zip_path, card_root)? {
        return Err(TauError::e(
            ErrorCode::SourceChangedSincePlan,
            "the card changed since the plan was reviewed; plan again",
        ));
    }
    let backup_dir = backup_root.join(&plan.id);
    if plan.nothing_to_do {
        return Ok(InstallReport {
            backup_dir,
            files_written: 0,
            bytes_written: 0,
            files_backed_up: 0,
            obsolete_removed: 0,
            caches_cleared: 0,
            stubs_swept: 0,
            nothing_to_do: true,
            checks: Vec::new(),
        });
    }
    if backup_dir.join(JOURNAL).exists() {
        return Err(refuse(
            "this plan was already run once and its backup is still there; plan again",
        ));
    }

    // 2. Backup, verified, then the journal.
    let mut backed = Vec::new();
    for file in &plan.backup {
        let source = card_root.join(&file.path);
        let Ok(bytes) = fs::read(&source) else {
            continue; // gone since the plan: nothing to lose
        };
        let dest = backup_dir.join("files").join(&file.path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        sync::write_durable(&dest, &bytes)?;
        let hash = sync::sha256_bytes(&bytes);
        if sync::sha256_bytes(&fs::read(&dest)?) != hash {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                format!(
                    "the backup of {} did not read back the same, so nothing was changed",
                    file.path
                ),
            ));
        }
        backed.push(json!({"path": file.path, "sha256": hash, "bytes": bytes.len()}));
    }
    let created: Vec<&str> = plan
        .files
        .items
        .iter()
        .filter(|i| i.state == DifferenceState::OnlyLeft)
        .map(|i| i.path.as_str())
        .collect();
    write_journal(
        &backup_dir,
        &json!({
            "version": 1, "state": "started", "plan_id": plan.id,
            "card": card_root.to_string_lossy(), "zip": zip_path.file_name().map(|n| n.to_string_lossy()),
            "files": backed, "created": created,
        }),
    )?;

    // 3 and 4. The changes; any failure rolls everything back.
    let changes = || -> Result<(package::PackageReport, usize, usize, usize), TauError> {
        let written = package::execute_install(zip_path, card_root, &plan.files, &plan.files.id)?;
        for path in &plan.obsolete_to_remove {
            remove_file(card_root, path)?;
        }
        for path in &plan.caches_to_clear {
            remove_file(card_root, path)?;
        }
        for path in &plan.stubs_to_sweep {
            remove_file(card_root, path)?;
        }
        Ok((
            written,
            plan.obsolete_to_remove.len(),
            plan.caches_to_clear.len(),
            plan.stubs_to_sweep.len(),
        ))
    };
    let (written, obsolete_removed, caches_cleared, stubs_swept) = match changes() {
        Ok(done) => done,
        Err(error) => {
            return Err(match rollback(card_root, &backup_dir) {
                Ok(r) => TauError::e(
                    error.code(),
                    format!(
                        "{} The card was put back as it was ({} files restored, {} new files removed).",
                        error.message, r.restored, r.created_removed
                    ),
                ),
                Err(rb) => TauError::e(
                    error.code(),
                    format!(
                        "{} Putting the card back also failed ({}); the backup is in {}.",
                        error.message,
                        rb.message,
                        backup_dir.display()
                    ),
                ),
            });
        }
    };

    // 5. The shared post-install check, per core.
    let mut checks = Vec::new();
    for core in &plan.update.cores {
        checks.push(update::post_install_check_with(
            card_root,
            &core.package.core_id,
            Some(zip_path),
            docs,
        )?);
    }
    mark_done(&backup_dir)?;
    Ok(InstallReport {
        backup_dir,
        files_written: written.written,
        bytes_written: written.bytes_written,
        files_backed_up: backed.len(),
        obsolete_removed,
        caches_cleared,
        stubs_swept,
        nothing_to_do: false,
        checks,
    })
}

fn remove_file(card_root: &Path, relative: &str) -> Result<(), TauError> {
    let path = card_root.join(relative);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(TauError::e(
            ErrorCode::Io,
            format!("could not remove {relative}: {e}."),
        )),
    }
}

fn write_journal(backup_dir: &Path, value: &Value) -> Result<(), TauError> {
    fs::create_dir_all(backup_dir)?;
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| TauError::e(ErrorCode::Json, e.to_string()))?;
    sync::write_durable(&backup_dir.join(JOURNAL), &bytes)
}

fn mark_done(backup_dir: &Path) -> Result<(), TauError> {
    let mut journal = read_journal(backup_dir)?;
    journal["state"] = json!("done");
    write_journal(backup_dir, &journal)
}

fn read_journal(backup_dir: &Path) -> Result<Value, TauError> {
    let bytes = fs::read(backup_dir.join(JOURNAL))
        .map_err(|_| refuse("there is no install journal in that backup folder"))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| refuse(format!("the install journal is damaged: {e}")))
}

/// Puts the card back as it was before the install that made `backup_dir`: every backed-up file is restored (its
/// backup copy is hash-checked first and the restored file read back), then the files the install created are
/// removed. Safe to run twice.
pub fn rollback(card_root: &Path, backup_dir: &Path) -> Result<RollbackReport, TauError> {
    let journal = read_journal(backup_dir)?;
    if journal.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(refuse(
            "this install journal is from a version Omega does not know",
        ));
    }
    if journal.get("card").and_then(Value::as_str) != Some(&card_root.to_string_lossy()) {
        return Err(refuse("this backup was made for a different card folder"));
    }
    let mut restored = 0;
    for entry in journal
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(path), Some(hash)) = (
            entry.get("path").and_then(Value::as_str),
            entry.get("sha256").and_then(Value::as_str),
        ) else {
            return Err(refuse("the install journal has a malformed entry"));
        };
        if path.split('/').any(|c| c == ".." || c.is_empty()) || path.starts_with('/') {
            return Err(refuse(format!(
                "the install journal names an unsafe path ({path})"
            )));
        }
        let saved = fs::read(backup_dir.join("files").join(path))
            .map_err(|_| refuse(format!("the backup of {path} is missing")))?;
        if sync::sha256_bytes(&saved) != hash {
            return Err(refuse(format!(
                "the backup of {path} no longer matches its recorded hash; it was not restored"
            )));
        }
        let dest = card_root.join(path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        sync::write_durable(&dest, &saved)?;
        if sync::sha256_bytes(&fs::read(&dest)?) != hash {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                format!("{path} did not read back the same after it was restored"),
            ));
        }
        restored += 1;
    }
    let mut created_removed = 0;
    for path in journal
        .get("created")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if path.split('/').any(|c| c == ".." || c.is_empty()) || path.starts_with('/') {
            return Err(refuse(format!(
                "the install journal names an unsafe path ({path})"
            )));
        }
        let file = card_root.join(path);
        if file.is_file() {
            fs::remove_file(&file)?;
            created_removed += 1;
            // Leave no empty folders behind that only the install made.
            let mut dir = file.parent().map(Path::to_path_buf);
            while let Some(d) = dir {
                if d == card_root || fs::remove_dir(&d).is_err() {
                    break;
                }
                dir = d.parent().map(Path::to_path_buf);
            }
        }
    }
    let mut journal = journal;
    journal["state"] = json!("rolled_back");
    write_journal(backup_dir, &journal)?;
    Ok(RollbackReport {
        restored,
        created_removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::CATALOG_CACHES;
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
    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tau-installexec-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                + crate::test_uniq()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
    fn card(name: &str) -> PathBuf {
        let c = scratch(name).join("card");
        fs::create_dir_all(c.join("Cores")).unwrap();
        fs::create_dir_all(c.join("Assets")).unwrap();
        c
    }
    fn backups(card: &Path) -> PathBuf {
        card.parent().unwrap().join("backups")
    }
    fn install_raw(zip: &Path, card: &Path) {
        let p = package::plan_install(zip, card).unwrap();
        package::execute_install(zip, card, &p, &p.id).unwrap();
    }
    fn snapshot(root: &Path) -> Vec<(String, String)> {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
            let mut entries: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    out.push((
                        p.strip_prefix(root).unwrap().to_string_lossy().into_owned(),
                        sync::sha256_bytes(&fs::read(&p).unwrap()),
                    ));
                }
            }
        }
        let mut out = Vec::new();
        walk(root, root, &mut out);
        out
    }

    /// A card as it is in real life: an older Tau, the user's library and themes, a second core, stale caches, stubs.
    fn lived_in_card(name: &str) -> PathBuf {
        let card = card(name);
        install_raw(&alpha3(), &card);
        let common = card.join("Assets/tau/common");
        let index = crate::build_index(
            &crate::synth(3, 1, 1),
            &[],
            &crate::root_prefix(&common).unwrap(),
            &mut Vec::new(),
        )
        .unwrap();
        fs::write(common.join("tau-library.tdb"), &index).unwrap();
        fs::write(common.join("tau-assets.bin"), b"themes").unwrap();
        fs::write(common.join("song.mp3"), b"music").unwrap();
        fs::create_dir_all(card.join("Cores/other.Core")).unwrap();
        fs::write(card.join("Cores/other.Core/core.json"), b"{}").unwrap();
        fs::create_dir_all(card.join("System")).unwrap();
        for cache in CATALOG_CACHES {
            fs::write(card.join(cache), b"old cache").unwrap();
        }
        fs::write(common.join("._tau.rom"), b"stub").unwrap();
        card
    }

    #[test]
    fn an_update_backs_up_writes_cleans_checks_and_keeps_the_users_files() {
        let card = lived_in_card("update");
        let before = snapshot(&card);
        let index_bytes = fs::read(card.join("Assets/tau/common/tau-library.tdb")).unwrap();
        let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
        let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
        assert!(!report.nothing_to_do && report.ok(), "{:#?}", report.checks);
        assert!(report.files_written > 0 && report.files_backed_up >= 7);
        assert_eq!((report.caches_cleared, report.stubs_swept), (5, 1));
        let common = card.join("Assets/tau/common");
        assert_eq!(
            fs::read(common.join("tau-library.tdb")).unwrap(),
            index_bytes
        );
        assert_eq!(fs::read(common.join("tau-assets.bin")).unwrap(), b"themes");
        assert_eq!(fs::read(common.join("song.mp3")).unwrap(), b"music");
        assert!(fs::read(card.join("Cores/other.Core/core.json")).unwrap() == b"{}");
        assert!(CATALOG_CACHES.iter().all(|c| !card.join(c).exists()));
        assert!(!common.join("._tau.rom").exists());
        // The card now is the new build, and the journal says so.
        let again = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
        assert!(again.nothing_to_do);
        assert_ne!(snapshot(&card), before);
        let journal: Value =
            serde_json::from_slice(&fs::read(report.backup_dir.join(JOURNAL)).unwrap()).unwrap();
        assert_eq!(journal["state"], "done");
        assert!(report.backup_dir.starts_with(backups(&card)));
        fs::remove_dir_all(card.parent().unwrap()).unwrap();
    }

    #[test]
    fn rollback_puts_the_card_back_exactly_as_it_was() {
        let card = lived_in_card("rollback");
        let before = snapshot(&card);
        let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
        let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
        let undone = rollback(&card, &report.backup_dir).unwrap();
        assert!(undone.restored >= 7);
        // Everything except the swept `._` stub (junk, deliberately not restored) is byte-identical to before.
        let after = snapshot(&card);
        let without_stub = |v: &[(String, String)]| {
            v.iter()
                .filter(|(p, _)| !p.contains("/._"))
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(without_stub(&after), without_stub(&before));
        // Safe to run twice.
        rollback(&card, &report.backup_dir).unwrap();
        assert_eq!(without_stub(&snapshot(&card)), without_stub(&before));
        fs::remove_dir_all(card.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_first_install_rolls_back_to_an_untouched_card() {
        let card = card("first");
        fs::create_dir_all(card.join("System")).unwrap();
        fs::write(card.join(CATALOG_CACHES[0]), b"old").unwrap();
        let before = snapshot(&card);
        let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
        let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
        assert!(card.join("Cores/alfatreze.TAU/core.json").is_file());
        let undone = rollback(&card, &report.backup_dir).unwrap();
        assert_eq!(undone.created_removed, 15);
        assert_eq!(snapshot(&card), before);
        assert!(
            !card.join("Cores/alfatreze.TAU").exists(),
            "folders only the install made are gone too"
        );
        fs::remove_dir_all(card.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_failure_part_way_rolls_back_by_itself() {
        let card = lived_in_card("failure");
        let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
        // After planning, the stub the plan will sweep becomes a folder: removing it fails after the files were written.
        let stub = card.join("Assets/tau/common/._tau.rom");
        fs::remove_file(&stub).unwrap();
        fs::create_dir(&stub).unwrap();
        fs::write(stub.join("keep"), b"x").unwrap();
        let before = snapshot(&card);
        let error = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap_err();
        assert!(
            error.message.contains("put back as it was"),
            "{}",
            error.message
        );
        assert_eq!(snapshot(&card), before, "the card is exactly as it was");
        fs::remove_dir_all(card.parent().unwrap()).unwrap();
    }

    #[test]
    fn nothing_is_touched_when_the_checks_before_the_first_write_fail() {
        let card = lived_in_card("guards");
        let before = snapshot(&card);
        let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
        // A wrong token.
        assert_eq!(
            execute(&alpha4(), &card, &plan, "nope", &backups(&card), &[])
                .unwrap_err()
                .code(),
            ErrorCode::ConfirmationMismatch
        );
        // A backup on the card itself.
        assert_eq!(
            execute(
                &alpha4(),
                &card,
                &plan,
                &plan.id,
                &card.join("backups"),
                &[]
            )
            .unwrap_err()
            .code(),
            ErrorCode::UnsafeBackupLocation
        );
        // A plan that went stale.
        install_raw(&alpha4(), &card); // someone installed it meanwhile: every file's planned state flips
        let before_stale = snapshot(&card);
        assert_eq!(
            execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[])
                .unwrap_err()
                .code(),
            ErrorCode::SourceChangedSincePlan
        );
        assert_eq!(snapshot(&card), before_stale);
        assert!(!backups(&card).exists());
        let _ = before;
        // A refused plan (a downgrade nobody chose).
        install_raw(&alpha4(), &card);
        let down = install_plan::plan(&alpha3(), &card, &[], false).unwrap();
        let err = execute(&alpha3(), &card, &down, &down.id, &backups(&card), &[]).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InstallRefused);
        fs::remove_dir_all(card.parent().unwrap()).unwrap();
    }

    #[test]
    fn running_the_same_plan_twice_is_refused_the_second_time() {
        let card = lived_in_card("twice");
        let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
        execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
        // The card no longer matches the plan, so it is stale rather than silently repeated.
        let second = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap_err();
        assert_eq!(second.code(), ErrorCode::SourceChangedSincePlan);
        fs::remove_dir_all(card.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_damaged_backup_is_never_restored() {
        let card = lived_in_card("damaged");
        let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
        let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
        let victim = report.backup_dir.join("files/Assets/tau/common/tau.rom");
        fs::write(&victim, b"corrupted backup").unwrap();
        let installed = snapshot(&card);
        let err = rollback(&card, &report.backup_dir).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InstallRefused);
        assert!(err.message.contains("no longer matches"), "{}", err.message);
        // Files restored before the damaged one are fine; the damaged one was not written.
        assert_eq!(
            fs::read(card.join("Assets/tau/common/tau.rom")).unwrap(),
            snapshot_file(&installed, "Assets/tau/common/tau.rom", &card)
        );
        fs::remove_dir_all(card.parent().unwrap()).unwrap();
    }

    fn snapshot_file(_snap: &[(String, String)], rel: &str, card: &Path) -> Vec<u8> {
        fs::read(card.join(rel)).unwrap()
    }

    #[test]
    fn the_same_build_does_nothing_and_makes_no_backup() {
        let card = lived_in_card("same");
        let first = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
        execute(&alpha4(), &card, &first, &first.id, &backups(&card), &[]).unwrap();
        let again = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
        let report = execute(&alpha4(), &card, &again, &again.id, &backups(&card), &[]).unwrap();
        assert!(report.nothing_to_do && report.checks.is_empty());
        assert_eq!(
            fs::read_dir(backups(&card)).unwrap().count(),
            1,
            "only the first install made a backup"
        );
        fs::remove_dir_all(card.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_journal_for_another_card_is_not_applied() {
        let card = lived_in_card("other");
        let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
        let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
        let elsewhere = self::card("elsewhere");
        assert_eq!(
            rollback(&elsewhere, &report.backup_dir).unwrap_err().code(),
            ErrorCode::InstallRefused
        );
        fs::remove_dir_all(card.parent().unwrap()).unwrap();
        fs::remove_dir_all(elsewhere.parent().unwrap()).unwrap();
    }
}
