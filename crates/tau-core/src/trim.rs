//! Silence trimming at sync: how much leading and trailing silence a track gets marked to skip. Nothing
//! changes a file. The result is two counts of sample frames at the file's own rate, `lead_skip` and `tail_skip`,
//! the pair tau-alpha's `PLAYER_V2_SPEC.md` section 10.1 asks for,
//! counted on top of any encoder delay and padding the file already declares.

use tau_audio::silence::{EdgeSilence, LEVELS_DB};

/// Limits the settings are kept within: a setting outside them is brought back, never refused.
pub const THRESHOLD_DB10: (i16, i16) = (-800, -400);
pub const KEEP_MS: (u32, u32) = (0, 1_000);
pub const MIN_MS: (u32, u32) = (500, 10_000);
pub const MAX_MS: (u32, u32) = (1_000, 60_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct TrimSettings {
    /// Off by default.
    pub enabled: bool,
    /// Below this level (tenths of a dB below full scale, `-600` = -60 dB) audio counts as silence.
    pub threshold_db10: i16,
    /// Silence left in place at each end so a track still starts and ends naturally.
    pub keep_ms: u32,
    /// An end is only marked when at least this much silence would be skipped.
    pub min_ms: u32,
    /// The most skipped at either end, so a long quiet intro or fade is never cut away.
    pub max_ms: u32,
}

impl Default for TrimSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_db10: -600,
            keep_ms: 100,
            min_ms: 2_000,
            max_ms: 10_000,
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
            keep_ms: self.keep_ms.clamp(KEEP_MS.0, KEEP_MS.1),
            min_ms: self.min_ms.clamp(MIN_MS.0, MIN_MS.1),
            max_ms: self.max_ms.clamp(MAX_MS.0, MAX_MS.1),
        }
    }

    pub fn threshold_db(&self) -> f32 {
        f32::from(self.clamped().threshold_db10) / 10.0
    }
}

/// Frames to skip at each end, at the track's own rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrimPoints {
    /// Frames to skip at the start (`lead_skip` in tau-alpha's `PLAYER_V2_SPEC.md` section 10.1).
    pub lead_skip: u32,
    /// Frames to skip at the end (`tail_skip`).
    pub tail_skip: u32,
}

impl TrimPoints {
    pub fn is_none(&self) -> bool {
        self.lead_skip == 0 && self.tail_skip == 0
    }
}

/// What to skip for a measured track. A silent track is never trimmed.
pub fn plan_trim(edge: &EdgeSilence, settings: &TrimSettings) -> TrimPoints {
    let s = settings.clamped();
    if !settings.enabled || edge.lead >= edge.frames {
        return TrimPoints::default();
    }
    let ms = |m: u32| u64::from(edge.rate) * u64::from(m) / 1000;
    let end_of = |silence: u64| {
        let cut = silence.saturating_sub(ms(s.keep_ms));
        if cut < ms(s.min_ms) {
            0
        } else {
            cut.min(ms(s.max_ms)) as u32
        }
    };
    TrimPoints {
        lead_skip: end_of(edge.lead),
        tail_skip: end_of(edge.tail),
    }
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

    #[test]
    fn off_by_default_and_a_disabled_setting_trims_nothing() {
        assert!(!TrimSettings::default().enabled);
        assert!(plan_trim(&edge(441_000, 441_000), &TrimSettings::default()).is_none());
    }

    #[test]
    fn silence_is_cut_down_to_what_the_keep_setting_leaves() {
        // 4 s at the start, 2.5 s at the end, keep 100 ms
        let p = plan_trim(&edge(176_400, 110_250), &on());
        assert_eq!(
            (p.lead_skip, p.tail_skip),
            (176_400 - 4_410, 110_250 - 4_410)
        );
    }

    #[test]
    fn short_silences_are_left_alone() {
        // 2 s of silence minus 100 ms kept is 1.9 s: under the 2 s minimum
        assert!(plan_trim(&edge(88_200, 88_200), &on()).is_none());
        // 2.2 s: 2.1 s to skip, over the minimum
        assert_eq!(plan_trim(&edge(97_020, 0), &on()).lead_skip, 97_020 - 4_410);
    }

    #[test]
    fn at_most_the_cap_is_skipped_at_either_end() {
        let p = plan_trim(&edge(44_100 * 60, 44_100 * 30), &on());
        assert_eq!((p.lead_skip, p.tail_skip), (441_000, 441_000));
    }

    #[test]
    fn a_silent_track_is_never_trimmed() {
        let all = EdgeSilence {
            rate: 44_100,
            frames: 441_000,
            lead: 441_000,
            tail: 0,
        };
        assert!(plan_trim(&all, &on()).is_none());
    }

    #[test]
    fn settings_outside_the_limits_are_brought_back_inside() {
        let s = TrimSettings {
            enabled: true,
            threshold_db10: -1500,
            keep_ms: 99_999,
            min_ms: 0,
            max_ms: 5,
        }
        .clamped();
        assert_eq!(
            (s.threshold_db10, s.keep_ms, s.min_ms, s.max_ms),
            (-800, 1_000, 500, 1_000)
        );
        let s = TrimSettings {
            threshold_db10: 0,
            ..on()
        };
        assert_eq!(s.threshold_db(), -40.0);
    }
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

static ACTIVE: std::sync::RwLock<TrimSettings> = std::sync::RwLock::new(TrimSettings {
    enabled: false,
    threshold_db10: -600,
    keep_ms: 100,
    min_ms: 2_000,
    max_ms: 10_000,
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
