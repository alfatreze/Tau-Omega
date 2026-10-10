use super::*;
use crate::update::CATALOG_CACHES;
use std::time::{SystemTime, UNIX_EPOCH};

fn pkg(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/packages")
        .join(name)
}
fn alpha3() -> PathBuf {
    pkg("alfatreze.TAU_0.6.0_2026-10-04.zip")
}
fn alpha4() -> PathBuf {
    pkg("alfatreze.TAU_0.6.0_2026-10-07.zip")
}
fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "tau-installexec-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            + crate::test_uniq()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}
fn card(name: &str) -> PathBuf {
    let c = scratch(name).join("card");
    fs::create_dir_all(c.join("Cores")).unwrap();
    fs::create_dir_all(c.join("Assets")).unwrap();
    c
}
fn backups(card: &Path) -> PathBuf {
    card.parent().unwrap().join("backups")
}
fn install_raw(zip: &Path, card: &Path) {
    let p = package::plan_install(zip, card).unwrap();
    package::execute_install(zip, card, &p, &p.id).unwrap();
}
fn snapshot(root: &Path) -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, out);
            } else {
                out.push((
                    p.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    sync::sha256_bytes(&fs::read(&p).unwrap()),
                ));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

/// A card as it is in real life: an older Tau, the user's library and themes, a second core, stale caches, stubs.
fn lived_in_card(name: &str) -> PathBuf {
    let card = card(name);
    install_raw(&alpha3(), &card);
    let common = card.join("Assets/tau/common");
    let index = crate::build_index(
        &crate::synth(3, 1, 1),
        &[],
        &crate::root_prefix(&common).unwrap(),
        &mut Vec::new(),
    )
    .unwrap();
    fs::write(common.join("tau-library.tdb"), &index).unwrap();
    fs::write(common.join("tau-assets.bin"), b"themes").unwrap();
    fs::write(common.join("song.mp3"), b"music").unwrap();
    fs::create_dir_all(card.join("Cores/other.Core")).unwrap();
    fs::write(card.join("Cores/other.Core/core.json"), b"{}").unwrap();
    fs::create_dir_all(card.join("System")).unwrap();
    for cache in CATALOG_CACHES {
        fs::write(card.join(cache), b"old cache").unwrap();
    }
    fs::write(common.join("._tau.rom"), b"stub").unwrap();
    card
}

#[test]
fn an_update_backs_up_writes_cleans_checks_and_keeps_the_users_files() {
    let card = lived_in_card("update");
    let before = snapshot(&card);
    let index_bytes = fs::read(card.join("Assets/tau/common/tau-library.tdb")).unwrap();
    let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
    assert!(!report.nothing_to_do && report.ok(), "{:#?}", report.checks);
    assert!(report.files_written > 0 && report.files_backed_up >= 7);
    assert_eq!((report.caches_cleared, report.stubs_swept), (5, 1));
    let common = card.join("Assets/tau/common");
    assert_eq!(
        fs::read(common.join("tau-library.tdb")).unwrap(),
        index_bytes
    );
    assert_eq!(fs::read(common.join("tau-assets.bin")).unwrap(), b"themes");
    assert_eq!(fs::read(common.join("song.mp3")).unwrap(), b"music");
    assert!(fs::read(card.join("Cores/other.Core/core.json")).unwrap() == b"{}");
    assert!(CATALOG_CACHES.iter().all(|c| !card.join(c).exists()));
    assert!(!common.join("._tau.rom").exists());
    // The card now is the new build, and the journal says so.
    let again = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    assert!(again.nothing_to_do);
    assert_ne!(snapshot(&card), before);
    let journal: Value =
        serde_json::from_slice(&fs::read(report.backup_dir.join(JOURNAL)).unwrap()).unwrap();
    assert_eq!(journal["state"], "done");
    assert!(report.backup_dir.starts_with(backups(&card)));
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
}

#[test]
fn rollback_puts_the_card_back_exactly_as_it_was() {
    let card = lived_in_card("rollback");
    let before = snapshot(&card);
    let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
    let undone = rollback(&card, &report.backup_dir).unwrap();
    assert!(undone.restored >= 7);
    // Everything except the swept `._` stub (junk, deliberately not restored) is byte-identical to before.
    let after = snapshot(&card);
    let without_stub = |v: &[(String, String)]| {
        v.iter()
            .filter(|(p, _)| !p.contains("/._"))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(without_stub(&after), without_stub(&before));
    // Safe to run twice.
    rollback(&card, &report.backup_dir).unwrap();
    assert_eq!(without_stub(&snapshot(&card)), without_stub(&before));
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
}

#[test]
fn a_first_install_rolls_back_to_an_untouched_card() {
    let card = card("first");
    fs::create_dir_all(card.join("System")).unwrap();
    fs::write(card.join(CATALOG_CACHES[0]), b"old").unwrap();
    let before = snapshot(&card);
    let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
    assert!(card.join("Cores/alfatreze.TAU/core.json").is_file());
    let undone = rollback(&card, &report.backup_dir).unwrap();
    assert_eq!(undone.created_removed, 15);
    assert_eq!(snapshot(&card), before);
    assert!(
        !card.join("Cores/alfatreze.TAU").exists(),
        "folders only the install made are gone too"
    );
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
}

#[test]
fn a_failure_part_way_rolls_back_by_itself() {
    let card = lived_in_card("failure");
    let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    // After planning, the stub the plan will sweep becomes a folder: removing it fails after the files were written.
    let stub = card.join("Assets/tau/common/._tau.rom");
    fs::remove_file(&stub).unwrap();
    fs::create_dir(&stub).unwrap();
    fs::write(stub.join("keep"), b"x").unwrap();
    let before = snapshot(&card);
    let error = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap_err();
    assert!(
        error.message.contains("put back as it was"),
        "{}",
        error.message
    );
    assert_eq!(snapshot(&card), before, "the card is exactly as it was");
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
}

#[test]
fn nothing_is_touched_when_the_checks_before_the_first_write_fail() {
    let card = lived_in_card("guards");
    let before = snapshot(&card);
    let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    // A wrong token.
    assert_eq!(
        execute(&alpha4(), &card, &plan, "nope", &backups(&card), &[])
            .unwrap_err()
            .code(),
        ErrorCode::ConfirmationMismatch
    );
    // A backup on the card itself.
    assert_eq!(
        execute(
            &alpha4(),
            &card,
            &plan,
            &plan.id,
            &card.join("backups"),
            &[]
        )
        .unwrap_err()
        .code(),
        ErrorCode::UnsafeBackupLocation
    );
    // A plan that went stale.
    install_raw(&alpha4(), &card); // someone installed it meanwhile: every file's planned state flips
    let before_stale = snapshot(&card);
    assert_eq!(
        execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[])
            .unwrap_err()
            .code(),
        ErrorCode::SourceChangedSincePlan
    );
    assert_eq!(snapshot(&card), before_stale);
    assert!(!backups(&card).exists());
    let _ = before;
    // A refused plan (a downgrade nobody chose).
    install_raw(&alpha4(), &card);
    let down = install_plan::plan(&alpha3(), &card, &[], false).unwrap();
    let err = execute(&alpha3(), &card, &down, &down.id, &backups(&card), &[]).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InstallRefused);
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
}

