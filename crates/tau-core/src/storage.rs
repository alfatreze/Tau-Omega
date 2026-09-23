//! Read-only storage capacity checks: whether a plan's `bytes_to_write`
//! would actually fit on its destination volume. Nothing here writes to
//! disk or reserves space -- a caller re-checks at execute time the same
//! way every other plan in this crate is only as fresh as its own inputs.

use crate::{ErrorCode, TauError};
use std::path::{Path, PathBuf};

/// A safety cushion reserved for filesystem overhead (journal blocks,
/// directory entries, wear-levelling on SD cards) and any concurrent
/// writes, on top of the bytes a plan says it needs. Deliberately generous:
/// SD cards commonly reserve their own spare area, but this crate has no way
/// to know a specific card's actual overhead, so it stays a fixed, named
/// constant rather than a guess embedded in call sites.
pub const DEFAULT_MARGIN_BYTES: u64 = 16 * 1024 * 1024;

/// Free/total space on the volume containing a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VolumeSpace {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

/// Reads the free/total space of the volume containing `path`. `path` need
/// not exist yet (a plan's destination is often a file or folder that
/// hasn't been created) -- the nearest existing ancestor directory is
/// queried instead, which is always on the same volume.
pub fn volume_space(path: &Path) -> Result<VolumeSpace, TauError> {
    let existing = nearest_existing_ancestor(path)?;
    Ok(VolumeSpace {
        total_bytes: fs4::total_space(&existing)?,
        available_bytes: fs4::available_space(&existing)?,
    })
}

/// Whether `bytes_needed` fits in the available space at `path`'s volume,
/// after reserving `margin_bytes` on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapacityCheck {
    pub space: VolumeSpace,
    pub bytes_needed: u64,
    pub margin_bytes: u64,
    pub fits: bool,
}

/// Checks whether a plan's `bytes_needed` fits at `path`'s volume. Read-only:
/// this reports what the volume looks like right now, it reserves nothing,
/// so a caller should still handle a full-disk failure at execute time --
/// the same "plan is only as fresh as when it was made" rule every other
/// plan in this crate already lives under.
pub fn check_capacity(
    path: &Path,
    bytes_needed: u64,
    margin_bytes: u64,
) -> Result<CapacityCheck, TauError> {
    let space = volume_space(path)?;
    let fits = space.available_bytes >= bytes_needed.saturating_add(margin_bytes);
    Ok(CapacityCheck {
        space,
        bytes_needed,
        margin_bytes,
        fits,
    })
}

fn nearest_existing_ancestor(path: &Path) -> Result<PathBuf, TauError> {
    let mut candidate = path.to_path_buf();
    loop {
        if candidate.as_os_str().is_empty() {
            candidate = std::env::current_dir()?;
        }
        if candidate.is_dir() {
            return Ok(candidate);
        }
        match candidate.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => candidate = parent.to_path_buf(),
            _ => {
                return Err(TauError::e(
                    ErrorCode::InvalidPathReference,
                    "no existing directory found on this path to check capacity against",
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn scratch_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tau-storage-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn reports_real_volume_totals_for_an_existing_directory() {
        let root = scratch_root("existing");
        let space = volume_space(&root).unwrap();
        assert!(space.total_bytes > 0);
        assert!(space.total_bytes >= space.available_bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn walks_up_to_an_existing_ancestor_for_a_not_yet_created_destination() {
        let root = scratch_root("ancestor");
        let not_yet_created = root.join("new-folder/report.json");
        let space = volume_space(&not_yet_created).unwrap();
        assert!(space.total_bytes > 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_huge_request_never_fits() {
        let root = scratch_root("huge");
        let check = check_capacity(&root, u64::MAX / 2, DEFAULT_MARGIN_BYTES).unwrap();
        assert!(!check.fits);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_tiny_request_fits_with_margin() {
        let root = scratch_root("tiny");
        let check = check_capacity(&root, 1, DEFAULT_MARGIN_BYTES).unwrap();
        assert!(check.fits);
        assert_eq!(check.bytes_needed, 1);
        assert_eq!(check.margin_bytes, DEFAULT_MARGIN_BYTES);
        fs::remove_dir_all(root).unwrap();
    }
}
