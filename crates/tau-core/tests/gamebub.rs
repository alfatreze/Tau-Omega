//! End-to-end tests for multi-device detection (G0), Game Bub read-only inspection (G1),
//! Game Bub Core Directory catalog & update verdicts (G2), and safe zip vetting/install/update/remove (G3).

use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};
use tau_core::{
    core_update::Status,
    device::{self, DeviceKind},
    gamebub, gamebub_install,
};

fn scratch(tag: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let p = std::env::temp_dir().join(format!(
        "tau-gb-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn put(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}

fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut cur = Cursor::new(Vec::new());
    {
        let mut zw = zip::ZipWriter::new(&mut cur);
        let opts = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(data).unwrap();
        }
        zw.finish().unwrap();
    }
    cur.into_inner()
}

fn gb_core_json(id: &str, name: &str, version: &str, bit: &str) -> Vec<u8> {
    serde_json::to_vec_pretty(&serde_json::json!({
        "min_firmware": "1.1.0-beta.5",
        "metadata": {
            "id": id,
            "name": name,
            "author": "Test Author",
            "version": version,
            "release_date": "2026-11-24",
            "license": "MIT"
        },
        "bitstreams": [
            { "target": "gamebub_rev4", "filename": bit }
        ],
        "hardware": { "cartridge_enable": "no", "cartridge_selectable": false }
    }))
    .unwrap()
}

#[test]
fn device_detection_distinguishes_pocket_gamebub_and_blank_cards_safely() {
    let blank = scratch("blank");
    assert_eq!(device::detect_card(&blank).device, None);

    let pocket = scratch("pocket");
    put(
        &pocket,
        "Cores/alfatreze.TAU/core.json",
        br#"{"core":{"metadata":{"version":"0.7.5","platform_ids":["tau"]}}}"#,
    );
    assert_eq!(
        device::detect_card(&pocket).device,
        Some(DeviceKind::AnaloguePocket)
    );

    let gb = scratch("gb");
    put(
        &gb,
        "cores/elipsitz.N64/core.json",
        &gb_core_json("elipsitz.N64", "Nintendo 64", "0.3.0", "n64.bit"),
    );
    assert_eq!(device::detect_card(&gb).device, Some(DeviceKind::GameBub));

    // BIOS-only Game Bub card (no cores installed yet)
    let gb_bios = scratch("gb-bios");
    put(&gb_bios, "system/gba.bios.bin", &[0u8; 16384]);
    assert_eq!(
        device::detect_card(&gb_bios).device,
        Some(DeviceKind::GameBub)
    );
}

#[test]
fn gamebub_card_inspector_checks_bitstreams_required_files_settings_and_bios() {
    let card = scratch("inspect");
    put(
        &card,
        "cores/jeremy.PSX/core.json",
        &gb_core_json("jeremy.PSX", "PlayStation", "0.2.1", "psx-rev4.bit"),
    );
    put(&card, "cores/jeremy.PSX/psx-rev4.bit", b"fake-bitstream");
    put(
        &card,
        "cores/jeremy.PSX/files.json",
        br#"{"files":[
            {"id":0,"label":"Disc","user_selected":true,"extensions":["bin","cue"],"optional":false},
            {"id":1,"label":"BIOS","filename":"scph1001.bin","optional":false,"read_only":true},
            {"id":2,"label":"Save","dependent_on_0":true,"extensions":["sav"],"optional":true}
        ]}"#,
    );
    put(
        &card,
        "cores/jeremy.PSX/settings.json",
        br#"{"settings":[{"id":0,"label":"Region","type":"list","address":"0x0"}]}"#,
    );
    put(&card, "system/gameboy.bios-dmg.bin", &[0u8; 256]);
    put(&card, "system/gba.bios.bin", &[0u8; 100]); // wrong size

    let view = gamebub::inspect_card(&card);
    assert_eq!(view.cores.len(), 1);
    assert!(view.cores[0].missing_bitstreams.is_empty());
    assert_eq!(view.cores[0].missing_required_files, ["scph1001.bin"]);
    assert_eq!(view.cores[0].settings.len(), 1);
    assert_eq!(view.bios.len(), 3);
    assert!(view.bios[0].present && view.bios[0].size_ok);
    assert!(!view.bios[1].present);
    assert!(view.bios[2].present && !view.bios[2].size_ok);
}

#[test]
fn gamebub_catalog_parses_cores_json_and_computes_update_verdicts() {
    let json = serde_json::to_vec(&serde_json::json!({
        "schema_version": 1,
        "generated_at": "2026-11-26T13:56:12Z",
        "default_target": "gamebub_rev4",
        "cores": [
            {
                "id": "elipsitz.N64",
                "name": "Nintendo 64 (MIRLO64)",
                "author": "tortugaveloz, elipsitz",
                "description": "N64 core",
                "version": "0.3.0",
                "release_date": "2026-11-23",
                "license": "Apache-2.0",
                "min_firmware": "1.1.0-beta.5",
                "targets": ["gamebub_rev4"],
                "repo": "elipsitz/n64-gamebub",
                "download_url": "https://github.com/elipsitz/n64-gamebub/releases/download/v0.3.0/elipsitz.N64-v0.3.0.zip",
                "asset_size": 12345,
                "asset_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "zip_layout": "<id>"
            }
        ]
    }))
    .unwrap();

    let cat = gamebub::parse_catalog(&json).unwrap();
    assert_eq!(cat.cores.len(), 1);
    let c = &cat.cores[0];
    assert_eq!(c.device, DeviceKind::GameBub);
    assert_eq!(c.source, gamebub::GAMEBUB_SOURCE);
    assert_eq!(c.repository.owner, "elipsitz");
    assert_eq!(c.repository.name, "n64-gamebub");
    assert_eq!(c.targets, ["gamebub_rev4"]);
    assert_eq!(c.firmware_required, "1.1.0-beta.5");

    let card = scratch("updates");
    put(
        &card,
        "cores/elipsitz.N64/core.json",
        &gb_core_json("elipsitz.N64", "Nintendo 64", "0.2.0", "n64.bit"),
    );
    let updates = gamebub::check_updates(&card, &cat);
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].status, Status::UpdateAvailable);
}

