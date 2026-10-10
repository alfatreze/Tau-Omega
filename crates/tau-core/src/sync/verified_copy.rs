//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

/// An AppleDouble stub is a few KB; anything bigger named `._*` is somebody's file and is left alone.
pub(super) const APPLEDOUBLE_MAX: u64 = 64 * 1024;

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
pub(super) const ART_SIDECAR_MAX_BYTES: u64 = 24 * 1024;

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
pub(super) const INDEX_MAX_BYTES: u64 = 4 * 1024 * 1024;

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
pub(super) fn preflight_space(plan: &SyncPlan) -> Result<(), TauError> {
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
pub(super) fn verify_written(
    written: &Path,
    expected: &str,
    source: &Path,
) -> Result<(), TauError> {
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
pub(super) struct Proven {
    pub(super) index: usize,
    pub(super) rel: String,
    pub(super) output_sha: String,
    pub(super) verified_ms: i64,
}

/// How much card data a plan re-reads, at most, to check the ledger's claims (oldest-verified first).
pub(super) const CANARY_BUDGET_BYTES: i64 = 64 * 1024 * 1024;

/// The ledger may only ever cause a *missed update*, never a loss, and this bounds how long a missed
/// update can last: each plan re-hashes the least recently verified remembered files, up to a byte budget.
/// If one is not what the ledger said, that file and every remembered file not yet re-checked is copied
/// again (distrust everything unchecked), and the ledger forgets them.
pub(super) fn run_canary(
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

pub(super) fn copy_verified(item: &CopyItem) -> Result<String, TauError> {
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
