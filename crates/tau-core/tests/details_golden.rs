//! The track details read from real-format files must equal what tau-alpha's reference extractor
//! (`tools/tau_details.py`) reads from the same bytes (`testdata/gen_details.py` makes both).
use serde_json::Value;
use std::{fs, path::PathBuf};
use tau_core::ROOT_PREFIX;
use tau_core::details::{F_SIDECAR, extract};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/details/common/Details")
}

#[test]
fn details_equal_the_reference_extractor() {
    let expected: Value =
        serde_json::from_slice(&fs::read(dir().join("../../expected.json")).unwrap()).unwrap();
    let mut checked = 0;
    for (name, want) in expected.as_object().unwrap() {
        let path = if name == "mp3_peak_overflow.mp3" {
            dir().join("../../extra").join(name)
        } else {
            dir().join(name)
        };
        let got = extract(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        let n = |k: &str| want[k].as_i64().unwrap();
        assert_eq!(got.codec as i64, n("codec"), "{name} codec");
        assert_eq!(got.channels as i64, n("ch"), "{name} channels");
        assert_eq!(got.bits as i64, n("bits"), "{name} bits");
        assert_eq!((got.flags & !F_SIDECAR) as i64, n("flags"), "{name} flags");
        assert_eq!(got.rate as i64, n("rate"), "{name} rate");
        assert_eq!(got.size as i64, n("size"), "{name} size");
        assert_eq!(got.kbps as i64, n("kbps"), "{name} kbps");
        assert_eq!(got.cover_kib as i64, n("cover_kib"), "{name} cover");
        assert_eq!(got.gain_track as i64, n("tg"), "{name} track gain");
        assert_eq!(got.gain_album as i64, n("ag"), "{name} album gain");
        assert_eq!(got.peak_track as i64, n("tp"), "{name} track peak");
        assert_eq!(got.peak_album as i64, n("ap"), "{name} album peak");
        assert_eq!(got.delay as i64, n("delay"), "{name} delay");
        assert_eq!(got.padding as i64, n("pad"), "{name} padding");
        assert_eq!(got.encoder, want["enc"].as_str().unwrap(), "{name} encoder");
        checked += 1;
    }
    assert_eq!(checked, 18);
}

#[test]
fn the_index_with_details_is_byte_identical_to_the_reference() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/details");
    let scan = tau_core::scan_dir(&root.join("common"), false).unwrap();
    let got = tau_core::build_index(&scan.entries, &scan.playlists, ROOT_PREFIX, &mut Vec::new())
        .unwrap();
    assert_eq!(got, fs::read(root.join("index.tdb")).unwrap());
}
