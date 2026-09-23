//! Portable playlist import/export/write helpers.
//!
//! Create, rename, reorder, and import all go through the same
//! plan -> review -> confirm -> execute shape as [`crate::sync::plan`] /
//! [`crate::sync::execute`]: a `plan_*` function resolves and validates
//! everything up front and returns a [`PlaylistPlan`] carrying a content-hash
//! `id`; [`execute`] only proceeds when the caller's confirmation token
//! matches that id, and never re-derives anything from a caller-supplied
//! path string, so the plan reviewed by a user is exactly what gets written.

use crate::{Entry, ErrorCode, Playlist, TauError};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

/// Exports a playlist as a conventional UTF-8 `.m3u` using media-relative paths.
pub fn export_m3u(
    path: impl AsRef<Path>,
    playlist: &Playlist,
    entries: &[Entry],
) -> Result<(), TauError> {
    let mut output = String::new();
    for &id in &playlist.rel_ids {
        let entry = entries.get(id).ok_or_else(|| TauError {
            code: ErrorCode::IndexRecordRange,
            message: "playlist item".into(),
        })?;
        output.push_str(&entry.rel);
        output.push('\n');
    }
    fs::write(path, output).map_err(TauError::from)
}

/// A reviewed, not-yet-written playlist change: create, reorder (both are a
/// plain write of an ordered track list to a file), rename, or import.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlaylistPlan {
    pub id: String,
    /// The `.m3u` path this plan will write, relative to the media root.
    pub file: String,
    /// Set only for a rename: the file being renamed away from.
    pub previous_file: Option<String>,
    /// Resolved media-relative track paths, in final order.
    pub tracks: Vec<String>,
    /// Import-only: source lines that could not be matched to a library track.
    pub dropped: Vec<String>,
    pub overwrites_existing: bool,
}

/// Plans creating or reordering a playlist: writing `tracks` (media-relative
/// paths), in the given order, to `file`. Used for both "create" (a file
/// that does not exist yet) and "reorder" (rewriting an existing file with a
/// new track order) -- both are the same operation from this function's
/// point of view. Every path in `tracks` must already be a track in
/// `entries`; unlike [`plan_import`], a path that doesn't resolve here is a
/// caller error (the front-end is expected to only ever pass back paths it
/// was just given), not a silently-dropped line.
pub fn plan_write(
    common: &Path,
    file: &str,
    tracks: &[String],
    entries: &[Entry],
) -> Result<PlaylistPlan, TauError> {
    let path = validate_m3u_path(common, file)?;
    let known: BTreeSet<&str> = entries.iter().map(|entry| entry.rel.as_str()).collect();
    if let Some(missing) = tracks.iter().find(|track| !known.contains(track.as_str())) {
        return Err(TauError::e(
            ErrorCode::NotFound,
            format!("{missing}: not a track in this media root"),
        ));
    }
    Ok(PlaylistPlan {
        id: plan_id(file, None, tracks, &[]),
        file: file.to_string(),
        previous_file: None,
        tracks: tracks.to_vec(),
        dropped: Vec::new(),
        overwrites_existing: path.is_file(),
    })
}

