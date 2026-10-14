use tau_audio::decode::probe;
use tau_audio::loudness::{album_lufs, gain_db, measure};
use tau_audio::pipeline::{Pipeline, Target};

fn write_tone(path: &std::path::Path, amplitude: f64, seconds: u32) {
    let rate = 48_000u32;
    let mut data = Vec::new();
    for i in 0..rate * seconds {
        let v = ((2.0 * std::f64::consts::PI * 1000.0 * i as f64 / rate as f64).sin()
            * amplitude
            * 32767.0) as i16;
        data.extend(v.to_le_bytes());
        data.extend(v.to_le_bytes());
    }
    let mut out = Vec::new();
    out.extend(b"RIFF");
    out.extend((36 + data.len() as u32).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(rate.to_le_bytes());
    out.extend((rate * 4).to_le_bytes());
    out.extend(4u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(data);
    std::fs::write(path, out).unwrap();
}

#[test]
fn a_known_tone_measures_where_the_standard_says_and_the_album_is_between() {
    let dir = std::env::temp_dir().join(format!("tau-audio-loud-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (a, b) = (dir.join("a.wav"), dir.join("b.wav"));
    write_tone(&a, 0.1, 5);
    write_tone(&b, 0.5, 5);
    let open = |p: &std::path::Path| {
        let info = probe(p).unwrap();
        Pipeline::open(p, Target::native(&info)).unwrap()
    };
    let ma = measure(&mut open(&a), &mut |_| true).unwrap();
    let mb = measure(&mut open(&b), &mut |_| true).unwrap();
    // a stereo 1 kHz tone at -20 dBFS peak measures about -20 LUFS
    assert!((ma.track_lufs + 20.0).abs() < 0.5, "{}", ma.track_lufs);
    assert!((ma.peak - 0.1).abs() < 0.002, "{}", ma.peak);
    // 0.5 is 14 dB louder than 0.1
    assert!(((mb.track_lufs - ma.track_lufs) - 13.98).abs() < 0.2);
    let album = album_lufs(&[&ma, &mb]).unwrap();
    // the loud track dominates (the quiet one falls below the relative gate)
    assert!(
        album >= ma.track_lufs && album <= mb.track_lufs + 0.01,
        "{album}"
    );
    assert!(gain_db(ma.track_lufs) > gain_db(mb.track_lufs));
}
