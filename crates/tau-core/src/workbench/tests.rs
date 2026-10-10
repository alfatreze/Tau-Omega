use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "tau-wb-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            + crate::test_uniq()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}
fn library(root: &Path) {
    for (dir, files) in [
        (
            "Miles Davis/Kind of Blue",
            vec!["01 So What.mp3", "02 Blue in Green.mp3"],
        ),
        ("John Coltrane/Blue Train", vec!["01 Blue Train.flac"]),
        ("Mingus/Ah Um", vec!["01 Goodbye Pork Pie Hat.mp3"]),
    ] {
        fs::create_dir_all(root.join(dir)).unwrap();
        for f in files {
            fs::write(root.join(dir).join(f), format!("audio:{dir}/{f}")).unwrap();
        }
    }
    fs::write(root.join("Miles Davis/Kind of Blue/cover.jpg"), b"jpg").unwrap();
}
fn card() -> (PathBuf, PathBuf) {
    let root = tmp("card");
    let common = root.join("Assets/tau/common");
    fs::create_dir_all(&common).unwrap();
    (root, common)
}

#[test]
fn lists_albums_with_sizes_and_counts() {
    let lib = tmp("lib");
    library(&lib);
    let listing = list_library(&lib, &mut None).unwrap();
    assert_eq!(listing.albums.len(), 3);
    let kob = listing
        .albums
        .iter()
        .find(|a| a.title == "Kind of Blue")
        .unwrap();
    assert_eq!(kob.id, "Miles Davis/Kind of Blue");
    assert_eq!(kob.tracks, 2);
    assert!(kob.bytes > 0 && kob.has_cover);
    assert_eq!(listing.tracks.len(), 4);
}

fn png(width: u16, height: u16) -> Vec<u8> {
    let rgb: Vec<u8> = (0..width as usize * height as usize)
        .flat_map(|i| [(i % 251) as u8, 90, 160])
        .collect();
    crate::image::rgb8_to_png(width, height, &rgb).unwrap()
}
fn decode_b64(s: &str) -> Vec<u8> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD.decode(s).unwrap()
}
fn png_size(bytes: &[u8]) -> (u32, u32) {
    (
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    )
}

#[test]
fn a_folder_cover_becomes_a_small_thumbnail() {
    let lib = tmp("thumb-folder");
    fs::create_dir_all(lib.join("A/B")).unwrap();
    fs::write(lib.join("A/B/01.mp3"), b"audio").unwrap();
    fs::write(lib.join("A/B/cover.png"), png(200, 100)).unwrap();
    let t = album_thumbnails(&lib, &["A/B".to_string()], 96).unwrap();
    let bytes = decode_b64(t[0].png_base64.as_ref().unwrap());
    assert_eq!(
        &bytes[..8],
        &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
    );
    assert_eq!(png_size(&bytes), (96, 48)); // longest side 96, proportions kept
    // a small picture is never enlarged
    fs::write(lib.join("A/B/cover.png"), png(40, 40)).unwrap();
    let t = album_thumbnails(&lib, &["A/B".to_string()], 96).unwrap();
    assert_eq!(
        png_size(&decode_b64(t[0].png_base64.as_ref().unwrap())),
        (40, 40)
    );
}

