//! Album-level library operations behind the library workbench screen:
//! list a media root as albums and tracks, plan copying a *selection* of
//! albums, and plan/execute removing albums from a card.
//!
//! Everything here follows the engine's existing plan -> confirm -> execute
//! shape and its safety rules: sources are never modified, removals are
//! backed up first when a backup folder is given, verified before deletion,
//! and the index is rebuilt (and verified) last.

use crate::{
    ErrorCode, Progress, ProgressObserver, Stage, TauError, Warning, ascii_name, cover, playlist,
    scan_dir_with_progress, sync, tick,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

/// One album folder: a directory that directly contains audio files.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlbumInfo {
    /// Folder path relative to the listed root, `/` separated. Empty when the
    /// audio sits directly in the root.
    pub id: String,
    /// Where this folder lands on a card: `id` with every component in the
    /// device's ASCII form. Equal to `id` for a listing of a card.
    pub dest_id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<String>,
    pub tracks: usize,
    pub bytes: u64,
    pub has_cover: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrackInfo {
    pub rel: String,
    pub album_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub secs: u16,
    pub bytes: u64,
    pub format: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlaylistInfo {
    pub name: String,
    pub file: String,
    pub tracks: usize,
}

/// The Pocket index's hard capacity limits, so a front-end can show how full
/// the library is and stop before an over-limit sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndexLimits {
    pub max_tracks: usize,
    pub max_albums: usize,
    pub max_artists: usize,
}
impl Default for IndexLimits {
    fn default() -> Self {
        Self {
            max_tracks: crate::MAX_TRACKS,
            max_albums: crate::MAX_ALBUMS,
            max_artists: crate::MAX_ARTISTS,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LibraryListing {
    pub limits: IndexLimits,
    pub albums: Vec<AlbumInfo>,
    pub tracks: Vec<TrackInfo>,
    pub playlists: Vec<PlaylistInfo>,
    pub warnings: Vec<Warning>,
    /// Files whose tags came from the verification ledger instead of being read from the file.
    pub files_reused: u64,
    /// Files whose tags were read from the file itself.
    pub files_read: u64,
}

/// Device ASCII form of a `/`-separated relative folder path.
pub fn dest_dir(id: &str) -> String {
    id.split('/')
        .filter(|part| !part.is_empty())
        .map(sync::ascii_file_name)
        .collect::<Vec<_>>()
        .join("/")
}

fn most_common(values: impl Iterator<Item = String>) -> Option<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for value in values.filter(|v| !v.trim().is_empty()) {
        *counts.entry(value).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(value, _)| value)
}

/// Lists a media root as albums (one per audio folder), tracks and playlists.
/// Read-only.
pub fn list_library(
    root: &Path,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<LibraryListing, TauError> {
    if !root.is_dir() {
        return Err(TauError::e(
            ErrorCode::NotFound,
            format!("not a folder: {}", root.display()),
        ));
    }
    let scan = scan_dir_with_progress(root, true, progress)?;
    let mut by_dir: BTreeMap<String, Vec<&crate::Entry>> = BTreeMap::new();
    for entry in &scan.entries {
        by_dir.entry(entry.dir.clone()).or_default().push(entry);
    }
    let root_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut listing = LibraryListing {
        warnings: scan.warnings.clone(),
        files_reused: scan.reused,
        files_read: scan.read,
        ..Default::default()
    };
    for (dir, entries) in by_dir {
        let tag = |e: &crate::Entry, key: &str| e.tags.get(key).cloned().unwrap_or_default();
        let folder_name = dir
            .rsplit('/')
            .next()
            .filter(|n| !n.is_empty())
            .unwrap_or(&root_name)
            .to_string();
        let title = most_common(entries.iter().map(|e| tag(e, "TALB"))).unwrap_or(folder_name);
        let artist = most_common(entries.iter().map(|e| tag(e, "TPE2")))
            .or_else(|| most_common(entries.iter().map(|e| tag(e, "TPE1"))))
            .unwrap_or_default();
        let year = most_common(
            entries
                .iter()
                .map(|e| format!("{}{}", tag(e, "TDRC"), tag(e, "TYER"))),
        )
        .map(|y| y.chars().take(4).collect::<String>())
        .filter(|y| y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()));
        let mut bytes = 0;
        for entry in &entries {
            let size = fs::metadata(root.join(&entry.rel))
                .map(|m| m.len())
                .unwrap_or(0);
            bytes += size;
            listing.tracks.push(TrackInfo {
                rel: entry.rel.clone(),
                album_id: dir.clone(),
                title: {
                    let t = tag(entry, "TIT2");
                    if t.is_empty() {
                        entry
                            .file
                            .rsplit_once('.')
                            .map_or(entry.file.clone(), |(stem, _)| stem.to_string())
                    } else {
                        t
                    }
                },
                artist: {
                    let a = tag(entry, "TPE1");
                    if a.is_empty() { tag(entry, "TPE2") } else { a }
                },
                album: tag(entry, "TALB"),
                secs: entry.secs,
                bytes: size,
                format: if entry.fmt == 2 { "FLAC" } else { "MP3" }.into(),
            });
        }
        let has_cover = cover::find_cover(&root.join(&dir)).is_some();
        listing.albums.push(AlbumInfo {
            dest_id: dest_dir(&dir),
            id: dir,
            title,
            artist,
            year,
            tracks: entries.len(),
            bytes,
            has_cover,
        });
    }
    listing.albums.sort_by(|a, b| {
        (a.artist.to_lowercase(), a.title.to_lowercase())
            .cmp(&(b.artist.to_lowercase(), b.title.to_lowercase()))
    });
    listing.playlists = scan
        .playlists
        .iter()
        .map(|p| PlaylistInfo {
            name: p.name.clone(),
            file: p.file.clone(),
            tracks: p.rel_ids.len(),
        })
        .collect();
    Ok(listing)
}

/// Counts the audio files under a media root and the folders that hold them,
/// without reading any tags (cheap enough to run on every plan).
pub(crate) fn count_audio(
    root: &Path,
) -> Result<(usize, std::collections::BTreeSet<String>), TauError> {
    fn walk(
        root: &Path,
        at: &Path,
        tracks: &mut usize,
        dirs: &mut std::collections::BTreeSet<String>,
    ) -> Result<(), TauError> {
        for child in fs::read_dir(at)? {
            let path = child?.path();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.starts_with("._") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, tracks, dirs)?;
            } else if sync::audio_file(&path) {
                *tracks += 1;
                let dir = path
                    .parent()
                    .and_then(|p| p.strip_prefix(root).ok())
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                dirs.insert(dir);
            }
        }
        Ok(())
    }
    let mut tracks = 0;
    let mut dirs = std::collections::BTreeSet::new();
    walk(root, root, &mut tracks, &mut dirs)?;
    Ok((tracks, dirs))
}

/// One album's cover thumbnail: a small PNG, base64-encoded, or `None` when the
/// album has no readable picture (the UI then shows a placeholder).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Thumbnail {
    pub id: String,
    pub png_base64: Option<String>,
}

