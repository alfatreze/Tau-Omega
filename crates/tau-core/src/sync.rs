//! T2's intentionally narrow write boundary: make a pure plan first, then
//! execute that exact plan after an explicit token confirmation.

use crate::{
    ErrorCode, Progress, ProgressObserver, Stage, TauError, Warning, WarningCode, ascii_name,
    build_index, cover, image, parse, scan_dir_with_progress, tick, verify,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CopyItem {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub cover: Option<CoverItem>,
    pub state: CopyState,
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoverItem {
    pub source: PathBuf,
    pub sha256: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum CopyState {
    New,
    Update,
    Same,
}
/// A folder-level `tau-art/cover_128.pal256.timg` sidecar this plan would
/// write. One entry per album folder (not per track), since the cover is
/// shared by every track in it — matching `tools/sync_media.py
/// --art-variants`'s own one-file-per-album convention.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArtSidecarItem {
    pub source_folder: PathBuf,
    pub destination: PathBuf,
    pub cover_source: PathBuf,
    pub cover_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeleteItem {
    pub destination: PathBuf,
    pub relative: PathBuf,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SyncPlan {
    pub id: String,
    pub destination: PathBuf,
    pub root_prefix: String,
    pub items: Vec<CopyItem>,
    pub deletions: Vec<DeleteItem>,
    pub embed_covers: bool,
    pub art_sidecars: Vec<ArtSidecarItem>,
    pub warnings: Vec<Warning>,
    pub bytes_to_write: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SyncReport {
    pub plan_id: String,
    pub copied: usize,
    pub unchanged: usize,
    pub bytes_written: u64,
    pub deleted: usize,
    pub art_sidecars_written: usize,
    pub index_path: PathBuf,
    pub index_sha256: String,
    pub warnings: Vec<Warning>,
}

/// Options for [`plan`]. `Default` gives the plain behaviour: add sources
/// under the destination, no mirror deletion, no cover embedding.
///
/// Replaces the four-deep `plan` -> `plan_with_options` -> `plan_with_features`
/// -> `plan_with_layout` telescoping-constructor chain (P1-3): one public
/// entry point, one options struct, so a new capability adds a field here
/// instead of another wrapper function.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlanOptions {
    /// Delete destination files that no longer correspond to a source.
    pub mirror: bool,
    /// Embed a discovered folder cover into destination MP3/FLAC copies. A
    /// cover hash is part of the plan token, preventing an unreviewed
    /// replacement.
    pub embed_covers: bool,
    /// Write a `tau-art/cover_128.pal256.timg` sidecar next to each album
    /// folder that has a discovered cover, encoded per tau-alpha's decided
    /// default (`IMAGE_FORMATS.md` D-I01/D-I02). Forward-prep: no firmware
    /// reader exists yet, so this has no effect on the Pocket itself today
    /// (`docs/FIRMWARE_SYNC.md`'s "Watched interfaces"); it does feed this
    /// app's own decode-and-preview path.
    pub art_sidecar_pal256: bool,
}

/// Produces a read-only sync plan. Sources are copied below `common` using
/// ASCII-safe names and never modified. `common` must be a Tau media root.
pub fn plan(
    sources: &[PathBuf],
    common: &Path,
    root_prefix: &str,
    options: PlanOptions,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<SyncPlan, TauError> {
    plan_with_layout(sources, common, root_prefix, options, true, progress)
}

/// Plans a whole-library copy from one explicit Tau media root to another.
/// Destination paths mirror the source exactly; no containing source-folder is
/// introduced. This is the non-destructive first half of multi-core transfer.
pub fn plan_core_copy(
    source_common: &Path,
    destination_common: &Path,
    root_prefix: &str,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<SyncPlan, TauError> {
    validate_media_root(source_common)?;
    if source_common.canonicalize()? == destination_common.canonicalize()? {
        return Err(TauError::e(
            ErrorCode::SamePath,
            "source and destination core media roots must differ",
        ));
    }
    plan_with_layout(
        &[source_common.to_path_buf()],
        destination_common,
        root_prefix,
        PlanOptions::default(),
        false,
        progress,
    )
}

fn plan_with_layout(
    sources: &[PathBuf],
    common: &Path,
    root_prefix: &str,
    options: PlanOptions,
    include_source_root: bool,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<SyncPlan, TauError> {
    validate_media_root(common)?;
    if sources.is_empty() {
        return Err(TauError::e(
            ErrorCode::NoSources,
            "at least one source is required",
        ));
    }
    let mut candidates = Vec::new();
    for source in sources {
        if !source.exists() {
            return Err(TauError::e(
                ErrorCode::SourceMissing,
                format!("source does not exist: {}", source.display()),
            ));
        }
        if source.is_dir() {
            collect_source(source, source, include_source_root, &mut candidates)?;
        } else {
            candidates.push((
                source.clone(),
                PathBuf::from(ascii_file_name(&required_file_name(source)?)),
            ));
        }
    }
    plan_candidates(candidates, common, root_prefix, options, progress)
}

/// Plans copying an explicit list of `(source file, relative destination)`
/// pairs below `common`. This is the planning core shared by whole-folder
/// plans and by album selections (`workbench::plan_selection`).
pub(crate) fn plan_candidates(
    mut candidates: Vec<(PathBuf, PathBuf)>,
    common: &Path,
    root_prefix: &str,
    options: PlanOptions,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<SyncPlan, TauError> {
    let PlanOptions {
        mirror,
        embed_covers,
        art_sidecar_pal256,
    } = options;
    validate_media_root(common)?;
    if candidates.is_empty() {
        return Err(TauError::e(
            ErrorCode::NoSources,
            "at least one source is required",
        ));
    }
    let destination = common
        .canonicalize()
        .unwrap_or_else(|_| common.to_path_buf());
    candidates.sort_by(|a, b| a.1.cmp(&b.1));
    let mut seen = std::collections::BTreeSet::new();
    let mut seen_art_folders = std::collections::BTreeSet::new();
    let mut items = Vec::new();
    let mut art_sidecars = Vec::new();
    let mut warnings = Vec::new();
    let mut bytes_to_write = 0;
    let total = candidates.len() as u64;
    for (done, (source, relative)) in candidates.into_iter().enumerate() {
        tick(
            progress,
            Progress {
                stage: Stage::Hashing,
                done: done as u64,
                total,
                path: Some(relative.to_string_lossy().into_owned()),
            },
        )?;
        // FAT/exFAT ignore case, so `Song.mp3` and `song.mp3` are the same file on
        // the card: the second copy would silently replace the first.
        if !seen.insert(relative.to_string_lossy().to_lowercase()) {
            return Err(TauError::e(
                ErrorCode::NameCollision,
                format!("name collision (the card ignores letter case): {}", relative.display()),
            ));
        }
        let target = destination.join(&relative);
        if source == target {
            return Err(TauError::e(
                ErrorCode::SamePath,
                format!(
                    "source and destination are the same file: {}",
                    source.display()
                ),
            ));
        }
        let bytes = fs::metadata(&source)?.len();
        let sha256 = sha256_file(&source)?;
        let cover = if embed_covers && audio_file(&source) {
            match source.parent().and_then(cover::find_cover) {
                Some(path) => {
                    cover::validate_jpeg_cover(&path)?;
                    Some(CoverItem {
                        sha256: sha256_file(&path)?,
                        source: path,
                    })
                }
                None => None,
            }
        } else {
            None
        };
        if art_sidecar_pal256
            && audio_file(&source)
            && let Some(source_folder) = source.parent()
            && seen_art_folders.insert(source_folder.to_path_buf())
            && let Some(cover_source) = cover::find_cover(source_folder)
        {
            let destination_folder = target.parent().ok_or_else(|| {
                TauError::e(
                    ErrorCode::InvalidPathReference,
                    "destination file has no parent folder",
                )
            })?;
            art_sidecars.push(ArtSidecarItem {
                source_folder: source_folder.to_path_buf(),
                destination: destination_folder.join(image::pal256_sidecar_name(128)),
                cover_sha256: sha256_file(&cover_source)?,
                cover_source,
            });
        }
        let mut state = if target.is_file()
            && fs::metadata(&target)?.len() == bytes
            && sha256_file(&target).ok().as_deref() == Some(&sha256)
        {
            CopyState::Same
        } else if target.exists() {
            CopyState::Update
        } else {
            CopyState::New
        };
        // Artwork changes the resulting bytes, so an existing raw source copy
        // must be explicitly updated when a cover is requested.
        if cover.is_some() && state == CopyState::Same {
            state = CopyState::Update;
        }
        if state != CopyState::Same {
            bytes_to_write += bytes;
        }
        items.push(CopyItem {
            source,
            destination: target,
            bytes,
            sha256,
            cover,
            state,
        });
    }
    if items.is_empty() {
        warnings.push(Warning::new(
            WarningCode::NoMediaFound,
            "No supported audio or playlist files were found in the selected sources.",
        ));
    }
    let deletions = if mirror {
        mirror_deletions(&destination, &items)?
    } else {
        Vec::new()
    };
    let mut hasher = Sha256::new();
    for item in &items {
        hasher.update(item.source.to_string_lossy().as_bytes());
        hasher.update(item.destination.to_string_lossy().as_bytes());
        hasher.update(item.sha256.as_bytes());
        if let Some(cover) = &item.cover {
            hasher.update(cover.source.to_string_lossy().as_bytes());
            hasher.update(cover.sha256.as_bytes());
        }
    }
    for item in &art_sidecars {
        hasher.update(item.destination.to_string_lossy().as_bytes());
        hasher.update(item.cover_sha256.as_bytes());
    }
    for item in &deletions {
        hasher.update(item.relative.to_string_lossy().as_bytes());
        hasher.update(item.sha256.as_bytes());
    }
    hasher.update([mirror as u8]);
    hasher.update([embed_covers as u8]);
    hasher.update([art_sidecar_pal256 as u8]);
    hasher.update(root_prefix.as_bytes());
    // The full SHA-256 hex digest (P2-1): a 32-bit truncation is thin for a
    // token that may be persisted or handed across a process boundary, and
    // the old `T2-` prefix leaked an internal roadmap phase label into a
    // durable identifier. Confirmation is a plain string comparison
    // (`execute*` checks `confirmation == plan.id`), so widening it and
    // dropping the prefix changes nothing about how a caller uses it.
    let id = format!("{:x}", hasher.finalize());
    Ok(SyncPlan {
        id,
        destination,
        root_prefix: root_prefix.into(),
        items,
        deletions,
        embed_covers,
        art_sidecars,
        warnings,
        bytes_to_write,
    })
}

/// Executes a freshly reviewed plan. Every copied file is SHA-256 verified;
/// the index is written last via a temporary file and parsed before rename.
pub fn execute(
    plan: &SyncPlan,
    confirmation: &str,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<SyncReport, TauError> {
    execute_with_mirror(plan, confirmation, None, None, progress)
}

/// Mirror deletion is deliberately a separate confirmation and requires an
/// external backup folder. Copy/verify always finishes before a deletion starts.
pub fn execute_with_mirror(
    plan: &SyncPlan,
    confirmation: &str,
    delete_confirmation: Option<&str>,
    backup_root: Option<&Path>,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<SyncReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    validate_media_root(&plan.destination)?;
    preflight_space(plan)?;
    // Repair what an earlier interrupted run left behind before adding to it.
    recover_index(&plan.destination)?;
    sweep_stale_temps(&plan.destination, plan.items.iter().map(|i| i.destination.as_path()));
    let mut copied = 0;
    let mut unchanged = 0;
    let mut bytes_written = 0;
    let copy_total = plan.bytes_to_write;
    let mut copy_done = 0;
    for item in &plan.items {
        match item.state {
            CopyState::Same => unchanged += 1,
            CopyState::New | CopyState::Update => {
                tick(
                    progress,
                    Progress {
                        stage: Stage::Copying,
                        done: copy_done,
                        total: copy_total,
                        path: Some(item.destination.to_string_lossy().into_owned()),
                    },
                )?;
                copy_verified(item)?;
                copied += 1;
                bytes_written += item.bytes;
                copy_done += item.bytes;
            }
        }
    }
    let mut art_sidecars_written = 0;
    for item in &plan.art_sidecars {
        if sha256_file(&item.cover_source).ok().as_deref() != Some(item.cover_sha256.as_str()) {
            return Err(TauError::e(
                ErrorCode::SourceChangedSincePlan,
                format!(
                    "cover changed since the plan was reviewed: {}",
                    item.cover_source.display()
                ),
            ));
        }
        let packed = image::encode_pal256_bytes(&fs::read(&item.cover_source)?, 128)?;
        if let Some(parent) = item.destination.parent() {
            fs::create_dir_all(parent)?;
        }
        // Temp file, verify, then rename: a card pulled mid-write must not leave a
        // truncated sidecar in place of a previous good one.
        let temp = item
            .destination
            .with_extension(format!("tau-omega-{}.tmp", std::process::id()));
        write_durable(&temp, &packed)?;
        // Read the just-written file back and require it to be exactly what was
        // meant (and a valid TIM1), the same write-then-verify discipline as
        // every other write path here.
        let read_back = read_back_bytes(&temp)?;
        if read_back != packed || image::decode_tim1(&read_back).is_err() {
            let _ = fs::remove_file(&temp);
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                format!("cover file verification failed: {}", item.destination.display()),
            ));
        }
        fs::rename(&temp, &item.destination)?;
        art_sidecars_written += 1;
    }
    let mut deleted = 0;
    if !plan.deletions.is_empty() {
        if delete_confirmation != Some(plan.id.as_str()) {
            return Err(TauError::e(
                ErrorCode::ConfirmationMismatch,
                "mirror deletions need a second matching confirmation token",
            ));
        }
        let backup_root = backup_root.ok_or_else(|| {
            TauError::e(
                ErrorCode::UnsafeBackupLocation,
                "mirror deletions need a visible backup folder",
            )
        })?;
        if backup_is_inside(backup_root, &plan.destination) {
            return Err(TauError::e(
                ErrorCode::UnsafeBackupLocation,
                "backup folder must be outside the card media root",
            ));
        }
        let delete_total = plan.deletions.len() as u64;
        for (done, item) in plan.deletions.iter().enumerate() {
            tick(
                progress,
                Progress {
                    stage: Stage::Deleting,
                    done: done as u64,
                    total: delete_total,
                    path: Some(item.relative.to_string_lossy().into_owned()),
                },
            )?;
            backup_then_delete(item, backup_root, &plan.id)?;
            deleted += 1;
        }
    }
    let index_path = plan.destination.join("tau-library.tdb");
    if copied == 0 && index_path.is_file() {
        let current = fs::read(&index_path)?;
        if parse(&current).is_ok() && verify(&current, Some(&plan.destination))?.is_empty() {
            return Ok(SyncReport {
                plan_id: plan.id.clone(),
                copied,
                unchanged,
                bytes_written,
                deleted,
                art_sidecars_written,
                index_path,
                index_sha256: sha256_bytes(&current),
                warnings: plan.warnings.clone(),
            });
        }
    }
    let scan = scan_dir_with_progress(&plan.destination, true, progress)?;
    let mut warnings = plan.warnings.clone();
    warnings.extend(scan.warnings);
    let index = build_index(
        &scan.entries,
        &scan.playlists,
        &plan.root_prefix,
        &mut warnings,
    )?;
    parse(&index)?;
    let temp = plan
        .destination
        .join(format!(".tau-library-{}.tmp", plan.id));
    write_durable(&temp, &index)?;
    let reparse = read_back_bytes(&temp)?;
    parse(&reparse)?;
    if reparse != index {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            "index write verification failed",
        ));
    }
    swap_in_index(&temp, &index_path)?;
    Ok(SyncReport {
        plan_id: plan.id.clone(),
        copied,
        unchanged,
        bytes_written,
        deleted,
        art_sidecars_written,
        index_path,
        index_sha256: sha256_bytes(&index),
        warnings,
    })
}

/// Moves a complete core library only after its destination copy and index
/// have verified. Source deletion has its own matching token and every source
/// file is copied to an external backup before removal. The source index is
/// rebuilt last so both cores remain loadable after a successful move.
pub fn execute_core_move(
    plan: &SyncPlan,
    confirmation: &str,
    delete_confirmation: &str,
    source_common: &Path,
    source_root_prefix: &str,
    backup_root: &Path,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<SyncReport, TauError> {
    if confirmation != plan.id || delete_confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "a core move needs both matching copy and delete confirmation tokens",
        ));
    }
    validate_media_root(source_common)?;
    if backup_root.as_os_str().is_empty() {
        return Err(TauError::e(
            ErrorCode::UnsafeBackupLocation,
            "move backup folder must be an explicit host path",
        ));
    }
    let source_common = source_common.canonicalize()?;
    if backup_is_inside(backup_root, &source_common) || backup_is_inside(backup_root, &plan.destination) {
        return Err(TauError::e(
            ErrorCode::UnsafeBackupLocation,
            "move backup folder must be outside both core media roots",
        ));
    }
    if plan.items.iter().any(|item| {
        item.source
            .canonicalize()
            .map(|source| !source.starts_with(&source_common))
            .unwrap_or(true)
    }) {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            "move plan contains a source outside the chosen source media root",
        ));
    }
    let mut report = execute(plan, confirmation, progress)?;
    let mut deleted = 0;
    let total = plan.items.len() as u64;
    for (done, item) in plan.items.iter().enumerate() {
        tick(
            progress,
            Progress {
                stage: Stage::Deleting,
                done: done as u64,
                total,
                path: Some(item.source.to_string_lossy().into_owned()),
            },
        )?;
        let canonical_source = item.source.canonicalize()?;
        let relative = canonical_source.strip_prefix(&source_common).map_err(|_| {
            TauError::e(
                ErrorCode::InvalidPathReference,
                "move plan contains an invalid source path",
            )
        })?;
        let backup = backup_root.join(&plan.id).join(relative);
        let backup_item = CopyItem {
            source: item.source.clone(),
            destination: backup,
            bytes: item.bytes,
            sha256: item.sha256.clone(),
            cover: None,
            state: CopyState::New,
        };
        copy_verified(&backup_item)?;
        fs::remove_file(&item.source)?;
        deleted += 1;
    }
    rebuild_index(
        &source_common,
        source_root_prefix,
        &plan.id,
        &mut report.warnings,
        progress,
    )?;
    report.deleted = deleted;
    Ok(report)
}

fn mirror_deletions(destination: &Path, copies: &[CopyItem]) -> Result<Vec<DeleteItem>, TauError> {
    let keep = copies
        .iter()
        .map(|item| {
            item.destination
                .strip_prefix(destination)
                .unwrap()
                .to_path_buf()
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut all = Vec::new();
    collect_files(destination, destination, &mut all)?;
    all.sort();
    all.into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(destination).ok()?.to_path_buf();
            let name = relative.file_name()?.to_string_lossy();
            if keep.contains(&relative)
                || name == "tau-library.tdb"
                || name.starts_with(".tau-library-")
                || !supported(&path)
            {
                return None;
            }
            Some((path, relative))
        })
        .map(|(destination, relative)| {
            Ok(DeleteItem {
                bytes: fs::metadata(&destination)?.len(),
                sha256: sha256_file(&destination)?,
                destination,
                relative,
            })
        })
        .collect()
}
pub(crate) fn rebuild_index(
    media_root: &Path,
    root_prefix: &str,
    plan_id: &str,
    warnings: &mut Vec<Warning>,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<(), TauError> {
    let scan = scan_dir_with_progress(media_root, true, progress)?;
    warnings.extend(scan.warnings);
    let index = build_index(&scan.entries, &scan.playlists, root_prefix, warnings)?;
    parse(&index)?;
    let temp = media_root.join(format!(".tau-library-source-{plan_id}.tmp"));
    write_durable(&temp, &index)?;
    let reparse = read_back_bytes(&temp)?;
    if reparse != index {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            "source index write verification failed",
        ));
    }
    parse(&reparse)?;
    swap_in_index(&temp, &media_root.join("tau-library.tdb"))?;
    Ok(())
}
fn collect_files(root: &Path, at: &Path, out: &mut Vec<PathBuf>) -> Result<(), TauError> {
    for child in fs::read_dir(at)? {
        let path = child?.path();
        if path.is_dir() {
            collect_files(root, &path, out)?;
        } else if path.is_file() && path.strip_prefix(root).is_ok() {
            out.push(path);
        }
    }
    Ok(())
}
/// Backs a card file up to the host in **one pass over the card**: the bytes are
/// hashed while they are copied, the hash must equal the one the user reviewed
/// (otherwise the file changed since the plan and nothing is kept), and the
/// finished backup is read back from the host disk before it is renamed into
/// place. Does not delete anything. The old per-file route read each card file
/// four times, which over the Pocket's USB mode is the difference between
/// minutes and an hour for a large album.
pub(crate) fn backup_copy_streaming(
    item: &DeleteItem,
    backup_root: &Path,
    plan_id: &str,
) -> Result<(), TauError> {
    let backup = backup_root.join(plan_id).join(&item.relative);
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = backup.with_extension(format!("tau-omega-{}.tmp", std::process::id()));
    let result = (|| -> Result<(), TauError> {
        let mut source = fs::File::open(&item.destination)?;
        let mut target = fs::File::create(&temp)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 1 << 20];
        loop {
            let n = source.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            target.write_all(&buffer[..n])?;
        }
        target.sync_all()?;
        if format!("{:x}", hasher.finalize()) != item.sha256 {
            return Err(TauError::e(
                ErrorCode::SourceChangedSincePlan,
                format!("changed since the plan was reviewed: {}", item.relative.display()),
            ));
        }
        verify_written(&temp, &item.sha256, &item.destination)
    })();
    let result = result.and_then(|()| fs::rename(&temp, &backup).map_err(TauError::from));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub(crate) fn backup_then_delete(
    item: &DeleteItem,
    backup_root: &Path,
    plan_id: &str,
) -> Result<(), TauError> {
    let backup = backup_root.join(plan_id).join(&item.relative);
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent)?;
    }
    let copy = CopyItem {
        source: item.destination.clone(),
        destination: backup.clone(),
        bytes: item.bytes,
        sha256: item.sha256.clone(),
        cover: None,
        state: CopyState::New,
    };
    copy_verified(&copy)?;
    if sha256_file(&backup)? != item.sha256 {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            format!("backup verification failed: {}", item.relative.display()),
        ));
    }
    fs::remove_file(&item.destination)?;
    Ok(())
}

