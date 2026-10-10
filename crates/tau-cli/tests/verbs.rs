//! The app-parity verbs, run as the real binary with `--json`, against real artefacts in `testdata/`.
use std::{fs, path::PathBuf, process::Command};

fn data(rel: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(rel)
        .to_string_lossy()
        .into_owned()
}
fn run(args: &[&str]) -> (bool, serde_json::Value, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_tau"))
        .args(args)
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    (
        o.status.success(),
        serde_json::from_str(&out).unwrap_or(serde_json::Value::Null),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn report_code_reads_both_views_to_the_same_report() {
    let (ok, grid, _) = run(&[
        "report-code",
        &data("screenshots/20261004_194856.png"),
        "--json",
    ]);
    let (ok2, qr, _) = run(&[
        "report-code",
        &data("screenshots/20261004_194907.png"),
        "--json",
    ]);
    assert!(ok && ok2);
    assert_eq!(grid, qr);
    assert_eq!(grid["profile"], "USER CHECK");
}

#[test]
fn settings_and_halcyon_verbs_give_json() {
    let (ok, v, _) = run(&[
        "settings",
        &data("interact_persist/check_all_passed.json"),
        "--json",
    ]);
    assert!(ok);
    assert!(v["settings"].as_array().is_some_and(|s| !s.is_empty()));
    assert!(v["check_summary"].is_object());
    let (ok, v, _) = run(&[
        "halcyon-import",
        &data("halcyon/cans.txt"),
        "--name",
        "CANS",
        "--json",
    ]);
    assert!(ok);
    assert_eq!(
        (v["filters"].as_u64(), v["preamp_db"].as_f64()),
        (Some(4), Some(-6.2))
    );
    assert_eq!(v["preset"]["kind"], "raw");
    let (ok, v, _) = run(&[
        "halcyon-show",
        &data("../crates/tau-core/testdata/assets/sunset.tau-assets.bin"),
        "--json",
    ]);
    assert!(ok);
    assert_eq!(v, serde_json::json!([]));
}

#[test]
fn package_verbs_inspect_and_plan_without_writing() {
    let zip = data("packages/alfatreze.TAU_0.6.0_2026-10-04.zip");
    let (ok, v, _) = run(&["package-inspect", &zip, "--json"]);
    assert!(ok && v.is_object());
    let card = std::env::temp_dir().join(format!("tau-cli-pkg-{}", std::process::id()));
    fs::create_dir_all(&card).unwrap();
    let (ok, v, e) = run(&[
        "package-plan",
        &zip,
        "--card",
        card.to_str().unwrap(),
        "--json",
    ]);
    assert!(ok, "{e}");
    assert!(v["new_files"].as_u64().unwrap() > 0 && v["id"].is_string());
    assert_eq!(
        fs::read_dir(&card).unwrap().count(),
        0,
        "planning wrote something"
    );
    let (ok, _, e) = run(&[
        "package-install",
        &zip,
        "--card",
        card.to_str().unwrap(),
        "--confirm",
        "x",
    ]);
    assert!(!ok && e.contains("--yes"), "{e}");
    let _ = fs::remove_dir_all(&card);
}

#[test]
fn errors_are_json_on_stderr_with_json() {
    let (ok, _, e) = run(&["report-code", "/no/such.png", "--json"]);
    assert!(!ok);
    let v: serde_json::Value = serde_json::from_str(e.trim()).unwrap();
    assert!(
        v["error"]["code"].is_string() && v["error"]["message"].is_string(),
        "{e}"
    );
}
