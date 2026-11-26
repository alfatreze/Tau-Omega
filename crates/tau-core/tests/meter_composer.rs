//! Tests for `meter_bin::compose`, `meter_package::parse`, and `meter_install::{plan, execute}`.
//! Uses only committed fixtures in `testdata/meter_bin/`, so every test runs in CI on all platforms.

use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};
use tau_core::{
    meter_bin::{self, ComposeEntry, FLAG_FALLBACK, FLAG_VISIBLE, Refusal},
    meter_install::{self, MeterAction, RowEdit},
    meter_package, preset_files,
};

fn scratch(tag: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let p = std::env::temp_dir().join(format!(
        "tau-mc-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}

fn v2_seven() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/meter_bin/v2_seven.bin"),
    )
    .unwrap()
}

fn put(card: &Path, rel: &str, bytes: &[u8]) {
    let p = card.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

fn setup_card(card: &Path, version: &str, reads_user_file: bool, ship_v2: bool) {
    put(
        card,
        "Cores/alfatreze.TAU/core.json",
        format!(r#"{{"core":{{"metadata":{{"version":"{version}","platform_ids":["tau"]}}}}}}"#)
            .as_bytes(),
    );
    let slots = if reads_user_file {
        r#"{"filename":"tau-meters.bin"},{"filename":"tau-library.tdb"}"#
    } else {
        r#"{"filename":"tau-library.tdb"}"#
    };
    put(
        card,
        "Cores/alfatreze.TAU/data.json",
        format!(r#"{{"data":{{"data_slots":[{slots}]}}}}"#).as_bytes(),
    );
    if ship_v2 {
        put(card, "Assets/tau/alfatreze.TAU/tau-packs.bin", &v2_seven());
    }
}

/// Builds a synthetic `.tmeter` zip from a real valid `TMPK` pack (taken from `v2_seven.bin`).
#[allow(clippy::too_many_arguments)]
fn make_tmeter(
    id: &str,
    name: &str,
    author: &str,
    pack: &[u8],
    include_source: bool,
    include_presets: bool,
    extra_files: &[(&str, &[u8])],
    tamper_pin: bool,
) -> Vec<u8> {
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    files.insert("meter.bin".into(), pack.to_vec());
    files.insert("LICENSE".into(), b"MIT License\n".to_vec());
    files.insert("README.md".into(), b"# Test Meter\n".to_vec());
    if include_source {
        files.insert("source/meter.c".into(), b"// source\n".to_vec());
    }
    if include_presets {
        let mid = u16::from_str_radix(&id[4..8], 16).unwrap();
        let p0 = preset_files::build_preset(mid, 0, "Default", &[], &[]).unwrap();
        let idx = preset_files::build_index(
            mid,
            &[preset_files::IndexEntry {
                id: 0,
                name: "Default".into(),
                hidden: false,
            }],
        )
        .unwrap();
        files.insert("presets/index.tpi".into(), idx);
        files.insert("presets/p0.tpr".into(), p0);
    }
    for (k, v) in extra_files {
        files.insert((*k).to_string(), (*v).to_vec());
    }

    let abi_raw = u16::from_le_bytes([pack[4], pack[5]]);
    let abi_str = format!("{}.{}", abi_raw >> 8, abi_raw & 0xFF);

    let mut pins = serde_json::Map::new();
    for (k, v) in &files {
        let mut h = sha(v);
        if tamper_pin && k == "meter.bin" {
            h = "0".repeat(64);
        }
        pins.insert(k.clone(), serde_json::Value::String(h));
    }
    let manifest = serde_json::json!({
        "schema": 1,
        "id": id,
        "name": name,
        "version": "1.2.3",
        "author": author,
        "licence": "MIT",
        "homepage": "",
        "abi": abi_str,
        "tau_min": "0.7",
        "params_rev": 1,
        "files": pins,
        "signature": null
    });

    let mut cur = Cursor::new(Vec::new());
    {
        let mut zw = zip::ZipWriter::new(&mut cur);
        let opts = zip::write::SimpleFileOptions::default();
        zw.start_file("meter.json", opts).unwrap();
        zw.write_all(serde_json::to_vec_pretty(&manifest).unwrap().as_slice())
            .unwrap();
        for (k, v) in &files {
            zw.start_file(k, opts).unwrap();
            zw.write_all(v).unwrap();
        }
        zw.finish().unwrap();
    }
    cur.into_inner()
}

#[test]
fn compose_round_trip_matches_v2_seven_byte_for_byte() {
    let orig = v2_seven();
    let entries = meter_bin::extract_entries(&orig).unwrap();
    assert_eq!(entries.len(), 7);
    let recomposed = meter_bin::compose(&entries).unwrap();
    assert_eq!(recomposed, orig, "recomposed v2 bin must be byte-identical");
}

#[test]
fn compose_refuses_missing_or_hidden_fallback_and_hash_collisions() {
    let entries = meter_bin::extract_entries(&v2_seven()).unwrap();

    // No fallback row
    let mut no_fb = entries.clone();
    for e in &mut no_fb {
        e.flags &= !FLAG_FALLBACK;
    }
    assert_eq!(
        meter_bin::compose(&no_fb).unwrap_err().refusal,
        Refusal::NoFallback
    );

    // Hidden fallback row
    let mut hidden_fb = entries.clone();
    hidden_fb[0].flags &= !FLAG_VISIBLE;
    assert_eq!(
        meter_bin::compose(&hidden_fb).unwrap_err().refusal,
        Refusal::Reserved
    );

    // Duplicate id_hash
    let mut dup = entries.clone();
    let mut clash = entries[1].clone();
    clash.id = format!("{}ffffffffffffffffffffffff", &entries[0].id[..8]);
    dup.push(clash);
    assert_eq!(meter_bin::compose(&dup).unwrap_err().refusal, Refusal::Dup);

    // Too many meters (> 64)
    let mut many: Vec<ComposeEntry> = Vec::new();
    for i in 0..65usize {
        let mut e = entries[if i == 0 { 0 } else { 1 }].clone();
        e.id = format!("{i:08x}{:024x}", i);
        e.flags = FLAG_VISIBLE | if i == 0 { FLAG_FALLBACK } else { 0 };
        many.push(e);
    }
    assert_eq!(
        meter_bin::compose(&many).unwrap_err().refusal,
        Refusal::Count
    );
}

#[test]
fn meter_package_validates_pins_paths_source_flag_and_presets() {
    let entries = meter_bin::extract_entries(&v2_seven()).unwrap();
    let pack = &entries[1].pack;

    let ok_zip = make_tmeter(
        "deadbeef11223344556677889900aabb",
        "TEST METER",
        "Alice",
        pack,
        true,
        true,
        &[],
        false,
    );
    let pkg = meter_package::parse(&ok_zip).unwrap();
    assert!(!pkg.summary.binary_only);
    assert_eq!(pkg.summary.preset_count, 1);
    assert_eq!(pkg.summary.id_hash_hex, "deadbeef");
    assert_eq!(
        pkg.summary.trust_label,
        meter_package::INDEPENDENT_TRUST_LABEL
    );
    assert_eq!(
        pkg.summary.warning_notice,
        meter_package::FULL_ACCESS_NOTICE
    );

    // Binary-only when source/ is absent
    let bin_only = make_tmeter(
        "deadbeef11223344556677889900aabb",
        "TEST METER",
        "Alice",
        pack,
        false,
        false,
        &[],
        false,
    );
    assert!(meter_package::parse(&bin_only).unwrap().summary.binary_only);

    // Tampered pin refused
    let bad_pin = make_tmeter(
        "deadbeef11223344556677889900aabb",
        "TEST METER",
        "Alice",
        pack,
        true,
        false,
        &[],
        true,
    );
    assert!(meter_package::parse(&bad_pin).is_err());

    // Unexpected file refused
    let extra = make_tmeter(
        "deadbeef11223344556677889900aabb",
        "TEST METER",
        "Alice",
        pack,
        true,
        false,
        &[("evil.sh", b"echo hi")],
        false,
    );
    assert!(meter_package::parse(&extra).is_err());

    // Path traversal refused
    let traversal = make_tmeter(
        "deadbeef11223344556677889900aabb",
        "TEST METER",
        "Alice",
        pack,
        true,
        false,
        &[("../escape.txt", b"no")],
        false,
    );
    assert!(meter_package::parse(&traversal).is_err());
}

#[test]
fn install_refuses_pre_075_core_and_supports_install_edit_source_check_and_reset() {
    let entries = meter_bin::extract_entries(&v2_seven()).unwrap();
    let pack = &entries[1].pack;
    let pkg_v1 = make_tmeter(
        "cafebabe11223344556677889900aabb",
        "CUSTOM ONE",
        "Author A",
        pack,
        true,
        true,
        &[],
        false,
    );

    // Pre-0.7.5 core without tau-meters.bin slot is refused
    let old_card = scratch("old-card");
    setup_card(&old_card, "0.7.1", false, true);
    let act = MeterAction::InstallPackage {
        package_bytes: pkg_v1.clone(),
        allow_source_change: false,
        overwrite_presets: false,
    };
    assert!(meter_install::plan(&old_card, &act, &BTreeMap::new()).is_err());

    // v0.7.5 card: install package + presets
    let card = scratch("v075-card");
    let backup = scratch("v075-backup");
    setup_card(&card, "0.7.5", true, true);
    let plan = meter_install::plan(&card, &act, &BTreeMap::new()).unwrap();
    assert_eq!(plan.action_kind, "install");
    assert_eq!(plan.rows.len(), 8);
    assert_eq!(plan.preset_files_to_write.len(), 2);

    // Wrong confirmation token refused
    assert!(meter_install::execute(&card, &act, &BTreeMap::new(), "wrong-token", &backup).is_err());
    assert!(!card.join("Assets/tau/common/tau-meters.bin").exists());

    // Execute with valid token
    let rep = meter_install::execute(&card, &act, &BTreeMap::new(), &plan.id, &backup).unwrap();
    assert_eq!(rep.presets_written, 2);
    let bin_bytes = fs::read(card.join("Assets/tau/common/tau-meters.bin")).unwrap();
    let parsed = meter_bin::parse(&bin_bytes).unwrap();
    assert_eq!(parsed.rows.len(), 8);
    assert_eq!(parsed.rows[7].name, "CUSTOM ONE");
    assert!(
        card.join("Assets/tau/common/tau-presets/cafebabe/index.tpi")
            .is_file()
    );
    assert!(
        card.join("Assets/tau/common/tau-presets/cafebabe/p0.tpr")
            .is_file()
    );

    // Same id from a DIFFERENT author is flagged and blocked until allow_source_change = true
    let pkg_other_author = make_tmeter(
        "cafebabe11223344556677889900aabb",
        "CUSTOM ONE",
        "Author B",
        pack,
        true,
        true,
        &[],
        false,
    );
    let mut authors = BTreeMap::new();
    authors.insert("cafebabe11223344556677889900aabb".into(), "Author A".into());
    let act_blocked = MeterAction::InstallPackage {
        package_bytes: pkg_other_author.clone(),
        allow_source_change: false,
        overwrite_presets: false,
    };
    let plan2 = meter_install::plan(&card, &act_blocked, &authors).unwrap();
    assert!(plan2.same_id_different_source);
    assert!(
        plan2.preset_skipped_existing,
        "existing presets preserved by default"
    );
    assert!(meter_install::execute(&card, &act_blocked, &authors, &plan2.id, &backup).is_err());

    let act_allowed = MeterAction::InstallPackage {
        package_bytes: pkg_other_author,
        allow_source_change: true,
        overwrite_presets: false,
    };
    let rep2 = meter_install::execute(&card, &act_allowed, &authors, &plan2.id, &backup).unwrap();
    assert_eq!(rep2.presets_written, 0);
    assert!(card.join("Assets/tau/common/tau-meters.bin.prev").is_file());
    assert!(rep2.backup_dir.is_some());

    // Edit list: hide second meter, reorder last meter to position 1
    let mut edits: Vec<RowEdit> = parsed
        .rows
        .iter()
        .map(|r| RowEdit {
            id: r.id.clone(),
            visible: r.visible(),
        })
        .collect();
    edits[1].visible = false;
    let last = edits.pop().unwrap();
    edits.insert(1, last);
    let edit_act = MeterAction::EditList { rows: edits };
    let edit_plan = meter_install::plan(&card, &edit_act, &authors).unwrap();
    assert_eq!(edit_plan.rows[1].name, "CUSTOM ONE");
    assert!(!edit_plan.rows[2].visible);
    meter_install::execute(&card, &edit_act, &authors, &edit_plan.id, &backup).unwrap();

    // Reset to shipped meters
    let reset_plan = meter_install::plan(&card, &MeterAction::ResetToShipped, &authors).unwrap();
    meter_install::execute(
        &card,
        &MeterAction::ResetToShipped,
        &authors,
        &reset_plan.id,
        &backup,
    )
    .unwrap();
    assert!(!card.join("Assets/tau/common/tau-meters.bin").exists());
}
