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

/// A core that plays only from a playlist: the playlist Omega keeps in step with the tracks on the card, and the most tracks the core plays.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlaylistTarget {
    /// The playlist file, relative to the media root (`playlist.m3u`).
    pub file: String,
    pub max_tracks: Option<u32>,
    /// The largest playlist file the core reads, in bytes (about 48 characters per line at 256 tracks).
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_bytes: Option<u32>,
    /// The longest playlist name the core remembers for its next launch (a longer one falls back to `playlist.m3u`), unless the
    /// playlist is listed in `list_file`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub name_limit: Option<u8>,
    /// The playlist that lists the other playlists by file name (`playlists.m3u`). Omega keeps it in step when the core has one.
    #[cfg_attr(feature = "serde", serde(default))]
    pub list_file: Option<String>,
    /// The core's name, for messages.
    pub core_name: String,
}

/// Some albums go into a playlist: an existing one (their tracks are appended, none twice) or a new one. An album is an id from
/// `add_albums` (being copied in this run) or an album folder already on the card.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlaylistAssign {
    /// The playlist file, relative to the media root (`Road Trip.m3u`).
    pub file: String,
    pub albums: Vec<String>,
}

/// An assignment with its albums resolved to their folders on the card.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResolvedAssign {
    pub file: String,
    pub dirs: Vec<String>,
}

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
    /// Set when the target core plays only from a playlist (and not from the library index): the file to write and the cap on tracks.
    #[cfg_attr(feature = "serde", serde(default))]
    pub playlist: Option<PlaylistTarget>,
    /// Albums to put into playlists, in the same run.
    #[cfg_attr(feature = "serde", serde(default))]
    pub playlist_assign: Vec<PlaylistAssign>,
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
    #[cfg_attr(feature = "serde", serde(default))]
    pub playlist: Option<PlaylistTarget>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub assigns: Vec<ResolvedAssign>,
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
    /// The playlist written for a playlist-only core, and how many tracks it lists.
    #[cfg_attr(feature = "serde", serde(default))]
    pub playlist_file: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub playlist_tracks: usize,
    /// How many playlist files this run wrote or extended.
    #[cfg_attr(feature = "serde", serde(default))]
    pub playlists_changed: usize,
    /// Skip silence: the tables on the card and the tracks they mark, after this run (0 when the setting is off).
    #[cfg_attr(feature = "serde", serde(default))]
    pub skip_tracks: usize,
    #[cfg_attr(feature = "serde", serde(default))]
    pub skip_files: usize,
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
    if request.add_albums.is_empty()
        && request.remove_albums.is_empty()
        && request.edits.is_empty()
        && request.playlist_assign.is_empty()
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
            request.options.clone(),
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
    let assigns = resolve_assigns(request)?;
    if !assigns.is_empty() || request.playlist.is_some() {
        check_playlists(
            common,
            request,
            &assigns,
            sync_plan.as_ref(),
            removal.as_ref(),
        )?;
    }
    let mut hasher = Sha256::new();
    hasher.update(b"changes-v1");
    if let Some(p) = &request.playlist {
        hasher.update(
            format!(
                "playlist:{}:{:?}:{:?}:{:?}:{:?}",
                p.file, p.max_tracks, p.max_bytes, p.name_limit, p.list_file
            )
            .as_bytes(),
        );
    }
    for a in &assigns {
        hasher.update(format!("assign:{}:{}", a.file, a.dirs.join("|")).as_bytes());
    }
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
        playlist: request.playlist.clone(),
        assigns,
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
    if let Some(target) = &plan.playlist {
        report.playlist_file = Some(target.file.clone());
    }
    if !plan.assigns.is_empty() {
        report.phase = "playlist".into();
        let (changed, tracks) = apply_assigns(plan)?;
        report.playlists_changed = changed;
        report.playlist_tracks = tracks;
        // The library index lists the playlists, so a core that reads it needs the index read again.
        if plan.playlist.is_none() {
            let mut warnings = Vec::new();
            sync::rebuild_index(
                &plan.destination,
                &plan.root_prefix,
                &plan.id[..8],
                &mut warnings,
                progress,
            )?;
        }
    }
    if crate::trim::active().enabled {
        (report.skip_files, report.skip_tracks) = crate::trim::summarise(&plan.destination);
    }
    report.phase = "done".into();
    Ok(())
}

