//! Skip silence at sync, following tau-alpha's `docs/features/SKIP_SILENCE_SPEC.md` (the contract between the player
//! and Omega): how many sample pairs of leading and trailing silence a track is marked to skip, written as one
//! `tau-skip.bin` per album folder. Nothing changes an audio file.

use std::{collections::BTreeMap, path::Path};
use tau_audio::silence::{EdgeSilence, LEVELS_DB};

/// Limits the settings are kept within: a setting outside them is brought back, never refused.
pub const THRESHOLD_DB10: (i16, i16) = (-800, -400);
pub const LEAD_MARGIN_MS: (u32, u32) = (0, 1_000);
pub const TAIL_MARGIN_MS: (u32, u32) = (0, 2_000);
pub const MIN_MS: (u32, u32) = (1_000, 10_000);
/// The file name the player looks for, in the folder that holds the tracks.
pub const SKIP_FILE: &str = "tau-skip.bin";
/// Entries a file holds (the player reads it with one 4,096 byte read).
pub const MAX_ENTRIES: usize = 340;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct TrimSettings {
    /// Off by default.
    pub enabled: bool,
    /// Below this level (tenths of a dB below full scale, `-600` = -60 dB) a 10 ms window counts as silent.
    pub threshold_db10: i16,
    /// Silence left in front of the first sound (a natural attack is not silence).
    pub lead_margin_ms: u32,
    /// Silence left after the last sound (a natural decay is not silence).
    pub tail_margin_ms: u32,
    /// An end is only marked when its silent run lasts at least this long (and at least its margin plus 1 s).
    pub min_ms: u32,
}

impl Default for TrimSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_db10: -600,
            lead_margin_ms: 200,
            tail_margin_ms: 500,
            min_ms: 2_000,
        }
    }
}

impl TrimSettings {
    /// The same settings with every value inside its limits.
    pub fn clamped(self) -> Self {
        Self {
            enabled: self.enabled,
            threshold_db10: self
                .threshold_db10
                .clamp(THRESHOLD_DB10.0, THRESHOLD_DB10.1),
            lead_margin_ms: self
                .lead_margin_ms
                .clamp(LEAD_MARGIN_MS.0, LEAD_MARGIN_MS.1),
            tail_margin_ms: self
                .tail_margin_ms
                .clamp(TAIL_MARGIN_MS.0, TAIL_MARGIN_MS.1),
            min_ms: self.min_ms.clamp(MIN_MS.0, MIN_MS.1),
        }
    }

    pub fn threshold_db(&self) -> f32 {
        f32::from(self.clamped().threshold_db10) / 10.0
    }
}

/// Sample pairs to skip at each end, at the track's own rate (counted from the first sample of the music).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrimPoints {
    pub lead_skip: u32,
    pub tail_skip: u32,
}

impl TrimPoints {
    pub fn is_none(&self) -> bool {
        self.lead_skip == 0 && self.tail_skip == 0
    }
}

/// What to skip for a measured track (spec section 4). A run is written only when it lasts at least `min_ms` and at
/// least its margin plus 1 s; the margin is left in place. Both ends together must leave more than 1 s of track,
/// and a silent track writes nothing.
pub fn plan_trim(edge: &EdgeSilence, settings: &TrimSettings) -> TrimPoints {
    let s = settings.clamped();
    if !settings.enabled || edge.lead >= edge.frames {
        return TrimPoints::default();
    }
    let ms = |m: u32| u64::from(edge.rate) * u64::from(m) / 1000;
    let end_of = |run: u64, margin: u32| {
        if run < ms(s.min_ms) || run < ms(margin) + ms(1000) {
            0
        } else {
            u32::try_from(run - ms(margin)).unwrap_or(0)
        }
    };
    let points = TrimPoints {
        lead_skip: end_of(edge.lead, s.lead_margin_ms),
        tail_skip: end_of(edge.tail, s.tail_margin_ms),
    };
    if u64::from(points.lead_skip) + u64::from(points.tail_skip) + ms(1000) >= edge.frames {
        return TrimPoints::default();
    }
    points
}

/// FNV-1a, 32 bit, of a file name as it is on the card (spec section 2.1).
pub fn name_hash(name: &str) -> u32 {
    name.bytes().fold(2_166_136_261u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16_777_619)
    })
}

