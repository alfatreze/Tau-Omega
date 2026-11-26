//! End-to-end check of the two updated SDK-only Lissajous meter packages in `../Tau alpha meters/` (skipped when absent, so not in CI):
//! 1. `LISSAJOUS` v1.1.0 (`a62829eea3754c7e97e5c084a46ed09d.tmeter` / `Lissajous-for-Tau-Alpha.zip`)
//! 2. `LISSAJOUS SHIFT` v1.0.0 (`09c21e82b7b34bdb8034c72687c4bc4b.tmeter` / `Lissajous-Shift-for-Tau-Alpha.zip`)
//!
//! Verifies that both packages built against the public SDK alone pass Omega's `.tmeter` validator, compose cleanly with the 7 shipped meters
//! into a 9-row `tau-meters.bin` that passes `parse`, `boot_check`, `pack_image_ok`, `availability`, and `tuning_collisions`, and seed all
//! factory preset files cleanly on a scratch v0.7.5 card.

use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use tau_core::{
    meter_bin::{self, Availability},
    meter_install::{self, MeterAction},
    meter_package, preset_files,
};

fn meters_root() -> Option<PathBuf> {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../Tau alpha meters");
    (d.join("Lissajous-for-Tau-Alpha.zip").is_file()
        && d.join("Lissajous-Shift-for-Tau-Alpha.zip").is_file())
    .then_some(d)
}

fn read_tmeter_from_zip(zip_path: &Path, inner: &str) -> Vec<u8> {
    let bytes = fs::read(zip_path).unwrap();
    let mut za = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = Vec::new();
    za.by_name(inner).unwrap().read_to_end(&mut out).unwrap();
    out
}

fn scratch(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("tau-sdk-lissajous-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn put(card: &Path, rel: &str, bytes: &[u8]) {
    let p = card.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

#[test]
fn both_sdk_built_lissajous_variants_are_sound_and_install_cleanly() {
    let Some(root) = meters_root() else { return };

    let liss_zip = read_tmeter_from_zip(
        &root.join("Lissajous-for-Tau-Alpha.zip"),
        "lissajous/dist/a62829eea3754c7e97e5c084a46ed09d.tmeter",
    );
    let shift_zip = read_tmeter_from_zip(
        &root.join("Lissajous-Shift-for-Tau-Alpha.zip"),
        "lissajous-shift/dist/09c21e82b7b34bdb8034c72687c4bc4b.tmeter",
    );

    // Check extracted zips match the dist/ copies when present
    if let Ok(dist) = fs::read(root.join("lissajous/dist/a62829eea3754c7e97e5c084a46ed09d.tmeter"))
    {
        assert_eq!(liss_zip, dist);
    }
    if let Ok(dist) =
        fs::read(root.join("lissajous-shift/dist/09c21e82b7b34bdb8034c72687c4bc4b.tmeter"))
    {
        assert_eq!(shift_zip, dist);
    }

    // 1. Both packages pass Omega's strict .tmeter validator
    let p1 = meter_package::parse(&liss_zip).expect("LISSAJOUS .tmeter must be sound");
    assert_eq!(
        (
            p1.summary.id.as_str(),
            p1.summary.name.as_str(),
            p1.summary.version.as_str(),
            p1.summary.params_rev,
            p1.summary.preset_count,
            p1.summary.binary_only
        ),
        (
            "a62829eea3754c7e97e5c084a46ed09d",
            "LISSAJOUS",
            "1.1.0",
            2,
            10,
            false
        )
    );

    let p2 = meter_package::parse(&shift_zip).expect("LISSAJOUS SHIFT .tmeter must be sound");
    assert_eq!(
        (
            p2.summary.id.as_str(),
            p2.summary.name.as_str(),
            p2.summary.version.as_str(),
            p2.summary.params_rev,
            p2.summary.preset_count,
            p2.summary.binary_only
        ),
        (
            "09c21e82b7b34bdb8034c72687c4bc4b",
            "LISSAJOUS SHIFT",
            "1.0.0",
            1,
            8,
            false
        )
    );

    // 2. Install both variants onto a scratch v0.7.5 card
    let card = scratch("card");
    let backup = scratch("backup");
    put(
        &card,
        "Cores/alfatreze.TAU/core.json",
        br#"{"core":{"metadata":{"version":"0.7.5","platform_ids":["tau"]}}}"#,
    );
    put(
        &card,
        "Cores/alfatreze.TAU/data.json",
        br#"{"data":{"data_slots":[{"filename":"tau-meters.bin"},{"filename":"tau-library.tdb"}]}}"#,
    );
    let shipped = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/meter_bin/v2_seven.bin"),
    )
    .unwrap();
    put(&card, "Assets/tau/alfatreze.TAU/tau-packs.bin", &shipped);

    let authors = BTreeMap::new();
    for (bytes, expected_presets) in [(&liss_zip, 11usize), (&shift_zip, 9usize)] {
        let act = MeterAction::InstallPackage {
            package_bytes: bytes.clone(),
            allow_source_change: false,
            overwrite_presets: false,
        };
        let plan = meter_install::plan(&card, &act, &authors).unwrap();
        assert_eq!(plan.action_kind, "install");
        assert!(
            plan.warnings.is_empty(),
            "unexpected warnings: {:?}",
            plan.warnings
        );
        let rep = meter_install::execute(&card, &act, &authors, &plan.id, &backup).unwrap();
        assert_eq!(rep.presets_written, expected_presets);
    }

    // 3. Verify the resulting 9-row tau-meters.bin and both preset folders
    let bin = fs::read(card.join("Assets/tau/common/tau-meters.bin")).unwrap();
    let parsed = meter_bin::parse(&bin).unwrap();
    assert_eq!(parsed.rows.len(), 9);
    assert!(meter_bin::boot_check(&bin, bin.len()).is_ok());
    assert!(parsed.rows[0].fallback());
    assert_eq!(parsed.rows[7].name, "LISSAJOUS");
    assert_eq!(parsed.rows[8].name, "LISSAJOUS SHIFT");
    assert!(
        parsed
            .rows
            .iter()
            .all(|r| meter_bin::pack_image_ok(&bin, r) && r.preview(&bin).is_some())
    );
    assert!(
        meter_bin::tuning_collisions(&parsed.rows)
            .iter()
            .all(|&c| !c),
        "no tuning collisions across all 9 meters"
    );
    assert!(
        parsed
            .rows
            .iter()
            .all(|r| meter_bin::availability(r, 0x0102, 0x0007) == Availability::Available)
    );

    for (hex, mid, count) in [
        ("a62829ee", 0x29eeu16, 10usize),
        ("09c21e82", 0x1e82u16, 8usize),
    ] {
        let dir = card.join("Assets/tau/common/tau-presets").join(hex);
        let (idx_mid, entries) =
            preset_files::parse_index(&fs::read(dir.join("index.tpi")).unwrap()).unwrap();
        assert_eq!((idx_mid, entries.len()), (mid, count));
        for e in &entries {
            let p = preset_files::parse_preset(
                &fs::read(dir.join(preset_files::preset_file_name(e.id))).unwrap(),
            )
            .unwrap();
            assert_eq!(
                (p.meter_id, p.id, p.name.as_str()),
                (mid, e.id, e.name.as_str())
            );
        }
    }
}