/// Resolves each assignment's albums to folders on the card: an id being added in this run lands in its destination folder, any other
/// is an album folder already there. Checks the playlist file names.
fn resolve_assigns(request: &ChangeRequest) -> Result<Vec<ResolvedAssign>, TauError> {
    let reserved = request
        .playlist
        .as_ref()
        .and_then(|p| p.list_file.as_deref());
    let mut out: Vec<ResolvedAssign> = Vec::new();
    for a in &request.playlist_assign {
        let file = a.file.trim();
        if file.is_empty()
            || file.len() > 120
            || file.contains(['/', '\\', '\0', ':'])
            || file.starts_with('.')
            || !file.to_ascii_lowercase().ends_with(".m3u")
            || reserved.is_some_and(|r| r.eq_ignore_ascii_case(file))
        {
            return Err(TauError::e(
                ErrorCode::InvalidMediaRoot,
                format!("{file:?} is not a playlist file name."),
            ));
        }
        let dirs: Vec<String> = a
            .albums
            .iter()
            .map(|id| {
                if request.add_albums.contains(id) {
                    workbench::dest_dir(id)
                } else {
                    id.clone()
                }
            })
            .collect();
        match out.iter_mut().find(|r| r.file.eq_ignore_ascii_case(file)) {
            Some(existing) => existing.dirs.extend(dirs),
            None => out.push(ResolvedAssign {
                file: file.to_string(),
                dirs,
            }),
        }
    }
    Ok(out)
}

fn in_dirs(rel: &str, dirs: &[String]) -> bool {
    let dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
    dirs.iter()
        .any(|d| dir == d || dir.starts_with(&format!("{d}/")))
}

