//! Split out of the parent module unchanged (see the parent's docs).

use super::*;
use crate::convert::Action;

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
            collect_source(
                source,
                source,
                include_source_root,
                options.convert.enabled,
                &mut candidates,
            )?;
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
        convert: settings,
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
    let mut folder_tracks: std::collections::BTreeMap<PathBuf, Vec<PathBuf>> = Default::default();
    for (source, _) in &candidates {
        if audio_file(source)
            && let Some(folder) = source.parent()
        {
            folder_tracks
                .entry(folder.to_path_buf())
                .or_default()
                .push(source.clone());
        }
    }
    let mut seen_art_folders = std::collections::BTreeSet::new();
    let mut warned_covers = std::collections::BTreeSet::new();
    let mut ledger = ledger::Session::open(&destination);
    let mut proven: Vec<Proven> = Vec::new();
    let mut items = Vec::new();
    let mut art_sidecars = Vec::new();
    let mut warnings = Vec::new();
    let mut skipped: Vec<SkippedItem> = Vec::new();
    // Conversion and ReplayGain decisions for every file (a no-op while both are off).
    let prepared = prepare(&candidates, &settings, &mut warnings, progress)?;
    let lame = settings.enabled.then(|| settings.find_lame()).flatten();
    let mut bytes_to_write = 0;
    let total = candidates.len() as u64;
    for (done, (source, relative)) in candidates.into_iter().enumerate() {
        let decision = &prepared[done];
        tick(
            progress,
            Progress {
                stage: Stage::Hashing,
                done: done as u64,
                total,
                path: Some(relative.to_string_lossy().into_owned()),
            },
        )?;
        if let Action::Skip(reason) = &decision.action {
            skipped.push(SkippedItem {
                source: source.clone(),
                reason: reason.clone(),
            });
            continue;
        }
        let converted = matches!(decision.action, Action::Convert(_));
        let relative = match &decision.action {
            Action::Convert(kind) => convert::converted_name(&relative, kind),
            _ => relative,
        };
        // FAT/exFAT ignore case, so `Song.mp3` and `song.mp3` are the same file on
        // the card: the second copy would silently replace the first.
        if !seen.insert(relative.to_string_lossy().to_lowercase()) {
            if converted {
                skipped.push(SkippedItem {
                    source: source.clone(),
                    reason: "a file with the same name (after conversion) is already in this copy"
                        .into(),
                });
                continue;
            }
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
        let source_bytes = fs::metadata(&source)?.len();
        if audio_file(&source)
            && matches!(decision.action, Action::Copy)
            && let Ok(details) = crate::details::extract(&source)
            && let Some(reason) = crate::details::unplayable_reason(&details)
        {
            warnings.push(Warning::new(
                WarningCode::WillNotPlay,
                format!(
                    "{}: copied, but the Pocket will not play it ({reason})",
                    relative.display()
                ),
            ));
        }
        let sha256 = match &decision.sha256 {
            Some(sha) => sha.clone(),
            None => sha256_file(&source)?,
        };
        let transform = match (&decision.action, decision.replaygain.is_empty()) {
            (Action::Convert(kind), _) => {
                let info = decision.info.as_ref();
                Some(Transform {
                    kind: Some(kind.clone()),
                    reason: decision.reason.clone(),
                    replaygain: decision.replaygain.clone(),
                    frames: info.and_then(|i| {
                        i.frames
                            .map(|f| (f * kind.rate() as u64).div_ceil(i.rate.max(1) as u64))
                    }),
                    source_rate: info.map_or(0, |i| i.rate),
                })
            }
            (_, false) => Some(Transform {
                kind: None,
                reason: "ReplayGain tags added".into(),
                replaygain: decision.replaygain.clone(),
                frames: None,
                source_rate: 0,
            }),
            _ => None,
        };
        let bytes = match (&transform, &decision.info) {
            (
                Some(Transform {
                    kind: Some(kind), ..
                }),
                Some(info),
            ) => convert::estimate_bytes(info, source_bytes, kind),
            (Some(_), _) => source_bytes + 4096,
            _ => source_bytes,
        };
        let cover = if embed_covers && audio_file(&source) && !converted {
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
            && (audio_file(&source) || converted)
            && let Some(source_folder) = source.parent()
            && seen_art_folders.insert(source_folder.to_path_buf())
            && let Some((cover_source, cover_sha256, from_embedded)) = art_source(
                source_folder,
                folder_tracks.get(source_folder).map_or(&[], Vec::as_slice),
            )?
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
                cover_sha256,
                cover_source,
                from_embedded,
            });
        }
        // The ledger remembers files this tool wrote and verified itself: made from this exact source and
        // cover by this embed version, and still exactly as it left them (size and time). That makes
        // "already on the card" a statement we can prove without reading the card file, including for
        // cover-embedded copies, which can never equal their source and so used to be rewritten on every
        // re-sync. A small canary below re-hashes a few of these to catch anything that drifted.
        let rel_key = relative.to_string_lossy().replace('\\', "/");
        let cover_sha = variant_key(cover.as_ref(), transform.as_ref());
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
            || (transform.is_none()
                && target.is_file()
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
            transform,
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
        if let Some(transform) = &item.transform {
            hasher.update(transform.spec().as_bytes());
        }
    }
    for item in &skipped {
        hasher.update(item.source.to_string_lossy().as_bytes());
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
        skipped,
        lame,
    })
}

/// What the ledger compares to know a card file was made from these exact inputs: the embedded cover and
/// the conversion or tags applied.
pub(super) fn variant_key(cover: Option<&CoverItem>, transform: Option<&Transform>) -> String {
    let cover = cover.map(|c| c.sha256.clone()).unwrap_or_default();
    match transform {
        Some(t) => format!("{cover}|{}", t.spec()),
        None => cover,
    }
}

/// Where a folder's cover sidecar comes from: a folder image first (`cover.jpg`, ...), else the picture
/// embedded in the first track of the folder that has one (MP3 APIC, FLAC PICTURE). Returns the source
/// path, its hash and whether it is embedded art.
pub(crate) fn art_source(
    folder: &Path,
    tracks: &[PathBuf],
) -> Result<Option<(PathBuf, String, bool)>, TauError> {
    if let Some(file) = cover::find_cover(folder) {
        let hash = sha256_file(&file)?;
        return Ok(Some((file, hash, false)));
    }
    for track in tracks {
        if let Some(bytes) = cover::extract_embedded_cover(track) {
            return Ok(Some((track.clone(), sha256_bytes(&bytes), true)));
        }
    }
    if let Some(file) = cover::find_any_image(folder) {
        let hash = sha256_file(&file)?;
        return Ok(Some((file, hash, false)));
    }
    Ok(None)
}
