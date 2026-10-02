//! Host-side, durable job reports. A journal is always outside a card's media
//! root, so recovery information remains available if a card is removed.

use crate::{ErrorCode, ProgressObserver, TauError, Warning, changes, sync};
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

/// Executes a reviewed workbench change set (add/remove/edit) while recording
/// its lifecycle in a host-side journal, like [`execute_to_journal`]. The
/// journal is written before anything on the card changes.
///
/// `context` is caller-supplied, human-oriented detail (card and core names,
/// the titles being added/removed/edited, how the card is connected) stored
/// verbatim under `"context"` so a history screen can say *what* a sync did in
/// words. A failed run records how far it got (`"partial"`) and the engine's
/// stable `error_code`, so the failure can be explained accurately later.
pub fn execute_changes_to_journal(
    plan: &changes::ChangePlan,
    confirmation: &str,
    backup_root: Option<&Path>,
    journal_path: &Path,
    context: Option<serde_json::Value>,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<changes::ChangeReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    validate_location(journal_path, &plan.destination)?;
    let started = timestamp();
    let ctx = ChangeJournal {
        plan,
        path: journal_path,
        context: context.as_ref(),
        started,
    };
    ctx.write("running", None, None)?;
    let mut report = changes::ChangeReport::default();
    match changes::execute_changes_with(plan, confirmation, backup_root, &mut report, progress) {
        Ok(()) => {
            ctx.write("completed", Some(&report), None)?;
            Ok(report)
        }
        Err(error) => {
            let _ = ctx.write("failed", Some(&report), Some(&error));
            Err(error)
        }
    }
}

struct ChangeJournal<'a> {
    plan: &'a changes::ChangePlan,
    path: &'a Path,
    context: Option<&'a serde_json::Value>,
    started: u64,
}

impl ChangeJournal<'_> {
    fn write(
        &self,
        state: &str,
        report: Option<&changes::ChangeReport>,
        error: Option<&TauError>,
    ) -> Result<(), TauError> {
        let plan = self.plan;
        let now = timestamp();
        let finished = state != "running";
        let duration = now.saturating_sub(self.started);
        let ok = state == "completed";
        let value = json!({
            "tool": "tau-omega",
            "format": 2,
            "kind": "library_changes",
            "state": state,
            "recorded_at_unix": now,
            "started_at_unix": self.started,
            "finished_at_unix": finished.then_some(now),
            "duration_secs": finished.then_some(duration),
            "context": self.context,
            "plan": {
                "id": plan.id,
                "destination": plan.destination,
                "files": plan.files_total,
                "deletions": plan.removal.as_ref().map_or(0, |r| r.items.len()),
                "bytes_to_write": plan.bytes_to_write,
                "adds": plan.sync.as_ref().map_or(0, |s| s.items.len()),
                "edits": plan.edit.as_ref().map_or(0, |e| e.items.len()),
            },
            "result": report.filter(|_| ok).map(|r| json!({
                "copied": r.copied,
                "unchanged": r.unchanged,
                "deleted": r.deleted,
                "edited": r.edited,
                "reapplied_edits": r.reapplied,
                "playlists_updated": r.playlists_updated,
                "bytes_written": r.bytes_written,
                "bytes_per_sec": (duration > 0).then(|| r.bytes_written / duration),
                "phase": r.phase,
                "backup_dir": r.backup_dir,
                "index_path": r.index_path,
            })),
            "partial": report.filter(|_| state == "failed").map(|r| json!({
                "phase": r.phase,
                "copied": r.copied,
                "edited": r.edited,
                "deleted": r.deleted,
                "bytes_written": r.bytes_written,
            })),
            "error": error.map(ToString::to_string),
            "error_code": error.map(|e| e.code().as_u16()),
        });
        write_json_atomic(self.path, &value)
    }
}

