//! Read-only proof of the Cores section's engine against a real Pocket card (`TAU_REAL_CORES_CARD=/Volumes/<card>`): the platform tree,
//! one core's details, files, saves, settings and screenshots. Nothing is written. Run with
//! `TAU_REAL_CORES_CARD=/Volumes/Pock cargo test -p tau-core --test real_card_cores -- --ignored --nocapture`.

use std::path::PathBuf;
use tau_core::{core_files, core_screens, core_settings, corehub};

fn card() -> Option<PathBuf> {
    std::env::var_os("TAU_REAL_CORES_CARD")
        .map(PathBuf::from)
        .filter(|p| p.join("Cores").is_dir())
}

#[test]
#[ignore]
fn the_real_card_reads_cleanly_end_to_end() {
    let Some(card) = card() else { return };
    let tree = corehub::platform_tree(&card).unwrap();
    let cores: usize = tree.iter().map(|n| n.cores.len()).sum();
    println!("{} platforms, {cores} cores", tree.len());
    for n in tree.iter().take(8) {
        println!(
            "  {:<28} {:<14} {} core(s): {}",
            n.platform.name,
            n.platform.id,
            n.cores.len(),
            n.cores
                .iter()
                .map(|c| c.shortname.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    assert!(cores > 0);
    let mut checked = 0;
    for node in &tree {
        for c in &node.cores {
            let d = corehub::core_detail(&card, &c.id).unwrap_or_else(|e| panic!("{}: {e}", c.id));
            let f =
                core_files::core_files(&card, &c.id).unwrap_or_else(|e| panic!("{}: {e}", c.id));
            let s = core_settings::core_settings(&card, &c.id)
                .unwrap_or_else(|e| panic!("{}: {e}", c.id));
            let shots =
                core_screens::screenshots(&card, &c.id).unwrap_or_else(|e| panic!("{}: {e}", c.id));
            checked += 1;
            if c.id == "Spiritualized.GBA" || c.id == "Spiritualized.GB" {
                let files: usize = f
                    .folders
                    .iter()
                    .filter(|l| l.folder.rel_path.starts_with("Assets"))
                    .map(|l| l.total_files)
                    .sum();
                let saves: usize = f
                    .folders
                    .iter()
                    .filter(|l| l.folder.rel_path.starts_with("Saves"))
                    .map(|l| l.total_files)
                    .sum();
                let states = f
                    .folders
                    .iter()
                    .find(|l| l.folder.rel_path.starts_with("Memories"))
                    .map_or(0, |l| l.total_files);
                println!(
                    "{} {}: {} asset files, {saves} saves, {states} states, {} saved settings, {} presets, {} likely screenshots of {}, missing: {:?}",
                    d.id,
                    d.version,
                    files,
                    s.values.len(),
                    s.presets,
                    shots.likely,
                    shots.total,
                    f.missing
                        .iter()
                        .map(|m| m.filename.as_str())
                        .collect::<Vec<_>>()
                );
                for v in s.values.iter().take(4) {
                    println!(
                        "    {} = {} (default {})",
                        v.name, v.display, v.default_display
                    );
                }
            }
        }
    }
    println!(
        "{checked} cores read without an error; Pocket Sync backups: {}",
        core_files::save_backups(&card).len()
    );
}
