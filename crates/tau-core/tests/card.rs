//! Card inspection against the real Analogue Platform Framework `data.json`
//! layout.
//!
//! Regression cover for a bug that shipped: library detection read `data` as an
//! array, which no real core uses, so every genuine Tau card reported as
//! "legacy" and the whole media-library feature stayed switched off. The unit
//! fixture had invented the flat shape, so the tests agreed with the code and
//! both were wrong. The literals below are copied from a real shipped core —
//! keep them that way.

use std::{fs, path::PathBuf};

fn card_with(data_json: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "tau-omega-card-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("Cores/alfatreze.TAU")).unwrap();
    fs::create_dir_all(root.join("Assets/tau/common")).unwrap();
    fs::write(
        root.join("Cores/alfatreze.TAU/core.json"),
        r#"{"core":{"metadata":{"author":"alfatreze","shortname":"TAU","version":"0.4.0","platform_ids":["tau"]}}}"#,
    )
    .unwrap();
    fs::write(root.join("Cores/alfatreze.TAU/data.json"), data_json).unwrap();
    root
}

/// The exact slot table the shipped v0.4.0 Tau core declares.
const REAL_DATA_JSON: &str = r#"{"data":{"magic":"APF_VER_1","data_slots":[
    {"name":"Firmware","id":1,"required":true,"parameters":"0x5","filename":"tau.rom"},
    {"name":"Music file","id":2,"required":false,"parameters":"0x2"},
    {"name":"Playlist","id":3,"required":false,"parameters":"0x2","filename":"playlist.m3u"},
    {"name":"Loading image","id":4,"required":false,"deferload":true,"parameters":"0x1","filename":"tau-loading.bin"},
    {"name":"Media library index","id":5,"required":false,"deferload":true,"parameters":"0x1","filename":"tau-library.tdb"},
    {"name":"Cold image","id":6,"required":false,"deferload":true,"parameters":"0x1","filename":"tau-cold.bin"}
]}}"#;

#[test]
fn a_real_shipped_core_is_recognised_as_library_capable() {
    let root = card_with(REAL_DATA_JSON);
    let card = tau_core::inspect_card(&root).unwrap();
    assert!(card.is_pocket_card);
    assert_eq!(card.cores.len(), 1);
    assert!(
        card.cores[0].library_capable,
        "the shipped core declares tau-library.tdb in data.data_slots and must be detected"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_core_without_the_library_slot_is_not_library_capable() {
    let root = card_with(
        r#"{"data":{"magic":"APF_VER_1","data_slots":[
            {"name":"Music file","id":2,"required":false,"parameters":"0x2","filename":"track.mp3"}
        ]}}"#,
    );
    let card = tau_core::inspect_card(&root).unwrap();
    assert!(!card.cores[0].library_capable);
    fs::remove_dir_all(root).unwrap();
}

/// Detection keys on the filename, not the slot id: the id is the core author's
/// choice, and a second core family may serve the index from a different slot.
#[test]
fn detection_follows_the_filename_not_the_slot_id() {
    let root = card_with(
        r#"{"data":{"magic":"APF_VER_1","data_slots":[
            {"name":"Media library index","id":9,"required":false,"filename":"tau-library.tdb"}
        ]}}"#,
    );
    let card = tau_core::inspect_card(&root).unwrap();
    assert!(card.cores[0].library_capable);
    fs::remove_dir_all(root).unwrap();
}

/// Older hand-written cores use a flat slot array; keep accepting it.
#[test]
fn the_flat_legacy_shape_is_still_accepted() {
    let root = card_with(r#"{"data":[{"id":5,"filename":"tau-library.tdb"}]}"#);
    let card = tau_core::inspect_card(&root).unwrap();
    assert!(card.cores[0].library_capable);
    fs::remove_dir_all(root).unwrap();
}

/// `Platforms/<platform>.json`'s own `category` field (the exact shape the
/// real shipped `tau.json` declares) is the general, author-independent
/// signal for "is this a media player core" -- verified end to end through
/// `inspect_card`, not just a private helper.
#[test]
fn reads_the_platform_category_from_the_real_shipped_shape() {
    let root = card_with(REAL_DATA_JSON);
    fs::create_dir_all(root.join("Platforms")).unwrap();
    fs::write(
        root.join("Platforms/tau.json"),
        r#"{"platform":{"category":"Media Players","name":"TAU","year":2026,"manufacturer":"alfatreze"}}"#,
    )
    .unwrap();
    let card = tau_core::inspect_card(&root).unwrap();
    assert_eq!(
        card.cores[0].platform_category.as_deref(),
        Some("Media Players")
    );
    fs::remove_dir_all(root).unwrap();
}

/// A card with no `Platforms/<platform>.json` at all reports `None`, not an
/// error -- this is display metadata, not something `inspect_card` should
/// fail over.
#[test]
fn missing_platform_json_reports_no_category_rather_than_failing() {
    let root = card_with(REAL_DATA_JSON);
    let card = tau_core::inspect_card(&root).unwrap();
    assert_eq!(card.cores[0].platform_category, None);
    fs::remove_dir_all(root).unwrap();
}
