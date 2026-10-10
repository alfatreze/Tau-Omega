//! Halcyon user EQ presets: the `PRST` section of `tau-assets.bin` and the Equalizer APO / AutoEQ importer.
//!
//! An independent Rust writer/reader for the format Tau Alpha defines in `docs/features/HALCYON_DATA_FORMAT.md`
//! (reference tool `tools/halcyon_assets.py`), checked byte for byte against files that tool produced
//! (`testdata/halcyon/`). The firmware does not clamp, so **this writer owns every safety rule**: names the device font can
//! draw and unique among the presets and not a built-in, control positions in range, a preamp that only attenuates, 24-bit
//! coefficients, and no stage whose poles are on or outside the unit circle once quantised to Q2.22.
//! Coefficients are Q2.22 little endian, order b0 b1 b2 a1 a2, Direct Form I.

use crate::{ErrorCode, TauError};

const MAGIC: &[u8; 4] = b"TPRS";
const VERSION: u16 = 1;
const NAME_LEN: usize = 16;
pub const MAX_PRESETS: usize = 8;
pub const MAX_RAW_STAGES: usize = 10;
const QF: u32 = 22;
const ONE: i64 = 1 << QF;
const FS: f64 = 48000.0;
/// Built-in presets a user preset may not reuse.
pub const RESERVED: [&str; 8] = [
    "FLAT",
    "WARM",
    "CLEAR",
    "BASS",
    "VOCAL",
    "SPEECH",
    "LOW VOLUME",
    "SMOOTH",
];
/// The six Halcyon controls, in the order they are stored.
pub const CONTROLS: [&str; 6] = ["warmth", "bass", "vocal", "punch", "sibilance", "air"];

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", rename_all = "snake_case"))]
pub enum HalcyonPreset {
    /// Six positions: -5..=5, sibilance 0..=5.
    Control { name: String, controls: [i8; 6] },
    /// Raw biquads, Q2.22 integers `[b0, b1, b2, a1, a2]`; `preamp` in Q2.22, above 0 and at most 1.0.
    Raw {
        name: String,
        preamp: i32,
        stages: Vec<[i32; 5]>,
    },
}

impl HalcyonPreset {
    pub fn name(&self) -> &str {
        match self {
            Self::Control { name, .. } | Self::Raw { name, .. } => name,
        }
    }
}

/// An imported profile, with what the user should be told about it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ApoImport {
    pub preset: HalcyonPreset,
    /// The preamp actually used (dB, at most 0): the more negative of the file's and the peak-safe value.
    pub preamp_db: f64,
    /// The largest boost of the quantised cascade (dB), before the preamp.
    pub peak_gain_db: f64,
    pub filters: usize,
}

fn bad(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidAssetsFile, msg)
}

fn i24(v: i32) -> Result<[u8; 3], TauError> {
    if !(-(1 << 23)..(1 << 23)).contains(&v) {
        return Err(bad(format!("{v} does not fit in 24 bits")));
    }
    let b = v.to_le_bytes();
    Ok([b[0], b[1], b[2]])
}

fn r24(b: &[u8]) -> i32 {
    let v = b[0] as i32 | (b[1] as i32) << 8 | (b[2] as i32) << 16;
    if v >= 1 << 23 { v - (1 << 24) } else { v }
}

/// Poles strictly inside the unit circle for integer Q2.22 `a1`, `a2`.
pub fn stable_q(a1: i32, a2: i32) -> bool {
    let (a1f, a2f) = (a1 as f64 / ONE as f64, a2 as f64 / ONE as f64);
    let disc = a1f * a1f - 4.0 * a2f;
    let r = if disc >= 0.0 {
        let s = disc.sqrt();
        ((-a1f + s) / 2.0).abs().max(((-a1f - s) / 2.0).abs())
    } else {
        a2f.max(0.0).sqrt()
    };
    r < 1.0
}