/// Cover thumbnails for the given album folders under `root`: the folder's
/// cover picture if it has one, otherwise the picture embedded in its first
/// track. Never fails the batch because one album has no or a broken picture.
/// Read-only.
pub fn album_thumbnails(
    root: &Path,
    ids: &[String],
    long_side: u16,
) -> Result<Vec<Thumbnail>, TauError> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let picture = checked_dir(root, id).ok().and_then(|dir| {
            let bytes = match cover::find_cover(&dir) {
                Some(file) => fs::read(file).ok(),
                None => direct_files(&dir)
                    .ok()?
                    .into_iter()
                    .filter(|f| sync::audio_file(f))
                    .find_map(|f| cover::extract_embedded_cover(&f)),
            }?;
            if bytes.len() > 16 << 20 {
                return None;
            }
            crate::image::thumbnail_png(&bytes, long_side).ok()
        });
        out.push(Thumbnail {
            id: id.clone(),
            png_base64: picture.map(|p| STANDARD.encode(p)),
        });
    }
    Ok(out)
}

/// Rejects an album id that could leave `root` (absolute, `..`, prefixes).
pub(crate) fn checked_dir(root: &Path, id: &str) -> Result<PathBuf, TauError> {
    let relative = Path::new(id);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            format!("album id must be a relative folder: {id}"),
        ));
    }
    let dir = root.join(relative);
    if !dir.is_dir() {
        return Err(TauError::e(
            ErrorCode::NotFound,
            format!("album folder not found: {id}"),
        ));
    }
    Ok(dir)
}

