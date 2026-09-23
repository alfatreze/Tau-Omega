//! Decoding the Analogue Pocket firmware's `TAUD1` diagnostics report: the
//! full Check-report record encoded in a QR code on the Pocket's screen
//! (`tau-alpha/docs/TEST_SUITE_SPEC.md` section 8), a byte-exact port of
//! `tau-alpha/tools/decode_tau_suite.py`'s `parse_record`/`from_text`. This
//! is the *full* report -- every test's value, build info, memory-timing
//! histograms, error history -- as opposed to [`crate::diag::CheckSummary`],
//! which only decodes the tiny 4-word persisted pass/fail summary.
//!
//! Reading the QR from a screenshot needs two steps this module also owns:
//! decoding the PNG ([`png`]) and locating/reading the QR grid ([`rqrr`])
//! before the `TAUD1:` text can even be extracted. Read-only throughout --
//! nothing here writes a QR code or a report, only decodes one that already
//! exists.

use crate::{ErrorCode, TauError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use std::{fs::File, io::BufReader, path::Path};

const FORMAT: u8 = 1;

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
const RESULTS: [&str; 4] = ["PASS", "FAIL", "SKIPPED", "N/A"];
const TESTS: [&str; 14] = [
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
    "Blit storm (30 s)",
];

fn test_name(id: u8) -> String {
    TESTS
        .get(id as usize)
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("test {id}"))
}

fn result_name(code: u8) -> String {
    RESULTS
        .get(code as usize)
        .map(|name| name.to_string())
        .unwrap_or_else(|| code.to_string())
}

fn bad_record(message: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidTaudRecord, message)
}

/// One `SR_T_TEST` entry (tag 3): a single Check test's id, name, pass/fail
/// state and raw value.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TaudTest {
    pub id: u8,
    pub name: String,
    pub result: String,
    pub value: u32,
    /// Only set for the Blit storm test (id 13, `CT_BLT`, B-127/B-139):
    /// the SDRAM-busy permille over the test window. `None` when the
    /// installed bitstream has no busy counter (`TAU_SDRAM_BUSY` off),
    /// which reports as a sentinel `0xFFFF` rather than a false 0%.
    pub busy_permille: Option<u32>,
    /// Only set for the Blit storm test: whether playback ran for the
    /// whole window without dropping out.
    pub audio_full: Option<bool>,
}

/// Firmware build identity (entry tag 1).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TaudBuild {
    pub firmware: String,
    pub bitstream: String,
    pub flags: u32,
    pub heap_gap: u32,
}

/// SDRAM/PSRAM read/write cost in cycles (entry tags 4/5). Older reports
/// (B-056..B-059) only carried an average and a worst case, so `read_min`/
/// `write_min` are `None` for those; a later firmware adds the best case too.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TaudCycles {
    pub read_min: Option<u64>,
    pub read_avg: u64,
    pub read_max: u64,
    pub write_min: Option<u64>,
    pub write_avg: u64,
    pub write_max: u64,
}

/// The `CT_AUD` playback-counter window (entry tag 8).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TaudAudio {
    pub late_underruns: u64,
    pub audio_full: bool,
    pub stall_ms: u64,
    pub window_s: u64,
}

/// A decoder-stage-cost measurement over one `CT_AUD` window (entry tag 13,
/// B-088/B-089): percentage of the window spent in each decode stage.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TaudDecodeProfile {
    pub h_pct: u64,
    pub i_pct: u64,
    pub s_pct: u64,
    pub r_pct: u64,
}

/// One track's result from a Decode Profile Sweep (entry tag 14, repeatable;
/// B-090..B-092/B-096). `title` is truncated to whatever fit, not
/// NUL-terminated.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TaudDecodeSweepEntry {
    pub track: u8,
    pub speed_pct: u8,
    pub h_pct: u16,
    pub i_pct: u16,
    pub s_pct: u16,
    pub r_pct: u16,
    pub title: String,
}

/// Everything outside the per-test `tests` list, keyed the same way as
/// `tau-alpha`'s own `entries` dict. A tag whose value doesn't match one of
/// the known structured shapes falls back to its raw little-endian values
/// (mirrors the Python decoder returning a plain list in that case, instead
/// of the structured dict).
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TaudEntries {
    pub build: Option<TaudBuild>,
    pub memory: Vec<u64>,
    pub sdram: Option<TaudCycles>,
    pub sdram_raw: Vec<u64>,
    pub psram: Option<TaudCycles>,
    pub psram_raw: Vec<u64>,
    pub cold: Vec<u64>,
    pub time: Vec<u64>,
    pub audio: Option<TaudAudio>,
    pub audio_raw: Vec<u64>,
    pub library: Vec<u64>,
    pub settings: Vec<u64>,
    pub errors: Vec<u64>,
    pub notes: Vec<u64>,
    pub decode_profile: Option<TaudDecodeProfile>,
    pub decode_profile_raw: Vec<u64>,
    pub decode_sweep: Vec<TaudDecodeSweepEntry>,
}

