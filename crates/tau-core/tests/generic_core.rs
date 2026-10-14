//! A community (non-Tau) core through the reviewed install path: new install, same build, update, downgrade, a
//! version that cannot be ordered, and the user's own files. A generic core has no `bitstream.rbf_r`/`tau.rom` pair,
//! so nothing about Tau's firmware pairing may be claimed or required of it.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tau_core::{
    install_exec, install_plan,
    update::{PairStatus, UpdateVerdict},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "tau-generic-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&d).unwrap();
    d
}

fn core_json(version: &str, date: &str) -> String {
    format!(
        r#"{{"core":{{"magic":"APF_VER_1","metadata":{{"platform_ids":["ardu"],"shortname":"Arduboy","author":"agg23","description":"x","version":"{version}","date_release":"{date}"}},"framework":{{"target_product":"Analogue Pocket","version_required":"1.1","dock":{{"supported":true}}}},"cores":[{{"name":"default","id":0,"filename":"arduboy.rev"}}]}}}}"#
    )
}

fn zip(dir: &Path, version: &str, date: &str, rev: u8) -> PathBuf {
    let path = dir.join(format!("agg23.Arduboy.{version}.zip"));
    let mut w = ZipWriter::new(fs::File::create(&path).unwrap());
    let o = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, data) in [
        ("Cores/agg23.Arduboy/core.json", core_json(version, date).into_bytes()),
        ("Cores/agg23.Arduboy/arduboy.rev", vec![rev; 4096]),
        ("Cores/agg23.Arduboy/data.json", br#"{"data":{"magic":"APF_VER_1","data_slots":[{"name":"ROM","id":1,"required":false,"parameters":"0x1","extensions":["hex"]}]}}"#.to_vec()),
        ("Assets/ardu/common/.gitkeep", vec![]),
        ("Platforms/ardu.json", br#"{"platform":{"category":"Handheld","name":"Arduboy","manufacturer":"Arduboy","year":2016}}"#.to_vec()),
    ] {
        w.start_file(name, o).unwrap();
        w.write_all(&data).unwrap();
    }
    w.finish().unwrap();
    path
}

fn install(
    zip: &Path,
    card: &Path,
    backups: &Path,
    allow_older: bool,
) -> install_exec::InstallReport {
    let plan = install_plan::plan(zip, card, &[], allow_older).unwrap();
    assert!(plan.refused.is_none(), "{:?}", plan.refused);
    install_exec::execute(zip, card, &plan, &plan.id, backups, &[]).unwrap()
}

#[test]
fn a_new_generic_core_installs_without_any_firmware_claims() {
    let (d, card, bk) = (scratch("new"), scratch("card"), scratch("bk"));
    let z = zip(&d, "0.9.0", "2022-09-03", 1);
    let plan = install_plan::plan(&z, &card, &[], false).unwrap();
    assert!(plan.refused.is_none());
    let core = &plan.update.cores[0];
    assert_eq!(core.verdict, UpdateVerdict::NewInstall);
    assert_eq!(core.pair, PairStatus::NotApplicable);
    assert!(
        !plan.cautions.iter().any(|c| c.contains("firmware")),
        "{:?}",
        plan.cautions
    );
    assert!(
        !core.reasons.iter().any(|r| r.contains("bitstream")),
        "{:?}",
        core.reasons
    );
    let report = install_exec::execute(&z, &card, &plan, &plan.id, &bk, &[]).unwrap();
    assert!(report.ok(), "{:#?}", report.checks);
    let pairing = report.checks[0]
        .items
        .iter()
        .find(|i| i.name == "firmware pairing")
        .unwrap();
    assert_eq!(
        format!("{:?}", pairing.status),
        "Pass",
        "{}",
        pairing.detail
    );
    assert!(card.join("Cores/agg23.Arduboy/arduboy.rev").is_file());
}

#[test]
fn the_same_build_is_recognised_and_does_nothing() {
    let (d, card, bk) = (scratch("same"), scratch("card"), scratch("bk"));
    let z = zip(&d, "0.9.0", "2022-09-03", 1);
    install(&z, &card, &bk, false);
    let plan = install_plan::plan(&z, &card, &[], false).unwrap();
    assert_eq!(plan.update.cores[0].verdict, UpdateVerdict::SameBuild);
    assert!(plan.nothing_to_do);
}

#[test]
fn a_newer_version_is_an_update_and_the_users_files_survive() {
    let (d, card, bk) = (scratch("upd"), scratch("card"), scratch("bk"));
    install(&zip(&d, "0.9.0", "2022-09-03", 1), &card, &bk, false);
    // the user's own ROM and the licence file they added
    fs::create_dir_all(card.join("Assets/ardu/common")).unwrap();
    fs::write(card.join("Assets/ardu/common/game.hex"), b"my rom").unwrap();
    fs::write(card.join("Cores/agg23.Arduboy/beta.bin"), b"licence").unwrap();
    let newer = zip(&d, "1.0.0", "2026-01-01", 2);
    let plan = install_plan::plan(&newer, &card, &[], false).unwrap();
    assert_eq!(
        plan.update.cores[0].verdict,
        UpdateVerdict::Update,
        "{:?}",
        plan.update.cores[0].reasons
    );
    assert!(
        plan.update.cores[0].reasons[0].contains("0.9.0")
            && plan.update.cores[0].reasons[0].contains("1.0.0")
    );
    assert!(
        plan.backup
            .iter()
            .any(|b| b.path == "Cores/agg23.Arduboy/arduboy.rev")
    );
    let report = install_exec::execute(&newer, &card, &plan, &plan.id, &bk, &[]).unwrap();
    assert!(report.ok());
    assert_eq!(
        fs::read(card.join("Cores/agg23.Arduboy/arduboy.rev")).unwrap()[0],
        2
    );
    assert_eq!(
        fs::read(card.join("Assets/ardu/common/game.hex")).unwrap(),
        b"my rom"
    );
    assert_eq!(
        fs::read(card.join("Cores/agg23.Arduboy/beta.bin")).unwrap(),
        b"licence"
    );
    // and it can be undone exactly
    install_exec::rollback(&card, &report.backup_dir).unwrap();
    assert_eq!(
        fs::read(card.join("Cores/agg23.Arduboy/arduboy.rev")).unwrap()[0],
        1
    );
}

#[test]
fn an_older_version_is_a_downgrade_that_needs_the_users_choice() {
    let (d, card, bk) = (scratch("down"), scratch("card"), scratch("bk"));
    install(&zip(&d, "1.0.0", "2026-01-01", 2), &card, &bk, false);
    let older = zip(&d, "0.9.0", "2022-09-03", 1);
    let plan = install_plan::plan(&older, &card, &[], false).unwrap();
    assert_eq!(plan.update.cores[0].verdict, UpdateVerdict::Older);
    assert!(plan.refused.is_some());
    assert!(
        install_plan::plan(&older, &card, &[], true)
            .unwrap()
            .refused
            .is_none()
    );
}

#[test]
fn versions_that_cannot_be_ordered_are_not_guessed() {
    let (d, card, bk) = (scratch("odd"), scratch("card"), scratch("bk"));
    install(&zip(&d, "c68f02e", "2026-10-01", 1), &card, &bk, false);
    let other = zip(&d, "9a1b2c3", "2026-10-11", 2);
    let plan = install_plan::plan(&other, &card, &[], false).unwrap();
    assert_eq!(
        plan.update.cores[0].verdict,
        UpdateVerdict::SameDateDifferentBuild
    );
    assert!(plan.refused.is_none());
    assert!(!plan.cautions.is_empty());
}

/// The real Arduboy `data.json`: a slot with no `id` and a required file the user supplies. A genuine community core
/// like this must install with warnings, never be called "do not rely on this install".
#[test]
fn a_real_world_data_json_without_slot_ids_is_a_warning_not_a_failure() {
    let (d, card, bk) = (scratch("noid"), scratch("card"), scratch("bk"));
    let path = d.join("agg23.Arduboy.0.9.0.zip");
    let mut w = ZipWriter::new(fs::File::create(&path).unwrap());
    let o = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, data) in [
        ("Cores/agg23.Arduboy/core.json", core_json("0.9.0", "2022-09-03").into_bytes()),
        ("Cores/agg23.Arduboy/arduboy.rev", vec![1u8; 64]),
        ("Cores/agg23.Arduboy/data.json", br#"{"data":{"magic":"APF_VER_1","data_slots":[{"name":"HEX","required":true,"parameters":257,"extensions":["hex"],"address":"0x00000000"},{"name":"BIOS","id":2,"required":true,"filename":"bios.bin"}]}}"#.to_vec()),
    ] {
        w.start_file(name, o).unwrap();
        w.write_all(&data).unwrap();
    }
    w.finish().unwrap();
    let plan = install_plan::plan(&path, &card, &[], false).unwrap();
    let report = install_exec::execute(&path, &card, &plan, &plan.id, &bk, &[]).unwrap();
    assert!(report.ok(), "{:#?}", report.checks);
    let slots = report.checks[0]
        .items
        .iter()
        .find(|i| i.name == "data slots")
        .unwrap();
    assert_eq!(format!("{:?}", slots.status), "Warn");
    assert!(
        slots.detail.contains("declares no id") && slots.detail.contains("bios.bin"),
        "{}",
        slots.detail
    );
}
