//! Discovering screenshots on a card, read-only. The Pocket saves every
//! screenshot to `Memories/Screenshots/` at the card root (confirmed by
//! inspecting a real mounted card -- see `tau-alpha/docs/AUDIT_TRAIL.md`
//! B-133), named `YYYYMMDD_HHMMSS.png`. This is the "find them automatically"
//! complement to [`crate::taud::read_qr_report`], which already decodes one
//! screenshot the caller has picked -- this module is how a caller finds
//! that file in the first place, rather than hunting the filesystem by hand.

use crate::TauError;
use std::{
    fs,
    io,
    path::{Path, PathBuf},
};

/// One screenshot found under `Memories/Screenshots/`. `captured_at` is
/// `None` for a file that doesn't match the Pocket's own `YYYYMMDD_HHMMSS.png`
/// naming (a hand-copied fixture, a partial transfer, or anything else
/// dropped into that folder) -- listed anyway rather than silently dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScreenshotEntry {
    pub path: PathBuf,
    pub filename: String,
    pub bytes: u64,
    pub captured_at: Option<String>,
}

/// Lists every screenshot on a card, newest first. Returns an empty list
/// (not an error) when `Memories/Screenshots/` doesn't exist -- a perfectly
/// normal card that has never taken a screenshot. Skips hidden/junk files
/// (e.g. Finder's `._*` AppleDouble siblings) and anything that isn't a
/// `.png`.
pub fn list_screenshots(card_root: impl AsRef<Path>) -> Result<Vec<ScreenshotEntry>, TauError> {
    let dir = card_root.as_ref().join("Memories").join("Screenshots");
    let read_dir = match fs::read_dir(&dir) {
        Ok(read_dir) => read_dir,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(TauError::from(error)),
    };

    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().into_owned();
        if filename.starts_with('.') {
            continue;
        }
        if !filename.to_ascii_lowercase().ends_with(".png") {
            continue;
        }
        let bytes = entry.metadata()?.len();
        let captured_at = parse_pocket_timestamp(&filename);
        entries.push(ScreenshotEntry {
            path,
            filename,
            bytes,
            captured_at,
        });
    }
    // The Pocket's own YYYYMMDD_HHMMSS naming sorts chronologically as a
    // plain string, so this also puts anything unparsed at a stable
    // (alphabetical) position rather than requiring captured_at to sort.
    entries.sort_by(|a, b| b.filename.cmp(&a.filename));
    Ok(entries)
}

/// Parses `YYYYMMDD_HHMMSS.png` (or `.PNG`) into `"YYYY-MM-DDTHH:MM:SS"`.
/// `None` for anything else, rather than guessing at a looser format.
fn parse_pocket_timestamp(filename: &str) -> Option<String> {
    let stem = filename
        .strip_suffix(".png")
        .or_else(|| filename.strip_suffix(".PNG"))?;
    if stem.len() != 15 || stem.as_bytes()[8] != b'_' {
        return None;
    }
    let (date, time) = (&stem[0..8], &stem[9..15]);
    if !date.bytes().all(|b| b.is_ascii_digit()) || !time.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!(
        "{}-{}-{}T{}:{}:{}",
        &date[0..4],
        &date[4..6],
        &date[6..8],
        &time[0..2],
        &time[2..4],
        &time[4..6]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch_card(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tau-screenshots-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("Memories/Screenshots")).unwrap();
        root
    }

    fn real_fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/screenshots")
            .join(name)
    }

    #[test]
    fn lists_no_screenshots_as_an_empty_list_not_an_error() {
        let card = std::env::temp_dir().join(format!(
            "tau-screenshots-none-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&card).unwrap();
        assert_eq!(list_screenshots(&card).unwrap(), Vec::new());
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn finds_real_screenshots_newest_first_and_parses_the_pocket_timestamp() {
        let card = scratch_card("real");
        let dir = card.join("Memories/Screenshots");
        // Real files (copied, not hand-built) -- same fixture policy as
        // taud's own tests.
        fs::copy(
            real_fixture("20260921_220631.png"),
            dir.join("20260921_220631.png"),
        )
        .unwrap();
        fs::copy(
            real_fixture("20260921_234957.png"),
            dir.join("20260921_234957.png"),
        )
        .unwrap();
        // A Finder AppleDouble sibling, and a non-PNG file -- both must be
        // skipped, the same junk a real mounted card actually has.
        fs::write(dir.join("._20260921_220631.png"), b"junk").unwrap();
        fs::write(dir.join("notes.txt"), b"not a screenshot").unwrap();

        let entries = list_screenshots(&card).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].filename, "20260921_234957.png");
        assert_eq!(
            entries[0].captured_at.as_deref(),
            Some("2026-09-21T23:49:57")
        );
        assert_eq!(entries[1].filename, "20260921_220631.png");
        assert_eq!(
            entries[1].captured_at.as_deref(),
            Some("2026-09-21T22:06:31")
        );
        assert!(entries.iter().all(|entry| entry.bytes > 0));
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn lists_a_file_with_no_pocket_timestamp_shape_anyway() {
        let card = scratch_card("odd-name");
        fs::write(
            card.join("Memories/Screenshots/photo.png"),
            b"not empty",
        )
        .unwrap();
        let entries = list_screenshots(&card).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].filename, "photo.png");
        assert_eq!(entries[0].captured_at, None);
        fs::remove_dir_all(card).unwrap();
    }
}
