//! Read-only firmware diagnostics and persisted settings.

use crate::{ErrorCode, TauError};
use serde_json::Value;
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PersistedSetting {
    pub id: u64,
    pub kind: String,
    pub value: Value,
}

/// Reads an Interact persisted-settings file without mutating its source.
///
/// The real APF layout nests `variables` under `interact_persist`
/// (`{"interact_persist": {"magic": "...", "variables": [...]}}`) -- found by
/// copying real hardware-captured `interact_persist.json` files as test
/// fixtures rather than trusting a hand-written shape, the same class of bug
/// `slots_have_library` had (this one meant the Settings screen never
/// actually read anything from a real card). A bare top-level `variables` is
/// still accepted, for older hand-written fixtures/tools.
pub fn read_persisted_settings(path: impl AsRef<Path>) -> Result<Vec<PersistedSetting>, TauError> {
    let bytes = fs::read(path)?;
    let json: Value = serde_json::from_slice(&bytes).map_err(|error| {
        TauError::e(
            ErrorCode::Json,
            format!("invalid persisted settings: {error}"),
        )
    })?;
    Ok(json
        .pointer("/interact_persist/variables")
        .or_else(|| json.get("variables"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some(PersistedSetting {
                id: entry.get("id")?.as_u64()?,
                kind: entry
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                value: entry.get("val").cloned().unwrap_or(Value::Null),
            })
        })
        .collect())
}

/// The persist ids (20-23) that carry the firmware's Check-report summary,
/// per `tau-alpha/tools/decode_tau_suite.py`'s `words_from_interact`
/// default. **Overloaded**: every shipped core also declares these same ids
/// as legacy playlist state, and nothing in the file distinguishes the two
/// uses -- [`decode_check_summary`] rejects a value that isn't format 1
/// rather than guessing.
pub const CHECK_SUMMARY_IDS: [u64; 4] = [20, 21, 22, 23];

const CHECK_FORMAT: u32 = 1;
const PROFILES: [&str; 8] = [
    "none",
    "USER CHECK",
    "QUICK",
    "STANDARD",
    "FULL",
    "ENDURANCE",
    "CUSTOM",
    "RESTART-SET",
];
const VERDICTS: [&str; 4] = [
    "no result",
    "all checks passed",
    "some checks failed",
    "check incomplete",
];
const TESTS: [&str; 13] = [
    "SDRAM window test",
    "SDRAM read/write cost",
    "PSRAM window test",
    "Cold code test",
    "Playlist / library check",
    "Playback counters",
    "Timings",
    "Stress R1 (30 s)",
    "Stress R2 (30 s)",
    "Stress R3 (30 s)",
    "Soak",
    "Track changes (10)",
    "Cold code x20",
];

fn test_name(i: u32) -> String {
    TESTS
        .get(i as usize)
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("test {i}"))
}

/// A decoded firmware Check-report summary: byte-exact port of
/// `tau-alpha/tools/decode_tau_suite.py`'s `unpack_words` (see
/// `docs/TEST_SUITE_SPEC.md` there for the bitfield layout). Verified
/// against real hardware-captured `interact_persist.json` fixtures, cross-
/// checked against that Python reference's own output.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CheckSummary {
    pub profile: String,
    pub run: u32,
    pub verdict: String,
    pub passed: Vec<String>,
    pub failed: Vec<String>,
    pub worst_access_cycles: u32,
    pub cold_cycles_per_word: f64,
    pub late_underruns: u32,
    pub draw_stall_ms: u32,
    pub last_load_s: f64,
    pub library_error: u32,
    pub cold_error: u32,
    pub firmware_minor: u32,
}

/// Decodes four raw persist words (see [`CHECK_SUMMARY_IDS`]) as a firmware
/// Check-report summary. Rejects anything that isn't format 1 -- expected
/// and normal on most cards, since these same ids are legacy playlist state
/// everywhere a Check has never been run.
pub fn decode_check_summary(words: [u32; 4]) -> Result<CheckSummary, TauError> {
    if words.iter().any(|&word| word > 0x7FFF_FFFF) {
        return Err(TauError::e(
            ErrorCode::NotACheckSummary,
            "check summary words must be 31-bit values",
        ));
    }
    if words[0] & 15 != CHECK_FORMAT {
        return Err(TauError::e(
            ErrorCode::NotACheckSummary,
            "these persist ids do not hold a Check summary (likely legacy playlist state)",
        ));
    }
    let passed_mask = words[1] & 0x7FFF;
    let failed_mask = (words[1] >> 15) & 0x7FFF;
    Ok(CheckSummary {
        profile: PROFILES[((words[0] >> 4) & 7) as usize].to_string(),
        run: (words[0] >> 7) & 255,
        verdict: VERDICTS[((words[0] >> 15) & 3) as usize].to_string(),
        passed: (0..15)
            .filter(|i| (passed_mask >> i) & 1 != 0)
            .map(test_name)
            .collect(),
        failed: (0..15)
            .filter(|i| (failed_mask >> i) & 1 != 0)
            .map(test_name)
            .collect(),
        worst_access_cycles: words[2] & 511,
        cold_cycles_per_word: ((words[2] >> 9) & 511) as f64 / 10.0,
        late_underruns: (words[2] >> 18) & 63,
        draw_stall_ms: (words[2] >> 24) & 127,
        last_load_s: (words[3] & 4095) as f64 / 10.0,
        library_error: (words[3] >> 12) & 63,
        cold_error: (words[3] >> 18) & 63,
        firmware_minor: (words[3] >> 24) & 127,
    })
}

