use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn space_is_counted_in_whole_clusters_and_checked_before_writing() {
    assert_eq!(round_up_to(1, 32768), 32768);
    assert_eq!(round_up_to(32768, 32768), 32768);
    assert_eq!(round_up_to(32769, 32768), 65536);
    assert_eq!(round_up_to(0, 32768), 0);
    // 1,000 one-byte files really cost 1,000 clusters, not 1,000 bytes.
    let plan = SyncPlan {
        id: String::new(),
        destination: PathBuf::new(),
        root_prefix: String::new(),
        items: (0..1000)
            .map(|n| CopyItem {
                source: PathBuf::from(format!("{n}")),
                destination: PathBuf::from(format!("d{n}")),
                bytes: 1,
                sha256: String::new(),
                cover: None,
                state: CopyState::New,
            })
            .collect(),
        deletions: Vec::new(),
        embed_covers: false,
        art_sidecars: Vec::new(),
        warnings: Vec::new(),
        bytes_to_write: 1000,
    };
    let needed = bytes_on_disk(&plan, 131_072);
    assert!(needed >= 1000 * 131_072, "exFAT-sized clusters: {needed}");
    // Files that are already on the card cost nothing.
    let same = SyncPlan {
        items: plan
            .items
            .iter()
            .cloned()
            .map(|mut i| {
                i.state = CopyState::Same;
                i
            })
            .collect(),
        ..plan.clone()
    };
    assert!(bytes_on_disk(&same, 131_072) < 1000 * 131_072);
    // The check refuses with its own code, and passes when there is room.
    assert_eq!(
        ensure_space(10, 1_000, 0).unwrap_err().code(),
        ErrorCode::InsufficientSpace
    );
    assert_eq!(
        ensure_space(1_000, 1_000, 1).unwrap_err().code(),
        ErrorCode::InsufficientSpace
    );
    ensure_space(2_000, 1_000, 1_000).unwrap();
}

fn synced_card(name: &str) -> PathBuf {
    let source = root(&format!("{name}-source"));
    let card = root(&format!("{name}-card"));
    let common = card.join("Assets/tau/common");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&common).unwrap();
    fs::write(source.join("01 Track.mp3"), b"audio one").unwrap();
    let sync_plan = plan(
        &[source],
        &common,
        "/Assets/tau/common/",
        PlanOptions {
            mirror: false,
            embed_covers: false,
            art_sidecar_pal256: false,
        },
        &mut None,
    )
    .unwrap();
    execute(&sync_plan, &sync_plan.id, &mut None).unwrap();
    common
}

/// An index swap cut short must always leave a complete index to recover,
/// never "no index" (FAT cannot rename over an existing file atomically).
#[test]
fn an_interrupted_index_swap_is_recovered_and_a_finished_one_leaves_nothing_behind() {
    let common = synced_card("index-swap");
    let live = common.join("tau-library.tdb");
    let good = fs::read(&live).unwrap();
    assert!(
        !common.join(".tau-library.tdb.prev").exists(),
        "a finished swap leaves no .prev"
    );
    assert_eq!(recover_index(&common).unwrap(), IndexRecovery::Clean);

    // Card pulled between "move old aside" and "move new into place".
    fs::rename(&live, common.join(".tau-library.tdb.prev")).unwrap();
    assert!(index_needs_recovery(&common));
    assert_eq!(recover_index(&common).unwrap(), IndexRecovery::Restored);
    assert_eq!(fs::read(&live).unwrap(), good);
    assert!(!index_needs_recovery(&common));

    // A live index that is truncated garbage next to a good .prev.
    fs::write(common.join(".tau-library.tdb.prev"), &good).unwrap();
    fs::write(&live, b"TLIB truncated").unwrap();
    assert_eq!(recover_index(&common).unwrap(), IndexRecovery::Restored);
    assert_eq!(fs::read(&live).unwrap(), good);

    // Swap finished, cleanup missed: the stale .prev just goes away.
    fs::write(common.join(".tau-library.tdb.prev"), b"old").unwrap();
    assert_eq!(recover_index(&common).unwrap(), IndexRecovery::Clean);
    assert!(!common.join(".tau-library.tdb.prev").exists());
    assert_eq!(fs::read(&live).unwrap(), good);

    // Never invents an index: a bad .prev with no live index is left alone.
    fs::remove_file(&live).unwrap();
    fs::write(common.join(".tau-library.tdb.prev"), b"junk").unwrap();
    assert_eq!(recover_index(&common).unwrap(), IndexRecovery::Clean);
    assert!(!live.exists());
}

