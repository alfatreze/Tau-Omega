//! Tau Alpha's local test meter, LISSAJOUS LOCAL (message 034, A034/2): read **in place** from their git-ignored `work/omega-test-meters/` on this
//! Mac (skipped when absent). It checks what Omega can check today without a composer: the 8-row user bin, the package (`.tmeter`) against its own
//! pins and against the pack inside the bin, and the 10 factory preset files. Omega composes nothing and copies nothing here.

use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::PathBuf};
use tau_core::{meter_bin, preset_files};

const ID: &str = "30245146a183cbd63e059742fd3af3f6";

fn dir() -> Option<PathBuf> {
    let d =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tau-alpha/work/omega-test-meters");
    d.join("card-stage/Assets/tau/common/tau-meters.bin")
        .is_file()
        .then_some(d)
}
fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}

#[test]
fn the_user_bin_with_the_test_meter_reads_as_message_034_says() {
    let Some(d) = dir() else { return };
    let bin = fs::read(d.join("card-stage/Assets/tau/common/tau-meters.bin")).unwrap();
    assert_eq!(
        (bin.len(), sha(&bin).as_str()),
        (
            67_124,
            "f342ccea2d88a86b3ea9d198296b24c9be8146e662bd7901f9bb50a2c5590ff2"
        )
    );
    let parsed = meter_bin::parse(&bin).unwrap();
    assert_eq!((parsed.version, parsed.rows.len()), (2, 8));
    let names: Vec<&str> = parsed.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "WINAMP BARS",
            "WINAMP SCOPE",
            "CHLADNI",
            "LAYERED WAVE",
            "VU MASTER",
            "WATERFALL",
            "SOUND MAP",
            "LISSAJOUS LOCAL"
        ]
    );
    assert!(
        parsed.rows[0].fallback() && parsed.rows.iter().skip(1).all(|r| !r.fallback()),
        "Winamp Bars first and flagged as the fallback"
    );
    assert!(
        parsed.rows.iter().all(|r| r.params_rev == 1 && r.visible()),
        "every row params_rev 1"
    );
    let last = parsed.rows.last().unwrap();
    assert_eq!(
        (last.id.as_str(), last.id_hash, last.binary_only()),
        (ID, 0x3024_5146, false),
        "it has source, so it is not binary-only"
    );
    // the seven shipped meters in it are the ones Omega knows by id
    for (i, row) in parsed.rows.iter().take(7).enumerate() {
        assert_eq!(
            row.id_hash,
            tau_core::persist_layout::SHIPPED_METERS[i].0,
            "{}",
            row.name
        );
    }
    assert!(
        meter_bin::boot_check(&bin, bin.len()).is_ok(),
        "the Pocket will use this file"
    );
    assert!(
        parsed
            .rows
            .iter()
            .all(|r| meter_bin::pack_image_ok(&bin, r) && r.preview(&bin).is_some())
    );
    assert!(
        meter_bin::tuning_collisions(&parsed.rows)
            .iter()
            .all(|c| !c),
        "no two meters share their low 16 id bits"
    );
    assert_eq!(
        parsed
            .rows
            .iter()
            .map(|r| meter_bin::availability(r, 0x0102, 0x0007))
            .filter(|a| *a != meter_bin::Availability::Available)
            .count(),
        0,
        "all eight run on a TAU 0.7 firmware at ABI 1.2"
    );
}

#[test]
fn the_package_matches_its_own_pins_and_the_pack_inside_the_bin() {
    let Some(d) = dir() else { return };
    let bytes = fs::read(d.join(format!("dist/{ID}.tmeter"))).unwrap();
    assert_eq!(
        sha(&bytes),
        "2aba1db56947a7667bef4f3a4a2ebf71195d7d90a42a78854ebe8f58444a39d9"
    );
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut read = |name: &str| -> Vec<u8> {
        let mut out = Vec::new();
        zip.by_name(name)
            .unwrap_or_else(|_| panic!("{name} is in the package"))
            .read_to_end(&mut out)
            .unwrap();
        out
    };
    let manifest: serde_json::Value = serde_json::from_slice(&read("meter.json")).unwrap();
    assert_eq!(
        (
            manifest["id"].as_str(),
            manifest["name"].as_str(),
            manifest["author"].as_str()
        ),
        (Some(ID), Some("LISSAJOUS LOCAL"), Some("Local test"))
    );
    let pack = read("meter.bin");
    assert_eq!(
        manifest["files"]["meter.bin"].as_str(),
        Some(sha(&pack).as_str()),
        "meter.bin matches its pin"
    );
    // the pack in the package is exactly the one composed into the bin
    let bin = fs::read(d.join("card-stage/Assets/tau/common/tau-meters.bin")).unwrap();
    let row = meter_bin::parse(&bin).unwrap().rows.pop().unwrap();
    assert_eq!(
        &bin[row.pack_off as usize..(row.pack_off + row.pack_size) as usize],
        pack.as_slice()
    );
    assert_eq!(
        u32::from(manifest["params_rev"].as_u64().unwrap() as u16),
        u32::from(row.params_rev)
    );
}

#[test]
fn the_ten_factory_presets_read_as_omegas_own_preset_code_reads_them() {
    let Some(d) = dir() else { return };
    let folder = d.join("card-stage/Assets/tau/common/tau-presets/30245146");
    let (meter_id, entries) =
        preset_files::parse_index(&fs::read(folder.join("index.tpi")).unwrap()).unwrap();
    assert_eq!(
        meter_id, 0x5146,
        "the meter id inside the files is the low 16 bits of the id hash (informational)"
    );
    assert_eq!(entries.len(), 10);
    assert_eq!(
        entries.iter().map(|e| e.id).collect::<Vec<_>>(),
        (0..10).collect::<Vec<u8>>()
    );
    for e in &entries {
        let p = preset_files::parse_preset(
            &fs::read(folder.join(preset_files::preset_file_name(e.id))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            (p.meter_id, p.id, p.name.as_str()),
            (0x5146, e.id, e.name.as_str()),
            "{}",
            e.name
        );
    }
}