/// Audio (and album-local playlist) files directly in `dir`, sorted.
pub(crate) fn direct_files(dir: &Path) -> Result<Vec<PathBuf>, TauError> {
    let mut files = Vec::new();
    for child in fs::read_dir(dir)? {
        let child = child?;
        let name = child.file_name().to_string_lossy().into_owned();
        let path = child.path();
        if path.is_file() && !sync::is_junk(&name) && sync::supported(&path) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Plans copying the chosen album folders of `library_root` onto the card
/// media root `common`. Each album keeps its folder structure relative to
/// `library_root` (device-ASCII names), so re-adding an album that is already
/// on the card is an in-place update, not a duplicate. Sources are read-only.
pub fn plan_selection(
    library_root: &Path,
    album_ids: &[String],
    common: &Path,
    root_prefix: &str,
    options: sync::PlanOptions,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<sync::SyncPlan, TauError> {
    if album_ids.is_empty() {
        return Err(TauError::e(
            ErrorCode::NoSources,
            "select at least one album",
        ));
    }
    let mut candidates = Vec::new();
    for id in album_ids {
        let dir = checked_dir(library_root, id)?;
        let dest = dest_dir(id);
        for file in direct_files(&dir)? {
            let name = sync::ascii_file_name(&file.file_name().unwrap().to_string_lossy());
            let relative = if dest.is_empty() {
                PathBuf::from(name)
            } else {
                Path::new(&dest).join(name)
            };
            candidates.push((file, relative));
        }
    }
    sync::plan_candidates(candidates, common, root_prefix, options, progress)
}

/// A reviewed, not-yet-applied removal of whole albums from a card.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RemovalPlan {
    pub id: String,
    pub destination: PathBuf,
    pub albums: Vec<String>,
    pub items: Vec<sync::DeleteItem>,
    pub bytes: u64,
    /// Playlists that reference a removed track and will be rewritten without it.
    pub playlist_updates: Vec<playlist::PlaylistPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RemovalReport {
    pub plan_id: String,
    pub deleted: usize,
    pub bytes_removed: u64,
    /// Where the removed files were copied first, if a backup folder was used.
    pub backup_dir: Option<PathBuf>,
    pub playlists_updated: usize,
    pub index_path: PathBuf,
}

/// Plans removing whole albums from the card media root `common`: every
/// file directly in each album folder (audio, folder cover, art sidecar).
/// Sub-folders are separate albums and are left alone. Read-only.
pub fn plan_removal(common: &Path, album_ids: &[String]) -> Result<RemovalPlan, TauError> {
    sync::validate_media_root(common)?;
    if album_ids.is_empty() {
        return Err(TauError::e(
            ErrorCode::NoSources,
            "select at least one album to remove",
        ));
    }
    let destination = common.canonicalize()?;
    let scan = crate::scan_dir(&destination, true)?;
    let mut items = Vec::new();
    let mut removed_rels = std::collections::BTreeSet::new();
    for id in album_ids {
        let dir = checked_dir(&destination, id)?;
        for child in fs::read_dir(&dir)? {
            let path = child?.path();
            if !path.is_file() {
                continue;
            }
            let relative = path.strip_prefix(&destination).unwrap().to_path_buf();
            if sync::supported(&path) {
                removed_rels.insert(relative.to_string_lossy().replace('\\', "/"));
            }
            items.push(sync::DeleteItem {
                bytes: fs::metadata(&path)?.len(),
                sha256: sync::sha256_file(&path)?,
                destination: path,
                relative,
            });
        }
        // The optional per-album art sidecar folder is removed with the album.
        let art = dir.join("tau-art");
        if art.is_dir() {
            for child in fs::read_dir(&art)? {
                let path = child?.path();
                if path.is_file() {
                    items.push(sync::DeleteItem {
                        bytes: fs::metadata(&path)?.len(),
                        sha256: sync::sha256_file(&path)?,
                        relative: path.strip_prefix(&destination).unwrap().to_path_buf(),
                        destination: path,
                    });
                }
            }
        }
    }
    items.sort_by(|a, b| a.relative.cmp(&b.relative));
    if items.is_empty() {
        return Err(TauError::e(
            ErrorCode::NotFound,
            "the selected albums contain no files",
        ));
    }
    let mut playlist_updates = Vec::new();
    for pl in &scan.playlists {
        let remaining: Vec<String> = pl
            .rel_ids
            .iter()
            .map(|&i| scan.entries[i].rel.clone())
            .filter(|rel| !removed_rels.contains(rel))
            .collect();
        if remaining.len() != pl.rel_ids.len() {
            playlist_updates.push(playlist::plan_write(
                &destination,
                &pl.file,
                &remaining,
                &scan.entries,
            )?);
        }
    }
    let bytes = items.iter().map(|i| i.bytes).sum();
    let mut hasher = Sha256::new();
    hasher.update(b"removal-v1");
    for item in &items {
        hasher.update(item.relative.to_string_lossy().as_bytes());
        hasher.update(item.sha256.as_bytes());
    }
    for update in &playlist_updates {
        hasher.update(update.id.as_bytes());
    }
    Ok(RemovalPlan {
        id: format!("{:x}", hasher.finalize()),
        destination,
        albums: album_ids.to_vec(),
        items,
        bytes,
        playlist_updates,
    })
}

/// Applies a reviewed removal. With `backup_root`, each file is copied there
/// (and verified) before it is deleted; the backup folder must be outside the
/// card media root. Files that changed since the plan are refused. Playlists
/// are rewritten without the removed tracks and the index is rebuilt last.
pub fn execute_removal(
    plan: &RemovalPlan,
    confirmation: &str,
    backup_root: Option<&Path>,
    root_prefix: &str,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<RemovalReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    sync::validate_media_root(&plan.destination)?;
    if let Some(backup) = backup_root
        && (backup.as_os_str().is_empty() || sync::backup_is_inside(backup, &plan.destination))
    {
        return Err(TauError::e(
            ErrorCode::UnsafeBackupLocation,
            "backup folder must be outside the card media root",
        ));
    }
    let total = plan.items.len() as u64;
    // Phase 0: a backup that cannot fit must stop the run before anything is
    // touched (a full host disk used to be discovered halfway through an album).
    if let Some(backup) = backup_root {
        let existing = crate::storage::nearest_existing_ancestor(backup)?;
        let unit = fs4::allocation_granularity(&existing)
            .ok()
            .filter(|u| *u > 0)
            .unwrap_or(4096);
        let needed: u64 = plan
            .items
            .iter()
            .map(|i| sync::round_up_to(i.bytes, unit))
            .sum();
        if let Ok(available) = fs4::available_space(&existing) {
            sync::ensure_space(available, needed, crate::storage::DEFAULT_MARGIN_BYTES)?;
        }
    }
    // Phase 1: prove every file is what was reviewed and (with a backup) copy it
    // out and verify the copy. Nothing on the card is deleted in this phase, so a
    // failure or a cancel here never leaves a half-removed album.
    for (done, item) in plan.items.iter().enumerate() {
        tick(
            progress,
            Progress {
                stage: Stage::Copying,
                done: done as u64,
                total,
                path: Some(item.relative.to_string_lossy().into_owned()),
            },
        )?;
        match backup_root {
            Some(backup) => sync::backup_copy_streaming(item, backup, &plan.id)?,
            None => {
                if sync::sha256_file(&item.destination).ok().as_deref()
                    != Some(item.sha256.as_str())
                {
                    return Err(TauError::e(
                        ErrorCode::SourceChangedSincePlan,
                        format!(
                            "changed since the plan was reviewed: {}",
                            item.relative.display()
                        ),
                    ));
                }
            }
        }
    }
    // Phase 2: every file is now backed up and verified. Delete them all. A cancel
    // request is deliberately ignored from here on so an album is never left
    // half-deleted; this phase is only file removals and is quick.
    for (done, item) in plan.items.iter().enumerate() {
        let _ = tick(
            progress,
            Progress {
                stage: Stage::Deleting,
                done: done as u64,
                total,
                path: Some(item.relative.to_string_lossy().into_owned()),
            },
        );
        fs::remove_file(&item.destination)?;
    }
    // Tidy folders the removal emptied (deepest first); never the root.
    let mut dirs: Vec<PathBuf> = plan
        .items
        .iter()
        .filter_map(|i| i.destination.parent().map(Path::to_path_buf))
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for dir in dirs {
        if dir != plan.destination && fs::remove_dir(&dir).is_ok() {
            // removed an emptied folder; a non-empty one is left as is
        }
    }
    for update in &plan.playlist_updates {
        playlist::execute(&plan.destination, update, &update.id)?;
    }
    let mut warnings = Vec::new();
    sync::rebuild_index(
        &plan.destination,
        root_prefix,
        &plan.id,
        &mut warnings,
        progress,
    )?;
    Ok(RemovalReport {
        plan_id: plan.id.clone(),
        deleted: plan.items.len(),
        bytes_removed: plan.bytes,
        backup_dir: backup_root.map(|b| b.join(&plan.id)),
        playlists_updated: plan.playlist_updates.len(),
        index_path: plan.destination.join("tau-library.tdb"),
    })
}

/// The device-ASCII display of a name, exposed so a front-end can preview
/// what a rename will look like on the Pocket.
pub fn device_name(input: &str) -> String {
    ascii_name(input)
}

#[cfg(test)]
mod tests;
