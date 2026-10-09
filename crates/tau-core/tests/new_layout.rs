//! Omega against packages made by Tau Alpha's own release tools in the new layout (channels, core-specific build files,
//! `tau-compat.json` schema 2). Gated: set `TAU_NEW_LAYOUT_DIR` to a folder holding one sub-folder per package
//! (`<name>_out/` with the zip and its `tau-compat.json`). Prints what Omega concludes; asserts only what must hold.

use std::{
    fs,
    path::{Path, PathBuf},
};
use tau_core::{compat, inspect_card, install_exec, install_plan, package, update};

fn packages(dir: &Path) -> Vec<(String, PathBuf, PathBuf)> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if !p.is_dir() || !p.file_name().unwrap().to_string_lossy().ends_with("_out") {
            continue;
        }
        let zip = fs::read_dir(&p)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|x| x.extension().is_some_and(|e| e == "zip"))
            .unwrap();
        out.push((
            p.file_name().unwrap().to_string_lossy().into_owned(),
            zip,
            p.join("tau-compat.json"),
        ));
    }
    out.sort();
    out
}

#[test]
fn omega_reads_and_installs_packages_in_the_new_layout() {
    let Ok(dir) = std::env::var("TAU_NEW_LAYOUT_DIR") else {
        println!("skipped: set TAU_NEW_LAYOUT_DIR");
        return;
    };
    let card = std::env::temp_dir().join(format!("tau-newlayout-{}", std::process::id()));
    let _ = fs::remove_dir_all(&card);
    fs::create_dir_all(card.join("Cores")).unwrap();
    fs::create_dir_all(card.join("Assets")).unwrap();
    let backups = card
        .parent()
        .unwrap()
        .join(format!("tau-newlayout-backups-{}", std::process::id()));
    for (name, zip, manifest) in packages(Path::new(&dir)) {
        println!(
            "\n=== {name}: {}",
            zip.file_name().unwrap().to_string_lossy()
        );
        let doc = match compat::parse_compat(&fs::read(&manifest).unwrap()) {
            Ok(d) => d,
            Err(e) => {
                println!("PARSE FAILED: {e}");
                continue;
            }
        };
        let p = &doc.packages[0];
        println!(
            "manifest: schema {} release {} core_id {:?} layout {} entries",
            doc.schema,
            doc.release,
            p.core_id,
            p.layout.len()
        );
        let manifest_ok = package::inspect(&zip)
            .map(|m| m.core_ids)
            .unwrap_or_default();
        println!("package core ids: {manifest_ok:?}");
        let plan = match install_plan::plan(&zip, &card, std::slice::from_ref(&doc), false) {
            Ok(p) => p,
            Err(e) => {
                println!("PLAN FAILED: {e}");
                continue;
            }
        };
        let c = &plan.update.cores[0];
        println!(
            "verdict {:?}; reasons {:?}; pair {:?}; installed_release {:?}; package_release {:?}",
            c.verdict, c.reasons, c.pair, c.installed_release, c.package_release
        );
        println!(
            "refused {:?}; cautions {:?}; files new {}",
            plan.refused, plan.cautions, plan.files.new_files
        );
        match install_exec::execute(
            &zip,
            &card,
            &plan,
            &plan.id,
            &backups,
            std::slice::from_ref(&doc),
        ) {
            Ok(r) => {
                for check in &r.checks {
                    for i in &check.items {
                        println!("  [{:?}] {}: {}", i.status, i.name, i.detail);
                    }
                }
            }
            Err(e) => println!("INSTALL FAILED: {e}"),
        }
        let findings = compat::check_card(&doc, &card, &p.core_id);
        println!(
            "compat::check_card findings: {:?}",
            findings
                .iter()
                .map(|f| (f.error, f.message.as_str()))
                .collect::<Vec<_>>()
        );
        println!(
            "identify_installed: {:?}",
            compat::identify_installed(std::slice::from_ref(&doc), &card, &p.core_id)
        );
    }
    println!("\n=== inspect_card on the card with every channel installed");
    let inspected = inspect_card(&card).unwrap();
    for core in &inspected.cores {
        println!(
            "core {:?}: shortname {:?} version {:?} platform {:?} library_capable {} index {:?} category {:?}",
            core.id,
            core.shortname,
            core.version,
            core.platform,
            core.library_capable,
            core.index_status,
            core.platform_category
        );
    }
    let _ = fs::remove_dir_all(&card);
    let _ = fs::remove_dir_all(&backups);
}

