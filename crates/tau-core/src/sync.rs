//! T2's intentionally narrow write boundary: make a pure plan first, then
//! execute that exact plan after an explicit token confirmation.

use crate::{
    ErrorCode, Progress, ProgressObserver, Stage, TauError, Warning, WarningCode, ascii_name,
    build_index, cover, image, ledger, parse, scan_dir_with_progress, tick, verify,
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
    let mut warned_covers = std::collections::BTreeSet::new();
    let mut ledger = ledger::Session::open(&destination);
    let mut proven: Vec<Proven> = Vec::new();
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
                format!(
                    "name collision (the card ignores letter case): {}",
                    relative.display()
                ),
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
                Some(path) => match cover::validate_jpeg_cover(&path) {
                    Ok(()) => Some(CoverItem {
                        sha256: sha256_file(&path)?,
                        source: path,
                    }),
                    // A cover that cannot be embedded must not stop the whole sync: copy the
                    // songs without it, and say so once per album.
                    Err(error) if error.code() == ErrorCode::UnsupportedCover => {
                        if warned_covers.insert(path.clone()) {
                            warnings.push(Warning::new(
                                WarningCode::CoverNotEmbedded,
                                format!(
                                    "{}: the cover was not put inside the songs ({})",
                                    path.parent().and_then(|p| p.file_name()).map_or_else(
                                        || path.display().to_string(),
                                        |n| n.to_string_lossy().into_owned()
                                    ),
                                    error.message
                                ),
                            ));
                        }
                        None
                    }
                    Err(error) => return Err(error),
                },
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
        // The ledger remembers files this tool wrote and verified itself: made from this exact source and
        // cover by this embed version, and still exactly as it left them (size and time). That makes
        // "already on the card" a statement we can prove without reading the card file, including for
        // cover-embedded copies, which can never equal their source and so used to be rewritten on every
        // re-sync. A small canary below re-hashes a few of these to catch anything that drifted.
        let rel_key = relative.to_string_lossy().replace('\\', "/");
        let cover_sha = cover.as_ref().map(|c| c.sha256.clone()).unwrap_or_default();
        let remembered = match (
            ledger.as_ref(),
            fs::metadata(&target)
                .ok()
                .and_then(|m| ledger::fingerprint(&m)),
        ) {
            (Some(session), Some(print)) => session
                .provenance(&rel_key, &print)
                .filter(|(p, _)| p.source_sha == sha256 && p.cover_sha == cover_sha),
            _ => None,
        };
        // Same when the ledger remembers this very copy, else when the file on the card hashes equal.
        let same = remembered.is_some()
            || (target.is_file()
                && fs::metadata(&target)?.len() == bytes
                && sha256_file(&target).ok().as_deref() == Some(&sha256));
        let mut state = if same {
            CopyState::Same
        } else if target.exists() {
            CopyState::Update
        } else {
            CopyState::New
        };
        // Artwork changes the resulting bytes, so an existing raw source copy
        // must be explicitly updated when a cover is requested (unless the ledger proves this very
        // copy was made from these inputs).
        if cover.is_some() && state == CopyState::Same && remembered.is_none() {
            state = CopyState::Update;
        }
        if let Some((prov, verified_ms)) = remembered {
            proven.push(Proven {
                index: items.len(),
                rel: rel_key,
                output_sha: prov.output_sha,
                verified_ms,
            });
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
    run_canary(
        &mut items,
        &mut proven,
        &mut ledger,
        &mut bytes_to_write,
        &mut warnings,
    );
    if let Some(session) = ledger.take() {
        session.finish();
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
    let result = execute_with_mirror_inner(
        plan,
        confirmation,
        delete_confirmation,
        backup_root,
        progress,
    );
    // Whether the run finished or stopped part way, do not leave macOS's metadata stubs behind (a refused
    // confirmation wrote nothing, so skip the walk then).
    if !matches!(&result, Err(e) if e.code() == ErrorCode::ConfirmationMismatch) {
        sweep_appledouble(&plan.destination);
    }
    result
}

fn execute_with_mirror_inner(
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
    sweep_stale_temps(
        &plan.destination,
        plan.items.iter().map(|i| i.destination.as_path()),
    );
    let mut copied = 0;
    let mut unchanged = 0;
    let mut bytes_written = 0;
    let mut remembered = ledger::Session::open(&plan.destination);
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
                let output_sha = copy_verified(item)?;
                // Remember what was just written and verified (the read-back bypassed the cache), keyed by the
                // file as it stands after the final rename.
                if let Some(session) = remembered.as_mut()
                    && let Ok(meta) = fs::metadata(&item.destination)
                    && let Some(print) = ledger::fingerprint(&meta)
                    && let Ok(rel) = item.destination.strip_prefix(&plan.destination)
                {
                    session.record_verified(
                        &rel.to_string_lossy().replace('\\', "/"),
                        print,
                        ledger::Provenance {
                            source_sha: item.sha256.clone(),
                            cover_sha: item
                                .cover
                                .as_ref()
                                .map(|c| c.sha256.clone())
                                .unwrap_or_default(),
                            embed_version: ledger::EMBED_VERSION,
                            output_sha,
                        },
                    );
                }
                copied += 1;
                bytes_written += item.bytes;
                copy_done += item.bytes;
            }
        }
    }
    if let Some(session) = remembered.take() {
        session.finish();
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
                format!(
                    "cover file verification failed: {}",
                    item.destination.display()
                ),
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
    let result = execute_core_move_inner(
        plan,
        confirmation,
        delete_confirmation,
        source_common,
        source_root_prefix,
        backup_root,
        progress,
    );
    if !matches!(&result, Err(e) if e.code() == ErrorCode::ConfirmationMismatch) {
        // Files were deleted from the source core as well as written to the destination.
        sweep_appledouble(source_common);
    }
    result
}

