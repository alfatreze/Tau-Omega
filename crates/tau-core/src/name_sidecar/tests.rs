use super::*;

fn track(rel: &str, file: &str, title: Option<&str>, idx_title: &str) -> TrackSide {
    TrackSide {
        rel: rel.into(),
        file: file.into(),
        title: title.map(String::from),
        artist: Some("Artist".into()),
        album: Some("Album".into()),
        idx_title: idx_title.into(),
        idx_artist: "Artist".into(),
        idx_album: "Album".into(),
    }
}

fn side_of(tracks: Vec<TrackSide>) -> NameSide {
    NameSide {
        tracks,
        albums: vec![AlbumSide {
            title: Some("Album".into()),
            artist: Some("Artist".into()),
            idx_title: "Album".into(),
            idx_artist: "Artist".into(),
        }],
        artists: vec![ArtistSide {
            name: Some("Artist".into()),
            idx_name: "Artist".into(),
        }],
    }
}

#[test]
fn clean_text_follows_spec_3() {
    assert_eq!(
        clean_text(Some("a\tb\u{0085}c"), 127).as_deref(),
        Some("a b c")
    );
    assert_eq!(
        clean_text(Some("Cafe\u{301}"), 127).as_deref(),
        Some("Café")
    );
    assert_eq!(clean_text(Some("  "), 127), None);
    assert_eq!(clean_text(None, 127), None);
    // a cut never leaves a dangling combining mark or joiner
    assert_eq!(clean_text(Some("abe\u{301}"), 3).as_deref(), Some("abé"));
    // a joiner left at the end by the cut is dropped too
    assert_eq!(clean_text(Some("ab\u{200d}c"), 3).as_deref(), Some("ab"));
    let cut = clean_text(Some(&format!("{}\u{301}", "x".repeat(127))), 127).unwrap();
    assert_eq!(cut.chars().count(), 127);
}

#[test]
fn a_plain_library_gets_no_sidecar() {
    let side = side_of(vec![track("A/01.mp3", "01.mp3", Some("Song"), "Song")]);
    let s = build_sidecar(7, &side, &BTreeMap::new(), (1, 1, 1)).unwrap();
    assert!(s.is_empty(), "{:?}", s.manifest);
}

#[test]
fn only_what_differs_is_recorded() {
    let side = side_of(vec![
        track("A/01.mp3", "01.mp3", Some("Song"), "Song"),
        track("A/02.mp3", "02.mp3", Some("Песня"), "Pesnia"),
        track("A/03.mp3", "03.mp3", Some("Same"), "Same"),
    ]);
    let mut origins = BTreeMap::new();
    origins.insert("A/03.mp3".to_string(), "A/Тот самый.mp3".to_string());
    let s = build_sidecar(7, &side, &origins, (3, 1, 1)).unwrap();
    let records = s.manifest["tracks"].as_array().unwrap();
    // 01 carries nothing extra; 02 has a tag the index folded; 03 has another original path
    let ids: Vec<_> = records.iter().map(|r| r["id"].as_u64().unwrap()).collect();
    assert_eq!(ids, [1, 2]);
    assert_eq!(records[0]["title"], "Песня");
    assert_eq!(records[1]["orig"], "A/Тот самый.mp3");
    let parsed = parse_bin(&s.binary.unwrap(), Some(7), Some((3, 1, 1))).unwrap();
    assert_eq!(parsed.tracks[0], [None, None, None, None]);
    assert_eq!(parsed.tracks[1][1].as_deref(), Some("Песня"));
    assert_eq!(parsed.tracks[2][0].as_deref(), Some("Тот самый.mp3"));
    assert_eq!(parsed.albums[0], [None, None]);
}

