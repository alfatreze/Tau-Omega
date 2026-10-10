//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

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

pub(super) fn execute_with_mirror_inner(
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

pub(super) fn execute_core_move_inner(
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
