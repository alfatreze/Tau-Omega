//! Read-only media-root comparison for the multi-core workflow.

use crate::TauError;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifferenceState {
    OnlyLeft,
    OnlyRight,
    Different,
    Identical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaDifference {
    pub relative: PathBuf,
    pub state: DifferenceState,
    pub left_bytes: Option<u64>,
    pub right_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaComparison {
    pub left: PathBuf,
    pub right: PathBuf,
    pub differences: Vec<MediaDifference>,
}

impl MediaComparison {
    pub fn count(&self, state: DifferenceState) -> usize {
        self.differences
            .iter()
            .filter(|item| item.state == state)
            .count()
    }
}

/// Compares supported media and playlist files by card-relative path and
/// SHA-256. It never reads or writes the Tau index and makes no filesystem
/// changes, which makes it safe as the first step of a later copy/move flow.
pub fn media_roots(left: &Path, right: &Path) -> Result<MediaComparison, TauError> {
    validate_media_root(left)?;
    validate_media_root(right)?;
    let left = left.canonicalize()?;
    let right = right.canonicalize()?;
    let left_files = files_by_relative_path(&left)?;
    let right_files = files_by_relative_path(&right)?;
    let mut paths = left_files
        .keys()
        .chain(right_files.keys())
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    let differences = paths
        .into_iter()
        .map(
            |relative| match (left_files.get(&relative), right_files.get(&relative)) {
                (Some(left), Some(right)) if left.hash == right.hash => MediaDifference {
                    relative,
                    state: DifferenceState::Identical,
                    left_bytes: Some(left.bytes),
                    right_bytes: Some(right.bytes),
                },
                (Some(left), Some(right)) => MediaDifference {
                    relative,
                    state: DifferenceState::Different,
                    left_bytes: Some(left.bytes),
                    right_bytes: Some(right.bytes),
                },
                (Some(left), None) => MediaDifference {
                    relative,
                    state: DifferenceState::OnlyLeft,
                    left_bytes: Some(left.bytes),
                    right_bytes: None,
                },
                (None, Some(right)) => MediaDifference {
                    relative,
                    state: DifferenceState::OnlyRight,
                    left_bytes: None,
                    right_bytes: Some(right.bytes),
                },
                (None, None) => unreachable!("comparison paths originate from one side"),
            },
        )
        .collect();
    Ok(MediaComparison {
        left,
        right,
        differences,
    })
}

struct MediaFile {
    bytes: u64,
    hash: String,
}

fn files_by_relative_path(root: &Path) -> Result<BTreeMap<PathBuf, MediaFile>, TauError> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            Ok((
                relative,
                MediaFile {
                    bytes: fs::metadata(&path)?.len(),
                    hash: sha256_file(&path)?,
                },
            ))
        })
        .collect()
}

fn collect_files(root: &Path, at: &Path, files: &mut Vec<PathBuf>) -> Result<(), TauError> {
    for entry in fs::read_dir(at)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(root, &path, files)?;
        } else if path.is_file() && supported(&path) && path.strip_prefix(root).is_ok() {
            files.push(path);
        }
    }
    Ok(())
}

fn supported(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("mp3" | "flac" | "m3u")
    )
}

fn validate_media_root(root: &Path) -> Result<(), TauError> {
    let components = root.components().collect::<Vec<_>>();
    let has_assets = components.iter().any(
        |component| matches!(component, std::path::Component::Normal(name) if *name == "Assets"),
    );
    if !has_assets || root.file_name().is_none_or(|name| name != "common") || !root.is_dir() {
        return Err(TauError::Io(
            "both locations must be explicit Assets/<platform>/common media roots".into(),
        ));
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, TauError> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn classifies_media_without_mutating_either_core() {
        let root = std::env::temp_dir().join(format!(
            "tau-compare-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left/Assets/tau/common");
        let right = root.join("right/Assets/tau-test/common");
        fs::create_dir_all(left.join("album")).unwrap();
        fs::create_dir_all(right.join("album")).unwrap();
        fs::write(left.join("same.mp3"), b"same").unwrap();
        fs::write(right.join("same.mp3"), b"same").unwrap();
        fs::write(left.join("album/left.flac"), b"left").unwrap();
        fs::write(right.join("album/right.flac"), b"right").unwrap();
        fs::write(left.join("changed.mp3"), b"left").unwrap();
        fs::write(right.join("changed.mp3"), b"right").unwrap();
        let result = media_roots(&left, &right).unwrap();
        assert_eq!(result.count(DifferenceState::Identical), 1);
        assert_eq!(result.count(DifferenceState::OnlyLeft), 1);
        assert_eq!(result.count(DifferenceState::OnlyRight), 1);
        assert_eq!(result.count(DifferenceState::Different), 1);
        assert_eq!(fs::read(left.join("same.mp3")).unwrap(), b"same");
        fs::remove_dir_all(root).unwrap();
    }
}
