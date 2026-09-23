//! Read-only backup planning for an arbitrary folder -- unlike
//! [`crate::compare::media_roots`], `source`/`destination` are not required
//! to be `Assets/<platform>/common` media roots, since a backup target is
//! commonly just a plain folder on an external drive. Nothing here writes to
//! disk; this is a dry-run preview only (`STATUS_HANDOFF.md`'s "backup/
//! package dry-run views" -- an execute path is deliberately not built yet,
//! since a backup destination is exactly the kind of write this app treats
//! with the most caution).

use crate::{
    compare::{self, DifferenceState},
    ErrorCode, TauError,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BackupItem {
    pub relative: PathBuf,
    pub state: DifferenceState,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BackupPlan {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub items: Vec<BackupItem>,
    pub new_files: usize,
    pub updated_files: usize,
    pub unchanged_files: usize,
    /// Files present at the destination but not in the source. Reported for
    /// awareness only -- a backup never deletes anything; that stays
    /// `sync`'s separately-confirmed `mirror` option.
    pub destination_only_files: usize,
    pub bytes_to_write: u64,
}

/// Plans backing up `source` onto `destination`: every supported file under
/// `source` that is new or different at `destination` would be copied.
/// `destination` need not exist yet -- a first-ever backup commonly starts
/// from nothing, which is treated as "nothing to compare against" rather
/// than an error.
pub fn plan(source: &Path, destination: &Path) -> Result<BackupPlan, TauError> {
    if !source.is_dir() {
        return Err(TauError::e(
            ErrorCode::InvalidMediaRoot,
            "backup source must be an existing folder",
        ));
    }
    let source = source
        .canonicalize()
        .map_err(|error| TauError::e(ErrorCode::Io, error.to_string()))?;
    let source_files = compare::files_by_relative_path(&source)?;
    let destination_files = if destination.is_dir() {
        compare::files_by_relative_path(destination)?
    } else {
        BTreeMap::new()
    };

    let mut relatives: Vec<_> = source_files
        .keys()
        .chain(destination_files.keys())
        .cloned()
        .collect();
    relatives.sort();
    relatives.dedup();

    let mut items = Vec::with_capacity(relatives.len());
    let (mut new_files, mut updated_files, mut unchanged_files, mut destination_only_files) =
        (0, 0, 0, 0);
    let mut bytes_to_write = 0u64;
    for relative in relatives {
        let source_file = source_files.get(&relative);
        let destination_file = destination_files.get(&relative);
        let (state, bytes) = match (source_file, destination_file) {
            (Some(source_file), None) => {
                new_files += 1;
                bytes_to_write += source_file.bytes;
                (DifferenceState::OnlyLeft, source_file.bytes)
            }
            (Some(source_file), Some(destination_file))
                if source_file.hash != destination_file.hash =>
            {
                updated_files += 1;
                bytes_to_write += source_file.bytes;
                (DifferenceState::Different, source_file.bytes)
            }
            (Some(source_file), Some(_)) => {
                unchanged_files += 1;
                (DifferenceState::Identical, source_file.bytes)
            }
            (None, Some(destination_file)) => {
                destination_only_files += 1;
                (DifferenceState::OnlyRight, destination_file.bytes)
            }
            (None, None) => unreachable!("relative paths come from one side or the other"),
        };
        items.push(BackupItem {
            relative,
            state,
            bytes,
        });
    }

    Ok(BackupPlan {
        source,
        destination: destination.to_path_buf(),
        items,
        new_files,
        updated_files,
        unchanged_files,
        destination_only_files,
        bytes_to_write,
    })
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
            "tau-backup-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn first_backup_to_a_folder_that_does_not_exist_yet_is_all_new() {
        let root = scratch_root("first");
        let source = root.join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("a.mp3"), b"one").unwrap();
        fs::write(source.join("b.flac"), b"two").unwrap();
        let destination = root.join("backup-not-created-yet");
        let plan = plan(&source, &destination).unwrap();
        assert_eq!(plan.new_files, 2);
        assert_eq!(plan.updated_files, 0);
        assert_eq!(plan.unchanged_files, 0);
        assert_eq!(plan.destination_only_files, 0);
        assert_eq!(plan.bytes_to_write, 6);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn classifies_new_updated_unchanged_and_destination_only() {
        let root = scratch_root("classify");
        let source = root.join("source");
        let destination = root.join("destination");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::write(source.join("same.mp3"), b"same").unwrap();
        fs::write(destination.join("same.mp3"), b"same").unwrap();
        fs::write(source.join("changed.mp3"), b"new-content").unwrap();
        fs::write(destination.join("changed.mp3"), b"old-content").unwrap();
        fs::write(source.join("new.flac"), b"brand new").unwrap();
        fs::write(destination.join("stale.flac"), b"only at destination").unwrap();
        let result = plan(&source, &destination).unwrap();
        assert_eq!(result.new_files, 1);
        assert_eq!(result.updated_files, 1);
        assert_eq!(result.unchanged_files, 1);
        assert_eq!(result.destination_only_files, 1);
        assert_eq!(
            result.bytes_to_write,
            b"new-content".len() as u64 + b"brand new".len() as u64
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_a_source_that_is_not_a_folder() {
        let root = scratch_root("not-a-folder");
        assert!(plan(&root.join("missing"), &root.join("dest")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
