//! `tau-assets.bin`, the optional theme file a Tau core reads (data slot 8).
//!
//! Layout and rules are the firmware's (`docs/features/THEME_FILE_FORMAT.md` in tau-alpha, written by
//! `tools/tau_assets.py`); this module is an independent Rust writer/reader checked byte for byte against a file that
//! reference tool produced (`testdata/assets/`). Only the `THEM` section is written here. A file read from a card keeps
//! its themes; any other section (such as `METR`) is not carried, so writing replaces the whole file.
//!
//! The firmware trusts only the CRCs, so the *writer* owns the safety rules: both polarities, a name the device font
//! can draw, and the contrast rules the firmware's own theme generator enforces (text on every surface and on the
//! background ramp behind every accent the device offers). Colours are RGB565 on the device, so every colour is
//! snapped first and the report shows what will really appear.

use crate::{ErrorCode, TauError};
use std::collections::BTreeMap;

const MAGIC: &[u8; 4] = b"TAUA";
/// Section tag in the container table, and the magic inside the section (they differ in the firmware format).
const SECTION_THEM: &[u8; 4] = b"THEM";
const THEM_MAGIC: &[u8; 4] = b"TTHM";
const VERSION: u16 = 1;
const NAME_LEN: usize = 16;
/// Role count the firmware knows (`TR_COUNT` in `fw/theme.h`). The file carries all of them; roles 0, 6 and 8 are not themeable.
const ROLE_COUNT: usize = 21;
pub const MAX_THEMES: usize = 4;
/// Themes the firmware already has; a file may not reuse their names.
pub const BUILTIN_NAMES: [&str; 2] = ["TAU", "OCEAN"];
const LIGHT_ACC_MAX_L: i32 = 110;

/// The themeable roles: file key and the firmware's enum index. Order is the firmware's `ROLES` list.
pub const ROLES: [(&str, usize); 18] = [
    ("bg_bottom", 1), ("surface", 2), ("surface_track", 3), ("text_primary", 4), ("text_secondary", 5),
    ("on_accent", 7), ("ok", 9), ("warn", 10), ("danger", 11), ("base", 12), ("chrome", 13), ("pill", 14),
    ("error", 15), ("faint", 16), ("splash_bg", 17), ("splash_bar", 18), ("fs_red", 19), ("fs_track", 20),
];

/// The accent colours the device offers (`ui_palette[]` in `fw/player.c`, RGB565). The contrast rules are checked against all of them.
const PALETTE: [u16; 19] = [
    0x18C3, 0xF7BE, 0x2A83, 0xE73B, 0x8C51, 0xD1E6, 0xE429, 0x5D0D, 0x4BD4, 0x9BF6, 0xFEA0, 0xE429, 0xD1E6, 0xE4B6,
    0x4BD4, 0x5D0D, 0x6AD2, 0xBDF7, 0x8C71,
];

/// One polarity of a theme as the editor holds it: `#RRGGBB` per role key, plus the background luma.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolarityInput {
    pub bg_luma: u8,
    pub colors: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ThemeInput {
    pub name: String,
    pub dark: PolarityInput,
    pub light: PolarityInput,
}

/// How one contrast rule came out for one polarity.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContrastCheck {
    pub polarity: String,
    pub text: String,
    pub against: String,
    pub worst: f64,
    pub needed: f64,
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ThemeReport {
    /// Plain-language problems; empty means the theme can be written.
    pub problems: Vec<String>,
    pub checks: Vec<ContrastCheck>,
    /// Every colour as the device will show it after RGB565 snapping (`#RRGGBB`), for `dark` then `light`.
    pub dark_snapped: BTreeMap<String, String>,
    pub light_snapped: BTreeMap<String, String>,
}

fn invalid(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidTheme, msg)
}
fn bad_file(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidAssetsFile, msg)
}

/// `#RRGGBB` (or `0xRGB565`) to RGB565, rounding exactly like the firmware tools.
pub fn snap(text: &str) -> Option<u16> {
    let t = text.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u16::from_str_radix(h, 16).ok();
    }
    let h = t.strip_prefix('#')?;
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let c = |i: usize| u32::from_str_radix(&h[i..i + 2], 16).ok();
    let (r, g, b) = (c(0)?, c(2)?, c(4)?);
    Some((((r * 31 + 127) / 255) << 11 | ((g * 63 + 127) / 255) << 5 | ((b * 31 + 127) / 255)) as u16)
}

