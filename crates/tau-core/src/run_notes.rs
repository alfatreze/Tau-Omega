//! Run notes for developers (`docs/DEV_LOOP_SPEC.md`, D4): one note per install of a test build, kept by Omega on this computer (never on the card): which
//! package went on (its hashes), when, where the backup is, one free line of the developer's own, and a snapshot of what the Pocket reported afterwards
//! (the Results page's runs). A note exports as a plain Markdown report, the per-iteration record the loop used to keep by hand.

use crate::{
    ErrorCode, TauError,
    results::{DiffLine, Run},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

/// Notes kept; the oldest are dropped past this.
pub const MAX_NOTES: usize = 500;
/// Longest free-text line accepted.
pub const MAX_TEXT: usize = 4000;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RunResult {
    pub profile: String,
    pub captured_at: String,
    pub verdict: String,
    pub failing: Vec<String>,
    pub firmware: String,
    pub bitstream: String,
    pub changes: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RunNote {
    pub id: String,
    /// The card's volume name.
    pub card: String,
    /// When the install finished, in the host's local clock and the Pocket's stamp shape (`YYYY-MM-DDTHH:MM:SS`).
    pub stamp: String,
    pub core_id: String,
    pub shortname: String,
    pub version: String,
    /// The package's file name.
    pub package: String,
    pub package_sha256: String,
    /// `(file name inside the package, SHA-256)` for each bitstream.
    pub bitstreams: Vec<(String, String)>,
    pub backup_dir: String,
    pub files_written: u64,
    /// The developer's own line.
    pub text: String,
    /// Core-check errors the developer chose to install over ("L09: message"); empty when none were overridden.
    #[cfg_attr(feature = "serde", serde(default))]
    pub lint_overrides: Vec<String>,
    pub results: Vec<RunResult>,
    /// When the results were last attached (stamp), if ever.
    pub results_at: Option<String>,
}

fn io(e: impl std::fmt::Display) -> TauError {
    TauError::e(ErrorCode::Io, e.to_string())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// The SHA-256 of a package zip, and of each bitstream (`*.rbf_r`, `*.rbf`) inside it.
pub fn package_facts(zip_path: &Path) -> Result<(String, Vec<(String, String)>), TauError> {
    let mut f = fs::File::open(zip_path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    let zip_hash = hex(&h.finalize());
    let mut bits = Vec::new();
    let mut archive = zip::ZipArchive::new(fs::File::open(zip_path)?).map_err(io)?;
    for i in 0..archive.len() {
        let mut e = archive.by_index(i).map_err(io)?;
        let name = e.name().to_string();
        let lower = name.to_ascii_lowercase();
        if e.is_file()
            && (lower.ends_with(".rbf_r") || lower.ends_with(".rbf"))
            && e.size() <= 64 << 20
        {
            let mut h = Sha256::new();
            loop {
                let n = e.read(&mut buf).map_err(io)?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
            bits.push((name, hex(&h.finalize())));
        }
    }
    Ok((zip_hash, bits))
}

fn load(path: &Path) -> Vec<RunNote> {
    #[cfg(feature = "serde")]
    {
        fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    #[cfg(not(feature = "serde"))]
    {
        let _ = path;
        Vec::new()
    }
}

#[cfg(feature = "serde")]
fn save(path: &Path, notes: &[RunNote]) -> Result<(), TauError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.part");
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(notes)
            .map_err(|e| TauError::e(ErrorCode::Json, e.to_string()))?,
    )?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// The notes of one card, newest first.
#[cfg(feature = "serde")]
pub fn list(store: &Path, card: &str) -> Vec<RunNote> {
    let mut v: Vec<RunNote> = load(store).into_iter().filter(|n| n.card == card).collect();
    v.sort_by(|a, b| b.stamp.cmp(&a.stamp).then_with(|| b.id.cmp(&a.id)));
    v
}

/// Every note on this computer (any card), in storage order.
#[cfg(feature = "serde")]
pub fn list_all(store: &Path) -> Vec<RunNote> {
    load(store)
}

/// Adds a note; the id is `<stamp digits>-<n>` so two installs in one second stay distinct.
#[cfg(feature = "serde")]
pub fn add(store: &Path, mut note: RunNote) -> Result<RunNote, TauError> {
    let mut all = load(store);
    let base: String = note.stamp.chars().filter(char::is_ascii_digit).collect();
    let n = all.iter().filter(|x| x.id.starts_with(&base)).count();
    note.id = format!("{base}-{n}");
    note.text = note.text.chars().take(MAX_TEXT).collect();
    all.push(note.clone());
    if all.len() > MAX_NOTES {
        all.sort_by(|a, b| a.stamp.cmp(&b.stamp));
        let cut = all.len() - MAX_NOTES;
        all.drain(..cut);
    }
    save(store, &all)?;
    Ok(note)
}

#[cfg(feature = "serde")]
fn edit(store: &Path, id: &str, f: impl FnOnce(&mut RunNote)) -> Result<RunNote, TauError> {
    let mut all = load(store);
    let Some(n) = all.iter_mut().find(|n| n.id == id) else {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            "That run note no longer exists.",
        ));
    };
    f(n);
    let out = n.clone();
    save(store, &all)?;
    Ok(out)
}

#[cfg(feature = "serde")]
pub fn set_text(store: &Path, id: &str, text: &str) -> Result<RunNote, TauError> {
    let text: String = text.chars().take(MAX_TEXT).collect();
    edit(store, id, |n| n.text = text)
}

/// Stores a snapshot of what the Pocket reported (the Results page's runs) so the note keeps it after the card is used for something else.
#[cfg(feature = "serde")]
pub fn attach_results(
    store: &Path,
    id: &str,
    results: Vec<RunResult>,
    at: &str,
) -> Result<RunNote, TauError> {
    edit(store, id, |n| {
        n.results = results;
        n.results_at = Some(at.to_string());
    })
}

#[cfg(feature = "serde")]
pub fn delete(store: &Path, id: &str) -> Result<(), TauError> {
    let mut all = load(store);
    let before = all.len();
    all.retain(|n| n.id != id);
    if all.len() == before {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            "That run note no longer exists.",
        ));
    }
    save(store, &all)
}

/// Turns the Results page's runs into the snapshot a note keeps. `describe` gives a run's report details (time, verdict, failing tests, build).
pub fn snapshot(
    runs: &[Run],
    describe: impl Fn(&Run) -> Option<(String, String, Vec<String>, String, String)>,
) -> Vec<RunResult> {
    runs.iter()
        .filter_map(|r| {
            let (captured_at, verdict, failing, firmware, bitstream) = describe(r)?;
            Some(RunResult {
                profile: r.profile.clone(),
                captured_at,
                verdict,
                failing,
                firmware,
                bitstream,
                changes: r.diff.clone(),
            })
        })
        .collect()
}

/// The note as a plain Markdown report.
pub fn markdown(n: &RunNote) -> String {
    let mut s = format!("# {} {} on {}\n\n", n.shortname, n.version, n.card);
    s.push_str(&format!(
        "- Installed: {}\n- Core: `{}`\n- Package: `{}`\n- Package SHA-256: `{}`\n",
        n.stamp.replace('T', " "),
        n.core_id,
        n.package,
        n.package_sha256
    ));
    for (name, h) in &n.bitstreams {
        s.push_str(&format!("- Bitstream `{name}` SHA-256: `{h}`\n"));
    }
    s.push_str(&format!(
        "- Files written: {}\n- Backup: `{}`\n\n",
        n.files_written, n.backup_dir
    ));
    if !n.lint_overrides.is_empty() {
        s.push_str("- **Core check errors overridden for this install:**\n");
        for o in &n.lint_overrides {
            s.push_str(&format!("  - {o}\n"));
        }
        s.push('\n');
    }
    s.push_str("## Note\n\n");
    s.push_str(if n.text.trim().is_empty() {
        "(none)"
    } else {
        n.text.trim()
    });
    s.push_str("\n\n## What the Pocket reported\n\n");
    if n.results.is_empty() {
        s.push_str("No results were attached to this run.\n");
    } else {
        if let Some(at) = &n.results_at {
            s.push_str(&format!("Attached {}.\n\n", at.replace('T', " ")));
        }
        for r in &n.results {
            s.push_str(&format!(
                "### {} ({})\n\n- Verdict: {}\n",
                r.profile,
                r.captured_at.replace('T', " "),
                r.verdict
            ));
            if !r.firmware.is_empty() || !r.bitstream.is_empty() {
                s.push_str(&format!(
                    "- Build: firmware {}, bitstream {}\n",
                    r.firmware, r.bitstream
                ));
            }
            if !r.failing.is_empty() {
                s.push_str(&format!("- Failing: {}\n", r.failing.join(", ")));
            }
            if r.changes.is_empty() {
                s.push_str("- Against the previous run of this profile: no measurable change\n");
            } else {
                s.push_str("- Against the previous run of this profile:\n");
                for c in &r.changes {
                    s.push_str(&format!(
                        "  - {:?}: {} {} → {}\n",
                        c.change, c.label, c.before, c.after
                    ));
                }
            }
            s.push('\n');
        }
    }
    s
}

/// A file name for the exported report: `run-<core>-<stamp digits>.md`, safe on every file system.
pub fn report_file_name(n: &RunNote) -> String {
    let core: String = n
        .shortname
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let stamp: String = n.stamp.chars().filter(char::is_ascii_digit).collect();
    format!("run-{core}-{stamp}.md")
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::results::Change;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("omega-runnotes-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn note(card: &str, stamp: &str, short: &str) -> RunNote {
        RunNote {
            id: String::new(),
            card: card.into(),
            stamp: stamp.into(),
            core_id: format!("alfatreze.{short}"),
            shortname: short.into(),
            version: "0.6.0".into(),
            package: format!("{short}.zip"),
            package_sha256: "ab".repeat(32),
            bitstreams: vec![("Cores/x/bitstream.rbf_r".into(), "cd".repeat(32))],
            backup_dir: "/backups/1".into(),
            files_written: 12,
            text: String::new(),
            lint_overrides: Vec::new(),
            results: Vec::new(),
            results_at: None,
        }
    }

    #[test]
    fn notes_are_kept_per_card_newest_first_with_distinct_ids() {
        let d = tmp("list");
        let store = d.join("run-notes.json");
        let a = add(&store, note("Pocket", "2026-10-14T10:00:00", "TAU DEV 1")).unwrap();
        let b = add(&store, note("Pocket", "2026-10-14T10:00:00", "TAU DEV 2")).unwrap();
        add(
            &store,
            note("CARDWRITE", "2026-10-14T11:00:00", "TAU DEV 3"),
        )
        .unwrap();
        assert_ne!(a.id, b.id, "two installs in one second stay distinct");
        let l = list(&store, "Pocket");
        assert_eq!(l.len(), 2);
        assert_eq!(
            l[0].shortname, "TAU DEV 2",
            "same stamp: the later id first"
        );
        assert!(list(&store, "Nobody").is_empty());
    }

    #[test]
    fn text_is_saved_trimmed_to_a_limit_and_a_missing_note_is_an_error() {
        let d = tmp("text");
        let store = d.join("n.json");
        let n = add(&store, note("Pocket", "2026-10-14T10:00:00", "X")).unwrap();
        let s = set_text(&store, &n.id, "  gain click gone  ").unwrap();
        assert_eq!(s.text, "  gain click gone  ", "stored as typed");
        let long = "x".repeat(MAX_TEXT + 50);
        assert_eq!(set_text(&store, &n.id, &long).unwrap().text.len(), MAX_TEXT);
        assert!(set_text(&store, "nope", "x").is_err());
        delete(&store, &n.id).unwrap();
        assert!(delete(&store, &n.id).is_err());
        assert!(list(&store, "Pocket").is_empty());
    }

    #[test]
    fn the_store_is_capped_dropping_the_oldest() {
        let d = tmp("cap");
        let store = d.join("n.json");
        for i in 0..(MAX_NOTES + 3) {
            add(
                &store,
                note(
                    "Pocket",
                    &format!("2026-10-14T10:{:02}:{:02}", (i / 60) % 60, i % 60),
                    &format!("B{i}"),
                ),
            )
            .unwrap();
        }
        let l = list(&store, "Pocket");
        assert_eq!(l.len(), MAX_NOTES);
        assert!(!l.iter().any(|n| n.shortname == "B0"), "the oldest went");
    }

    #[test]
    fn a_report_names_the_install_the_hashes_the_note_and_each_result() {
        let mut n = note("Pocket", "2026-10-14T10:05:00", "TAU DEV 112");
        n.text = "Clicks at 1.75x are still there.".into();
        let clean = markdown(&n);
        assert!(!clean.contains("overridden"));
        n.lint_overrides = vec!["L09: The bitstream file is missing.".into()];
        assert!(
            markdown(&n).contains(
                "overridden for this install:**\n  - L09: The bitstream file is missing."
            )
        );
        n.lint_overrides.clear();
        let empty = markdown(&n);
        assert!(
            empty.contains("# TAU DEV 112 0.6.0 on Pocket")
                && empty.contains("No results were attached")
        );
        n.results = vec![RunResult {
            profile: "STANDARD".into(),
            captured_at: "2026-10-14T10:20:00".into(),
            verdict: "some checks failed".into(),
            failing: vec!["Track changes".into()],
            firmware: "fw-2".into(),
            bitstream: "bs-1".into(),
            changes: vec![DiffLine {
                kind: "metric".into(),
                label: "Late underruns".into(),
                before: "0".into(),
                after: "2".into(),
                change: Change::Worse,
            }],
        }];
        n.results_at = Some("2026-10-14T10:30:00".into());
        let r = markdown(&n);
        assert!(
            r.contains("Installed: 2026-10-14 10:05:00")
                && r.contains(&"ab".repeat(32))
                && r.contains(&"cd".repeat(32))
        );
        assert!(
            r.contains("Clicks at 1.75x are still there.")
                && r.contains("Failing: Track changes")
                && r.contains("Worse: Late underruns 0 → 2")
        );
        assert_eq!(report_file_name(&n), "run-TAU_DEV_112-20261014100500.md");
    }

    #[test]
    fn package_facts_hash_the_zip_and_every_bitstream_inside() {
        use std::io::Write as _;
        let d = tmp("facts");
        let zip_path = d.join("p.zip");
        let mut w = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        w.start_file("Cores/a.B/bitstream.rbf_r", o).unwrap();
        w.write_all(b"bits").unwrap();
        w.start_file("Cores/a.B/core.json", o).unwrap();
        w.write_all(b"{}").unwrap();
        w.finish().unwrap();
        let (zh, bits) = package_facts(&zip_path).unwrap();
        assert_eq!(zh.len(), 64);
        assert_eq!(bits.len(), 1);
        assert_eq!(bits[0].0, "Cores/a.B/bitstream.rbf_r");
        assert_eq!(bits[0].1, hex(&Sha256::digest(b"bits")));
    }
}
