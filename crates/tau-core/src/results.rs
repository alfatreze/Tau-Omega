//! The Results page's engine (`docs/DEV_LOOP_SPEC.md`, D3): what the Pocket's own screenshots say about a test build. Every screenshot in
//! `Memories/Screenshots` is tried as a Check report (pixel grid, then QR); the ones that are reports are decoded, and two reports of the same profile
//! are compared line by line so "did this build change anything?" has an answer without reading two screens by eye. Reads only.
//!
//! The Pocket stamps a screenshot with its own clock (no time zone), so "since the last install" compares that stamp with a stamp the host builds from
//! its local clock in the same shape (`YYYY-MM-DDTHH:MM:SS`).

use crate::{
    TauError,
    screenshots::{self, ScreenshotEntry},
    taud::{self, TaudCycles, TaudReport},
};
use std::path::Path;

/// Most screenshots decoded in one scan (newest first); older ones are not looked at.
pub const MAX_DECODED: usize = 60;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResultShot {
    pub filename: String,
    pub path: String,
    /// `YYYY-MM-DDTHH:MM:SS` from the file name, when it has the Pocket's own form.
    pub captured_at: Option<String>,
    pub bytes: u64,
    /// The screenshot is at or after `since`.
    pub since_install: bool,
    /// Decoded when the screenshot is a Check report page; `None` for any other screenshot.
    pub report: Option<TaudReport>,
    /// A report page that could not be decoded (damaged grid, cropped QR), with the reason.
    pub problem: Option<String>,
}

fn shot(e: &ScreenshotEntry, since: Option<&str>) -> ResultShot {
    let since_install = match (since, &e.captured_at) {
        (Some(s), Some(c)) => c.as_str() >= s,
        (Some(_), None) => false,
        (None, _) => true,
    };
    let (report, problem) = match taud::read_screenshot_report(&e.path) {
        Ok(r) => (Some(r), None),
        Err(err) if err.code() == crate::ErrorCode::NoQrCodeFound => (None, None),
        Err(err) => (None, Some(err.message)),
    };
    ResultShot {
        filename: e.filename.clone(),
        path: e.path.to_string_lossy().into_owned(),
        captured_at: e.captured_at.clone(),
        bytes: e.bytes,
        since_install,
        report,
        problem,
    }
}