#[test]
fn only_old_leftover_temp_files_of_this_tool_are_swept() {
    let common = synced_card("sweep");
    let album = common.join("Album");
    fs::create_dir_all(&album).unwrap();
    let old_temp = album.join("Song.tau-omega-4242.tmp");
    let fresh_temp = album.join("Other.tau-omega-4242.tmp");
    let foreign = album.join("keep.tmp");
    let index_temp = common.join(".tau-library-abc.tmp");
    for f in [&old_temp, &fresh_temp, &foreign, &index_temp] {
        fs::write(f, b"x").unwrap();
    }
    let two_hours_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 3600);
    for f in [&old_temp, &foreign, &index_temp] {
        fs::File::options()
            .write(true)
            .open(f)
            .unwrap()
            .set_modified(two_hours_ago)
            .unwrap();
    }
    let written = [album.join("Song.mp3")];
    sweep_stale_temps(&common, written.iter().map(|p| p.as_path()));
    assert!(!old_temp.exists(), "an old leftover of ours is removed");
    assert!(
        !index_temp.exists(),
        "an old leftover index temp is removed"
    );
    assert!(
        fresh_temp.exists(),
        "a recent temp may belong to a run in progress"
    );
    assert!(
        foreign.exists(),
        "files that are not ours are never touched"
    );
}

fn opts(embed: bool) -> PlanOptions {
    PlanOptions {
        mirror: false,
        embed_covers: embed,
        art_sidecar_pal256: false,
    }
}

/// The ledger remembers what this tool wrote and verified, so re-adding an album that is already on the
/// card is recognised as "already there" without reading the card (an embedded copy can never equal its
/// source, so it used to be rewritten on every re-sync). Anything that no longer matches is copied again,
/// and a card file altered behind its back (same size, same time) is caught by the canary.
#[test]
fn a_resync_of_an_embedded_album_is_recognised_and_every_change_is_still_noticed() {
    crate::ledger::register_test_locator();
    let base = std::env::temp_dir().join(format!(
        "tau-ledger-it-sync-{}",
        std::process::id() as u128 + crate::test_uniq()
    ));
    let (album, common) = (base.join("lib/Album"), base.join("card/Assets/tau/common"));
    for dir in [&album, &common] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(album.join("01.mp3"), b"audio one, long enough").unwrap();
    fs::write(album.join("02.mp3"), b"audio two, also long").unwrap();
    fs::write(album.join("cover.jpg"), [0xff, 0xd8, 0xff, 0xd9]).unwrap();
    let sources = [album.clone()];
    let make = || {
        plan(
            &sources,
            &common,
            "/Assets/tau/common/",
            opts(true),
            &mut None,
        )
        .unwrap()
    };

    let first = make();
    assert!(first.items.iter().all(|i| i.state == CopyState::New));
    execute(&first, &first.id, &mut None).unwrap();

    // Nothing changed anywhere: both files are provably already there, so nothing is planned.
    let again = make();
    assert!(
        again.items.iter().all(|i| i.state == CopyState::Same),
        "{:?}",
        again.items.iter().map(|i| i.state).collect::<Vec<_>>()
    );
    assert_eq!(
        again.bytes_to_write, 0,
        "no rewriting of an album that is already on the card"
    );

    // A source changes: that file is copied again, the other is still recognised.
    fs::write(album.join("02.mp3"), b"audio two, REVISED and longer").unwrap();
    let revised = make();
    let states: Vec<_> = revised.items.iter().map(|i| i.state).collect();
    assert_eq!(states, [CopyState::Same, CopyState::Update]);
    fs::write(album.join("02.mp3"), b"audio two, also long").unwrap(); // put it back

    // The cover changes: every embedded copy is stale.
    fs::write(
        album.join("cover.jpg"),
        [0xff, 0xd8, 0xff, 0xe0, 0xff, 0xd9],
    )
    .unwrap();
    assert!(
        make().items.iter().all(|i| i.state == CopyState::Update),
        "a different cover means different bytes"
    );
    fs::write(album.join("cover.jpg"), [0xff, 0xd8, 0xff, 0xd9]).unwrap();
    assert!(make().items.iter().all(|i| i.state == CopyState::Same));

    // Someone alters a card file but keeps its size and its time: the ledger cannot see that, the
    // canary does, and it distrusts everything it could not re-check.
    let victim = common.join("Album/01.mp3");
    let kept_time = fs::metadata(&victim).unwrap().modified().unwrap();
    let mut bytes = fs::read(&victim).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xff;
    fs::write(&victim, &bytes).unwrap();
    fs::File::options()
        .write(true)
        .open(&victim)
        .unwrap()
        .set_modified(kept_time)
        .unwrap();
    let caught = make();
    let altered = caught
        .items
        .iter()
        .find(|i| i.destination.ends_with("Album/01.mp3"))
        .unwrap();
    assert_eq!(
        altered.state,
        CopyState::Update,
        "the altered file is always caught and copied again"
    );
    // The other file is either re-checked and confirmed (Same) or, if the canary hit the altered file
    // first, distrusted along with everything it had not checked yet (Update). Both are safe.
    assert!(
        caught
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::CardFileChanged)
    );
    let repaired = caught.bytes_to_write;
    assert!(repaired > 0);
    execute(&caught, &caught.id, &mut None).unwrap();
    assert!(
        make().items.iter().all(|i| i.state == CopyState::Same),
        "after the repair the ledger trusts the card again"
    );

    // Forgetting the card costs nothing but time: the same plan falls back to reading and hashing.
    assert!(crate::ledger::clear(&common));
    assert!(
        make().items.iter().all(|i| i.state == CopyState::Update),
        "without the ledger an embedded copy cannot be proven, so it is rewritten (the old behaviour)"
    );
    fs::remove_dir_all(base).unwrap();
}

