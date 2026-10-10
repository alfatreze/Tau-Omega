//! Real-card proof for the removal that keeps your data and for settings migration (adaptation step E).
//! Gated and `#[ignore]`d: nothing runs unless `TAU_REAL_E_CARD` names a folder called `CARDWRITE`.
//!
//! - `TAU_REAL_E_CARD`  the card root, which must be `.../CARDWRITE`
//! - `TAU_REAL_E_MODE`  one of
//!   `snapshot`      write `hash  path` lines for the card (needs `TAU_REAL_E_SNAPSHOT=<file>`), read only
//!   `remove-plan`   plan removing `alfatreze.TAU` with and without keep-media; proves the card did not change
//!   `remove-keep`   plant two small "music" files in `Assets/tau/common`, remove `alfatreze.TAU` keeping media, check the
//!   core is gone and the planted files are byte-identical, then delete the planted files
//!   `remove-all`    remove `alfatreze.TAU` and its whole platform (the old behaviour)
//!   `migrate`       plant `alfatreze.TAU_OMEGA_E_OLD` + settings, plan/execute/rollback a migration to
//!   `alfatreze.TAU_OMEGA_E_NEW` with a hand-made manifest, then remove everything it planted
//!
//! `remove-*` need `alfatreze.TAU` installed first (the existing `real_card_install_run` does that).

use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tau_core::{compat, inspect_card, remove, settings_migrate};

