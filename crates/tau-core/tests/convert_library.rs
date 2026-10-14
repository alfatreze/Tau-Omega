//! A source library with files the Pocket cannot play: they are listed when conversion is on and become
//! converted copies when the album is selected.
use std::{fs, path::Path};
use tau_core::convert::ConvertSettings;
use tau_core::sync::PlanOptions;
use tau_core::workbench::{list_library, list_library_with, plan_selection};

fn write_wav(path: &Path, rate: u32, frames: usize) {
    let mut data = Vec::new();
    for i in 0..frames {
        let v =
            ((2.0 * std::f64::consts::PI * 440.0 * i as f64 / rate as f64).sin() * 12000.0) as i16;
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
    fs::write(path, out).unwrap();
}

#[test]
fn convertible_files_are_listed_only_when_asked_and_planned_as_conversions() {
    let base = std::env::temp_dir().join(format!("tau-conv-lib-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let lib = base.join("lib");
    let common = base.join("card/Assets/tau/common");
    fs::create_dir_all(lib.join("Album")).unwrap();
    fs::create_dir_all(&common).unwrap();
    write_wav(&lib.join("Album/01 Song.wav"), 44_100, 44_100);

    assert!(list_library(&lib, &mut None).unwrap().tracks.is_empty());
    let listing = list_library_with(&lib, true, &mut None).unwrap();
    assert_eq!(listing.tracks.len(), 1);
    assert_eq!(listing.tracks[0].format, "WAV");
    assert_eq!(listing.albums.len(), 1);

    let options = PlanOptions {
        convert: ConvertSettings::product(),
        ..PlanOptions::default()
    };
    let p = plan_selection(
        &lib,
        &["Album".to_string()],
        &common,
        "/Assets/tau/common/",
        options.clone(),
        &mut None,
    )
    .unwrap();
    assert_eq!(p.items.len(), 1);
    assert!(p.items[0].destination.ends_with("Album/01 Song.flac"));
    // without conversion the same selection has nothing to copy
    let plain = plan_selection(
        &lib,
        &["Album".to_string()],
        &common,
        "/Assets/tau/common/",
        PlanOptions::default(),
        &mut None,
    );
    assert!(plain.is_err() || plain.unwrap().items.is_empty());
    fs::remove_dir_all(&base).unwrap();
}
