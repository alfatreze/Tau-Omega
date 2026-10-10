//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

/// Host hook that drops the operating system's cached copy of a file so the next
/// read comes from the device. Without it a read-back right after a write is
/// served from memory: measured on macOS against a disk image whose content was
/// changed underneath (a plain read returned the stale bytes; `F_NOCACHE` on the
/// reading side did not help; invalidating the file's pages did). `tau-core`
/// forbids `unsafe`, so the platform call lives in the host and is registered here.
pub type CacheEvictor = fn(&Path) -> bool;
pub(super) static CACHE_EVICTOR: std::sync::OnceLock<CacheEvictor> = std::sync::OnceLock::new();
pub(super) static EVICTIONS_FAILED: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

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
pub(super) const LIVE_INDEX: &str = "tau-library.tdb";
pub(super) const PREVIOUS_INDEX: &str = ".tau-library.tdb.prev";
/// A temp file this old cannot belong to a run that is still going.
pub(super) const STALE_TEMP_AGE: std::time::Duration = std::time::Duration::from_secs(60 * 60);

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
