//! What happens to a source file on its way to the Pocket: copied as it is, converted into something the
//! player can play, or left out with a reason. Settings, the decision for each file, and size estimates.
//!
//! Principles: a file the Pocket can play is never re-encoded; conversion never changes a source; lossless
//! sources become FLAC (rate folded into the 44.1/48 kHz families, at most stereo, bit depth kept up to 24),
//! lossy sources become MP3 through the user's own `lame`.

use crate::details;
use std::path::{Path, PathBuf};
use tau_audio::decode::{SourceInfo, probe};
use tau_audio::pocket;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Mode {
    /// Match the best quality the Pocket accepts.
    #[default]
    Auto,
    /// Use the limits below.
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ReplayGainMode {
    /// Do not touch ReplayGain tags.
    Off,
    /// Keep tags that exist and add what is missing (default).
    #[default]
    AddMissing,
    /// Measure everything again and replace every tag (in the copies only).
    Rebuild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Mp3Quality {
    /// LAME's variable-rate quality, 0 (best, about 245 kbit/s) to 9.
    Vbr(u8),
    /// A constant bit rate in kbit/s.
    Cbr(u16),
}

impl From<Mp3Quality> for tau_audio::lame::Quality {
    fn from(q: Mp3Quality) -> Self {
        match q {
            Mp3Quality::Vbr(v) => Self::Vbr(v),
            Mp3Quality::Cbr(k) => Self::Cbr(k),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct ConvertSettings {
    /// Convert files the Pocket cannot play. Off means they are left out (and listed).
    pub enabled: bool,
    pub mode: Mode,
    /// Manual: the highest FLAC sample rate to write, 44100 or 48000.
    pub max_rate: u32,
    /// Manual: the FLAC bit depth to write at most, 16 or 24.
    pub flac_bits: u32,
    /// Manual: also convert playable FLAC files that are above these limits (default off).
    pub reduce_playable: bool,
    /// Manual: MP3 quality for lossy sources. Auto uses `Vbr(0)`.
    pub mp3: Mp3Quality,
    /// A `lame` the user chose (otherwise it is looked for).
    pub lame_path: Option<PathBuf>,
    /// Where the app keeps a `lame` it installed.
    pub lame_dir: Option<PathBuf>,
    /// A host file where measured loudness is remembered between runs (never on the card). `None` keeps it
    /// in memory for one plan.
    pub analysis_cache: Option<PathBuf>,
    pub replaygain: ReplayGainMode,
}

impl Default for ConvertSettings {
    /// Everything off: what plain copies (core to core, tests) use.
    fn default() -> Self {
        Self {
            enabled: false,
            mode: Mode::Auto,
            max_rate: 48_000,
            flac_bits: 24,
            reduce_playable: false,
            mp3: Mp3Quality::Vbr(0),
            lame_path: None,
            lame_dir: None,
            analysis_cache: None,
            replaygain: ReplayGainMode::Off,
        }
    }
}

impl ConvertSettings {
    /// The defaults the app and the CLI use for a sync.
    pub fn product() -> Self {
        Self {
            enabled: true,
            replaygain: ReplayGainMode::AddMissing,
            ..Self::default()
        }
    }

    pub fn find_lame(&self) -> Option<PathBuf> {
        tau_audio::lame::find(self.lame_path.as_deref(), self.lame_dir.as_deref())
    }
}

/// Runs `lame --version` and returns its first line (proves the program works).
pub fn lame_version(path: &Path) -> Result<String, crate::TauError> {
    tau_audio::lame::version(path)
        .map_err(|e| crate::TauError::e(crate::ErrorCode::UnsupportedTags, e.to_string()))
}

/// How a converted file is made.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Kind {
    Flac {
        rate: u32,
        bits: u32,
        channels: u16,
        dither: bool,
    },
    Mp3 {
        quality: Mp3Quality,
        rate: u32,
        channels: u16,
    },
}

impl Kind {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Flac { .. } => "flac",
            Self::Mp3 { .. } => "mp3",
        }
    }

    pub fn rate(&self) -> u32 {
        match self {
            Self::Flac { rate, .. } | Self::Mp3 { rate, .. } => *rate,
        }
    }

    pub fn channels(&self) -> u16 {
        match self {
            Self::Flac { channels, .. } | Self::Mp3 { channels, .. } => *channels,
        }
    }

    /// A stable text for the plan token and the ledger.
    pub fn spec(&self) -> String {
        match self {
            Self::Flac {
                rate,
                bits,
                channels,
                dither,
            } => format!("flac:{rate}:{bits}:{channels}:{}", *dither as u8),
            Self::Mp3 {
                quality,
                rate,
                channels,
            } => match quality {
                Mp3Quality::Vbr(v) => format!("mp3:v{v}:{rate}:{channels}"),
                Mp3Quality::Cbr(k) => format!("mp3:b{k}:{rate}:{channels}"),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Copy the bytes unchanged.
    Copy,
    Convert(Kind),
    /// Leave out, with the reason shown to the user.
    Skip(String),
}

/// The decision for one source file.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub action: Action,
    /// Why (shown in the review): empty for a plain copy.
    pub reason: String,
    pub info: Option<SourceInfo>,
}

impl Decision {
    fn copy(info: Option<SourceInfo>) -> Self {
        Self {
            action: Action::Copy,
            reason: String::new(),
            info,
        }
    }
    fn skip(reason: impl Into<String>, info: Option<SourceInfo>) -> Self {
        let reason = reason.into();
        Self {
            action: Action::Skip(reason.clone()),
            reason,
            info,
        }
    }
}

/// Extensions this tool tries to convert (the Pocket plays only MP3 and FLAC).
pub fn convertible_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some(
            "wav"
                | "wave"
                | "aif"
                | "aiff"
                | "aifc"
                | "m4a"
                | "m4b"
                | "mp4"
                | "aac"
                | "ogg"
                | "oga"
        )
    )
}

fn lossless_target(info: &SourceInfo, s: &ConvertSettings) -> Kind {
    let src_bits = info.bits.unwrap_or(16).clamp(16, 24);
    let channels = info.channels.min(2);
    let (rate, bits) = match s.mode {
        Mode::Auto => (pocket::auto_rate(info.rate), src_bits),
        Mode::Manual => {
            let limit = if s.max_rate <= 44_100 { 44_100 } else { 48_000 };
            let rate = if info.rate <= limit { info.rate } else { limit };
            let bits = src_bits.min(if s.flac_bits <= 16 { 16 } else { 24 });
            (rate, bits)
        }
    };
    let dither = bits == 16 && info.bits.unwrap_or(16) > 16;
    Kind::Flac {
        rate,
        bits,
        channels,
        dither,
    }
}

fn mp3_rate(rate: u32) -> u32 {
    const OK: [u32; 9] = [
        8_000, 11_025, 12_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000,
    ];
    if OK.contains(&rate) {
        rate
    } else if rate.is_multiple_of(44_100) || (rate < 48_000 && rate.is_multiple_of(11_025)) {
        44_100
    } else {
        48_000
    }
}

fn lossy_target(info: &SourceInfo, s: &ConvertSettings) -> Kind {
    let quality = match s.mode {
        Mode::Auto => Mp3Quality::Vbr(0),
        Mode::Manual => s.mp3,
    };
    Kind::Mp3 {
        quality,
        rate: mp3_rate(info.rate),
        channels: info.channels.min(2),
    }
}

/// Decides what to do with `path`. `lame_available` says whether MP3 can be made. Probing reads only the
/// file's headers.
pub fn decide(path: &Path, s: &ConvertSettings, lame_available: bool) -> Decision {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "mp3" => Decision::copy(None),
        "flac" => {
            let Ok(info) = probe(path) else {
                return Decision::copy(None); // unreadable here: copy it as it is, the Problems check names it
            };
            let mut why = Vec::new();
            if info.rate > pocket::MAX_FLAC_RATE {
                why.push(format!(
                    "{} kHz is above the 48 kHz the Pocket plays",
                    info.rate as f64 / 1000.0
                ));
            }
            if info.channels > pocket::MAX_CHANNELS {
                why.push(format!(
                    "{} channels (the Pocket plays at most 2)",
                    info.channels
                ));
            }
            if let Some(bits) = info.bits
                && !matches!(bits, 8 | 16 | 20 | 24)
            {
                why.push(format!("{bits}-bit FLAC is not supported by the player"));
            }
            if why.is_empty() && s.mode == Mode::Manual && s.reduce_playable && s.enabled {
                let limit = if s.max_rate <= 44_100 { 44_100 } else { 48_000 };
                let depth = if s.flac_bits <= 16 { 16 } else { 24 };
                if info.rate > limit {
                    why.push(format!("above your {} kHz limit", limit as f64 / 1000.0));
                }
                if info.bits.is_some_and(|b| b > depth) {
                    why.push(format!("above your {depth}-bit limit"));
                }
            }
            if why.is_empty() {
                return Decision::copy(Some(info));
            }
            let reason = why.join(", ");
            if !s.enabled {
                return Decision {
                    action: Action::Copy,
                    reason: format!("copied as it is, but the Pocket will not play it: {reason}"),
                    info: Some(info),
                };
            }
            let kind = lossless_target(&info, s);
            Decision {
                action: Action::Convert(kind),
                reason,
                info: Some(info),
            }
        }
        _ => {
            let info = match probe(path) {
                Ok(i) => i,
                Err(e) => return Decision::skip(format!("this format cannot be read ({e})"), None),
            };
            if !s.enabled {
                return Decision::skip("the Pocket plays MP3 and FLAC only", Some(info));
            }
            let name = info.codec.to_uppercase();
            if info.lossless {
                let kind = lossless_target(&info, s);
                Decision {
                    action: Action::Convert(kind),
                    reason: format!("{name} becomes FLAC"),
                    info: Some(info),
                }
            } else if lame_available {
                let kind = lossy_target(&info, s);
                Decision {
                    action: Action::Convert(kind),
                    reason: format!(
                        "{name} becomes MP3 (converting a lossy file cannot add quality)"
                    ),
                    info: Some(info),
                }
            } else {
                Decision::skip(
                    format!("{name} needs LAME to become MP3, and LAME is not installed"),
                    Some(info),
                )
            }
        }
    }
}

/// The destination file name for a converted file: the same name with the new extension.
pub fn converted_name(relative: &Path, kind: &Kind) -> PathBuf {
    relative.with_extension(kind.extension())
}

/// A rough size of the converted file, for the card-space check and the review.
pub fn estimate_bytes(info: &SourceInfo, source_bytes: u64, kind: &Kind) -> u64 {
    let seconds = info.frames.map(|f| f as f64 / info.rate.max(1) as f64);
    match kind {
        Kind::Flac {
            rate,
            bits,
            channels,
            ..
        } => match info.frames {
            Some(frames) => {
                let out_frames = frames as f64 * *rate as f64 / info.rate.max(1) as f64;
                (out_frames * *channels as f64 * (*bits as f64 / 8.0) * 0.65) as u64
            }
            None => source_bytes,
        },
        Kind::Mp3 { quality, .. } => {
            let kbps = match quality {
                Mp3Quality::Vbr(v) => [
                    245.0, 225.0, 190.0, 175.0, 165.0, 130.0, 115.0, 100.0, 85.0, 65.0,
                ][(*v).min(9) as usize],
                Mp3Quality::Cbr(k) => *k as f64,
            };
            seconds.map_or(source_bytes, |s| (s * kbps * 1000.0 / 8.0) as u64)
        }
    }
}

/// True when the details say this file already has every ReplayGain value `mode` would add.
pub fn has_full_replaygain(d: &details::Details) -> bool {
    use details::*;
    let need = F_RG_TRACK_GAIN | F_RG_TRACK_PEAK | F_RG_ALBUM_GAIN | F_RG_ALBUM_PEAK;
    d.flags & need == need
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(
        codec: &str,
        lossless: bool,
        rate: u32,
        channels: u16,
        bits: Option<u32>,
    ) -> SourceInfo {
        SourceInfo {
            codec: codec.into(),
            lossless,
            rate,
            channels,
            bits,
            frames: Some(rate as u64 * 60),
        }
    }

    #[test]
    fn auto_lossless_targets_follow_the_rate_family_and_keep_depth() {
        let s = ConvertSettings::product();
        assert_eq!(
            lossless_target(&info("pcm", true, 96_000, 2, Some(24)), &s),
            Kind::Flac {
                rate: 48_000,
                bits: 24,
                channels: 2,
                dither: false
            }
        );
        assert_eq!(
            lossless_target(&info("flac", true, 88_200, 2, Some(24)), &s),
            Kind::Flac {
                rate: 44_100,
                bits: 24,
                channels: 2,
                dither: false
            }
        );
        assert_eq!(
            lossless_target(&info("pcm", true, 192_000, 6, Some(32)), &s),
            Kind::Flac {
                rate: 48_000,
                bits: 24,
                channels: 2,
                dither: false
            }
        );
        assert_eq!(
            lossless_target(&info("pcm", true, 44_100, 2, Some(8)), &s),
            Kind::Flac {
                rate: 44_100,
                bits: 16,
                channels: 2,
                dither: false
            }
        );
    }

    #[test]
    fn manual_limits_cap_rate_and_depth_and_dither_when_reducing() {
        let s = ConvertSettings {
            mode: Mode::Manual,
            max_rate: 44_100,
            flac_bits: 16,
            ..ConvertSettings::product()
        };
        assert_eq!(
            lossless_target(&info("pcm", true, 96_000, 2, Some(24)), &s),
            Kind::Flac {
                rate: 44_100,
                bits: 16,
                channels: 2,
                dither: true
            }
        );
        assert_eq!(
            lossless_target(&info("pcm", true, 48_000, 2, Some(16)), &s),
            Kind::Flac {
                rate: 44_100,
                bits: 16,
                channels: 2,
                dither: false
            }
        );
    }

    #[test]
    fn mp3_rates_stay_in_the_legal_set() {
        assert_eq!(mp3_rate(44_100), 44_100);
        assert_eq!(mp3_rate(96_000), 48_000);
        assert_eq!(mp3_rate(88_200), 44_100);
        assert_eq!(mp3_rate(22_050), 22_050);
    }

    #[test]
    fn names_and_specs_are_stable() {
        let k = Kind::Flac {
            rate: 48_000,
            bits: 24,
            channels: 2,
            dither: false,
        };
        assert_eq!(
            converted_name(Path::new("A/b.wav"), &k),
            PathBuf::from("A/b.flac")
        );
        assert_eq!(k.spec(), "flac:48000:24:2:0");
        assert_eq!(
            Kind::Mp3 {
                quality: Mp3Quality::Vbr(0),
                rate: 44_100,
                channels: 2
            }
            .spec(),
            "mp3:v0:44100:2"
        );
    }

    #[test]
    fn mp3_files_are_copied_and_unreadable_flacs_too() {
        let s = ConvertSettings::product();
        assert_eq!(
            decide(Path::new("/nonexistent/a.mp3"), &s, true).action,
            Action::Copy
        );
        assert_eq!(
            decide(Path::new("/nonexistent/a.flac"), &s, true).action,
            Action::Copy
        );
        assert!(matches!(
            decide(Path::new("/nonexistent/a.wav"), &s, true).action,
            Action::Skip(_)
        ));
    }
}
