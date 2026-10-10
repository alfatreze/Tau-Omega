use super::*;

const SUNSET_JSON: &str = include_str!("../../testdata/assets/sunset.json");
const SUNSET_BIN: &[u8] = include_bytes!("../../testdata/assets/sunset.tau-assets.bin");

/// The reference theme JSON (`#RRGGBB` per key, bg_luma per polarity) as an editor input.
fn sunset() -> ThemeInput {
    let v: serde_json::Value = serde_json::from_str(SUNSET_JSON).unwrap();
    let pol = |p: &str| {
        let o = v[p].as_object().unwrap();
        PolarityInput {
            bg_luma: o["bg_luma"].as_u64().unwrap() as u8,
            colors: ROLES
                .iter()
                .map(|(k, _)| (k.to_string(), o[*k].as_str().unwrap().to_string()))
                .collect(),
        }
    };
    ThemeInput {
        name: v["name"].as_str().unwrap().into(),
        dark: pol("dark"),
        light: pol("light"),
    }
}

#[test]
fn writes_exactly_what_the_firmwares_own_tool_writes() {
    assert_eq!(pack_assets(&[sunset()]).unwrap(), SUNSET_BIN);
}

#[test]
fn a_written_file_reads_back_to_the_same_bytes() {
    let themes = parse_assets(SUNSET_BIN).unwrap();
    assert_eq!(themes.len(), 1);
    assert_eq!(themes[0].name, "SUNSET");
    assert_eq!(pack_assets(&themes).unwrap(), SUNSET_BIN);
}

#[test]
fn every_flipped_byte_and_every_truncation_is_refused() {
    for i in 0..SUNSET_BIN.len() {
        let mut b = SUNSET_BIN.to_vec();
        b[i] ^= 0x01;
        assert!(parse_assets(&b).is_err(), "flipping byte {i} was accepted");
    }
    for n in 0..SUNSET_BIN.len() {
        assert!(
            parse_assets(&SUNSET_BIN[..n]).is_err(),
            "a file cut to {n} bytes was accepted"
        );
    }
}

#[test]
fn contrast_figures_match_the_firmware_generators_report() {
    let r = check_theme(&sunset());
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    let worst = |pol: &str, text: &str, against: &str| {
        r.checks
            .iter()
            .find(|c| c.polarity == pol && c.text == text && c.against == against)
            .unwrap()
            .worst
    };
    for (pol, text, against, want) in [
        ("dark", "text_primary", "surface/base/ramp", 12.21),
        ("dark", "text_secondary", "surface/base", 7.14),
        ("dark", "text_secondary", "ramp", 5.69),
        ("light", "text_primary", "surface/base/ramp", 10.11),
        ("light", "text_secondary", "surface/base", 6.71),
        ("light", "text_secondary", "ramp", 4.86),
    ] {
        assert!(
            (worst(pol, text, against) - want).abs() < 0.006,
            "{pol} {text} vs {against}: {} != {want}",
            worst(pol, text, against)
        );
    }
}

#[test]
fn a_faint_theme_is_refused_and_says_why() {
    let mut t = sunset();
    let surface = t.dark.colors["surface"].clone();
    t.dark.colors.insert("text_primary".into(), surface);
    let r = check_theme(&t);
    assert!(
        r.problems
            .iter()
            .any(|p| p.contains("text primary") && p.contains("too faint")),
        "{:?}",
        r.problems
    );
    assert!(pack_assets(&[t]).is_err());
}