/// The `tau-skip.bin` bytes for `(file name, skip)` entries (spec section 2): `TSK1`, count, reserved, CRC-32 of the
/// entries, then 12 bytes each. Tracks with nothing to skip are left out; more than [`MAX_ENTRIES`] are cut. `None`
/// when there is nothing to write.
pub fn skip_table(entries: &[(String, TrimPoints)]) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    let mut count = 0u16;
    for (name, p) in entries
        .iter()
        .filter(|(_, p)| !p.is_none())
        .take(MAX_ENTRIES)
    {
        body.extend(name_hash(name).to_le_bytes());
        body.extend(p.lead_skip.to_le_bytes());
        body.extend(p.tail_skip.to_le_bytes());
        count += 1;
    }
    if count == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(12 + body.len());
    out.extend(b"TSK1");
    out.extend(count.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend(crc32fast::hash(&body).to_le_bytes());
    out.extend(body);
    Some(out)
}

/// A hand edit of one track: `never` skips nothing; otherwise a typed lead and/or tail replaces the detected one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Override {
    pub never: bool,
    pub lead: Option<u32>,
    pub tail: Option<u32>,
}

/// The skip a track gets once its override is applied. A typed pair that would leave under 1 s of track is
/// refused (the detected values stand).
pub fn with_override(auto: TrimPoints, o: Option<&Override>, edge: &EdgeSilence) -> TrimPoints {
    let Some(o) = o else { return auto };
    if o.never {
        return TrimPoints::default();
    }
    let p = TrimPoints {
        lead_skip: o.lead.unwrap_or(auto.lead_skip),
        tail_skip: o.tail.unwrap_or(auto.tail_skip),
    };
    let one_second = u64::from(edge.rate);
    if u64::from(p.lead_skip) + u64::from(p.tail_skip) + one_second >= edge.frames {
        return auto;
    }
    p
}

type OverrideMap = BTreeMap<String, Override>;
static OVERRIDES: std::sync::RwLock<BTreeMap<std::path::PathBuf, OverrideMap>> =
    std::sync::RwLock::new(BTreeMap::new());

/// The hand edits kept for the card whose media root is `root` (track path relative to the root). Set by the app
/// from its preferences; a sync that rebuilds the index applies them.
pub fn set_overrides(root: &Path, edits: OverrideMap) {
    let mut all = OVERRIDES.write().unwrap_or_else(|e| e.into_inner());
    if edits.is_empty() {
        all.remove(root);
    } else {
        all.insert(root.to_path_buf(), edits);
    }
}

pub fn overrides_for(root: &Path) -> OverrideMap {
    OVERRIDES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(root)
        .cloned()
        .unwrap_or_default()
}