/// An entry whose tag this decoder does not recognise -- kept, not dropped,
/// so a newer firmware field is visible even before this decoder knows its
/// shape (the same forward-compatibility `tau-alpha`'s own decoder gives).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TaudUnknownEntry {
    pub tag: u8,
    pub hex: String,
}

/// A fully decoded `TAUD1` Check report.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TaudReport {
    pub format: u8,
    pub profile: String,
    pub tests: Vec<TaudTest>,
    pub entries: TaudEntries,
    pub unknown: Vec<TaudUnknownEntry>,
    pub verdict: String,
}

/// Strips the `TAUD1:` prefix and decodes the base64url payload (no padding,
/// matching how the firmware encoder emits it). Whitespace anywhere in
/// `text` is removed first, the same defensive normalisation the Python
/// decoder applies before decoding a value that came from a QR scanner or a
/// pasted short report.
pub fn from_text(text: &str) -> Result<Vec<u8>, TauError> {
    let joined: String = text.split_whitespace().collect();
    let payload = joined
        .strip_prefix("TAUD1:")
        .ok_or_else(|| bad_record("not a TAUD1 report"))?;
    URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|error| bad_record(format!("invalid TAUD1 base64: {error}")))
}

fn le_to_u64(chunk: &[u8]) -> u64 {
    let mut buffer = [0u8; 8];
    buffer[..chunk.len()].copy_from_slice(chunk);
    u64::from_le_bytes(buffer)
}

