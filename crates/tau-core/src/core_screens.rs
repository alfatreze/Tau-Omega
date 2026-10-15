//! Screenshots on the card, and which of them probably belong to a core.
//!
//! The Pocket writes every screenshot to one flat folder (`Memories/Screenshots/<YYYYMMDD_HHMMSS>.png`) and the PNG holds no
//! text chunks, so the card does **not** record which core took it. What the card does record is when each core's save states
//! were made (`Memories/Save States/<core>/<YYYYMMDD_HHMMSS>_...`). A screenshot taken within [`WINDOW_SECS`] of one of a core's
//! save states is listed as "likely this core", with that reason; everything else stays unassigned. Both stamps are the Pocket's
//! own wall clock, so they are compared as plain wall-clock seconds (no time zone involved).

use crate::{ErrorCode, TauError, core_files::stamp_text, corehub};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// How close a screenshot must be to a save state of the core to count as "likely this core".
pub const WINDOW_SECS: i64 = 600;
/// Screenshots returned per call (the total is still counted).
pub const SHOT_LIMIT: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shot {
    pub file: String,
    /// `YYYY-MM-DD HH:MM:SS` from the file name.
    pub taken: String,
    pub bytes: u64,
    /// Taken close to one of this core's save states; `None` when it cannot be told.
    pub likely: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScreenshotView {
    /// Newest first, at most [`SHOT_LIMIT`].
    pub shots: Vec<Shot>,
    pub total: usize,
    pub likely: usize,
    /// How many save states of this core the match used.
    pub states: usize,
    pub window_secs: i64,
}

/// `YYYYMMDD_HHMMSS` -> seconds on an arbitrary wall-clock axis (days since 1970-01-01 by the civil calendar, then the time).
pub fn stamp_secs(name: &str) -> Option<i64> {
    let t = stamp_text(name)?;
    let n = |a: usize, b: usize| t[a..b].parse::<i64>().ok();
    let (y, m, d, h, mi, s) = (
        n(0, 4)?,
        n(5, 7)?,
        n(8, 10)?,
        n(11, 13)?,
        n(14, 16)?,
        n(17, 19)?,
    );
    // days from civil (Howard Hinnant)
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + s)
}

fn screenshots_dir(card: &Path) -> PathBuf {
    card.join("Memories").join("Screenshots")
}

/// A screenshot file name that is a plain `.png` inside the screenshots folder (the UI hands names back to us).
pub fn screenshot_path(card: &Path, file: &str) -> Result<PathBuf, TauError> {
    if file.is_empty()
        || file.contains(['/', '\\', '\0'])
        || file.contains("..")
        || !file.to_ascii_lowercase().ends_with(".png")
    {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            "That is not a screenshot on this card.",
        ));
    }
    let path = screenshots_dir(card).join(file);
    path.is_file()
        .then_some(path)
        .ok_or_else(|| TauError::e(ErrorCode::NotFound, "That screenshot is not on this card."))
}

pub fn screenshots(card: &Path, core_id: &str) -> Result<ScreenshotView, TauError> {
    corehub::core_dir(card, core_id)?;
    let state_times: Vec<i64> = fs::read_dir(card.join("Memories/Save States").join(core_id))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| stamp_secs(&e.file_name().to_string_lossy()))
        .collect();
    let mut shots = Vec::new();
    for e in fs::read_dir(screenshots_dir(card))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || !name.to_ascii_lowercase().ends_with(".png") {
            continue;
        }
        let Some(taken) = stamp_text(&name) else {
            continue;
        };
        let secs = stamp_secs(&name).unwrap_or(0);
        let nearest = state_times.iter().map(|t| (t - secs).abs()).min();
        let likely = nearest.filter(|d| *d <= WINDOW_SECS).map(|d| {
            if d < 60 {
                "taken within a minute of one of this core's save states".to_string()
            } else {
                format!(
                    "taken {} min from one of this core's save states",
                    (d + 59) / 60
                )
            }
        });
        let bytes = e.metadata().map_or(0, |m| m.len());
        shots.push(Shot {
            file: name,
            taken,
            bytes,
            likely,
        });
    }
    shots.sort_by(|a, b| b.file.cmp(&a.file));
    let total = shots.len();
    let likely = shots.iter().filter(|s| s.likely.is_some()).count();
    shots.truncate(SHOT_LIMIT);
    Ok(ScreenshotView {
        shots,
        total,
        likely,
        states: state_times.len(),
        window_secs: WINDOW_SECS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corehub::testcard::card;

    #[test]
    fn stamps_become_seconds_on_one_axis() {
        assert_eq!(stamp_secs("19700101_000000.png"), Some(0));
        assert_eq!(stamp_secs("19700102_000001.png"), Some(86_401));
        assert_eq!(
            stamp_secs("20000301_000000.png").unwrap() - stamp_secs("20000229_000000.png").unwrap(),
            86_400
        );
        assert_eq!(stamp_secs("2026-01-01.png"), None);
    }

    #[test]
    fn near_a_save_state_is_likely_this_core_and_the_rest_is_unassigned() {
        let card = card("shots");
        let v = screenshots(&card, "Spiritualized.GBA").unwrap();
        assert_eq!((v.total, v.likely, v.states, v.window_secs), (3, 2, 1, 600));
        // newest first
        assert_eq!(
            v.shots.iter().map(|s| s.file.as_str()).collect::<Vec<_>>(),
            [
                "20260305_090000.png",
                "20260101_121000.png",
                "20260101_120300.png"
            ]
        );
        assert!(v.shots[0].likely.is_none());
        assert!(v.shots[2].likely.as_deref().unwrap().contains("3 min"));
        // 12:10:00 is exactly 10 minutes after 12:00:00: inside the window
        assert!(v.shots[1].likely.is_some());
        // a core with no save states claims nothing
        let none = screenshots(&card, "other.GBAlt").unwrap();
        assert_eq!((none.total, none.likely, none.states), (3, 0, 0));
    }

    #[test]
    fn screenshot_paths_stay_inside_the_folder() {
        let card = card("shotpath");
        assert!(screenshot_path(&card, "20260101_120300.png").is_ok());
        for bad in [
            "../x.png",
            "a/b.png",
            "",
            "20260101_120300.txt",
            "nothere.png",
        ] {
            assert!(screenshot_path(&card, bad).is_err(), "{bad}");
        }
    }
}
