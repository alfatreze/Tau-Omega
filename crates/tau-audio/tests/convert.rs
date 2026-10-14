use std::path::PathBuf;
use tau_audio::decode::{Decoder, probe};
use tau_audio::flac_out::{self, Picture};
use tau_audio::pipeline::{Pipeline, Target};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tau-audio-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A little-endian PCM WAV writer (16 or 24 bit) for test inputs.
fn write_wav(path: &std::path::Path, rate: u32, channels: u16, bits: u16, samples: &[i32]) {
    let bytes = (bits / 8) as usize;
    let mut data = Vec::new();
    for s in samples {
        data.extend_from_slice(&s.to_le_bytes()[..bytes]);
    }
    let mut out = Vec::new();
    out.extend(b"RIFF");
    out.extend((36 + data.len() as u32).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(channels.to_le_bytes());
    out.extend(rate.to_le_bytes());
    out.extend((rate * channels as u32 * bytes as u32).to_le_bytes());
    out.extend((channels * bytes as u16).to_le_bytes());
    out.extend(bits.to_le_bytes());
    out.extend(b"data");
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(data);
    std::fs::write(path, out).unwrap();
}

fn noise(frames: usize, channels: usize, bits: u32) -> Vec<i32> {
    let mut x = 12345u32;
    let scale = 1i64 << (bits - 1);
    (0..frames * channels)
        .map(|i| {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            let t = (i / channels) as f64 / 40.0;
            let tone = (t.sin() * 0.5 * scale as f64) as i64;
            let n = ((x >> 8) as i64 % (scale / 8)) - scale / 16;
            (tone + n).clamp(-scale, scale - 1) as i32
        })
        .collect()
}

#[test]
fn a_24_bit_wav_becomes_a_bit_exact_flac_at_the_same_rate() {
    let dir = tmp("exact");
    let samples = noise(20_000, 2, 24);
    let wav = dir.join("in.wav");
    write_wav(&wav, 48_000, 2, 24, &samples);
    let info = probe(&wav).unwrap();
    assert_eq!((info.rate, info.channels, info.bits), (48_000, 2, Some(24)));
    assert!(info.lossless);
    let mut pipeline = Pipeline::open(&wav, Target::native(&info)).unwrap();
    let out = dir.join("out.flac");
    let comments = vec![("TITLE".to_string(), "Test".to_string())];
    let frames = flac_out::encode(
        &mut pipeline,
        24,
        false,
        &comments,
        Some(Picture {
            mime: "image/jpeg",
            data: &[0xff, 0xd8, 0xff, 0xd9],
        }),
        &out,
        &mut |_| true,
    )
    .unwrap();
    assert_eq!(frames, 20_000);
    let back_info = probe(&out).unwrap();
    assert_eq!(
        (
            back_info.codec.as_str(),
            back_info.rate,
            back_info.channels,
            back_info.bits
        ),
        ("flac", 48_000, 2, Some(24))
    );
    let mut dec = Decoder::open(&out).unwrap();
    let mut f = Vec::new();
    while dec.next(&mut f).unwrap() {}
    assert_eq!(f.len(), samples.len());
    let scale = (1i64 << 23) as f32;
    for (a, b) in samples.iter().zip(&f) {
        assert_eq!(*a, (b * scale).round() as i32);
    }
}

#[test]
fn a_96k_surround_file_becomes_48k_stereo() {
    let dir = tmp("surround");
    let samples = noise(9_600, 6, 24);
    let wav = dir.join("in.wav");
    write_wav(&wav, 96_000, 6, 24, &samples);
    let info = probe(&wav).unwrap();
    assert_eq!(info.channels, 6);
    let target = Target {
        rate: tau_audio::pocket::auto_rate(info.rate),
        channels: 2,
    };
    assert_eq!(target.rate, 48_000);
    let mut pipeline = Pipeline::open(&wav, target).unwrap();
    let out = dir.join("out.flac");
    flac_out::encode(&mut pipeline, 24, false, &[], None, &out, &mut |_| true).unwrap();
    let back = probe(&out).unwrap();
    assert_eq!((back.rate, back.channels), (48_000, 2));
    assert_eq!(back.frames, Some(4_800));
}

#[test]
fn reducing_to_16_bit_with_dither_keeps_the_signal() {
    let dir = tmp("dither");
    let samples = noise(10_000, 2, 24);
    let wav = dir.join("in.wav");
    write_wav(&wav, 44_100, 2, 24, &samples);
    let info = probe(&wav).unwrap();
    let mut pipeline = Pipeline::open(&wav, Target::native(&info)).unwrap();
    let out = dir.join("out.flac");
    flac_out::encode(&mut pipeline, 16, true, &[], None, &out, &mut |_| true).unwrap();
    assert_eq!(probe(&out).unwrap().bits, Some(16));
    let mut dec = Decoder::open(&out).unwrap();
    let mut f = Vec::new();
    while dec.next(&mut f).unwrap() {}
    let worst = samples
        .iter()
        .zip(&f)
        .map(|(a, b)| ((*a as f32 / 8_388_608.0) - b).abs())
        .fold(0f32, f32::max);
    assert!(worst < 3.0 / 32768.0, "worst error {worst}");
}

#[test]
fn cancelling_stops_the_encode() {
    let dir = tmp("cancel");
    let wav = dir.join("in.wav");
    write_wav(&wav, 44_100, 2, 16, &noise(50_000, 2, 16));
    let info = probe(&wav).unwrap();
    let mut pipeline = Pipeline::open(&wav, Target::native(&info)).unwrap();
    let r = flac_out::encode(
        &mut pipeline,
        16,
        false,
        &[],
        None,
        &dir.join("o.flac"),
        &mut |frames| frames < 8_000,
    );
    assert!(r.is_err());
}

#[test]
fn an_mp3_comes_out_of_lame_when_it_is_installed() {
    let Some(lame) = tau_audio::lame::find(None, None) else {
        eprintln!("lame is not installed here; skipped");
        return;
    };
    let dir = tmp("lame");
    let wav = dir.join("in.wav");
    write_wav(&wav, 44_100, 2, 16, &noise(44_100, 2, 16));
    let info = probe(&wav).unwrap();
    let mut pipeline = Pipeline::open(&wav, Target::native(&info)).unwrap();
    let out = dir.join("out.mp3");
    let tags = tau_audio::lame::Tags {
        title: Some("Test Title".into()),
        artist: Some("Someone".into()),
        ..Default::default()
    };
    let frames = tau_audio::lame::encode(
        &lame,
        &mut pipeline,
        tau_audio::lame::Quality::Vbr(2),
        &tags,
        None,
        &out,
        &mut |_| true,
    )
    .unwrap();
    assert_eq!(frames, 44_100);
    let back = probe(&out).unwrap();
    assert_eq!(
        (back.codec.as_str(), back.rate, back.channels),
        ("mp3", 44_100, 2)
    );
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"ID3"));
    // lame writes ID3v2 text as Latin-1 or UTF-16 depending on its version
    let utf16: Vec<u8> = "Test Title"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    assert!(
        bytes.windows(10).any(|w| w == b"Test Title")
            || bytes.windows(utf16.len()).any(|w| w == utf16.as_slice())
    );
}
