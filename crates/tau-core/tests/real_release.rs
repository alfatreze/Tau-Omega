//! Omega against a REAL published release (gated): set `TAU_REAL_RELEASE_DIR` to a folder holding a release's zips and
//! its `tau-compat.json` (for example Tau's `release/` after `make_release.py`). Installs into a throwaway folder, never
//! a real card.

use std::{fs, path::PathBuf};
use tau_core::{compat, install_exec, install_plan, release_check, update};

#[test]
fn a_real_release_installs_checks_and_is_offered_to_the_right_channel() {
    let Ok(dir) = std::env::var("TAU_REAL_RELEASE_DIR") else {
        println!("skipped: set TAU_REAL_RELEASE_DIR");
        return;
    };
    let dir = PathBuf::from(dir);
    let doc = compat::parse_compat(&fs::read(dir.join("tau-compat.json")).unwrap()).unwrap();
    println!(
        "release {} schema {} packages {}",
        doc.release,
        doc.schema,
        doc.packages.len()
    );
    assert!(
        doc.source_commit.is_some() && !doc.source_dirty,
        "a release must come from a clean tree"
    );
    let docs = std::slice::from_ref(&doc);

    // 1. Each package: zip hash agrees, installs into a scratch card, every check passes, features are judged.
    for package in &doc.packages {
        let zip = dir.join(&package.zip);
        let bytes = fs::read(&zip).unwrap();
        let sums =
            release_check::parse_sums(&fs::read_to_string(dir.join("SHA256SUMS.txt")).unwrap());
        release_check::verify_download(&package.zip, &bytes, &sums, Some(&doc)).unwrap();

        let card = std::env::temp_dir().join(format!(
            "tau-real-release-{}-{}",
            std::process::id(),
            package.core_id.replace(' ', "_")
        ));
        let _ = fs::remove_dir_all(&card);
        fs::create_dir_all(card.join("Cores")).unwrap();
        fs::create_dir_all(card.join("Assets")).unwrap();
        let backups = card.with_extension("backups");
        let plan = install_plan::plan(&zip, &card, docs, false).unwrap();
        assert!(plan.refused.is_none(), "{:?}", plan.refused);
        let core = &plan.update.cores[0];
        println!(
            "{}: {:?}; pair {:?}; release {:?}; notes {:?}",
            package.core_id, core.verdict, core.pair, core.package_release, core.reasons
        );
        assert!(
            matches!(core.pair, update::PairStatus::Verified { .. }),
            "{:?}",
            core.pair
        );
        assert_eq!(core.package_release.as_deref(), Some(doc.release.as_str()));
        let report = install_exec::execute(&zip, &card, &plan, &plan.id, &backups, docs).unwrap();
        for check in &report.checks {
            for item in &check.items {
                println!("  [{:?}] {}: {}", item.status, item.name, item.detail);
            }
        }
        assert!(
            report.ok(),
            "{}: a post-install check failed",
            package.core_id
        );
        let findings = compat::check_card(&doc, &card, &package.core_id);
        assert!(findings.iter().all(|f| !f.error), "{:?}", findings);
        // The installed files are recognised as this release by hash.
        assert_eq!(
            compat::identify_installed(docs, &card, &package.core_id),
            vec![doc.release.clone()]
        );
        // The media platform of a Preview core is `tau`, its build platform its own.
        let inspected = tau_core::inspect_card(&card).unwrap();
        let installed = inspected
            .cores
            .iter()
            .find(|c| c.id == package.core_id)
            .unwrap();
        println!(
            "  platform {} media_platform {} platforms {:?}",
            installed.platform, installed.media_platform, installed.platforms
        );
        let _ = fs::remove_dir_all(&card);
        let _ = fs::remove_dir_all(&backups);
    }

    // 2. The update check with this release's real asset names.
    let assets: Vec<String> = doc.packages.iter().map(|p| p.zip.clone()).collect();
    let json = serde_json::json!([{
        "tag_name": doc.release, "name": doc.release, "prerelease": doc.prerelease,
        "published_at": format!("{}T10:00:00Z", doc.date_release),
        "assets": assets.iter().map(|n| serde_json::json!({"name": n, "size": 1,
            "browser_download_url": format!("{}{}/{}", release_check::DOWNLOAD_PREFIX, doc.release, n)})).collect::<Vec<_>>()
    }]);
    let releases = release_check::parse_releases(json.to_string().as_bytes()).unwrap();
    for package in &doc.packages {
        let installed = release_check::Installed {
            core_id: Some(package.core_id.clone()),
            version: Some("0.6.0-preview.0".into()),
            release: Some("v0.6.0-preview.0".into()),
            date_release: None,
        };
        let check = release_check::evaluate_for_core(&releases, &installed, None, docs).unwrap();
        println!(
            "{}: {} zips {:?}",
            package.core_id,
            check.message,
            check.zips.iter().map(|z| &z.name).collect::<Vec<_>>()
        );
        assert_eq!(check.newer, Some(true));
        assert_eq!(check.zips.len(), 1);
        assert_eq!(check.zips[0].name, package.zip);
    }
    // A Stable user is not offered a Preview core as an update.
    let stable = release_check::Installed {
        core_id: Some("alfatreze.TAU".into()),
        version: Some("0.6.0".into()),
        release: Some("v0.6.0-alpha.3".into()),
        date_release: None,
    };
    let check = release_check::evaluate_for_core(&releases, &stable, None, docs).unwrap();
    println!(
        "stable user: {} newer {:?} zips {} others {:?}",
        check.message,
        check.newer,
        check.zips.len(),
        check.others
    );
    assert_eq!(check.newer, Some(false));
    assert!(check.zips.is_empty() && check.others.len() == 1);

    // 3. Persisted-setting names from this release's registry.
    let names = release_check::persist_names(docs);
    println!("persist names: {names:?}");
    assert_eq!(
        names.get(&16).map(String::as_str),
        Some("Halcyon EQ preset")
    );
}