/// Always embedding covers must never make a whole sync fail because one album's
/// cover is not a baseline JPEG: the songs are copied without it and a warning
/// names the album, once, however many tracks it has.
#[test]
fn an_unusable_cover_warns_instead_of_failing_the_sync() {
    let base = root("bad-cover");
    let (album, other) = (
        base.join("lib/Bad Cover Album"),
        base.join("lib/Good Album"),
    );
    let common = base.join("card/Assets/tau/common");
    for dir in [&album, &other, &common] {
        fs::create_dir_all(dir).unwrap();
    }
    for n in 1..=3 {
        fs::write(album.join(format!("0{n}.mp3")), format!("audio {n}")).unwrap();
    }
    fs::write(other.join("01.mp3"), b"audio good").unwrap();
    fs::write(album.join("cover.jpg"), b"this is not a jpeg at all").unwrap();
    fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/images/cover455.jpg"),
        other.join("cover.jpg"),
    )
    .unwrap();
    let sync_plan = plan(
        &[album.clone(), other.clone()],
        &common,
        "/Assets/tau/common/",
        PlanOptions {
            mirror: false,
            embed_covers: true,
            art_sidecar_pal256: false,
        },
        &mut None,
    )
    .expect("an unusable cover must not fail the plan");
    let covers: Vec<_> = sync_plan
        .warnings
        .iter()
        .filter(|w| w.code == WarningCode::CoverNotEmbedded)
        .collect();
    assert_eq!(
        covers.len(),
        1,
        "one warning for the album, not one per track: {:?}",
        sync_plan.warnings
    );
    assert!(covers[0].message.contains("Bad Cover Album"));
    let with_cover = sync_plan.items.iter().filter(|i| i.cover.is_some()).count();
    assert_eq!(
        with_cover, 1,
        "only the album with a usable cover gets one embedded"
    );
    execute(&sync_plan, &sync_plan.id, &mut None).unwrap();
    assert!(
        common.join("Bad Cover Album/03.mp3").is_file(),
        "the songs are still copied"
    );
}

/// The card's file system ignores letter case, so two sources that differ only
/// by case must be refused at plan time instead of the second silently
/// replacing the first on the card.
#[test]
fn names_that_differ_only_by_case_collide() {
    let base = root("case-collision");
    let (upper, lower) = (base.join("x/Rock"), base.join("y/rock"));
    let common = base.join("card/Assets/tau/common");
    for dir in [&upper, &lower, &common] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(upper.join("01.mp3"), b"first").unwrap();
    fs::write(lower.join("01.mp3"), b"second").unwrap();
    let error = plan(
        &[upper, lower],
        &common,
        "/Assets/tau/common/",
        PlanOptions {
            mirror: false,
            embed_covers: false,
            art_sidecar_pal256: false,
        },
        &mut None,
    )
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::NameCollision);
}