#[test]
fn the_reader_refuses_what_the_firmware_refuses() {
    let side = side_of(vec![track("A/01.mp3", "01.mp3", Some("Песня"), "Pesnia")]);
    let bin = build_sidecar(9, &side, &BTreeMap::new(), (1, 1, 1))
        .unwrap()
        .binary
        .unwrap();
    assert!(parse_bin(&bin, Some(9), Some((1, 1, 1))).is_ok());
    assert!(
        parse_bin(&bin, Some(10), Some((1, 1, 1))).is_err(),
        "another build id"
    );
    assert!(
        parse_bin(&bin, Some(9), Some((2, 1, 1))).is_err(),
        "other counts"
    );
    assert!(
        parse_bin(&bin[..bin.len() - 1], None, None).is_err(),
        "short"
    );
    let mut bad = bin.clone();
    bad[40] ^= 1; // padding is not covered; flip the CRC-covered version instead
    bad[4] ^= 0x7F;
    assert!(parse_bin(&bad, None, None).is_err(), "header crc / version");
    let mut bad = bin.clone();
    let last = bad.len() - 1;
    bad[last] = b'x'; // the pool must end with NUL
    assert!(parse_bin(&bad, None, None).is_err());
}

#[test]
fn over_the_cap_the_file_names_go_first_then_the_binary() {
    // 7,000 tracks with long original file names and a short title each: names alone overflow.
    let long = "n".repeat(200);
    let tracks: Vec<_> = (0..7000)
        .map(|i| {
            let mut t = track(
                &format!("A/{i:05}.mp3"),
                &format!("{i:05}.mp3"),
                Some(&format!("Тема {i}")),
                &format!("Tema {i}"),
            );
            t.rel = format!("A/{i:05}.mp3");
            t
        })
        .collect();
    let mut origins = BTreeMap::new();
    for t in &tracks {
        origins.insert(t.rel.clone(), format!("A/{long}{}.mp3", t.file));
    }
    let side = NameSide {
        tracks,
        ..side_of(vec![])
    };
    let side = NameSide {
        albums: vec![],
        artists: vec![],
        ..side
    };
    let s = build_sidecar(1, &side, &origins, (7000, 0, 0)).unwrap();
    let bin = s.binary.expect("fits once the file names are dropped");
    assert!(bin.len() <= BIN_CAP);
    let parsed = parse_bin(&bin, Some(1), Some((7000, 0, 0))).unwrap();
    assert!(
        parsed.tracks.iter().all(|t| t[0].is_none()),
        "file names dropped"
    );
    assert!(
        parsed.tracks[5][1]
            .as_deref()
            .is_some_and(|t| t.starts_with("Тема"))
    );
}

