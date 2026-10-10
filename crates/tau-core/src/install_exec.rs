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
    // Folders this install will create (none of them exist yet): rollback removes exactly these, and no others.
    let created_dirs: std::collections::BTreeSet<String> = created
        .iter()
        .flat_map(|path| ancestors(path))
        .filter(|dir| !card_root.join(dir).exists())
        .collect();
    write_journal(
        &backup_dir,
        &json!({
            "version": 1, "state": "started", "plan_id": plan.id,
            "card": card_root.to_string_lossy(), "zip": zip_path.file_name().map(|n| n.to_string_lossy()),
            "files": backed, "created": created, "created_dirs": created_dirs,
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
        // The operating system creates a `._` companion for everything written (a file and each folder above it),
        // after the plan was made: sweep those too, so the card is left clean.
        let mut after_write = 0;
        for item in plan
            .files
            .items
            .iter()
            .filter(|i| i.state != DifferenceState::Identical)
        {
            for stub in stub_companions(&item.path) {
                if card_root.join(&stub).is_file() && !plan.stubs_to_sweep.contains(&stub) {
                    remove_file(card_root, &stub)?;
                    after_write += 1;
                }
            }
        }
        Ok((
            written,
            plan.obsolete_to_remove.len(),
            plan.caches_to_clear.len(),
            plan.stubs_to_sweep.len() + after_write,
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

/// The `._` companions macOS may create for a card-relative path: for the file itself and for every folder above it.
fn stub_companions(relative: &str) -> Vec<String> {
    let parts: Vec<&str> = relative.split('/').collect();
    (0..parts.len())
        .map(|i| {
            let dir = parts[..i].join("/");
            let prefix = if dir.is_empty() {
                String::new()
            } else {
                format!("{dir}/")
            };
            format!("{prefix}._{}", parts[i])
        })
        .collect()
}

/// Every folder above a card-relative file path (`a/b/c.txt` -> `a`, `a/b`).
fn ancestors(relative: &str) -> Vec<String> {
    let parts: Vec<&str> = relative.split('/').collect();
    (1..parts.len()).map(|i| parts[..i].join("/")).collect()
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
        // The OS creates a `._` companion for what it writes; one that was not there before this restore is ours to remove.
        let companion = stub_companions(path).pop();
        let had_companion = companion
            .as_ref()
            .is_some_and(|c| card_root.join(c).exists());
        sync::write_durable(&dest, &saved)?;
        if sync::sha256_bytes(&fs::read(&dest)?) != hash {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                format!("{path} did not read back the same after it was restored"),
            ));
        }
        if let (Some(companion), false) = (companion, had_companion) {
            remove_file(card_root, &companion)?;
        }
        restored += 1;
    }
    let mut created_removed = 0;
    let created: Vec<String> = journal
        .get("created")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    for path in &created {
        if path.split('/').any(|c| c == ".." || c.is_empty()) || path.starts_with('/') {
            return Err(refuse(format!(
                "the install journal names an unsafe path ({path})"
            )));
        }
        let file = card_root.join(path);
        if file.is_file() {
            fs::remove_file(&file)?;
            created_removed += 1;
        }
        // The OS's `._` companion of the file goes with it.
        if let Some(stub) = stub_companions(path).pop() {
            remove_file(card_root, &stub)?;
        }
    }
    // Folders the install created, deepest first, once only OS companions are left in them. (A journal written before
    // this was recorded falls back to every folder above a created file.)
    let mut dirs: Vec<String> = match journal.get("created_dirs").and_then(Value::as_array) {
        Some(list) => list
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        None => created.iter().flat_map(|p| ancestors(p)).collect(),
    };
    dirs.sort();
    dirs.dedup();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.matches('/').count()));
    for dir in &dirs {
        if dir.split('/').any(|c| c == ".." || c.is_empty()) || dir.starts_with('/') {
            return Err(refuse(format!(
                "the install journal names an unsafe folder ({dir})"
            )));
        }
        let path = card_root.join(dir);
        let Ok(read) = fs::read_dir(&path) else {
            continue;
        };
        let entries: Vec<_> = read.flatten().collect();
        if entries
            .iter()
            .all(|e| e.file_name().to_string_lossy().starts_with("._"))
        {
            for e in &entries {
                if e.path().is_file() {
                    fs::remove_file(e.path())?;
                }
            }
            if fs::remove_dir(&path).is_ok()
                && let Some(stub) = stub_companions(dir).pop()
            {
                remove_file(card_root, &stub)?;
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
mod tests;