/// Reads a Check summary directly from a persisted-settings file, pulling
/// [`CHECK_SUMMARY_IDS`] out of whatever [`read_persisted_settings`] found.
pub fn read_check_summary(path: impl AsRef<Path>) -> Result<CheckSummary, TauError> {
    let settings = read_persisted_settings(path)?;
    let mut words = [0u32; 4];
    for (slot, id) in words.iter_mut().zip(CHECK_SUMMARY_IDS) {
        *slot = settings
            .iter()
            .find(|setting| setting.id == id)
            .and_then(|setting| setting.value.as_u64())
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| {
                TauError::e(
                    ErrorCode::NotACheckSummary,
                    format!("persist id {id} not found or not a plain integer"),
                )
            })?;
    }
    decode_check_summary(words)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn reads_known_variables_without_mutating_source() {
        let path = std::env::temp_dir().join(format!(
            "tau-settings-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(
            &path,
            br#"{"variables":[{"id":10,"type":"u32","val":7},{"id":24,"type":"u32","val":123}]}"#,
        )
        .unwrap();
        let settings = read_persisted_settings(&path).unwrap();
        assert_eq!(settings.len(), 2);
        assert_eq!(settings[0].id, 10);
        assert_eq!(settings[0].kind, "u32");
        assert_eq!(settings[0].value, serde_json::json!(7));
        assert!(path.is_file());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn reads_the_real_apf_nested_shape() {
        // The real layout (every hardware-captured interact_persist.json):
        // `{"interact_persist": {"magic": "...", "variables": [...]}}`, not
        // a bare top-level `variables`. This is the bug reads_known_variables
        // above didn't catch, because its own fixture used the flat shape.
        let path = std::env::temp_dir().join(format!(
            "tau-settings-nested-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(
            &path,
            br#"{"interact_persist":{"magic":"APF_VER_1","variables":[{"id":10,"type":"slider_u32","val":7}]}}"#,
        )
        .unwrap();
        let settings = read_persisted_settings(&path).unwrap();
        assert_eq!(settings.len(), 1);
        assert_eq!(settings[0].id, 10);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_invalid_json() {
        let path = std::env::temp_dir().join("tau-settings-invalid.json");
        fs::write(&path, b"not json").unwrap();
        assert!(read_persisted_settings(&path).is_err());
        fs::remove_file(path).unwrap();
    }

    fn testdata(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/interact_persist")
            .join(name)
    }

    #[test]
    fn decodes_a_real_all_passed_check_summary() {
        // Ground truth cross-checked against tau-alpha's own
        // tools/decode_tau_suite.py --interact against the same file.
        let summary = read_check_summary(testdata("check_all_passed.json")).unwrap();
        assert_eq!(summary.profile, "USER CHECK");
        assert_eq!(summary.run, 5);
        assert_eq!(summary.verdict, "all checks passed");
        assert_eq!(
            summary.passed,
            vec![
                "SDRAM window test",
                "SDRAM read/write cost",
                "PSRAM window test",
                "Cold code test",
                "Playlist / library check",
                "Playback counters",
                "Timings",
            ]
        );
        assert!(summary.failed.is_empty());
        assert_eq!(summary.worst_access_cycles, 356);
        assert_eq!(summary.cold_cycles_per_word, 0.0);
        assert_eq!(summary.late_underruns, 0);
        assert_eq!(summary.last_load_s, 0.4);
        assert_eq!(summary.firmware_minor, 3);
    }

    #[test]
    fn decodes_a_real_some_failed_check_summary() {
        let summary = read_check_summary(testdata("check_some_failed.json")).unwrap();
        assert_eq!(summary.profile, "USER CHECK");
        assert_eq!(summary.run, 2);
        assert_eq!(summary.verdict, "some checks failed");
        assert_eq!(
            summary.failed,
            vec!["SDRAM window test", "SDRAM read/write cost"]
        );
        assert_eq!(summary.last_load_s, 15.2);
    }

    #[test]
    fn refuses_a_real_card_where_the_same_ids_hold_legacy_playlist_state() {
        // Persist ids 20-23 are overloaded (docs/FIRMWARE_SYNC.md): every
        // core also declares them as legacy playlist state, and this real
        // fixture is exactly that case, not a Check summary.
        assert!(read_check_summary(testdata("legacy_no_check.json")).is_err());
    }
}
