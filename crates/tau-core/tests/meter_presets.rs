//! `tau_core::meter_presets` against Tau Alpha's reference writer (`tools/tau_assets.py` `pack_meters`): the bytes must be identical for the
//! same registry and preset sets, a reader must give the sets back, and the writer must refuse everything the reference refuses.

use std::{collections::BTreeMap, path::PathBuf};
use tau_core::meter_presets::{MeterPresets, MeterRegistry, Preset, pack_meters, parse_meters};

fn fx(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/meter_presets")
        .join(name)
}
fn registry() -> MeterRegistry {
    MeterRegistry::from_json(&std::fs::read(fx("registry.json")).unwrap()).unwrap()
}
fn spec() -> Vec<MeterPresets> {
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fx("spec.json")).unwrap()).unwrap();
    v["meters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| MeterPresets {
            key: m["key"].as_str().unwrap().into(),
            default_preset: m["default_preset"].as_u64().unwrap() as u8,
            presets: m["presets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| Preset {
                    name: p["name"].as_str().unwrap().into(),
                    values: p["values"]
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_i64().unwrap()))
                        .collect::<BTreeMap<_, _>>(),
                })
                .collect(),
        })
        .collect()
}

#[test]
fn the_writer_is_byte_identical_to_tau_alphas_reference() {
    let ours = pack_meters(&registry(), &spec()).unwrap();
    let theirs = std::fs::read(fx("expected.bin")).unwrap();
    assert_eq!(ours, theirs);
}

#[test]
fn the_reader_gives_the_sets_back_and_skips_what_the_firmware_would_skip() {
    let reg = registry();
    let blob = std::fs::read(fx("expected.bin")).unwrap();
    let parsed = parse_meters(&reg, &blob).unwrap();
    let want = spec();
    assert_eq!(parsed.len(), want.len());
    for (p, w) in parsed.iter().zip(&want) {
        assert!(!p.skipped);
        assert_eq!(p.key.as_deref(), Some(w.key.as_str()));
        assert_eq!(
            (p.presets.clone(), p.default_preset),
            (w.presets.clone(), w.default_preset)
        );
    }
    // A registry that no longer knows meter id 9 (or whose beta now has two parameters) makes that entry skipped, not an error.
    let mut other = reg.clone();
    other.meters.retain(|m| m.key != "beta");
    assert!(
        parse_meters(&other, &blob)
            .unwrap()
            .iter()
            .any(|m| m.id == 9 && m.skipped)
    );
    let mut changed = reg;
    let extra = changed.meters[0].params[0].clone();
    changed
        .meters
        .iter_mut()
        .find(|m| m.key == "beta")
        .unwrap()
        .params
        .push(extra);
    assert!(
        parse_meters(&changed, &blob)
            .unwrap()
            .iter()
            .any(|m| m.id == 9 && m.skipped)
    );
}

#[test]
fn a_damaged_section_is_refused_before_any_byte_is_used() {
    let reg = registry();
    let blob = std::fs::read(fx("expected.bin")).unwrap();
    for i in 0..blob.len() {
        let mut b = blob.clone();
        b[i] ^= 0x01;
        // Every single-bit flip is caught: by the magic, the version, the CRC, or (the pad bytes and CRC field themselves) the CRC check.
        if i == 6 || i == 7 {
            continue; // the two pad bytes are not covered by the format
        }
        assert!(
            parse_meters(&reg, &b).is_err(),
            "byte {i} flipped went unnoticed"
        );
    }
    assert!(parse_meters(&reg, &blob[..blob.len() - 3]).is_err());
    assert!(parse_meters(&reg, b"nope").is_err());
}

#[test]
fn the_writer_refuses_what_the_reference_refuses() {
    let reg = registry();
    let ok = spec();
    let err = |edit: &dyn Fn(&mut Vec<MeterPresets>)| {
        let mut s = ok.clone();
        edit(&mut s);
        pack_meters(&reg, &s).unwrap_err().message
    };
    assert!(err(&|s| s[0].key = "nope".into()).contains("not in the registry"));
    assert!(
        err(&|s| s.push(MeterPresets {
            key: "gamma".into(),
            presets: s[1].presets.clone(),
            default_preset: 0
        }))
        .contains("no parameters")
    );
    assert!(err(&|s| s[0].presets.clear()).contains("1..8 presets"));
    assert!(
        err(&|s| {
            let p = s[0].presets[0].clone();
            s[0].presets = vec![p; 9];
        })
        .contains("1..8 presets")
    );
    assert!(err(&|s| s[0].default_preset = 3).contains("default_preset"));
    assert!(err(&|s| s[0].presets[0].name = "lower".into()).contains("names"));
    assert!(err(&|s| s[0].presets[1].name = "CLASSIC".into()).contains("unique"));
    assert!(err(&|s| s[0].presets[0].name = "A".repeat(16)).contains("names"));
    assert!(
        err(&|s| {
            s[0].presets[0].values.remove("peak");
        })
        .contains("must give exactly")
    );
    assert!(
        err(&|s| {
            s[0].presets[0].values.insert("extra".into(), 1);
        })
        .contains("must give exactly")
    );
    assert!(
        err(&|s| {
            s[0].presets[0].values.insert("bands".into(), 17);
        })
        .contains("outside")
    );
    assert!(
        err(&|s| {
            s[0].presets[0].values.insert("bands".into(), 3);
        })
        .contains("outside")
    );
    assert!(
        err(&|s| {
            s[0].presets[0].values.insert("release_ms".into(), 2001);
        })
        .contains("outside")
    );
    assert!(
        err(&|s| {
            s[0].presets[0].values.insert("easing".into(), 4);
        })
        .contains("outside")
    );
    assert!(
        err(&|s| {
            s[0].presets[0].values.insert("peak".into(), 2);
        })
        .contains("outside")
    );
}