/// Writes (or removes, when a folder has nothing to skip) `tau-skip.bin` in every album folder of `common` that holds
/// scanned tracks, from each track's `_sil` measurement and `settings`. Other folders are left alone. Returns the
/// number of files written and the number of tracks beyond [`MAX_ENTRIES`] that were left out.
pub fn write_skip_files(
    common: &Path,
    entries: &[crate::scan::Entry],
    settings: &TrimSettings,
) -> std::io::Result<(usize, usize)> {
    let edits = overrides_for(common);
    let mut folders: BTreeMap<&str, Vec<(String, TrimPoints)>> = BTreeMap::new();
    for e in entries {
        let levels = e.tags.get("_sil").and_then(|t| decode_levels(t));
        let p = levels
            .as_ref()
            .map(|l| {
                let auto = plan_trim_levels(l, settings);
                with_override(auto, edits.get(&e.rel), &l[0])
            })
            .unwrap_or_default();
        folders
            .entry(e.dir.as_str())
            .or_default()
            .push((e.file.clone(), p));
    }
    // A folder that no longer holds any scanned track (an album removed) must not keep an orphan table.
    let known: std::collections::BTreeSet<std::path::PathBuf> =
        folders.keys().map(|d| common.join(d)).collect();
    fn orphans(at: &Path, known: &std::collections::BTreeSet<std::path::PathBuf>, depth: usize) {
        let Ok(read) = std::fs::read_dir(at) else {
            return;
        };
        for e in read.flatten() {
            let p = e.path();
            if e.file_type().is_ok_and(|t| t.is_dir()) && depth < 12 {
                orphans(&p, known, depth + 1);
            } else if e.file_name() == SKIP_FILE && !known.contains(at) {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    orphans(common, &known, 0);
    let (mut written, mut cut) = (0, 0);
    for (dir, tracks) in folders {
        let path = common.join(dir).join(SKIP_FILE);
        cut += tracks
            .iter()
            .filter(|(_, p)| !p.is_none())
            .count()
            .saturating_sub(MAX_ENTRIES);
        match skip_table(&tracks) {
            Some(bytes) => {
                let tmp = path.with_extension("bin.part");
                std::fs::write(&tmp, bytes)?;
                std::fs::rename(&tmp, &path)?;
                written += 1;
            }
            None => {
                if path.is_file() {
                    std::fs::remove_file(&path)?;
                }
            }
        }
    }
    Ok((written, cut))
}

/// Rewrites every folder's `tau-skip.bin` from a fresh scan (the ledger keeps the measurements, so only new or changed
/// files are decoded). Used after a hand edit.
pub fn rewrite_skip_files(
    common: &Path,
    settings: &TrimSettings,
) -> Result<(usize, usize), crate::TauError> {
    let scan = crate::scan::scan_dir(common, false)?;
    write_skip_files(common, &scan.entries, settings).map_err(crate::TauError::from)
}

/// One track in the skip-silence list.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SkipTrack {
    pub rel: String,
    pub dir: String,
    pub file: String,
    pub secs: u16,
    /// Seconds of source rate, so the UI can show times.
    pub rate: u32,
    /// `None` until the track has been measured (skip silence was off, or the file could not be read).
    pub skip: Option<TrimPoints>,
    pub edit: Option<Override>,
}

/// Every track under `common` with what would be skipped now (the setting's values and any hand edit).
pub fn skip_tracks(
    common: &Path,
    settings: &TrimSettings,
) -> Result<Vec<SkipTrack>, crate::TauError> {
    let scan = crate::scan::scan_dir(common, false)?;
    let edits = overrides_for(common);
    Ok(scan
        .entries
        .iter()
        .map(|e| {
            let levels = e.tags.get("_sil").and_then(|t| decode_levels(t));
            SkipTrack {
                rel: e.rel.clone(),
                dir: e.dir.clone(),
                file: e.file.clone(),
                secs: e.secs,
                rate: levels.as_ref().map_or(0, |l| l[0].rate),
                skip: levels.as_ref().map(|l| {
                    with_override(plan_trim_levels(l, settings), edits.get(&e.rel), &l[0])
                }),
                edit: edits.get(&e.rel).copied(),
            }
        })
        .collect())
}

/// What the editor shows for one track.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SkipDetail {
    pub rate: u32,
    pub frames: u64,
    /// 400 buckets of loudness, 0 (below -80 dB) to 255 (full scale).
    pub envelope: Vec<u8>,
    /// What the detector finds with the current settings, before any hand edit.
    pub auto: TrimPoints,
    pub skip: TrimPoints,
    pub edit: Option<Override>,
}

/// Decodes one track and returns its waveform and what would be skipped.
pub fn skip_detail(
    common: &Path,
    rel: &str,
    settings: &TrimSettings,
) -> Result<SkipDetail, crate::TauError> {
    if rel.split('/').any(|c| c == ".." || c.is_empty()) {
        return Err(crate::TauError::e(
            crate::ErrorCode::InvalidPathReference,
            "not a track path",
        ));
    }
    let a = tau_audio::silence::analyse(&common.join(rel), &mut |_| true)
        .map_err(|e| crate::TauError::e(crate::ErrorCode::UnsupportedTags, e.to_string()))?;
    let want = settings.threshold_db();
    let level = LEVELS_DB
        .iter()
        .enumerate()
        .min_by(|x, y| (x.1 - want).abs().total_cmp(&(y.1 - want).abs()))
        .map_or(0, |(i, _)| i);
    let on = TrimSettings {
        enabled: true,
        ..*settings
    };
    let auto = plan_trim(&a.levels[level], &on);
    let edit = overrides_for(common).get(rel).copied();
    let envelope = (0..400)
        .map(|b| {
            let (lo, hi) = (
                b * a.peaks.len() / 400,
                ((b + 1) * a.peaks.len() / 400).max(b * a.peaks.len() / 400 + 1),
            );
            let peak = a.peaks[lo.min(a.peaks.len().saturating_sub(1))..hi.min(a.peaks.len())]
                .iter()
                .fold(0f32, |m, &p| m.max(p));
            if peak <= 0.0 {
                0
            } else {
                (((20.0 * peak.log10() + 80.0) / 80.0).clamp(0.0, 1.0) * 255.0) as u8
            }
        })
        .collect();
    Ok(SkipDetail {
        rate: a.rate,
        frames: a.frames,
        envelope,
        auto,
        skip: with_override(auto, edit.as_ref(), &a.levels[level]),
        edit,
    })
}

/// One track's measurement at every level of [`LEVELS_DB`], kept as text in the scan's tag map so the ledger
/// cache holds it with the other tags: `rate,frames;lead,tail;lead,tail;...` (nine pairs).
pub fn encode_levels(levels: &[EdgeSilence]) -> Option<String> {
    let first = levels.first()?;
    if levels.len() != LEVELS_DB.len() {
        return None;
    }
    let mut out = format!("{},{}", first.rate, first.frames);
    for l in levels {
        out.push_str(&format!(";{},{}", l.lead, l.tail));
    }
    Some(out)
}

pub fn decode_levels(text: &str) -> Option<Vec<EdgeSilence>> {
    let mut parts = text.split(';');
    let (rate, frames) = parts.next()?.split_once(',')?;
    let (rate, frames): (u32, u64) = (rate.parse().ok()?, frames.parse().ok()?);
    let levels: Vec<EdgeSilence> = parts
        .map(|p| {
            let (l, t) = p.split_once(',')?;
            Some(EdgeSilence {
                rate,
                frames,
                lead: l.parse().ok()?,
                tail: t.parse().ok()?,
            })
        })
        .collect::<Option<_>>()?;
    (levels.len() == LEVELS_DB.len()).then_some(levels)
}

/// What to skip for a track measured at every level: the level nearest the setting's threshold decides.
pub fn plan_trim_levels(levels: &[EdgeSilence], settings: &TrimSettings) -> TrimPoints {
    let want = settings.threshold_db();
    let nearest = LEVELS_DB
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - want).abs().total_cmp(&(b.1 - want).abs()))
        .map_or(0, |(i, _)| i);
    levels
        .get(nearest)
        .map_or_else(TrimPoints::default, |e| plan_trim(e, settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(lead: u64, tail: u64) -> EdgeSilence {
        EdgeSilence {
            rate: 44_100,
            frames: 44_100 * 200,
            lead,
            tail,
        }
    }
    fn on() -> TrimSettings {
        TrimSettings {
            enabled: true,
            ..Default::default()
        }
    }
    const S: u64 = 44_100;

    #[test]
    fn off_by_default_and_a_disabled_setting_trims_nothing() {
        assert!(!TrimSettings::default().enabled);
        assert!(plan_trim(&edge(10 * S, 10 * S), &TrimSettings::default()).is_none());
    }

    #[test]
    fn the_margins_are_left_in_place_at_each_end() {
        // 10 s at the start keeps 200 ms; 60 s at the end keeps 500 ms
        let p = plan_trim(&edge(10 * S, 60 * S), &on());
        assert_eq!(
            (p.lead_skip as u64, p.tail_skip as u64),
            (10 * S - S / 5, 60 * S - S / 2)
        );
    }

    #[test]
    fn a_run_must_last_two_seconds_and_its_margin_plus_a_second() {
        assert!(plan_trim(&edge(2 * S - 1, 2 * S - 1), &on()).is_none());
        assert_eq!(
            plan_trim(&edge(2 * S, 0), &on()).lead_skip as u64,
            2 * S - S / 5
        );
        // a long margin raises the bar: 1.5 s margin needs a 2.5 s run even though 2 s is the minimum
        let s = TrimSettings {
            enabled: true,
            tail_margin_ms: 1_500,
            ..Default::default()
        };
        assert!(plan_trim(&edge(0, 2 * S + S / 4), &s).is_none());
        assert!(!plan_trim(&edge(0, 2 * S + S / 2), &s).is_none());
    }

    #[test]
    fn a_silent_track_and_a_track_that_would_be_all_skipped_write_nothing() {
        let all = EdgeSilence {
            rate: 44_100,
            frames: 441_000,
            lead: 441_000,
            tail: 0,
        };
        assert!(plan_trim(&all, &on()).is_none());
        let short = EdgeSilence {
            rate: 44_100,
            frames: 6 * S,
            lead: 3 * S,
            tail: 3 * S,
        };
        assert!(plan_trim(&short, &on()).is_none());
    }

    #[test]
    fn settings_outside_the_limits_are_brought_back_inside() {
        let s = TrimSettings {
            enabled: true,
            threshold_db10: -1500,
            lead_margin_ms: 99_999,
            tail_margin_ms: 99_999,
            min_ms: 0,
        }
        .clamped();
        assert_eq!(
            (
                s.threshold_db10,
                s.lead_margin_ms,
                s.tail_margin_ms,
                s.min_ms
            ),
            (-800, 1_000, 2_000, 1_000)
        );
        assert_eq!(
            TrimSettings {
                threshold_db10: 0,
                ..on()
            }
            .threshold_db(),
            -40.0
        );
    }

    #[test]
    fn the_hashes_and_the_example_file_equal_the_specs_bytes() {
        assert_eq!(name_hash("01 Intro.mp3"), 0xCB8F_593C);
        assert_eq!(name_hash("02 Hidden Track.mp3"), 0xFD52_1DCE);
        let got = skip_table(&[
            (
                "01 Intro.mp3".into(),
                TrimPoints {
                    lead_skip: 441_000,
                    tail_skip: 0,
                },
            ),
            (
                "02 Hidden Track.mp3".into(),
                TrimPoints {
                    lead_skip: 0,
                    tail_skip: 2_646_000,
                },
            ),
            ("03 Nothing.mp3".into(), TrimPoints::default()),
        ])
        .unwrap();
        let want: Vec<u8> = [
            0x54, 0x53, 0x4B, 0x31, 0x02, 0x00, 0x00, 0x00, 0x90, 0x08, 0xD0, 0x87, 0x3C, 0x59,
            0x8F, 0xCB, 0xA8, 0xBA, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0xCE, 0x1D, 0x52, 0xFD,
            0x00, 0x00, 0x00, 0x00, 0xF0, 0x5F, 0x28, 0x00,
        ]
        .to_vec();
        assert_eq!(got, want);
    }

    #[test]
    fn nothing_to_skip_writes_no_table_and_the_table_is_cut_at_340() {
        assert!(skip_table(&[("a.mp3".into(), TrimPoints::default())]).is_none());
        let many: Vec<_> = (0..400)
            .map(|i| {
                (
                    format!("{i:03}.mp3"),
                    TrimPoints {
                        lead_skip: 1,
                        tail_skip: 0,
                    },
                )
            })
            .collect();
        let t = skip_table(&many).unwrap();
        assert_eq!(
            (u16::from_le_bytes([t[4], t[5]]), t.len()),
            (340, 12 + 12 * 340)
        );
        assert!(t.len() <= 4096);
    }
}

static ACTIVE: std::sync::RwLock<TrimSettings> = std::sync::RwLock::new(TrimSettings {
    enabled: false,
    threshold_db10: -600,
    lead_margin_ms: 200,
    tail_margin_ms: 500,
    min_ms: 2_000,
});

/// The settings the app is running with: set once from the saved preferences and again when they change. A
/// scan measures silence only while they are enabled; the index build reads them to decide each track's skip.
/// Off until the app sets it, so every other caller (tests, the command line) is unchanged.
pub fn set_active(settings: TrimSettings) {
    *ACTIVE.write().unwrap_or_else(|e| e.into_inner()) = settings.clamped();
}

pub fn active() -> TrimSettings {
    *ACTIVE.read().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod level_tests {
    use super::*;

    fn lv(lead: u64, tail: u64) -> Vec<EdgeSilence> {
        (0..LEVELS_DB.len())
            .map(|i| EdgeSilence {
                rate: 44_100,
                frames: 44_100 * 200,
                lead: lead + i as u64,
                tail,
            })
            .collect()
    }

    #[test]
    fn levels_round_trip_as_text_and_bad_text_is_refused() {
        let l = lv(88_200, 44_100);
        assert_eq!(decode_levels(&encode_levels(&l).unwrap()).unwrap(), l);
        assert!(decode_levels("44100,100;1,2").is_none());
        assert!(decode_levels("x").is_none());
        assert!(encode_levels(&l[..3]).is_none());
    }

    #[test]
    fn the_level_nearest_the_threshold_decides() {
        let mut l = lv(0, 0);
        l[0].lead = 441_000; // -80 dB: 10 s of "silence" only at the most sensitive level
        let strict = TrimSettings {
            enabled: true,
            threshold_db10: -800,
            ..Default::default()
        };
        let normal = TrimSettings {
            enabled: true,
            ..Default::default()
        };
        assert!(plan_trim_levels(&l, &strict).lead_skip > 0);
        assert!(plan_trim_levels(&l, &normal).is_none());
        // between two levels: -62 dB is nearest -60
        let s = TrimSettings {
            enabled: true,
            threshold_db10: -620,
            ..Default::default()
        };
        assert_eq!(plan_trim_levels(&l, &s), plan_trim_levels(&l, &normal));
    }
}
