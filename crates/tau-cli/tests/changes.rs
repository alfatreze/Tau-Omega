//! The card-changing verbs on a scratch card (never a real one): plan first, writes need `--confirm <plan-id> --yes`, backups outside the card.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

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
    let v = serde_json::from_slice(&o.stdout).unwrap_or(serde_json::Value::Null);
    (
        o.status.success(),
        v,
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}
fn scratch(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("tau-cli-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}
/// A scratch card with the 0.6.0 package installed through the CLI itself.
fn installed_card(base: &Path) -> PathBuf {
    let card = base.join("card");
    fs::create_dir_all(&card).unwrap();
    let zip = data("packages/alfatreze.TAU_0.6.0_2026-10-04.zip");
    let (ok, plan, e) = run(&[
        "package-plan",
        &zip,
        "--card",
        card.to_str().unwrap(),
        "--json",
    ]);
    assert!(ok, "{e}");
    let id = plan["id"].as_str().unwrap();
    let (ok, _, e) = run(&[
        "package-install",
        &zip,
        "--card",
        card.to_str().unwrap(),
        "--confirm",
        id,
        "--yes",
        "--json",
    ]);
    assert!(ok, "{e}");
    card
}

#[test]
fn presets_and_themes_install_through_assets_verbs() {
    let base = scratch("assets");
    let card = installed_card(&base);
    let media = card.join("Assets/tau/common");
    let backups = base.join("backups");
    let presets = base.join("presets.json");
    fs::write(
        &presets,
        r#"[{"kind":"control","name":"CLI CTRL","controls":[1,2,0,0,3,-1]}]"#,
    )
    .unwrap();
    let (m, p, b) = (
        media.to_str().unwrap(),
        presets.to_str().unwrap(),
        backups.to_str().unwrap(),
    );
    let (ok, plan, e) = run(&["assets-plan", m, "--presets", p, "--json"]);
    assert!(ok, "{e}");
    assert_eq!(plan["presets"], serde_json::json!(["CLI CTRL"]));
    assert!(
        !media.join("tau-assets.bin").exists(),
        "planning wrote something"
    );
    let id = plan["id"].as_str().unwrap().to_string();
    // refused: no --yes; a backup folder inside the card
    assert!(!run(&["assets-install", m, "--presets", p, "--confirm", &id]).0);
    let inside = card.join("bk");
    assert!(
        !run(&[
            "assets-install",
            m,
            "--presets",
            p,
            "--confirm",
            &id,
            "--backup-dir",
            inside.to_str().unwrap(),
            "--yes"
        ])
        .0
    );
    let (ok, _, e) = run(&[
        "assets-install",
        m,
        "--presets",
        p,
        "--confirm",
        &id,
        "--backup-dir",
        b,
        "--yes",
        "--json",
    ]);
    assert!(ok, "{e}");
    let (ok, shown, _) = run(&[
        "halcyon-show",
        media.join("tau-assets.bin").to_str().unwrap(),
        "--json",
    ]);
    assert!(ok);
    assert_eq!(shown[0]["name"], "CLI CTRL");
    // a theme read from a real file checks clean
    let (ok, themes, _) = run(&[
        "theme-show",
        &data("../crates/tau-core/testdata/assets/sunset.tau-assets.bin"),
        "--json",
    ]);
    assert!(ok);
    let one = base.join("theme.json");
    fs::write(&one, serde_json::to_vec(&themes[0]).unwrap()).unwrap();
    let (ok, report, e) = run(&["theme-check", one.to_str().unwrap(), "--json"]);
    assert!(ok, "{e}");
    assert_eq!(report["problems"], serde_json::json!([]));
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn remove_plans_then_removes_only_with_the_plan_id() {
    let base = scratch("remove");
    let card = installed_card(&base);
    let c = card.to_str().unwrap();
    let (ok, plan, e) = run(&["remove-plan", c, "--core", "alfatreze.TAU", "--json"]);
    assert!(ok, "{e}");
    let id = plan["id"].as_str().unwrap().to_string();
    assert!(
        card.join("Cores/alfatreze.TAU").exists(),
        "planning removed something"
    );
    assert!(
        !run(&["remove", c, "--core", "alfatreze.TAU", "--confirm", &id]).0,
        "needs --yes"
    );
    assert!(
        !run(&[
            "remove",
            c,
            "--core",
            "alfatreze.TAU",
            "--confirm",
            "wrong",
            "--yes"
        ])
        .0,
        "wrong plan id"
    );
    assert!(card.join("Cores/alfatreze.TAU").exists());
    let (ok, _, e) = run(&[
        "remove",
        c,
        "--core",
        "alfatreze.TAU",
        "--confirm",
        &id,
        "--yes",
        "--json",
    ]);
    assert!(ok, "{e}");
    assert!(!card.join("Cores/alfatreze.TAU").exists());
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn update_and_refresh_and_changes_plan_without_writing() {
    let base = scratch("plans");
    let card = installed_card(&base);
    let zip = data("packages/alfatreze.TAU_0.4.0_2026-09-22.zip");
    let (ok, plan, e) = run(&[
        "update-plan",
        &zip,
        "--card",
        card.to_str().unwrap(),
        "--json",
    ]);
    assert!(ok, "{e}");
    assert!(plan["id"].is_string());
    let media = card.join("Assets/tau/common");
    let (ok, _, e) = run(&["library-refresh-plan", media.to_str().unwrap(), "--json"]);
    assert!(ok, "{e}");
    let (ok, _, e) = run(&["changes-plan", media.to_str().unwrap(), "--json"]);
    assert!(
        !ok && e.contains("no changes"),
        "an empty request is refused: {e}"
    );
    let (ok, _, e) = run(&[
        "library-refresh",
        media.to_str().unwrap(),
        "--confirm",
        "x",
        "--backup-dir",
        base.join("bk").to_str().unwrap(),
    ]);
    assert!(!ok && e.contains("--yes"), "{e}");
    let _ = fs::remove_dir_all(&base);
}
