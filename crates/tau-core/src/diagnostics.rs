//! "Send diagnostics": reads what a Check run leaves on a card and packs it into one zip a person can attach to a bug
//! report. Nothing is uploaded and nothing is written to the card; the zip goes to a folder outside it. Spec:
//! `DIAGNOSTICS_COLLECTOR.md`. The record formats are decoded by `diag` and `taud`.
//!
//! Privacy: only Tau cores are read, no other core's settings; media is reported as counts and total size, never
//! as file names; screenshots are copied unchanged and only those that hold a Check QR code.

use crate::diag::CheckSummary;
use crate::{ErrorCode, TauError, diag, inspect_card, parse, screenshots, storage, sync, taud};
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// How many of the newest screenshots to look at for Check QR codes.
pub const DEFAULT_SCREENSHOT_LIMIT: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiagFile {
    /// Path on the card, relative to its root.
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiagIndex {
    pub tracks: u16,
    pub albums: u16,
    pub artists: u16,
    pub playlists: u16,
    pub build_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiagCore {
    pub id: String,
    pub version: String,
    pub date_release: String,
    pub platform: String,
    /// The persisted-settings file, relative to the card root, when the core has one.
    pub persist_path: Option<String>,
    /// The Check summary saved in it; `None` with `check_note` saying why.
    pub check: Option<CheckSummary>,
    pub check_note: Option<String>,
    pub files: Vec<DiagFile>,
    pub index: Option<DiagIndex>,
    pub media_files: u64,
    pub media_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiagShot {
    pub filename: String,
    pub captured_at: Option<String>,
    /// The decoded Check report when the screenshot holds a Check QR code.
    pub report: Option<taud::TaudReport>,
    /// Why there is no report (a plain screenshot, a damaged record).
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiagReading {
    pub card: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub cores: Vec<DiagCore>,
    pub shots: Vec<DiagShot>,
    /// How many screenshots were looked at, so "none found" can say how far it looked.
    pub shots_examined: usize,
    /// Plain-language observations: no Check run yet, QR and saved summary disagreeing, and so on.
    pub notes: Vec<String>,
}

fn file_entry(card: &Path, rel: &str) -> Option<DiagFile> {
    let path = card.join(rel);
    let meta = fs::metadata(&path).ok().filter(|m| m.is_file())?;
    Some(DiagFile {
        path: rel.to_string(),
        bytes: meta.len(),
        sha256: sync::sha256_file(&path).ok()?,
    })
}

fn media_totals(dir: &Path) -> (u64, u64) {
    let (mut files, mut bytes) = (0, 0);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
        {
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() {
                stack.push(e.path());
            } else if kind.is_file() && !e.file_name().to_string_lossy().starts_with('.') {
                files += 1;
                bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    (files, bytes)
}

fn modified_ms(path: &Path) -> Option<u128> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis())
}

/// Reads a card's Tau diagnostics. Read-only. `screenshot_limit` bounds how many of the newest screenshots are searched
/// for Check QR codes (0 means [`DEFAULT_SCREENSHOT_LIMIT`]).
pub fn read(card_root: &Path, screenshot_limit: usize) -> Result<DiagReading, TauError> {
    let card = inspect_card(card_root)?;
    let root = card.root.clone();
    let space = storage::volume_space(&root)?;
    let mut cores = Vec::new();
    let mut notes = Vec::new();
    for core in card
        .cores
        .iter()
        .filter(|c| c.library_capable || c.id.to_ascii_lowercase().starts_with("alfatreze.tau"))
    {
        let date = fs::read(root.join("Cores").join(&core.id).join("core.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|j| {
                j.pointer("/core/metadata/date_release")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_default();
        let persist_rel = format!("Settings/{}/Interact/_core/interact_persist.json", core.id);
        let persist_abs = root.join(&persist_rel);
        let (persist_path, check, check_note) = if persist_abs.is_file() {
            match diag::read_check_summary(&persist_abs) {
                Ok(s) => (Some(persist_rel), Some(s), None),
                Err(e) if e.code() == ErrorCode::NotACheckSummary => (
                    Some(persist_rel),
                    None,
                    Some("No Check has been run on this core yet. Run Settings > Check on the Pocket (Diagnostic Build), then quit the core so it saves.".to_string()),
                ),
                Err(e) => (Some(persist_rel), None, Some(format!("The saved settings could not be read: {}", e.message))),
            }
        } else {
            (None, None, Some("This core has not saved any settings yet. Quit it once after a Check so it saves.".to_string()))
        };
        let common = format!("Assets/{}/common", core.platform);
        let mut files: Vec<DiagFile> = Vec::new();
        for rel in [
            format!("Cores/{}/bitstream.rbf_r", core.id),
            format!("{common}/tau.rom"),
            format!("{common}/tau-cold.bin"),
            format!("{common}/tau-assets.bin"),
            format!("{common}/tau-library.tdb"),
        ] {
            files.extend(file_entry(&root, &rel));
        }
        let index = fs::read(root.join(&common).join("tau-library.tdb"))
            .ok()
            .and_then(|b| parse(&b).ok())
            .map(|i| DiagIndex {
                tracks: i.counts.tracks,
                albums: i.counts.albums,
                artists: i.counts.artists,
                playlists: i.counts.playlists,
                build_id: i.build_id,
            });
        let (media_files, media_bytes) = media_totals(&root.join(&common));
        cores.push(DiagCore {
            id: core.id.clone(),
            version: core.version.clone(),
            date_release: date,
            platform: core.platform.clone(),
            persist_path,
            check,
            check_note,
            files,
            index,
            media_files,
            media_bytes,
        });
    }
    if cores.is_empty() {
        notes.push("No Tau core was found on this card.".to_string());
    }

    let limit = if screenshot_limit == 0 {
        DEFAULT_SCREENSHOT_LIMIT
    } else {
        screenshot_limit
    };
    let listed = screenshots::list_screenshots(&root)?;
    let examined = listed.len().min(limit);
    let mut shots = Vec::new();
    for entry in listed.into_iter().take(limit) {
        match taud::read_qr_report(&entry.path) {
            Ok(report) => shots.push(DiagShot {
                filename: entry.filename,
                captured_at: entry.captured_at,
                report: Some(report),
                note: None,
            }),
            Err(e) if e.code() == ErrorCode::NoQrCodeFound => {}
            Err(e) => shots.push(DiagShot {
                filename: entry.filename,
                captured_at: entry.captured_at,
                report: None,
                note: Some(format!(
                    "A QR code was found but its report is damaged: {}",
                    e.message
                )),
            }),
        }
    }
    if shots.is_empty() {
        notes.push(format!(
            "No Check QR code was found in the {examined} newest screenshot{}. On the Pocket, open the Check's QR page and press Menu+Start to take a screenshot.",
            if examined == 1 { "" } else { "s" }
        ));
    }
    // Do the newest QR report and the newest saved summary tell the same story? (They can be different runs.)
    let newest_persist = cores.iter().filter(|c| c.check.is_some()).max_by_key(|c| {
        c.persist_path
            .as_ref()
            .and_then(|p| modified_ms(&root.join(p)))
            .unwrap_or(0)
    });
    if let (Some(core), Some(shot)) = (newest_persist, shots.iter().find(|s| s.report.is_some())) {
        let (summary, report) = (
            core.check.as_ref().expect("filtered"),
            shot.report.as_ref().expect("filtered"),
        );
        if summary.verdict != report.verdict {
            notes.push(format!(
                "The newest screenshot says “{}” but {} saved “{}”. They are probably from different runs; the saved run counter is {}.",
                report.verdict, core.id, summary.verdict, summary.run
            ));
        }
    }
    Ok(DiagReading {
        card: root.to_string_lossy().into_owned(),
        total_bytes: space.total_bytes,
        free_bytes: space.available_bytes,
        cores,
        shots,
        shots_examined: examined,
        notes,
    })
}

fn size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    if bytes as f64 >= 1024.0 * MB {
        format!("{:.1} GB", bytes as f64 / (1024.0 * MB))
    } else {
        format!("{:.1} MB", bytes as f64 / MB)
    }
}

/// A readable summary of a reading, as Markdown, for the zip and for "Copy summary".
pub fn summary_markdown(r: &DiagReading) -> String {
    let mut s = String::from("# Tau diagnostics\n\n");
    s.push_str(&format!(
        "Card space: {} free of {}.\n\n",
        size(r.free_bytes),
        size(r.total_bytes)
    ));
    for n in &r.notes {
        s.push_str(&format!("> {n}\n"));
    }
    if !r.notes.is_empty() {
        s.push('\n');
    }
    for c in &r.cores {
        s.push_str(&format!(
            "## {} {} ({})\n\n",
            c.id, c.version, c.date_release
        ));
        match (&c.check, &c.check_note) {
            (Some(k), _) => {
                s.push_str(&format!(
                    "Saved Check summary: **{}**, profile {}, run {}. Passed: {}. Failed: {}. Worst memory access {} cycles, {} late underruns, draw stall {} ms.\n\n",
                    k.verdict,
                    k.profile,
                    k.run,
                    if k.passed.is_empty() { "none".into() } else { k.passed.join(", ") },
                    if k.failed.is_empty() { "none".into() } else { k.failed.join(", ") },
                    k.worst_access_cycles,
                    k.late_underruns,
                    k.draw_stall_ms
                ));
            }
            (None, Some(note)) => s.push_str(&format!("{note}\n\n")),
            (None, None) => {}
        }
        if let Some(i) = &c.index {
            s.push_str(&format!(
                "Library index: {} tracks, {} albums, {} artists, {} playlists (build {:08X}).\n\n",
                i.tracks, i.albums, i.artists, i.playlists, i.build_id
            ));
        }
        s.push_str(&format!(
            "Media folder: {} files, {}.\n\n",
            c.media_files,
            size(c.media_bytes)
        ));
        for f in &c.files {
            s.push_str(&format!(
                "- `{}`: {} bytes, sha256 {}\n",
                f.path,
                f.bytes,
                &f.sha256[..16.min(f.sha256.len())]
            ));
        }
        s.push('\n');
    }
    for sh in &r.shots {
        if let Some(rep) = &sh.report {
            s.push_str(&format!(
                "## Screenshot {}\n\nProfile {}, **{}**.\n\n",
                sh.filename, rep.profile, rep.verdict
            ));
            for t in &rep.tests {
                s.push_str(&format!("- {}: {} ({})\n", t.name, t.result, t.value));
            }
            if let Some(b) = &rep.entries.build {
                s.push_str(&format!(
                    "\nBuild: firmware {}, bitstream {}, heap gap {} bytes.\n",
                    b.firmware, b.bitstream, b.heap_gap
                ));
            }
            s.push('\n');
        } else if let Some(n) = &sh.note {
            s.push_str(&format!("## Screenshot {}\n\n{n}\n\n", sh.filename));
        }
    }
    s
}

/// Days since 1970-01-01 to a civil date (proleptic Gregorian), so the zip name needs no date library.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// `YYYY-MM-DD` in UTC for a unix time in seconds.
pub fn utc_date(unix_seconds: u64) -> String {
    let (y, m, d) = civil_from_days((unix_seconds / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(feature = "serde")]
mod pack {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    /// The zip's file name: `tau-diagnostics-<core>-<version>-<UTC date>.zip`, from the core with the newest Check.
    pub fn zip_name(r: &DiagReading, unix_seconds: u64) -> String {
        let clean = |s: &str| {
            s.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        };
        let core = r
            .cores
            .iter()
            .find(|c| c.check.is_some())
            .or_else(|| r.cores.first());
        let (id, version) = core
            .map(|c| (clean(&c.id), clean(&c.version)))
            .unwrap_or(("card".into(), "unknown".into()));
        format!(
            "tau-diagnostics-{id}-{version}-{}.zip",
            utc_date(unix_seconds)
        )
    }

    /// Writes the diagnostics zip into `dest_dir` (outside the card) and returns its path. Contents: `report.json`,
    /// `report.md`, `card.json`, `README.txt`, the saved settings of each Tau core and the screenshots that hold a
    /// Check QR code, all copied unchanged. The zip is built under a temporary name and renamed when complete.
    pub fn create_zip(
        r: &DiagReading,
        card_root: &Path,
        dest_dir: &Path,
        unix_seconds: u64,
    ) -> Result<PathBuf, TauError> {
        let card = card_root.canonicalize()?;
        if sync::backup_is_inside(dest_dir, &card) {
            return Err(TauError::e(
                ErrorCode::UnsafeBackupLocation,
                "the diagnostics zip must be saved outside the card",
            ));
        }
        fs::create_dir_all(dest_dir)?;
        let name = zip_name(r, unix_seconds);
        let final_path = dest_dir.join(&name);
        let temp = dest_dir.join(format!(".{name}.tmp"));
        let result = (|| -> Result<(), TauError> {
            let file = fs::File::create(&temp)?;
            let mut zip = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            let add = |zip: &mut zip::ZipWriter<fs::File>,
                       path: &str,
                       data: &[u8]|
             -> Result<(), TauError> {
                zip.start_file(path, opts)
                    .map_err(|e| TauError::e(ErrorCode::Io, format!("zip: {e}")))?;
                zip.write_all(data)?;
                Ok(())
            };
            add(&mut zip, "README.txt", README.as_bytes())?;
            add(&mut zip, "report.md", summary_markdown(r).as_bytes())?;
            add(
                &mut zip,
                "report.json",
                &serde_json::to_vec_pretty(r)
                    .map_err(|e| TauError::e(ErrorCode::Json, e.to_string()))?,
            )?;
            let card_json = serde_json::json!({
                "total_bytes": r.total_bytes, "free_bytes": r.free_bytes,
                "cores": r.cores.iter().map(|c| serde_json::json!({
                    "id": c.id, "version": c.version, "date_release": c.date_release, "platform": c.platform,
                    "files": c.files, "index": c.index, "media_files": c.media_files, "media_bytes": c.media_bytes,
                })).collect::<Vec<_>>(),
            });
            add(
                &mut zip,
                "card.json",
                &serde_json::to_vec_pretty(&card_json)
                    .map_err(|e| TauError::e(ErrorCode::Json, e.to_string()))?,
            )?;
            for c in &r.cores {
                if let Some(rel) = &c.persist_path {
                    add(
                        &mut zip,
                        &format!("settings/{}/interact_persist.json", c.id),
                        &fs::read(card.join(rel))?,
                    )?;
                }
            }
            for s in r.shots.iter().filter(|s| s.report.is_some()) {
                add(
                    &mut zip,
                    &format!("screenshots/{}", s.filename),
                    &fs::read(card.join("Memories/Screenshots").join(&s.filename))?,
                )?;
            }
            zip.finish()
                .map_err(|e| TauError::e(ErrorCode::Io, format!("zip: {e}")))?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = fs::remove_file(&temp);
            return Err(e);
        }
        if final_path.exists() {
            fs::remove_file(&final_path)?;
        }
        fs::rename(&temp, &final_path)?;
        Ok(final_path)
    }

    const README: &str = "This zip was made by Tau Omega from a Pocket card.\n\nIt holds the Tau Check results saved on the card (the saved settings of each Tau core and any screenshots of a Check QR code, unchanged), a summary, and the sizes and checksums of the Tau core files. It lists the media folder as counts and a total size only, with no file or track names, and it contains nothing from other cores.\n\nNothing was uploaded: this file only exists on your computer until you choose to send it.\n";
}
#[cfg(feature = "serde")]
pub use pack::{create_zip, zip_name};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_right_across_awkward_days() {
        assert_eq!(utc_date(0), "1970-01-01");
        assert_eq!(utc_date(951_782_400), "2000-02-29"); // a leap day in a century leap year
        assert_eq!(utc_date(1_791_000_000), "2026-10-03");
        assert_eq!(utc_date(4_107_542_400), "2100-03-01"); // 2100 is not a leap year
    }

    use std::path::PathBuf;

    fn data(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(rel)
    }

    /// A fake card: one Tau core with a saved Check summary (a real captured file), a media folder with track names
    /// that must never appear in the output, and screenshots (real QR pages plus one plain screenshot).
    fn card(name: &str, persist: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tau-diag-{name}-{}-{}",
            std::process::id(),
            crate::test_uniq()
        ));
        let _ = fs::remove_dir_all(&root);
        let core = root.join("Cores/alfatreze.TAU_TEST");
        fs::create_dir_all(&core).unwrap();
        fs::write(core.join("core.json"), r#"{"core":{"metadata":{"platform_ids":["tau_test"],"shortname":"TAU_TEST","author":"alfatreze","version":"0.6.0","date_release":"2026-09-30"}}}"#).unwrap();
        fs::write(
            core.join("data.json"),
            r#"{"data":{"data_slots":[{"id":5,"filename":"tau-library.tdb"}]}}"#,
        )
        .unwrap();
        fs::write(core.join("bitstream.rbf_r"), b"bitstream bytes").unwrap();
        let media = root.join("Assets/tau_test/common");
        fs::create_dir_all(media.join("Secret Artist/Private Album")).unwrap();
        fs::write(
            media.join("Secret Artist/Private Album/My Private Song.mp3"),
            vec![1u8; 2048],
        )
        .unwrap();
        fs::write(media.join("tau.rom"), b"rom").unwrap();
        fs::create_dir_all(root.join("Settings/alfatreze.TAU_TEST/Interact/_core")).unwrap();
        fs::copy(
            data(persist),
            root.join("Settings/alfatreze.TAU_TEST/Interact/_core/interact_persist.json"),
        )
        .unwrap();
        // Another core's settings, which must never be read into a report.
        fs::create_dir_all(root.join("Settings/someone.Else/Interact/_core")).unwrap();
        fs::write(
            root.join("Settings/someone.Else/Interact/_core/interact_persist.json"),
            b"{\"private\": true}",
        )
        .unwrap();
        let shots = root.join("Memories/Screenshots");
        fs::create_dir_all(&shots).unwrap();
        for f in [
            "20260928_135722.png",
            "20260927_233104.png",
            "20260921_231822.png",
        ] {
            fs::copy(data(&format!("screenshots/{f}")), shots.join(f)).unwrap();
        }
        root
    }

    #[test]
    fn reads_the_check_the_card_holds_and_never_the_media_names() {
        let root = card("read", "interact_persist/check_all_passed.json");
        let r = read(&root, 0).unwrap();
        assert_eq!(r.cores.len(), 1);
        let c = &r.cores[0];
        assert_eq!(
            (c.id.as_str(), c.version.as_str(), c.date_release.as_str()),
            ("alfatreze.TAU_TEST", "0.6.0", "2026-09-30")
        );
        let k = c.check.as_ref().expect("a real Check summary");
        assert_eq!(
            (k.profile.as_str(), k.run, k.verdict.as_str()),
            ("USER CHECK", 5, "all checks passed")
        );
        assert_eq!(c.media_files, 2, "the media folder is counted, not named");
        assert!(
            c.files.iter().any(|f| f.path.ends_with("bitstream.rbf_r")
                && f.bytes == 15
                && f.sha256.len() == 64)
        );
        assert!(c.files.iter().any(|f| f.path.ends_with("tau.rom")));
        // Two of the three screenshots hold a Check QR; the plain one is skipped.
        let names: Vec<_> = r.shots.iter().map(|s| s.filename.as_str()).collect();
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names.contains(&"20260928_135722.png") && names.contains(&"20260927_233104.png"));
        assert_eq!(r.shots_examined, 3);
        let md = summary_markdown(&r);
        assert!(md.contains("all checks passed") && md.contains("alfatreze.TAU_TEST"));
        assert!(
            !md.contains("Secret Artist") && !md.contains("My Private Song"),
            "a media name leaked into the summary"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn says_so_plainly_when_no_check_has_been_run_or_saved() {
        let root = card("legacy", "interact_persist/legacy_no_check.json");
        let r = read(&root, 0).unwrap();
        assert!(r.cores[0].check.is_none());
        assert!(
            r.cores[0]
                .check_note
                .as_ref()
                .unwrap()
                .contains("No Check has been run")
        );
        fs::remove_file(
            root.join("Settings/alfatreze.TAU_TEST/Interact/_core/interact_persist.json"),
        )
        .unwrap();
        let r = read(&root, 0).unwrap();
        assert!(
            r.cores[0].persist_path.is_none()
                && r.cores[0]
                    .check_note
                    .as_ref()
                    .unwrap()
                    .contains("not saved any settings")
        );
        fs::remove_dir_all(root.join("Memories")).unwrap();
        let r = read(&root, 0).unwrap();
        assert!(r.shots.is_empty() && r.notes.iter().any(|n| n.contains("No Check QR code")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_card_with_no_tau_core_says_so() {
        let root = std::env::temp_dir().join(format!(
            "tau-diag-empty-{}-{}",
            std::process::id(),
            crate::test_uniq()
        ));
        fs::create_dir_all(root.join("Cores")).unwrap();
        fs::create_dir_all(root.join("Assets")).unwrap();
        let r = read(&root, 0).unwrap();
        assert!(r.cores.is_empty() && r.notes.iter().any(|n| n.contains("No Tau core")));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn the_zip_holds_the_evidence_unchanged_and_nothing_private() {
        use std::io::Read;
        let root = card("zip", "interact_persist/check_some_failed.json");
        let r = read(&root, 0).unwrap();
        let out = std::env::temp_dir().join(format!(
            "tau-diag-out-{}-{}",
            std::process::id(),
            crate::test_uniq()
        ));
        assert!(
            create_zip(&r, &root, &root.join("Memories"), 0).is_err(),
            "a zip may not be written onto the card"
        );
        let zip_path = create_zip(&r, &root, &out, 1_791_000_000).unwrap();
        assert_eq!(
            zip_path.file_name().unwrap(),
            "tau-diagnostics-alfatreze.TAU_TEST-0.6.0-2026-10-03.zip"
        );
        let mut z = zip::ZipArchive::new(fs::File::open(&zip_path).unwrap()).unwrap();
        let mut names: Vec<String> = (0..z.len())
            .map(|i| z.by_index(i).unwrap().name().to_string())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "README.txt",
                "card.json",
                "report.json",
                "report.md",
                "screenshots/20260927_233104.png",
                "screenshots/20260928_135722.png",
                "settings/alfatreze.TAU_TEST/interact_persist.json"
            ]
        );
        let mut shot = Vec::new();
        z.by_name("screenshots/20260927_233104.png")
            .unwrap()
            .read_to_end(&mut shot)
            .unwrap();
        assert_eq!(
            shot,
            fs::read(data("screenshots/20260927_233104.png")).unwrap(),
            "a screenshot was altered"
        );
        let mut all_text = String::new();
        for n in ["README.txt", "card.json", "report.json", "report.md"] {
            z.by_name(n).unwrap().read_to_string(&mut all_text).unwrap();
        }
        assert!(
            !all_text.contains("Secret Artist")
                && !all_text.contains("My Private Song")
                && !all_text.contains("private\": true"),
            "something private leaked"
        );
        assert!(all_text.contains("some checks failed"));
        let leftovers: Vec<_> = fs::read_dir(&out)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            leftovers.len(),
            1,
            "a temporary file was left behind: {leftovers:?}"
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(out);
    }

    /// Read-only run on a real card: `TAU_REAL_CARD=/Volumes/Pock TAU_REAL_ZIP_DIR=<folder off the card> cargo test -p tau-core
    /// real_card_diagnostics --features serde -- --ignored --nocapture`. Checks the card is unchanged afterwards.
    #[cfg(feature = "serde")]
    #[test]
    #[ignore]
    fn real_card_diagnostics_read_and_zip_without_touching_the_card() {
        let (Ok(card), Ok(out)) = (
            std::env::var("TAU_REAL_CARD"),
            std::env::var("TAU_REAL_ZIP_DIR"),
        ) else {
            return;
        };
        let card = PathBuf::from(card);
        let listing = |dir: &Path| {
            let mut v = Vec::new();
            let mut stack = vec![dir.to_path_buf()];
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
                        v.push((p, e.metadata().map(|m| m.len()).unwrap_or(0)))
                    }
                }
            }
            v.sort();
            v
        };
        let before = [
            listing(&card.join("Settings")),
            listing(&card.join("Memories")),
            listing(&card.join("Cores")),
        ];
        let t = std::time::Instant::now();
        let r = read(&card, 0).unwrap();
        println!(
            "read in {:?}: {} cores, {} screenshots with a report of {} examined, notes {:?}",
            t.elapsed(),
            r.cores.len(),
            r.shots.len(),
            r.shots_examined,
            r.notes
        );
        for c in &r.cores {
            println!(
                "  {} {} check={:?} note={:?} index={:?} media={} files",
                c.id,
                c.version,
                c.check.as_ref().map(|k| (&k.verdict, k.run)),
                c.check_note,
                c.index,
                c.media_files
            );
        }
        let zip = create_zip(&r, &card, Path::new(&out), 1_791_000_000).unwrap();
        println!(
            "zip: {} ({} bytes)",
            zip.display(),
            fs::metadata(&zip).unwrap().len()
        );
        let after = [
            listing(&card.join("Settings")),
            listing(&card.join("Memories")),
            listing(&card.join("Cores")),
        ];
        assert_eq!(before, after, "reading changed the card");
    }
}
