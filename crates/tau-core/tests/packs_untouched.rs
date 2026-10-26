//! A Tau core's build-bound files (`tau-packs.bin` = data slot 9, `tau.rom`, `tau-cold.bin`, `tau-loading.bin` in
//! `Assets/<platform>/<core id>/`) belong to the release that was installed: an update that does not ship one leaves it alone,
//! the file tools refuse to delete or add them by hand, and only a package that ships them replaces them (Tau Alpha's rule:
//! "Omega reads it, never writes it, never merges it into tau-assets.bin, never deletes it").

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tau_core::{
    core_add::{self, AddState, Destination},
    core_remove, install_exec, install_plan,
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const ID: &str = "alfatreze.TAU";

fn scratch(name: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!(
        "tau-packs-{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(&d).unwrap();
    d
}

/// A Tau-shaped package: core.json on platform `tau`, data slots 1 (firmware) and 9 (meter packs, core-specific, optional).
fn tau_zip(dir: &Path, version: &str, rom: &[u8], packs: Option<&[u8]>) -> PathBuf {
    let path = dir.join(format!(
        "alfatreze.TAU_{version}_2026-10-2{}.zip",
        u8::from(packs.is_some())
    ));
    let core = format!(
        r#"{{"core":{{"magic":"APF_VER_1","metadata":{{"platform_ids":["tau"],"shortname":"TAU","author":"alfatreze","description":"x","version":"{version}","date_release":"2026-10-21"}},"framework":{{"target_product":"Analogue Pocket","version_required":"1.1","dock":{{"supported":true}}}},"cores":[{{"name":"default","id":0,"filename":"bitstream.rbf_r"}}]}}}}"#
    );
    let data = r#"{"data":{"magic":"APF_VER_1","data_slots":[{"name":"Firmware","id":1,"required":true,"parameters":"0x2","filename":"tau.rom"},{"name":"Meter packs","id":9,"required":false,"deferload":true,"parameters":"0x2","filename":"tau-packs.bin"}]}}"#;
    let mut w = ZipWriter::new(fs::File::create(&path).unwrap());
    let o = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut files: Vec<(String, Vec<u8>)> = vec![
        (format!("Cores/{ID}/core.json"), core.into_bytes()),
        (format!("Cores/{ID}/bitstream.rbf_r"), vec![7u8; 4096]),
        (format!("Cores/{ID}/data.json"), data.as_bytes().to_vec()),
        (format!("Assets/tau/{ID}/tau.rom"), rom.to_vec()),
        ("Platforms/tau.json".into(), br#"{"platform":{"category":"Media Players","name":"TAU","manufacturer":"alfatreze","year":2026}}"#.to_vec()),
    ];
    if let Some(p) = packs {
        files.push((format!("Assets/tau/{ID}/tau-packs.bin"), p.to_vec()));
    }
    for (name, bytes) in files {
        w.start_file(name, o).unwrap();
        w.write_all(&bytes).unwrap();
    }
    w.finish().unwrap();
    path
}

fn install(zip: &Path, card: &Path, backups: &Path) {
    let plan = install_plan::plan(zip, card, &[], true).unwrap();
    assert!(plan.refused.is_none(), "{:?}", plan.refused);
    install_exec::execute(zip, card, &plan, &plan.id, backups, &[]).unwrap();
}

#[test]
fn the_meter_packs_file_is_left_alone_unless_a_release_ships_it() {
    let (d, card, bk) = (scratch("zips"), scratch("card"), scratch("bk"));
    let packs_rel = format!("Assets/tau/{ID}/tau-packs.bin");
    let rom_rel = format!("Assets/tau/{ID}/tau.rom");
    let (p1, p2) = (vec![0xA1u8; 3000], vec![0xB2u8; 3500]);

    // A release that ships the packs installs them.
    install(&tau_zip(&d, "0.7.0", b"rom 1", Some(&p1)), &card, &bk);
    assert_eq!(fs::read(card.join(&packs_rel)).unwrap(), p1);

    // An update whose package has no tau-packs.bin neither deletes nor changes it.
    install(&tau_zip(&d, "0.7.1", b"rom 2", None), &card, &bk);
    assert_eq!(
        fs::read(card.join(&rom_rel)).unwrap(),
        b"rom 2",
        "the firmware is replaced by the package that ships it"
    );
    assert_eq!(
        fs::read(card.join(&packs_rel)).unwrap(),
        p1,
        "tau-packs.bin survives an update that does not ship it"
    );

    // The file tools refuse the build-bound files, for packs and firmware alike.
    for rel in [&packs_rel, &rom_rel] {
        let err = core_remove::plan_remove_files(&card, ID, std::slice::from_ref(rel)).unwrap_err();
        assert!(
            err.message.contains("part of alfatreze.TAU's build"),
            "{rel}: {}",
            err.message
        );
    }
    assert_eq!(fs::read(card.join(&packs_rel)).unwrap(), p1);

    // Adding a file of the same name by hand is refused, even when replacing is allowed.
    let mine = scratch("mine").join("tau-packs.bin");
    fs::write(&mine, b"my own packs").unwrap();
    for replace in [false, true] {
        let plan = core_add::plan_add(
            &card,
            ID,
            "tau",
            Destination::CoreOnly,
            std::slice::from_ref(&mine),
            replace,
        )
        .unwrap();
        assert!(
            plan.items
                .iter()
                .all(|i| matches!(i.state, AddState::Refused(_))),
            "replace={replace}: {:?}",
            plan.items.iter().map(|i| &i.state).collect::<Vec<_>>()
        );
    }
    assert_eq!(fs::read(card.join(&packs_rel)).unwrap(), p1);

    // A later release that ships new packs replaces them (the file is the build's own).
    install(&tau_zip(&d, "0.7.2", b"rom 3", Some(&p2)), &card, &bk);
    assert_eq!(fs::read(card.join(&packs_rel)).unwrap(), p2);
}

#[test]
fn another_cores_files_of_the_same_name_are_not_protected() {
    // The rule is for Tau's own build files; a community core's own-named files are the user's to manage.
    assert!(tau_core::core_update::is_build_bound_file(
        "alfatreze.TAU Preview",
        "tau-packs.bin"
    ));
    assert!(tau_core::core_update::is_build_bound_file(
        "alfatreze.TAU DEV 184",
        "TAU.ROM"
    ));
    assert!(!tau_core::core_update::is_build_bound_file(
        "agg23.Arduboy",
        "tau.rom"
    ));
    assert!(!tau_core::core_update::is_build_bound_file(
        "alfatreze.TAU",
        "song.mp3"
    ));
}