/// The lines of an existing playlist file (empty when there is none).
fn playlist_lines(common: &Path, file: &str) -> Vec<String> {
    std::fs::read_to_string(common.join(file))
        .map(|t| {
            t.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// `existing` followed by the tracks of `dirs` found in `rels` (in path order) that are not in it yet.
fn merged_playlist(
    existing: Vec<String>,
    rels: &std::collections::BTreeSet<String>,
    dirs: &[String],
) -> Vec<String> {
    let mut out = existing;
    let have: std::collections::BTreeSet<String> = out.iter().cloned().collect();
    out.extend(
        rels.iter()
            .filter(|r| in_dirs(r, dirs) && !have.contains(*r))
            .cloned(),
    );
    out
}

/// Before anything is copied: every playlist the run would write must fit the core's limits (tracks and file size per playlist, and the
/// name when the core would not remember it).
fn check_playlists(
    common: &Path,
    request: &ChangeRequest,
    assigns: &[ResolvedAssign],
    sync_plan: Option<&sync::SyncPlan>,
    removal: Option<&workbench::RemovalPlan>,
) -> Result<(), TauError> {
    let Some(target) = &request.playlist else {
        return Ok(());
    };
    let name = target.core_name.as_str();
    if target.list_file.is_none()
        && let Some(limit) = target.name_limit
        && let Some(bad) = assigns
            .iter()
            .map(|a| a.file.as_str())
            .chain(std::iter::once(target.file.as_str()))
            .find(|f| f.len() > usize::from(limit))
    {
        return Err(TauError::e(
            ErrorCode::InvalidMediaRoot,
            format!(
                "{name} only remembers a playlist name of up to {limit} characters, and {bad:?} is longer, so it would fall back to playlist.m3u at the next launch."
            ),
        ));
    }
    if assigns.is_empty() {
        return Ok(());
    }
    // the tracks that will be on the card after this run
    let root = common.canonicalize()?;
    let rel = |p: &Path| {
        p.strip_prefix(&root)
            .ok()
            .map(|r| r.to_string_lossy().replace('\\', "/"))
    };
    let mut rels: std::collections::BTreeSet<String> = crate::scan::scan_dir(common, false)?
        .entries
        .iter()
        .map(|e| e.rel.clone())
        .collect();
    if let Some(r) = removal {
        for i in r.items.iter().filter(|i| sync::audio_file(&i.destination)) {
            if let Some(x) = rel(&i.destination) {
                rels.remove(&x);
            }
        }
    }
    if let Some(sp) = sync_plan {
        for i in sp.items.iter().filter(|i| sync::audio_file(&i.destination)) {
            if let Some(x) = rel(&i.destination) {
                rels.insert(x);
            }
        }
    }
    for a in assigns {
        let lines = merged_playlist(playlist_lines(common, &a.file), &rels, &a.dirs);
        if let Some(max) = target.max_tracks
            && lines.len() > max as usize
        {
            let over = lines.len() - max as usize;
            return Err(TauError::e(
                ErrorCode::IndexCapExceeded,
                format!(
                    "{:?} would hold {} tracks, but {name} plays at most {max} from one playlist. Remove {} or put them in another playlist.",
                    a.file,
                    lines.len(),
                    if over == 1 {
                        "1 track".to_string()
                    } else {
                        format!("{over} tracks")
                    }
                ),
            ));
        }
        if let Some(max) = target.max_bytes {
            let bytes: usize = lines.iter().map(|l| l.len() + 1).sum();
            if bytes > max as usize {
                return Err(TauError::e(
                    ErrorCode::IndexCapExceeded,
                    format!(
                        "{:?} would be {:.1} KB for {} tracks, but {name} reads at most {:.0} KB (a track's path takes about 48 characters on average). Use fewer tracks or shorten folder and file names.",
                        a.file,
                        bytes as f64 / 1024.0,
                        lines.len(),
                        f64::from(max) / 1024.0
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Writes (or extends) each assigned playlist beside its final name and renames it, so a core never sees half a playlist. Returns how many
/// playlists changed and how many tracks they hold together. For a core with a playlist of playlists, that list is kept in step.
fn apply_assigns(plan: &ChangePlan) -> Result<(usize, usize), TauError> {
    let common = &plan.destination;
    let rels: std::collections::BTreeSet<String> = crate::scan::scan_dir(common, false)?
        .entries
        .iter()
        .map(|e| e.rel.clone())
        .collect();
    let (mut changed, mut total) = (0, 0);
    for a in &plan.assigns {
        let lines = merged_playlist(playlist_lines(common, &a.file), &rels, &a.dirs);
        write_lines(common, &a.file, &lines)?;
        changed += 1;
        total += lines.len();
    }
    if let Some(list) = plan.playlist.as_ref().and_then(|t| t.list_file.as_deref()) {
        let mut names: Vec<String> = std::fs::read_dir(common)?
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| {
                n.to_ascii_lowercase().ends_with(".m3u")
                    && !n.starts_with('.')
                    && !n.eq_ignore_ascii_case(list)
            })
            .collect();
        names.sort_by_key(|n| {
            (
                !plan.playlist.as_ref().is_some_and(|t| t.file == *n),
                n.to_lowercase(),
            )
        });
        write_lines(common, list, &names)?;
    }
    Ok((changed, total))
}

fn write_lines(common: &Path, file: &str, lines: &[String]) -> Result<(), TauError> {
    let mut text = String::new();
    for l in lines {
        text.push_str(l);
        text.push('\n');
    }
    let tmp = common.join(format!(".{file}.part"));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, common.join(file))?;
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
            playlist: None,
            playlist_assign: vec![],
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

    /// A real, approved write through the app's own path (plan, execute, then remove). Runs only with
    /// `TAU_REAL_SYNC_MEDIA` (a card's `Assets/<platform>/common`), `TAU_REAL_SYNC_LIB` (a local library folder holding
    /// one album folder named `Omega Sweep Test`) and `TAU_REAL_SYNC_BACKUP` (a folder outside the card) set.
    /// Checks that no AppleDouble stub is left after the add or after the removal, and that the card ends as it began.
    #[test]
    #[ignore]
    fn real_card_sync_and_remove_leave_no_appledouble_stubs() {
        let (Ok(media), Ok(lib), Ok(backup)) = (
            std::env::var("TAU_REAL_SYNC_MEDIA"),
            std::env::var("TAU_REAL_SYNC_LIB"),
            std::env::var("TAU_REAL_SYNC_BACKUP"),
        ) else {
            return;
        };
        let (media, lib, backup) = (
            PathBuf::from(media),
            PathBuf::from(lib),
            PathBuf::from(backup),
        );
        let card = media
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        fn walk(dir: &Path, out: &mut Vec<(PathBuf, u64)>) {
            for e in fs::read_dir(dir)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
            {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.push((p, e.metadata().map(|m| m.len()).unwrap_or(0)));
                }
            }
        }
        let snapshot = || {
            let mut all = Vec::new();
            walk(&card.join("Assets"), &mut all);
            walk(&card.join("Cores"), &mut all);
            // The index and its ledger are rewritten by design; everything else must come back as it was.
            all.retain(|(p, _)| p.file_name().is_none_or(|n| n != "tau-library.tdb"));
            all.sort();
            all
        };
        let stubs = |dir: &Path| {
            let mut all = Vec::new();
            walk(dir, &mut all);
            all.into_iter()
                .filter(|(p, _)| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("._"))
                })
                .count()
        };
        let before = snapshot();
        println!("stubs before: {}", stubs(&media));
        let opts = sync::PlanOptions {
            mirror: false,
            embed_covers: true,
            art_sidecar_pal256: true,
            ..sync::PlanOptions::default()
        };
        let add = plan_changes(
            &media,
            &ChangeRequest {
                library_root: Some(lib),
                add_albums: vec!["Omega Sweep Test".into()],
                remove_albums: vec![],
                edits: vec![],
                options: opts.clone(),
                playlist: None,
                playlist_assign: vec![],
            },
            &mut None,
        )
        .unwrap();
        let report = execute_changes(&add, &add.id, Some(&backup), &mut None).unwrap();
        let after_add = stubs(&media);
        println!(
            "added: copied={} bytes={} phase={} stubs after add: {after_add}",
            report.copied, report.bytes_written, report.phase
        );
        let dest_album = add.sync.as_ref().unwrap().items[0]
            .destination
            .parent()
            .unwrap()
            .strip_prefix(&media)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        println!("album folder on card: {dest_album}");
        let remove = plan_changes(
            &media,
            &ChangeRequest {
                library_root: None,
                add_albums: vec![],
                remove_albums: vec![dest_album],
                edits: vec![],
                options: opts,
                playlist: None,
                playlist_assign: vec![],
            },
            &mut None,
        )
        .unwrap();
        let report = execute_changes(&remove, &remove.id, Some(&backup), &mut None).unwrap();
        let after_remove = stubs(&media);
        println!(
            "removed: deleted={} phase={} stubs after removal: {after_remove}",
            report.deleted, report.phase
        );
        let after = snapshot();
        let diff: Vec<_> = after
            .iter()
            .filter(|e| !before.contains(e))
            .chain(before.iter().filter(|e| !after.contains(e)))
            .collect();
        println!("entries that differ from before: {diff:?}");
        assert_eq!(after_add, 0, "stubs left after the add");
        assert_eq!(after_remove, 0, "stubs left after the removal");
        assert!(
            diff.is_empty(),
            "the card did not come back to how it started"
        );
    }

    fn target(
        max_tracks: Option<u32>,
        max_bytes: Option<u32>,
        name_limit: Option<u8>,
        list_file: Option<&str>,
    ) -> PlaylistTarget {
        PlaylistTarget {
            file: "playlist.m3u".into(),
            max_tracks,
            max_bytes,
            name_limit,
            list_file: list_file.map(String::from),
            core_name: "Mp3Player".into(),
        }
    }
    fn req(
        lib: &Path,
        add: &[&str],
        assigns: &[(&str, &[&str])],
        t: Option<PlaylistTarget>,
    ) -> ChangeRequest {
        ChangeRequest {
            library_root: Some(lib.to_path_buf()),
            add_albums: add.iter().map(|s| s.to_string()).collect(),
            playlist: t,
            playlist_assign: assigns
                .iter()
                .map(|(f, a)| PlaylistAssign {
                    file: f.to_string(),
                    albums: a.iter().map(|s| s.to_string()).collect(),
                })
                .collect(),
            ..Default::default()
        }
    }
    fn lines(common: &Path, f: &str) -> String {
        fs::read_to_string(common.join(f)).unwrap()
    }

    #[test]
    fn albums_added_to_a_playlist_are_appended_once_and_a_new_playlist_is_created() {
        let lib = library();
        let common = card_with(&lib, &["B/Two"]);
        // a new playlist, from an album being copied and an album already on the card
        let r = req(
            &lib,
            &["A/One"],
            &[("Road Trip.m3u", &["A/One", "B/Two"])],
            None,
        );
        let plan = plan_changes(&common, &r, &mut None).unwrap();
        let rep = execute_changes(&plan, &plan.id, None, &mut None).unwrap();
        assert_eq!((rep.playlists_changed, rep.playlist_tracks), (1, 2));
        assert_eq!(
            lines(&common, "Road Trip.m3u"),
            "A/One/01.mp3\nB/Two/01.mp3\n"
        );
        // more albums go to the end, none twice
        let r2 = req(
            &lib,
            &["C/Three"],
            &[("Road Trip.m3u", &["C/Three", "A/One"])],
            None,
        );
        let p2 = plan_changes(&common, &r2, &mut None).unwrap();
        execute_changes(&p2, &p2.id, None, &mut None).unwrap();
        assert_eq!(
            lines(&common, "Road Trip.m3u"),
            "A/One/01.mp3\nB/Two/01.mp3\nC/Three/01.mp3\n"
        );
        // assigning albums already on the card needs no copy at all
        let r3 = req(&lib, &[], &[("Quiet.m3u", &["B/Two"])], None);
        let p3 = plan_changes(&common, &r3, &mut None).unwrap();
        assert!(p3.sync.is_none() && p3.removal.is_none());
        execute_changes(&p3, &p3.id, None, &mut None).unwrap();
        assert_eq!(lines(&common, "Quiet.m3u"), "B/Two/01.mp3\n");
        assert!(!common.join(".Quiet.m3u.part").exists());
        // the library index lists the playlists for a core that reads it
        let index = crate::parse(fs::read(common.join("tau-library.tdb")).unwrap()).unwrap();
        assert!(index.counts.playlists >= 2, "{}", index.counts.playlists);
    }

    #[test]
    fn a_playlist_only_core_keeps_to_its_limits_per_playlist_and_keeps_its_list_of_playlists() {
        let lib = library();
        let common = card_with(&lib, &["B/Two"]);
        let t = || Some(target(Some(2), None, Some(12), Some("playlists.m3u")));
        // 3 tracks into one playlist is over a limit of 2 for that playlist: refused before anything is copied
        let over = req(
            &lib,
            &["A/One", "C/Three"],
            &[("playlist.m3u", &["A/One", "B/Two", "C/Three"])],
            t(),
        );
        let e = plan_changes(&common, &over, &mut None).unwrap_err();
        assert_eq!(e.code(), ErrorCode::IndexCapExceeded);
        assert!(
            e.message.contains("3 tracks")
                && e.message.contains("at most 2 from one playlist")
                && e.message.contains("Remove 1 track"),
            "{}",
            e.message
        );
        assert!(!common.join("A/One").exists(), "nothing was copied");
        // the same three tracks over two playlists are fine, and the card may hold more tracks than one playlist
        let ok = req(
            &lib,
            &["A/One", "C/Three"],
            &[
                ("playlist.m3u", &["A/One", "B/Two"]),
                ("A Long Playlist Name.m3u", &["C/Three"]),
            ],
            t(),
        );
        let plan = plan_changes(&common, &ok, &mut None).unwrap();
        let rep = execute_changes(&plan, &plan.id, None, &mut None).unwrap();
        assert_eq!(
            (rep.playlists_changed, rep.playlist_file.as_deref()),
            (2, Some("playlist.m3u"))
        );
        // a name longer than 12 characters is fine because the core's list of playlists names it, and the default playlist comes first
        assert_eq!(
            lines(&common, "playlists.m3u"),
            "playlist.m3u\nA Long Playlist Name.m3u\n"
        );
        assert_eq!(
            lines(&common, "A Long Playlist Name.m3u"),
            "C/Three/01.mp3\n"
        );
        // the list of playlists is not a playlist to write to
        let reserved = req(&lib, &[], &[("playlists.m3u", &["B/Two"])], t());
        assert_eq!(
            plan_changes(&common, &reserved, &mut None)
                .unwrap_err()
                .code(),
            ErrorCode::InvalidMediaRoot
        );
        // a bad file name is refused
        for bad in ["../x.m3u", "x.txt", ".hidden.m3u", ""] {
            let r = req(&lib, &[], &[(bad, &["B/Two"])], t());
            assert_eq!(
                plan_changes(&common, &r, &mut None).unwrap_err().code(),
                ErrorCode::InvalidMediaRoot,
                "{bad}"
            );
        }
    }

    #[test]
    fn the_playlist_file_size_limit_is_per_playlist_and_checked_before_anything_is_copied() {
        let lib = library();
        let common = card_with(&lib, &["B/Two"]);
        let t = Some(target(Some(256), Some(30), None, None));
        let three = req(
            &lib,
            &["A/One", "C/Three"],
            &[("playlist.m3u", &["A/One", "B/Two", "C/Three"])],
            t.clone(),
        );
        let e = plan_changes(&common, &three, &mut None).unwrap_err();
        assert_eq!(e.code(), ErrorCode::IndexCapExceeded);
        assert!(
            e.message.contains("3 tracks") && e.message.contains("Mp3Player reads at most"),
            "{}",
            e.message
        );
        assert!(!common.join("A/One").exists(), "nothing was copied");
        let two = req(
            &lib,
            &["A/One"],
            &[("playlist.m3u", &["A/One", "B/Two"])],
            t,
        );
        assert!(plan_changes(&common, &two, &mut None).is_ok());
    }

    #[test]
    fn a_playlist_name_longer_than_the_core_remembers_is_refused_unless_the_core_lists_it() {
        let lib = library();
        let common = card_with(&lib, &["B/Two"]);
        let t = Some(target(None, None, Some(12), None));
        let long = req(
            &lib,
            &[],
            &[("my long playlist.m3u", &["B/Two"])],
            t.clone(),
        );
        let e = plan_changes(&common, &long, &mut None).unwrap_err();
        assert!(
            e.message.contains("12 characters") && e.message.contains("playlist.m3u"),
            "{}",
            e.message
        );
        assert!(
            plan_changes(
                &common,
                &req(&lib, &[], &[("playlist.m3u", &["B/Two"])], t),
                &mut None
            )
            .is_ok(),
            "exactly 12 is allowed"
        );
    }
}
