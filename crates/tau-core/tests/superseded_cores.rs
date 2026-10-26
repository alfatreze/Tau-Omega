//! Which cores an install offers to remove as superseded: the manifest's `replaces` when it lists them, otherwise the known
//! supersessions from v0.6.0 (the v0.6.0 and v0.6.1 manifests carry `replaces: []`), and never a core that is newer than the package.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tau_core::install_plan;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

fn scratch(name: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!(
        "tau-super-{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(&d).unwrap();
    d
}

fn core_json(id: &str, version: &str) -> String {
    let short = id.split_once('.').map_or(id, |(_, n)| n);
    format!(
        r#"{{"core":{{"magic":"APF_VER_1","metadata":{{"platform_ids":["tau"],"shortname":"{short}","author":"alfatreze","description":"x","version":"{version}","date_release":"2026-10-21"}},"framework":{{"target_product":"Analogue Pocket","version_required":"1.1","dock":{{"supported":true}}}},"cores":[{{"name":"default","id":0,"filename":"bitstream.rbf_r"}}]}}}}"#
    )
}

fn zip(dir: &Path, id: &str, version: &str) -> PathBuf {
    let path = dir.join(format!("{}_{version}.zip", id.replace(' ', "_")));
    let mut w = ZipWriter::new(fs::File::create(&path).unwrap());
    let o = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in [
        (format!("Cores/{id}/core.json"), core_json(id, version).into_bytes()),
        (format!("Cores/{id}/bitstream.rbf_r"), vec![7u8; 2048]),
        ("Platforms/tau.json".to_string(), br#"{"platform":{"category":"Media Players","name":"TAU","manufacturer":"alfatreze","year":2026}}"#.to_vec()),
    ] {
        w.start_file(name, o).unwrap();
        w.write_all(&bytes).unwrap();
    }
    w.finish().unwrap();
    path
}

fn put_core(card: &Path, id: &str, version: &str) {
    let d = card.join("Cores").join(id);
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("core.json"), core_json(id, version)).unwrap();
}

fn candidates(zip: &Path, card: &Path) -> Vec<String> {
    install_plan::plan(zip, card, &[], true)
        .unwrap()
        .superseded_candidates
}

fn card_with_old_tau_cores() -> PathBuf {
    let card = scratch("card");
    fs::create_dir_all(card.join("Assets")).unwrap();
    for (id, v) in [
        ("alfatreze.TAU", "0.6.0"),
        ("alfatreze.TAU_DIAGNOSTIC", "0.5.0"),
        ("alfatreze.TAU Diagnostics", "0.6.0-alpha.3"),
        ("alfatreze.TAU Preview", "0.6.0-preview.2"),
        ("alfatreze.TAU Preview Diagnostics", "0.6.0-preview.1"),
        ("agg23.NES", "1.0.0"),
    ] {
        put_core(&card, id, v);
    }
    card
}

#[test]
fn a_stable_tau_offers_the_diagnostics_and_preview_cores_without_a_manifest_saying_so() {
    let (d, card) = (scratch("zips"), card_with_old_tau_cores());
    let got = candidates(&zip(&d, "alfatreze.TAU", "0.6.1"), &card);
    assert_eq!(
        got,
        [
            "alfatreze.TAU Diagnostics",
            "alfatreze.TAU Preview",
            "alfatreze.TAU Preview Diagnostics",
            "alfatreze.TAU_DIAGNOSTIC"
        ],
        "the Stable core itself and other authors' cores are never offered"
    );
}

#[test]
fn a_preview_ahead_of_the_stable_is_not_obsolete() {
    let (d, card) = (scratch("zips"), card_with_old_tau_cores());
    put_core(&card, "alfatreze.TAU Preview", "0.7.0-preview.1"); // newer than the 0.6.1 Stable being installed
    let got = candidates(&zip(&d, "alfatreze.TAU", "0.6.1"), &card);
    assert!(
        !got.contains(&"alfatreze.TAU Preview".to_string()),
        "{got:?}"
    );
    assert!(got.contains(&"alfatreze.TAU Diagnostics".to_string()));
}

#[test]
fn a_preview_replaces_only_its_own_diagnostics_core_never_the_stable() {
    let (d, card) = (scratch("zips"), card_with_old_tau_cores());
    let got = candidates(&zip(&d, "alfatreze.TAU Preview", "0.6.0-preview.2"), &card);
    assert_eq!(got, ["alfatreze.TAU Preview Diagnostics"], "{got:?}");
    // Before the Diagnostics core was dropped (preview.1 still shipped it) nothing is assumed.
    let got = candidates(&zip(&d, "alfatreze.TAU Preview", "0.6.0-preview.1"), &card);
    assert!(got.is_empty(), "{got:?}");
}

#[test]
fn another_cores_install_offers_nothing_tau_related() {
    let (d, card) = (scratch("zips"), card_with_old_tau_cores());
    assert!(candidates(&zip(&d, "agg23.NES", "1.1.0"), &card).is_empty());
}

#[test]
fn from_v0_7_0_the_diagnostics_core_is_optional_and_never_offered_for_removal_by_a_fallback() {
    let (d, card) = (scratch("zips"), card_with_old_tau_cores());
    for v in ["0.7.0", "0.7.0-preview.1", "0.7.1"] {
        let got = candidates(&zip(&d, "alfatreze.TAU", v), &card);
        assert!(
            !got.contains(&"alfatreze.TAU Diagnostics".to_string()),
            "{v}: {got:?}"
        );
        assert!(
            !got.contains(&"alfatreze.TAU Preview Diagnostics".to_string()),
            "{v}: {got:?}"
        );
    }
    // The last release before v0.7.0 still uses the fallback.
    assert!(
        candidates(&zip(&d, "alfatreze.TAU", "0.6.9"), &card)
            .contains(&"alfatreze.TAU Diagnostics".to_string())
    );
}