/// 1..=15 characters of `A-Z 0-9 space _ -`, not a built-in name.
pub fn check_name(name: &str) -> Result<(), TauError> {
    let ok = (1..=15).contains(&name.len())
        && name.bytes().all(|b| {
            b.is_ascii_uppercase() || b.is_ascii_digit() || b == b' ' || b == b'_' || b == b'-'
        });
    if !ok {
        return Err(bad(format!(
            "Preset name {name:?}: use 1 to 15 characters of A-Z, 0-9, space, _ and - (the Pocket's font draws nothing else)."
        )));
    }
    if RESERVED.contains(&name) {
        return Err(bad(format!(
            "Preset name {name:?} is one of the built-in presets; pick another."
        )));
    }
    Ok(())
}

/// The `PRST` section bytes for these presets (1..=8). Refuses anything the firmware would not check itself.
pub fn pack_presets(presets: &[HalcyonPreset]) -> Result<Vec<u8>, TauError> {
    if !(1..=MAX_PRESETS).contains(&presets.len()) {
        return Err(bad(format!(
            "A file holds 1 to {MAX_PRESETS} Halcyon presets."
        )));
    }
    let mut body = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for p in presets {
        check_name(p.name())?;
        if seen.contains(&p.name()) {
            return Err(bad(format!("Two presets are named {:?}.", p.name())));
        }
        seen.push(p.name());
        let mut name = [0u8; NAME_LEN];
        name[..p.name().len()].copy_from_slice(p.name().as_bytes());
        match p {
            HalcyonPreset::Control { name: n, controls } => {
                for (i, v) in controls.iter().enumerate() {
                    let (lo, hi) = if CONTROLS[i] == "sibilance" {
                        (0, 5)
                    } else {
                        (-5, 5)
                    };
                    if !(lo..=hi).contains(v) {
                        return Err(bad(format!(
                            "{n}: {} is {v}, allowed {lo} to {hi}.",
                            CONTROLS[i]
                        )));
                    }
                }
                body.push(0);
                body.extend_from_slice(&name);
                body.push(0);
                body.extend(controls.iter().map(|v| *v as u8));
            }
            HalcyonPreset::Raw {
                name: n,
                preamp,
                stages,
            } => {
                if !(1..=MAX_RAW_STAGES).contains(&stages.len()) {
                    return Err(bad(format!("{n}: 1 to {MAX_RAW_STAGES} stages.")));
                }
                if *preamp <= 0 || *preamp as i64 > ONE {
                    return Err(bad(format!(
                        "{n}: the preamp must be above 0 and at most 1.0 (it can only turn the sound down)."
                    )));
                }
                body.push(1);
                body.extend_from_slice(&name);
                body.push(0);
                body.push(stages.len() as u8);
                body.extend_from_slice(&i24(*preamp)?);
                for (i, c) in stages.iter().enumerate() {
                    if !stable_q(c[3], c[4]) {
                        return Err(bad(format!(
                            "{n}: stage {} has poles on or outside the unit circle and would blow up.",
                            i + 1
                        )));
                    }
                    for v in c {
                        body.extend_from_slice(&i24(*v)?);
                    }
                }
            }
        }
    }
    let mut out = Vec::with_capacity(12 + body.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.push(0);
    out.push(presets.len() as u8);
    out.extend_from_slice(&crc32fast::hash(&body).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Reads a `PRST` section, verifying magic, version, CRC, sizes and stability before returning anything.
pub fn parse_presets(d: &[u8]) -> Result<Vec<HalcyonPreset>, TauError> {
    if d.len() < 12 || &d[..4] != MAGIC {
        return Err(bad("This is not a Halcyon preset section."));
    }
    let version = u16::from_le_bytes([d[4], d[5]]);
    let count = d[7] as usize;
    if version != VERSION || !(1..=MAX_PRESETS).contains(&count) {
        return Err(bad(
            "The Halcyon preset header is not one this version understands.",
        ));
    }
    if crc32fast::hash(&d[12..]) != u32::from_le_bytes([d[8], d[9], d[10], d[11]]) {
        return Err(bad("The Halcyon presets failed their check."));
    }
    let mut pos = 12;
    let mut out = Vec::new();
    let short = || bad("A Halcyon preset runs past the end of the section.");
    for _ in 0..count {
        let head = d.get(pos..pos + 18).ok_or_else(short)?;
        let name_bytes = &head[1..17];
        let end = name_bytes.iter().position(|b| *b == 0).unwrap_or(NAME_LEN);
        let name = String::from_utf8_lossy(&name_bytes[..end]).into_owned();
        let typ = head[0];
        pos += 18;
        match typ {
            0 => {
                let v = d.get(pos..pos + 6).ok_or_else(short)?;
                pos += 6;
                out.push(HalcyonPreset::Control {
                    name,
                    controls: [
                        v[0] as i8, v[1] as i8, v[2] as i8, v[3] as i8, v[4] as i8, v[5] as i8,
                    ],
                });
            }
            1 => {
                let n = *d.get(pos).ok_or_else(short)? as usize;
                let preamp = r24(d.get(pos + 1..pos + 4).ok_or_else(short)?);
                pos += 4;
                if !(1..=MAX_RAW_STAGES).contains(&n) {
                    return Err(bad("A Halcyon preset has an impossible stage count."));
                }
                let mut stages = Vec::new();
                for _ in 0..n {
                    let s = d.get(pos..pos + 15).ok_or_else(short)?;
                    pos += 15;
                    let c = [
                        r24(&s[0..3]),
                        r24(&s[3..6]),
                        r24(&s[6..9]),
                        r24(&s[9..12]),
                        r24(&s[12..15]),
                    ];
                    if !stable_q(c[3], c[4]) {
                        return Err(bad("A Halcyon preset holds an unstable stage."));
                    }
                    stages.push(c);
                }
                out.push(HalcyonPreset::Raw {
                    name,
                    preamp,
                    stages,
                });
            }
            t => return Err(bad(format!("Unknown Halcyon preset type {t}."))),
        }
    }
    if pos != d.len() {
        return Err(bad("The Halcyon preset section has bytes left over."));
    }
    Ok(out)
}

// ---- Equalizer APO / AutoEQ text -> raw preset ------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Peak,
    LowShelf,
    HighShelf,
}

struct Cursor<'a> {
    s: &'a str,
}

impl<'a> Cursor<'a> {
    fn ws(&mut self) {
        self.s = self.s.trim_start();
    }
    /// Case-insensitive literal after optional whitespace.
    fn lit(&mut self, word: &str) -> bool {
        self.ws();
        if self.s.len() >= word.len()
            && self.s.is_char_boundary(word.len())
            && self.s[..word.len()].eq_ignore_ascii_case(word)
        {
            self.s = &self.s[word.len()..];
            true
        } else {
            false
        }
    }
    fn digits(&mut self) {
        self.ws();
        let n = self.s.bytes().take_while(u8::is_ascii_digit).count();
        self.s = &self.s[n..];
    }
    fn number(&mut self, signed: bool) -> Option<f64> {
        self.ws();
        let neg = signed && self.s.starts_with('-');
        let t = if neg { &self.s[1..] } else { self.s };
        let n = t
            .bytes()
            .take_while(|b| b.is_ascii_digit() || *b == b'.')
            .count();
        if n == 0 {
            return None;
        }
        let v: f64 = t[..n].parse().ok()?;
        self.s = &t[n..];
        Some(if neg { -v } else { v })
    }
    fn word(&mut self) -> &'a str {
        self.ws();
        let n = self.s.bytes().take_while(u8::is_ascii_alphabetic).count();
        let w = &self.s[..n];
        self.s = &self.s[n..];
        w
    }
}