/// The read-back must go through the host's cache evictor before it reads.
#[test]
fn read_backs_evict_the_cache_first() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CALLS: AtomicU64 = AtomicU64::new(0);
    fn counting(_: &Path) -> bool {
        CALLS.fetch_add(1, Ordering::SeqCst);
        true
    }
    register_cache_evictor(counting);
    let dir = std::env::temp_dir().join(format!(
        "tau-evict-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            + crate::test_uniq()
    ));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("w.bin");
    fs::write(&file, b"payload").unwrap();
    let before = CALLS.load(Ordering::SeqCst);
    verify_written(&file, &sha256_bytes(b"payload"), Path::new("s")).unwrap();
    assert_eq!(read_back_bytes(&file).unwrap(), b"payload");
    // Other tests in this process may also evict, so only a lower bound is exact.
    assert!(CALLS.load(Ordering::SeqCst) >= before + 2);
    assert!(readback_report().0, "an evictor is registered");
    fs::remove_dir_all(dir).unwrap();
}

/// The check every write ends with must actually fail on wrong bytes (the
/// cover-embedding path used to end in a test that could never fail).
#[test]
fn verify_written_rejects_wrong_bytes_and_removes_the_temp_file() {
    let dir = std::env::temp_dir().join(format!(
        "tau-verify-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            + crate::test_uniq()
    ));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("x.tmp");
    fs::write(&file, b"what actually reached the card").unwrap();
    let intended = sha256_bytes(b"what we meant to write");
    let error = verify_written(&file, &intended, Path::new("src.mp3")).unwrap_err();
    assert_eq!(error.code(), ErrorCode::VerificationFailed);
    assert!(
        !file.exists(),
        "a failed verification must not leave the temp file"
    );
    fs::write(&file, b"exact").unwrap();
    verify_written(&file, &sha256_bytes(b"exact"), Path::new("src.mp3")).unwrap();
    assert!(file.exists());
    fs::remove_dir_all(dir).unwrap();
}

/// P1-2: `sources: &[PathBuf]` is a public parameter to `plan`/
/// `plan_with_features`; a path with no derivable file name (`/`, `.`)
/// used to reach a bare `.file_name().unwrap()` here and panic the whole
/// plan instead of failing it cleanly.
#[test]
fn required_file_name_does_not_panic_on_a_nameless_path() {
    assert!(required_file_name(Path::new("/")).is_err());
    assert!(required_file_name(Path::new(".")).is_err());
    assert_eq!(
        required_file_name(Path::new("/tmp/foo.mp3")).unwrap(),
        "foo.mp3"
    );
}

fn root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "tau-sync-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            + crate::test_uniq()
    ))
}
/// P2-1: the token used to be a 32-bit-truncated hash formatted `T2-xxxxxxxx`,
/// thin for something that may be persisted or handed across a process
/// boundary, and the `T2-` leaked an internal roadmap phase label into a
/// durable identifier. It's now the full SHA-256 hex digest, unprefixed.
#[test]
fn plan_id_is_a_full_sha256_hex_digest_with_no_phase_prefix() {
    let source = root("token-source");
    let common = root("token-card").join("Assets/tau/common");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&common).unwrap();
    fs::write(source.join("01.mp3"), b"music").unwrap();
    let sync_plan = plan(
        std::slice::from_ref(&source),
        &common,
        "/Assets/tau/common/",
        PlanOptions::default(),
        &mut None,
    )
    .unwrap();
    assert_eq!(sync_plan.id.len(), 64);
    assert!(sync_plan.id.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(!sync_plan.id.starts_with("T2-"));
    fs::remove_dir_all(source).unwrap();
    fs::remove_dir_all(common.ancestors().nth(2).unwrap()).unwrap();
}