fn tag_width(tag: u8) -> usize {
    match tag {
        2 | 4 | 5 | 6 | 8 => 2,
        7 | 9 => 4,
        _ => 1,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Parses a raw `TD`-magic record (after base64 decoding): validates the
/// magic, the trailing CRC32, and the format byte, then walks the `{tag u8,
/// length u8, value}` entries. Byte-exact port of
/// `tau-alpha/tools/decode_tau_suite.py`'s `parse_record`.
pub fn parse_record(record: &[u8]) -> Result<TaudReport, TauError> {
    if record.len() < 8 || &record[0..2] != b"TD" {
        return Err(bad_record("bad magic"));
    }
    let body = &record[..record.len() - 4];
    let expected_crc =
        u32::from_le_bytes(record[record.len() - 4..].try_into().expect("4 bytes"));
    if crc32fast::hash(body) != expected_crc {
        return Err(bad_record(
            "CRC mismatch (record damaged or incomplete)",
        ));
    }
    if record[2] != FORMAT {
        return Err(bad_record(format!("unsupported format {}", record[2])));
    }
    let profile_id = record[3];
    let end = record.len() - 4;
    let mut i = 4usize;
    let mut tests = Vec::new();
    let mut entries = TaudEntries::default();
    let mut unknown = Vec::new();

    while i < end {
        if i + 2 > end {
            return Err(bad_record("truncated entry"));
        }
        let tag = record[i];
        let n = record[i + 1] as usize;
        if i + 2 + n > end {
            return Err(bad_record("truncated entry"));
        }
        let value = &record[i + 2..i + 2 + n];
        i += 2 + n;

        match tag {
            3 if n == 6 => {
                let id = value[0];
                let result = value[1];
                let raw = u32::from_le_bytes(value[2..6].try_into().expect("4 bytes"));
                let mut test = TaudTest {
                    id,
                    name: test_name(id),
                    result: result_name(result),
                    value: raw,
                    busy_permille: None,
                    audio_full: None,
                };
                if id == 13 {
                    let busy = raw & 0xFFFF;
                    test.busy_permille = if busy == 0xFFFF { None } else { Some(busy) };
                    test.audio_full = Some(raw & 0x1_0000 != 0);
                }
                tests.push(test);
            }
            14 if n >= 10 => {
                entries.decode_sweep.push(TaudDecodeSweepEntry {
                    track: value[0],
                    speed_pct: value[1],
                    h_pct: u16::from_le_bytes([value[2], value[3]]),
                    i_pct: u16::from_le_bytes([value[4], value[5]]),
                    s_pct: u16::from_le_bytes([value[6], value[7]]),
                    r_pct: u16::from_le_bytes([value[8], value[9]]),
                    title: String::from_utf8_lossy(&value[10..n]).into_owned(),
                });
            }
            1 => {
                if n < 16 {
                    return Err(bad_record("truncated build entry"));
                }
                let firmware = u32::from_le_bytes(value[0..4].try_into().expect("4 bytes"));
                let bitstream = u32::from_le_bytes(value[4..8].try_into().expect("4 bytes"));
                let flags = u32::from_le_bytes(value[8..12].try_into().expect("4 bytes"));
                let heap_gap = u32::from_le_bytes(value[12..16].try_into().expect("4 bytes"));
                entries.build = Some(TaudBuild {
                    firmware: format!(
                        "{}.{}.{}",
                        (firmware >> 16) & 255,
                        (firmware >> 8) & 255,
                        firmware & 255
                    ),
                    bitstream: format!("{bitstream:08X}"),
                    flags,
                    heap_gap,
                });
            }
            2 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 => {
                let width = tag_width(tag);
                let values: Vec<u64> = value.chunks_exact(width).map(le_to_u64).collect();
                match tag {
                    4 | 5 => {
                        let cycles = match values.as_slice() {
                            [read_min, read_avg, read_max, write_min, write_avg, write_max] => {
                                Some(TaudCycles {
                                    read_min: Some(*read_min),
                                    read_avg: *read_avg,
                                    read_max: *read_max,
                                    write_min: Some(*write_min),
                                    write_avg: *write_avg,
                                    write_max: *write_max,
                                })
                            }
                            [read_avg, read_max, write_avg, write_max] => Some(TaudCycles {
                                read_min: None,
                                read_avg: *read_avg,
                                read_max: *read_max,
                                write_min: None,
                                write_avg: *write_avg,
                                write_max: *write_max,
                            }),
                            _ => None,
                        };
                        match (tag, cycles) {
                            (4, Some(c)) => entries.sdram = Some(c),
                            (4, None) => entries.sdram_raw = values,
                            (5, Some(c)) => entries.psram = Some(c),
                            (5, None) => entries.psram_raw = values,
                            _ => unreachable!(),
                        }
                    }
                    8 => {
                        if let [late_underruns, audio_full, stall_ms, window_s] =
                            values.as_slice()
                        {
                            entries.audio = Some(TaudAudio {
                                late_underruns: *late_underruns,
                                audio_full: *audio_full != 0,
                                stall_ms: *stall_ms,
                                window_s: *window_s,
                            });
                        } else {
                            entries.audio_raw = values;
                        }
                    }
                    13 => {
                        if let [h_pct, i_pct, s_pct, r_pct] = values.as_slice() {
                            entries.decode_profile = Some(TaudDecodeProfile {
                                h_pct: *h_pct,
                                i_pct: *i_pct,
                                s_pct: *s_pct,
                                r_pct: *r_pct,
                            });
                        } else {
                            entries.decode_profile_raw = values;
                        }
                    }
                    2 => entries.memory = values,
                    6 => entries.cold = values,
                    7 => entries.time = values,
                    9 => entries.library = values,
                    10 => entries.settings = values,
                    11 => entries.errors = values,
                    12 => entries.notes = values,
                    _ => unreachable!(),
                }
            }
            _ => unknown.push(TaudUnknownEntry {
                tag,
                hex: hex_encode(value),
            }),
        }
    }

    let verdict = if tests.iter().any(|test| test.result == "FAIL") {
        VERDICTS[2]
    } else if !tests.is_empty() {
        VERDICTS[1]
    } else {
        VERDICTS[0]
    }
    .to_string();

    Ok(TaudReport {
        format: record[2],
        profile: PROFILES
            .get(profile_id as usize)
            .map(|name| name.to_string())
            .unwrap_or_else(|| format!("profile {profile_id}")),
        tests,
        entries,
        unknown,
        verdict,
    })
}

/// Decodes a `TAUD1:...` text payload (as read from a QR code or pasted
/// directly) into a full report.
pub fn parse_text(text: &str) -> Result<TaudReport, TauError> {
    parse_record(&from_text(text)?)
}

/// Locates a QR code in a PNG screenshot and returns its decoded text,
/// without parsing it as a `TAUD1` record (in case the QR carries something
/// else). Tries every grid the detector finds, in case a screenshot happens
/// to contain more than one. Real screenshots only, not a resized or
/// recompressed copy -- the same caveat `decode_tau_suite.py --qr` documents.
pub fn read_qr_text(path: impl AsRef<Path>) -> Result<String, TauError> {
    let (width, height, grey) = decode_png_greyscale(path.as_ref())?;
    let mut prepared =
        rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| grey[y * width + x]);
    for grid in prepared.detect_grids() {
        if let Ok((_meta, text)) = grid.decode() {
            return Ok(text);
        }
    }
    Err(TauError::e(
        ErrorCode::NoQrCodeFound,
        "no decodable QR code found in this image",
    ))
}

/// Reads a PNG screenshot of a Check QR page and returns the fully decoded
/// report: [`read_qr_text`] then [`parse_text`].
pub fn read_qr_report(path: impl AsRef<Path>) -> Result<TaudReport, TauError> {
    parse_text(&read_qr_text(path)?)
}

fn decode_png_greyscale(path: &Path) -> Result<(usize, usize, Vec<u8>), TauError> {
    let file = File::open(path)?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| TauError::e(ErrorCode::Io, format!("invalid PNG: {error}")))?;
    let buffer_size = reader
        .output_buffer_size()
        .ok_or_else(|| TauError::e(ErrorCode::Io, "PNG frame too large to decode"))?;
    let mut buffer = vec![0u8; buffer_size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| TauError::e(ErrorCode::Io, format!("invalid PNG frame: {error}")))?;
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => {
            return Err(TauError::e(
                ErrorCode::Io,
                "indexed PNG was not expanded to a plain colour type",
            ));
        }
    };
    let width = info.width as usize;
    let height = info.height as usize;
    let mut grey = Vec::with_capacity(width * height);
    for pixel in buffer[..info.buffer_size()].chunks_exact(channels) {
        let luma = if channels <= 2 {
            pixel[0]
        } else {
            ((pixel[0] as u16 + pixel[1] as u16 + pixel[2] as u16) / 3) as u8
        };
        grey.push(luma);
    }
    Ok((width, height, grey))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/screenshots")
            .join(name)
    }

    #[test]
    fn decodes_a_real_full_profile_pass_qr() {
        // Ground truth cross-checked against tau-alpha's own
        // tools/decode_tau_suite.py --qr on the same file (B-069's evidence).
        let report = read_qr_report(fixture("20260921_234254.png")).unwrap();
        assert_eq!(report.format, 1);
        assert_eq!(report.profile, "FULL");
        assert_eq!(report.verdict, "all checks passed");
        assert_eq!(report.tests.len(), 13);
        let sdram_speed = report
            .tests
            .iter()
            .find(|test| test.name == "SDRAM read/write cost")
            .unwrap();
        assert_eq!(sdram_speed.result, "PASS");
        assert_eq!(sdram_speed.value, 46);
        let track_changes = report
            .tests
            .iter()
            .find(|test| test.name == "Track changes (10)")
            .unwrap();
        assert_eq!(track_changes.result, "SKIPPED");
        let build = report.entries.build.as_ref().unwrap();
        assert_eq!(build.firmware, "0.3.0");
        assert_eq!(build.bitstream, "4D503317");
        let sdram = report.entries.sdram.as_ref().unwrap();
        assert_eq!(sdram.read_min, Some(46));
        assert_eq!(sdram.read_max, 490);
        let audio = report.entries.audio.as_ref().unwrap();
        assert_eq!(audio.late_underruns, 0);
        assert!(!audio.audio_full);
        assert!(report.unknown.is_empty());
    }

    #[test]
    fn decodes_a_real_sdram_speed_failure() {
        // B-060's evidence: SDRAM read/write cost FAIL, worst 506 cycles.
        let report = read_qr_report(fixture("20260921_220638.png")).unwrap();
        assert_eq!(report.verdict, "some checks failed");
        let sdram_speed = report
            .tests
            .iter()
            .find(|test| test.name == "SDRAM read/write cost")
            .unwrap();
        assert_eq!(sdram_speed.result, "FAIL");
        assert_eq!(sdram_speed.value, 506);
        let window_test = report
            .tests
            .iter()
            .find(|test| test.name == "SDRAM window test")
            .unwrap();
        assert_eq!(window_test.result, "PASS");
    }

    #[test]
    fn decodes_a_real_user_check_pass_with_a_short_run() {
        // B-062's evidence: USER CHECK profile, all seven base tests PASS.
        let report = read_qr_report(fixture("20260921_224317.png")).unwrap();
        assert_eq!(report.profile, "USER CHECK");
        assert_eq!(report.verdict, "all checks passed");
        assert_eq!(report.tests.len(), 7);
    }

    #[test]
    fn decodes_a_real_standard_profile_with_stress_tests() {
        // B-067's evidence: base seven plus Stress R1/R2/R3, all PASS.
        let report = read_qr_report(fixture("20260921_231110.png")).unwrap();
        assert_eq!(report.tests.len(), 10);
        assert!(report
            .tests
            .iter()
            .any(|test| test.name == "Stress R1 (30 s)" && test.result == "PASS"));
    }

    #[test]
    fn refuses_a_non_qr_screenshot() {
        // The now-playing screen captured by the Menu+Start bug (B-067) --
        // a real screenshot with no QR code in it at all.
        let error = read_qr_report(fixture("20260921_231822.png")).unwrap_err();
        assert_eq!(error.code(), ErrorCode::NoQrCodeFound);
    }

    #[test]
    fn from_text_rejects_a_missing_prefix() {
        assert!(from_text("not a report").is_err());
    }

    #[test]
    fn parse_record_rejects_a_corrupted_crc() {
        let mut record = b"TD\x01\x01".to_vec();
        record.extend_from_slice(&[0, 0, 0, 0]);
        assert!(parse_record(&record).is_err());
    }
}