/// `(preamp_db, [(kind, fc, gain_db, q)])`; disabled filters are dropped and unrecognised lines are ignored.
fn parse_apo(text: &str) -> (f64, Vec<(Kind, f64, f64, f64)>) {
    let mut pre = 0.0;
    let mut filters = Vec::new();
    for line in text.lines() {
        let mut c = Cursor { s: line };
        if c.lit("preamp") {
            if c.lit(":")
                && let Some(v) = c.number(true)
                && c.lit("db")
            {
                pre = v;
            }
            continue;
        }
        let mut c = Cursor { s: line };
        if !c.lit("filter") {
            continue;
        }
        c.digits();
        if !c.lit(":") {
            continue;
        }
        let on = c.word().eq_ignore_ascii_case("on");
        let kind = match c.word().to_ascii_uppercase().as_str() {
            "PK" | "PEQ" => Kind::Peak,
            "LSC" | "LS" => Kind::LowShelf,
            "HSC" | "HS" => Kind::HighShelf,
            _ => continue,
        };
        if !c.lit("fc") {
            continue;
        }
        let Some(fc) = c.number(false) else { continue };
        if !c.lit("hz") || !c.lit("gain") {
            continue;
        }
        let Some(gain) = c.number(true) else { continue };
        if !c.lit("db") || !on {
            continue;
        }
        let q = if c.lit("q") { c.number(false) } else { None };
        let q = q.unwrap_or(if kind == Kind::Peak { 1.0 } else { 0.707 });
        filters.push((kind, fc, gain, q));
    }
    (pre, filters)
}

