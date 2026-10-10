#[test]
fn a_core_specific_required_slot_is_looked_for_in_the_core_folder() {
    let root = std::env::temp_dir().join(format!(
        "tau-slots-h4-{}-{}",
        std::process::id(),
        crate::test_uniq()
    ));
    let (core_dir, common, core_assets) = (
        root.join("Cores/alfatreze.TAU"),
        root.join("Assets/tau/common"),
        root.join("Assets/tau/alfatreze.TAU"),
    );
    for d in [&core_dir, &common, &core_assets] {
        fs::create_dir_all(d).unwrap();
    }
    fs::write(core_dir.join("data.json"), br#"{"data":{"data_slots":[{"name":"Firmware","id":1,"required":true,"parameters":"0x10A","filename":"tau.rom"}]}}"#).unwrap();
    fs::write(common.join("tau.rom"), b"old place").unwrap();
    let r = data_slot_problems(&core_dir, std::slice::from_ref(&common), &core_assets).unwrap();
    assert!(
        r.problems.iter().any(|p| p.contains("tau.rom")),
        "a core-specific slot is not satisfied by common/: {:?}",
        r.problems
    );
    fs::write(core_assets.join("tau.rom"), b"rom").unwrap();
    assert!(
        data_slot_problems(&core_dir, std::slice::from_ref(&common), &core_assets)
            .unwrap()
            .problems
            .is_empty()
    );
    let _ = fs::remove_dir_all(root);
}

use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn pkg(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/packages")
        .join(name)
}
fn alpha3() -> PathBuf {
    pkg("alfatreze.TAU_0.6.0_2026-10-04.zip")
}
fn alpha4() -> PathBuf {
    pkg("alfatreze.TAU_0.6.0_2026-10-07.zip")
}
fn diagnostic() -> PathBuf {
    pkg("alfatreze.TAU_DIAGNOSTIC_0.6.0_2026-10-07.zip")
}
fn old_release() -> PathBuf {
    pkg("alfatreze.TAU_0.4.0_2026-09-22.zip")
}

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "tau-update-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            + crate::test_uniq()
    ));
    fs::create_dir_all(root.join("Cores")).unwrap();
    fs::create_dir_all(root.join("Assets")).unwrap();
    root
}

fn install(zip: &Path, card: &Path) {
    let plan = package::plan_install(zip, card).unwrap();
    package::execute_install(zip, card, &plan, &plan.id).unwrap();
}

fn verdict(zip: &Path, card: &Path) -> UpdateAssessment {
    assess_update(zip, card).unwrap().cores.remove(0)
}

#[test]
fn real_roms_carry_the_pairing_marker() {
    let reader = ZipReader::open(&alpha4()).unwrap();
    let rom = reader.read("Assets/tau/common/tau.rom").unwrap();
    assert_eq!(rom_pair_marker(&rom), Some(vec!["4D50331A".to_string()]));
    // TAUFWNEED may be absent from this ROM; when present it must parse.
    if let Some(needs) = rom_needs_marker(&rom) {
        assert!(
            needs
                .iter()
                .all(|f| f.bytes().all(|b| b.is_ascii_uppercase() || b == b'_'))
        );
    }
    let old = ZipReader::open(&old_release()).unwrap();
    let old_rom = old.read("Assets/tau/common/tau.rom").unwrap();
    assert_eq!(
        rom_pair_marker(&old_rom),
        None,
        "the 0.4.0 ROM predates the marker"
    );
}

#[test]
fn marker_parser_rejects_malformed_lists() {
    assert_eq!(
        rom_pair_marker(b"xxTAUFWPAIR:4D50331A,4D50331B;yy"),
        Some(vec!["4D50331A".into(), "4D50331B".into()])
    );
    assert_eq!(
        rom_pair_marker(b"TAUFWPAIR:4d50331a;"),
        None,
        "lower case is not what the firmware writes"
    );
    assert_eq!(rom_pair_marker(b"TAUFWPAIR:4D5033;"), None, "short");
    assert_eq!(
        rom_pair_marker(b"TAUFWPAIR:4D50331A"),
        None,
        "no terminator"
    );
    assert_eq!(rom_needs_marker(b"TAUFWNEED:;"), Some(vec![]));
    assert_eq!(
        rom_needs_marker(b"TAUFWNEED:HALCYON,LPC;"),
        Some(vec!["HALCYON".into(), "LPC".into()])
    );
    assert_eq!(rom_needs_marker(b"TAUFWNEED:halcyon;"), None);
}