#[test]
fn names_ranges_and_missing_colours_are_refused() {
    let mut t = sunset();
    t.name = "sunset".into();
    assert!(!check_theme(&t).problems.is_empty());
    t.name = "TAU".into();
    assert!(
        check_theme(&t)
            .problems
            .iter()
            .any(|p| p.contains("built-in"))
    );
    let mut t = sunset();
    t.dark.bg_luma = 10;
    assert!(!check_theme(&t).problems.is_empty());
    let mut t = sunset();
    t.light.colors.remove("ok");
    assert!(
        check_theme(&t)
            .problems
            .iter()
            .any(|p| p.contains("no colour"))
    );
    let mut t = sunset();
    t.dark.colors.insert("ok".into(), "green".into());
    assert!(
        check_theme(&t)
            .problems
            .iter()
            .any(|p| p.contains("not a colour"))
    );
    assert!(pack_assets(&[]).is_err());
    let (a, mut b) = (sunset(), sunset());
    assert!(
        pack_assets(&[a.clone(), a.clone()]).is_err(),
        "duplicate names"
    );
    b.name = "SUNSET 2".into();
    assert!(pack_assets(&[a, b]).is_ok());
}

#[test]
fn snapping_matches_the_device() {
    assert_eq!(snap("#FFFFFF"), Some(0xFFFF));
    assert_eq!(snap("#000000"), Some(0));
    assert_eq!(snap("0x2945"), Some(0x2945));
    assert_eq!(snap("#10060"), None);
    assert_eq!(to_hex(0xFFFF), "#FFFFFF");
    // What the editor shows is what the device shows: snapping twice changes nothing.
    let once = snap("#2A1820").unwrap();
    assert_eq!(snap(&to_hex(once)), Some(once));
}

fn card(name: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "tau-assets-{name}-{}-{}",
        std::process::id(),
        crate::test_uniq()
    ));
    let _ = fs::remove_dir_all(&root);
    let media = root.join("Assets").join("tau").join("common");
    fs::create_dir_all(&media).unwrap();
    let core = root.join("Cores").join("alfatreze.TAU");
    fs::create_dir_all(&core).unwrap();
    fs::write(
        core.join("core.json"),
        r#"{"core":{"metadata":{"platform_ids":["tau"],"version":"0.6.0"}}}"#,
    )
    .unwrap();
    fs::write(
        core.join("data.json"),
        r#"{"data":{"data_slots":[{"id":8,"filename":"tau-assets.bin"}]}}"#,
    )
    .unwrap();
    (root, media)
}

#[test]
fn plans_without_writing_and_names_the_cores_that_will_read_it() {
    let (root, media) = card("plan");
    let plan = plan_install(&[sunset()], &media).unwrap();
    assert!(plan.existing.is_none() && !plan.interrupted_install && plan.warnings.is_empty());
    assert_eq!(plan.themes, vec!["SUNSET"]);
    assert_eq!(
        plan.readers,
        vec![ThemeFileReader {
            core_id: "alfatreze.TAU".into(),
            version: "0.6.0".into(),
            declares_slot: true
        }]
    );
    assert_eq!(plan.bytes, SUNSET_BIN.len() as u64);
    assert!(!media.join(FILE_NAME).exists(), "planning wrote something");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn installs_verifies_and_leaves_no_temp_files() {
    let (root, media) = card("install");
    let plan = plan_install(&[sunset()], &media).unwrap();
    let report = execute_install(&[sunset()], &media, &plan, &plan.id, None).unwrap();
    assert_eq!(fs::read(media.join(FILE_NAME)).unwrap(), SUNSET_BIN);
    assert!(!report.replaced && report.backup.is_none());
    let left: Vec<_> = fs::read_dir(&media)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, vec![FILE_NAME]);
    let _ = fs::remove_dir_all(root);
}

/// A file carrying meter presets (`METR`) and Halcyon user EQ presets (`PRST`) around an older theme section.
fn file_with_presets(order_them_middle: bool) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut other = sunset();
    other.name = "OLDER".into();
    let them = pack_them(&[other]).unwrap();
    let metr = b"TMTR\x01\x00\x00\x01meter-preset-bytes".to_vec();
    let prst = b"TPRS\x01\x00\x00\x02user-eq-preset-bytes".to_vec();
    let file = if order_them_middle {
        pack_container(&[(*b"METR", &metr), (*SECTION_THEM, &them), (*b"PRST", &prst)])
    } else {
        pack_container(&[(*b"METR", &metr), (*b"PRST", &prst)])
    };
    (file, metr, prst)
}