#[test]
fn an_embedded_mp3_or_flac_picture_is_found() {
    let lib = tmp("thumb-embedded");
    let picture = png(64, 64);
    // MP3: ID3v2.3 with an APIC frame (encoding 0, mime, type 3, empty description)
    let mut apic = vec![0u8];
    apic.extend_from_slice(b"image/png\0");
    apic.push(3);
    apic.push(0);
    apic.extend_from_slice(&picture);
    let mut frame = b"APIC".to_vec();
    frame.extend_from_slice(&(apic.len() as u32).to_be_bytes());
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(&apic);
    let mut mp3 = b"ID3".to_vec();
    mp3.extend_from_slice(&[3, 0, 0]);
    let n = frame.len();
    mp3.extend_from_slice(&[
        ((n >> 21) & 127) as u8,
        ((n >> 14) & 127) as u8,
        ((n >> 7) & 127) as u8,
        (n & 127) as u8,
    ]);
    mp3.extend_from_slice(&frame);
    mp3.extend_from_slice(&[0xff, 0xfb, 0x90, 0]);
    fs::create_dir_all(lib.join("M/one")).unwrap();
    fs::write(lib.join("M/one/01.mp3"), mp3).unwrap();
    // FLAC: STREAMINFO then a PICTURE block
    let mut block = Vec::new();
    {
        let v = 3u32;
        block.extend_from_slice(&v.to_be_bytes());
    }
    block.extend_from_slice(&9u32.to_be_bytes());
    block.extend_from_slice(b"image/png");
    block.extend_from_slice(&0u32.to_be_bytes());
    for v in [64u32, 64, 24, 0] {
        block.extend_from_slice(&v.to_be_bytes());
    }
    block.extend_from_slice(&(picture.len() as u32).to_be_bytes());
    block.extend_from_slice(&picture);
    let mut flac = b"fLaC".to_vec();
    flac.extend_from_slice(&[0, 0, 0, 34]);
    flac.extend_from_slice(&[0u8; 34]);
    flac.push(0x80 | 6);
    flac.extend_from_slice(&(block.len() as u32).to_be_bytes()[1..]);
    flac.extend_from_slice(&block);
    flac.extend_from_slice(&[0xff, 0xf8]);
    fs::create_dir_all(lib.join("F/two")).unwrap();
    fs::write(lib.join("F/two/01.flac"), flac).unwrap();
    let t = album_thumbnails(&lib, &["M/one".to_string(), "F/two".to_string()], 96).unwrap();
    for entry in &t {
        assert_eq!(
            png_size(&decode_b64(
                entry
                    .png_base64
                    .as_ref()
                    .unwrap_or_else(|| panic!("no picture for {}", entry.id))
            )),
            (64, 64)
        );
    }
}

#[test]
fn albums_without_or_with_broken_pictures_give_none_not_an_error() {
    let lib = tmp("thumb-none");
    for d in ["A/none", "A/broken", "A/escape"] {
        fs::create_dir_all(lib.join(d)).unwrap();
        fs::write(lib.join(d).join("01.mp3"), b"audio").unwrap();
    }
    fs::write(lib.join("A/broken/cover.jpg"), b"not an image").unwrap();
    let ids: Vec<String> = ["A/none", "A/broken", "../outside", "A/missing"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let t = album_thumbnails(&lib, &ids, 96).unwrap();
    assert_eq!(t.len(), 4);
    assert!(t.iter().all(|x| x.png_base64.is_none()));
}

#[test]
fn dest_dir_is_device_ascii() {
    assert_eq!(dest_dir("Björk/Post"), "Bjork/Post");
}

#[test]
fn selection_plan_copies_only_chosen_albums_and_keeps_structure() {
    let lib = tmp("sel-lib");
    library(&lib);
    let (_root, common) = card();
    let plan = plan_selection(
        &lib,
        &["Miles Davis/Kind of Blue".to_string()],
        &common,
        "/Assets/tau/common/",
        sync::PlanOptions::default(),
        &mut None,
    )
    .unwrap();
    assert_eq!(plan.items.len(), 2);
    assert!(plan.items.iter().all(|i| {
        i.destination
            .to_string_lossy()
            .replace('\\', "/")
            .contains("Miles Davis/Kind of Blue")
    }));
    sync::execute(&plan, &plan.id, &mut None).unwrap();
    // re-running is an in-place no-op
    let again = plan_selection(
        &lib,
        &["Miles Davis/Kind of Blue".to_string()],
        &common,
        "/Assets/tau/common/",
        sync::PlanOptions::default(),
        &mut None,
    )
    .unwrap();
    assert!(again.items.iter().all(|i| i.state == sync::CopyState::Same));
    // the source is untouched
    assert!(
        lib.join("Mingus/Ah Um/01 Goodbye Pork Pie Hat.mp3")
            .is_file()
    );
}

#[test]
fn selection_rejects_paths_that_escape_the_library() {
    let lib = tmp("esc-lib");
    library(&lib);
    let (_r, common) = card();
    let err = plan_selection(
        &lib,
        &["../etc".into()],
        &common,
        "/Assets/tau/common/",
        sync::PlanOptions::default(),
        &mut None,
    )
    .unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidPathReference);
}

fn synced_card() -> (PathBuf, PathBuf, PathBuf) {
    let lib = tmp("rm-lib");
    library(&lib);
    let (root, common) = card();
    let plan = plan_selection(
        &lib,
        &[
            "Miles Davis/Kind of Blue".to_string(),
            "John Coltrane/Blue Train".to_string(),
        ],
        &common,
        "/Assets/tau/common/",
        sync::PlanOptions::default(),
        &mut None,
    )
    .unwrap();
    sync::execute(&plan, &plan.id, &mut None).unwrap();
    (lib, root, common)
}