/// Resolves `path` for "is it inside that folder" comparisons even when it
/// does not exist yet: the nearest existing ancestor is canonicalised (so
/// symlinked temp/volume roots such as macOS `/var` -> `/private/var` compare
/// equal) and the not-yet-created remainder is appended unchanged.
pub(crate) fn resolve_for_compare(path: &Path) -> PathBuf {
    if let Ok(real) = path.canonicalize() {
        return real;
    }
    let mut tail = Vec::new();
    let mut cursor = path;
    while let Some(parent) = cursor.parent() {
        if let Some(name) = cursor.file_name() {
            tail.push(name.to_os_string());
        }
        if let Ok(real) = parent.canonicalize() {
            return tail.iter().rev().fold(real, |acc, part| acc.join(part));
        }
        cursor = parent;
    }
    path.to_path_buf()
}

/// Whether a backup folder lies inside `root` (the card media root), compared
/// after resolving symlinks on both sides.
pub(crate) fn backup_is_inside(backup: &Path, root: &Path) -> bool {
    resolve_for_compare(backup).starts_with(resolve_for_compare(root))
}

pub(crate) fn validate_media_root(common: &Path) -> Result<(), TauError> {
    let components: Vec<_> = common.components().collect();
    let has_assets = components
        .iter()
        .any(|c| matches!(c,Component::Normal(n) if *n == "Assets"));
    if !has_assets || common.file_name().is_none_or(|n| n != "common") {
        return Err(TauError::e(
            ErrorCode::InvalidMediaRoot,
            "destination must be an explicit Assets/<platform>/common media root",
        ));
    }
    if !common.is_dir() {
        return Err(TauError::e(
            ErrorCode::InvalidMediaRoot,
            "destination media root does not exist",
        ));
    }
    Ok(())
}
fn collect_source(
    root: &Path,
    at: &Path,
    include_root: bool,
    out: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), TauError> {
    let mut children: Vec<_> = fs::read_dir(at)?.filter_map(Result::ok).collect();
    children.sort_by_key(|e| e.file_name());
    for child in children {
        let path = child.path();
        let name = child.file_name().to_string_lossy().to_string();
        if is_junk(&name) {
            continue;
        }
        if path.is_dir() {
            collect_source(root, &path, false, out)?;
        } else if path.is_file() && supported(&path) {
            let rel = path.strip_prefix(root).unwrap();
            let mut converted = PathBuf::new();
            if include_root {
                converted.push(ascii_name(&required_file_name(root)?));
            }
            for part in rel.components() {
                converted.push(ascii_file_name(part.as_os_str().to_string_lossy().as_ref()));
            }
            out.push((path, converted));
        }
    }
    Ok(())
}
pub(crate) fn supported(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp3" | "flac" | "m3u")
    )
}
pub(crate) fn audio_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp3" | "flac")
    )
}
pub(crate) fn is_junk(name: &str) -> bool {
    name.starts_with("._") || matches!(name, ".DS_Store" | "Thumbs.db")
}
/// A source's file name, used to derive an ASCII-safe destination name.
/// `Path::file_name()` returns `None` for a handful of paths (`/`, `.`,
/// `..`, a bare prefix like `C:\`) that can still pass `.exists()`; a plain
/// `.unwrap()` here would let a caller-supplied source path panic the whole
/// plan instead of failing it cleanly (P1-2).
fn required_file_name(path: &Path) -> Result<String, TauError> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| {
            TauError::e(
                ErrorCode::InvalidPathReference,
                format!(
                    "source has no file name to derive a destination name from: {}",
                    path.display()
                ),
            )
        })
}
pub(crate) fn ascii_file_name(name: &str) -> String {
    let name = ascii_name(name);
    if name.is_empty() {
        "track".into()
    } else {
        name
    }
}
pub(crate) fn sha256_file(path: &Path) -> Result<String, TauError> {
    let mut file = fs::File::open(path)?;
    let mut h = Sha256::new();
    // On the heap: commands now run on thread-pool threads with small stacks.
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
/// Host hook that drops the operating system's cached copy of a file so the next
/// read comes from the device. Without it a read-back right after a write is
/// served from memory: measured on macOS against a disk image whose content was
/// changed underneath (a plain read returned the stale bytes; `F_NOCACHE` on the
/// reading side did not help; invalidating the file's pages did). `tau-core`
/// forbids `unsafe`, so the platform call lives in the host and is registered here.
pub type CacheEvictor = fn(&Path) -> bool;
static CACHE_EVICTOR: std::sync::OnceLock<CacheEvictor> = std::sync::OnceLock::new();
static EVICTIONS_FAILED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Registers the host's cache evictor (first registration wins).
pub fn register_cache_evictor(evictor: CacheEvictor) {
    let _ = CACHE_EVICTOR.set(evictor);
}

/// Whether read-backs are checked against the device (an evictor is registered)
/// and how many evictions have failed since start. A failure means that one
/// read-back may have been served from cache.
pub fn readback_report() -> (bool, u64) {
    (
        CACHE_EVICTOR.get().is_some(),
        EVICTIONS_FAILED.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// Drops the cached copy of `path`, if the host can; counts a failure otherwise.
pub(crate) fn evict_cache(path: &Path) {
    if let Some(evictor) = CACHE_EVICTOR.get()
        && !evictor(path)
    {
        EVICTIONS_FAILED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// A read-back of something just written: evict, then read.
pub(crate) fn read_back_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
    evict_cache(path);
    fs::read(path)
}

pub(crate) fn sha256_bytes(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
pub(crate) fn write_durable(path: &Path, data: &[u8]) -> Result<(), TauError> {
    let mut file = fs::File::create(path)?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(())
}
const LIVE_INDEX: &str = "tau-library.tdb";
const PREVIOUS_INDEX: &str = ".tau-library.tdb.prev";
/// A temp file this old cannot belong to a run that is still going.
const STALE_TEMP_AGE: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Replaces the live index with the already-verified `temp` while keeping the
/// old one as `.tau-library.tdb.prev` until the new one is in place. FAT cannot
/// rename over an existing file atomically: a card pulled mid-swap used to be
/// able to end with **no** index (and the Pocket trusts the index). Now an
/// interrupted swap always leaves a complete index to recover (`recover_index`).
pub(crate) fn swap_in_index(temp: &Path, live: &Path) -> Result<(), TauError> {
    let previous = live.with_file_name(PREVIOUS_INDEX);
    if live.is_file() {
        if previous.exists() {
            fs::remove_file(&previous)?;
        }
        fs::rename(live, &previous)?;
    }
    fs::rename(temp, live)?;
    let _ = fs::remove_file(previous);
    Ok(())
}

/// What `recover_index` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexRecovery {
    /// Nothing to repair.
    Clean,
    /// The live index was missing or unreadable; the previous complete one was put back.
    Restored,
}

/// Repairs an index swap that was interrupted (see [`swap_in_index`]). Only ever
/// puts back a previous index that parses; never invents one. Called at the
/// start of a confirmed run, and cheap enough for a UI to call read-only first
/// via [`index_needs_recovery`].
pub fn recover_index(media_root: &Path) -> Result<IndexRecovery, TauError> {
    let live = media_root.join(LIVE_INDEX);
    let previous = media_root.join(PREVIOUS_INDEX);
    if !previous.is_file() {
        return Ok(IndexRecovery::Clean);
    }
    let live_ok = fs::read(&live).ok().is_some_and(|b| parse(&b).is_ok());
    if live_ok {
        // The swap finished; only the cleanup was missed.
        let _ = fs::remove_file(&previous);
        return Ok(IndexRecovery::Clean);
    }
    if fs::read(&previous).ok().is_some_and(|b| parse(&b).is_ok()) {
        if live.exists() {
            fs::remove_file(&live)?;
        }
        fs::rename(&previous, &live)?;
        return Ok(IndexRecovery::Restored);
    }
    Ok(IndexRecovery::Clean)
}

/// Read-only: whether `recover_index` would change anything.
pub fn index_needs_recovery(media_root: &Path) -> bool {
    let previous = media_root.join(PREVIOUS_INDEX);
    previous.is_file()
        && !fs::read(media_root.join(LIVE_INDEX))
            .ok()
            .is_some_and(|b| parse(&b).is_ok())
}

/// Removes this tool's own leftover temp files (`*.tau-omega-<pid>.tmp`,
/// `.tau-library*.tmp`) older than an hour from the media root and from the
/// folders a run is about to write into. They only exist after a crash or an
/// ejected card, are never read by the scan, and otherwise eat clusters forever.
pub(crate) fn sweep_stale_temps<'a>(media_root: &Path, written: impl Iterator<Item = &'a Path>) {
    let mut dirs: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
    dirs.insert(media_root.to_path_buf());
    for path in written {
        if let Some(parent) = path.parent() {
            dirs.insert(parent.to_path_buf());
        }
    }
    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let ours = (name.contains(".tau-omega-") && name.ends_with(".tmp"))
                || (name.starts_with(".tau-library") && name.ends_with(".tmp"));
            if !ours {
                continue;
            }
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age >= STALE_TEMP_AGE);
            if old {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// Sizes a sidecar `.timg` at most (a 128 px palette cover is about 17 KB).
const ART_SIDECAR_MAX_BYTES: u64 = 24 * 1024;

/// `bytes` rounded up to whole allocation units, which is what a file really
/// costs on FAT/exFAT (clusters can be 32-128 KB, so many small files waste a lot).
pub(crate) fn round_up_to(bytes: u64, unit: u64) -> u64 {
    let unit = unit.max(1);
    bytes.div_ceil(unit) * unit
}

/// What writing this plan will occupy on the card, counting whole clusters:
/// every new or updated file (plus its embedded cover, which makes the copy
/// larger than its source), every art sidecar, and the index (written next to
/// the old one before the swap, so counted twice).
pub(crate) fn bytes_on_disk(plan: &SyncPlan, unit: u64) -> u64 {
    let mut total = 0u64;
    for item in plan
        .items
        .iter()
        .filter(|i| i.state != CopyState::Same)
    {
        let cover = item
            .cover
            .as_ref()
            .and_then(|c| fs::metadata(&c.source).ok())
            .map_or(0, |m| m.len());
        total = total.saturating_add(round_up_to(item.bytes.saturating_add(cover), unit));
    }
    total = total.saturating_add(
        (plan.art_sidecars.len() as u64).saturating_mul(round_up_to(ART_SIDECAR_MAX_BYTES, unit)),
    );
    total.saturating_add(2 * round_up_to(INDEX_MAX_BYTES, unit))
}

/// The index file's hard cap (see `DATA_FORMATS.md`).
const INDEX_MAX_BYTES: u64 = 4 * 1024 * 1024;

pub(crate) fn ensure_space(available: u64, needed: u64, margin: u64) -> Result<(), TauError> {
    if available < needed.saturating_add(margin) {
        return Err(TauError::e(
            ErrorCode::InsufficientSpace,
            format!(
                "not enough room on the card: this needs about {} MB (counting whole clusters) plus a safety margin, and {} MB is free",
                needed.div_ceil(1_000_000),
                available / 1_000_000
            ),
        ));
    }
    Ok(())
}

/// Refuses a run that cannot fit **before anything is written**. Without this a
/// full card failed halfway: some files copied, the old index left in place.
fn preflight_space(plan: &SyncPlan) -> Result<(), TauError> {
    let unit = fs4::allocation_granularity(&plan.destination)
        .ok()
        .filter(|u| *u > 0)
        .unwrap_or(4096);
    let Ok(available) = fs4::available_space(&plan.destination) else {
        return Ok(()); // cannot measure: do not block, the write itself will report a full card
    };
    ensure_space(
        available,
        bytes_on_disk(plan, unit),
        crate::storage::DEFAULT_MARGIN_BYTES,
    )
}

/// Reads `written` back and requires its SHA-256 to equal `expected`; on a
/// mismatch the temporary file is removed so nothing unverified can be renamed
/// into place. Every write path (plain copy and cover-embedded copy) ends here.
fn verify_written(written: &Path, expected: &str, source: &Path) -> Result<(), TauError> {
    evict_cache(written);
    if sha256_file(written)? != expected {
        let _ = fs::remove_file(written);
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            format!("verification failed: {}", source.display()),
        ));
    }
    Ok(())
}

fn copy_verified(item: &CopyItem) -> Result<(), TauError> {
    if let Some(parent) = item.destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = item
        .destination
        .with_extension(format!("tau-omega-{}.tmp", std::process::id()));
    if sha256_file(&item.source)? != item.sha256 {
        return Err(TauError::e(
            ErrorCode::SourceChangedSincePlan,
            format!(
                "source changed since the plan was reviewed: {}",
                item.source.display()
            ),
        ));
    }
    if let Some(cover) = &item.cover {
        if sha256_file(&cover.source)? != cover.sha256 {
            return Err(TauError::e(
                ErrorCode::SourceChangedSincePlan,
                format!(
                    "cover changed since the plan was reviewed: {}",
                    cover.source.display()
                ),
            ));
        }
        let embedded = match item
            .source
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref()
        {
            Some("mp3") => cover::embed_mp3_bytes(&item.source, &cover.source),
            Some("flac") => cover::embed_flac_bytes(&item.source, &cover.source),
            _ => unreachable!("only audio files receive cover plans"),
        }?;
        // The copy's bytes intentionally differ from the source, so there is no
        // plan hash to compare with. Hash exactly what is about to be written,
        // write it durably, then require the read-back to match it.
        let expected = sha256_bytes(&embedded);
        write_durable(&temp, &embedded).inspect_err(|_| {
            let _ = fs::remove_file(&temp);
        })?;
        drop(embedded);
        verify_written(&temp, &expected, &item.source)?;
    } else {
        {
            let mut source = fs::File::open(&item.source)?;
            let mut target = fs::File::create(&temp)?;
            io::copy(&mut source, &mut target)?;
            target.sync_all()?;
        }
        verify_written(&temp, &item.sha256, &item.source)?;
    }
    fs::rename(temp, &item.destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn space_is_counted_in_whole_clusters_and_checked_before_writing() {
        assert_eq!(round_up_to(1, 32768), 32768);
        assert_eq!(round_up_to(32768, 32768), 32768);
        assert_eq!(round_up_to(32769, 32768), 65536);
        assert_eq!(round_up_to(0, 32768), 0);
        // 1,000 one-byte files really cost 1,000 clusters, not 1,000 bytes.
        let plan = SyncPlan {
            id: String::new(),
            destination: PathBuf::new(),
            root_prefix: String::new(),
            items: (0..1000)
                .map(|n| CopyItem {
                    source: PathBuf::from(format!("{n}")),
                    destination: PathBuf::from(format!("d{n}")),
                    bytes: 1,
                    sha256: String::new(),
                    cover: None,
                    state: CopyState::New,
                })
                .collect(),
            deletions: Vec::new(),
            embed_covers: false,
            art_sidecars: Vec::new(),
            warnings: Vec::new(),
            bytes_to_write: 1000,
        };
        let needed = bytes_on_disk(&plan, 131_072);
        assert!(needed >= 1000 * 131_072, "exFAT-sized clusters: {needed}");
        // Files that are already on the card cost nothing.
        let same = SyncPlan { items: plan.items.iter().cloned().map(|mut i| { i.state = CopyState::Same; i }).collect(), ..plan.clone() };
        assert!(bytes_on_disk(&same, 131_072) < 1000 * 131_072);
        // The check refuses with its own code, and passes when there is room.
        assert_eq!(ensure_space(10, 1_000, 0).unwrap_err().code(), ErrorCode::InsufficientSpace);
        assert_eq!(ensure_space(1_000, 1_000, 1).unwrap_err().code(), ErrorCode::InsufficientSpace);
        ensure_space(2_000, 1_000, 1_000).unwrap();
    }

    fn synced_card(name: &str) -> PathBuf {
        let source = root(&format!("{name}-source"));
        let card = root(&format!("{name}-card"));
        let common = card.join("Assets/tau/common");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&common).unwrap();
        fs::write(source.join("01 Track.mp3"), b"audio one").unwrap();
        let sync_plan = plan(
            &[source],
            &common,
            "/Assets/tau/common/",
            PlanOptions { mirror: false, embed_covers: false, art_sidecar_pal256: false },
            &mut None,
        )
        .unwrap();
        execute(&sync_plan, &sync_plan.id, &mut None).unwrap();
        common
    }

    /// An index swap cut short must always leave a complete index to recover,
    /// never "no index" (FAT cannot rename over an existing file atomically).
    #[test]
    fn an_interrupted_index_swap_is_recovered_and_a_finished_one_leaves_nothing_behind() {
        let common = synced_card("index-swap");
        let live = common.join("tau-library.tdb");
        let good = fs::read(&live).unwrap();
        assert!(!common.join(".tau-library.tdb.prev").exists(), "a finished swap leaves no .prev");
        assert_eq!(recover_index(&common).unwrap(), IndexRecovery::Clean);

        // Card pulled between "move old aside" and "move new into place".
        fs::rename(&live, common.join(".tau-library.tdb.prev")).unwrap();
        assert!(index_needs_recovery(&common));
        assert_eq!(recover_index(&common).unwrap(), IndexRecovery::Restored);
        assert_eq!(fs::read(&live).unwrap(), good);
        assert!(!index_needs_recovery(&common));

        // A live index that is truncated garbage next to a good .prev.
        fs::write(common.join(".tau-library.tdb.prev"), &good).unwrap();
        fs::write(&live, b"TLIB truncated").unwrap();
        assert_eq!(recover_index(&common).unwrap(), IndexRecovery::Restored);
        assert_eq!(fs::read(&live).unwrap(), good);

        // Swap finished, cleanup missed: the stale .prev just goes away.
        fs::write(common.join(".tau-library.tdb.prev"), b"old").unwrap();
        assert_eq!(recover_index(&common).unwrap(), IndexRecovery::Clean);
        assert!(!common.join(".tau-library.tdb.prev").exists());
        assert_eq!(fs::read(&live).unwrap(), good);

        // Never invents an index: a bad .prev with no live index is left alone.
        fs::remove_file(&live).unwrap();
        fs::write(common.join(".tau-library.tdb.prev"), b"junk").unwrap();
        assert_eq!(recover_index(&common).unwrap(), IndexRecovery::Clean);
        assert!(!live.exists());
    }

    #[test]
    fn only_old_leftover_temp_files_of_this_tool_are_swept() {
        let common = synced_card("sweep");
        let album = common.join("Album");
        fs::create_dir_all(&album).unwrap();
        let old_temp = album.join("Song.tau-omega-4242.tmp");
        let fresh_temp = album.join("Other.tau-omega-4242.tmp");
        let foreign = album.join("keep.tmp");
        let index_temp = common.join(".tau-library-abc.tmp");
        for f in [&old_temp, &fresh_temp, &foreign, &index_temp] {
            fs::write(f, b"x").unwrap();
        }
        let two_hours_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 3600);
        for f in [&old_temp, &foreign, &index_temp] {
            fs::File::options().write(true).open(f).unwrap().set_modified(two_hours_ago).unwrap();
        }
        let written = [album.join("Song.mp3")];
        sweep_stale_temps(&common, written.iter().map(|p| p.as_path()));
        assert!(!old_temp.exists(), "an old leftover of ours is removed");
        assert!(!index_temp.exists(), "an old leftover index temp is removed");
        assert!(fresh_temp.exists(), "a recent temp may belong to a run in progress");
        assert!(foreign.exists(), "files that are not ours are never touched");
    }

    /// The card's file system ignores letter case, so two sources that differ only
    /// by case must be refused at plan time instead of the second silently
    /// replacing the first on the card.
    #[test]
    fn names_that_differ_only_by_case_collide() {
        let base = root("case-collision");
        let (upper, lower) = (base.join("x/Rock"), base.join("y/rock"));
        let common = base.join("card/Assets/tau/common");
        for dir in [&upper, &lower, &common] {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(upper.join("01.mp3"), b"first").unwrap();
        fs::write(lower.join("01.mp3"), b"second").unwrap();
        let error = plan(
            &[upper, lower],
            &common,
            "/Assets/tau/common/",
            PlanOptions { mirror: false, embed_covers: false, art_sidecar_pal256: false },
            &mut None,
        )
        .unwrap_err();
        assert_eq!(error.code(), ErrorCode::NameCollision);
    }

    /// The read-back must go through the host's cache evictor before it reads.
    #[test]
    fn read_backs_evict_the_cache_first() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static CALLS: AtomicU64 = AtomicU64::new(0);
        fn counting(_: &Path) -> bool {
            CALLS.fetch_add(1, Ordering::SeqCst);
            true
        }
        register_cache_evictor(counting);
        let dir = std::env::temp_dir().join(format!(
            "tau-evict-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() + crate::test_uniq()
        ));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("w.bin");
        fs::write(&file, b"payload").unwrap();
        let before = CALLS.load(Ordering::SeqCst);
        verify_written(&file, &sha256_bytes(b"payload"), Path::new("s")).unwrap();
        assert_eq!(read_back_bytes(&file).unwrap(), b"payload");
        // Other tests in this process may also evict, so only a lower bound is exact.
        assert!(CALLS.load(Ordering::SeqCst) >= before + 2);
        assert!(readback_report().0, "an evictor is registered");
        fs::remove_dir_all(dir).unwrap();
    }

    /// The check every write ends with must actually fail on wrong bytes (the
    /// cover-embedding path used to end in a test that could never fail).
    #[test]
    fn verify_written_rejects_wrong_bytes_and_removes_the_temp_file() {
        let dir = std::env::temp_dir().join(format!(
            "tau-verify-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() + crate::test_uniq()
        ));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("x.tmp");
        fs::write(&file, b"what actually reached the card").unwrap();
        let intended = sha256_bytes(b"what we meant to write");
        let error = verify_written(&file, &intended, Path::new("src.mp3")).unwrap_err();
        assert_eq!(error.code(), ErrorCode::VerificationFailed);
        assert!(!file.exists(), "a failed verification must not leave the temp file");
        fs::write(&file, b"exact").unwrap();
        verify_written(&file, &sha256_bytes(b"exact"), Path::new("src.mp3")).unwrap();
        assert!(file.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    /// P1-2: `sources: &[PathBuf]` is a public parameter to `plan`/
    /// `plan_with_features`; a path with no derivable file name (`/`, `.`)
    /// used to reach a bare `.file_name().unwrap()` here and panic the whole
    /// plan instead of failing it cleanly.
    #[test]
    fn required_file_name_does_not_panic_on_a_nameless_path() {
        assert!(required_file_name(Path::new("/")).is_err());
        assert!(required_file_name(Path::new(".")).is_err());
        assert_eq!(
            required_file_name(Path::new("/tmp/foo.mp3")).unwrap(),
            "foo.mp3"
        );
    }

    fn root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tau-sync-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() + crate::test_uniq()
        ))
    }
    /// P2-1: the token used to be a 32-bit-truncated hash formatted `T2-xxxxxxxx`,
    /// thin for something that may be persisted or handed across a process
    /// boundary, and the `T2-` leaked an internal roadmap phase label into a
    /// durable identifier. It's now the full SHA-256 hex digest, unprefixed.
    #[test]
    fn plan_id_is_a_full_sha256_hex_digest_with_no_phase_prefix() {
        let source = root("token-source");
        let common = root("token-card").join("Assets/tau/common");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&common).unwrap();
        fs::write(source.join("01.mp3"), b"music").unwrap();
        let sync_plan = plan(
            &[source.clone()],
            &common,
            "/Assets/tau/common/",
            PlanOptions::default(),
            &mut None,
        )
        .unwrap();
        assert_eq!(sync_plan.id.len(), 64);
        assert!(sync_plan.id.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!sync_plan.id.starts_with("T2-"));
        fs::remove_dir_all(source).unwrap();
        fs::remove_dir_all(common.ancestors().nth(2).unwrap()).unwrap();
    }

    #[test]
    fn sync_is_plan_first_and_leaves_sources_untouched() {
        let source = root("source");
        let common = root("card").join("Assets/tau/common");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&common).unwrap();
        fs::write(source.join("01 Nausicaä.mp3"), b"music").unwrap();
        let original = sha256_file(&source.join("01 Nausicaä.mp3")).unwrap();
        let sync_plan = plan(
            &[source.clone()],
            &common,
            "/Assets/tau/common/",
            PlanOptions::default(),
            &mut None,
        )
        .unwrap();
        assert!(common.read_dir().unwrap().next().is_none());
        assert!(execute(&sync_plan, "wrong", &mut None).is_err());
        let report = execute(&sync_plan, &sync_plan.id, &mut None).unwrap();
        assert_eq!(report.copied, 1);
        assert_eq!(
            sha256_file(&source.join("01 Nausicaä.mp3")).unwrap(),
            original
        );
        assert!(common.join("tau-library.tdb").is_file());
        assert!(
            common
                .join(source.file_name().unwrap())
                .join("01 Nausicaa.mp3")
                .is_file()
        );
        let retry = plan(
            &[source.clone()],
            &common,
            "/Assets/tau/common/",
            PlanOptions::default(),
            &mut None,
        )
        .unwrap();
        assert_eq!(retry.items[0].state, CopyState::Same);
        let no_op = execute(&retry, &retry.id, &mut None).unwrap();
        assert_eq!(no_op.copied, 0);
        fs::remove_dir_all(source).unwrap();
        fs::remove_dir_all(common.ancestors().nth(2).unwrap()).unwrap();
    }

    #[test]
    fn failed_copy_keeps_the_previous_index() {
        let source = root("changed-source");
        let common = root("previous-index").join("Assets/tau/common");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&common).unwrap();
        let media = source.join("01 Track.mp3");
        fs::write(&media, b"before").unwrap();
        let plan = plan(
            &[source.clone()],
            &common,
            "/Assets/tau/common/",
            PlanOptions::default(),
            &mut None,
        )
        .unwrap();
        let index = common.join("tau-library.tdb");
        fs::write(&index, b"previous index bytes").unwrap();
        fs::write(&media, b"after!").unwrap();
        assert!(execute(&plan, &plan.id, &mut None).is_err());
        assert_eq!(fs::read(index).unwrap(), b"previous index bytes");
        fs::remove_dir_all(source).unwrap();
        fs::remove_dir_all(common.ancestors().nth(2).unwrap()).unwrap();
    }

    #[test]
    fn mirror_requires_second_confirmation_and_verified_backup() {
        let source = root("mirror-source");
        let card = root("mirror-card");
        let common = card.join("Assets/tau/common");
        let backup = root("mirror-backup");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&common).unwrap();
        fs::write(source.join("01 Keep.mp3"), b"keep").unwrap();
        fs::write(common.join("old.mp3"), b"old").unwrap();
        let plan = plan(
            &[source.clone()],
            &common,
            "/Assets/tau/common/",
            PlanOptions {
                mirror: true,
                embed_covers: false,
                art_sidecar_pal256: false,
            },
            &mut None,
        )
        .unwrap();
        assert_eq!(plan.deletions.len(), 1);
        assert!(execute_with_mirror(&plan, &plan.id, None, Some(&backup), &mut None).is_err());
        assert!(common.join("old.mp3").exists());
        let report =
            execute_with_mirror(&plan, &plan.id, Some(&plan.id), Some(&backup), &mut None).unwrap();
        assert_eq!(report.deleted, 1);
        assert!(!common.join("old.mp3").exists());
        assert_eq!(
            fs::read(backup.join(&plan.id).join("old.mp3")).unwrap(),
            b"old"
        );
        fs::remove_dir_all(source).unwrap();
        fs::remove_dir_all(card).unwrap();
        fs::remove_dir_all(backup).unwrap();
    }

    #[test]
    fn reviewed_cover_is_embedded_only_in_the_destination_copy() {
        let source = root("cover-source");
        let card = root("cover-card");
        let common = card.join("Assets/tau/common");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&common).unwrap();
        let track = source.join("01 Track.mp3");
        fs::write(&track, b"audio").unwrap();
        fs::write(source.join("cover.jpg"), [0xff, 0xd8, 0xff, 0xd9]).unwrap();
        let sync_plan = plan(
            &[source.clone()],
            &common,
            "/Assets/tau/common/",
            PlanOptions {
                mirror: false,
                embed_covers: true,
                art_sidecar_pal256: false,
            },
            &mut None,
        )
        .unwrap();
        assert!(sync_plan.items[0].cover.is_some());
        execute(&sync_plan, &sync_plan.id, &mut None).unwrap();
        assert_eq!(fs::read(&track).unwrap(), b"audio");
        let copy = fs::read(
            common
                .join(source.file_name().unwrap())
                .join("01 Track.mp3"),
        )
        .unwrap();
        assert!(copy.windows(4).any(|window| window == b"APIC"));
        fs::remove_dir_all(source).unwrap();
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn art_sidecar_is_planned_once_per_album_and_written_verifiably() {
        let source = root("art-sidecar-source");
        let card = root("art-sidecar-card");
        let common = card.join("Assets/tau/common");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&common).unwrap();
        fs::write(source.join("01 Track.mp3"), b"audio one").unwrap();
        fs::write(source.join("02 Track.mp3"), b"audio two").unwrap();
        let mut real_cover = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        real_cover.push("../../testdata/images/cover455.jpg");
        fs::copy(&real_cover, source.join("cover.jpg")).unwrap();

        let sync_plan = plan(
            &[source.clone()],
            &common,
            "/Assets/tau/common/",
            PlanOptions {
                mirror: false,
                embed_covers: false,
                art_sidecar_pal256: true,
            },
            &mut None,
        )
        .unwrap();
        // One album, two tracks -> exactly one sidecar, not one per track.
        assert_eq!(sync_plan.art_sidecars.len(), 1);
        let report = execute(&sync_plan, &sync_plan.id, &mut None).unwrap();
        assert_eq!(report.art_sidecars_written, 1);

        let sidecar_path = common
            .join(source.file_name().unwrap())
            .join(image::pal256_sidecar_name(128));
        let decoded = image::decode_tim1(&fs::read(&sidecar_path).unwrap()).unwrap();
        assert_eq!((decoded.width, decoded.height), (128, 128));

        // Re-planning and re-executing with nothing changed writes it again
        // (idempotent, not "only once ever") but the plan token is identical.
        let second_plan = plan(
            &[source.clone()],
            &common,
            "/Assets/tau/common/",
            PlanOptions {
                mirror: false,
                embed_covers: false,
                art_sidecar_pal256: true,
            },
            &mut None,
        )
        .unwrap();
        assert_eq!(second_plan.id, sync_plan.id);

        fs::remove_dir_all(source).unwrap();
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn core_copy_preserves_paths_and_rebuilds_only_the_destination() {
        let card = root("core-copy");
        let source = card.join("Assets/tau/common");
        let destination = card.join("Assets/tau-test/common");
        fs::create_dir_all(source.join("album")).unwrap();
        fs::create_dir_all(&destination).unwrap();
        let track = source.join("album/01 Track.mp3");
        fs::write(&track, b"music").unwrap();
        let plan =
            plan_core_copy(&source, &destination, "/Assets/tau-test/common/", &mut None).unwrap();
        assert_eq!(
            plan.items[0].destination,
            plan.destination.join("album/01 Track.mp3")
        );
        execute(&plan, &plan.id, &mut None).unwrap();
        assert_eq!(fs::read(&track).unwrap(), b"music");
        assert_eq!(
            fs::read(destination.join("album/01 Track.mp3")).unwrap(),
            b"music"
        );
        assert!(destination.join("tau-library.tdb").is_file());
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn core_move_requires_second_token_and_keeps_a_verified_backup() {
        let card = root("core-move");
        let backup = root("core-move-backup");
        let source = card.join("Assets/tau/common");
        let destination = card.join("Assets/tau-test/common");
        fs::create_dir_all(source.join("album")).unwrap();
        fs::create_dir_all(&destination).unwrap();
        let track = source.join("album/01 Track.mp3");
        fs::write(&track, b"music").unwrap();
        let plan =
            plan_core_copy(&source, &destination, "/Assets/tau-test/common/", &mut None).unwrap();
        assert!(
            execute_core_move(
                &plan,
                &plan.id,
                "wrong",
                &source,
                "/Assets/tau/common/",
                &backup,
                &mut None,
            )
            .is_err()
        );
        assert!(track.exists());
        let report = execute_core_move(
            &plan,
            &plan.id,
            &plan.id,
            &source,
            "/Assets/tau/common/",
            &backup,
            &mut None,
        )
        .unwrap();
        assert_eq!(report.deleted, 1);
        assert!(!track.exists());
        assert_eq!(
            fs::read(backup.join(&plan.id).join("album/01 Track.mp3")).unwrap(),
            b"music"
        );
        assert!(source.join("tau-library.tdb").is_file());
        assert!(destination.join("tau-library.tdb").is_file());
        fs::remove_dir_all(card).unwrap();
        fs::remove_dir_all(backup).unwrap();
    }

    #[test]
    fn cancelling_partway_through_a_plan_stops_hashing() {
        let source = root("cancel-source");
        let common = root("cancel-card").join("Assets/tau/common");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&common).unwrap();
        fs::write(source.join("01.mp3"), b"one").unwrap();
        fs::write(source.join("02.mp3"), b"two").unwrap();
        let mut seen = 0;
        let mut observer = |_progress: Progress| {
            seen += 1;
            false
        };
        let mut observer: Option<&mut dyn ProgressObserver> = Some(&mut observer);
        let error = plan(
            &[source.clone()],
            &common,
            "/Assets/tau/common/",
            PlanOptions::default(),
            &mut observer,
        )
        .unwrap_err();
        assert_eq!(error.code(), ErrorCode::Cancelled);
        assert_eq!(seen, 1);
        fs::remove_dir_all(source).unwrap();
        fs::remove_dir_all(common.ancestors().nth(2).unwrap()).unwrap();
    }
}