fn rgb8(c: u16) -> (i32, i32, i32) {
    let c = c as i32;
    ((c >> 11) * 255 / 31, ((c >> 5) & 0x3F) * 255 / 63, (c & 0x1F) * 255 / 31)
}

/// RGB565 back to `#RRGGBB` as the screen shows it.
pub fn to_hex(c: u16) -> String {
    let (r, g, b) = rgb8(c);
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn pack565(r: i32, g: i32, b: i32) -> u16 {
    ((((r * 31 + 127) / 255) << 11) | (((g * 63 + 127) / 255) << 5) | ((b * 31 + 127) / 255)) as u16
}

fn luminance(c: u16) -> f64 {
    let f = |x: i32| {
        let v = x as f64 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = rgb8(c);
    0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
}

fn contrast(a: u16, b: u16) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Mirror of `th_accent_of()`: on Light, an accent brighter than the cap is scaled down to it.
fn acc_eff(a: u16, light: bool) -> u16 {
    if !light {
        return a;
    }
    let (r, g, b) = rgb8(a);
    let l = (2126 * r + 7152 * g + 722 * b) / 10000;
    if l > LIGHT_ACC_MAX_L {
        pack565(r * LIGHT_ACC_MAX_L / l, g * LIGHT_ACC_MAX_L / l, b * LIGHT_ACC_MAX_L / l)
    } else {
        a
    }
}

/// Mirror of `ui_grad_set()`: the top of the background ramp for an accent at a luma.
fn grad_top(accent: u16, luma: i32) -> u16 {
    let (r, g, b) = rgb8(accent);
    let l = ((2126 * r + 7152 * g + 722 * b) / 10000).max(1);
    let scale = |v: i32| (v * luma + l / 2) / l;
    let mix = |v: i32| ((v + luma + 1) / 2).min(255);
    pack565(mix(scale(r)), mix(scale(g)), mix(scale(b)))
}

/// The firmware's per-channel blend (`ui_mix`); the division floors, as the reference does.
fn mix565(a: u16, b: u16, t: i32, n: i32) -> u16 {
    let (a, b) = (a as i32, b as i32);
    let ch = |x: i32, y: i32| x + ((y - x) * t).div_euclid(n);
    ((ch(a >> 11, b >> 11) << 11) | (ch((a >> 5) & 0x3F, (b >> 5) & 0x3F) << 5) | ch(a & 0x1F, b & 0x1F)) as u16
}

/// Contrast rules: text role, backgrounds ("ramp" = the background ramp behind every device accent), minimum ratio.
const RULES: [(&str, &[&str], f64); 3] = [
    ("text_primary", &["surface", "base", "ramp"], 4.5),
    ("text_secondary", &["surface", "base"], 3.0),
    ("text_secondary", &["ramp"], 2.6),
];

fn read_polarity(
    label: &str,
    p: &PolarityInput,
    problems: &mut Vec<String>,
) -> Option<BTreeMap<&'static str, u16>> {
    let mut out = BTreeMap::new();
    if !(20..=235).contains(&p.bg_luma) {
        problems.push(format!("{label}: the background brightness must be between 20 and 235."));
    }
    for (key, _) in ROLES {
        match p.colors.get(key).map(|v| snap(v)) {
            Some(Some(c)) => {
                out.insert(key, c);
            }
            Some(None) => problems.push(format!("{label}: “{key}” is not a colour (use #RRGGBB).")),
            None => problems.push(format!("{label}: “{key}” has no colour.")),
        }
    }
    (out.len() == ROLES.len()).then_some(out)
}

/// Checks one theme: names, ranges and the contrast rules. Never writes anything.
pub fn check_theme(t: &ThemeInput) -> ThemeReport {
    let mut report = ThemeReport::default();
    let name_ok = !t.name.is_empty()
        && t.name.len() <= NAME_LEN - 1
        && t.name.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || matches!(b, b' ' | b'_' | b'-'));
    if !name_ok {
        report.problems.push("The name must be 1 to 15 characters: capital letters, digits, space, _ or - (the Pocket's font is capitals only).".into());
    }
    if BUILTIN_NAMES.contains(&t.name.as_str()) {
        report.problems.push(format!("“{}” is already a built-in theme; pick another name.", t.name));
    }
    for (label, light, p) in [("Dark", false, &t.dark), ("Light", true, &t.light)] {
        let Some(c) = read_polarity(label, p, &mut report.problems) else { continue };
        let snapped: BTreeMap<String, String> = c.iter().map(|(k, v)| (k.to_string(), to_hex(*v))).collect();
        if light { report.light_snapped = snapped } else { report.dark_snapped = snapped }
        if !(20..=235).contains(&p.bg_luma) {
            continue;
        }
        let luma = p.bg_luma as i32;
        for (text, backs, need) in RULES {
            let mut worst = 99.0f64;
            for back in backs {
                if *back == "ramp" {
                    for ac in PALETTE {
                        let top = grad_top(acc_eff(ac, light), luma);
                        for col in [top, mix565(top, c["bg_bottom"], 20, 40)] {
                            worst = worst.min(contrast(c[text], col));
                        }
                    }
                } else {
                    worst = worst.min(contrast(c[text], c[*back]));
                }
            }
            let ok = worst >= need;
            if !ok {
                report.problems.push(format!(
                    "{label}: {} on {} is too faint ({worst:.2}, needs {need}).",
                    text.replace('_', " "),
                    backs.join(" / ").replace("ramp", "the background")
                ));
            }
            report.checks.push(ContrastCheck {
                polarity: label.to_lowercase(), text: text.into(), against: backs.join("/"), worst, needed: need, ok,
            });
        }
        if light {
            // On Light the accent is also used as text and fills against the surface.
            let worst = PALETTE.iter().map(|ac| contrast(acc_eff(*ac, true), c["surface"])).fold(99.0, f64::min);
            let ok = worst >= 3.0;
            if !ok {
                report.problems.push(format!("Light: some accent colours would be too faint on the surface ({worst:.2}, needs 3.0)."));
            }
            report.checks.push(ContrastCheck { polarity: "light".into(), text: "accent".into(), against: "surface".into(), worst, needed: 3.0, ok });
        }
    }
    report
}