#[test]
fn removal_backs_up_deletes_and_rebuilds_the_index() {
    let (_lib, _root, common) = synced_card();
    let backup = tmp("backup");
    let plan = plan_removal(&common, &["Miles Davis/Kind of Blue".to_string()]).unwrap();
    assert_eq!(plan.items.len(), 2); // sync embeds covers rather than copying the loose file
    let report = execute_removal(
        &plan,
        &plan.id,
        Some(&backup),
        "/Assets/tau/common/",
        &mut None,
    )
    .unwrap();
    assert_eq!(report.deleted, 2);
    assert!(!common.join("Miles Davis/Kind of Blue").exists());
    assert!(
        common
            .join("John Coltrane/Blue Train/01 Blue Train.flac")
            .is_file()
    );
    assert!(
        backup
            .join(&plan.id)
            .join("Miles Davis/Kind of Blue/01 So What.mp3")
            .is_file()
    );
    let index = fs::read(common.join("tau-library.tdb")).unwrap();
    assert!(crate::verify(&index, Some(&common)).unwrap().is_empty());
    assert_eq!(crate::parse(&index).unwrap().counts.tracks, 1);
}

#[test]
fn a_failed_backup_leaves_every_file_on_the_card() {
    let (_lib, _root, common) = synced_card();
    let backup = tmp("backup-fail");
    let plan = plan_removal(&common, &["Miles Davis/Kind of Blue".to_string()]).unwrap();
    // Make the SECOND file's backup impossible: a non-empty folder where its file should go.
    let blocker = backup
        .join(&plan.id)
        .join("Miles Davis/Kind of Blue/02 Blue in Green.mp3");
    fs::create_dir_all(blocker.join("inside")).unwrap();
    let result = execute_removal(
        &plan,
        &plan.id,
        Some(&backup),
        "/Assets/tau/common/",
        &mut None,
    );
    assert!(result.is_err());
    // The first file was backed up, but nothing was deleted from the card: no half-removed album.
    assert!(
        common
            .join("Miles Davis/Kind of Blue/01 So What.mp3")
            .is_file()
    );
    assert!(
        common
            .join("Miles Davis/Kind of Blue/02 Blue in Green.mp3")
            .is_file()
    );
    // And no unverified temp file is left in the backup.
    let leftovers: Vec<_> = walk(&backup)
        .into_iter()
        .filter(|p| p.to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
    }
    out
}

#[test]
fn removal_needs_the_matching_token_and_a_safe_backup_location() {
    let (_lib, _root, common) = synced_card();
    let plan = plan_removal(&common, &["Miles Davis/Kind of Blue".to_string()]).unwrap();
    assert_eq!(
        execute_removal(&plan, "nope", None, "/Assets/tau/common/", &mut None)
            .unwrap_err()
            .code(),
        ErrorCode::ConfirmationMismatch
    );
    assert_eq!(
        execute_removal(
            &plan,
            &plan.id,
            Some(&common.join("bak")),
            "/Assets/tau/common/",
            &mut None
        )
        .unwrap_err()
        .code(),
        ErrorCode::UnsafeBackupLocation
    );
    assert!(
        common
            .join("Miles Davis/Kind of Blue/01 So What.mp3")
            .is_file()
    );
}

#[test]
fn removal_refuses_a_file_changed_since_the_plan() {
    let (_lib, _root, common) = synced_card();
    let plan = plan_removal(&common, &["John Coltrane/Blue Train".to_string()]).unwrap();
    fs::write(
        common.join("John Coltrane/Blue Train/01 Blue Train.flac"),
        b"changed",
    )
    .unwrap();
    assert_eq!(
        execute_removal(&plan, &plan.id, None, "/Assets/tau/common/", &mut None)
            .unwrap_err()
            .code(),
        ErrorCode::SourceChangedSincePlan
    );
}

#[test]
fn removal_drops_removed_tracks_from_playlists() {
    let (_lib, _root, common) = synced_card();
    fs::write(
        common.join("Favourites.m3u"),
        "/Miles Davis/Kind of Blue/01 So What.mp3\n/John Coltrane/Blue Train/01 Blue Train.flac\n",
    )
    .unwrap();
    let plan = plan_removal(&common, &["Miles Davis/Kind of Blue".to_string()]).unwrap();
    assert_eq!(plan.playlist_updates.len(), 1);
    execute_removal(&plan, &plan.id, None, "/Assets/tau/common/", &mut None).unwrap();
    let text = fs::read_to_string(common.join("Favourites.m3u")).unwrap();
    assert!(!text.contains("So What") && text.contains("Blue Train"));
}