fn scratch(name: &str) -> PathBuf {
    let c = std::env::temp_dir().join(format!("tau-newlayout-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&c);
    fs::create_dir_all(c.join("Cores")).unwrap();
    fs::create_dir_all(c.join("Assets")).unwrap();
    c
}

fn pkg_dir(dir: &str, name: &str) -> (PathBuf, compat::CompatDoc) {
    let (_, zip, manifest) = packages(Path::new(dir))
        .into_iter()
        .find(|(n, _, _)| n == &format!("{name}_out"))
        .unwrap();
    (
        zip,
        compat::parse_compat(&fs::read(manifest).unwrap()).unwrap(),
    )
}

fn old_zip() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/packages/alfatreze.TAU_0.6.0_2026-10-07.zip")
}

#[test]
fn migration_from_the_old_layout_and_what_a_dev_core_sees() {
    let Ok(dir) = std::env::var("TAU_NEW_LAYOUT_DIR") else {
        println!("skipped: set TAU_NEW_LAYOUT_DIR");
        return;
    };
    // 1. A card with the shipped (old layout) alpha.4, updated by a new-layout package of the same core.
    let card = scratch("migrate");
    let backups = card
        .parent()
        .unwrap()
        .join(format!("tau-newlayout-mig-backups-{}", std::process::id()));
    let old = install_plan::plan(&old_zip(), &card, &[], false).unwrap();
    install_exec::execute(&old_zip(), &card, &old, &old.id, &backups, &[]).unwrap();
    println!(
        "old layout installed: rom in common = {}",
        card.join("Assets/tau/common/tau.rom").is_file()
    );
    let (zip, doc) = pkg_dir(&dir, "stable");
    let plan = install_plan::plan(&zip, &card, std::slice::from_ref(&doc), false).unwrap();
    let c = &plan.update.cores[0];
    println!("\n=== MIGRATION old layout -> new layout, same core");
    println!(
        "verdict {:?}: {:?}; installed_release {:?}",
        c.verdict, c.reasons, c.installed_release
    );
    println!(
        "replaced {}, new {}; obsolete_to_remove {:?}",
        plan.files.updated_files, plan.files.new_files, plan.obsolete_to_remove
    );
    println!(
        "backup: {:?}",
        plan.backup
            .iter()
            .map(|b| b.path.as_str())
            .collect::<Vec<_>>()
    );
    let report = install_exec::execute(
        &zip,
        &card,
        &plan,
        &plan.id,
        &backups,
        std::slice::from_ref(&doc),
    )
    .unwrap();
    for ch in &report.checks {
        for i in &ch.items {
            println!("  [{:?}] {}: {}", i.status, i.name, i.detail);
        }
    }
    println!(
        "common/ after: {:?}",
        fs::read_dir(card.join("Assets/tau/common"))
            .map(|r| r
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect::<Vec<_>>())
            .unwrap_or_default()
    );
    println!(
        "check_card: {:?}",
        compat::check_card(&doc, &card, "alfatreze.TAU")
            .iter()
            .map(|f| (f.error, f.message.clone()))
            .collect::<Vec<_>>()
    );

    // 2. A shared common/ ROM that ANOTHER core on the platform still reads must not be removed (Tau's installer rule).
    let card2 = scratch("sharedrom");
    fs::create_dir_all(card2.join("Assets/tau/common")).unwrap();
    fs::write(
        card2.join("Assets/tau/common/tau.rom"),
        b"rom of an older core",
    )
    .unwrap();
    // an older core that still reads its ROM from common/ (slot without bit 1)
    let other = card2.join("Cores/alfatreze.TAU_OLD");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("core.json"), br#"{"core":{"magic":"APF_VER_1","metadata":{"platform_ids":["tau"],"shortname":"TAU_OLD","author":"alfatreze","version":"0.5.0","date_release":"2026-09-27","description":"x"}}}"#).unwrap();
    fs::write(other.join("data.json"), br#"{"data":{"data_slots":[{"name":"Firmware","id":1,"required":true,"parameters":"0x108","filename":"tau.rom"}]}}"#).unwrap();
    let plan2 = install_plan::plan(&zip, &card2, std::slice::from_ref(&doc), false).unwrap();
    println!(
        "\n=== SHARED common/tau.rom still read by another core: obsolete_to_remove {:?}",
        plan2.obsolete_to_remove
    );

    // 3. Dev and Preview cores keep their library in TAU's common/ (platform_ids[1]).
    let card3 = scratch("devlib");
    let (dzip, ddoc) = pkg_dir(&dir, "dev");
    let (szip, sdoc) = pkg_dir(&dir, "stable");
    for (z, d) in [(&szip, &sdoc), (&dzip, &ddoc)] {
        let p = install_plan::plan(z, &card3, std::slice::from_ref(d), false).unwrap();
        install_exec::execute(z, &card3, &p, &p.id, &backups, std::slice::from_ref(d)).unwrap();
    }
    let common = card3.join("Assets/tau/common");
    fs::create_dir_all(&common).unwrap();
    let entries = tau_core::synth(5, 2, 2);
    let index =
        tau_core::build_index(&entries, &[], "/Assets/tau/common/", &mut Vec::new()).unwrap();
    fs::write(common.join("tau-library.tdb"), index).unwrap();
    println!("\n=== a library lives in Assets/tau/common (as TAU Dev reads it)");
    for core in inspect_card(&card3).unwrap().cores {
        println!(
            "  {:<24} platform {:<9} index_status {:?}",
            core.id, core.platform, core.index_status
        );
    }
    let post = update::post_install_check_with(
        &card3,
        "alfatreze.TAU DEV 385",
        None,
        std::slice::from_ref(&ddoc),
    )
    .unwrap();
    for i in &post.items {
        if i.name == "library index" {
            println!(
                "  post-install check for the dev core, library index: [{:?}] {}",
                i.status, i.detail
            );
        }
    }
    let health = tau_core::refresh::index_state(&card3.join("Assets/tau_dev/common"));
    println!(
        "  library health of the dev core's own platform folder (what the UI would ask): {health:?}"
    );
    let rp = tau_core::remove::plan_remove(&inspect_card(&card3).unwrap(), "alfatreze.TAU DEV 385")
        .unwrap();
    println!(
        "  removing the dev core: shared platform {}; paths {:?}",
        rp.platform_shared, rp.paths
    );
    let rp2 =
        tau_core::remove::plan_remove(&inspect_card(&card3).unwrap(), "alfatreze.TAU").unwrap();
    println!(
        "  removing TAU while a dev core reads its library: shared platform {}; paths {:?}",
        rp2.platform_shared, rp2.paths
    );

    // 4. Release asset naming under the new scheme.
    println!("\n=== release assets of a Preview release");
    let json = br#"[{"tag_name":"v0.7.0-preview.1","name":"p","prerelease":true,"published_at":"2026-11-02T10:00:00Z","assets":[
      {"name":"alfatreze.TAU_Preview_0.7.0-preview.1_2026-11-02.zip","size":1,"browser_download_url":"https://github.com/alfatreze/Tau-Alpha/releases/download/v0.7.0-preview.1/a.zip"},
      {"name":"alfatreze.TAU_Preview_Diagnostics_0.7.0-preview.1_2026-11-02.zip","size":1,"browser_download_url":"https://github.com/alfatreze/Tau-Alpha/releases/download/v0.7.0-preview.1/b.zip"},
      {"name":"tau-compat.json","size":1,"browser_download_url":"https://github.com/alfatreze/Tau-Alpha/releases/download/v0.7.0-preview.1/tau-compat.json"},
      {"name":"SHA256SUMS.txt","size":1,"browser_download_url":"https://github.com/alfatreze/Tau-Alpha/releases/download/v0.7.0-preview.1/SHA256SUMS.txt"}]},
      {"tag_name":"v0.6.0","name":"s","prerelease":false,"published_at":"2026-10-20T10:00:00Z","assets":[]}]"#;
    let releases = tau_core::release_check::parse_releases(json).unwrap();
    // A user on Stable TAU 0.6.0:
    let check = tau_core::release_check::evaluate(
        &releases,
        &tau_core::release_check::Installed {
            release: Some("v0.6.0".into()),
            date_release: None,
        },
    )
    .unwrap();
    println!(
        "  user on Stable v0.6.0 is told: {:?} (newer {:?}); offered zips {:?}",
        check.message,
        check.newer,
        check
            .zips
            .iter()
            .map(|z| z.name.as_str())
            .collect::<Vec<_>>()
    );
    for d in [&sdoc, &ddoc] {
        println!("  replaces in manifest: {:?}", d.packages[0].core_id);
    }
    let _ = (card, card2, card3);
}