#[test]
fn running_the_same_plan_twice_is_refused_the_second_time() {
    let card = lived_in_card("twice");
    let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
    // The card no longer matches the plan, so it is stale rather than silently repeated.
    let second = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap_err();
    assert_eq!(second.code(), ErrorCode::SourceChangedSincePlan);
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
}

#[test]
fn a_damaged_backup_is_never_restored() {
    let card = lived_in_card("damaged");
    let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
    let victim = report.backup_dir.join("files/Assets/tau/common/tau.rom");
    fs::write(&victim, b"corrupted backup").unwrap();
    let installed = snapshot(&card);
    let err = rollback(&card, &report.backup_dir).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InstallRefused);
    assert!(err.message.contains("no longer matches"), "{}", err.message);
    // Files restored before the damaged one are fine; the damaged one was not written.
    assert_eq!(
        fs::read(card.join("Assets/tau/common/tau.rom")).unwrap(),
        snapshot_file(&installed, "Assets/tau/common/tau.rom", &card)
    );
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
}

fn snapshot_file(_snap: &[(String, String)], rel: &str, card: &Path) -> Vec<u8> {
    fs::read(card.join(rel)).unwrap()
}

#[test]
fn the_same_build_does_nothing_and_makes_no_backup() {
    let card = lived_in_card("same");
    let first = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    execute(&alpha4(), &card, &first, &first.id, &backups(&card), &[]).unwrap();
    let again = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    let report = execute(&alpha4(), &card, &again, &again.id, &backups(&card), &[]).unwrap();
    assert!(report.nothing_to_do && report.checks.is_empty());
    assert_eq!(
        fs::read_dir(backups(&card)).unwrap().count(),
        1,
        "only the first install made a backup"
    );
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
}

