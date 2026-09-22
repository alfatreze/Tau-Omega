//! Host-side, durable job reports. A journal is always outside a card's media
//! root, so recovery information remains available if a card is removed.

use crate::{ErrorCode, ProgressObserver, TauError, Warning, sync};
use serde_json::json;
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

/// Executes a reviewed non-mirror plan while recording its lifecycle in a
/// user-selected host file. The journal is deliberately written before card
/// mutation; if it cannot be recorded, execution never starts.
pub fn execute_to_journal(
    plan: &sync::SyncPlan,
    confirmation: &str,
    journal_path: &Path,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<sync::SyncReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    validate_location(journal_path, &plan.destination)?;
    write_state(plan, journal_path, "running", None, None)?;
    match sync::execute(plan, confirmation, progress) {
        Ok(report) => {
            write_state(plan, journal_path, "completed", Some(&report), None)?;
            Ok(report)
        }
        Err(error) => {
            // Preserve the original operation error even if a failing disk
            // also prevents the follow-up report from being written.
            let _ = write_state(plan, journal_path, "failed", None, Some(&error.to_string()));
            Err(error)
        }
    }
}

/// Mirror jobs use the same host-side journal but retain the separate delete
/// confirmation and backup requirement of the sync engine.
pub fn execute_mirror_to_journal(
    plan: &sync::SyncPlan,
    confirmation: &str,
    delete_confirmation: Option<&str>,
    backup_root: Option<&Path>,
    journal_path: &Path,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<sync::SyncReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    validate_location(journal_path, &plan.destination)?;
    write_state(plan, journal_path, "running", None, None)?;
    match sync::execute_with_mirror(
        plan,
        confirmation,
        delete_confirmation,
        backup_root,
        progress,
    ) {
        Ok(report) => {
            write_state(plan, journal_path, "completed", Some(&report), None)?;
            Ok(report)
        }
        Err(error) => {
            let _ = write_state(plan, journal_path, "failed", None, Some(&error.to_string()));
            Err(error)
        }
    }
}

/// Records the copy-and-delete lifecycle of a core move in the same host-side
/// journal used by sync and mirror jobs.
///
/// This mirrors `sync::execute_core_move`'s own parameter list (itself
/// pre-existing before P0-3 added `progress`), rather than collapsing it into
/// an options struct here; that collapse is P1-3's job, done once across the
/// whole `plan`/`execute` family, not piecemeal per wrapper.
#[allow(clippy::too_many_arguments)]
pub fn execute_core_move_to_journal(
    plan: &sync::SyncPlan,
    confirmation: &str,
    delete_confirmation: &str,
    source_common: &Path,
    source_root_prefix: &str,
    backup_root: &Path,
    journal_path: &Path,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<sync::SyncReport, TauError> {
    if confirmation != plan.id || delete_confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "a core move needs both matching copy and delete confirmation tokens",
        ));
    }
    validate_location(journal_path, &plan.destination)?;
    validate_location(journal_path, source_common)?;
    write_state(plan, journal_path, "running", None, None)?;
    match sync::execute_core_move(
        plan,
        confirmation,
        delete_confirmation,
        source_common,
        source_root_prefix,
        backup_root,
        progress,
    ) {
        Ok(report) => {
            write_state(plan, journal_path, "completed", Some(&report), None)?;
            Ok(report)
        }
        Err(error) => {
            let _ = write_state(plan, journal_path, "failed", None, Some(&error.to_string()));
            Err(error)
        }
    }
}

fn validate_location(path: &Path, media_root: &Path) -> Result<(), TauError> {
    if path.as_os_str().is_empty() || path.is_dir() {
        return Err(TauError::e(
            ErrorCode::InvalidJournalLocation,
            "journal must name a host report file",
        ));
    }
    if path.starts_with(media_root) {
        return Err(TauError::e(
            ErrorCode::InvalidJournalLocation,
            "journal must be outside the card media root",
        ));
    }
    Ok(())
}