/// Plans renaming an existing playlist file, keeping its track list intact.
/// Bare (non-rooted) lines in the old file are normalised against its own
/// folder before being written back rooted at the new location, so the
/// playlist keeps resolving correctly even though its own folder is what
/// bare lines would otherwise have been read relative to.
pub fn plan_rename(common: &Path, old_file: &str, new_file: &str) -> Result<PlaylistPlan, TauError> {
    if old_file == new_file {
        return Err(TauError::e(
            ErrorCode::SamePath,
            "new name is the same as the current one",
        ));
    }
    let old_path = validate_m3u_path(common, old_file)?;
    let new_path = validate_m3u_path(common, new_file)?;
    if !old_path.is_file() {
        return Err(TauError::e(
            ErrorCode::SourceMissing,
            "the playlist being renamed no longer exists",
        ));
    }
    if new_path.is_file() {
        return Err(TauError::e(
            ErrorCode::NameCollision,
            "a playlist already exists at that name",
        ));
    }
    let base = Path::new(old_file)
        .parent()
        .map(|parent| parent.to_string_lossy().replace('\\', "/"))
        .filter(|base| !base.is_empty());
    let tracks = String::from_utf8_lossy(&fs::read(&old_path)?)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| match line.strip_prefix('/') {
            Some(rest) => rest.to_string(),
            None => match &base {
                Some(base) => format!("{base}/{line}"),
                None => line.to_string(),
            },
        })
        .collect::<Vec<_>>();
    Ok(PlaylistPlan {
        id: plan_id(new_file, Some(old_file), &tracks, &[]),
        file: new_file.to_string(),
        previous_file: Some(old_file.to_string()),
        tracks,
        dropped: Vec::new(),
        overwrites_existing: false,
    })
}

/// Plans importing an external `.m3u` (from another app, or another media
/// root) as a new playlist in this media root. Each source line is matched
/// against the already-scanned `entries`: first by treating it as a path
/// already rooted under `common` (an m3u exported from this exact folder),
/// then by its bare filename if that resolves to exactly one track. A line
/// that matches nothing, or matches more than one same-named track, is
/// dropped and listed in [`PlaylistPlan::dropped`] rather than failing the
/// whole import.
pub fn plan_import(
    common: &Path,
    source: &Path,
    dest_file: &str,
    entries: &[Entry],
) -> Result<PlaylistPlan, TauError> {
    let dest_path = validate_m3u_path(common, dest_file)?;
    let by_rel: BTreeMap<&str, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| (entry.rel.as_str(), i))
        .collect();
    let mut by_basename: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, entry) in entries.iter().enumerate() {
        by_basename
            .entry(entry.file.to_ascii_lowercase())
            .or_default()
            .push(i);
    }
    let text = String::from_utf8_lossy(&fs::read(source)?).into_owned();
    let mut tracks = Vec::new();
    let mut dropped = Vec::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let candidate = Path::new(line);
        let rooted = candidate
            .strip_prefix(common)
            .ok()
            .map(|rel| rel.to_string_lossy().replace('\\', "/"));
        let resolved = rooted
            .as_deref()
            .and_then(|rel| by_rel.get(rel).copied())
            .or_else(|| {
                let name = candidate.file_name()?.to_string_lossy().to_ascii_lowercase();
                match by_basename.get(&name) {
                    Some(ids) if ids.len() == 1 => Some(ids[0]),
                    _ => None,
                }
            });
        match resolved {
            Some(id) => tracks.push(entries[id].rel.clone()),
            None => dropped.push(line.to_string()),
        }
    }
    if tracks.is_empty() {
        return Err(TauError::e(
            ErrorCode::NoSources,
            "no line in this playlist matched a track already in the library",
        ));
    }
    Ok(PlaylistPlan {
        id: plan_id(dest_file, None, &tracks, &dropped),
        file: dest_file.to_string(),
        previous_file: None,
        tracks,
        dropped,
        overwrites_existing: dest_path.is_file(),
    })
}

/// Writes a reviewed [`PlaylistPlan`], renaming the previous file first if
/// this is a rename. Refuses unless `confirmation` matches `plan.id` exactly,
/// mirroring [`crate::sync::execute`]'s own token check.
pub fn execute(common: &Path, plan: &PlaylistPlan, confirmation: &str) -> Result<(), TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    let dest = validate_m3u_path(common, &plan.file)?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(previous) = &plan.previous_file
        && previous != &plan.file
    {
        let source = validate_m3u_path(common, previous)?;
        if source.is_file() {
            fs::rename(&source, &dest)?;
        }
    }
    let mut output = String::new();
    for track in &plan.tracks {
        output.push('/');
        output.push_str(track);
        output.push('\n');
    }
    fs::write(&dest, output).map_err(TauError::from)
}