#[test]
fn empty_card_is_a_new_install() {
    let card = scratch("new");
    let report = assess_update(&alpha4(), &card).unwrap();
    assert_eq!(report.cores.len(), 1);
    assert_eq!(report.cores[0].verdict, UpdateVerdict::NewInstall);
    assert_eq!(
        report.cores[0].pair,
        PairStatus::Verified {
            core_version: "4D50331A".into()
        }
    );
    assert!(report.files_replaced.is_empty());
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn same_zip_again_is_the_same_build() {
    let card = scratch("same");
    install(&alpha4(), &card);
    assert_eq!(verdict(&alpha4(), &card).verdict, UpdateVerdict::SameBuild);
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn newer_same_day_and_older_packages_are_told_apart() {
    // alpha3 -> alpha4 changes the ROM and the date only (the bitstream is the same).
    let card = scratch("order");
    install(&alpha3(), &card);
    let up = verdict(&alpha4(), &card);
    assert_eq!(up.verdict, UpdateVerdict::Update);
    assert!(
        up.reasons[0].contains("2026-10-07") && up.reasons[0].contains("2026-10-04"),
        "{:?}",
        up.reasons
    );
    let down = {
        install(&alpha4(), &card);
        verdict(&alpha3(), &card)
    };
    assert_eq!(down.verdict, UpdateVerdict::Older);

    fs::remove_dir_all(card).unwrap();
}

#[test]
fn different_bytes_on_the_same_date_is_ambiguous() {
    let card = scratch("ambiguous");
    install(&alpha4(), &card);
    // Same version and date in core.json, a different firmware file.
    let rom = card.join("Assets/tau/common/tau.rom");
    let mut bytes = fs::read(&rom).unwrap();
    *bytes.last_mut().unwrap() ^= 0xFF;
    fs::write(&rom, bytes).unwrap();
    let assessment = verdict(&alpha4(), &card);
    assert_eq!(assessment.verdict, UpdateVerdict::SameDateDifferentBuild);
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn the_diagnostic_build_is_not_an_update_of_the_normal_core() {
    let card = scratch("diag");
    install(&alpha4(), &card);
    // A different core id: its own new install, and the normal core is untouched.
    let assessment = verdict(&diagnostic(), &card);
    assert_eq!(assessment.verdict, UpdateVerdict::NewInstall);
    assert_eq!(assessment.package.core_id, "alfatreze.TAU_DIAGNOSTIC");
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn a_core_with_the_wrong_name_in_the_same_folder_is_a_mismatch() {
    let card = scratch("name");
    install(&alpha4(), &card);
    let core_json = card.join("Cores/alfatreze.TAU/core.json");
    let text = fs::read_to_string(&core_json)
        .unwrap()
        .replace("\"shortname\": \"TAU\"", "\"shortname\": \"OTHER\"");
    fs::write(&core_json, text).unwrap();
    assert_eq!(verdict(&alpha4(), &card).verdict, UpdateVerdict::Mismatch);
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn identity_without_a_marker_is_reported_not_refused() {
    let card = scratch("old");
    let report = assess_update(&old_release(), &card).unwrap();
    let core = &report.cores[0];
    assert_eq!(core.verdict, UpdateVerdict::NewInstall);
    assert_eq!(core.pair, PairStatus::NoMarker);
    assert!(
        core.reasons.iter().any(|r| r.contains("no pairing marker")),
        "{:?}",
        core.reasons
    );
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn a_pair_the_pocket_would_refuse_is_a_mismatch() {
    let mut package = identity_from("alfatreze.TAU", |p| {
        ZipReader::open(&alpha4()).unwrap().read(p)
    })
    .unwrap()
    .unwrap();
    package.rom_accepts = Some(vec!["4D503317".into()]); // what alpha.1 shipped with
    let assessment = assess(None, package);
    assert_eq!(assessment.verdict, UpdateVerdict::Mismatch);
    assert!(assessment.reasons[0].contains("black screen"));
}

#[test]
fn an_update_names_what_it_replaces_and_what_it_keeps() {
    let card = scratch("keeps");
    install(&alpha3(), &card);
    let common = card.join("Assets/tau/common");
    fs::write(common.join("tau-library.tdb"), b"library").unwrap();
    fs::write(common.join("tau-assets.bin"), b"themes").unwrap();
    fs::write(common.join("song.mp3"), b"music").unwrap();
    let report = assess_update(&alpha4(), &card).unwrap();
    assert_eq!(report.cores[0].verdict, UpdateVerdict::Update);
    assert!(
        report
            .files_replaced
            .contains(&"Assets/tau/common/tau.rom".to_string())
    );
    assert!(
        report
            .files_replaced
            .contains(&"Cores/alfatreze.TAU/core.json".to_string())
    );
    assert!(
        !report
            .files_replaced
            .iter()
            .any(|p| p.contains("bitstream")),
        "the bitstream is identical"
    );
    assert_eq!(
        report.user_files_kept,
        vec![
            "Assets/tau/common/tau-library.tdb",
            "Assets/tau/common/tau-assets.bin"
        ]
    );
    // Carrying the update out leaves the user's files byte-for-byte alone.
    install(&alpha4(), &card);
    assert_eq!(
        fs::read(common.join("tau-library.tdb")).unwrap(),
        b"library"
    );
    assert_eq!(fs::read(common.join("tau-assets.bin")).unwrap(), b"themes");
    assert_eq!(fs::read(common.join("song.mp3")).unwrap(), b"music");
    assert_eq!(verdict(&alpha4(), &card).verdict, UpdateVerdict::SameBuild);
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn a_zip_without_a_core_is_not_a_package() {
    let dir = scratch("notpkg");
    let zip_path = dir.join("x.zip");
    {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
        writer
            .start_file("readme.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"hi").unwrap();
        writer.finish().unwrap();
    }
    assert!(assess_update(&zip_path, &dir).is_err());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn version_order_is_numeric() {
    use std::cmp::Ordering::*;
    assert_eq!(compare_versions("0.10.0", "0.9.0"), Greater);
    assert_eq!(compare_versions("0.6.0", "0.6.0"), Equal);
    assert_eq!(compare_versions("0.5.0", "0.6.0"), Less);
    assert_eq!(compare_versions("0.6.0-alpha.4", "0.6.0"), Equal);
}

// ---- post-install check ----

fn find<'a>(report: &'a PostInstallReport, name: &str) -> &'a CheckItem {
    report
        .items
        .iter()
        .find(|i| i.name == name)
        .unwrap_or_else(|| panic!("no item {name}"))
}

#[test]
fn a_fresh_install_passes_with_only_the_expected_library_warning() {
    let card = scratch("check-fresh");
    install(&alpha4(), &card);
    let report = post_install_check(&card, "alfatreze.TAU", Some(&alpha4())).unwrap();
    for name in [
        "core.json",
        "files match package",
        "firmware pairing",
        "data slots",
        "catalog caches",
        "stray files",
    ] {
        assert_eq!(
            find(&report, name).status,
            CheckStatus::Pass,
            "{name}: {}",
            find(&report, name).detail
        );
    }
    let library = find(&report, "library index");
    assert_eq!(library.status, CheckStatus::Warn);
    assert!(library.detail.contains("Library file not found"));
    assert_eq!(report.verdict, CheckStatus::Warn);
    assert!(report.to_text().contains("[WARN] library index"));
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn a_changed_file_stale_caches_and_stubs_are_all_caught() {
    let card = scratch("check-bad");
    install(&alpha4(), &card);
    fs::write(card.join("Assets/tau/common/tau-cold.bin"), b"half written").unwrap();
    fs::create_dir_all(card.join("System")).unwrap();
    fs::write(card.join("System/cores_cache.bin"), b"old").unwrap();
    fs::write(card.join("Cores/alfatreze.TAU/._core.json"), b"stub").unwrap();
    let report = post_install_check(&card, "alfatreze.TAU", Some(&alpha4())).unwrap();
    assert_eq!(report.verdict, CheckStatus::Fail);
    assert!(
        find(&report, "files match package")
            .detail
            .contains("tau-cold.bin")
    );
    assert_eq!(find(&report, "catalog caches").status, CheckStatus::Warn);
    assert!(find(&report, "stray files").detail.contains("._core.json"));
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn a_missing_required_firmware_file_fails_the_slot_check() {
    let card = scratch("check-slot");
    install(&alpha4(), &card);
    fs::remove_file(card.join("Assets/tau/common/tau.rom")).unwrap();
    let report = post_install_check(&card, "alfatreze.TAU", None).unwrap();
    let slots = find(&report, "data slots");
    assert_eq!(slots.status, CheckStatus::Fail);
    assert!(slots.detail.contains("tau.rom"), "{}", slots.detail);
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn a_library_rooted_at_another_core_fails() {
    let card = scratch("check-root");
    install(&alpha4(), &card);
    let common = card.join("Assets/tau/common");
    let entries = crate::synth(5, 2, 2);
    let index =
        crate::build_index(&entries, &[], "/Assets/tau_dev_67/common/", &mut Vec::new()).unwrap();
    fs::write(common.join("tau-library.tdb"), &index).unwrap();
    let report = post_install_check(&card, "alfatreze.TAU", None).unwrap();
    let library = find(&report, "library index");
    assert_eq!(library.status, CheckStatus::Fail, "{}", library.detail);
    assert!(library.detail.contains("/Assets/tau_dev_67/common/"));
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn a_library_built_for_this_core_passes() {
    let card = scratch("check-lib");
    install(&alpha4(), &card);
    let common = card.join("Assets/tau/common");
    let entries = crate::synth(20, 4, 3);
    let index = crate::build_index(
        &entries,
        &[],
        &crate::root_prefix(&common).unwrap(),
        &mut Vec::new(),
    )
    .unwrap();
    fs::write(common.join("tau-library.tdb"), &index).unwrap();
    let report = post_install_check(&card, "alfatreze.TAU", None).unwrap();
    let library = find(&report, "library index");
    // The synthetic tracks have no files on this card: rooted correctly, but missing media.
    assert_eq!(library.status, CheckStatus::Warn, "{}", library.detail);
    assert!(library.detail.contains("rooted at this core") && library.detail.contains("missing"));
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn a_core_json_over_the_limits_fails() {
    let card = scratch("check-limits");
    install(&alpha4(), &card);
    let path = card.join("Cores/alfatreze.TAU/core.json");
    let long = "x".repeat(70);
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("TAU Music Player", &long);
    fs::write(&path, text).unwrap();
    let report = post_install_check(&card, "alfatreze.TAU", None).unwrap();
    let item = find(&report, "core.json");
    assert_eq!(item.status, CheckStatus::Fail);
    assert!(
        item.detail.contains("description is 70 characters"),
        "{}",
        item.detail
    );
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn a_missing_core_is_one_failed_item_not_an_error() {
    let card = scratch("check-none");
    let report = post_install_check(&card, "alfatreze.TAU", None).unwrap();
    assert_eq!(report.verdict, CheckStatus::Fail);
    assert_eq!(report.items.len(), 1);
    fs::remove_dir_all(card).unwrap();
}

// ---- release manifests ----

/// A schema-2 manifest built from a real package's own entries and hashes (what Tau's release tool records for it).
fn manifest(
    zip: &Path,
    tag: &str,
    previous: Option<&str>,
    persist: &[u64],
    version: &str,
) -> CompatDoc {
    let reader = ZipReader::open(zip).unwrap();
    let id = identity_from("alfatreze.TAU", |p| reader.read(p))
        .unwrap()
        .unwrap();
    let layout: Vec<_> = package::inspect(zip)
        .unwrap()
        .entries
        .iter()
        .map(|e| serde_json::json!({"path": e.path, "role": "owned", "sha256": e.sha256, "slot": null, "required": false}))
        .collect();
    let doc = serde_json::json!({
        "schema": 2, "release": tag, "date_release": id.date_release, "prerelease": true,
        "previous_release": previous,
        "packages": [{"zip": "x.zip", "zip_sha256": "00", "core_id": "alfatreze.TAU",
            "bitstream_sha256": id.bitstream_sha256, "bitstream_core_version": version,
            "rom_sha256": id.rom_sha256, "cold_sha256": id.cold_sha256,
            "rom_accepts": id.rom_accepts, "rom_needs": id.rom_needs, "layout": layout}],
        "requires_omega": {"persist_ids_changed": persist, "min_omega": "0.4.0"}
    });
    compat::parse_compat(&serde_json::to_vec(&doc).unwrap()).unwrap()
}

fn two_releases() -> Vec<CompatDoc> {
    vec![
        manifest(&alpha3(), "v0.6.0-alpha.3", None, &[], "4D50331A"),
        manifest(
            &alpha4(),
            "v0.6.0-alpha.4",
            Some("v0.6.0-alpha.3"),
            &[16],
            "4D50331A",
        ),
    ]
}

fn doc_with(id: &BuildIdentity, features: Option<&[&str]>, dirty: bool) -> CompatDoc {
    let doc = serde_json::json!({
        "schema": 2, "release": "v0.7.0-preview.1", "date_release": "2026-11-01", "prerelease": true,
        "source": {"commit": "abc1234", "dirty": dirty},
        "packages": [{"zip": "x.zip", "zip_sha256": "00", "core_id": id.core_id,
            "bitstream_sha256": id.bitstream_sha256, "bitstream_core_version": "4D50331A",
            "bitstream_features": features, "rom_version": "0.7.0-preview.1+abc1234",
            "rom_sha256": id.rom_sha256, "cold_sha256": id.cold_sha256,
            "rom_accepts": ["4D50331A"], "rom_needs": ["HALCYON"],
            "layout": [{"path": "Cores/x/core.json", "role": "owned", "sha256": "00", "slot": null, "required": false}]}],
        "requires_omega": {"min_omega": "0.4.0"}
    });
    compat::parse_compat(&serde_json::to_vec(&doc).unwrap()).unwrap()
}

#[test]
fn a_rom_needing_a_feature_the_bitstream_lacks_is_refused() {
    let mut id = package_identity(&alpha4(), "alfatreze.TAU").unwrap();
    id.rom_accepts = Some(vec!["4D50331A".into()]);
    id.rom_needs = Some(vec!["HALCYON".into(), "LPC".into()]);
    let lacking = doc_with(&id, Some(&["LPC"]), false);
    assert_eq!(
        pair_status_with(&id, std::slice::from_ref(&lacking)),
        PairStatus::MissingFeature {
            missing: vec!["HALCYON".into()],
            bitstream_has: vec!["LPC".into()]
        }
    );
    let a = assess_with(None, id.clone(), std::slice::from_ref(&lacking));
    assert_eq!(a.verdict, UpdateVerdict::Mismatch);
    assert!(a.reasons[0].contains("HALCYON") && a.reasons[0].contains("NO UNIT"));
    // The bitstream has everything, or the manifest does not say: not refused.
    let full = doc_with(&id, Some(&["HALCYON", "LPC", "CYMO"]), false);
    assert!(matches!(
        pair_status_with(&id, std::slice::from_ref(&full)),
        PairStatus::Verified { .. }
    ));
    let silent = doc_with(&id, None, false);
    assert!(matches!(
        pair_status_with(&id, std::slice::from_ref(&silent)),
        PairStatus::Verified { .. }
    ));
}

#[test]
fn a_release_notes_its_firmware_version_and_a_dirty_tree() {
    let mut id = package_identity(&alpha4(), "alfatreze.TAU").unwrap();
    id.rom_accepts = Some(vec!["4D50331A".into()]);
    let dirty = doc_with(&id, Some(&["HALCYON"]), true);
    let a = assess_with(Some(id.clone()), id.clone(), std::slice::from_ref(&dirty));
    // Same build: notes only appear when there is something to compare; check the new-install path instead.
    assert_eq!(a.verdict, UpdateVerdict::SameBuild);
    let mut older = id.clone();
    older.bitstream_sha256 = Some("00".repeat(32));
    older.version = "0.6.0".into();
    let a = assess_with(Some(older), id, std::slice::from_ref(&dirty));
    let joined = a.reasons.join(" | ");
    assert!(
        joined.contains("Firmware 0.7.0-preview.1+abc1234."),
        "{joined}"
    );
    assert!(
        joined.contains("uncommitted changes") && joined.contains("abc1234"),
        "{joined}"
    );
}

#[test]
fn same_day_builds_are_ordered_by_the_full_core_version() {
    let mut old = package_identity(&alpha4(), "alfatreze.TAU").unwrap();
    old.version = "0.7.0-dev.385".into();
    old.bitstream_sha256 = Some("11".repeat(32));
    let mut new = old.clone();
    new.version = "0.7.0-dev.386".into();
    new.bitstream_sha256 = Some("22".repeat(32));
    assert_eq!(
        assess(Some(old.clone()), new.clone()).verdict,
        UpdateVerdict::Update
    );
    assert_eq!(assess(Some(new), old).verdict, UpdateVerdict::Older);
}

#[test]
fn manifests_name_both_builds_order_them_and_list_changed_settings() {
    let docs = two_releases();
    let card = scratch("m-update");
    install(&alpha3(), &card);
    let report = assess_update_with(&alpha4(), &card, &docs).unwrap();
    let core = &report.cores[0];
    assert_eq!(core.verdict, UpdateVerdict::Update);
    assert_eq!(core.installed_release.as_deref(), Some("v0.6.0-alpha.3"));
    assert_eq!(core.package_release.as_deref(), Some("v0.6.0-alpha.4"));
    assert_eq!(
        core.reasons[0],
        "Update: v0.6.0-alpha.4 replaces v0.6.0-alpha.3."
    );
    assert_eq!(report.persist_changed, Some(vec![16]));

    install(&alpha4(), &card);
    let down = assess_update_with(&alpha3(), &card, &docs).unwrap();
    assert_eq!(down.cores[0].verdict, UpdateVerdict::Older);
    assert!(
        down.cores[0].reasons[0].contains("v0.6.0-alpha.3")
            && down.cores[0].reasons[0].contains("downgrade")
    );
    // Without manifests the same call still works, with version and date only.
    let plain = assess_update(&alpha3(), &card).unwrap();
    assert_eq!(plain.cores[0].installed_release, None);
    assert_eq!(plain.persist_changed, None);
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn tags_settle_two_builds_a_date_cannot() {
    // Pretend both packages are the same day: only the tags can order them.
    let docs = two_releases();
    let mut old = identity_from("alfatreze.TAU", |p| {
        ZipReader::open(&alpha3()).unwrap().read(p)
    })
    .unwrap()
    .unwrap();
    let mut new = identity_from("alfatreze.TAU", |p| {
        ZipReader::open(&alpha4()).unwrap().read(p)
    })
    .unwrap()
    .unwrap();
    old.date_release = "2026-10-07".into();
    new.date_release = "2026-10-07".into();
    assert_eq!(
        assess(Some(old.clone()), new.clone()).verdict,
        UpdateVerdict::SameDateDifferentBuild
    );
    assert_eq!(
        assess_with(Some(old), new, &docs).verdict,
        UpdateVerdict::Update
    );
}

#[test]
fn a_manifest_vouches_for_what_the_table_and_marker_cannot() {
    // The 0.4.0 ROM has no marker and its bitstream is not in the table.
    let reader = ZipReader::open(&old_release()).unwrap();
    let id = identity_from("alfatreze.TAU", |p| reader.read(p))
        .unwrap()
        .unwrap();
    assert_eq!(pair_status(&id), PairStatus::NoMarker);
    let doc = manifest(&old_release(), "v0.4.0", None, &[], "4D503310");
    assert_eq!(
        pair_status_with(&id, &[doc]),
        PairStatus::Verified {
            core_version: "4D503310".into()
        }
    );
}

#[test]
fn a_manifest_that_contradicts_the_rom_is_a_mismatch() {
    let docs = vec![manifest(&alpha4(), "v0.6.0-alpha.4", None, &[], "4D503317")];
    let card = scratch("m-mismatch");
    let report = assess_update_with(&alpha4(), &card, &docs).unwrap();
    assert_eq!(report.cores[0].verdict, UpdateVerdict::Mismatch);
    assert!(report.cores[0].reasons[0].contains("black screen"));
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn the_post_install_check_adds_the_release_layout_item() {
    let docs = two_releases();
    let card = scratch("m-check");
    install(&alpha4(), &card);
    let report = post_install_check_with(&card, "alfatreze.TAU", Some(&alpha4()), &docs).unwrap();
    assert_eq!(
        find(&report, "release layout").status,
        CheckStatus::Pass,
        "{}",
        find(&report, "release layout").detail
    );
    assert!(
        find(&report, "release layout")
            .detail
            .contains("v0.6.0-alpha.4")
    );
    // With no manifest the item is simply absent.
    let plain = post_install_check(&card, "alfatreze.TAU", Some(&alpha4())).unwrap();
    assert!(plain.items.iter().all(|i| i.name != "release layout"));
    // A damaged firmware file no longer matches any release: no layout item, and the file check fails.
    fs::write(card.join("Assets/tau/common/tau.rom"), b"broken").unwrap();
    let broken = post_install_check_with(&card, "alfatreze.TAU", Some(&alpha4()), &docs).unwrap();
    assert_eq!(broken.verdict, CheckStatus::Fail);
    fs::remove_dir_all(card).unwrap();
}
