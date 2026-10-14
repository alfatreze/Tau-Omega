//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

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