#[test]
fn a_journal_for_another_card_is_not_applied() {
    let card = lived_in_card("other");
    let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
    let elsewhere = self::card("elsewhere");
    assert_eq!(
        rollback(&elsewhere, &report.backup_dir).unwrap_err().code(),
        ErrorCode::InstallRefused
    );
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
    fs::remove_dir_all(elsewhere.parent().unwrap()).unwrap();
}

/// What macOS does on exFAT: a `._` companion for each file and folder it *creates* (never for folders that were
/// already there). `existing` lists the card-relative paths that pre-date the install.
fn add_os_stubs(card: &Path, relatives: &[&str], existing: &[&str]) {
    for r in relatives {
        let parts: Vec<&str> = r.split('/').collect();
        for i in 0..parts.len() {
            let base = parts[..=i].join("/");
            let dir = parts[..i].join("/");
            let target = if dir.is_empty() {
                card.join(format!("._{}", parts[i]))
            } else {
                card.join(&dir).join(format!("._{}", parts[i]))
            };
            // Only where the thing exists now and the folder is there; never beside something that pre-dated the install.
            if !existing.contains(&base.as_str())
                && card.join(&base).exists()
                && target.parent().is_some_and(Path::is_dir)
            {
                fs::write(target, b"AppleDouble").unwrap();
            }
        }
    }
}

#[test]
fn stubs_the_os_creates_after_the_plan_are_swept() {
    let card = card("os-sweep");
    install_raw(&alpha3(), &card);
    let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    // After the plan, "the OS" creates companions beside the files the update will overwrite.
    let paths: Vec<&str> = plan
        .files
        .items
        .iter()
        .filter(|i| i.state == DifferenceState::Different)
        .map(|i| i.path.as_str())
        .collect();
    add_os_stubs(&card, &paths, &[]);
    assert!(snapshot(&card).iter().any(|(p, _)| p.contains("/._")));
    let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
    assert!(report.stubs_swept > 0, "{}", report.stubs_swept);
    let stray = report.checks[0]
        .items
        .iter()
        .find(|i| i.name == "stray files")
        .unwrap();
    assert_eq!(stray.status, CheckStatus::Pass, "{}", stray.detail);
    assert!(
        snapshot(&card).iter().all(|(p, _)| !p.contains("/._")),
        "{:?}",
        snapshot(&card)
    );
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
}

#[test]
fn rollback_removes_the_os_companions_and_only_the_folders_the_install_made() {
    const EXISTING: &[&str] = &[
        "Assets",
        "Cores",
        "Platforms",
        "Platforms/_images",
        "Platforms/_images/ex.bin",
    ];
    let card = card("os-rollback");
    fs::create_dir_all(card.join("Platforms/_images")).unwrap();
    fs::write(card.join("Platforms/_images/ex.bin"), b"x").unwrap();
    fs::write(card.join("Platforms/_images/._ex.bin"), b"theirs").unwrap(); // already there: must stay
    let before = snapshot(&card);
    let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
    // The OS creates companions for the new files and new folders while they are written.
    let paths: Vec<&str> = plan.files.items.iter().map(|i| i.path.as_str()).collect();
    add_os_stubs(&card, &paths, EXISTING);
    assert!(
        snapshot(&card)
            .iter()
            .filter(|(p, _)| p.contains("/._") || p.starts_with("._"))
            .count()
            > 15
    );
    rollback(&card, &report.backup_dir).unwrap();
    assert_eq!(
        snapshot(&card),
        before,
        "byte-identical, including the stub that pre-dated the install"
    );
    assert!(!card.join("Cores/alfatreze.TAU").exists() && !card.join("Assets/tau").exists());
    assert!(
        card.join("Platforms/_images/ex.bin").is_file(),
        "folders that were already there stay"
    );
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
}

