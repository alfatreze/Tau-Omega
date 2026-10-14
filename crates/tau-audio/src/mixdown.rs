//! Folding multichannel audio to the stereo the Pocket plays.
//!
//! Rule (stated to users in the review): left gets the front-left, 0.707 of the centre and 0.707 of each
//! left surround or side channel; right the mirror image; the low-frequency channel is left out. Each
//! output channel is then divided by the sum of its weights, so a full-scale signal cannot clip. The result
//! is quieter than the source; ReplayGain brings the level back.

use symphonia::core::audio::Position;

const H: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// `(left, right)` weights of each interleaved channel, from the speaker positions the file declares.
fn weights_from_positions(bits: u64, channels: usize) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(channels);
    for bit in 0..64u32 {
        if bits & (1 << bit) == 0 {
            continue;
        }
        let p = Position::from_bits_truncate(1 << bit);
        out.push(match p {
            Position::FRONT_LEFT => (1.0, 0.0),
            Position::FRONT_RIGHT => (0.0, 1.0),
            Position::FRONT_CENTER => (H, H),
            Position::REAR_LEFT
            | Position::SIDE_LEFT
            | Position::FRONT_LEFT_CENTER
            | Position::FRONT_LEFT_WIDE => (H, 0.0),
            Position::REAR_RIGHT
            | Position::SIDE_RIGHT
            | Position::FRONT_RIGHT_CENTER
            | Position::FRONT_RIGHT_WIDE => (0.0, H),
            Position::REAR_CENTER => (0.5, 0.5),
            _ => (0.0, 0.0), // LFE, height and bottom channels are left out
        });
    }
    out.resize(channels, (0.0, 0.0));
    out
}

/// The usual order of a file that does not say (FLAC, WAV): L R C LFE Ls Rs [Lrs Rrs].
fn weights_by_count(channels: usize) -> Vec<(f32, f32)> {
    let mut w = vec![
        (1.0, 0.0),
        (0.0, 1.0),
        (H, H),
        (0.0, 0.0),
        (H, 0.0),
        (0.0, H),
        (H, 0.0),
        (0.0, H),
    ];
    match channels {
        3 => w.truncate(3),
        4 => w = vec![(1.0, 0.0), (0.0, 1.0), (H, 0.0), (0.0, H)], // quad: L R Ls Rs
        _ => w.truncate(channels.min(8)),
    }
    w.resize(channels, (0.0, 0.0));
    w
}

/// Folds interleaved `channels`-channel frames to stereo, appending to `out`. Mono and stereo are not
/// changed (the caller skips this for them).
pub fn to_stereo(input: &[f32], channels: usize, positions: Option<u64>, out: &mut Vec<f32>) {
    let mut w = match positions {
        Some(bits) if bits.count_ones() as usize == channels => {
            weights_from_positions(bits, channels)
        }
        _ => weights_by_count(channels),
    };
    let left_sum: f32 = w.iter().map(|x| x.0).sum();
    let right_sum: f32 = w.iter().map(|x| x.1).sum();
    for x in &mut w {
        if left_sum > 0.0 {
            x.0 /= left_sum;
        }
        if right_sum > 0.0 {
            x.1 /= right_sum;
        }
    }
    for frame in input.chunks_exact(channels) {
        let mut l = 0.0;
        let mut r = 0.0;
        for (sample, (wl, wr)) in frame.iter().zip(&w) {
            l += sample * wl;
            r += sample * wr;
        }
        out.push(l);
        out.push(r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_scale_5_1_cannot_clip_and_the_lfe_is_left_out() {
        // FL FR FC LFE BL BR at full scale in every channel
        let frame = [1.0f32; 6];
        let mut out = Vec::new();
        to_stereo(&frame, 6, None, &mut out);
        assert!(out[0] <= 1.0 + 1e-6 && out[1] <= 1.0 + 1e-6);
        // only the LFE: silence
        let mut out = Vec::new();
        to_stereo(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0], 6, None, &mut out);
        assert_eq!(out, vec![0.0, 0.0]);
        // only the front left: all in the left output
        let mut out = Vec::new();
        to_stereo(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0], 6, None, &mut out);
        assert!(out[0] > 0.0 && out[1] == 0.0);
    }

    #[test]
    fn declared_positions_decide_the_mapping() {
        // a file declaring front-right and front-left in that interleave order cannot happen (bit order is
        // fixed), so declare L R C and check the centre lands in both
        let bits = (Position::FRONT_LEFT | Position::FRONT_RIGHT | Position::FRONT_CENTER).bits();
        let mut out = Vec::new();
        to_stereo(&[0.0, 0.0, 1.0], 3, Some(bits), &mut out);
        assert!(out[0] > 0.0 && (out[0] - out[1]).abs() < 1e-6);
    }
}