fn scratch(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("tau-names-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn index_for(entries: &[crate::Entry]) -> (Vec<u8>, NameSide) {
    crate::build_index_with_names(entries, &[], "/Assets/tau/common/", &mut Vec::new()).unwrap()
}

fn entry(rel: &str, tags: &[(&str, &str)]) -> crate::Entry {
    let (dir, file) = rel.rsplit_once('/').unwrap();
    crate::Entry {
        rel: rel.into(),
        dir: dir.into(),
        file: file.into(),
        tags: tags
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        secs: 100,
        fmt: 0,
    }
}

#[test]
fn written_files_bind_to_the_index_and_stale_ones_are_removed() {
    let dir = scratch("write");
    let entries = [
        entry(
            "Pesni/01.mp3",
            &[("TIT2", "Песня"), ("TPE1", "Группа"), ("TALB", "Альбом")],
        ),
        entry(
            "Plain/01.mp3",
            &[("TIT2", "Song"), ("TPE1", "Artist"), ("TALB", "Album")],
        ),
    ];
    let (index, side) = index_for(&entries);
    let mut origins = BTreeMap::new();
    origins.insert("Pesni/01.mp3".to_string(), "Песни/01 Песня.mp3".to_string());
    let outcome = write_sidecar(&dir, &index, &side, &origins).unwrap();
    assert_eq!(
        outcome.records, 1,
        "only the Cyrillic track says more than the index"
    );
    let bin = fs::read(dir.join(BIN_NAME)).unwrap();
    let ix = crate::parse(&index).unwrap();
    let parsed = parse_bin(&bin, Some(ix.build_id), Some((2, 2, 2))).unwrap();
    assert!(
        parsed
            .tracks
            .iter()
            .any(|t| t[1].as_deref() == Some("Песня"))
    );
    assert!(
        parsed
            .artists
            .iter()
            .any(|a| a.as_deref() == Some("Группа"))
    );
    assert!(
        parsed
            .albums
            .iter()
            .any(|a| a[0].as_deref() == Some("Альбом"))
    );
    let manifest: Value = serde_json::from_slice(&fs::read(dir.join(JSON_NAME)).unwrap()).unwrap();
    assert_eq!(manifest["index_build_id"], ix.build_id);
    assert_eq!(manifest["tracks"][0]["orig"], "Песни/01 Песня.mp3");
    // no temp or previous files are left behind
    let left: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left.len(), 2, "{left:?}");

    // A later rebuild without origins keeps the earlier originals for files still there.
    let (index2, side2) = index_for(&[entries[0].clone(), entries[1].clone()]);
    write_sidecar(&dir, &index2, &side2, &BTreeMap::new()).unwrap();
    let manifest: Value = serde_json::from_slice(&fs::read(dir.join(JSON_NAME)).unwrap()).unwrap();
    assert_eq!(manifest["tracks"][0]["orig"], "Песни/01 Песня.mp3");

    // The Cyrillic track goes away: nothing is left to record, both files are removed.
    let (index3, side3) = index_for(&[entries[1].clone()]);
    let outcome = write_sidecar(&dir, &index3, &side3, &BTreeMap::new()).unwrap();
    assert!(outcome.removed);
    assert!(!dir.join(JSON_NAME).exists() && !dir.join(BIN_NAME).exists());
    fs::remove_dir_all(dir).unwrap();
}

/// Byte for byte against Tau Alpha's reference (`tools/name_sidecar.py build_sidecar`) on the
/// shared corpus: `TAU_SIDECAR_CASE=<json from that Python>`. Nothing is copied into this repository.
#[test]
#[ignore]
fn matches_the_reference_writer() {
    let Ok(path) = std::env::var("TAU_SIDECAR_CASE") else {
        return;
    };
    let case: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let opt = |v: &Value| v.as_str().map(String::from);
    let side = NameSide {
        tracks: case["side"]["tracks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| TrackSide {
                rel: t["rel"].as_str().unwrap().into(),
                file: t["file"].as_str().unwrap().into(),
                title: opt(&t["title"]),
                artist: opt(&t["artist"]),
                album: opt(&t["album"]),
                idx_title: t["idx_title"].as_str().unwrap().into(),
                idx_artist: t["idx_artist"].as_str().unwrap().into(),
                idx_album: t["idx_album"].as_str().unwrap().into(),
            })
            .collect(),
        albums: case["side"]["albums"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| AlbumSide {
                title: opt(&a["title"]),
                artist: opt(&a["artist"]),
                idx_title: a["idx_title"].as_str().unwrap().into(),
                idx_artist: a["idx_artist"].as_str().unwrap().into(),
            })
            .collect(),
        artists: case["side"]["artists"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| ArtistSide {
                name: opt(&a["name"]),
                idx_name: a["idx_name"].as_str().unwrap().into(),
            })
            .collect(),
    };
    let origins: BTreeMap<String, String> = case["origins"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().into()))
        .collect();
    let counts = (side.tracks.len(), side.albums.len(), side.artists.len());
    let s = build_sidecar(
        case["bid"].as_u64().unwrap() as u32,
        &side,
        &origins,
        counts,
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(manifest_bytes(&s.manifest)).unwrap(),
        case["manifest"].as_str().unwrap(),
        "manifest differs"
    );
    let hex: String = s
        .binary
        .unwrap()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(hex, case["bin"].as_str().unwrap(), "binary differs");
}
