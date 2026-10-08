//! The "do not index" marker for a Pocket card: an empty `.metadata_never_index` file at the card's root, which tells
//! macOS Spotlight not to scan the volume. Indexing a card full of music writes `.Spotlight-V100` to it, keeps a
//! background process reading it, and is the likeliest reason an eject or a delete reports "in use".
//!
//! Whole card only (the marker is honoured at a volume's root, and renaming folders to `.noindex` would break the
//! exact paths the Pocket opens). Adding it is a card write, so a host asks the user first and keeps their answer as a
//! setting. Removing it undoes exactly that: the file is zero bytes and nothing else is touched.

use crate::{ErrorCode, TauError};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const MARKER: &str = ".metadata_never_index";

/// The card a path belongs to: the nearest folder (the path itself, or one above it) that has both `Assets` and
/// `Cores`. `None` for a plain staging folder or a path on no card.
pub fn find_card_root(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() { path } else { path.parent()? };
    start
        .ancestors()
        .find(|dir| dir.join("Assets").is_dir() && dir.join("Cores").is_dir())
        .map(Path::to_path_buf)
}

/// Whether the marker is on the card containing `path`. `None` when `path` is not on a Pocket card.
pub fn is_present(path: &Path) -> Option<bool> {
    find_card_root(path).map(|root| root.join(MARKER).is_file())
}

/// Adds the marker to the card containing `path`. Returns `true` if it was created, `false` if it was already there
/// or `path` is not on a Pocket card (nothing is written then).
pub fn ensure(path: &Path) -> Result<bool, TauError> {
    let Some(root) = find_card_root(path) else {
        return Ok(false);
    };
    let marker = root.join(MARKER);
    if marker.exists() {
        return Ok(false);
    }
    fs::write(&marker, b"")?;
    // The OS may add a `._` companion beside what it writes; it is junk.
    let _ = fs::remove_file(root.join(format!("._{MARKER}")));
    Ok(true)
}

/// Removes the marker from the card containing `path`. Returns `true` if a marker was removed. Only an empty file
/// with exactly this name is removed: anything else at that path is left alone and reported.
pub fn remove(path: &Path) -> Result<bool, TauError> {
    let Some(root) = find_card_root(path) else {
        return Ok(false);
    };
    let marker = root.join(MARKER);
    match fs::metadata(&marker) {
        Err(_) => Ok(false),
        Ok(meta) if meta.is_file() && meta.len() == 0 => {
            fs::remove_file(&marker)?;
            let _ = fs::remove_file(root.join(format!("._{MARKER}")));
            Ok(true)
        }
        Ok(_) => Err(TauError::e(
            ErrorCode::InvalidPathReference,
            format!("{MARKER} on this card is not an empty file, so it was left alone"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("tau-marker-{name}-{}", crate::test_uniq()));
        fs::create_dir_all(root.join("Assets/tau/common")).unwrap();
        fs::create_dir_all(root.join("Cores")).unwrap();
        root
    }

    #[test]
    fn finds_the_card_from_a_card_a_media_root_or_a_file() {
        let root = card("find");
        fs::write(root.join("Assets/tau/common/x.mp3"), b"x").unwrap();
        for p in [
            root.clone(),
            root.join("Assets/tau/common"),
            root.join("Assets/tau/common/x.mp3"),
        ] {
            assert_eq!(find_card_root(&p).as_deref(), Some(root.as_path()), "{p:?}");
        }
        let staging =
            std::env::temp_dir().join(format!("tau-marker-staging-{}", crate::test_uniq()));
        fs::create_dir_all(&staging).unwrap();
        assert_eq!(
            find_card_root(&staging),
            None,
            "a plain folder is not a card"
        );
        assert_eq!(is_present(&staging), None);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(staging).unwrap();
    }

    #[test]
    fn adds_and_removes_exactly_one_empty_file() {
        let root = card("addremove");
        let before: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(is_present(&root), Some(false));
        assert!(
            ensure(&root.join("Assets/tau/common")).unwrap(),
            "created from a media root"
        );
        assert!(root.join(MARKER).is_file() && fs::metadata(root.join(MARKER)).unwrap().len() == 0);
        assert_eq!(is_present(&root), Some(true));
        assert!(!ensure(&root).unwrap(), "already there: nothing written");
        assert!(remove(&root).unwrap() && !root.join(MARKER).exists());
        assert!(!remove(&root).unwrap());
        let after: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(before, after, "the card is exactly as it was");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn writes_nothing_outside_a_pocket_card_and_never_removes_someone_elses_file() {
        let plain = std::env::temp_dir().join(format!("tau-marker-plain-{}", crate::test_uniq()));
        fs::create_dir_all(&plain).unwrap();
        assert!(!ensure(&plain).unwrap());
        assert!(!plain.join(MARKER).exists());
        let root = card("foreign");
        fs::write(root.join(MARKER), b"someone's note").unwrap();
        assert!(remove(&root).is_err());
        assert_eq!(fs::read(root.join(MARKER)).unwrap(), b"someone's note");
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(plain).unwrap();
    }
}
