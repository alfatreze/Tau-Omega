//! Silence trimming end to end: a real FLAC with silence at both ends is scanned, and the index it builds
//! carries a version 2 details section with the skips; with trimming off the section is version 1.
use std::{fs, path::Path};
use tau_audio::flac_out;
use tau_audio::pipeline::{Pipeline, Target};
use tau_core::scan_dir;
use tau_core::trim::{self, TrimSettings};

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

fn skip_file(dir: &Path) -> Option<Vec<u8>> {
    fs::read(dir.join("Album").join("tau-skip.bin")).ok()
}

#[test]
fn a_skip_table_is_written_next_to_the_tracks_from_the_measured_silence() {
    let dir = std::env::temp_dir().join(format!("tau-trim-index-{}", std::process::id()));
    let album = dir.join("Album");
    fs::create_dir_all(&album).unwrap();
    // 8 kHz, mono: 10 s of silence, 3 s of tone, 6 s of silence; and a track with no long silence
    flac(
        &album,
        "01 Long Silences.flac",
        8_000,
        80_000,
        24_000,
        48_000,
    );
    flac(&album, "02 Plain.flac", 8_000, 400, 24_000, 400);
    let build = || {
        let scan = scan_dir(&dir, false).unwrap();
        let t = trim::active();
        if t.enabled {
            trim::write_skip_files(&dir, &scan.entries, &t).unwrap();
        }
    };

    // off (the default): no file
    trim::set_active(TrimSettings::default());
    build();
    assert!(skip_file(&dir).is_none());

    // on: one entry, the long silences minus the margins (200 ms at the start, 500 ms at the end)
    trim::set_active(TrimSettings {
        enabled: true,
        ..Default::default()
    });
    build();
    let t = skip_file(&dir).expect("a skip table");
    assert_eq!(trim::summarise(&dir), (1, 1), "one table, one marked track");
    assert_eq!(&t[..4], b"TSK1");
    assert_eq!(u16::from_le_bytes([t[4], t[5]]), 1);
    assert_eq!(
        u32::from_le_bytes(t[8..12].try_into().unwrap()),
        crc32fast::hash(&t[12..])
    );
    assert_eq!(
        u32::from_le_bytes(t[12..16].try_into().unwrap()),
        trim::name_hash("01 Long Silences.flac")
    );
    assert_eq!(
        u32::from_le_bytes(t[16..20].try_into().unwrap()),
        80_000 - 1_600
    );
    assert_eq!(
        u32::from_le_bytes(t[20..24].try_into().unwrap()),
        48_000 - 4_000
    );

    // a longer minimum than the silences: nothing to write, and the old file goes
    trim::set_active(TrimSettings {
        enabled: true,
        min_ms: 10_000,
        ..Default::default()
    });
    build();
    let t = skip_file(&dir).expect("the 10 s lead still qualifies");
    assert_eq!(
        u32::from_le_bytes(t[20..24].try_into().unwrap()),
        0,
        "the 6 s tail no longer does"
    );

    // a hand edit: typed values are written as typed, "never skip" removes the track, and the list shows both
    trim::set_active(TrimSettings {
        enabled: true,
        ..Default::default()
    });
    let key = "Album/01 Long Silences.flac".to_string();
    trim::set_overrides(
        &dir,
        [(
            key.clone(),
            trim::Override {
                lead: Some(8_000),
                ..Default::default()
            },
        )]
        .into(),
    );
    let on = trim::active();
    trim::rewrite_skip_files(&dir, &on).unwrap();
    let t = skip_file(&dir).unwrap();
    assert_eq!(
        u32::from_le_bytes(t[16..20].try_into().unwrap()),
        8_000,
        "typed lead"
    );
    assert_eq!(
        u32::from_le_bytes(t[20..24].try_into().unwrap()),
        48_000 - 4_000,
        "the detected tail stays"
    );
    let list = trim::skip_tracks(&dir, &on).unwrap();
    let row = list.iter().find(|r| r.rel == key).unwrap();
    assert_eq!(
        (row.skip.unwrap().lead_skip, row.edit.is_some()),
        (8_000, true)
    );
    trim::set_overrides(
        &dir,
        [(
            key.clone(),
            trim::Override {
                never: true,
                ..Default::default()
            },
        )]
        .into(),
    );
    trim::rewrite_skip_files(&dir, &on).unwrap();
    assert!(
        skip_file(&dir).is_none(),
        "never skip leaves nothing to write"
    );
    // a typed pair that would leave under a second of track is refused
    trim::set_overrides(
        &dir,
        [(
            key.clone(),
            trim::Override {
                lead: Some(100_000),
                ..Default::default()
            },
        )]
        .into(),
    );
    trim::rewrite_skip_files(&dir, &on).unwrap();
    let t = skip_file(&dir).unwrap();
    assert_eq!(
        u32::from_le_bytes(t[16..20].try_into().unwrap()),
        80_000 - 1_600,
        "the detected lead stands"
    );
    // the detail has the waveform and what the detector finds
    let d = trim::skip_detail(&dir, &key, &on).unwrap();
    assert_eq!(
        (d.rate, d.envelope.len(), d.auto.lead_skip),
        (8_000, 400, 80_000 - 1_600)
    );
    assert!(
        d.envelope[0] == 0 && d.envelope[240] > 100,
        "silent at the start, loud in the middle"
    );
    trim::set_overrides(&dir, Default::default());

    // an album that is gone leaves no orphan table behind
    trim::set_active(TrimSettings {
        enabled: true,
        ..Default::default()
    });
    build();
    fs::remove_file(album.join("01 Long Silences.flac")).unwrap();
    fs::remove_file(album.join("02 Plain.flac")).unwrap();
    fs::create_dir_all(dir.join("Other")).unwrap();
    flac(&dir.join("Other"), "01 x.flac", 8_000, 400, 24_000, 400);
    build();
    assert!(
        skip_file(&dir).is_none(),
        "the table of a folder with no tracks is removed"
    );
    trim::set_active(TrimSettings::default());
    fs::remove_dir_all(dir).ok();
}
