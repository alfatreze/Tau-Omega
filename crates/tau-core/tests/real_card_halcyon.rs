//! Real-card proof of the Halcyon `PRST` install. Gated and `#[ignore]`d: nothing runs unless `TAU_REAL_H_CARD` names a folder
//! called `CARDWRITE`. Writes two test presets (a control preset and an APO-style raw preset) to `Assets/tau/common/tau-assets.bin`
//! through the reviewed install path (verified backup outside the card, read-back), then prints the file's hash for the log.
//! `TAU_REAL_H_BACKUP` is a backup folder outside the card. Run with `--ignored --nocapture`, then boot the Pocket.
use std::path::Path;
use tau_core::assets::{AssetsEdit, execute_install_edit, plan_install_edit, read_presets};
use tau_core::halcyon::{HalcyonPreset, import_apo};

#[test]
#[ignore]
fn real_card_halcyon_install() {
    let card = std::env::var("TAU_REAL_H_CARD").expect("TAU_REAL_H_CARD");
    let backup = std::env::var("TAU_REAL_H_BACKUP").expect("TAU_REAL_H_BACKUP");
    let card = Path::new(&card);
    assert_eq!(
        card.file_name().and_then(|n| n.to_str()),
        Some("CARDWRITE"),
        "refusing: only a volume called CARDWRITE"
    );
    let media = card.join("Assets/tau/common");
    let apo = "Preamp: -4.0 dB\nFilter 1: ON LSC Fc 105 Hz Gain 4.0 dB Q 0.70\nFilter 2: ON PK Fc 3000 Hz Gain -3.0 dB Q 1.50\nFilter 3: ON HSC Fc 9000 Hz Gain 2.0 dB Q 0.70\n";
    let raw = import_apo("OMEGA APO", apo).unwrap();
    println!(
        "APO import: {} filters, preamp {} dB, peak boost {} dB",
        raw.filters, raw.preamp_db, raw.peak_gain_db
    );
    let presets = vec![
        HalcyonPreset::Control {
            name: "OMEGA CTRL".into(),
            controls: [2, 3, 0, 1, 2, 1],
        },
        raw.preset,
    ];
    let plan = plan_install_edit(AssetsEdit::presets(&presets), &media).unwrap();
    println!(
        "plan: {} bytes, presets {:?}, warnings {:?}",
        plan.bytes, plan.presets, plan.warnings
    );
    let report = execute_install_edit(
        AssetsEdit::presets(&presets),
        &media,
        &plan,
        &plan.id,
        Some(Path::new(&backup)),
    )
    .unwrap();
    println!(
        "written {} bytes to {}",
        report.bytes_written,
        report.destination.display()
    );
    let back = read_presets(&std::fs::read(&report.destination).unwrap()).unwrap();
    assert_eq!(back, presets);
    println!(
        "read back OK: {:?}",
        back.iter().map(|p| p.name()).collect::<Vec<_>>()
    );
}

/// Syncs a folder on the card into `Assets/tau/common` and builds `tau-library.tdb` there (the app's sync path, journalled outside
/// the card). `TAU_REAL_H_SYNC_SRC` is the source folder; `TAU_REAL_H_JOURNAL` a file path outside the card. Same CARDWRITE guard.
#[test]
#[ignore]
fn real_card_halcyon_sync() {
    let card = std::env::var("TAU_REAL_H_CARD").expect("TAU_REAL_H_CARD");
    let src = std::env::var("TAU_REAL_H_SYNC_SRC").expect("TAU_REAL_H_SYNC_SRC");
    let journal = std::env::var("TAU_REAL_H_JOURNAL").expect("TAU_REAL_H_JOURNAL");
    let card = Path::new(&card);
    assert_eq!(
        card.file_name().and_then(|n| n.to_str()),
        Some("CARDWRITE"),
        "refusing: only a volume called CARDWRITE"
    );
    assert!(
        !Path::new(&journal).starts_with(card),
        "journal must be outside the card"
    );
    let common = card.join("Assets/tau/common");
    let root = tau_core::root_prefix(&common).unwrap();
    let opts = tau_core::sync::PlanOptions {
        mirror: false,
        embed_covers: false,
        art_sidecar_pal256: false,
    };
    let plan = tau_core::sync::plan(
        &[Path::new(&src).to_path_buf()],
        &common,
        &root,
        opts,
        &mut None,
    )
    .unwrap();
    println!(
        "plan {}: {} items, {} bytes, root {}, warnings {:?}",
        plan.id,
        plan.items.len(),
        plan.bytes_to_write,
        plan.root_prefix,
        plan.warnings
    );
    for i in &plan.items {
        println!("  {}", i.destination.strip_prefix(card).unwrap().display());
    }
    let report = tau_core::journal::execute_to_journal(
        &plan,
        &plan.id,
        "sync",
        Path::new(&journal),
        &mut None,
    )
    .unwrap();
    println!(
        "copied {} unchanged {} bytes {} index {} ({})",
        report.copied,
        report.unchanged,
        report.bytes_written,
        report.index_path.display(),
        report.index_sha256
    );
}
