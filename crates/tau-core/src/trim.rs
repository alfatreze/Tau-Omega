//! Silence trimming at sync: how much leading and trailing silence a track gets marked to skip. Nothing
//! changes a file. The result is two counts of sample frames at the file's own rate, `start` and `end`,
//! the same pair the player keeps per track (tau-alpha's `PLAYER_V2_SPEC.md`, `trim_start` and `trim_end`),
//! counted on top of any encoder delay and padding the file already declares.

use tau_audio::silence::EdgeSilence;

/// Limits the settings are kept within: a setting outside them is brought back, never refused.
pub const THRESHOLD_DB10: (i16, i16) = (-800, -400);
pub const KEEP_MS: (u32, u32) = (0, 1_000);
pub const MIN_MS: (u32, u32) = (100, 5_000);
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
            min_ms: 300,
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
    pub start: u32,
    pub end: u32,
}

impl TrimPoints {
    pub fn is_none(&self) -> bool {
        self.start == 0 && self.end == 0
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
        start: end_of(edge.lead),
        end: end_of(edge.tail),
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
        // 2 s at the start, 1 s at the end, keep 100 ms
        let p = plan_trim(&edge(88_200, 44_100), &on());
        assert_eq!((p.start, p.end), (88_200 - 4_410, 44_100 - 4_410));
    }

    #[test]
    fn short_silences_are_left_alone() {
        // 350 ms of silence minus 100 ms kept is 250 ms: under the 300 ms minimum
        assert!(plan_trim(&edge(15_435, 15_435), &on()).is_none());
        // 450 ms: 350 ms to skip, over the minimum
        assert_eq!(plan_trim(&edge(19_845, 0), &on()).start, 19_845 - 4_410);
    }

    #[test]
    fn at_most_the_cap_is_skipped_at_either_end() {
        let p = plan_trim(&edge(44_100 * 60, 44_100 * 30), &on());
        assert_eq!((p.start, p.end), (441_000, 441_000));
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
            (-800, 1_000, 100, 1_000)
        );
        let s = TrimSettings {
            threshold_db10: 0,
            ..on()
        };
        assert_eq!(s.threshold_db(), -40.0);
    }
}
