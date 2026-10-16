//! An MP3 with a LAME tag is measured from the first sample of the music, like the player counts it: the encoder
//! delay and padding are already removed by the decoder, so the lead of a file made from N frames of silence is N.
//! Skipped when no `lame` is installed.
use std::process::Command;
use tau_audio::silence::measure_edges;

#[test]
fn an_mp3_lead_is_counted_after_the_encoder_delay() {
    let Ok(lame) = Command::new("lame").arg("--version").output() else {
        eprintln!("no lame installed: skipped");
        return;
    };
    if !lame.status.success() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("tau-mp3sil-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rate = 44_100u32;
    let mut pcm: Vec<i16> = vec![0; 5 * rate as usize];
    pcm.extend((1..3 * rate as usize).map(|i| ((i as f64 * 0.05).sin() * 12_000.0) as i16));
    pcm.extend(vec![0i16; 4 * rate as usize]);
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
    let (wav, mp3) = (dir.join("a.wav"), dir.join("a.mp3"));
    std::fs::write(&wav, w).unwrap();
    let ok = Command::new("lame")
        .args(["-S", "-b", "128"])
        .arg(&wav)
        .arg(&mp3)
        .status()
        .unwrap();
    assert!(ok.success());
    let e = measure_edges(&mp3, -60.0, &mut |_| true).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        e.frames,
        pcm.len() as u64,
        "the decoder trims the encoder delay and padding"
    );
    assert_eq!(e.lead, 5 * u64::from(rate), "the lead is exact");
    // the tail starts in the encoder's ringing after the last sound, within one 10 ms window
    assert!(
        e.tail <= 4 * u64::from(rate) && e.tail + 450 >= 4 * u64::from(rate),
        "tail {}",
        e.tail
    );
}