fn execute_core_move_inner(
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
    if backup_is_inside(backup_root, &source_common)
        || backup_is_inside(backup_root, &plan.destination)
    {
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
                format!(
                    "changed since the plan was reviewed: {}",
                    item.relative.display()
                ),
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
    swap_in_file(temp, live, PREVIOUS_INDEX)
}

/// The same recoverable swap for any single file: the old one is kept as `previous_name`
/// (beside it) until the new one is in place.
pub(crate) fn swap_in_file(temp: &Path, live: &Path, previous_name: &str) -> Result<(), TauError> {
    let previous = live.with_file_name(previous_name);
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
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
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

/// An AppleDouble stub is a few KB; anything bigger named `._*` is somebody's file and is left alone.
const APPLEDOUBLE_MAX: u64 = 64 * 1024;

/// Removes the macOS AppleDouble stubs (`._<name>`) that the OS leaves beside files it writes, renames or deletes on
/// exFAT/FAT (SAFETY_RULES 7), from a Tau media root. Only regular files named `._*` and no larger than a stub are
/// removed; symlinks, directories and everything else are untouched. Errors are ignored: this is housekeeping and
/// must never fail a run that has otherwise succeeded. Returns how many were removed.
pub(crate) fn sweep_appledouble(media_root: &Path) -> usize {
    let mut removed = 0;
    let mut stack = vec![media_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file()
                && entry.file_name().to_string_lossy().starts_with("._")
                && entry.metadata().is_ok_and(|m| m.len() <= APPLEDOUBLE_MAX)
                && fs::remove_file(entry.path()).is_ok()
            {
                removed += 1;
            }
        }
    }
    removed
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
    for item in plan.items.iter().filter(|i| i.state != CopyState::Same) {
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

/// One file the ledger says is already on the card exactly as this tool wrote it.
struct Proven {
    index: usize,
    rel: String,
    output_sha: String,
    verified_ms: i64,
}

/// How much card data a plan re-reads, at most, to check the ledger's claims (oldest-verified first).
const CANARY_BUDGET_BYTES: i64 = 64 * 1024 * 1024;

/// The ledger may only ever cause a *missed update*, never a loss, and this bounds how long a missed
/// update can last: each plan re-hashes the least recently verified remembered files, up to a byte budget.
/// If one is not what the ledger said, that file and every remembered file not yet re-checked is copied
/// again (distrust everything unchecked), and the ledger forgets them.
fn run_canary(
    items: &mut [CopyItem],
    proven: &mut [Proven],
    ledger: &mut Option<ledger::Session>,
    bytes_to_write: &mut u64,
    warnings: &mut Vec<Warning>,
) {
    let Some(session) = ledger.as_mut() else {
        return;
    };
    proven.sort_by_key(|p| p.verified_ms);
    let mut budget = CANARY_BUDGET_BYTES;
    let mut drift = false;
    for p in proven.iter() {
        let item = &mut items[p.index];
        if !drift {
            if budget <= 0 {
                break; // checked enough for this plan; the rest stay trusted
            }
            budget -= i64::try_from(fs::metadata(&item.destination).map_or(0, |m| m.len()))
                .unwrap_or(i64::MAX);
            evict_cache(&item.destination);
            if sha256_file(&item.destination).ok().as_deref() == Some(p.output_sha.as_str()) {
                session.touch_verified(&p.rel);
                continue;
            }
            drift = true;
            warnings.push(Warning::new(
                WarningCode::CardFileChanged,
                format!("{}: this file on the card is not what Tau Omega wrote there, so it and any earlier files that could not be re-checked will be copied again", p.rel),
            ));
        }
        item.state = if item.destination.exists() {
            CopyState::Update
        } else {
            CopyState::New
        };
        *bytes_to_write += item.bytes;
        session.invalidate(&p.rel);
    }
}

fn copy_verified(item: &CopyItem) -> Result<String, TauError> {
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
    let output_sha;
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
        output_sha = expected;
    } else {
        {
            let mut source = fs::File::open(&item.source)?;
            let mut target = fs::File::create(&temp)?;
            io::copy(&mut source, &mut target)?;
            target.sync_all()?;
        }
        verify_written(&temp, &item.sha256, &item.source)?;
        output_sha = item.sha256.clone();
    }
    fs::rename(temp, &item.destination)?;
    Ok(output_sha)
}

#[cfg(test)]
mod tests;
