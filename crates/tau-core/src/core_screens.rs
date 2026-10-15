//! Screenshots on the card, and which core took each one.
//!
//! The Pocket writes every screenshot to one flat folder (`Memories/Screenshots/<YYYYMMDD_HHMMSS>.png`) and **appends a 600-byte block
//! to the very end of the file** (after the PNG and two embedded Analogue-format pictures) that names the core: author, core short
//! name (the `Cores/<Author.Core>` folder), core version, the file the core had loaded in its first slot, and the platform id and
//! name. Checked on all 489 screenshots of a real card: every file has it, at the same place, with the same layout.
//!
//! ```text
//! 0x00 magic 01 52 50 41     0x48 author (32)       0x68 core short name (32)   0x88 version (32)
//! 0xA8 loaded file (256)     0x1A8 platform id (16) 0x1B8 platform name (32)    the rest is zero
//! ```
//!
//! A screenshot with that block belongs to its core exactly. A file without it (an older firmware, or a file that was edited) falls
//! back to a labelled guess: taken within [`WINDOW_SECS`] of one of the core's save states (both stamps are the Pocket's wall
//! clock, compared as plain seconds, no time zone).

use crate::{ErrorCode, TauError, core_files::stamp_text, corehub};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

/// How close a screenshot must be to a save state of the core to count as "likely this core".
pub const WINDOW_SECS: i64 = 600;
/// Screenshots returned per call (the total is still counted).
pub const SHOT_LIMIT: usize = 2000;

/// Size and magic of the block the Pocket appends to a screenshot.
pub const META_LEN: usize = 600;
const META_MAGIC: [u8; 4] = [0x01, 0x52, 0x50, 0x41];

