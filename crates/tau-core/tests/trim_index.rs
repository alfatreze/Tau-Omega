//! Silence trimming end to end: a real FLAC with silence at both ends is scanned, and the index it builds
//! carries a version 2 details section with the skips; with trimming off the section is version 1.
use std::{fs, path::Path};
use tau_audio::flac_out;
use tau_audio::pipeline::{Pipeline, Target};
use tau_core::trim::{self, TrimSettings};
use tau_core::{ROOT_PREFIX, build_index, scan_dir};

fn wav(path: &Path, rate: u32, lead: usize, tone: usize, tail: usize) {
    let mut pcm: Vec<i16> = vec![0; lead];
    pcm.extend((0..tone).map(|i| (((i + 1) as f64 * 0.2).sin() * 12_000.0) as i16));
    pcm.extend(vec![0i16; tail]);
    let data: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    let mut w = Vec::new();
    w.extend(b"RIFF");
    w.extend((36 + data.len() as u32).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend(rate.to_le_bytes());
    w.extend((rate * 2).to_le_bytes());
    w.extend(2u16.to_le_bytes());
    w.extend(16u16.to_le_bytes());
    w.extend(b"data");
    w.extend((data.len() as u32).to_le_bytes());
    w.extend(data);
    fs::write(path, w).unwrap();
}

fn flac(dir: &Path, name: &str, rate: u32, lead: usize, tone: usize, tail: usize) {
    let src = dir.join("src.wav");
    wav(&src, rate, lead, tone, tail);
    let mut p = Pipeline::open(&src, Target { rate, channels: 1 }).unwrap();
    flac_out::encode(&mut p, 16, false, &[], None, &dir.join(name), &mut |_| true).unwrap();
    fs::remove_file(src).unwrap();
}

fn details_header(index: &[u8]) -> (u16, u16) {
    let at = index
        .windows(4)
        .position(|w| w == b"TDET")
        .expect("a details section");
    (
        u16::from_le_bytes([index[at + 4], index[at + 5]]),
        u16::from_le_bytes([index[at + 6], index[at + 7]]),
    )
}

#[test]
fn skips_are_measured_at_scan_and_written_as_a_version_2_details_section() {
    let dir = std::env::temp_dir().join(format!("tau-trim-index-{}", std::process::id()));
    let album = dir.join("Album");
    fs::create_dir_all(&album).unwrap();
    // 8 kHz, mono: 5 s of silence, 3 s of tone, 3 s of silence
    flac(
        &album,
        "01 Long Silences.flac",
        8_000,
        40_000,
        24_000,
        24_000,
    );
    let build = || {
        let scan = scan_dir(&dir, false).unwrap();
        build_index(&scan.entries, &scan.playlists, ROOT_PREFIX, &mut Vec::new()).unwrap()
    };

    // off (the default): version 1, 32-byte records
    trim::set_active(TrimSettings::default());
    assert_eq!(details_header(&build()), (1, 32));

    // on: version 2, 40-byte records, the skips are the silence minus the 100 ms kept
    trim::set_active(TrimSettings {
        enabled: true,
        ..Default::default()
    });
    let index = build();
    assert_eq!(details_header(&index), (2, 40));
    let at = index.windows(4).position(|w| w == b"TDET").unwrap() + 16;
    let lead = u32::from_le_bytes(index[at + 32..at + 36].try_into().unwrap());
    let tail = u32::from_le_bytes(index[at + 36..at + 40].try_into().unwrap());
    assert_eq!((lead, tail), (40_000 - 800, 24_000 - 800));

    // a second scan reuses the stored measurement, and a stricter setting changes the answer without reading the file
    trim::set_active(TrimSettings {
        enabled: true,
        keep_ms: 500,
        ..Default::default()
    });
    let index = build();
    let at = index.windows(4).position(|w| w == b"TDET").unwrap() + 16;
    assert_eq!(
        u32::from_le_bytes(index[at + 32..at + 36].try_into().unwrap()),
        40_000 - 4_000
    );

    // back to off: version 1 again
    trim::set_active(TrimSettings::default());
    assert_eq!(details_header(&build()), (1, 32));
    fs::remove_dir_all(dir).ok();
}