fn crc(b: &[u8]) -> u32 {
    crc32fast::hash(b)
}

fn pack_them(themes: &[ThemeInput]) -> Result<Vec<u8>, TauError> {
    if themes.is_empty() || themes.len() > MAX_THEMES {
        return Err(invalid(format!("A file holds 1 to {MAX_THEMES} themes.")));
    }
    let mut seen: Vec<&str> = Vec::new();
    let mut body = Vec::new();
    for t in themes {
        if seen.contains(&t.name.as_str()) {
            return Err(invalid(format!("Two themes are called “{}”.", t.name)));
        }
        seen.push(&t.name);
        let report = check_theme(t);
        if !report.problems.is_empty() {
            return Err(invalid(format!("{}: {}", t.name, report.problems.join(" "))));
        }
        let mut name = [0u8; NAME_LEN];
        name[..t.name.len()].copy_from_slice(t.name.as_bytes());
        body.extend_from_slice(&name);
        body.extend_from_slice(&[t.dark.bg_luma, t.light.bg_luma, 0, 0]);
        for p in [&t.dark, &t.light] {
            let mut roles = [0u16; ROLE_COUNT];
            for (key, idx) in ROLES {
                roles[idx] = snap(&p.colors[key]).expect("checked by check_theme");
            }
            for r in roles {
                body.extend_from_slice(&r.to_le_bytes());
            }
        }
    }
    let mut out = Vec::with_capacity(12 + body.len());
    out.extend_from_slice(THEM_MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.push(ROLE_COUNT as u8);
    out.push(themes.len() as u8);
    out.extend_from_slice(&crc(&body).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// The whole `tau-assets.bin` for these themes (a container with one `THEM` section). Refuses any theme that fails `check_theme`.
pub fn pack_assets(themes: &[ThemeInput]) -> Result<Vec<u8>, TauError> {
    let them = pack_them(themes)?;
    let mut table = Vec::new();
    table.extend_from_slice(SECTION_THEM);
    table.extend_from_slice(&(12u32 + 16).to_le_bytes());
    table.extend_from_slice(&(them.len() as u32).to_le_bytes());
    table.extend_from_slice(&crc(&them).to_le_bytes());
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&crc(&table).to_le_bytes());
    out.extend_from_slice(&table);
    out.extend_from_slice(&them);
    Ok(out)
}

fn u16le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Reads a `tau-assets.bin`, verifying the version and every CRC before using a byte, and returns its themes for editing.
/// A file without a `THEM` section has no themes (an empty list).
pub fn parse_assets(blob: &[u8]) -> Result<Vec<ThemeInput>, TauError> {
    if blob.len() < 12 || &blob[..4] != MAGIC {
        return Err(bad_file("This is not a Tau assets file."));
    }
    if u16le(blob, 4) != VERSION {
        return Err(bad_file(format!("Unsupported assets file version {}.", u16le(blob, 4))));
    }
    let n = u16le(blob, 6) as usize;
    if n > 8 || blob.len() < 12 + 16 * n {
        return Err(bad_file("The section table is damaged."));
    }
    let table = &blob[12..12 + 16 * n];
    if crc(table) != u32le(blob, 8) {
        return Err(bad_file("The section table failed its check."));
    }
    for i in 0..n {
        let e = &table[i * 16..i * 16 + 16];
        let (off, len) = (u32le(e, 4) as usize, u32le(e, 8) as usize);
        let end = off.checked_add(len).filter(|end| *end <= blob.len()).ok_or_else(|| bad_file("A section runs past the end of the file."))?;
        let data = &blob[off..end];
        if crc(data) != u32le(e, 12) {
            return Err(bad_file("A section failed its check."));
        }
        if &e[..4] == SECTION_THEM {
            return parse_them(data);
        }
    }
    Ok(Vec::new())
}

fn parse_them(d: &[u8]) -> Result<Vec<ThemeInput>, TauError> {
    if d.len() < 12 || &d[..4] != THEM_MAGIC || u16le(d, 4) != VERSION {
        return Err(bad_file("The theme section is damaged."));
    }
    let (rc, tc) = (d[6] as usize, d[7] as usize);
    if tc == 0 || crc(&d[12..]) != u32le(d, 8) {
        return Err(bad_file("The theme section failed its check."));
    }
    let per = NAME_LEN + 4 + 4 * rc;
    if d.len() != 12 + per * tc {
        return Err(bad_file("The theme section has the wrong size."));
    }
    let mut out = Vec::new();
    for i in 0..tc {
        let e = &d[12 + i * per..12 + (i + 1) * per];
        let name_end = e[..NAME_LEN].iter().position(|b| *b == 0).unwrap_or(NAME_LEN);
        let name = String::from_utf8_lossy(&e[..name_end]).into_owned();
        let pol = |luma: u8, base: usize| {
            let mut colors = BTreeMap::new();
            for (key, idx) in ROLES {
                // A file written for an older firmware may carry fewer roles; the missing ones start from white to be edited.
                let v = if idx < rc { u16le(e, base + 2 * idx) } else { 0xFFFF };
                colors.insert(key.to_string(), to_hex(v));
            }
            PolarityInput { bg_luma: luma, colors }
        };
        out.push(ThemeInput { name, dark: pol(e[16], 20), light: pol(e[17], 20 + 2 * rc) });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUNSET_JSON: &str = include_str!("../testdata/assets/sunset.json");
    const SUNSET_BIN: &[u8] = include_bytes!("../testdata/assets/sunset.tau-assets.bin");

    /// The reference theme JSON (`#RRGGBB` per key, bg_luma per polarity) as an editor input.
    fn sunset() -> ThemeInput {
        let v: serde_json::Value = serde_json::from_str(SUNSET_JSON).unwrap();
        let pol = |p: &str| {
            let o = v[p].as_object().unwrap();
            PolarityInput {
                bg_luma: o["bg_luma"].as_u64().unwrap() as u8,
                colors: ROLES.iter().map(|(k, _)| (k.to_string(), o[*k].as_str().unwrap().to_string())).collect(),
            }
        };
        ThemeInput { name: v["name"].as_str().unwrap().into(), dark: pol("dark"), light: pol("light") }
    }

    #[test]
    fn writes_exactly_what_the_firmwares_own_tool_writes() {
        assert_eq!(pack_assets(&[sunset()]).unwrap(), SUNSET_BIN);
    }

    #[test]
    fn a_written_file_reads_back_to_the_same_bytes() {
        let themes = parse_assets(SUNSET_BIN).unwrap();
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "SUNSET");
        assert_eq!(pack_assets(&themes).unwrap(), SUNSET_BIN);
    }

    #[test]
    fn every_flipped_byte_and_every_truncation_is_refused() {
        for i in 0..SUNSET_BIN.len() {
            let mut b = SUNSET_BIN.to_vec();
            b[i] ^= 0x01;
            assert!(parse_assets(&b).is_err(), "flipping byte {i} was accepted");
        }
        for n in 0..SUNSET_BIN.len() {
            assert!(parse_assets(&SUNSET_BIN[..n]).is_err(), "a file cut to {n} bytes was accepted");
        }
    }

    #[test]
    fn contrast_figures_match_the_firmware_generators_report() {
        let r = check_theme(&sunset());
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        let worst = |pol: &str, text: &str, against: &str| r.checks.iter().find(|c| c.polarity == pol && c.text == text && c.against == against).unwrap().worst;
        for (pol, text, against, want) in [
            ("dark", "text_primary", "surface/base/ramp", 12.21), ("dark", "text_secondary", "surface/base", 7.14),
            ("dark", "text_secondary", "ramp", 5.69), ("light", "text_primary", "surface/base/ramp", 10.11),
            ("light", "text_secondary", "surface/base", 6.71), ("light", "text_secondary", "ramp", 4.86),
        ] {
            assert!((worst(pol, text, against) - want).abs() < 0.006, "{pol} {text} vs {against}: {} != {want}", worst(pol, text, against));
        }
    }

    #[test]
    fn a_faint_theme_is_refused_and_says_why() {
        let mut t = sunset();
        let surface = t.dark.colors["surface"].clone();
        t.dark.colors.insert("text_primary".into(), surface);
        let r = check_theme(&t);
        assert!(r.problems.iter().any(|p| p.contains("text primary") && p.contains("too faint")), "{:?}", r.problems);
        assert!(pack_assets(&[t]).is_err());
    }

    #[test]
    fn names_ranges_and_missing_colours_are_refused() {
        let mut t = sunset();
        t.name = "sunset".into();
        assert!(!check_theme(&t).problems.is_empty());
        t.name = "TAU".into();
        assert!(check_theme(&t).problems.iter().any(|p| p.contains("built-in")));
        let mut t = sunset();
        t.dark.bg_luma = 10;
        assert!(!check_theme(&t).problems.is_empty());
        let mut t = sunset();
        t.light.colors.remove("ok");
        assert!(check_theme(&t).problems.iter().any(|p| p.contains("no colour")));
        let mut t = sunset();
        t.dark.colors.insert("ok".into(), "green".into());
        assert!(check_theme(&t).problems.iter().any(|p| p.contains("not a colour")));
        assert!(pack_assets(&[]).is_err());
        let (a, mut b) = (sunset(), sunset());
        assert!(pack_assets(&[a.clone(), a.clone()]).is_err(), "duplicate names");
        b.name = "SUNSET 2".into();
        assert!(pack_assets(&[a, b]).is_ok());
    }

    #[test]
    fn snapping_matches_the_device() {
        assert_eq!(snap("#FFFFFF"), Some(0xFFFF));
        assert_eq!(snap("#000000"), Some(0));
        assert_eq!(snap("0x2945"), Some(0x2945));
        assert_eq!(snap("#10060"), None);
        assert_eq!(to_hex(0xFFFF), "#FFFFFF");
        // What the editor shows is what the device shows: snapping twice changes nothing.
        let once = snap("#2A1820").unwrap();
        assert_eq!(snap(&to_hex(once)), Some(once));
    }
}
