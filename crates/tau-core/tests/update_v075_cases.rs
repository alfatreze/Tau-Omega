//! Tau Alpha's v0.7.5 update-flow test sheet (`docs/interop/rehearsal-v0.7.5/README.md` in their repository, message 034, A034/R-1): a card with
//! TAU 0.7.1 and each state of the per-meter settings file is updated to the real v0.7.5 with Omega's own install code, and what must be true
//! afterwards is checked. Real files, scratch cards only: it reads Tau Alpha's local `release/` folder (or `TAU_REAL_RELEASE_DIR`) and is skipped
//! when that is absent (as in CI). The settings and meter-bin fixtures are Omega's pinned copies (`testdata/meter_settings`, `testdata/meter_bin`).

use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tau_core::{
    compat, install_exec, install_plan, meter_bin,
    meter_settings::{self, FileState},
};

const SETTINGS: &str = "Saves/tau/alfatreze.TAU/tau-meter-settings.bin";
const BIN: &str = "Assets/tau/common/tau-meters.bin";

fn release_dir() -> Option<PathBuf> {
    let dir = std::env::var("TAU_REAL_RELEASE_DIR").map_or_else(
        |_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tau-alpha/release"),
        PathBuf::from,
    );
    (dir.join("alfatreze.TAU_0.7.1_2026-11-11.zip").is_file()
        && dir.join("alfatreze.TAU_0.7.5_2026-11-23.zip").is_file()
        && dir.join("tau-compat.json").is_file())
    .then_some(dir)
}
fn fixture(dir: &str, name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(dir)
            .join(name),
    )
    .unwrap()
}
fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn scratch(tag: &str) -> PathBuf {
    let c = std::env::temp_dir().join(format!("tau-v075-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&c);
    fs::create_dir_all(c.join("Cores")).unwrap();
    fs::create_dir_all(c.join("Assets")).unwrap();
    c
}
fn put(card: &Path, rel: &str, bytes: &[u8]) {
    let p = card.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}
fn install(zip: &Path, card: &Path, docs: &[compat::CompatDoc]) {
    let plan = install_plan::plan(zip, card, docs, false).unwrap();
    assert!(plan.refused.is_none(), "{:?}", plan.refused);
    let backups = card.with_extension("backups");
    let report = install_exec::execute(zip, card, &plan, &plan.id, &backups, docs).unwrap();
    assert!(report.ok(), "a post-install check failed");
}
fn core_version(card: &Path) -> String {
    let j: serde_json::Value =
        serde_json::from_slice(&fs::read(card.join("Cores/alfatreze.TAU/core.json")).unwrap())
            .unwrap();
    j["core"]["metadata"]["version"]
        .as_str()
        .unwrap()
        .to_string()
}
fn category(card: &Path) -> String {
    let j: serde_json::Value =
        serde_json::from_slice(&fs::read(card.join("Platforms/tau.json")).unwrap()).unwrap();
    j["platform"]["category"].as_str().unwrap().to_string()
}

/// A card with the real TAU 0.7.1 installed, `extra` files added, then updated to the real v0.7.5. Returns the card.
fn updated(
    tag: &str,
    dir: &Path,
    extra: &[(&str, Vec<u8>)],
) -> (PathBuf, Vec<meter_settings::FoundFile>) {
    let doc = compat::parse_compat(&fs::read(dir.join("tau-compat.json")).unwrap()).unwrap();
    let card = scratch(tag);
    install(&dir.join("alfatreze.TAU_0.7.1_2026-11-11.zip"), &card, &[]);
    assert_eq!(core_version(&card), "0.7.1");
    assert_eq!(category(&card), "Media Players", "0.7.1 says Media Players");
    for (rel, bytes) in extra {
        put(&card, rel, bytes);
    }
    let before = meter_settings::scan_card(&card);
    install(
        &dir.join("alfatreze.TAU_0.7.5_2026-11-23.zip"),
        &card,
        std::slice::from_ref(&doc),
    );
    (card, before)
}

#[test]
fn case_1_no_settings_file_updates_and_omega_creates_none_and_the_category_becomes_media() {
    let Some(dir) = release_dir() else { return };
    let (card, before) = updated("c1", &dir, &[]);
    assert!(before.is_empty());
    assert_eq!(core_version(&card), "0.7.5");
    assert!(
        !card.join("Saves").exists(),
        "Omega created nothing under Saves/"
    );
    assert_eq!(category(&card), "Media");
}

#[test]
fn case_2_a_saved_settings_file_is_byte_identical_after_the_update() {
    let Some(dir) = release_dir() else { return };
    let file = fixture("meter_settings", "three_records.bin");
    assert!(sha(&file).starts_with("82086c06ce0ba19b"));
    let (card, before) = updated("c2", &dir, &[(SETTINGS, file.clone())]);
    assert_eq!(core_version(&card), "0.7.5");
    assert_eq!(fs::read(card.join(SETTINGS)).unwrap(), file);
    assert_eq!(before[0].state, FileState::Saved(3));
    assert_eq!(
        meter_settings::scan_card(&card),
        before,
        "the scan sees the same file before and after"
    );
    let saves: Vec<_> = walk(&card.join("Saves"));
    assert_eq!(
        saves,
        [SETTINGS.to_string()],
        "nothing else under Saves/ was touched or made"
    );
}

#[test]
fn case_3_a_newer_version_file_is_left_alone_and_not_used() {
    let Some(dir) = release_dir() else { return };
    let file = fixture("meter_settings", "newer_version.bin");
    assert!(sha(&file).starts_with("fe45bff3db05260b"));
    let (card, before) = updated("c3", &dir, &[(SETTINGS, file.clone())]);
    assert_eq!(fs::read(card.join(SETTINGS)).unwrap(), file);
    assert_eq!(
        before[0].state,
        FileState::Newer,
        "Omega can say a newer-version file is present (TAU will ignore it)"
    );
    let parsed = meter_settings::parse(&file);
    assert!(
        parsed.records.is_empty() && parsed.active.is_none(),
        "a reader that knows version 1 does not use that file"
    );
    let doc = compat::parse_compat(&fs::read(dir.join("tau-compat.json")).unwrap()).unwrap();
    assert_eq!(doc.meter_settings_version, Some(1));
}

#[test]
fn case_4_a_wrong_size_file_is_warned_about_first_never_rewritten_and_can_be_deleted_with_a_plan_and_a_backup()
 {
    let Some(dir) = release_dir() else { return };
    let file = fixture("meter_settings", "wrong_size_8191.bin");
    assert!(sha(&file).starts_with("ff35516a2d6e7778"));
    let (card, before) = updated("c4", &dir, &[(SETTINGS, file.clone())]);
    // the warning exists before the update and after it; the update itself does not pad, truncate or rewrite the file
    assert_eq!(
        (before[0].state, before[0].bytes),
        (FileState::WrongSize, 8191)
    );
    assert_eq!(fs::read(card.join(SETTINGS)).unwrap(), file);
    assert_eq!(
        meter_settings::scan_card(&card)[0].state,
        FileState::WrongSize
    );
    // the one fix Omega offers: a plan, a confirmation, a verified backup outside the card, then exactly that file goes
    let plan = meter_settings::plan_delete(&card, SETTINGS).unwrap();
    assert!(plan.blocked.is_none());
    let backup = card.with_extension("sbackup");
    let report = meter_settings::execute_delete(&card, SETTINGS, &plan.id, &backup).unwrap();
    assert_eq!(fs::read(report.backup_dir.join(SETTINGS)).unwrap(), file);
    assert!(!card.join(SETTINGS).exists());
    assert_eq!(
        core_version(&card),
        "0.7.5",
        "the core itself is untouched by the delete"
    );
}

#[test]
fn case_5_a_zero_or_damaged_file_is_left_untouched_and_reads_as_defaults_or_the_older_generation() {
    let Some(dir) = release_dir() else { return };
    let zero = fixture("meter_settings", "zero_seed.bin");
    let (card, before) = updated("c5a", &dir, &[(SETTINGS, zero.clone())]);
    assert_eq!(fs::read(card.join(SETTINGS)).unwrap(), zero);
    assert_eq!(before[0].state, FileState::Defaults);
    assert_eq!(
        meter_settings::parse(&zero).status,
        meter_settings::Status::None
    );
    let damaged = fixture("meter_settings", "damaged_newest.bin");
    assert!(sha(&damaged).starts_with("136cc7103f9e9cd8"));
    let (card, before) = updated("c5b", &dir, &[(SETTINGS, damaged.clone())]);
    assert_eq!(fs::read(card.join(SETTINGS)).unwrap(), damaged);
    let p = meter_settings::parse(&damaged);
    assert_eq!(
        (p.status, p.active, p.records.len()),
        (meter_settings::Status::Ok, Some(1), 2),
        "the older generation is used"
    );
    assert_eq!(before[0].state, FileState::Saved(2));
}

#[test]
fn case_6_a_user_meter_bin_is_untouched_by_the_update_and_read_by_the_v2_reader() {
    let Some(dir) = release_dir() else { return };
    let bin = fixture("meter_bin", "v2_seven.bin");
    let (card, _) = updated("c6", &dir, &[(BIN, bin.clone())]);
    assert_eq!(core_version(&card), "0.7.5");
    assert_eq!(
        fs::read(card.join(BIN)).unwrap(),
        bin,
        "it is the user's file"
    );
    let read = fs::read(card.join(BIN)).unwrap();
    let parsed = meter_bin::parse(&read).unwrap();
    assert_eq!(
        (
            parsed.version,
            parsed.rows.len(),
            parsed.rows[0].name.as_str()
        ),
        (2, 7, "WINAMP BARS")
    );
    assert!(
        meter_bin::boot_check(&read, read.len()).is_ok(),
        "the Pocket will use it (it has the fallback row)"
    );
}

#[test]
fn case_7_the_release_manifest_says_what_the_sheet_says() {
    let Some(dir) = release_dir() else { return };
    let bytes = fs::read(dir.join("tau-compat.json")).unwrap();
    let doc = compat::parse_compat(&bytes).unwrap();
    assert_eq!(doc.meter_settings_version, Some(1));
    assert_eq!(
        doc.persist_ids_changed,
        [10, 15, 16, 20, 21, 22, 23, 27, 28, 29, 30, 33]
    );
    assert_eq!(doc.min_omega, "0.6.0");
    let raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for pkg in raw["packages"].as_array().unwrap() {
        let e = pkg["layout"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| {
                e["path"]
                    .as_str()
                    .unwrap()
                    .ends_with("/tau-meter-settings.bin")
            })
            .unwrap();
        assert_eq!(
            (e["role"].as_str(), e["slot"].as_u64(), e["seed"].as_bool()),
            (Some("user"), Some(11), Some(false))
        );
        assert_eq!(
            (
                e["format"]["name"].as_str(),
                e["format"]["version"].as_u64(),
                e["format"]["bytes"].as_u64()
            ),
            (Some("TMSF"), Some(1), Some(8192))
        );
    }
}

/// Not in the sheet: the read-only view over a real card (`tau_core::meters_on_card`) before and after the update. v0.7.1 ignores the user file
/// and runs its own bundle; v0.7.5 declares the data slot and uses the user file when it is sound.
#[test]
fn the_meters_view_tells_who_uses_the_user_file_before_and_after_the_update() {
    use tau_core::meters_on_card::{self, Uses};
    let Some(dir) = release_dir() else { return };
    let bin = fixture("meter_bin", "v2_seven.bin");
    let old = scratch("view-old");
    install(&dir.join("alfatreze.TAU_0.7.1_2026-11-11.zip"), &old, &[]);
    put(&old, BIN, &bin);
    let v = meters_on_card::read(&old);
    let c = &v.cores[0];
    assert_eq!(
        (
            c.core.as_str(),
            c.version.as_str(),
            c.reads_user_file,
            c.uses
        ),
        ("alfatreze.TAU", "0.7.1", false, Uses::OwnBundle),
        "0.7.1 ignores the user file"
    );
    let own = c.bundle.as_ref().unwrap();
    assert_eq!(
        (own.format, own.rows.len(), own.problem.is_none()),
        (1, 7, true),
        "its own bundle is a v1 bundle of seven meters"
    );
    assert_eq!(
        v.user_file.as_ref().unwrap().rows.len(),
        7,
        "the file is still shown, so a person can see it is there"
    );
    let (card, _) = updated("view-new", &dir, &[(BIN, bin)]);
    let v = meters_on_card::read(&card);
    let c = &v.cores[0];
    assert_eq!(
        (c.version.as_str(), c.reads_user_file, c.uses),
        ("0.7.5", true, Uses::UserFile),
        "0.7.5 uses it"
    );
    assert_eq!(
        c.bundle.as_ref().unwrap().format,
        2,
        "and its own bundle is a v2 bundle"
    );
    assert!(
        v.user_file
            .unwrap()
            .rows
            .iter()
            .all(|r| r.unavailable.is_none())
    );
}

/// Not in the sheet, but the release's own file: v0.7.5 ships `tau-packs.bin` as a v2 bin of its seven meters, and Omega's reader reads it.
#[test]
fn the_release_ships_its_seven_meters_as_a_v2_bin_that_omega_reads_and_the_pocket_accepts() {
    use std::io::Read;
    let Some(dir) = release_dir() else { return };
    let zip = fs::File::open(dir.join("alfatreze.TAU_0.7.5_2026-11-23.zip")).unwrap();
    let mut zip = zip::ZipArchive::new(zip).unwrap();
    let mut bin = Vec::new();
    zip.by_name("Assets/tau/alfatreze.TAU/tau-packs.bin")
        .unwrap()
        .read_to_end(&mut bin)
        .unwrap();
    let parsed = meter_bin::parse(&bin).unwrap();
    let names: Vec<&str> = parsed.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        (parsed.version, names),
        (
            2,
            [
                "WINAMP BARS",
                "WINAMP SCOPE",
                "CHLADNI",
                "LAYERED WAVE",
                "VU MASTER",
                "WATERFALL",
                "SOUND MAP"
            ]
            .to_vec()
        )
    );
    assert_eq!(parsed.fallback().unwrap().name, "WINAMP BARS");
    for (i, row) in parsed.rows.iter().enumerate() {
        assert_eq!(
            (row.id_hash, row.params_rev),
            tau_core::persist_layout::SHIPPED_METERS[i],
            "{}",
            row.name
        );
        assert!(meter_bin::pack_image_ok(&bin, row));
    }
    assert!(meter_bin::boot_check(&bin, bin.len()).is_ok());
}

fn walk(dir: &Path) -> Vec<String> {
    fn go(base: &Path, dir: &Path, out: &mut Vec<String>) {
        for e in fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                go(base, &p, out);
            } else {
                out.push(
                    p.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    let mut out = Vec::new();
    go(dir.parent().unwrap(), dir, &mut out);
    out.sort();
    out
}
