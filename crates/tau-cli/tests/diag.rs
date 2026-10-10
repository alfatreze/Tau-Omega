//! `tau diag`: reads a (scratch) card's Check results and screenshots, writes the zip only outside the card.
use std::{fs, path::PathBuf, process::Command};

fn scratch(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("tau-cli-diag-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn card(root: &std::path::Path) {
    let core = root.join("Cores/alfatreze.TAU_TEST");
    fs::create_dir_all(&core).unwrap();
    fs::write(core.join("core.json"), r#"{"core":{"metadata":{"platform_ids":["tau_test"],"shortname":"TAU_TEST","author":"alfatreze","version":"0.6.0","date_release":"2026-09-30"}}}"#).unwrap();
    fs::write(
        core.join("data.json"),
        r#"{"data":{"data_slots":[{"id":5,"filename":"tau-library.tdb"}]}}"#,
    )
    .unwrap();
    fs::write(core.join("bitstream.rbf_r"), b"bitstream bytes").unwrap();
    let shots = root.join("Memories/Screenshots");
    fs::create_dir_all(&shots).unwrap();
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/screenshots/20260927_233104.png");
    fs::copy(src, shots.join("20260927_233104.png")).unwrap();
}

#[test]
fn diag_prints_a_summary_and_writes_the_zip_outside_the_card() {
    let base = scratch("ok");
    let (cardroot, out) = (base.join("card"), base.join("out"));
    card(&cardroot);
    let tau = env!("CARGO_BIN_EXE_tau");
    let r = Command::new(tau)
        .args(["diag", cardroot.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let text = String::from_utf8_lossy(&r.stdout);
    assert!(
        text.contains("TAU_TEST") || text.contains("alfatreze.TAU_TEST"),
        "{text}"
    );
    let r = Command::new(tau)
        .args([
            "diag",
            cardroot.to_str().unwrap(),
            "--zip",
            out.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let zips: Vec<_> = fs::read_dir(&out).unwrap().flatten().collect();
    assert_eq!(zips.len(), 1);
    assert!(
        zips[0]
            .file_name()
            .to_string_lossy()
            .starts_with("tau-diagnostics-")
    );
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn diag_refuses_a_zip_folder_inside_the_card() {
    let base = scratch("inside");
    let cardroot = base.join("card");
    card(&cardroot);
    let inside = cardroot.join("here");
    let r = Command::new(env!("CARGO_BIN_EXE_tau"))
        .args([
            "diag",
            cardroot.to_str().unwrap(),
            "--zip",
            inside.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!r.status.success());
    assert!(!inside.exists(), "nothing may be written inside the card");
    let _ = fs::remove_dir_all(&base);
}