#[test]
fn a_companion_that_was_there_before_a_restore_is_kept() {
    let card = card("keep-stub");
    install_raw(&alpha3(), &card);
    let rom = card.join("Assets/tau/common/tau.rom");
    fs::write(card.join("Assets/tau/common/._tau.rom"), b"theirs").unwrap();
    let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
    assert!(rom.is_file());
    // The plan swept their stub (it is junk beside a file being replaced); put one back, as if the user had it.
    fs::write(card.join("Assets/tau/common/._tau.rom"), b"theirs").unwrap();
    rollback(&card, &report.backup_dir).unwrap();
    assert_eq!(
        fs::read(card.join("Assets/tau/common/._tau.rom")).unwrap(),
        b"theirs"
    );
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
}

#[test]
fn an_older_journal_without_folder_records_still_rolls_back_cleanly() {
    let card = card("old-journal");
    let before = snapshot(&card);
    let plan = install_plan::plan(&alpha4(), &card, &[], false).unwrap();
    let report = execute(&alpha4(), &card, &plan, &plan.id, &backups(&card), &[]).unwrap();
    let journal_path = report.backup_dir.join(JOURNAL);
    let mut journal: Value = serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
    journal.as_object_mut().unwrap().remove("created_dirs");
    fs::write(&journal_path, serde_json::to_vec(&journal).unwrap()).unwrap();
    let paths: Vec<String> = plan.files.items.iter().map(|i| i.path.clone()).collect();
    add_os_stubs(
        &card,
        &paths.iter().map(String::as_str).collect::<Vec<_>>(),
        &[],
    );
    rollback(&card, &report.backup_dir).unwrap();
    assert_eq!(snapshot(&card), before);
    fs::remove_dir_all(card.parent().unwrap()).unwrap();
}

#[test]
fn companions_cover_the_file_and_every_folder_above_it() {
    assert_eq!(
        stub_companions("a/b/c.txt"),
        ["._a", "a/._b", "a/b/._c.txt"]
    );
    assert_eq!(stub_companions("c.txt"), ["._c.txt"]);
    assert_eq!(ancestors("a/b/c.txt"), ["a", "a/b"]);
}