/// RBJ Audio EQ Cookbook, Q form for peaks and shelves (an APO shelf Q is a Q, not a slope). `a0` normalised to 1.
fn design_q(kind: Kind, f0: f64, gain_db: f64, q: f64) -> [f64; 5] {
    let a = 10f64.powf(gain_db / 40.0);
    let w0 = 2.0 * std::f64::consts::PI * f0 / FS;
    let (cw, sw) = (w0.cos(), w0.sin());
    let al = sw / (2.0 * q);
    let (b0, b1, b2, a0, a1, a2);
    match kind {
        Kind::Peak => {
            (b0, b1, b2) = (1.0 + al * a, -2.0 * cw, 1.0 - al * a);
            (a0, a1, a2) = (1.0 + al / a, -2.0 * cw, 1.0 - al / a);
        }
        Kind::LowShelf => {
            let t = 2.0 * a.sqrt() * al;
            b0 = a * ((a + 1.0) - (a - 1.0) * cw + t);
            b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cw);
            b2 = a * ((a + 1.0) - (a - 1.0) * cw - t);
            a0 = (a + 1.0) + (a - 1.0) * cw + t;
            a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cw);
            a2 = (a + 1.0) + (a - 1.0) * cw - t;
        }
        Kind::HighShelf => {
            let t = 2.0 * a.sqrt() * al;
            b0 = a * ((a + 1.0) + (a - 1.0) * cw + t);
            b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cw);
            b2 = a * ((a + 1.0) + (a - 1.0) * cw - t);
            a0 = (a + 1.0) - (a - 1.0) * cw + t;
            a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cw);
            a2 = (a + 1.0) - (a - 1.0) * cw - t;
        }
    }
    [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0]
}

/// Magnitude (dB) of a cascade of Q2.22 biquads at `f` Hz, 48 kHz sample rate (no preamp).
fn cascade_db(stages: &[[i32; 5]], f: f64) -> f64 {
    let sc = ONE as f64;
    let w = 2.0 * std::f64::consts::PI * f / FS;
    let (zr, zi) = (w.cos(), -w.sin());
    let (z2r, z2i) = (zr * zr - zi * zi, 2.0 * zr * zi);
    let (mut hr, mut hi) = (1.0f64, 0.0f64);
    for s in stages {
        let nr = s[0] as f64 / sc + s[1] as f64 / sc * zr + s[2] as f64 / sc * z2r;
        let ni = s[1] as f64 / sc * zi + s[2] as f64 / sc * z2i;
        let dr = 1.0 + s[3] as f64 / sc * zr + s[4] as f64 / sc * z2r;
        let di = s[3] as f64 / sc * zi + s[4] as f64 / sc * z2i;
        let dd = dr * dr + di * di;
        let (qr, qi) = ((nr * dr + ni * di) / dd, (ni * dr - nr * di) / dd);
        (hr, hi) = (hr * qr - hi * qi, hr * qi + hi * qr);
    }
    20.0 * (hr * hr + hi * hi).sqrt().log10()
}

/// The response of a raw preset, preamp included, at 200 log-spaced points from 20 Hz to 20 kHz as `(Hz, dB)`.
/// A control preset has no stored curve (the Pocket derives its stages from the six positions), so it gives `None`.
pub fn response_curve(preset: &HalcyonPreset) -> Option<Vec<(f64, f64)>> {
    let HalcyonPreset::Raw { preamp, stages, .. } = preset else {
        return None;
    };
    let pre_db = 20.0 * (*preamp as f64 / ONE as f64).log10();
    Some(
        (0..=199)
            .map(|i| {
                let f = 20.0 * 1000f64.powf(i as f64 / 199.0);
                (f, cascade_db(stages, f) + pre_db)
            })
            .collect(),
    )
}

