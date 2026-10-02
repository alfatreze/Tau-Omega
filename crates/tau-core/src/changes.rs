//! One reviewed change set for the library workbench: albums to add, albums
//! to remove and tag/cover edits, planned together and applied under a single
//! confirmation token. Each part keeps its own safety rules (see `sync`,
//! `workbench::plan_removal` and `tagedit`); this module only sequences them.
//!
//! Order of execution: copy new files (verified) -> re-apply recorded edits
//! to anything re-copied -> apply new edits -> remove albums (backed up) with
//! the index rebuilt after each step, so the card is loadable at every point.

use crate::{ErrorCode, ProgressObserver, TauError, root_prefix, sync, tagedit, workbench};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeRequest {
    /// The local library the albums to add come from.
    pub library_root: Option<PathBuf>,
    /// Album ids (folders relative to `library_root`) to add or update.
    pub add_albums: Vec<String>,
    /// Album ids (folders relative to the card media root) to remove.
    pub remove_albums: Vec<String>,
    pub edits: Vec<tagedit::EditRequest>,
    pub options: sync::PlanOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangePlan {
    pub id: String,
    pub destination: PathBuf,
    pub root_prefix: String,
    pub sync: Option<sync::SyncPlan>,
    pub removal: Option<workbench::RemovalPlan>,
    pub edit: Option<tagedit::EditPlan>,
    pub bytes_to_write: u64,
    pub bytes_to_remove: u64,
    pub files_total: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeReport {
    pub plan_id: String,
    pub copied: usize,
    pub unchanged: usize,
    pub bytes_written: u64,
    pub edited: usize,
    pub reapplied: usize,
    pub deleted: usize,
    pub playlists_updated: usize,
    pub backup_dir: Option<PathBuf>,
    pub index_path: PathBuf,
    /// How far the run got: `"copy"`, `"edit"`, `"remove"` (the step in progress or last started), or `"done"`.
    pub phase: String,
}

#[derive(Debug, Clone, Copy)]
struct Counts {
    tracks: usize,
    albums: usize,
}

/// Fails if the library after the change would exceed the index's capacity.
/// Tracks are counted exactly. Albums are estimated from folders (the index
/// groups by tag), so this can only under-count a folder holding several
/// albums, never block a library that fits.
fn check_index_limits(
    now: Counts,
    add: Counts,
    remove: Counts,
    limits: workbench::IndexLimits,
) -> Result<(), TauError> {
    let tracks = (now.tracks + add.tracks).saturating_sub(remove.tracks);
    let albums = (now.albums + add.albums).saturating_sub(remove.albums);
    if tracks > limits.max_tracks {
        return Err(TauError::e(
            ErrorCode::IndexCapExceeded,
            format!(
                "This would put {tracks} tracks on the Pocket, but its library holds at most {}. Remove some albums or add fewer.",
                limits.max_tracks
            ),
        ));
    }
    if albums > limits.max_albums {
        return Err(TauError::e(
            ErrorCode::IndexCapExceeded,
            format!(
                "This would put about {albums} albums on the Pocket, but its library holds at most {}. Remove some albums or add fewer.",
                limits.max_albums
            ),
        ));
    }
    Ok(())
}

/// Plans a change set against the card media root `common`. Read-only.
pub fn plan_changes(
    common: &Path,
    request: &ChangeRequest,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<ChangePlan, TauError> {
    sync::validate_media_root(common)?;
    if request.add_albums.is_empty() && request.remove_albums.is_empty() && request.edits.is_empty()
    {
        return Err(TauError::e(
            ErrorCode::NoSources,
            "there are no changes to apply",
        ));
    }
    let prefix = root_prefix(common)?;
    // An album being (re)written by a sync cannot also be edited or removed in
    // the same run: the edit/removal was reviewed against the old files.
    let adding: std::collections::BTreeSet<String> = request
        .add_albums
        .iter()
        .map(|id| workbench::dest_dir(id))
        .collect();
    for id in request
        .remove_albums
        .iter()
        .chain(request.edits.iter().map(|e| &e.album_id))
    {
        if adding.contains(id) {
            return Err(TauError::e(
                ErrorCode::NameCollision,
                format!("\"{id}\" is being added and changed in the same run; apply one first"),
            ));
        }
    }
    let sync_plan = if request.add_albums.is_empty() {
        None
    } else {
        let library = request.library_root.as_deref().ok_or_else(|| {
            TauError::e(
                ErrorCode::NoSources,
                "a library folder is needed to add albums",
            )
        })?;
        Some(workbench::plan_selection(
            library,
            &request.add_albums,
            common,
            &prefix,
            request.options,
            progress,
        )?)
    };
    let removal = if request.remove_albums.is_empty() {
        None
    } else {
        Some(workbench::plan_removal(common, &request.remove_albums)?)
    };
    let edit = if request.edits.is_empty() {
        None
    } else {
        Some(tagedit::plan_edit(common, &request.edits)?)
    };
    // Refuse a change set the Pocket's index could not hold *before* anything
    // is copied; otherwise every file would be written and the index step
    // would then fail.
    let (tracks_now, dirs_now) = workbench::count_audio(common)?;
    let new_tracks = sync_plan.as_ref().map_or(0, |p| {
        p.items
            .iter()
            .filter(|i| i.state == sync::CopyState::New && sync::audio_file(&i.destination))
            .count()
    });
    let new_albums = sync_plan.as_ref().map_or(0, |p| {
        p.items
            .iter()
            .filter(|i| i.state == sync::CopyState::New)
            .filter_map(|i| {
                i.destination
                    .parent()?
                    .strip_prefix(common.canonicalize().ok()?)
                    .ok()
                    .map(|d| d.to_string_lossy().replace('\\', "/"))
            })
            .filter(|d| !dirs_now.contains(d))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    });
    let removed_tracks = removal.as_ref().map_or(0, |r| {
        r.items
            .iter()
            .filter(|i| sync::audio_file(&i.destination))
            .count()
    });
    check_index_limits(
        Counts {
            tracks: tracks_now,
            albums: dirs_now.len(),
        },
        Counts {
            tracks: new_tracks,
            albums: new_albums,
        },
        Counts {
            tracks: removed_tracks,
            albums: request.remove_albums.len(),
        },
        workbench::IndexLimits::default(),
    )?;
    let mut hasher = Sha256::new();
    hasher.update(b"changes-v1");
    for id in [
        sync_plan.as_ref().map(|p| p.id.as_str()),
        removal.as_ref().map(|p| p.id.as_str()),
        edit.as_ref().map(|p| p.id.as_str()),
    ] {
        hasher.update(id.unwrap_or("-").as_bytes());
        hasher.update([0]);
    }
    let files_total = sync_plan.as_ref().map_or(0, |p| p.items.len())
        + removal.as_ref().map_or(0, |p| p.items.len())
        + edit.as_ref().map_or(0, |p| p.items.len());
    Ok(ChangePlan {
        id: format!("{:x}", hasher.finalize()),
        destination: common.canonicalize()?,
        root_prefix: prefix,
        bytes_to_write: sync_plan.as_ref().map_or(0, |p| p.bytes_to_write),
        bytes_to_remove: removal.as_ref().map_or(0, |p| p.bytes),
        files_total,
        sync: sync_plan,
        removal,
        edit,
    })
}

/// Applies a reviewed change set. `backup_root` (outside the card) is used for
/// removals; pass `None` only when the user chose to remove without a backup.
pub fn execute_changes(
    plan: &ChangePlan,
    confirmation: &str,
    backup_root: Option<&Path>,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<ChangeReport, TauError> {
    let mut report = ChangeReport::default();
    execute_changes_with(plan, confirmation, backup_root, &mut report, progress)?;
    Ok(report)
}

/// Like [`execute_changes`], but fills in `report` as it goes, so a caller
/// that sees an `Err` can still tell how far the run got (`report.phase` and
/// the counts so far). Used to journal a failed run accurately.
pub fn execute_changes_with(
    plan: &ChangePlan,
    confirmation: &str,
    backup_root: Option<&Path>,
    report: &mut ChangeReport,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<(), TauError> {
    let result = execute_changes_inner(plan, confirmation, backup_root, report, progress);
    // Finished or stopped part way: remove macOS's metadata stubs from the media root (a refused token wrote nothing).
    if !matches!(&result, Err(e) if e.code() == ErrorCode::ConfirmationMismatch) {
        sync::sweep_appledouble(&plan.destination);
    }
    result
}

fn execute_changes_inner(
    plan: &ChangePlan,
    confirmation: &str,
    backup_root: Option<&Path>,
    report: &mut ChangeReport,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<(), TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    report.plan_id = plan.id.clone();
    report.index_path = plan.destination.join("tau-library.tdb");
    report.phase = "start".into();
    if let Some(sync_plan) = &plan.sync {
        report.phase = "copy".into();
        let done = sync::execute(sync_plan, &sync_plan.id, progress)?;
        report.copied = done.copied;
        report.unchanged = done.unchanged;
        report.bytes_written = done.bytes_written;
        // Files just re-copied from the unedited source get their recorded
        // edits applied again.
        let rels: Vec<String> = sync_plan
            .items
            .iter()
            .filter(|i| i.state != sync::CopyState::Same)
            .filter_map(|i| i.destination.strip_prefix(&plan.destination).ok())
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .collect();
        report.reapplied = tagedit::reapply_for(&plan.destination, &rels)?;
        if report.reapplied > 0 {
            let mut warnings = Vec::new();
            sync::rebuild_index(
                &plan.destination,
                &plan.root_prefix,
                &sync_plan.id,
                &mut warnings,
                progress,
            )?;
        }
    }
    if let Some(edit) = &plan.edit {
        report.phase = "edit".into();
        let done = tagedit::execute_edit(edit, &edit.id, &plan.root_prefix, progress)?;
        report.edited = done.files_changed;
    }
    if let Some(removal) = &plan.removal {
        report.phase = "remove".into();
        let done = workbench::execute_removal(
            removal,
            &removal.id,
            backup_root,
            &plan.root_prefix,
            progress,
        )?;
        report.deleted = done.deleted;
        report.playlists_updated = done.playlists_updated;
        report.backup_dir = done.backup_dir;
    }
    report.phase = "done".into();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "tau-ch-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                + crate::test_uniq()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn library() -> PathBuf {
        let lib = tmp("lib");
        for dir in ["A/One", "B/Two", "C/Three"] {
            fs::create_dir_all(lib.join(dir)).unwrap();
            fs::write(lib.join(dir).join("01.mp3"), format!("audio {dir}")).unwrap();
        }
        lib
    }
    fn card_with(lib: &Path, albums: &[&str]) -> PathBuf {
        let root = tmp("card");
        let common = root.join("Assets/tau/common");
        fs::create_dir_all(&common).unwrap();
        let plan = workbench::plan_selection(
            lib,
            &albums.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &common,
            "/Assets/tau/common/",
            sync::PlanOptions::default(),
            &mut None,
        )
        .unwrap();
        sync::execute(&plan, &plan.id, &mut None).unwrap();
        common
    }

    #[test]
    fn adds_removes_and_edits_apply_together_with_a_verifying_index() {
        let lib = library();
        let common = card_with(&lib, &["B/Two", "C/Three"]);
        let backup = tmp("backup");
        let request = ChangeRequest {
            library_root: Some(lib.clone()),
            add_albums: vec!["A/One".into()],
            remove_albums: vec!["B/Two".into()],
            edits: vec![tagedit::EditRequest {
                album_id: "C/Three".into(),
                fields: tagedit::FieldEdits {
                    album: Some("Renamed".into()),
                    ..Default::default()
                },
                ..Default::default()
            }],
            options: sync::PlanOptions::default(),
        };
        let plan = plan_changes(&common, &request, &mut None).unwrap();
        assert!(plan.sync.is_some() && plan.removal.is_some() && plan.edit.is_some());
        assert_eq!(plan.files_total, 3);
        assert_eq!(
            execute_changes(&plan, "wrong", None, &mut None)
                .unwrap_err()
                .code(),
            ErrorCode::ConfirmationMismatch
        );
        let report = execute_changes(&plan, &plan.id, Some(&backup), &mut None).unwrap();
        assert_eq!((report.copied, report.edited, report.deleted), (1, 1, 1));
        assert!(common.join("A/One/01.mp3").is_file());
        assert!(!common.join("B/Two").exists());
        assert!(common.join("C/Three/01.mp3").is_file());
        let index = fs::read(&report.index_path).unwrap();
        assert!(crate::verify(&index, Some(&common)).unwrap().is_empty());
        assert_eq!(crate::parse(&index).unwrap().counts.tracks, 2);
        // the local library is untouched
        assert!(lib.join("B/Two/01.mp3").is_file());
    }

    #[test]
    fn a_resync_of_an_edited_album_keeps_the_edit() {
        let lib = library();
        let common = card_with(&lib, &["C/Three"]);
        let edit = ChangeRequest {
            edits: vec![tagedit::EditRequest {
                album_id: "C/Three".into(),
                fields: tagedit::FieldEdits {
                    album: Some("Mine".into()),
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let plan = plan_changes(&common, &edit, &mut None).unwrap();
        execute_changes(&plan, &plan.id, None, &mut None).unwrap();
        // the source changes, so the album is updated on the card
        fs::write(lib.join("C/Three/01.mp3"), b"audio changed upstream").unwrap();
        let add = ChangeRequest {
            library_root: Some(lib.clone()),
            add_albums: vec!["C/Three".into()],
            ..Default::default()
        };
        let plan = plan_changes(&common, &add, &mut None).unwrap();
        let report = execute_changes(&plan, &plan.id, None, &mut None).unwrap();
        assert_eq!((report.copied, report.reapplied), (1, 1));
        let (tags, _, _) = crate::read_tags(&common.join("C/Three/01.mp3")).unwrap();
        assert_eq!(tags["TALB"], "Mine");
        assert!(
            fs::read(common.join("C/Three/01.mp3"))
                .unwrap()
                .ends_with(b"audio changed upstream")
        );
    }

    #[test]
    fn one_album_cannot_be_added_and_removed_or_edited_in_the_same_run() {
        let lib = library();
        let common = card_with(&lib, &["B/Two"]);
        let request = ChangeRequest {
            library_root: Some(lib),
            add_albums: vec!["B/Two".into()],
            remove_albums: vec!["B/Two".into()],
            ..Default::default()
        };
        assert_eq!(
            plan_changes(&common, &request, &mut None)
                .unwrap_err()
                .code(),
            ErrorCode::NameCollision
        );
    }

    #[test]
    fn the_index_limits_are_enforced_before_anything_is_copied() {
        let limits = workbench::IndexLimits {
            max_tracks: 10,
            max_albums: 3,
            max_artists: 5,
        };
        let c = |tracks, albums| Counts { tracks, albums };
        assert!(check_index_limits(c(8, 2), c(2, 1), c(0, 0), limits).is_ok()); // exactly full is fine
        assert_eq!(
            check_index_limits(c(8, 2), c(3, 0), c(0, 0), limits)
                .unwrap_err()
                .code(),
            ErrorCode::IndexCapExceeded
        );
        assert_eq!(
            check_index_limits(c(8, 3), c(1, 1), c(0, 0), limits)
                .unwrap_err()
                .code(),
            ErrorCode::IndexCapExceeded
        );
        // removing in the same run makes room
        assert!(check_index_limits(c(8, 3), c(3, 1), c(2, 1), limits).is_ok());
        let message = check_index_limits(c(9, 1), c(5, 0), c(0, 0), limits)
            .unwrap_err()
            .message;
        assert!(message.contains("14 tracks") && message.contains("at most 10"));
    }

    #[test]
    fn a_real_plan_counts_what_is_already_on_the_card() {
        let lib = library();
        let common = card_with(&lib, &["B/Two", "C/Three"]);
        let (tracks, dirs) = workbench::count_audio(&common).unwrap();
        assert_eq!((tracks, dirs.len()), (2, 2));
        let request = ChangeRequest {
            library_root: Some(lib),
            add_albums: vec!["A/One".into()],
            ..Default::default()
        };
        assert!(plan_changes(&common, &request, &mut None).is_ok());
    }

    #[test]
    fn a_failed_run_reports_how_far_it_got() {
        let lib = library();
        let common = card_with(&lib, &["B/Two"]);
        // add one album (fine), then remove with an unsafe backup location (fails in the last step)
        let request = ChangeRequest {
            library_root: Some(lib),
            add_albums: vec!["A/One".into()],
            remove_albums: vec!["B/Two".into()],
            ..Default::default()
        };
        let plan = plan_changes(&common, &request, &mut None).unwrap();
        let mut report = ChangeReport::default();
        let err = execute_changes_with(
            &plan,
            &plan.id,
            Some(&common.join("inside-the-card")),
            &mut report,
            &mut None,
        )
        .unwrap_err();
        assert_eq!(err.code(), ErrorCode::UnsafeBackupLocation);
        assert_eq!(report.phase, "remove");
        assert_eq!(report.copied, 1); // the add finished before the removal failed
        assert!(common.join("B/Two/01.mp3").is_file()); // and nothing was removed
    }

    #[test]
    fn an_empty_request_is_refused() {
        let lib = library();
        let common = card_with(&lib, &["B/Two"]);
        assert!(plan_changes(&common, &ChangeRequest::default(), &mut None).is_err());
    }
}