#[test]
fn gamebub_install_supports_both_zip_layouts_multicore_zips_preserves_bios_and_refuses_pocket_cards()
 {
    let backup = scratch("gb-backup");

    // 1. `<id>` zip layout
    let n64_zip = make_zip(&[
        (
            "elipsitz.N64/core.json",
            &gb_core_json("elipsitz.N64", "Nintendo 64", "0.3.0", "n64.bit"),
        ),
        ("elipsitz.N64/n64.bit", b"bitstream-v1"),
    ]);
    let n64_sha = sha(&n64_zip);

    // Refuses Analogue Pocket card
    let pocket = scratch("pocket-refuse");
    put(
        &pocket,
        "Cores/alfatreze.TAU/core.json",
        br#"{"core":{"metadata":{"version":"0.7.5","platform_ids":["tau"]}}}"#,
    );
    assert!(gamebub_install::plan_install(&pocket, &n64_zip, None, Some(&n64_sha)).is_err());

    // Refuses wrong SHA-256
    let card = scratch("gb-card");
    assert!(gamebub_install::plan_install(&card, &n64_zip, None, Some("0000")).is_err());

    // Install `<id>` layout onto fresh card -> normalized under `cores/elipsitz.N64/`
    let plan = gamebub_install::plan_install(&card, &n64_zip, None, Some(&n64_sha)).unwrap();
    assert_eq!(plan.cores.len(), 1);
    assert_eq!(plan.cores[0].action, "install");
    let rep =
        gamebub_install::execute_install(&card, &n64_zip, None, Some(&n64_sha), &plan.id, &backup)
            .unwrap();
    assert_eq!(rep.cores_installed, ["elipsitz.N64"]);
    assert!(card.join("cores/elipsitz.N64/core.json").is_file());
    assert!(card.join("cores/elipsitz.N64/n64.bit").is_file());

    // Place a user BIOS inside `/cores/elipsitz.N64/` and update -> BIOS must survive!
    put(&card, "cores/elipsitz.N64/user-bios.bin", b"keep-me");
    let n64_v2 = make_zip(&[
        (
            "cores/elipsitz.N64/core.json",
            &gb_core_json("elipsitz.N64", "Nintendo 64", "0.4.0", "n64.bit"),
        ),
        ("cores/elipsitz.N64/n64.bit", b"bitstream-v2"),
    ]);
    let plan2 = gamebub_install::plan_install(&card, &n64_v2, None, None).unwrap();
    assert_eq!(plan2.cores[0].action, "update");
    assert_eq!(plan2.cores[0].previous_version.as_deref(), Some("0.3.0"));
    assert_eq!(plan2.backup_files.len(), 2);
    gamebub_install::execute_install(&card, &n64_v2, None, None, &plan2.id, &backup).unwrap();
    assert_eq!(
        fs::read(card.join("cores/elipsitz.N64/user-bios.bin")).unwrap(),
        b"keep-me"
    );
    assert_eq!(
        fs::read(card.join("cores/elipsitz.N64/n64.bit")).unwrap(),
        b"bitstream-v2"
    );

    // 2. Multi-core zip (`cores/<id>` layout): install only one core first, then both
    let multi_zip = make_zip(&[
        (
            "cores/apolkosnik.Minimig/core.json",
            &gb_core_json("apolkosnik.Minimig", "Minimig", "0.10.0", "mm.bit"),
        ),
        ("cores/apolkosnik.Minimig/mm.bit", b"mm-bit"),
        (
            "cores/apolkosnik.Minimig040/core.json",
            &gb_core_json(
                "apolkosnik.Minimig040",
                "Minimig 68040",
                "0.4.0",
                "mm040.bit",
            ),
        ),
        ("cores/apolkosnik.Minimig040/mm040.bit", b"mm040-bit"),
    ]);
    let p_one =
        gamebub_install::plan_install(&card, &multi_zip, Some("apolkosnik.Minimig"), None).unwrap();
    assert_eq!(p_one.cores.len(), 1);
    gamebub_install::execute_install(
        &card,
        &multi_zip,
        Some("apolkosnik.Minimig"),
        None,
        &p_one.id,
        &backup,
    )
    .unwrap();
    assert!(card.join("cores/apolkosnik.Minimig/core.json").is_file());
    assert!(!card.join("cores/apolkosnik.Minimig040/core.json").exists());

    // 3. Remove with verified backup
    let rm_plan = gamebub_install::plan_remove(&card, "apolkosnik.Minimig").unwrap();
    let rm_rep =
        gamebub_install::execute_remove(&card, "apolkosnik.Minimig", &rm_plan.id, &backup).unwrap();
    assert_eq!(rm_rep.files_removed, 2);
    assert!(!card.join("cores/apolkosnik.Minimig").exists());
    assert!(
        rm_rep
            .backup_dir
            .unwrap()
            .join("cores/apolkosnik.Minimig/mm.bit")
            .is_file()
    );
}