/// Imports an Equalizer APO / AutoEQ profile as a raw preset named `name`. Refuses (never silently changes): more than 10
/// filters, Fc outside 10 Hz..20 kHz, Q outside 0.1..20, gain beyond +-24 dB, a stage that is unstable or does not fit Q2.22.
/// The preamp is the more negative of the file's and the peak-safe value of the quantised cascade (attenuate only).
pub fn import_apo(name: &str, text: &str) -> Result<ApoImport, TauError> {
    check_name(name)?;
    let (pre_db, filters) = parse_apo(text);
    if filters.is_empty() {
        return Err(bad("No enabled filters were found in this profile."));
    }
    if filters.len() > MAX_RAW_STAGES {
        return Err(bad(format!(
            "This profile has {} filters; Halcyon takes {MAX_RAW_STAGES}. Reduce it first (an AutoEQ \"ten band\" export does).",
            filters.len()
        )));
    }
    let mut stages: Vec<[i32; 5]> = Vec::new();
    for (kind, f0, g, q) in &filters {
        if !(10.0..=20000.0).contains(f0) || !(0.1..=20.0).contains(q) || g.abs() > 24.0 {
            return Err(bad(format!(
                "Filter {kind:?} at {f0} Hz, gain {g} dB, Q {q} is outside 10 Hz to 20 kHz, Q 0.1 to 20, and +-24 dB."
            )));
        }
        let c = design_q(*kind, *f0, *g, *q);
        let mut qc = [0i32; 5];
        let mut fits = true;
        for (o, v) in qc.iter_mut().zip(c) {
            let r = (v * ONE as f64).round_ties_even();
            fits &= r.abs() < (1 << 23) as f64;
            *o = r as i32;
        }
        if !fits || !stable_q(qc[3], qc[4]) {
            return Err(bad(format!(
                "Filter {kind:?} at {f0} Hz, gain {g} dB, Q {q} cannot be represented stably."
            )));
        }
        stages.push(qc);
    }
    let worst = (0..=400)
        .map(|i| cascade_db(&stages, 20.0 * 1000f64.powf(i as f64 / 400.0)))
        .fold(-999.0f64, f64::max);
    let sc = ONE as f64;
    let safe_db = -worst.max(0.0);
    let use_db = if pre_db < 0.0 {
        0f64.min(pre_db).min(safe_db)
    } else {
        0f64.min(safe_db)
    };
    let preamp = (10f64.powf(use_db / 20.0) * sc).round_ties_even() as i32;
    let n = stages.len();
    Ok(ApoImport {
        preset: HalcyonPreset::Raw {
            name: name.to_string(),
            preamp,
            stages,
        },
        preamp_db: (use_db * 1000.0).round_ties_even() / 1000.0,
        peak_gain_db: (worst * 1000.0).round_ties_even() / 1000.0,
        filters: n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../testdata/halcyon")
                .join(name),
        )
        .unwrap()
    }
    fn text(name: &str) -> String {
        String::from_utf8(fixture(name)).unwrap()
    }

    // Byte-identical to the reference tool's output for the same profile.
    #[test]
    fn apo_import_matches_the_reference_tool_byte_for_byte() {
        for (txt, prst, name, db, peak) in [
            ("cans.txt", "cans.prst", "CANS", -6.2, 5.467),
            ("bright.txt", "bright.prst", "BRIGHT", -7.883, 7.883),
        ] {
            let imp = import_apo(name, &text(txt)).unwrap();
            assert_eq!(
                pack_presets(std::slice::from_ref(&imp.preset)).unwrap(),
                fixture(prst),
                "{txt}"
            );
            assert_eq!(
                (imp.preamp_db, imp.peak_gain_db, imp.filters),
                (db, peak, 4),
                "{txt}"
            );
        }
    }

    #[test]
    fn the_curve_matches_the_reported_peak_and_preamp() {
        let imp = import_apo("CANS", &text("cans.txt")).unwrap();
        let curve = response_curve(&imp.preset).unwrap();
        let top = curve.iter().map(|p| p.1).fold(-99.0, f64::max);
        // peak boost minus the preamp's attenuation: the curve never rises above 0 dB (the preamp is peak-safe)
        assert!(top <= 0.05, "{top}");
        assert!(
            response_curve(&HalcyonPreset::Control {
                name: "A".into(),
                controls: [0; 6]
            })
            .is_none()
        );
    }

    #[test]
    fn mixed_file_round_trips_and_repacks_identically() {
        let blob = fixture("mixed.prst");
        let presets = parse_presets(&blob).unwrap();
        assert_eq!(
            presets[0],
            HalcyonPreset::Control {
                name: "MY CONTROLS".into(),
                controls: [2, 1, 0, -1, 3, -2]
            }
        );
        assert_eq!(presets[1].name(), "CANS");
        assert_eq!(pack_presets(&presets).unwrap(), blob);
    }

    #[test]
    fn every_single_bit_flip_is_refused() {
        let blob = fixture("mixed.prst");
        for i in 12..blob.len() {
            let mut b = blob.clone();
            b[i] ^= 1;
            assert!(parse_presets(&b).is_err(), "byte {i}");
        }
    }

    #[test]
    fn the_writer_refuses_what_the_firmware_would_not_check() {
        let ctl = |name: &str, c: [i8; 6]| HalcyonPreset::Control {
            name: name.into(),
            controls: c,
        };
        for (p, why) in [
            (ctl("FLAT", [0; 6]), "built-in name"),
            (ctl("lower", [0; 6]), "lower case"),
            (ctl("SIXTEEN CHARS OK", [0; 6]), "16 characters"),
            (ctl("X", [0, 0, 0, 0, -1, 0]), "sibilance below 0"),
            (ctl("X", [6, 0, 0, 0, 0, 0]), "warmth above 5"),
            (
                HalcyonPreset::Raw {
                    name: "X".into(),
                    preamp: 1 << 23,
                    stages: vec![[1 << 22, 0, 0, 0, 0]],
                },
                "preamp above unity",
            ),
            (
                HalcyonPreset::Raw {
                    name: "X".into(),
                    preamp: 1000,
                    stages: vec![[1 << 22, 0, 0, -2 << 22, 1 << 22]],
                },
                "pole on the circle",
            ),
            (
                HalcyonPreset::Raw {
                    name: "X".into(),
                    preamp: 1000,
                    stages: vec![[1 << 23, 0, 0, 0, 0]],
                },
                "coefficient over 24 bits",
            ),
        ] {
            assert!(pack_presets(&[p]).is_err(), "{why}");
        }
        assert!(
            pack_presets(&[ctl("A", [0; 6]), ctl("A", [0; 6])]).is_err(),
            "duplicate"
        );
        assert!(pack_presets(&[]).is_err());
        let many: Vec<_> = (0..9).map(|i| ctl(&format!("P{i}"), [0; 6])).collect();
        assert!(pack_presets(&many).is_err(), "nine presets");
    }

    #[test]
    fn the_importer_refuses_instead_of_changing_a_profile() {
        let one = "Filter 1: ON PK Fc 100 Hz Gain 3 dB Q 1\n";
        assert!(import_apo("X", &one.repeat(11)).is_err(), "11 filters");
        assert!(
            import_apo("X", "Filter 1: ON PK Fc 5 Hz Gain 3 dB Q 1\n").is_err(),
            "Fc too low"
        );
        assert!(
            import_apo("X", "Filter 1: ON PK Fc 100 Hz Gain 30 dB Q 1\n").is_err(),
            "gain too big"
        );
        assert!(
            import_apo("X", "Filter 1: ON PK Fc 100 Hz Gain 3 dB Q 50\n").is_err(),
            "Q too big"
        );
        assert!(
            import_apo("X", "Filter 1: OFF PK Fc 100 Hz Gain 3 dB Q 1\n").is_err(),
            "nothing enabled"
        );
        assert!(import_apo("FLAT", one).is_err(), "built-in name");
    }
}