#[test]
fn sync_is_plan_first_and_leaves_sources_untouched() {
    let source = root("source");
    let common = root("card").join("Assets/tau/common");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&common).unwrap();
    fs::write(source.join("01 Nausicaä.mp3"), b"music").unwrap();
    let original = sha256_file(&source.join("01 Nausicaä.mp3")).unwrap();
    let sync_plan = plan(
        std::slice::from_ref(&source),
        &common,
        "/Assets/tau/common/",
        PlanOptions::default(),
        &mut None,
    )
    .unwrap();
    assert!(common.read_dir().unwrap().next().is_none());
    assert!(execute(&sync_plan, "wrong", &mut None).is_err());
    let report = execute(&sync_plan, &sync_plan.id, &mut None).unwrap();
    assert_eq!(report.copied, 1);
    assert_eq!(
        sha256_file(&source.join("01 Nausicaä.mp3")).unwrap(),
        original
    );
    assert!(common.join("tau-library.tdb").is_file());
    assert!(
        common
            .join(source.file_name().unwrap())
            .join("01 Nausicaa.mp3")
            .is_file()
    );
    let retry = plan(
        std::slice::from_ref(&source),
        &common,
        "/Assets/tau/common/",
        PlanOptions::default(),
        &mut None,
    )
    .unwrap();
    assert_eq!(retry.items[0].state, CopyState::Same);
    let no_op = execute(&retry, &retry.id, &mut None).unwrap();
    assert_eq!(no_op.copied, 0);
    fs::remove_dir_all(source).unwrap();
    fs::remove_dir_all(common.ancestors().nth(2).unwrap()).unwrap();
}

#[test]
fn failed_copy_keeps_the_previous_index() {
    let source = root("changed-source");
    let common = root("previous-index").join("Assets/tau/common");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&common).unwrap();
    let media = source.join("01 Track.mp3");
    fs::write(&media, b"before").unwrap();
    let plan = plan(
        std::slice::from_ref(&source),
        &common,
        "/Assets/tau/common/",
        PlanOptions::default(),
        &mut None,
    )
    .unwrap();
    let index = common.join("tau-library.tdb");
    fs::write(&index, b"previous index bytes").unwrap();
    fs::write(&media, b"after!").unwrap();
    assert!(execute(&plan, &plan.id, &mut None).is_err());
    assert_eq!(fs::read(index).unwrap(), b"previous index bytes");
    fs::remove_dir_all(source).unwrap();
    fs::remove_dir_all(common.ancestors().nth(2).unwrap()).unwrap();
}

#[test]
fn mirror_requires_second_confirmation_and_verified_backup() {
    let source = root("mirror-source");
    let card = root("mirror-card");
    let common = card.join("Assets/tau/common");
    let backup = root("mirror-backup");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&common).unwrap();
    fs::write(source.join("01 Keep.mp3"), b"keep").unwrap();
    fs::write(common.join("old.mp3"), b"old").unwrap();
    let plan = plan(
        std::slice::from_ref(&source),
        &common,
        "/Assets/tau/common/",
        PlanOptions {
            mirror: true,
            embed_covers: false,
            art_sidecar_pal256: false,
        },
        &mut None,
    )
    .unwrap();
    assert_eq!(plan.deletions.len(), 1);
    assert!(execute_with_mirror(&plan, &plan.id, None, Some(&backup), &mut None).is_err());
    assert!(common.join("old.mp3").exists());
    let report =
        execute_with_mirror(&plan, &plan.id, Some(&plan.id), Some(&backup), &mut None).unwrap();
    assert_eq!(report.deleted, 1);
    assert!(!common.join("old.mp3").exists());
    assert_eq!(
        fs::read(backup.join(&plan.id).join("old.mp3")).unwrap(),
        b"old"
    );
    fs::remove_dir_all(source).unwrap();
    fs::remove_dir_all(card).unwrap();
    fs::remove_dir_all(backup).unwrap();
}