/// The card's screenshots, newest first, the newest [`MAX_DECODED`] tried as reports.
pub fn scan(card: &Path, since: Option<&str>) -> Result<Vec<ResultShot>, TauError> {
    Ok(screenshots::list_screenshots(card)?
        .iter()
        .take(MAX_DECODED)
        .map(|e| shot(e, since))
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Change {
    Better,
    Worse,
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiffLine {
    /// `verdict`, `test`, `build` or `metric`.
    pub kind: String,
    pub label: String,
    pub before: String,
    pub after: String,
    pub change: Change,
}

fn line(
    kind: &str,
    label: impl Into<String>,
    before: impl Into<String>,
    after: impl Into<String>,
    change: Change,
) -> DiffLine {
    DiffLine {
        kind: kind.into(),
        label: label.into(),
        before: before.into(),
        after: after.into(),
        change,
    }
}

/// A number where lower is better, reported only when it moved by at least `pct` percent (and at least `min_abs`).
fn lower_better(
    out: &mut Vec<DiffLine>,
    label: &str,
    before: u64,
    after: u64,
    pct: u64,
    min_abs: u64,
) {
    let d = before.abs_diff(after);
    if d < min_abs.max(1) || d * 100 < before.max(1) * pct {
        return;
    }
    out.push(line(
        "metric",
        label,
        before.to_string(),
        after.to_string(),
        if after < before {
            Change::Better
        } else {
            Change::Worse
        },
    ));
}

fn cycles(out: &mut Vec<DiffLine>, name: &str, a: &Option<TaudCycles>, b: &Option<TaudCycles>) {
    if let (Some(a), Some(b)) = (a, b) {
        lower_better(
            out,
            &format!("{name} read, average cycles"),
            a.read_avg,
            b.read_avg,
            5,
            2,
        );
        lower_better(
            out,
            &format!("{name} read, worst cycles"),
            a.read_max,
            b.read_max,
            5,
            4,
        );
        lower_better(
            out,
            &format!("{name} write, average cycles"),
            a.write_avg,
            b.write_avg,
            5,
            2,
        );
        lower_better(
            out,
            &format!("{name} write, worst cycles"),
            a.write_max,
            b.write_max,
            5,
            4,
        );
    }
}

fn rank(result: &str) -> u8 {
    match result {
        "PASS" => 2,
        "FAIL" => 0,
        _ => 1,
    }
}

/// What changed from `before` to `after` (two reports of the same profile): the verdict, each test's result and (when it moved by a tenth or more) its
/// value, the build identity, and the headline numbers. An empty list means nothing measurable changed.
pub fn diff(before: &TaudReport, after: &TaudReport) -> Vec<DiffLine> {
    let mut out = Vec::new();
    if before.verdict != after.verdict {
        let change = if after.verdict.contains("passed") {
            Change::Better
        } else if after.verdict.contains("failed") {
            Change::Worse
        } else {
            Change::Changed
        };
        out.push(line(
            "verdict",
            "Verdict",
            &before.verdict,
            &after.verdict,
            change,
        ));
    }
    for t in &after.tests {
        let Some(p) = before.tests.iter().find(|x| x.id == t.id) else {
            out.push(line("test", &t.name, "not run", &t.result, Change::Changed));
            continue;
        };
        if p.result != t.result {
            let change = match rank(&t.result).cmp(&rank(&p.result)) {
                std::cmp::Ordering::Greater => Change::Better,
                std::cmp::Ordering::Less => Change::Worse,
                std::cmp::Ordering::Equal => Change::Changed,
            };
            out.push(line("test", &t.name, &p.result, &t.result, change));
        } else if p.value != t.value
            && p.value != 0
            && t.value != 0
            && u64::from(p.value).abs_diff(u64::from(t.value)) * 10 >= u64::from(p.value)
        {
            out.push(line(
                "test",
                format!("{} (value)", t.name),
                p.value.to_string(),
                t.value.to_string(),
                Change::Changed,
            ));
        }
    }
    for t in &before.tests {
        if !after.tests.iter().any(|x| x.id == t.id) {
            out.push(line("test", &t.name, &t.result, "not run", Change::Changed));
        }
    }
    if let (Some(a), Some(b)) = (&before.entries.build, &after.entries.build) {
        if a.firmware != b.firmware {
            out.push(line(
                "build",
                "Firmware",
                &a.firmware,
                &b.firmware,
                Change::Changed,
            ));
        }
        if a.bitstream != b.bitstream {
            out.push(line(
                "build",
                "Bitstream",
                &a.bitstream,
                &b.bitstream,
                Change::Changed,
            ));
        }
        if a.heap_gap != b.heap_gap {
            out.push(line(
                "build",
                "Free heap (bytes)",
                a.heap_gap.to_string(),
                b.heap_gap.to_string(),
                if b.heap_gap > a.heap_gap {
                    Change::Better
                } else {
                    Change::Worse
                },
            ));
        }
    }
    cycles(
        &mut out,
        "SDRAM",
        &before.entries.sdram,
        &after.entries.sdram,
    );
    cycles(
        &mut out,
        "PSRAM",
        &before.entries.psram,
        &after.entries.psram,
    );
    if let (Some(a), Some(b)) = (&before.entries.audio, &after.entries.audio) {
        lower_better(
            &mut out,
            "Late underruns",
            a.late_underruns,
            b.late_underruns,
            0,
            1,
        );
        lower_better(&mut out, "Draw stall (ms)", a.stall_ms, b.stall_ms, 10, 5);
    }
    if let (Some(a), Some(b)) = (&before.entries.load, &after.entries.load) {
        lower_better(&mut out, "CPU busy (%)", a.busy_pct, b.busy_pct, 0, 3);
    }
    out
}

/// One profile's newest report, and what changed from the report before it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Run {
    pub profile: String,
    /// File name of the newest report of this profile.
    pub latest: String,
    /// File name of the report before it (the same profile), when there is one.
    pub previous: Option<String>,
    pub diff: Vec<DiffLine>,
    pub since_install: bool,
}

/// Groups the decoded reports (`shots` newest first) by profile: the newest of each, compared with the one before it. Newest run first.
pub fn runs(shots: &[ResultShot]) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for (i, s) in shots.iter().enumerate() {
        let Some(r) = &s.report else { continue };
        if out.iter().any(|x| x.profile == r.profile) {
            continue;
        }
        let prev = shots[i + 1..]
            .iter()
            .find(|p| p.report.as_ref().is_some_and(|x| x.profile == r.profile));
        out.push(Run {
            profile: r.profile.clone(),
            latest: s.filename.clone(),
            previous: prev.map(|p| p.filename.clone()),
            diff: prev
                .and_then(|p| p.report.as_ref())
                .map_or_else(Vec::new, |p| diff(p, r)),
            since_install: s.since_install,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taud::{TaudAudio, TaudBuild, TaudEntries, TaudTest};

    fn test(id: u8, name: &str, result: &str, value: u32) -> TaudTest {
        TaudTest {
            id,
            name: name.into(),
            result: result.into(),
            value,
            busy_permille: None,
            audio_full: None,
        }
    }

    fn report(tests: Vec<TaudTest>, entries: TaudEntries) -> TaudReport {
        let verdict = if tests.iter().any(|t| t.result == "FAIL") {
            "some checks failed"
        } else {
            "all checks passed"
        };
        TaudReport {
            format: 1,
            profile: "STANDARD".into(),
            tests,
            entries,
            unknown: Vec::new(),
            verdict: verdict.into(),
        }
    }

    #[test]
    fn identical_reports_have_no_differences() {
        let a = report(
            vec![test(1, "SDRAM window", "PASS", 89)],
            TaudEntries::default(),
        );
        assert!(diff(&a, &a.clone()).is_empty());
    }

    #[test]
    fn a_test_that_starts_failing_is_worse_and_one_that_recovers_is_better() {
        let a = report(
            vec![
                test(1, "SDRAM window", "PASS", 89),
                test(2, "Track changes", "FAIL", 0),
            ],
            TaudEntries::default(),
        );
        let b = report(
            vec![
                test(1, "SDRAM window", "FAIL", 89),
                test(2, "Track changes", "PASS", 0),
            ],
            TaudEntries::default(),
        );
        let d = diff(&a, &b);
        let get = |l: &str| d.iter().find(|x| x.label == l).unwrap();
        assert_eq!(get("SDRAM window").change, Change::Worse);
        assert_eq!(get("Track changes").change, Change::Better);
        assert!(
            !d.iter().any(|x| x.kind == "verdict"),
            "both runs fail something, so the verdict is the same"
        );
        let clean = report(
            vec![test(1, "SDRAM window", "PASS", 89)],
            TaudEntries::default(),
        );
        assert_eq!(
            diff(&a, &clean)
                .iter()
                .find(|x| x.kind == "verdict")
                .unwrap()
                .change,
            Change::Better
        );
        // PASS to SKIPPED is neither a recovery nor a failure: it is just different, and a test that vanished is reported
        let c = report(
            vec![test(1, "SDRAM window", "SKIPPED", 0)],
            TaudEntries::default(),
        );
        let d2 = diff(&a, &c);
        assert_eq!(
            d2.iter()
                .find(|x| x.label == "SDRAM window")
                .unwrap()
                .change,
            Change::Worse
        );
        assert!(
            d2.iter()
                .any(|x| x.label == "Track changes" && x.after == "not run")
        );
    }

    #[test]
    fn values_only_count_when_they_move_by_a_tenth_and_numbers_say_which_way_is_better() {
        let ea = TaudEntries {
            audio: Some(TaudAudio {
                late_underruns: 0,
                audio_full: true,
                stall_ms: 100,
                window_s: 30,
            }),
            build: Some(TaudBuild {
                firmware: "fw-1".into(),
                bitstream: "bs-1".into(),
                flags: 0,
                heap_gap: 9000,
            }),
            ..TaudEntries::default()
        };
        let mut eb = ea.clone();
        eb.audio = Some(TaudAudio {
            late_underruns: 2,
            audio_full: true,
            stall_ms: 103,
            window_s: 30,
        });
        eb.build = Some(TaudBuild {
            firmware: "fw-2".into(),
            bitstream: "bs-1".into(),
            flags: 0,
            heap_gap: 8000,
        });
        let a = report(vec![test(5, "Cold x20", "PASS", 1000)], ea);
        let b = report(vec![test(5, "Cold x20", "PASS", 1040)], eb);
        let d = diff(&a, &b);
        assert!(!d.iter().any(|x| x.label.contains("Cold")), "4% is noise");
        assert!(
            !d.iter().any(|x| x.label.contains("Draw stall")),
            "3 ms of 100 is noise"
        );
        assert_eq!(
            d.iter()
                .find(|x| x.label == "Late underruns")
                .unwrap()
                .change,
            Change::Worse
        );
        assert_eq!(
            d.iter().find(|x| x.label == "Firmware").unwrap().after,
            "fw-2"
        );
        assert!(!d.iter().any(|x| x.label == "Bitstream"));
        assert_eq!(
            d.iter()
                .find(|x| x.label.starts_with("Free heap"))
                .unwrap()
                .change,
            Change::Worse
        );
        let c = report(
            vec![test(5, "Cold x20", "PASS", 1300)],
            TaudEntries::default(),
        );
        assert!(diff(&a, &c).iter().any(|x| x.label == "Cold x20 (value)"));
    }

    #[test]
    fn runs_pair_each_profile_with_its_own_previous_report() {
        let shot = |name: &str, profile: &str, result: &str| ResultShot {
            filename: name.into(),
            path: name.into(),
            captured_at: None,
            bytes: 1,
            since_install: name.starts_with("new"),
            report: Some(TaudReport {
                profile: profile.into(),
                ..report(
                    vec![test(1, "SDRAM window", result, 1)],
                    TaudEntries::default(),
                )
            }),
            problem: None,
        };
        let plain = ResultShot {
            report: None,
            ..shot("other.png", "x", "PASS")
        };
        let list = vec![
            shot("new3", "FULL", "PASS"),
            plain,
            shot("new2", "STANDARD", "FAIL"),
            shot("old1", "STANDARD", "PASS"),
            shot("old0", "FULL", "FAIL"),
        ];
        let r = runs(&list);
        assert_eq!(
            r.iter().map(|x| x.profile.as_str()).collect::<Vec<_>>(),
            ["FULL", "STANDARD"]
        );
        assert_eq!(r[0].previous.as_deref(), Some("old0"));
        assert_eq!(
            r[0].diff.iter().find(|d| d.kind == "test").unwrap().change,
            Change::Better
        );
        assert_eq!(r[1].previous.as_deref(), Some("old1"));
        assert_eq!(
            r[1].diff.iter().find(|d| d.kind == "test").unwrap().change,
            Change::Worse
        );
        assert!(r[0].since_install && r[1].since_install);
        assert!(
            runs(&list[4..]).iter().all(|x| x.previous.is_none()),
            "a first run has nothing to compare with"
        );
    }

    #[test]
    fn a_folder_without_screenshots_scans_to_nothing_and_a_plain_image_is_not_a_report() {
        let c = std::env::temp_dir().join(format!("omega-results-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&c);
        std::fs::create_dir_all(&c).unwrap();
        assert!(scan(&c, None).unwrap().is_empty());
        let s = c.join("Memories/Screenshots");
        std::fs::create_dir_all(&s).unwrap();
        std::fs::write(s.join("20261014_101500.png"), b"not really a png").unwrap();
        let v = scan(&c, Some("2026-10-14T10:00:00")).unwrap();
        assert_eq!(v.len(), 1);
        assert!(v[0].since_install && v[0].report.is_none());
        let later = scan(&c, Some("2026-10-14T11:00:00")).unwrap();
        assert!(!later[0].since_install, "before the baseline");
    }
}
