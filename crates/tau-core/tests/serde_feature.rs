//! Only compiled with `--features serde` (P1-1). Proves the wire shape the
//! optional feature produces matches the conventions every hand-written
//! boundary (tau-cli, the Tauri adapter) already established in P0-2: error
//! codes as numbers, warning/status codes as snake_case strings — not the
//! derive macro's own defaults (PascalCase variant names for both).
#![cfg(feature = "serde")]

use tau_core::compare::DifferenceState;
use tau_core::sync::CopyState;
use tau_core::{ErrorCode, IndexStatus, TauError, Warning, WarningCode};

#[test]
fn error_code_serialises_as_its_stable_number_not_a_variant_name() {
    let error = TauError {
        code: ErrorCode::InvalidMediaRoot,
        message: "destination must be Assets/<platform>/common".into(),
    };
    let json = serde_json::to_string(&error).unwrap();
    assert_eq!(
        json,
        r#"{"code":30,"message":"destination must be Assets/<platform>/common"}"#
    );
    let round_tripped: TauError = serde_json::from_str(&json).unwrap();
    assert_eq!(round_tripped, error);
}

#[test]
fn unknown_error_code_number_is_rejected_not_silently_accepted() {
    assert!(serde_json::from_str::<TauError>(r#"{"code":9999,"message":"x"}"#).is_err());
}

#[test]
fn warning_code_serialises_as_snake_case_matching_as_str() {
    let warning = Warning {
        code: WarningCode::PlaylistEntriesDropped,
        message: "x.m3u: 2 line(s) not in the library, dropped".into(),
    };
    let json = serde_json::to_string(&warning).unwrap();
    assert_eq!(
        json,
        r#"{"code":"playlist_entries_dropped","message":"x.m3u: 2 line(s) not in the library, dropped"}"#
    );
    assert_eq!(warning.code.as_str(), "playlist_entries_dropped");
    let round_tripped: Warning = serde_json::from_str(&json).unwrap();
    assert_eq!(round_tripped, warning);
}

#[test]
fn difference_state_and_copy_state_are_snake_case() {
    assert_eq!(
        serde_json::to_string(&DifferenceState::OnlyLeft).unwrap(),
        r#""only_left""#
    );
    assert_eq!(serde_json::to_string(&CopyState::New).unwrap(), r#""new""#);
}

#[test]
fn index_status_round_trips_through_json() {
    let ready = IndexStatus::Ready { tracks: 42 };
    let json = serde_json::to_string(&ready).unwrap();
    let back: IndexStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ready);
    assert_eq!(
        serde_json::to_string(&IndexStatus::NoIndex).unwrap(),
        r#""no_index""#
    );
}

#[test]
fn pathbuf_serialises_as_a_plain_string_like_display() {
    let path = std::path::PathBuf::from("/Users/me/Music/Al bum/01.mp3");
    let json = serde_json::to_string(&path).unwrap();
    assert_eq!(
        json,
        serde_json::to_string(&path.display().to_string()).unwrap()
    );
}

/// The library workbench UI (ui/src/lib/types.ts) sends this exact JSON to
/// `plan_changes` / `execute_changes`. If the engine's shape drifts, the real
/// app would fail at run time in a way no browser-mock test can see.
#[test]
fn change_request_accepts_exactly_what_the_ui_sends() {
    let json = r#"{
        "library_root": "/Users/me/Music",
        "add_albums": ["Miles Davis/Kind of Blue"],
        "remove_albums": ["Charles Mingus/Ah Um"],
        "edits": [
            {"album_id": "John Coltrane/Blue Train", "track": null,
             "fields": {"title": null, "artist": null, "album": "Blue Train (Remaster)", "album_artist": null, "year": "1958"},
             "cover": "/Users/me/cover.jpg"},
            {"album_id": "A/B", "track": "A/B/01.mp3",
             "fields": {"title": "Better", "artist": null, "album": null, "album_artist": null, "year": null},
             "cover": null}
        ],
        "options": {"mirror": false, "embed_covers": true, "art_sidecar_pal256": false}
    }"#;
    let request: tau_core::changes::ChangeRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.add_albums, ["Miles Davis/Kind of Blue"]);
    assert_eq!(request.edits.len(), 2);
    assert_eq!(request.edits[0].fields.album.as_deref(), Some("Blue Train (Remaster)"));
    assert_eq!(request.edits[0].cover.as_deref().map(|p| p.to_str().unwrap()), Some("/Users/me/cover.jpg"));
    assert_eq!(request.edits[1].track.as_deref(), Some("A/B/01.mp3"));
    assert!(request.options.embed_covers && !request.options.mirror);
}

#[test]
fn library_listing_serialises_the_fields_the_ui_reads() {
    let listing = tau_core::workbench::LibraryListing {
        albums: vec![tau_core::workbench::AlbumInfo {
            id: "A/B".into(),
            dest_id: "A/B".into(),
            title: "B".into(),
            artist: "A".into(),
            year: Some("1959".into()),
            tracks: 5,
            bytes: 10,
            has_cover: true,
        }],
        ..Default::default()
    };
    let value: serde_json::Value = serde_json::to_value(&listing).unwrap();
    for key in ["id", "dest_id", "title", "artist", "year", "tracks", "bytes", "has_cover"] {
        assert!(value["albums"][0].get(key).is_some(), "missing {key}");
    }
    for key in ["albums", "tracks", "playlists", "warnings", "limits"] {
        assert!(value.get(key).is_some(), "missing {key}");
    }
    assert_eq!(value["limits"]["max_tracks"], 16_384);
    assert_eq!(value["limits"]["max_albums"], 2_048);
}
