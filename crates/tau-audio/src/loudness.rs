//! Loudness for ReplayGain (EBU R128 integrated loudness, sample peak) measured on what is actually
//! written: the pipeline's output. Track and album values follow the ReplayGain 2.0 reference of -18 LUFS.

use crate::AudioError;
use crate::pipeline::Pipeline;
use ebur128::{EbuR128, Mode};

/// The ReplayGain 2.0 reference level.
pub const REFERENCE_LUFS: f64 = -18.0;

/// One track's measurement. Keeps the analyser so album loudness can combine tracks.
pub struct Measured {
    pub track_lufs: f64,
    /// Highest sample magnitude across channels (linear, 1.0 = full scale).
    pub peak: f64,
    analyser: EbuR128,
}

/// Measures a whole pipeline (consuming it). `progress` gets the frames measured so far; `false` cancels.
pub fn measure(
    pipeline: &mut Pipeline,
    progress: &mut dyn FnMut(u64) -> bool,
) -> Result<Measured, AudioError> {
    let channels = pipeline.target.channels as u32;
    let mut analyser = EbuR128::new(channels, pipeline.target.rate, Mode::I | Mode::SAMPLE_PEAK)
        .map_err(|e| AudioError::Process(format!("loudness analysis: {e}")))?;
    let mut buf = Vec::new();
    let mut frames = 0u64;
    loop {
        buf.clear();
        let n = pipeline.read(8192, &mut buf)?;
        if n == 0 {
            break;
        }
        frames += n as u64;
        if !progress(frames) {
            return Err(AudioError::Process("cancelled".into()));
        }
        let f: Vec<f32> = buf.iter().map(|&s| s as f32).collect();
        analyser
            .add_frames_f32(&f)
            .map_err(|e| AudioError::Process(format!("loudness analysis: {e}")))?;
    }
    let track_lufs = analyser
        .loudness_global()
        .map_err(|e| AudioError::Process(format!("loudness analysis: {e}")))?;
    let mut peak = 0.0f64;
    for c in 0..channels {
        peak = peak.max(
            analyser
                .sample_peak(c)
                .map_err(|e| AudioError::Process(format!("{e}")))?,
        );
    }
    Ok(Measured {
        track_lufs,
        peak,
        analyser,
    })
}

/// Gain in dB that brings `lufs` to the reference level. Silence (-inf) gets 0.
pub fn gain_db(lufs: f64) -> f64 {
    if lufs.is_finite() {
        REFERENCE_LUFS - lufs
    } else {
        0.0
    }
}

/// Album loudness over several measured tracks (same rate and channel count are not required).
pub fn album_lufs(tracks: &[&Measured]) -> Option<f64> {
    let analysers: Vec<&EbuR128> = tracks.iter().map(|m| &m.analyser).collect();
    EbuR128::loudness_global_multiple(analysers.into_iter())
        .ok()
        .filter(|l| l.is_finite())
}

/// The tag values (`-6.54 dB`, `0.987654`) in the form every ReplayGain-aware player reads.
pub fn format_gain(db: f64) -> String {
    format!("{db:+.2} dB")
}

pub fn format_peak(peak: f64) -> String {
    format!("{peak:.6}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_maps_to_zero_gain_and_formats_are_stable() {
        assert_eq!(gain_db(-18.0), 0.0);
        assert!((gain_db(-23.0) - 5.0).abs() < 1e-12);
        assert_eq!(gain_db(f64::NEG_INFINITY), 0.0);
        assert_eq!(format_gain(-6.543), "-6.54 dB");
        assert_eq!(format_gain(0.0), "+0.00 dB");
        assert_eq!(format_peak(0.9876543), "0.987654");
    }
}