/// What the Pocket recorded inside a screenshot.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShotMeta {
    /// `Author.Core`, the `Cores/` folder name.
    pub core: String,
    pub version: String,
    /// The file the core had loaded in its first slot (a game's ROM for most cores).
    pub loaded_file: String,
    pub platform_id: String,
    pub platform_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shot {
    pub file: String,
    /// `YYYY-MM-DD HH:MM:SS` from the file name.
    pub taken: String,
    pub bytes: u64,
    /// What the screenshot says about itself; `None` for a file without the block.
    pub meta: Option<ShotMeta>,
    /// The screenshot names this core.
    pub recorded: bool,
    /// Only for a file without the block: taken close to one of this core's save states.
    pub likely: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScreenshotView {
    /// Newest first, at most [`SHOT_LIMIT`].
    pub shots: Vec<Shot>,
    pub total: usize,
    /// Screenshots that name this core.
    pub recorded: usize,
    /// Screenshots without the block that were taken near this core's save states.
    pub likely: usize,
    /// Screenshots with the block (for any core): the rest are older files.
    pub with_meta: usize,
    /// How many save states of this core the guess used.
    pub states: usize,
    pub window_secs: i64,
}

fn field(block: &[u8], at: usize, len: usize) -> String {
    let raw = &block[at..at + len];
    let end = raw.iter().position(|b| *b == 0).unwrap_or(len);
    String::from_utf8_lossy(&raw[..end]).trim().to_string()
}

/// Reads the block from the last [`META_LEN`] bytes of a screenshot (the file is about 1 MB; only the tail is read).
pub fn read_meta(path: &Path) -> Option<ShotMeta> {
    let mut f = fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    if len < META_LEN as u64 {
        return None;
    }
    f.seek(SeekFrom::Start(len - META_LEN as u64)).ok()?;
    let mut block = [0u8; META_LEN];
    f.read_exact(&mut block).ok()?;
    parse_meta(&block)
}

pub fn parse_meta(block: &[u8]) -> Option<ShotMeta> {
    if block.len() != META_LEN || block[..4] != META_MAGIC {
        return None;
    }
    let (author, short) = (field(block, 0x48, 32), field(block, 0x68, 32));
    if author.is_empty()
        || short.is_empty()
        || author.chars().chain(short.chars()).any(char::is_control)
    {
        return None;
    }
    Some(ShotMeta {
        core: format!("{author}.{short}"),
        version: field(block, 0x88, 32),
        loaded_file: field(block, 0xA8, 256),
        platform_id: field(block, 0x1A8, 16),
        platform_name: field(block, 0x1B8, 32),
    })
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
        let meta = read_meta(&e.path());
        let recorded = meta
            .as_ref()
            .is_some_and(|m| m.core.eq_ignore_ascii_case(core_id));
        // The guess is only for files that do not say which core took them.
        let likely = if meta.is_none() {
            let secs = stamp_secs(&name).unwrap_or(0);
            state_times.iter().map(|t| (t - secs).abs()).min().filter(|d| *d <= WINDOW_SECS).map(|d| {
                if d < 60 { "no core is recorded in this older file; taken within a minute of one of this core's save states".to_string() } else { format!("no core is recorded in this older file; taken {} min from one of this core's save states", (d + 59) / 60) }
            })
        } else {
            None
        };
        let bytes = e.metadata().map_or(0, |m| m.len());
        shots.push(Shot {
            file: name,
            taken,
            bytes,
            meta,
            recorded,
            likely,
        });
    }
    shots.sort_by(|a, b| b.file.cmp(&a.file));
    let total = shots.len();
    let recorded = shots.iter().filter(|s| s.recorded).count();
    let likely = shots.iter().filter(|s| s.likely.is_some()).count();
    let with_meta = shots.iter().filter(|s| s.meta.is_some()).count();
    shots.truncate(SHOT_LIMIT);
    Ok(ScreenshotView {
        shots,
        total,
        recorded,
        likely,
        with_meta,
        states: state_times.len(),
        window_secs: WINDOW_SECS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corehub::testcard::{card, write};

    /// A screenshot file as the Pocket writes it: a (here fake) PNG body, then the 600-byte block.
    pub(crate) fn shot_bytes(
        core: &str,
        version: &str,
        loaded: &str,
        platform_id: &str,
        platform_name: &str,
    ) -> Vec<u8> {
        let (author, short) = core.split_once('.').unwrap();
        let mut b = vec![0x89, b'P', b'N', b'G', 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut block = vec![0u8; META_LEN];
        block[..4].copy_from_slice(&META_MAGIC);
        let mut put = |at: usize, s: &str| block[at..at + s.len()].copy_from_slice(s.as_bytes());
        put(0x48, author);
        put(0x68, short);
        put(0x88, version);
        put(0xA8, loaded);
        put(0x1A8, platform_id);
        put(0x1B8, platform_name);
        b.extend(block);
        b
    }

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
    fn the_block_at_the_end_of_a_screenshot_names_its_core() {
        let m = parse_meta(
            &shot_bytes(
                "Spiritualized.GBA",
                "1.2.0",
                "Boktai 2 (USA).gba",
                "gba",
                "Game Boy Advance",
            )[13..],
        )
        .unwrap();
        assert_eq!(
            (
                m.core.as_str(),
                m.version.as_str(),
                m.loaded_file.as_str(),
                m.platform_id.as_str(),
                m.platform_name.as_str()
            ),
            (
                "Spiritualized.GBA",
                "1.2.0",
                "Boktai 2 (USA).gba",
                "gba",
                "Game Boy Advance"
            )
        );
        // wrong magic, wrong size, empty names, control characters: nothing is claimed
        let good = shot_bytes("a.B", "1", "x", "p", "P")[13..].to_vec();
        let mut bad = good.clone();
        bad[0] = 0;
        assert!(parse_meta(&bad).is_none() && parse_meta(&good[1..]).is_none());
        let mut empty = good.clone();
        empty[0x48..0x68].fill(0);
        assert!(parse_meta(&empty).is_none());
        let mut ctl = good;
        ctl[0x68] = 0x07;
        assert!(parse_meta(&ctl).is_none());
    }

    #[test]
    fn a_recorded_core_is_exact_and_older_files_fall_back_to_a_labelled_guess() {
        let card = card("shots");
        // the fixture's three bare screenshots have no block; add recorded ones and one older file near the state
        write(
            &card.join("Memories/Screenshots/20260101_120300.png"),
            &shot_bytes(
                "Spiritualized.GBA",
                "1.2.0",
                "Boktai 2 (USA).gba",
                "gba",
                "Game Boy Advance",
            ),
        );
        write(
            &card.join("Memories/Screenshots/20260101_130000.png"),
            &shot_bytes("other.GBAlt", "0.3", "Mine.gba", "gba", "Game Boy Advance"),
        );
        let v = screenshots(&card, "Spiritualized.GBA").unwrap();
        assert_eq!(
            (v.total, v.recorded, v.with_meta, v.states, v.window_secs),
            (4, 1, 2, 1, 600)
        );
        let by = |f: &str| v.shots.iter().find(|s| s.file == f).unwrap();
        let exact = by("20260101_120300.png");
        assert!(exact.recorded && exact.likely.is_none());
        assert_eq!(
            exact.meta.as_ref().unwrap().loaded_file,
            "Boktai 2 (USA).gba"
        );
        // another core's screenshot is never this core's, even though its time is near nothing
        assert!(!by("20260101_130000.png").recorded && by("20260101_130000.png").likely.is_none());
        // an older file with no block, 10 minutes from the state: the guess, with its reason
        let guess = by("20260101_121000.png");
        assert!(
            !guess.recorded
                && guess
                    .likely
                    .as_deref()
                    .unwrap()
                    .contains("no core is recorded")
        );
        assert_eq!(v.likely, 1);
        // a far-away older file claims nothing
        assert!(by("20260305_090000.png").likely.is_none());
        // the other core sees its own
        let other = screenshots(&card, "other.GBAlt").unwrap();
        assert_eq!((other.recorded, other.likely), (1, 0));
    }

    #[test]
    fn a_recorded_screenshot_beats_the_save_state_guess() {
        // even right next to this core's save state, a screenshot that names another core is not this core's
        let card = card("shots2");
        write(
            &card.join("Memories/Screenshots/20260101_120100.png"),
            &shot_bytes("other.GBAlt", "0.3", "Mine.gba", "gba", "GBA"),
        );
        let v = screenshots(&card, "Spiritualized.GBA").unwrap();
        let s = v
            .shots
            .iter()
            .find(|s| s.file == "20260101_120100.png")
            .unwrap();
        assert!(!s.recorded && s.likely.is_none());
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
