//! Leading and trailing silence of a track: how many sample frames at the start and at the end stay below a
//! level. Measured on the decoded audio at the file's own rate, so the numbers are frames of the source (what
//! a player counts), never of a converted copy. Nothing here changes a file.

use crate::AudioError;
use crate::decode::Decoder;
use std::path::Path;

/// The silent stretches of one track, in sample frames at `rate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeSilence {
    pub rate: u32,
    /// Frames in the whole track.
    pub frames: u64,
    /// Silent frames before the first loud one. Equals `frames` when the whole track is silent.
    pub lead: u64,
    /// Silent frames after the last loud one. 0 when the whole track is silent (the lead holds it all).
    pub tail: u64,
}

/// Linear amplitude of `db` decibels below full scale (so `-60.0` is 0.001).
pub fn amplitude(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Accumulates silence over interleaved frames, any chunking.
pub struct Scanner {
    channels: usize,
    limit: f32,
    frames: u64,
    first_loud: Option<u64>,
    last_loud: u64,
}

impl Scanner {
    pub fn new(channels: u16, threshold_db: f32) -> Self {
        Self {
            channels: channels.max(1) as usize,
            limit: amplitude(threshold_db),
            frames: 0,
            first_loud: None,
            last_loud: 0,
        }
    }

    /// Feeds interleaved samples (a whole number of frames).
    pub fn feed(&mut self, interleaved: &[f32]) {
        for frame in interleaved.chunks_exact(self.channels) {
            if frame.iter().any(|s| s.abs() > self.limit) {
                if self.first_loud.is_none() {
                    self.first_loud = Some(self.frames);
                }
                self.last_loud = self.frames;
            }
            self.frames += 1;
        }
    }

    pub fn finish(&self, rate: u32) -> EdgeSilence {
        match self.first_loud {
            None => EdgeSilence {
                rate,
                frames: self.frames,
                lead: self.frames,
                tail: 0,
            },
            Some(first) => EdgeSilence {
                rate,
                frames: self.frames,
                lead: first,
                tail: self.frames - 1 - self.last_loud,
            },
        }
    }
}

/// The thresholds measured together in one pass, dB below full scale: -80 to -40 in 5 dB steps. A setting picks the
/// nearest one, so changing it never needs the file read again.
pub const LEVELS_DB: [f32; 9] = [
    -80.0, -75.0, -70.0, -65.0, -60.0, -55.0, -50.0, -45.0, -40.0,
];

/// Decodes `path` once and measures its edge silence at every level of [`LEVELS_DB`] (same order).
pub fn measure_levels(
    path: &Path,
    progress: &mut dyn FnMut(u64) -> bool,
) -> Result<Vec<EdgeSilence>, AudioError> {
    let mut dec = Decoder::open(path)?;
    let ch = dec.info.channels;
    let mut scans: Vec<Scanner> = LEVELS_DB.iter().map(|&d| Scanner::new(ch, d)).collect();
    let mut buf = Vec::new();
    let mut frames = 0u64;
    while {
        buf.clear();
        dec.next(&mut buf)?
    } {
        for s in &mut scans {
            s.feed(&buf);
        }
        frames += (buf.len() / ch.max(1) as usize) as u64;
        if !progress(frames) {
            return Err(AudioError::Process("cancelled".into()));
        }
    }
    Ok(scans.iter().map(|s| s.finish(dec.info.rate)).collect())
}

/// Decodes `path` once and measures its edge silence. `progress` gets the frames read so far; `false` cancels.
pub fn measure_edges(
    path: &Path,
    threshold_db: f32,
    progress: &mut dyn FnMut(u64) -> bool,
) -> Result<EdgeSilence, AudioError> {
    let mut dec = Decoder::open(path)?;
    let mut scan = Scanner::new(dec.info.channels, threshold_db);
    let mut buf = Vec::new();
    let mut frames = 0u64;
    loop {
        buf.clear();
        if !dec.next(&mut buf)? {
            break;
        }
        scan.feed(&buf);
        frames += (buf.len() / dec.info.channels.max(1) as usize) as u64;
        if !progress(frames) {
            return Err(AudioError::Process("cancelled".into()));
        }
    }
    Ok(scan.finish(dec.info.rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, ch: usize, level: f32) -> Vec<f32> {
        vec![level; n * ch]
    }

    #[test]
    fn leading_and_trailing_silence_are_counted_in_frames_whatever_the_chunking() {
        let mut all = tone(1000, 2, 0.0);
        all.extend(tone(5000, 2, 0.3));
        all.extend(tone(2500, 2, 0.0005)); // -66 dB: still silence at -60
        for chunk in [2usize, 7, 1000, 100_000] {
            let mut s = Scanner::new(2, -60.0);
            for c in all.chunks(chunk * 2) {
                s.feed(c);
            }
            let e = s.finish(44_100);
            assert_eq!(
                (e.frames, e.lead, e.tail),
                (8500, 1000, 2500),
                "chunk {chunk}"
            );
        }
    }

    #[test]
    fn one_loud_channel_is_enough_and_a_silent_track_has_no_tail() {
        let mut s = Scanner::new(2, -60.0);
        s.feed(&[0.0, 0.0, 0.0, 0.5, 0.0, 0.0]);
        let e = s.finish(48_000);
        assert_eq!((e.lead, e.tail), (1, 1));
        let mut s = Scanner::new(1, -60.0);
        s.feed(&tone(300, 1, 0.0));
        let e = s.finish(48_000);
        assert_eq!((e.frames, e.lead, e.tail), (300, 300, 0));
    }

    #[test]
    fn a_louder_threshold_counts_more_silence() {
        let mut all = tone(100, 1, 0.01); // -40 dB
        all.extend(tone(100, 1, 0.5));
        let mut quiet = Scanner::new(1, -60.0);
        quiet.feed(&all);
        let mut loud = Scanner::new(1, -30.0);
        loud.feed(&all);
        assert_eq!(quiet.finish(8000).lead, 0);
        assert_eq!(loud.finish(8000).lead, 100);
    }

    #[test]
    fn every_level_is_measured_in_one_pass() {
        assert_eq!(LEVELS_DB.len(), 9);
        assert!(LEVELS_DB.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn a_wav_file_is_measured_at_its_own_rate() {
        let rate = 8000u32;
        let mut pcm: Vec<i16> = vec![0; 800];
        pcm.extend(std::iter::repeat_n(16_000i16, 1600));
        pcm.extend(vec![3i16; 400]); // about -80 dB
        let data: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + data.len() as u32).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(rate.to_le_bytes());
        wav.extend((rate * 2).to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((data.len() as u32).to_le_bytes());
        wav.extend(data);
        let p = std::env::temp_dir().join(format!("tau-silence-{}.wav", std::process::id()));
        std::fs::write(&p, wav).unwrap();
        let e = measure_edges(&p, -60.0, &mut |_| true).unwrap();
        std::fs::remove_file(&p).ok();
        assert_eq!((e.rate, e.frames, e.lead, e.tail), (8000, 2800, 800, 400));
    }
}
