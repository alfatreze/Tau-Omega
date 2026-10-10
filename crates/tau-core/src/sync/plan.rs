//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

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

pub(super) fn plan_with_layout(
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