/// A relative `.m3u` path must stay inside the media root: no absolute path,
/// no `..` component. The file need not exist yet -- a create/import plan's
/// destination usually doesn't.
fn validate_m3u_path(common: &Path, file: &str) -> Result<PathBuf, TauError> {
    let trimmed = file.trim();
    if trimmed.is_empty() || !trimmed.to_ascii_lowercase().ends_with(".m3u") {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            "playlist file must be a relative .m3u path",
        ));
    }
    let candidate = Path::new(trimmed);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            "playlist path must stay inside the media root",
        ));
    }
    Ok(common.join(candidate))
}

fn plan_id(file: &str, previous_file: Option<&str>, tracks: &[String], dropped: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file.as_bytes());
    hasher.update([0]);
    if let Some(previous) = previous_file {
        hasher.update(previous.as_bytes());
    }
    hasher.update([0]);
    for track in tracks {
        hasher.update(track.as_bytes());
        hasher.update([0]);
    }
    for line in dropped {
        hasher.update(line.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::BTreeMap,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn exports_relative_lines_in_playlist_order() {
        let entries = vec![
            Entry {
                rel: "Artist/B/02.mp3".into(),
                dir: "Artist/B".into(),
                file: "02.mp3".into(),
                tags: BTreeMap::new(),
                secs: 1,
                fmt: 1,
            },
            Entry {
                rel: "Artist/A/01.flac".into(),
                dir: "Artist/A".into(),
                file: "01.flac".into(),
                tags: BTreeMap::new(),
                secs: 1,
                fmt: 2,
            },
        ];
        let playlist = Playlist {
            name: "Mix".into(),
            rel_ids: vec![1, 0],
            file: "Mix.m3u".into(),
        };
        let path = std::env::temp_dir().join(format!(
            "tau-playlist-{}.m3u",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        export_m3u(&path, &playlist, &entries).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "Artist/A/01.flac\nArtist/B/02.mp3\n"
        );
        fs::remove_file(path).unwrap();
    }

    fn scratch_root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tau-playlist-plan-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn tracks(entries: &[Entry], ids: &[usize]) -> Vec<String> {
        ids.iter().map(|&i| entries[i].rel.clone()).collect()
    }

    fn sample_entries() -> Vec<Entry> {
        vec![
            Entry {
                rel: "Artist/B/02.mp3".into(),
                dir: "Artist/B".into(),
                file: "02.mp3".into(),
                tags: BTreeMap::new(),
                secs: 1,
                fmt: 1,
            },
            Entry {
                rel: "Artist/A/01.flac".into(),
                dir: "Artist/A".into(),
                file: "01.flac".into(),
                tags: BTreeMap::new(),
                secs: 1,
                fmt: 2,
            },
        ]
    }

    #[test]
    fn create_then_reorder_then_execute_writes_rooted_lines() {
        let common = scratch_root("write");
        let entries = sample_entries();
        let plan = plan_write(&common, "Mix.m3u", &tracks(&entries, &[0, 1]), &entries).unwrap();
        assert!(!plan.overwrites_existing);
        execute(&common, &plan, &plan.id).unwrap();
        assert_eq!(
            fs::read_to_string(common.join("Mix.m3u")).unwrap(),
            "/Artist/B/02.mp3\n/Artist/A/01.flac\n"
        );
        let reorder = plan_write(&common, "Mix.m3u", &tracks(&entries, &[1, 0]), &entries).unwrap();
        assert!(reorder.overwrites_existing);
        assert_ne!(reorder.id, plan.id);
        execute(&common, &reorder, &reorder.id).unwrap();
        assert_eq!(
            fs::read_to_string(common.join("Mix.m3u")).unwrap(),
            "/Artist/A/01.flac\n/Artist/B/02.mp3\n"
        );
        fs::remove_dir_all(common).unwrap();
    }

    #[test]
    fn execute_refuses_a_stale_confirmation_token() {
        let common = scratch_root("token");
        let entries = sample_entries();
        let plan = plan_write(&common, "Mix.m3u", &tracks(&entries, &[0]), &entries).unwrap();
        assert!(execute(&common, &plan, "wrong").is_err());
        assert!(!common.join("Mix.m3u").exists());
        fs::remove_dir_all(common).unwrap();
    }

    #[test]
    fn rename_moves_the_file_and_keeps_its_tracks() {
        let common = scratch_root("rename");
        let entries = sample_entries();
        let created = plan_write(&common, "Mix.m3u", &tracks(&entries, &[0, 1]), &entries).unwrap();
        execute(&common, &created, &created.id).unwrap();
        let renamed = plan_rename(&common, "Mix.m3u", "Favourites.m3u").unwrap();
        assert_eq!(renamed.tracks, created.tracks);
        execute(&common, &renamed, &renamed.id).unwrap();
        assert!(!common.join("Mix.m3u").exists());
        assert_eq!(
            fs::read_to_string(common.join("Favourites.m3u")).unwrap(),
            "/Artist/B/02.mp3\n/Artist/A/01.flac\n"
        );
        fs::remove_dir_all(common).unwrap();
    }

    #[test]
    fn rename_refuses_a_name_collision() {
        let common = scratch_root("rename-collision");
        let entries = sample_entries();
        let a = plan_write(&common, "A.m3u", &tracks(&entries, &[0]), &entries).unwrap();
        execute(&common, &a, &a.id).unwrap();
        let b = plan_write(&common, "B.m3u", &tracks(&entries, &[1]), &entries).unwrap();
        execute(&common, &b, &b.id).unwrap();
        assert!(plan_rename(&common, "A.m3u", "B.m3u").is_err());
        fs::remove_dir_all(common).unwrap();
    }

    #[test]
    fn import_matches_rooted_and_bare_lines_and_drops_the_rest() {
        let common = scratch_root("import");
        let entries = sample_entries();
        let source = common.parent().unwrap().join(format!(
            "external-{}.m3u",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(
            &source,
            format!(
                "{}\n01.flac\nno-such-track.mp3\n",
                common.join("Artist/B/02.mp3").to_string_lossy()
            ),
        )
        .unwrap();
        let plan = plan_import(&common, &source, "Imported.m3u", &entries).unwrap();
        assert_eq!(plan.tracks, vec!["Artist/B/02.mp3", "Artist/A/01.flac"]);
        assert_eq!(plan.dropped, vec!["no-such-track.mp3"]);
        execute(&common, &plan, &plan.id).unwrap();
        assert_eq!(
            fs::read_to_string(common.join("Imported.m3u")).unwrap(),
            "/Artist/B/02.mp3\n/Artist/A/01.flac\n"
        );
        fs::remove_file(source).unwrap();
        fs::remove_dir_all(common).unwrap();
    }

    #[test]
    fn write_refuses_a_path_that_escapes_the_media_root() {
        let common = scratch_root("escape");
        let entries = sample_entries();
        assert!(plan_write(&common, "../outside.m3u", &tracks(&entries, &[0]), &entries).is_err());
        assert!(plan_write(&common, "not-an-m3u.txt", &tracks(&entries, &[0]), &entries).is_err());
        fs::remove_dir_all(common).unwrap();
    }

    #[test]
    fn write_refuses_a_track_not_in_the_library() {
        let common = scratch_root("unknown-track");
        let entries = sample_entries();
        assert!(plan_write(
            &common,
            "Mix.m3u",
            &["Nowhere/ghost.mp3".to_string()],
            &entries
        )
        .is_err());
        fs::remove_dir_all(common).unwrap();
    }
}