#[test]
fn reviewed_cover_is_embedded_only_in_the_destination_copy() {
    let source = root("cover-source");
    let card = root("cover-card");
    let common = card.join("Assets/tau/common");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&common).unwrap();
    let track = source.join("01 Track.mp3");
    fs::write(&track, b"audio").unwrap();
    fs::write(source.join("cover.jpg"), [0xff, 0xd8, 0xff, 0xd9]).unwrap();
    let sync_plan = plan(
        std::slice::from_ref(&source),
        &common,
        "/Assets/tau/common/",
        PlanOptions {
            mirror: false,
            embed_covers: true,
            art_sidecar_pal256: false,
        },
        &mut None,
    )
    .unwrap();
    assert!(sync_plan.items[0].cover.is_some());
    execute(&sync_plan, &sync_plan.id, &mut None).unwrap();
    assert_eq!(fs::read(&track).unwrap(), b"audio");
    let copy = fs::read(
        common
            .join(source.file_name().unwrap())
            .join("01 Track.mp3"),
    )
    .unwrap();
    assert!(copy.windows(4).any(|window| window == b"APIC"));
    fs::remove_dir_all(source).unwrap();
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn art_sidecar_is_planned_once_per_album_and_written_verifiably() {
    let source = root("art-sidecar-source");
    let card = root("art-sidecar-card");
    let common = card.join("Assets/tau/common");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&common).unwrap();
    fs::write(source.join("01 Track.mp3"), b"audio one").unwrap();
    fs::write(source.join("02 Track.mp3"), b"audio two").unwrap();
    let mut real_cover = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    real_cover.push("../../testdata/images/cover455.jpg");
    fs::copy(&real_cover, source.join("cover.jpg")).unwrap();

    let sync_plan = plan(
        std::slice::from_ref(&source),
        &common,
        "/Assets/tau/common/",
        PlanOptions {
            mirror: false,
            embed_covers: false,
            art_sidecar_pal256: true,
        },
        &mut None,
    )
    .unwrap();
    // One album, two tracks -> exactly one sidecar, not one per track.
    assert_eq!(sync_plan.art_sidecars.len(), 1);
    let report = execute(&sync_plan, &sync_plan.id, &mut None).unwrap();
    assert_eq!(report.art_sidecars_written, 1);

    let sidecar_path = common
        .join(source.file_name().unwrap())
        .join(image::pal256_sidecar_name(128));
    let decoded = image::decode_tim1(&fs::read(&sidecar_path).unwrap()).unwrap();
    assert_eq!((decoded.width, decoded.height), (128, 128));

    // Re-planning and re-executing with nothing changed writes it again
    // (idempotent, not "only once ever") but the plan token is identical.
    let second_plan = plan(
        std::slice::from_ref(&source),
        &common,
        "/Assets/tau/common/",
        PlanOptions {
            mirror: false,
            embed_covers: false,
            art_sidecar_pal256: true,
        },
        &mut None,
    )
    .unwrap();
    assert_eq!(second_plan.id, sync_plan.id);

    fs::remove_dir_all(source).unwrap();
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn core_copy_preserves_paths_and_rebuilds_only_the_destination() {
    let card = root("core-copy");
    let source = card.join("Assets/tau/common");
    let destination = card.join("Assets/tau-test/common");
    fs::create_dir_all(source.join("album")).unwrap();
    fs::create_dir_all(&destination).unwrap();
    let track = source.join("album/01 Track.mp3");
    fs::write(&track, b"music").unwrap();
    let plan =
        plan_core_copy(&source, &destination, "/Assets/tau-test/common/", &mut None).unwrap();
    assert_eq!(
        plan.items[0].destination,
        plan.destination.join("album/01 Track.mp3")
    );
    execute(&plan, &plan.id, &mut None).unwrap();
    assert_eq!(fs::read(&track).unwrap(), b"music");
    assert_eq!(
        fs::read(destination.join("album/01 Track.mp3")).unwrap(),
        b"music"
    );
    assert!(destination.join("tau-library.tdb").is_file());
    fs::remove_dir_all(card).unwrap();
}

#[test]
fn core_move_requires_second_token_and_keeps_a_verified_backup() {
    let card = root("core-move");
    let backup = root("core-move-backup");
    let source = card.join("Assets/tau/common");
    let destination = card.join("Assets/tau-test/common");
    fs::create_dir_all(source.join("album")).unwrap();
    fs::create_dir_all(&destination).unwrap();
    let track = source.join("album/01 Track.mp3");
    fs::write(&track, b"music").unwrap();
    let plan =
        plan_core_copy(&source, &destination, "/Assets/tau-test/common/", &mut None).unwrap();
    assert!(
        execute_core_move(
            &plan,
            &plan.id,
            "wrong",
            &source,
            "/Assets/tau/common/",
            &backup,
            &mut None,
        )
        .is_err()
    );
    assert!(track.exists());
    let report = execute_core_move(
        &plan,
        &plan.id,
        &plan.id,
        &source,
        "/Assets/tau/common/",
        &backup,
        &mut None,
    )
    .unwrap();
    assert_eq!(report.deleted, 1);
    assert!(!track.exists());
    assert_eq!(
        fs::read(backup.join(&plan.id).join("album/01 Track.mp3")).unwrap(),
        b"music"
    );
    assert!(source.join("tau-library.tdb").is_file());
    assert!(destination.join("tau-library.tdb").is_file());
    fs::remove_dir_all(card).unwrap();
    fs::remove_dir_all(backup).unwrap();
}

