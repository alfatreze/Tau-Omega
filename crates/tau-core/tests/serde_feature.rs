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
    assert_eq!(
        serde_json::to_string(&CopyState::New).unwrap(),
        r#""new""#
    );
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
    assert_eq!(json, serde_json::to_string(&path.display().to_string()).unwrap());
}