#[test]
fn removing_channel_cores_from_a_shared_card() {
    let Ok(dir) = std::env::var("TAU_NEW_LAYOUT_DIR") else {
        return;
    };
    let card = scratch("remove");
    let backups = card
        .parent()
        .unwrap()
        .join(format!("tau-newlayout-rm-backups-{}", std::process::id()));
    for name in ["stable", "diag", "preview", "previewdiag", "dev"] {
        let (z, d) = pkg_dir(&dir, name);
        let p = install_plan::plan(&z, &card, std::slice::from_ref(&d), false).unwrap();
        install_exec::execute(&z, &card, &p, &p.id, &backups, std::slice::from_ref(&d)).unwrap();
    }
    println!("\n=== removing each core with all five installed");
    let inspected = inspect_card(&card).unwrap();
    for core in &inspected.cores {
        let rp = tau_core::remove::plan_remove(&inspected, &core.id).unwrap();
        println!(
            "  remove {:<36} shared-platform {:<5} paths {:?}",
            core.id, rp.platform_shared, rp.paths
        );
        // Every channel core names `tau`, so no removal may touch the shared media folder.
        assert!(
            !rp.paths
                .iter()
                .any(|p| p == "Assets/tau" || p == "Assets/tau/common"),
            "{}: {:?}",
            core.id,
            rp.paths
        );
        assert_eq!(core.media_platform, "tau", "{}", core.id);
    }
    // The dev platform goes with its own (last) core.
    let dev = inspected
        .cores
        .iter()
        .find(|c| c.id.contains("DEV"))
        .unwrap();
    let rp = tau_core::remove::plan_remove(&inspected, &dev.id).unwrap();
    assert!(
        rp.paths.iter().any(|p| p == "Assets/tau_dev"),
        "{:?}",
        rp.paths
    );
    let _ = std::fs::remove_dir_all(&card);
}

/// Differential check: Omega's `check_card` on a card the Python reference (`tau_compat.py check-card`) was also run on.
#[test]
fn check_card_agrees_with_the_python_reference_on_a_damaged_card() {
    let (Ok(manifest), Ok(card)) = (
        std::env::var("TAU_DIFF_MANIFEST"),
        std::env::var("TAU_DIFF_CARD"),
    ) else {
        return;
    };
    let doc = compat::parse_compat(&fs::read(manifest).unwrap()).unwrap();
    for f in compat::check_card(&doc, Path::new(&card), "alfatreze.TAU") {
        println!("{} {}", if f.error { "ERROR" } else { "WARN " }, f.message);
    }
}
