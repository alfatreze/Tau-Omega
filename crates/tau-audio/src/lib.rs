//! Audio work for Tau Omega: reading any common audio file, getting it into what the Pocket plays
//! (sample rate, channels, bit depth), writing FLAC (built in) or MP3 (through the user's `lame`), and
//! measuring loudness for ReplayGain. Nothing here touches a card or a user's tags; callers decide what is
//! written where.

pub mod decode;
pub mod flac_out;
pub mod lame;
pub mod loudness;
pub mod mixdown;
pub mod pipeline;
pub mod resample;

use std::fmt;

#[derive(Debug)]
pub enum AudioError {
    /// The file could not be opened, probed or decoded.
    Decode(String),
    /// Resampling, encoding or another processing step failed.
    Process(String),
    /// Reading or writing a file.
    Io(String),
    /// `lame` is missing or failed.
    Lame(String),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(m) | Self::Process(m) | Self::Io(m) | Self::Lame(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for AudioError {}

impl From<std::io::Error> for AudioError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

/// What the Pocket can play, and so what a conversion aims at.
pub mod pocket {
    /// The player refuses FLAC above this rate.
    pub const MAX_FLAC_RATE: u32 = 48_000;
    pub const MAX_CHANNELS: u16 = 2;
    /// The bit depths the FLAC decoder accepts that this encoder can write.
    pub const FLAC_DEPTHS: [u32; 2] = [16, 24];

    /// The rate to resample to for a source rate that is too high: the 44.1 family goes to 44.1 kHz,
    /// everything else to 48 kHz. A rate the Pocket plays is returned unchanged.
    pub fn auto_rate(source: u32) -> u32 {
        if source <= MAX_FLAC_RATE {
            source
        } else if source.is_multiple_of(44_100) {
            44_100
        } else {
            48_000
        }
    }
}

#[cfg(test)]
mod tests {
    use super::pocket::auto_rate;

    #[test]
    fn auto_rate_keeps_the_family() {
        assert_eq!(auto_rate(44_100), 44_100);
        assert_eq!(auto_rate(48_000), 48_000);
        assert_eq!(auto_rate(88_200), 44_100);
        assert_eq!(auto_rate(176_400), 44_100);
        assert_eq!(auto_rate(96_000), 48_000);
        assert_eq!(auto_rate(192_000), 48_000);
        assert_eq!(auto_rate(32_000), 32_000);
    }
}
