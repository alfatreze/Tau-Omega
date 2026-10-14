//! Decode, fold to stereo, resample: one pull-style source of interleaved `f64` frames at the target rate.

use crate::AudioError;
use crate::decode::{Decoder, SourceInfo};
use crate::mixdown;
use crate::resample::Resample;
use std::path::Path;

/// What the output should be (rate and channels; the bit depth is applied when samples are written).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub rate: u32,
    pub channels: u16,
}

impl Target {
    /// Keep the source's rate and channel count, folding more than two channels to stereo.
    pub fn native(info: &SourceInfo) -> Self {
        Self {
            rate: info.rate,
            channels: info.channels.min(2),
        }
    }
}

pub struct Pipeline {
    decoder: Decoder,
    resample: Option<Resample>,
    source_channels: usize,
    fold: bool,
    pub target: Target,
    pub info: SourceInfo,
    pending: Vec<f64>,
    raw: Vec<f32>,
    folded: Vec<f32>,
    eof: bool,
    /// Source frames decoded so far.
    pub frames_read: u64,
}

impl Pipeline {
    pub fn open(path: &Path, target: Target) -> Result<Self, AudioError> {
        let decoder = Decoder::open(path)?;
        let info = decoder.info.clone();
        let source_channels = info.channels as usize;
        let fold = source_channels > 2;
        if (fold && target.channels != 2) || (!fold && target.channels as usize != source_channels)
        {
            return Err(AudioError::Process(format!(
                "cannot turn {source_channels} channels into {}",
                target.channels
            )));
        }
        let resample = if info.rate != target.rate {
            Some(Resample::new(
                info.rate,
                target.rate,
                target.channels as usize,
            )?)
        } else {
            None
        };
        Ok(Self {
            decoder,
            resample,
            source_channels,
            fold,
            target,
            info,
            pending: Vec::new(),
            raw: Vec::new(),
            folded: Vec::new(),
            eof: false,
            frames_read: 0,
        })
    }

    fn pump(&mut self) -> Result<(), AudioError> {
        if self.eof {
            return Ok(());
        }
        self.raw.clear();
        if !self.decoder.next(&mut self.raw)? {
            self.eof = true;
            if let Some(r) = self.resample.as_mut() {
                r.finish(&mut self.pending)?;
            }
            return Ok(());
        }
        self.frames_read += (self.raw.len() / self.source_channels) as u64;
        let samples: &[f32] = if self.fold {
            self.folded.clear();
            mixdown::to_stereo(
                &self.raw,
                self.source_channels,
                self.decoder.positions,
                &mut self.folded,
            );
            &self.folded
        } else {
            &self.raw
        };
        match self.resample.as_mut() {
            Some(r) => {
                let wide: Vec<f64> = samples.iter().map(|&s| s as f64).collect();
                r.push(&wide, &mut self.pending)?;
            }
            None => self.pending.extend(samples.iter().map(|&s| s as f64)),
        }
        Ok(())
    }

    /// Moves up to `frames` frames into `out` (interleaved). Returns how many frames were moved; 0 means done.
    pub fn read(&mut self, frames: usize, out: &mut Vec<f64>) -> Result<usize, AudioError> {
        let ch = self.target.channels as usize;
        while self.pending.len() < frames * ch && !self.eof {
            self.pump()?;
        }
        let take = (self.pending.len() / ch).min(frames);
        out.extend(self.pending.drain(..take * ch));
        Ok(take)
    }
}

/// Rounds samples to `bits` with optional triangular dither (used only when reducing the depth).
pub struct Quantizer {
    bits: u32,
    dither: bool,
    state: u64,
}

impl Quantizer {
    pub fn new(bits: u32, dither: bool) -> Self {
        Self {
            bits,
            dither,
            state: 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn rand(&mut self) -> f64 {
        // xorshift64*, mapped to [0, 1)
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        (self.state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn to_i32(&mut self, sample: f64) -> i32 {
        let scale = (1i64 << (self.bits - 1)) as f64;
        let mut v = sample * scale;
        if self.dither {
            v += self.rand() - self.rand(); // TPDF, +-1 LSB
        }
        v.round().clamp(-scale, scale - 1.0) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantizing_rounds_and_clamps() {
        let mut q = Quantizer::new(16, false);
        assert_eq!(q.to_i32(0.0), 0);
        assert_eq!(q.to_i32(1.0), 32767);
        assert_eq!(q.to_i32(-1.0), -32768);
        assert_eq!(q.to_i32(0.5), 16384);
        assert_eq!(q.to_i32(2.0), 32767);
        let mut q24 = Quantizer::new(24, false);
        assert_eq!(q24.to_i32(1.0), 8_388_607);
    }

    #[test]
    fn dither_stays_within_one_step_and_is_not_constant() {
        let mut q = Quantizer::new(16, true);
        let vals: Vec<i32> = (0..2000).map(|_| q.to_i32(0.25)).collect();
        assert!(vals.iter().all(|v| (*v - 8192).abs() <= 1));
        assert!(vals.iter().any(|v| *v != vals[0]));
    }
}
