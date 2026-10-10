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