const OS_OWNED: [&str; 6] = [
    ".Spotlight-V100",
    ".Trashes",
    ".fseventsd",
    "System Volume Information",
    ".metadata_never_index",
    "FOUND.000",
];

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn snapshot(root: &Path) -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            if OS_OWNED.contains(&name.as_str()) || name.starts_with("._") {
                continue;
            }
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, out);
            } else if let Ok(bytes) = fs::read(&p) {
                out.push((
                    p.strip_prefix(root).unwrap().to_string_lossy().into_owned(),
                    sha(&bytes),
                ));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

fn untouched(before: &[(String, String)], after: &[(String, String)], allowed_prefixes: &[&str]) {
    let strip = |v: &[(String, String)]| -> Vec<(String, String)> {
        v.iter()
            .filter(|(p, _)| !allowed_prefixes.iter().any(|a| p.starts_with(a)))
            .cloned()
            .collect()
    };
    assert_eq!(
        strip(before),
        strip(after),
        "files outside the test's own areas changed"
    );
}

#[test]
#[ignore]
fn real_card_e_run() {
    let (Ok(card), Ok(mode)) = (
        std::env::var("TAU_REAL_E_CARD"),
        std::env::var("TAU_REAL_E_MODE"),
    ) else {
        println!("skipped: set TAU_REAL_E_CARD and TAU_REAL_E_MODE");
        return;
    };
    let card = PathBuf::from(card);
    assert_eq!(
        card.file_name().and_then(|n| n.to_str()),
        Some("CARDWRITE"),
        "refusing: this test only runs on a volume called CARDWRITE"
    );
    let before = snapshot(&card);
    match mode.as_str() {
        "snapshot" => {
            let file = std::env::var("TAU_REAL_E_SNAPSHOT").expect("TAU_REAL_E_SNAPSHOT");
            let text: String = before.iter().map(|(p, h)| format!("{h}  {p}\n")).collect();
            fs::write(&file, text).unwrap();
            println!("{} files written to {file}", before.len());
        }
        "remove-plan" => {
            let inspected = inspect_card(&card).unwrap();
            for keep in [true, false] {
                let plan =
                    remove::plan_remove_with(&inspected, "alfatreze.TAU", keep, &[]).unwrap();
                println!(
                    "keep_media={keep}: {} files, {} bytes, media_kept={}, platform_shared={}",
                    plan.files_to_remove,
                    plan.bytes_to_remove,
                    plan.media_kept,
                    plan.platform_shared
                );
                for p in &plan.paths {
                    println!("  {p}");
                }
            }
            untouched(&before, &snapshot(&card), &[]);
            println!("PLAN ONLY: the card is byte-identical to before");
        }
        "remove-keep" => {
            let common = card.join("Assets/tau/common");
            fs::create_dir_all(&common).unwrap();
            let planted = [
                ("omega-e-song-1.bin", vec![7u8; 4096]),
                ("tau-library.tdb", vec![9u8; 512]),
            ];
            for (name, bytes) in &planted {
                fs::write(common.join(name), bytes).unwrap();
            }
            let inspected = inspect_card(&card).unwrap();
            let plan = remove::plan_remove_with(&inspected, "alfatreze.TAU", true, &[]).unwrap();
            assert!(plan.media_kept, "media must be reported as kept");
            for p in &plan.paths {
                assert!(
                    p != "Assets/tau" && p != "Assets/tau/common" && !p.starts_with("Platforms"),
                    "{p}"
                );
            }
            println!("plan: {:?}", plan.paths);
            let report = remove::execute_remove(&plan, &plan.id).unwrap();
            println!(
                "removed {} files, {} bytes",
                report.removed_files, report.bytes_removed
            );
            assert!(!card.join("Cores/alfatreze.TAU").exists());
            for (name, bytes) in &planted {
                assert_eq!(
                    fs::read(common.join(name)).unwrap(),
                    *bytes,
                    "{name} changed"
                );
            }
            for (name, _) in &planted {
                fs::remove_file(common.join(name)).unwrap();
            }
            println!(
                "KEEP VERIFIED: the core is gone, the planted library and media files were untouched"
            );
        }
        "remove-all" => {
            let inspected = inspect_card(&card).unwrap();
            let plan = remove::plan_remove(&inspected, "alfatreze.TAU").unwrap();
            assert!(!plan.media_kept);
            println!("plan: {:?}", plan.paths);
            let report = remove::execute_remove(&plan, &plan.id).unwrap();
            println!(
                "removed {} files, {} bytes",
                report.removed_files, report.bytes_removed
            );
            assert!(!card.join("Cores/alfatreze.TAU").exists());
            assert!(!card.join("Assets/tau").exists());
            println!("WHOLE-PLATFORM REMOVAL VERIFIED");
        }
        "migrate" => {
            const OLD: &str = "alfatreze.TAU_OMEGA_E_OLD";
            const NEW: &str = "alfatreze.TAU_OMEGA_E_NEW";
            let old_core = card.join("Cores").join(OLD);
            let settings_dir = card.join("Settings").join(OLD).join("Interact/_core");
            assert!(
                !old_core.exists() && !card.join("Settings").join(OLD).exists(),
                "test namespace is not empty"
            );
            fs::create_dir_all(&old_core).unwrap();
            fs::write(old_core.join("core.json"), b"omega e test core").unwrap();
            fs::create_dir_all(&settings_dir).unwrap();
            let persist = br#"{"interact_persist":{"magic":"APF_VER_1","variables":[{"id":10,"type":"slider_u32","val":7}]}}"#;
            fs::write(settings_dir.join("interact_persist.json"), persist).unwrap();
            let doc = |release: &str, core: &str, replaces: &[&str], registry: &str, hash: &str| {
                let value = serde_json::json!({
                    "schema": 2, "release": release, "date_release": "2026-11-01", "prerelease": false,
                    "packages": [{"zip": "x.zip", "zip_sha256": "00", "core_id": core, "replaces": replaces,
                        "bitstream_sha256": "00", "bitstream_core_version": "4D50331A", "rom_sha256": "00", "cold_sha256": "00",
                        "rom_accepts": [], "rom_needs": [],
                        "layout": [{"path": format!("Cores/{OLD}/core.json"), "role": "owned", "sha256": hash, "slot": null, "required": false}]}],
                    "persist_registry": serde_json::from_str::<serde_json::Value>(registry).unwrap(),
                    "requires_omega": {"min_omega": "0.4.0"}
                });
                compat::parse_compat(&serde_json::to_vec(&value).unwrap()).unwrap()
            };
            let old_hash = sha(b"omega e test core");
            let result = std::panic::catch_unwind(|| {
                let ok_registry = r#"{"10": {"name": "Volume", "meaning": 1, "since": "v0.5.0"}}"#;
                let docs = vec![
                    doc("v0.6.0", OLD, &[], "{}", &old_hash),
                    doc("v0.7.0", NEW, &[OLD], ok_registry, "00"),
                ];
                let plan = settings_migrate::plan(&card, OLD, NEW, &docs).unwrap();
                println!("allowed={} reasons={:?}", plan.allowed, plan.reasons);
                assert!(plan.allowed);
                let report = settings_migrate::execute(&plan, &plan.id).unwrap();
                let copied = fs::read(card.join(&plan.dest)).unwrap();
                assert_eq!(copied, persist, "the copy differs from the source");
                assert_eq!(
                    fs::read(card.join(&plan.source)).unwrap(),
                    persist,
                    "the old file changed"
                );
                println!("copied {} bytes to {}", report.bytes, report.dest);
                let again = settings_migrate::plan(&card, OLD, NEW, &docs).unwrap();
                assert!(!again.allowed, "a second run must refuse to overwrite");
                settings_migrate::rollback(&card, &report).unwrap();
                assert!(
                    !card.join("Settings").join(NEW).exists(),
                    "rollback left the new core's settings behind"
                );
                assert_eq!(fs::read(card.join(&plan.source)).unwrap(), persist);
                // A changed id refuses and names it.
                let changed = r#"{"10": {"name": "Volume", "meaning": 2, "since": "v0.7.0"}}"#;
                let docs = vec![
                    doc("v0.6.0", OLD, &[], "{}", &old_hash),
                    doc("v0.7.0", NEW, &[OLD], changed, "00"),
                ];
                let refused = settings_migrate::plan(&card, OLD, NEW, &docs).unwrap();
                assert!(
                    !refused.allowed && refused.changed_ids == vec![(10, "Volume".to_string())]
                );
                assert!(settings_migrate::execute(&refused, &refused.id).is_err());
                println!(
                    "MIGRATION VERIFIED: copy identical, old file untouched, overwrite refused, rollback clean, changed id refused"
                );
            });
            // Always remove what this mode planted.
            let _ = fs::remove_dir_all(&old_core);
            let _ = fs::remove_dir_all(card.join("Settings").join(OLD));
            let _ = fs::remove_dir_all(card.join("Settings").join(NEW));
            if let Err(e) = result {
                std::panic::resume_unwind(e);
            }
        }
        other => panic!("unknown mode {other}"),
    }
    if mode == "migrate" {
        untouched(&before, &snapshot(&card), &[]);
        println!("CARD UNCHANGED: every file matches the snapshot taken before the run");
    }
}
