//! Streaming sample-rate conversion (rubato's FFT resampler, `f64`), trimmed to the exact output length.

use crate::AudioError;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

const CHUNK: usize = 2048;

pub struct Resample {
    inner: Fft<f64>,
    channels: usize,
    queue: Vec<f64>,
    to_trim: usize,
    frames_in: u64,
    frames_out: u64,
    out_buf: Vec<f64>,
    rate_in: u64,
    rate_out: u64,
}

fn err(e: impl std::fmt::Display) -> AudioError {
    AudioError::Process(format!("resampling failed: {e}"))
}

impl Resample {
    pub fn new(rate_in: u32, rate_out: u32, channels: usize) -> Result<Self, AudioError> {
        let inner = Fft::<f64>::new(
            rate_in as usize,
            rate_out as usize,
            CHUNK,
            channels,
            FixedSync::Input,
        )
        .map_err(err)?;
        let out_buf = vec![0.0; inner.output_frames_max() * channels];
        let to_trim = inner.output_delay();
        Ok(Self {
            inner,
            channels,
            queue: Vec::new(),
            to_trim,
            frames_in: 0,
            frames_out: 0,
            out_buf,
            rate_in: rate_in as u64,
            rate_out: rate_out as u64,
        })
    }

    /// Feeds interleaved frames; appends whatever output is ready to `out`.
    pub fn push(&mut self, input: &[f64], out: &mut Vec<f64>) -> Result<(), AudioError> {
        self.frames_in += (input.len() / self.channels) as u64;
        self.queue.extend_from_slice(input);
        loop {
            let need = self.inner.input_frames_next();
            if self.queue.len() / self.channels < need {
                return Ok(());
            }
            let take = need * self.channels;
            let (_used, produced) = {
                let chunk =
                    InterleavedSlice::new(&self.queue[..take], self.channels, need).map_err(err)?;
                let max = self.out_buf.len() / self.channels;
                let mut dest = InterleavedSlice::new_mut(&mut self.out_buf, self.channels, max)
                    .map_err(err)?;
                self.inner
                    .process_into_buffer(&chunk, &mut dest, None)
                    .map_err(err)?
            };
            self.queue.drain(..take);
            self.emit(produced, out);
        }
    }

    fn emit(&mut self, produced: usize, out: &mut Vec<f64>) {
        let skip = self.to_trim.min(produced);
        self.to_trim -= skip;
        let data = &self.out_buf[skip * self.channels..produced * self.channels];
        out.extend_from_slice(data);
        self.frames_out += (produced - skip) as u64;
    }

    /// Flushes the tail so the total output is exactly `ceil(frames_in * ratio)` frames.
    pub fn finish(&mut self, out: &mut Vec<f64>) -> Result<(), AudioError> {
        let expected = (self.frames_in * self.rate_out).div_ceil(self.rate_in);
        let need = self.inner.input_frames_next();
        let have = self.queue.len() / self.channels;
        if have > 0 || self.frames_out < expected {
            let mut padded = std::mem::take(&mut self.queue);
            padded.resize(need * self.channels, 0.0);
            let mut partial = Some(have);
            while self.frames_out < expected {
                let (_used, produced) = {
                    let chunk =
                        InterleavedSlice::new(&padded[..], self.channels, need).map_err(err)?;
                    let max = self.out_buf.len() / self.channels;
                    let mut dest = InterleavedSlice::new_mut(&mut self.out_buf, self.channels, max)
                        .map_err(err)?;
                    let indexing = Indexing {
                        input_offset: 0,
                        output_offset: 0,
                        active_channels_mask: None,
                        partial_len: partial,
                    };
                    self.inner
                        .process_into_buffer(&chunk, &mut dest, Some(&indexing))
                        .map_err(err)?
                };
                partial = Some(0);
                if produced == 0 {
                    break;
                }
                let before = out.len();
                self.emit(produced, out);
                let over = self.frames_out.saturating_sub(expected) as usize;
                if over > 0 {
                    out.truncate(before.max(out.len() - over * self.channels));
                    self.frames_out = expected;
                }
            }
        }
        // The streaming steps can run a frame or so past the exact length; the caller keeps the unconsumed
        // tail in `out`, so the surplus is removed from its end.
        if self.frames_out > expected {
            let extra = ((self.frames_out - expected) as usize * self.channels).min(out.len());
            out.truncate(out.len() - extra);
            self.frames_out = expected;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frames: usize, rate: f64, hz: f64) -> Vec<f64> {
        (0..frames)
            .map(|i| (2.0 * std::f64::consts::PI * hz * i as f64 / rate).sin() * 0.5)
            .collect()
    }

    #[test]
    fn the_output_length_is_exact_and_the_tone_survives() {
        let input = tone(44_100 * 2, 44_100.0, 1000.0);
        let mut r = Resample::new(44_100, 48_000, 1).unwrap();
        let mut out = Vec::new();
        for piece in input.chunks(5000) {
            r.push(piece, &mut out).unwrap();
        }
        r.finish(&mut out).unwrap();
        assert_eq!(out.len(), 96_000);
        // compare the middle of the output with an ideal 1 kHz tone at 48 kHz (allow a small phase offset)
        let ideal = tone(96_000, 48_000.0, 1000.0);
        let rms = |v: &[f64]| (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt();
        let diff: Vec<f64> = (10_000..80_000).map(|i| out[i] - ideal[i]).collect();
        assert!(rms(&diff) < 0.01, "error rms {}", rms(&diff));
    }

    #[test]
    fn a_short_clip_and_stereo_keep_their_length() {
        let mono = tone(1000, 96_000.0, 440.0);
        let mut stereo = Vec::new();
        for s in &mono {
            stereo.push(*s);
            stereo.push(-*s);
        }
        let mut r = Resample::new(96_000, 48_000, 2).unwrap();
        let mut out = Vec::new();
        r.push(&stereo, &mut out).unwrap();
        r.finish(&mut out).unwrap();
        assert_eq!(out.len() / 2, 500);
        assert!(out.chunks(2).all(|f| (f[0] + f[1]).abs() < 1e-9));
    }
}