fn write_json_atomic(path: &Path, value: &serde_json::Value) -> Result<(), TauError> {
    let encoded = serde_json::to_vec_pretty(value)
        .map_err(|e| TauError::e(ErrorCode::Json, format!("journal encoding failed: {e}")))?;
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

/// Deletes old Tau Omega journals in `dir`: everything beyond the newest
/// `keep_last` entries and everything older than `keep_days` days. A limit of
/// `0` means "no limit" for that rule. Only files that are recognisably
/// Tau Omega journals are ever touched, and a journal still marked `running`
/// that is under a day old is left alone. Returns how many were removed.
pub fn prune_journals(
    dir: impl AsRef<Path>,
    keep_last: usize,
    keep_days: u64,
) -> Result<usize, TauError> {
    prune_at(dir.as_ref(), keep_last, keep_days, timestamp())
}

fn prune_at(dir: &Path, keep_last: usize, keep_days: u64, now: u64) -> Result<usize, TauError> {
    let mut ours = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        if let Ok(value) = read_journal(&path)
            && value.get("tool").and_then(serde_json::Value::as_str) == Some("tau-omega")
            && let Some(at) = value
                .get("recorded_at_unix")
                .and_then(serde_json::Value::as_u64)
        {
            let running = value.get("state").and_then(serde_json::Value::as_str) == Some("running");
            ours.push((path, at, running));
        }
    }
    ours.sort_by_key(|(_, at, _)| std::cmp::Reverse(*at));
    let mut removed = 0;
    for (index, (path, at, running)) in ours.into_iter().enumerate() {
        if running && now.saturating_sub(at) < 86_400 {
            continue;
        }
        let too_many = keep_last > 0 && index >= keep_last;
        let too_old = keep_days > 0 && now.saturating_sub(at) > keep_days * 86_400;
        if (too_many || too_old) && fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

/// Deletes every Tau Omega journal in `dir` (the user's "clear history").
pub fn clear_journals(dir: impl AsRef<Path>) -> Result<usize, TauError> {
    let dir = dir.as_ref();
    let mut removed = 0;
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        if let Ok(value) = read_journal(&path)
            && value.get("tool").and_then(serde_json::Value::as_str) == Some("tau-omega")
            && fs::remove_file(&path).is_ok()
        {
            removed += 1;
        }
    }
    Ok(removed)
}

fn validate_location(path: &Path, media_root: &Path) -> Result<(), TauError> {
    if path.as_os_str().is_empty() || path.is_dir() {
        return Err(TauError::e(
            ErrorCode::InvalidJournalLocation,
            "journal must name a host report file",
        ));
    }
    if crate::sync::backup_is_inside(path, media_root) {
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
    /// The engine's stable error code for a failed run.
    pub error_code: Option<u64>,
    pub started_at_unix: Option<u64>,
    pub duration_secs: Option<u64>,
    pub edited: Option<u64>,
    pub bytes_written: Option<u64>,
    pub bytes_per_sec: Option<u64>,
    /// Where a failed run stopped (`copy`, `edit`, `remove`) or `done`.
    pub phase: Option<String>,
    /// Caller-supplied human detail stored with the journal (titles, card, connection).
    pub context: Option<serde_json::Value>,
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
        copied: value
            .pointer("/result/copied")
            .and_then(serde_json::Value::as_u64),
        deleted: value
            .pointer("/result/deleted")
            .and_then(serde_json::Value::as_u64),
        error: value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        error_code: value.get("error_code").and_then(serde_json::Value::as_u64),
        started_at_unix: value
            .get("started_at_unix")
            .and_then(serde_json::Value::as_u64),
        duration_secs: value
            .get("duration_secs")
            .and_then(serde_json::Value::as_u64),
        edited: value
            .pointer("/result/edited")
            .and_then(serde_json::Value::as_u64),
        bytes_written: value
            .pointer("/result/bytes_written")
            .and_then(serde_json::Value::as_u64),
        bytes_per_sec: value
            .pointer("/result/bytes_per_sec")
            .and_then(serde_json::Value::as_u64),
        phase: value
            .pointer("/result/phase")
            .or_else(|| value.pointer("/partial/phase"))
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        context: value.get("context").filter(|c| !c.is_null()).cloned(),
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
    fn change_sets_are_journalled_outside_the_card() {
        let root = std::env::temp_dir().join(format!("tau-journal-changes-{}", timestamp()));
        let lib = root.join("lib/A/One");
        let common = root.join("card/Assets/tau/common");
        fs::create_dir_all(&lib).unwrap();
        fs::create_dir_all(&common).unwrap();
        fs::write(lib.join("01.mp3"), b"music").unwrap();
        let request = changes::ChangeRequest {
            library_root: Some(root.join("lib")),
            add_albums: vec!["A/One".into()],
            ..Default::default()
        };
        let plan = changes::plan_changes(&common, &request, &mut None).unwrap();
        // inside the card is refused before anything is written
        assert!(
            execute_changes_to_journal(
                &plan,
                &plan.id,
                None,
                &common.join("j.json"),
                None,
                &mut None
            )
            .is_err()
        );
        assert!(!common.join("A").exists());
        let report = root.join("reports/changes.json");
        fs::create_dir_all(report.parent().unwrap()).unwrap();
        execute_changes_to_journal(
            &plan,
            &plan.id,
            None,
            &report,
            Some(json!({"card": "Pocket", "items": [{"kind": "add", "title": "One"}]})),
            &mut None,
        )
        .unwrap();
        let journal = read_journal(&report).unwrap();
        assert_eq!(journal["kind"], "library_changes");
        assert_eq!(journal["state"], "completed");
        assert_eq!(journal["result"]["copied"], 1);
        let listed = list_journals(root.join("reports")).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].context.as_ref().unwrap()["card"], "Pocket");
        assert_eq!(listed[0].phase.as_deref(), Some("done"));
        assert!(listed[0].duration_secs.is_some() && listed[0].started_at_unix.is_some());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_failed_change_set_is_journalled_with_how_far_it_got() {
        let root = std::env::temp_dir().join(format!("tau-journal-failed-{}", timestamp()));
        let lib = root.join("lib");
        let common = root.join("card/Assets/tau/common");
        fs::create_dir_all(lib.join("A/One")).unwrap();
        fs::create_dir_all(lib.join("B/Two")).unwrap();
        fs::create_dir_all(common.join("B/Two")).unwrap();
        fs::write(lib.join("A/One/01.mp3"), b"one").unwrap();
        fs::write(common.join("B/Two/01.mp3"), b"two").unwrap();
        let request = changes::ChangeRequest {
            library_root: Some(lib),
            add_albums: vec!["A/One".into()],
            remove_albums: vec!["B/Two".into()],
            ..Default::default()
        };
        let plan = changes::plan_changes(&common, &request, &mut None).unwrap();
        let report = root.join("reports/failed.json");
        fs::create_dir_all(report.parent().unwrap()).unwrap();
        // a backup folder inside the card makes the removal step fail after the add finished
        assert!(
            execute_changes_to_journal(
                &plan,
                &plan.id,
                Some(&common.join("bak")),
                &report,
                None,
                &mut None
            )
            .is_err()
        );
        let entry = &list_journals(root.join("reports")).unwrap()[0];
        assert_eq!(entry.state, "failed");
        assert_eq!(entry.phase.as_deref(), Some("remove"));
        assert_eq!(entry.error_code, Some(36));
        assert!(entry.error.is_some());
        let raw = read_journal(&report).unwrap();
        assert_eq!(raw["partial"]["copied"], 1);
        fs::remove_dir_all(root).unwrap();
    }

    fn fake_journal(dir: &Path, name: &str, at: u64, state: &str, tool: &str) {
        fs::write(
            dir.join(name),
            serde_json::to_vec(&json!({
                "tool": tool, "kind": "library_changes", "state": state, "recorded_at_unix": at,
                "plan": {"id": name, "destination": "/x", "files": 1}
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn pruning_keeps_the_newest_and_the_recent_and_never_touches_foreign_files() {
        let dir = std::env::temp_dir().join(format!("tau-prune-{}", timestamp()));
        fs::create_dir_all(&dir).unwrap();
        let now = 1_800_000_000u64;
        let day = 86_400u64;
        for (i, age_days) in [0u64, 1, 2, 3, 40, 400].iter().enumerate() {
            fake_journal(
                &dir,
                &format!("j{i}.json"),
                now - age_days * day,
                "completed",
                "tau-omega",
            );
        }
        fs::write(dir.join("notes.json"), br#"{"hello": "world"}"#).unwrap();
        fake_journal(
            &dir,
            "other-tool.json",
            now - 900 * day,
            "completed",
            "someone-else",
        );
        // keep the newest 4 and nothing older than 30 days: drops the 40- and 400-day-old entries
        assert_eq!(prune_at(&dir, 4, 30, now).unwrap(), 2);
        assert!(
            dir.join("j3.json").exists()
                && !dir.join("j4.json").exists()
                && !dir.join("j5.json").exists()
        );
        // by count only
        assert_eq!(prune_at(&dir, 2, 0, now).unwrap(), 2);
        assert!(dir.join("j1.json").exists() && !dir.join("j2.json").exists());
        // 0 and 0 means keep everything
        assert_eq!(prune_at(&dir, 0, 0, now).unwrap(), 0);
        // foreign json files survive every rule
        assert!(dir.join("notes.json").exists() && dir.join("other-tool.json").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_fresh_running_journal_is_not_pruned_but_a_stale_one_is() {
        let dir = std::env::temp_dir().join(format!("tau-prune-running-{}", timestamp()));
        fs::create_dir_all(&dir).unwrap();
        let now = 1_800_000_000u64;
        fake_journal(&dir, "fresh.json", now - 60, "running", "tau-omega");
        fake_journal(&dir, "newest.json", now, "completed", "tau-omega");
        fake_journal(&dir, "stale.json", now - 3 * 86_400, "running", "tau-omega");
        assert_eq!(prune_at(&dir, 1, 0, now).unwrap(), 1);
        assert!(
            dir.join("fresh.json").exists()
                && dir.join("newest.json").exists()
                && !dir.join("stale.json").exists()
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn clearing_history_removes_only_our_journals() {
        let dir = std::env::temp_dir().join(format!("tau-clear-{}", timestamp()));
        fs::create_dir_all(&dir).unwrap();
        fake_journal(&dir, "a.json", 5, "completed", "tau-omega");
        fake_journal(&dir, "b.json", 6, "failed", "tau-omega");
        fs::write(dir.join("keep.json"), b"{}").unwrap();
        assert_eq!(clear_journals(&dir).unwrap(), 2);
        assert!(dir.join("keep.json").exists());
        fs::remove_dir_all(dir).unwrap();
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
        execute_to_journal(&plan, &plan.id, "sync", &reports.join("a.json"), &mut None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let plan = sync::plan(
            std::slice::from_ref(&source),
            &common,
            "/Assets/tau/common/",
            sync::PlanOptions::default(),
            &mut None,
        )
        .unwrap();
        execute_to_journal(&plan, &plan.id, "sync", &reports.join("b.json"), &mut None).unwrap();
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
