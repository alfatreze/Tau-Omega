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
///
/// `kind` is a short, stable label (`"sync"`, `"core_copy"`, ...) the caller
/// already knows statically; it is recorded verbatim so [`list_journals`] can
/// tell operations apart without guessing from a plan's shape.
pub fn execute_to_journal(
    plan: &sync::SyncPlan,
    confirmation: &str,
    kind: &str,
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
    write_state(plan, journal_path, kind, "running", None, None)?;
    match sync::execute(plan, confirmation, progress) {
        Ok(report) => {
            write_state(plan, journal_path, kind, "completed", Some(&report), None)?;
            Ok(report)
        }
        Err(error) => {
            // Preserve the original operation error even if a failing disk
            // also prevents the follow-up report from being written.
            let _ = write_state(
                plan,
                journal_path,
                kind,
                "failed",
                None,
                Some(&error.to_string()),
            );
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
    write_state(plan, journal_path, "mirror", "running", None, None)?;
    match sync::execute_with_mirror(
        plan,
        confirmation,
        delete_confirmation,
        backup_root,
        progress,
    ) {
        Ok(report) => {
            write_state(
                plan,
                journal_path,
                "mirror",
                "completed",
                Some(&report),
                None,
            )?;
            Ok(report)
        }
        Err(error) => {
            let _ = write_state(
                plan,
                journal_path,
                "mirror",
                "failed",
                None,
                Some(&error.to_string()),
            );
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
    write_state(plan, journal_path, "core_move", "running", None, None)?;
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
            write_state(
                plan,
                journal_path,
                "core_move",
                "completed",
                Some(&report),
                None,
            )?;
            Ok(report)
        }
        Err(error) => {
            let _ = write_state(
                plan,
                journal_path,
                "core_move",
                "failed",
                None,
                Some(&error.to_string()),
            );
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
    kind: &str,
    state: &str,
    report: Option<&sync::SyncReport>,
    error: Option<&str>,
) -> Result<(), TauError> {
    let state = json!({
        "tool": "tau-omega",
        "format": 1,
        "kind": kind,
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

/// A row for the Jobs list: the small, stable subset of a journal worth
/// showing without opening the full record. [`read_journal`] still returns
/// the complete file for a detail view.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JournalSummary {
    pub path: std::path::PathBuf,
    pub kind: String,
    pub state: String,
    pub recorded_at_unix: u64,
    pub plan_id: String,
    pub destination: String,
    pub files: u64,
    pub copied: Option<u64>,
    pub deleted: Option<u64>,
    pub error: Option<String>,
}

/// Lists every journal (`*.json`, one level deep) in a configured reports
/// directory, newest first. A file that fails to parse or is missing a
/// required field is skipped rather than failing the whole listing -- one
/// corrupt or foreign `.json` file should not hide the rest of a user's job
/// history.
pub fn list_journals(dir: impl AsRef<Path>) -> Result<Vec<JournalSummary>, TauError> {
    let mut summaries = Vec::new();
    for entry in fs::read_dir(dir.as_ref())? {
        let path = entry?.path();
        if !path.is_file() || path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        if let Ok(value) = read_journal(&path)
            && let Some(summary) = summary_from(&path, &value)
        {
            summaries.push(summary);
        }
    }
    summaries.sort_by_key(|summary| std::cmp::Reverse(summary.recorded_at_unix));
    Ok(summaries)
}

fn summary_from(path: &Path, value: &serde_json::Value) -> Option<JournalSummary> {
    Some(JournalSummary {
        path: path.to_path_buf(),
        kind: value.get("kind")?.as_str()?.to_string(),
        state: value.get("state")?.as_str()?.to_string(),
        recorded_at_unix: value.get("recorded_at_unix")?.as_u64()?,
        plan_id: value.pointer("/plan/id")?.as_str()?.to_string(),
        destination: value.pointer("/plan/destination")?.as_str()?.to_string(),
        files: value.pointer("/plan/files")?.as_u64()?,
        copied: value.pointer("/result/copied").and_then(serde_json::Value::as_u64),
        deleted: value.pointer("/result/deleted").and_then(serde_json::Value::as_u64),
        error: value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    })
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
        let result = execute_to_journal(&plan, &plan.id, "sync", &report, &mut None).unwrap();
        assert_eq!(result.copied, 1);
        let journal: serde_json::Value =
            serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
        assert_eq!(journal["state"], "completed");
        assert_eq!(journal["kind"], "sync");
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
        assert!(execute_to_journal(&plan, "wrong", "sync", &report, &mut None).is_err());
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

    #[test]
    fn lists_journals_newest_first_and_skips_the_unreadable() {
        let root = std::env::temp_dir().join(format!("tau-journal-list-{}", timestamp()));
        let source = root.join("source");
        let common = root.join("card/Assets/tau/common");
        let reports = root.join("reports");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&common).unwrap();
        fs::create_dir_all(&reports).unwrap();
        fs::write(source.join("track.mp3"), b"music").unwrap();
        let plan = sync::plan(
            std::slice::from_ref(&source),
            &common,
            "/Assets/tau/common/",
            sync::PlanOptions::default(),
            &mut None,
        )
        .unwrap();
        execute_to_journal(
            &plan,
            &plan.id,
            "sync",
            &reports.join("a.json"),
            &mut None,
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let plan = sync::plan(
            std::slice::from_ref(&source),
            &common,
            "/Assets/tau/common/",
            sync::PlanOptions::default(),
            &mut None,
        )
        .unwrap();
        execute_to_journal(
            &plan,
            &plan.id,
            "sync",
            &reports.join("b.json"),
            &mut None,
        )
        .unwrap();
        fs::write(reports.join("not-a-journal.json"), b"not json").unwrap();
        fs::write(reports.join("ignored.txt"), b"ignore me").unwrap();
        let summaries = list_journals(&reports).unwrap();
        assert_eq!(summaries.len(), 2);
        assert!(summaries[0].recorded_at_unix >= summaries[1].recorded_at_unix);
        assert_eq!(summaries[0].kind, "sync");
        assert_eq!(summaries[0].state, "completed");
        // The second sync's plan sees the first sync's file as already
        // present, so it copies nothing -- that's correct sync behaviour,
        // not a bug in this test.
        assert_eq!(summaries[0].copied, Some(0));
        fs::remove_dir_all(root).unwrap();
    }
}