/// Hashes every file on a real card except what the operating system owns and rewrites on its own.
fn real_snapshot(root: &Path) -> Vec<(String, String)> {
    const OS_OWNED: [&str; 5] = [
        ".Spotlight-V100",
        ".Trashes",
        ".fseventsd",
        "System Volume Information",
        "FOUND.000",
    ];
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            if OS_OWNED.contains(&e.file_name().to_string_lossy().as_ref()) {
                continue;
            }
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, out);
            } else {
                out.push((
                    p.strip_prefix(root).unwrap().to_string_lossy().into_owned(),
                    sync::sha256_bytes(&fs::read(&p).unwrap()),
                ));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

/// The real-card run (roadmap 1a). Gated; **read-only unless `TAU_REAL_INSTALL_MODE` is `install` or `rollback`**.
/// - `TAU_REAL_INSTALL_CARD`   the card root (e.g. `/Volumes/CARDWRITE`)
/// - `TAU_REAL_INSTALL_ZIP`    a release zip
/// - `TAU_REAL_INSTALL_BACKUP` a backup folder **outside the card**
/// - `TAU_REAL_INSTALL_MODE`   `plan` (prints the plan, proves nothing was written), `install` (executes it and
///   prints the post-install checks), or `rollback` (needs `TAU_REAL_INSTALL_JOURNAL`, the backup folder of the
///   install to undo, and compares the card with `TAU_REAL_INSTALL_BEFORE`, a file of `hash  path` lines).
///
/// `TAU_REAL_INSTALL_SNAPSHOT=<file>` writes such a file for the card as it is now.
#[test]
#[ignore]
fn real_card_install_run() {
    let (Ok(card), Ok(zip), Ok(backup), Ok(mode)) = (
        std::env::var("TAU_REAL_INSTALL_CARD"),
        std::env::var("TAU_REAL_INSTALL_ZIP"),
        std::env::var("TAU_REAL_INSTALL_BACKUP"),
        std::env::var("TAU_REAL_INSTALL_MODE"),
    ) else {
        println!("skipped: set TAU_REAL_INSTALL_CARD, _ZIP, _BACKUP and _MODE");
        return;
    };
    let (card, zip, backup) = (
        PathBuf::from(card),
        PathBuf::from(zip),
        PathBuf::from(backup),
    );
    let lines = |v: &[(String, String)]| {
        v.iter()
            .map(|(p, h)| format!("{h}  {p}"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    };
    if let Ok(file) = std::env::var("TAU_REAL_INSTALL_SNAPSHOT") {
        fs::write(&file, lines(&real_snapshot(&card))).unwrap();
        println!("snapshot of the card written to {file}");
    }
    let docs = crate::release_check::manifests_for_install(&backup.join("manifests"), &zip);
    match mode.as_str() {
        "plan" => {
            let before = real_snapshot(&card);
            let plan = install_plan::plan(&zip, &card, &docs, false).unwrap();
            println!("plan id: {}", plan.id);
            for core in &plan.update.cores {
                println!("{:?}: {}", core.verdict, core.reasons.join(" "));
                println!("pairing: {:?}", core.pair);
            }
            println!(
                "refused: {:?}\ncautions: {:?}\nnothing_to_do: {}",
                plan.refused, plan.cautions, plan.nothing_to_do
            );
            println!(
                "files: {} new, {} replaced, {} unchanged, {} bytes to write",
                plan.files.new_files,
                plan.files.updated_files,
                plan.files.unchanged_files,
                plan.files.bytes_to_write
            );
            for item in &plan.files.items {
                println!("  {:?} {} ({} bytes)", item.state, item.path, item.bytes);
            }
            println!(
                "backup ({} files, {} bytes):",
                plan.backup.len(),
                plan.backup_bytes
            );
            for b in &plan.backup {
                println!("  {} ({} bytes)", b.path, b.bytes);
            }
            println!(
                "caches to clear: {:?}\nobsolete: {:?}\nstubs to sweep: {:?}",
                plan.caches_to_clear, plan.obsolete_to_remove, plan.stubs_to_sweep
            );
            println!(
                "kept: {:?}\nsuperseded candidates: {:?}\ncapacity: {:?}",
                plan.user_files_kept, plan.superseded_candidates, plan.capacity
            );
            assert_eq!(
                real_snapshot(&card),
                before,
                "planning must not change the card"
            );
            println!("PLAN ONLY: the card is byte-identical to before");
        }
        "install" => {
            let plan = install_plan::plan(&zip, &card, &docs, false).unwrap();
            let report = execute(&zip, &card, &plan, &plan.id, &backup, &docs).unwrap();
            println!("backup folder: {}", report.backup_dir.display());
            println!(
                "written {} ({} bytes), backed up {}, caches {}, stubs {}, obsolete {}",
                report.files_written,
                report.bytes_written,
                report.files_backed_up,
                report.caches_cleared,
                report.stubs_swept,
                report.obsolete_removed
            );
            for check in &report.checks {
                print!("{}", check.to_text());
            }
            assert!(report.ok(), "a post-install check failed");
        }
        "rollback" => {
            let journal = PathBuf::from(
                std::env::var("TAU_REAL_INSTALL_JOURNAL").expect("TAU_REAL_INSTALL_JOURNAL"),
            );
            let r = rollback(&card, &journal).unwrap();
            println!(
                "restored {} files, removed {} created files",
                r.restored, r.created_removed
            );
            if let Ok(before) = std::env::var("TAU_REAL_INSTALL_BEFORE") {
                let after = lines(&real_snapshot(&card));
                let before = fs::read_to_string(before).unwrap();
                let strip = |t: &str| {
                    t.lines()
                        .filter(|l| !l.contains("/._") && !l.contains("  ._"))
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    strip(&after),
                    strip(&before),
                    "the card differs from the snapshot beyond `._` junk"
                );
                println!("ROLLBACK VERIFIED: the card matches the snapshot (apart from `._` junk)");
            }
        }
        other => panic!("unknown mode {other}"),
    }
}