#[test]
fn cancelling_partway_through_a_plan_stops_hashing() {
    let source = root("cancel-source");
    let common = root("cancel-card").join("Assets/tau/common");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&common).unwrap();
    fs::write(source.join("01.mp3"), b"one").unwrap();
    fs::write(source.join("02.mp3"), b"two").unwrap();
    let mut seen = 0;
    let mut observer = |_progress: Progress| {
        seen += 1;
        false
    };
    let mut observer: Option<&mut dyn ProgressObserver> = Some(&mut observer);
    let error = plan(
        std::slice::from_ref(&source),
        &common,
        "/Assets/tau/common/",
        PlanOptions::default(),
        &mut observer,
    )
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::Cancelled);
    assert_eq!(seen, 1);
    fs::remove_dir_all(source).unwrap();
    fs::remove_dir_all(common.ancestors().nth(2).unwrap()).unwrap();
}

#[test]
fn the_sweep_removes_only_stub_sized_appledouble_files_and_nothing_else() {
    let common = root("appledouble").join("Assets/tau/common");
    let album = common.join("Artist/Album");
    fs::create_dir_all(&album).unwrap();
    fs::write(album.join("01.mp3"), b"audio").unwrap();
    fs::write(album.join("._01.mp3"), vec![0u8; 4096]).unwrap(); // the stub beside a file
    fs::write(common.join("._Artist"), vec![0u8; 4096]).unwrap(); // the stub beside a folder
    fs::write(album.join("._gone.mp3"), vec![0u8; 4096]).unwrap(); // an orphan: its file was deleted
    fs::write(album.join("._big"), vec![0u8; 100 * 1024]).unwrap(); // too big to be a stub: somebody's file
    fs::write(album.join(".hidden"), b"x").unwrap(); // a hidden file that is not an AppleDouble
    fs::write(album.join(".DS_Store"), b"x").unwrap();
    assert_eq!(sweep_appledouble(&common), 3);
    assert!(
        album.join("01.mp3").is_file()
            && album.join(".hidden").is_file()
            && album.join(".DS_Store").is_file()
    );
    assert!(album.join("._big").is_file(), "a large ._ file was removed");
    assert!(
        !album.join("._01.mp3").exists()
            && !common.join("._Artist").exists()
            && !album.join("._gone.mp3").exists()
    );
    assert_eq!(
        sweep_appledouble(&common),
        0,
        "a second sweep finds nothing"
    );
    assert_eq!(
        sweep_appledouble(&common.join("missing")),
        0,
        "a missing folder is not an error"
    );
}

#[test]
fn a_confirmed_sync_leaves_no_appledouble_stubs_and_a_refused_one_touches_nothing() {
    let source = root("ad-sync-source");
    let card = root("ad-sync-card");
    let common = card.join("Assets/tau/common");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&common).unwrap();
    fs::write(source.join("01 Track.mp3"), b"audio one").unwrap();
    let sync_plan = plan(
        &[source],
        &common,
        "/Assets/tau/common/",
        PlanOptions {
            mirror: false,
            embed_covers: false,
            art_sidecar_pal256: false,
        },
        &mut None,
    )
    .unwrap();
    // Something the OS left behind earlier.
    let planted = common.join("._old.mp3");
    fs::write(&planted, vec![0u8; 4096]).unwrap();
    assert!(execute(&sync_plan, "wrong token", &mut None).is_err());
    assert!(planted.exists(), "a refused run changed the card");
    execute(&sync_plan, &sync_plan.id, &mut None).unwrap();
    assert!(!planted.exists(), "the stub survived a confirmed sync");
    assert!(
        sync_plan.items.iter().all(|i| i.destination.is_file()),
        "the copied file is missing"
    );
}