#[test]
fn replacing_keeps_meter_and_eq_presets_byte_for_byte_and_backs_up() {
    let (root, media) = card("replace");
    let (first, metr, prst) = file_with_presets(true);
    fs::write(media.join(FILE_NAME), &first).unwrap();
    let backups = root.with_extension("backups");
    let plan = plan_install(&[sunset()], &media).unwrap();
    let e = plan.existing.as_ref().unwrap();
    assert_eq!(e.themes, vec!["OLDER"]);
    assert_eq!(e.other_sections, vec!["METR", "PRST"]);
    assert!(plan.warnings.iter().any(|w| w.contains("kept unchanged")));
    let report = execute_install(&[sunset()], &media, &plan, &plan.id, Some(&backups)).unwrap();
    assert!(report.replaced);
    assert_eq!(fs::read(report.backup.unwrap()).unwrap(), first);
    let _ = fs::remove_dir_all(&backups);
    let now = fs::read(media.join(FILE_NAME)).unwrap();
    let tags: Vec<_> = read_sections(&now)
        .unwrap()
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    assert_eq!(
        tags,
        vec![*b"METR", *SECTION_THEM, *b"PRST"],
        "section order kept, THEM replaced in place"
    );
    assert_eq!(
        kept_sections(&now),
        vec![(*b"METR", metr), (*b"PRST", prst)],
        "preset bytes changed"
    );
    assert_eq!(parse_assets(&now).unwrap()[0].name, "SUNSET");
    assert_eq!(
        plan.sha256,
        sync::sha256_bytes(&now),
        "the plan's hash is the file written"
    );
    assert!(!media.join(PREVIOUS_NAME).exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_file_without_themes_gets_them_first_and_keeps_the_rest() {
    let (file, metr, prst) = file_with_presets(false);
    let out = pack_assets_keeping(&[sunset()], Some(&file)).unwrap();
    let tags: Vec<_> = read_sections(&out)
        .unwrap()
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    assert_eq!(tags, vec![*SECTION_THEM, *b"METR", *b"PRST"]);
    assert_eq!(
        kept_sections(&out),
        vec![(*b"METR", metr), (*b"PRST", prst)]
    );
    // With no other sections the result is exactly the reference single-section file.
    assert_eq!(
        pack_assets_keeping(&[sunset()], Some(SUNSET_BIN)).unwrap(),
        SUNSET_BIN
    );
    assert_eq!(pack_assets_keeping(&[sunset()], None).unwrap(), SUNSET_BIN);
}

#[test]
fn a_newer_container_is_refused_and_left_alone() {
    let (root, media) = card("newer");
    let mut newer = file_with_presets(true).0;
    newer[4] = 2; // container version 2
    fs::write(media.join(FILE_NAME), &newer).unwrap();
    let err = plan_install(&[sunset()], &media).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidAssetsFile);
    assert!(err.to_string().contains("newer Tau"), "{err}");
    assert_eq!(fs::read(media.join(FILE_NAME)).unwrap(), newer);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_result_over_the_firmware_limits_is_refused() {
    let them = pack_them(&[sunset()]).unwrap();
    let big = vec![0u8; MAX_FILE_BYTES];
    let file = pack_container(&[(*SECTION_THEM, &them), (*b"PRST", &big)]);
    assert!(
        pack_assets_keeping(&[sunset()], Some(&file))
            .unwrap_err()
            .to_string()
            .contains("at most 65536")
    );
    let parts: Vec<([u8; 4], &[u8])> = (0..8u8)
        .map(|i| ([b'X', b'X', b'X', b'0' + i], &b"x"[..]))
        .collect();
    let full = pack_container(&parts);
    assert!(
        pack_assets_keeping(&[sunset()], Some(&full))
            .unwrap_err()
            .to_string()
            .contains("at most 8")
    );
}

/// Tau Alpha's shared fixture (`docs/schemas/fixtures/tau-assets-roundtrip.bin`, THEM + METR + PRST from the reference
/// packers): referenced in place, never copied (cross-project rule). Skipped when the sibling checkout is absent.
#[test]
fn tau_alpha_round_trip_fixture_keeps_metr_and_prst() {
    let base = std::env::var("TAU_ALPHA_ROOT")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../../tau-alpha").to_string());
    let path = Path::new(&base).join("docs/schemas/fixtures/tau-assets-roundtrip.bin");
    let Ok(fixture) = fs::read(&path) else {
        eprintln!("skipped: {} not found (set TAU_ALPHA_ROOT)", path.display());
        return;
    };
    let before = kept_sections(&fixture);
    assert_eq!(
        before.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
        vec![*b"METR", *b"PRST"]
    );
    let out = pack_assets_keeping(&[sunset()], Some(&fixture)).unwrap();
    assert_eq!(
        kept_sections(&out),
        before,
        "METR/PRST bytes changed on a theme edit"
    );
    assert_eq!(parse_assets(&out).unwrap()[0].name, "SUNSET");
}

#[test]
fn an_unreadable_existing_file_is_flagged_and_still_backed_up() {
    let (root, media) = card("junk");
    fs::write(media.join(FILE_NAME), b"not a tau file").unwrap();
    let plan = plan_install(&[sunset()], &media).unwrap();
    assert!(!plan.existing.as_ref().unwrap().readable);
    assert!(plan.warnings.iter().any(|w| w.contains("cannot be read")));
    let report = execute_install(
        &[sunset()],
        &media,
        &plan,
        &plan.id,
        Some(&root.with_extension("backups")),
    )
    .unwrap();
    assert_eq!(fs::read(report.backup.unwrap()).unwrap(), b"not a tau file");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn refuses_a_wrong_token_a_changed_card_and_an_unsafe_backup() {
    let (root, media) = card("refuse");
    let plan = plan_install(&[sunset()], &media).unwrap();
    assert_eq!(
        execute_install(&[sunset()], &media, &plan, "nope", None)
            .unwrap_err()
            .code(),
        ErrorCode::ConfirmationMismatch
    );
    // The card's file changes after review.
    fs::write(media.join(FILE_NAME), b"changed behind our back").unwrap();
    assert_eq!(
        execute_install(&[sunset()], &media, &plan, &plan.id, None)
            .unwrap_err()
            .code(),
        ErrorCode::SourceChangedSincePlan
    );
    assert_eq!(
        fs::read(media.join(FILE_NAME)).unwrap(),
        b"changed behind our back",
        "a refused install changed the file"
    );
    let fresh = plan_install(&[sunset()], &media).unwrap();
    assert_eq!(
        execute_install(&[sunset()], &media, &fresh, &fresh.id, Some(&root))
            .unwrap_err()
            .code(),
        ErrorCode::UnsafeBackupLocation
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn an_interrupted_install_is_recovered_not_lost() {
    let (root, media) = card("recover");
    fs::write(media.join(PREVIOUS_NAME), SUNSET_BIN).unwrap(); // the old file, moved aside; the new one never arrived
    let plan = plan_install(&[sunset()], &media).unwrap();
    assert!(plan.interrupted_install);
    // Installing something else first restores the old file, so it is the "existing" one that gets replaced and backed up.
    let mut t = sunset();
    t.name = "SUNSET 2".into();
    let plan2 = plan_install(&[t.clone()], &media).unwrap();
    assert!(plan2.interrupted_install);
    recover(&media).unwrap();
    assert_eq!(fs::read(media.join(FILE_NAME)).unwrap(), SUNSET_BIN);
    assert!(!media.join(PREVIOUS_NAME).exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn refuses_a_folder_that_is_not_a_media_root_and_a_faint_theme() {
    let (root, media) = card("bad");
    assert!(plan_install(&[sunset()], &root).is_err());
    assert!(plan_install(&[sunset()], &media.join("missing")).is_err());
    let mut t = sunset();
    t.dark
        .colors
        .insert("text_primary".into(), t.dark.colors["surface"].clone());
    assert!(plan_install(&[t], &media).is_err());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn warns_when_no_core_asks_for_the_file() {
    let (root, media) = card("noslot");
    fs::write(
        root.join("Cores/alfatreze.TAU/data.json"),
        r#"{"data":{"data_slots":[{"id":5,"filename":"tau-library.tdb"}]}}"#,
    )
    .unwrap();
    let plan = plan_install(&[sunset()], &media).unwrap();
    assert!(plan.warnings.iter().any(|w| w.contains("ignored")));
    let _ = fs::remove_dir_all(root);
}

/// Read-only check against a real card: `TAU_REAL_CARD=/Volumes/Pock cargo test -p tau-core real_card -- --ignored --nocapture`.
/// Plans (never writes) an install for every `Assets/<platform>/common` folder and prints what it found.
#[test]
#[ignore]
fn real_card_plan_is_read_only_and_finds_the_readers() {
    let Ok(card) = std::env::var("TAU_REAL_CARD") else {
        return;
    };
    let before: Vec<_> = walk(Path::new(&card).join("Assets"));
    for entry in fs::read_dir(Path::new(&card).join("Assets"))
        .unwrap()
        .filter_map(Result::ok)
    {
        let media = entry.path().join("common");
        if !media.is_dir() {
            continue;
        }
        let plan = plan_install(&[sunset()], &media).unwrap();
        println!(
            "{}: existing={:?} readers={:?} warnings={:?}",
            media.display(),
            plan.existing
                .as_ref()
                .map(|e| (e.bytes, &e.themes, e.readable)),
            plan.readers,
            plan.warnings
        );
    }
    assert_eq!(
        before,
        walk(Path::new(&card).join("Assets")),
        "planning changed the card"
    );
}
fn walk(root: PathBuf) -> Vec<(PathBuf, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
        {
            let p = e.path();
            if p.is_dir() {
                stack.push(p)
            } else {
                out.push((p, e.metadata().map(|m| m.len()).unwrap_or(0)))
            }
        }
    }
    out.sort();
    out
}

/// A real, approved write. Only runs when both variables are set:
/// `TAU_REAL_WRITE_MEDIA=/Volumes/Pock/Assets/<platform>/common TAU_REAL_WRITE_BACKUP=<folder outside the card>`.
/// Installs a theme called OMEGA TEST through the same plan/execute path the app uses, then checks that the only
/// change on the whole card is that one file, that the replaced file is in the backup, and that no temp file is left.
#[test]
#[ignore]
fn real_card_install_changes_only_the_theme_file() {
    let (Ok(media), Ok(backup)) = (
        std::env::var("TAU_REAL_WRITE_MEDIA"),
        std::env::var("TAU_REAL_WRITE_BACKUP"),
    ) else {
        return;
    };
    let media = PathBuf::from(media);
    let card = media
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let snapshot = |card: &Path| -> Vec<(PathBuf, u64)> {
        let mut all = walk(card.join("Assets"));
        all.extend(walk(card.join("Cores")));
        all
    };
    let before = snapshot(&card);
    let old = fs::read(media.join(FILE_NAME)).ok();
    let mut theme = sunset();
    theme.name = "OMEGA TEST".into();
    let plan = plan_install(&[theme.clone()], &media).unwrap();
    println!(
        "plan: existing={:?} readers={:?} warnings={:?}",
        plan.existing, plan.readers, plan.warnings
    );
    let report = execute_install(
        &[theme.clone()],
        &media,
        &plan,
        &plan.id,
        Some(Path::new(&backup)),
    )
    .unwrap();
    println!("report: {report:?}");
    let after = snapshot(&card);
    let live = media.join(FILE_NAME);
    let changed: Vec<_> = after
        .iter()
        .filter(|e| !before.contains(e))
        .chain(before.iter().filter(|e| !after.contains(e)))
        .collect();
    println!("changed entries: {changed:?}");
    assert!(
        changed.iter().all(|(p, _)| *p == live),
        "something other than the theme file changed"
    );
    assert_eq!(fs::read(&live).unwrap(), pack_assets(&[theme]).unwrap());
    if let (Some(old), Some(b)) = (old, report.backup.as_ref()) {
        assert_eq!(
            fs::read(b).unwrap(),
            old,
            "the backup is not the file that was replaced"
        );
    }
    assert!(!media.join(TEMP_NAME).exists() && !media.join(PREVIOUS_NAME).exists());
}

fn two_presets() -> Vec<HalcyonPreset> {
    vec![
        HalcyonPreset::Control {
            name: "MY CONTROLS".into(),
            controls: [2, 1, 0, -1, 3, -2],
        },
        HalcyonPreset::Raw {
            name: "FLATISH".into(),
            preamp: 1 << 21,
            stages: vec![[1 << 22, 0, 0, 0, 0]],
        },
    ]
}

#[test]
fn a_preset_edit_keeps_themes_and_meter_presets_byte_for_byte() {
    let (file, metr, _old_prst) = file_with_presets(true);
    let before_them = read_sections(&file)
        .unwrap()
        .into_iter()
        .find(|(t, _)| t == SECTION_THEM)
        .unwrap()
        .1
        .to_vec();
    let presets = two_presets();
    let out = pack_assets_edit(AssetsEdit::presets(&presets), Some(&file)).unwrap();
    let sections = read_sections(&out).unwrap();
    assert_eq!(
        sections.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
        vec![*b"METR", *SECTION_THEM, *b"PRST"],
        "order kept, PRST replaced in place"
    );
    assert_eq!(sections[0].1, &metr[..]);
    assert_eq!(sections[1].1, &before_them[..]);
    assert_eq!(read_presets(&out).unwrap(), presets);
    // the other direction: a theme edit keeps the new presets
    let again = pack_assets_keeping(&parse_assets(&out).unwrap(), Some(&out)).unwrap();
    assert_eq!(again, out);
}

#[test]
fn presets_are_added_to_a_file_without_them_and_can_be_removed() {
    let presets = two_presets();
    let added = pack_assets_edit(AssetsEdit::presets(&presets), Some(SUNSET_BIN)).unwrap();
    assert_eq!(
        parse_assets(&added).unwrap(),
        parse_assets(SUNSET_BIN).unwrap(),
        "themes untouched"
    );
    assert_eq!(read_presets(&added).unwrap(), presets);
    let removed = pack_assets_edit(AssetsEdit::presets(&[]), Some(&added)).unwrap();
    assert_eq!(
        removed, SUNSET_BIN,
        "removing the presets gives the original file back"
    );
    // no existing file: a presets-only container
    let alone = pack_assets_edit(AssetsEdit::presets(&presets), None).unwrap();
    assert_eq!(read_presets(&alone).unwrap(), presets);
    assert!(parse_assets(&alone).unwrap().is_empty());
}

#[test]
fn preset_install_on_a_card_verifies_and_keeps_the_themes() {
    let (root, media) = card("presets");
    fs::write(media.join(FILE_NAME), SUNSET_BIN).unwrap();
    let presets = two_presets();
    let plan = plan_install_edit(AssetsEdit::presets(&presets), &media).unwrap();
    assert_eq!(plan.presets, vec!["MY CONTROLS", "FLATISH"]);
    assert!(plan.themes.is_empty());
    let backups = root.with_extension("backups");
    let report = execute_install_edit(
        AssetsEdit::presets(&presets),
        &media,
        &plan,
        &plan.id,
        Some(&backups),
    )
    .unwrap();
    assert!(report.replaced && report.backup.is_some());
    let now = fs::read(media.join(FILE_NAME)).unwrap();
    assert_eq!(read_presets(&now).unwrap(), presets);
    assert_eq!(
        parse_assets(&now).unwrap(),
        parse_assets(SUNSET_BIN).unwrap()
    );
    // a changed preset list invalidates the reviewed plan
    let other = vec![HalcyonPreset::Control {
        name: "OTHER".into(),
        controls: [0; 6],
    }];
    assert!(
        execute_install_edit(AssetsEdit::presets(&other), &media, &plan, &plan.id, None).is_err()
    );
    let _ = fs::remove_dir_all(root);
}