/// Renders structured warnings as `{"code": "...", "message": "..."}` pairs so
/// the journal stays machine-readable rather than a flat list of sentences.
fn warnings_json(warnings: &[Warning]) -> serde_json::Value {
    json!(
        warnings
            .iter()
            .map(|warning| json!({ "code": warning.code.as_str(), "message": warning.message }))
            .collect::<Vec<_>>()
    )
}

fn write_state(
    plan: &sync::SyncPlan,
    path: &Path,
    state: &str,
    report: Option<&sync::SyncReport>,
    error: Option<&str>,
) -> Result<(), TauError> {
    let state = json!({
        "tool": "tau-omega",
        "format": 1,
        "state": state,
        "recorded_at_unix": timestamp(),
        "plan": {
            "id": plan.id,
            "destination": plan.destination,
            "files": plan.items.len(),
            "deletions": plan.deletions.len(),
            "bytes_to_write": plan.bytes_to_write,
            "embed_covers": plan.embed_covers,
        },
        "result": report.map(|result| json!({
            "copied": result.copied,
            "unchanged": result.unchanged,
            "deleted": result.deleted,
            "bytes_written": result.bytes_written,
            "index_path": result.index_path,
            "index_sha256": result.index_sha256,
            "warnings": warnings_json(&result.warnings),
        })),
        "error": error,
    });
    let encoded = serde_json::to_vec_pretty(&state).map_err(|error| {
        TauError::e(ErrorCode::Json, format!("journal encoding failed: {error}"))
    })?;
    let temp = path.with_extension(format!("tau-journal-{}.tmp", std::process::id()));
    {
        use std::io::Write;
        let mut file = fs::File::create(&temp)?;
        file.write_all(&encoded)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    fs::rename(temp, path)?;
    Ok(())
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Reads a host-side Tau Omega journal without changing it.
pub fn read_journal(path: impl AsRef<Path>) -> Result<serde_json::Value, TauError> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| TauError::e(ErrorCode::Json, format!("invalid journal: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn report_is_written_before_and_after_a_verified_job() {
        let root = std::env::temp_dir().join(format!("tau-journal-{}", timestamp()));
        let source = root.join("source");
        let common = root.join("card/Assets/tau/common");
        let report = root.join("reports/sync.json");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&common).unwrap();
        fs::create_dir_all(report.parent().unwrap()).unwrap();
        fs::write(source.join("track.mp3"), b"music").unwrap();
        let plan = sync::plan(
            &[source],
            &common,
            "/Assets/tau/common/",
            sync::PlanOptions::default(),
            &mut None,
        )
        .unwrap();
        let result = execute_to_journal(&plan, &plan.id, &report, &mut None).unwrap();
        assert_eq!(result.copied, 1);
        let journal: serde_json::Value =
            serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
        assert_eq!(journal["state"], "completed");
        assert_eq!(journal["result"]["copied"], 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bad_confirmation_does_not_create_a_journal() {
        let root = std::env::temp_dir().join(format!("tau-journal-token-{}", timestamp()));
        let source = root.join("source");
        let common = root.join("card/Assets/tau/common");
        let report = PathBuf::from(&root).join("sync.json");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&common).unwrap();
        fs::write(source.join("track.mp3"), b"music").unwrap();
        let plan = sync::plan(
            &[source],
            &common,
            "/Assets/tau/common/",
            sync::PlanOptions::default(),
            &mut None,
        )
        .unwrap();
        assert!(execute_to_journal(&plan, "wrong", &report, &mut None).is_err());
        assert!(!report.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn core_move_refuses_a_report_inside_the_source_media_root() {
        let root = std::env::temp_dir().join(format!("tau-journal-move-{}", timestamp()));
        let source = root.join("card/Assets/tau/common");
        let destination = root.join("card/Assets/tau-test/common");
        let backup = root.join("backup");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::write(source.join("track.mp3"), b"music").unwrap();
        let plan =
            sync::plan_core_copy(&source, &destination, "/Assets/tau-test/common/", &mut None)
                .unwrap();
        let report = source.join("move.json");
        assert!(
            execute_core_move_to_journal(
                &plan,
                &plan.id,
                &plan.id,
                &source,
                "/Assets/tau/common/",
                &backup,
                &report,
                &mut None,
            )
            .is_err()
        );
        assert!(source.join("track.mp3").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
