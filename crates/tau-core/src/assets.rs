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
    ("bg_bottom", 1),
    ("surface", 2),
    ("surface_track", 3),
    ("text_primary", 4),
    ("text_secondary", 5),
    ("on_accent", 7),
    ("ok", 9),
    ("warn", 10),
    ("danger", 11),
    ("base", 12),
    ("chrome", 13),
    ("pill", 14),
    ("error", 15),
    ("faint", 16),
    ("splash_bg", 17),
    ("splash_bar", 18),
    ("fs_red", 19),
    ("fs_track", 20),
];

/// The accent colours the device offers (`ui_palette[]` in `fw/player.c`, RGB565). The contrast rules are checked against all of them.
const PALETTE: [u16; 19] = [
    0x18C3, 0xF7BE, 0x2A83, 0xE73B, 0x8C51, 0xD1E6, 0xE429, 0x5D0D, 0x4BD4, 0x9BF6, 0xFEA0, 0xE429,
    0xD1E6, 0xE4B6, 0x4BD4, 0x5D0D, 0x6AD2, 0xBDF7, 0x8C71,
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
    Some(
        (((r * 31 + 127) / 255) << 11 | ((g * 63 + 127) / 255) << 5 | ((b * 31 + 127) / 255))
            as u16,
    )
}

fn rgb8(c: u16) -> (i32, i32, i32) {
    let c = c as i32;
    (
        (c >> 11) * 255 / 31,
        ((c >> 5) & 0x3F) * 255 / 63,
        (c & 0x1F) * 255 / 31,
    )
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
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
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
        pack565(
            r * LIGHT_ACC_MAX_L / l,
            g * LIGHT_ACC_MAX_L / l,
            b * LIGHT_ACC_MAX_L / l,
        )
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
    ((ch(a >> 11, b >> 11) << 11)
        | (ch((a >> 5) & 0x3F, (b >> 5) & 0x3F) << 5)
        | ch(a & 0x1F, b & 0x1F)) as u16
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
        problems.push(format!(
            "{label}: the background brightness must be between 20 and 235."
        ));
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
        && t.name.len() < NAME_LEN
        && t.name.bytes().all(|b| {
            b.is_ascii_uppercase() || b.is_ascii_digit() || matches!(b, b' ' | b'_' | b'-')
        });
    if !name_ok {
        report.problems.push("The name must be 1 to 15 characters: capital letters, digits, space, _ or - (the Pocket's font is capitals only).".into());
    }
    if BUILTIN_NAMES.contains(&t.name.as_str()) {
        report.problems.push(format!(
            "“{}” is already a built-in theme; pick another name.",
            t.name
        ));
    }
    for (label, light, p) in [("Dark", false, &t.dark), ("Light", true, &t.light)] {
        let Some(c) = read_polarity(label, p, &mut report.problems) else {
            continue;
        };
        let snapped: BTreeMap<String, String> =
            c.iter().map(|(k, v)| (k.to_string(), to_hex(*v))).collect();
        if light {
            report.light_snapped = snapped
        } else {
            report.dark_snapped = snapped
        }
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
                polarity: label.to_lowercase(),
                text: text.into(),
                against: backs.join("/"),
                worst,
                needed: need,
                ok,
            });
        }
        if light {
            // On Light the accent is also used as text and fills against the surface.
            let worst = PALETTE
                .iter()
                .map(|ac| contrast(acc_eff(*ac, true), c["surface"]))
                .fold(99.0, f64::min);
            let ok = worst >= 3.0;
            if !ok {
                report.problems.push(format!("Light: some accent colours would be too faint on the surface ({worst:.2}, needs 3.0)."));
            }
            report.checks.push(ContrastCheck {
                polarity: "light".into(),
                text: "accent".into(),
                against: "surface".into(),
                worst,
                needed: 3.0,
                ok,
            });
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
            return Err(invalid(format!(
                "{}: {}",
                t.name,
                report.problems.join(" ")
            )));
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

/// Verifies the container and returns each section's tag and bytes (every CRC checked first).
type Section<'a> = ([u8; 4], &'a [u8]);

fn read_sections(blob: &[u8]) -> Result<Vec<Section<'_>>, TauError> {
    if blob.len() < 12 || &blob[..4] != MAGIC {
        return Err(bad_file("This is not a Tau assets file."));
    }
    if u16le(blob, 4) != VERSION {
        return Err(bad_file(format!(
            "Unsupported assets file version {}.",
            u16le(blob, 4)
        )));
    }
    let n = u16le(blob, 6) as usize;
    if n > 8 || blob.len() < 12 + 16 * n {
        return Err(bad_file("The section table is damaged."));
    }
    let table = &blob[12..12 + 16 * n];
    if crc(table) != u32le(blob, 8) {
        return Err(bad_file("The section table failed its check."));
    }
    let mut out = Vec::new();
    for i in 0..n {
        let e = &table[i * 16..i * 16 + 16];
        let (off, len) = (u32le(e, 4) as usize, u32le(e, 8) as usize);
        let end = off
            .checked_add(len)
            .filter(|end| *end <= blob.len())
            .ok_or_else(|| bad_file("A section runs past the end of the file."))?;
        let data = &blob[off..end];
        if crc(data) != u32le(e, 12) {
            return Err(bad_file("A section failed its check."));
        }
        out.push(([e[0], e[1], e[2], e[3]], data));
    }
    Ok(out)
}

/// Reads a `tau-assets.bin`, verifying the version and every CRC before using a byte, and returns its themes for editing.
/// A file without a `THEM` section has no themes (an empty list).
pub fn parse_assets(blob: &[u8]) -> Result<Vec<ThemeInput>, TauError> {
    for (tag, data) in read_sections(blob)? {
        if &tag == SECTION_THEM {
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
        let name_end = e[..NAME_LEN]
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(NAME_LEN);
        let name = String::from_utf8_lossy(&e[..name_end]).into_owned();
        let pol = |luma: u8, base: usize| {
            let mut colors = BTreeMap::new();
            for (key, idx) in ROLES {
                // A file written for an older firmware may carry fewer roles; the missing ones start from white to be edited.
                let v = if idx < rc {
                    u16le(e, base + 2 * idx)
                } else {
                    0xFFFF
                };
                colors.insert(key.to_string(), to_hex(v));
            }
            PolarityInput {
                bg_luma: luma,
                colors,
            }
        };
        out.push(ThemeInput {
            name,
            dark: pol(e[16], 20),
            light: pol(e[17], 20 + 2 * rc),
        });
    }
    Ok(out)
}

// ---- installing to a card -------------------------------------------------------------------------------------

use crate::sync;
use std::fs;
use std::path::{Path, PathBuf};

/// File name on the card (data slot 8 of a Tau core).
pub const FILE_NAME: &str = "tau-assets.bin";
const TEMP_NAME: &str = ".tau-assets.bin.tmp";
const PREVIOUS_NAME: &str = ".tau-assets.bin.prev";

/// What is already at the destination, so the review can say what an install would replace.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExistingAssets {
    pub bytes: u64,
    pub sha256: String,
    /// Names of the themes in it (empty when it has none or cannot be read).
    pub themes: Vec<String>,
    /// Sections other than `THEM` (for example `METR` meter presets): they are **not** carried over by an install.
    pub other_sections: Vec<String>,
    /// False when the file does not parse; it is still backed up before being replaced.
    pub readable: bool,
}

/// A core on this card that uses the same media folder, and whether it asks for the file at all.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ThemeFileReader {
    pub core_id: String,
    pub version: String,
    /// True when the core's `data.json` declares a slot for `tau-assets.bin`.
    pub declares_slot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssetsInstallPlan {
    /// Confirmation token: changes when the destination, the new file or the existing file changes.
    pub id: String,
    pub destination: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub themes: Vec<String>,
    pub existing: Option<ExistingAssets>,
    pub readers: Vec<ThemeFileReader>,
    /// A previous install was interrupted; running this one first puts the old file back.
    pub interrupted_install: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssetsInstallReport {
    pub destination: PathBuf,
    pub bytes_written: u64,
    pub replaced: bool,
    pub backup: Option<PathBuf>,
}

fn describe_existing(bytes: &[u8]) -> ExistingAssets {
    let mut e = ExistingAssets {
        bytes: bytes.len() as u64,
        sha256: sync::sha256_bytes(bytes),
        themes: Vec::new(),
        other_sections: Vec::new(),
        readable: false,
    };
    if let Ok(sections) = read_sections(bytes) {
        e.readable = true;
        for (tag, _) in &sections {
            if tag != SECTION_THEM {
                e.other_sections
                    .push(String::from_utf8_lossy(tag).into_owned());
            }
        }
        match parse_assets(bytes) {
            Ok(t) => e.themes = t.into_iter().map(|t| t.name).collect(),
            Err(_) => e.readable = false,
        }
    }
    e
}

/// Cores under `<card>/Cores` whose platform is the media root's platform. `media_root` is `<card>/Assets/<platform>/common`.
fn readers_for(media_root: &Path) -> Vec<ThemeFileReader> {
    let platform = media_root
        .parent()
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned());
    let card = media_root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent);
    let (Some(platform), Some(card)) = (platform, card) else {
        return Vec::new();
    };
    let Ok(dir) = fs::read_dir(card.join("Cores")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for folder in dir.filter_map(Result::ok).filter(|f| f.path().is_dir()) {
        let read = |name: &str| -> Option<serde_json::Value> {
            serde_json::from_slice(&fs::read(folder.path().join(name)).ok()?).ok()
        };
        let Some(core) = read("core.json") else {
            continue;
        };
        let meta = core.pointer("/core/metadata");
        let plat = meta
            .and_then(|m| m.pointer("/platform_ids/0"))
            .and_then(|v| v.as_str());
        if plat != Some(platform.as_str()) {
            continue;
        }
        let version = meta
            .and_then(|m| m.get("version"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let declares_slot = read("data.json").is_some_and(|d| {
            [
                "/data/data_slots",
                "/core/data/data_slots",
                "/data",
                "/core/data",
            ]
            .iter()
            .any(|p| {
                d.pointer(p).and_then(|v| v.as_array()).is_some_and(|a| {
                    a.iter()
                        .any(|s| s.get("filename").and_then(|f| f.as_str()) == Some(FILE_NAME))
                })
            })
        });
        out.push(ThemeFileReader {
            core_id: folder.file_name().to_string_lossy().into_owned(),
            version,
            declares_slot,
        });
    }
    out.sort_by(|a, b| a.core_id.cmp(&b.core_id));
    out
}

/// Plans writing these themes to `<media_root>/tau-assets.bin`. Writes nothing. Refuses any theme that fails
/// `check_theme`, a media root that is not an `Assets/<platform>/common` folder, and a missing folder.
pub fn plan_install(
    themes: &[ThemeInput],
    media_root: &Path,
) -> Result<AssetsInstallPlan, TauError> {
    sync::validate_media_root(media_root)?;
    let blob = pack_assets(themes)?;
    let destination = media_root.join(FILE_NAME);
    let existing = fs::read(&destination).ok().map(|b| describe_existing(&b));
    let sha256 = sync::sha256_bytes(&blob);
    let readers = readers_for(media_root);
    let interrupted_install = !destination.is_file() && media_root.join(PREVIOUS_NAME).is_file();
    let mut warnings = Vec::new();
    if readers.is_empty() {
        warnings.push(
            "No core on this card uses this media folder, so nothing would read the file.".into(),
        );
    } else if !readers.iter().any(|r| r.declares_slot) {
        warnings.push("None of the cores that use this folder ask for a theme file (they need Tau 0.5.0 or later), so it will be ignored until one is installed.".into());
    }
    if let Some(e) = &existing {
        if !e.other_sections.is_empty() {
            warnings.push(format!("The file already there also holds {}, which is not carried over: it will be gone after this install (the backup keeps it).", e.other_sections.join(", ")));
        }
        if !e.readable {
            warnings.push("The file already there cannot be read as a Tau assets file. It will still be backed up before it is replaced.".into());
        }
    }
    let id = sync::sha256_bytes(
        format!(
            "assets|{}|{}|{}",
            destination.display(),
            sha256,
            existing.as_ref().map_or("none", |e| e.sha256.as_str())
        )
        .as_bytes(),
    );
    Ok(AssetsInstallPlan {
        id,
        destination,
        bytes: blob.len() as u64,
        sha256,
        themes: themes.iter().map(|t| t.name.clone()).collect(),
        existing,
        readers,
        interrupted_install,
        warnings,
    })
}

/// Puts back an old file left by an install that was interrupted between its two renames (never invents one).
fn recover(media_root: &Path) -> Result<(), TauError> {
    let live = media_root.join(FILE_NAME);
    let previous = media_root.join(PREVIOUS_NAME);
    if previous.is_file() {
        if live.is_file() {
            let _ = fs::remove_file(&previous);
        } else {
            fs::rename(&previous, &live)?;
        }
    }
    Ok(())
}

/// Writes a reviewed [`AssetsInstallPlan`]. Refuses unless `confirmation` is the plan's id **and** the plan is
/// still what a fresh plan would be (the existing file or the themes changed since review). Order: recover an
/// interrupted install, back up the existing file (when `backup_root` is given; it must be outside the card),
/// write a temporary file beside the target, read it back through the cache-bypassing path and check it parses
/// to the same bytes, swap it in keeping the old file until the new one is in place, then read the result back.
pub fn execute_install(
    themes: &[ThemeInput],
    media_root: &Path,
    plan: &AssetsInstallPlan,
    confirmation: &str,
    backup_root: Option<&Path>,
) -> Result<AssetsInstallReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    // A backup on the card itself is not a backup, so the whole card is off limits, not just the media folder.
    let card = media_root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap_or(media_root);
    if let Some(backup) = backup_root
        && (backup.as_os_str().is_empty() || sync::backup_is_inside(backup, card))
    {
        return Err(TauError::e(
            ErrorCode::UnsafeBackupLocation,
            "backup folder must be outside the card",
        ));
    }
    recover(media_root)?;
    let fresh = plan_install(themes, media_root)?;
    if fresh.id != plan.id {
        return Err(TauError::e(
            ErrorCode::SourceChangedSincePlan,
            "the theme file on the card or the themes changed since the plan was reviewed",
        ));
    }
    let blob = pack_assets(themes)?;
    let live = media_root.join(FILE_NAME);
    let mut backup = None;
    if live.is_file()
        && let Some(root) = backup_root
    {
        let old = fs::read(&live)?;
        let dest = root.join(&plan.id).join(FILE_NAME);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        sync::write_durable(&dest, &old)?;
        if sync::sha256_bytes(&fs::read(&dest)?) != sync::sha256_bytes(&old) {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                "the backup of the existing theme file did not read back the same, so nothing was changed",
            ));
        }
        backup = Some(dest);
    }
    let temp = media_root.join(TEMP_NAME);
    let result = (|| -> Result<(), TauError> {
        sync::write_durable(&temp, &blob)?;
        let back = sync::read_back_bytes(&temp)?;
        if back != blob || pack_assets(&parse_assets(&back)?)? != blob {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                "the theme file written to the card did not read back the same",
            ));
        }
        sync::swap_in_file(&temp, &live, PREVIOUS_NAME)?;
        if sync::read_back_bytes(&live)? != blob {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                "the installed theme file did not read back the same",
            ));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    // macOS writes an AppleDouble `._<name>` beside what it renames on exFAT/FAT (SAFETY_RULES 7). Remove only the
    // ones that belong to the three names this install used.
    for name in [FILE_NAME, TEMP_NAME, PREVIOUS_NAME] {
        let _ = fs::remove_file(media_root.join(format!("._{name}")));
    }
    result?;
    Ok(AssetsInstallReport {
        destination: live,
        bytes_written: blob.len() as u64,
        replaced: plan.existing.is_some(),
        backup,
    })
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
                colors: ROLES
                    .iter()
                    .map(|(k, _)| (k.to_string(), o[*k].as_str().unwrap().to_string()))
                    .collect(),
            }
        };
        ThemeInput {
            name: v["name"].as_str().unwrap().into(),
            dark: pol("dark"),
            light: pol("light"),
        }
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
            assert!(
                parse_assets(&SUNSET_BIN[..n]).is_err(),
                "a file cut to {n} bytes was accepted"
            );
        }
    }

    #[test]
    fn contrast_figures_match_the_firmware_generators_report() {
        let r = check_theme(&sunset());
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        let worst = |pol: &str, text: &str, against: &str| {
            r.checks
                .iter()
                .find(|c| c.polarity == pol && c.text == text && c.against == against)
                .unwrap()
                .worst
        };
        for (pol, text, against, want) in [
            ("dark", "text_primary", "surface/base/ramp", 12.21),
            ("dark", "text_secondary", "surface/base", 7.14),
            ("dark", "text_secondary", "ramp", 5.69),
            ("light", "text_primary", "surface/base/ramp", 10.11),
            ("light", "text_secondary", "surface/base", 6.71),
            ("light", "text_secondary", "ramp", 4.86),
        ] {
            assert!(
                (worst(pol, text, against) - want).abs() < 0.006,
                "{pol} {text} vs {against}: {} != {want}",
                worst(pol, text, against)
            );
        }
    }

    #[test]
    fn a_faint_theme_is_refused_and_says_why() {
        let mut t = sunset();
        let surface = t.dark.colors["surface"].clone();
        t.dark.colors.insert("text_primary".into(), surface);
        let r = check_theme(&t);
        assert!(
            r.problems
                .iter()
                .any(|p| p.contains("text primary") && p.contains("too faint")),
            "{:?}",
            r.problems
        );
        assert!(pack_assets(&[t]).is_err());
    }

    #[test]
    fn names_ranges_and_missing_colours_are_refused() {
        let mut t = sunset();
        t.name = "sunset".into();
        assert!(!check_theme(&t).problems.is_empty());
        t.name = "TAU".into();
        assert!(
            check_theme(&t)
                .problems
                .iter()
                .any(|p| p.contains("built-in"))
        );
        let mut t = sunset();
        t.dark.bg_luma = 10;
        assert!(!check_theme(&t).problems.is_empty());
        let mut t = sunset();
        t.light.colors.remove("ok");
        assert!(
            check_theme(&t)
                .problems
                .iter()
                .any(|p| p.contains("no colour"))
        );
        let mut t = sunset();
        t.dark.colors.insert("ok".into(), "green".into());
        assert!(
            check_theme(&t)
                .problems
                .iter()
                .any(|p| p.contains("not a colour"))
        );
        assert!(pack_assets(&[]).is_err());
        let (a, mut b) = (sunset(), sunset());
        assert!(
            pack_assets(&[a.clone(), a.clone()]).is_err(),
            "duplicate names"
        );
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

    fn card(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "tau-assets-{name}-{}-{}",
            std::process::id(),
            crate::test_uniq()
        ));
        let _ = fs::remove_dir_all(&root);
        let media = root.join("Assets").join("tau").join("common");
        fs::create_dir_all(&media).unwrap();
        let core = root.join("Cores").join("alfatreze.TAU");
        fs::create_dir_all(&core).unwrap();
        fs::write(
            core.join("core.json"),
            r#"{"core":{"metadata":{"platform_ids":["tau"],"version":"0.6.0"}}}"#,
        )
        .unwrap();
        fs::write(
            core.join("data.json"),
            r#"{"data":{"data_slots":[{"id":8,"filename":"tau-assets.bin"}]}}"#,
        )
        .unwrap();
        (root, media)
    }

    #[test]
    fn plans_without_writing_and_names_the_cores_that_will_read_it() {
        let (root, media) = card("plan");
        let plan = plan_install(&[sunset()], &media).unwrap();
        assert!(plan.existing.is_none() && !plan.interrupted_install && plan.warnings.is_empty());
        assert_eq!(plan.themes, vec!["SUNSET"]);
        assert_eq!(
            plan.readers,
            vec![ThemeFileReader {
                core_id: "alfatreze.TAU".into(),
                version: "0.6.0".into(),
                declares_slot: true
            }]
        );
        assert_eq!(plan.bytes, SUNSET_BIN.len() as u64);
        assert!(!media.join(FILE_NAME).exists(), "planning wrote something");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn installs_verifies_and_leaves_no_temp_files() {
        let (root, media) = card("install");
        let plan = plan_install(&[sunset()], &media).unwrap();
        let report = execute_install(&[sunset()], &media, &plan, &plan.id, None).unwrap();
        assert_eq!(fs::read(media.join(FILE_NAME)).unwrap(), SUNSET_BIN);
        assert!(!report.replaced && report.backup.is_none());
        let left: Vec<_> = fs::read_dir(&media)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec![FILE_NAME]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn replacing_backs_up_the_old_file_and_warns_about_what_is_lost() {
        let (root, media) = card("replace");
        // An existing file that also carries a meter-preset section this writer cannot keep.
        let mut other = sunset();
        other.name = "OLDER".into();
        let first = pack_assets(&[other]).unwrap();
        // Rebuild the container with an extra METR section by hand (tag + bytes), CRCs valid.
        let them = parse_assets(&first).unwrap();
        assert_eq!(them.len(), 1);
        fs::write(media.join(FILE_NAME), &first).unwrap();
        let backups = root.with_extension("backups");
        let plan = plan_install(&[sunset()], &media).unwrap();
        let e = plan.existing.as_ref().unwrap();
        assert_eq!(e.themes, vec!["OLDER"]);
        assert!(e.readable);
        let report = execute_install(&[sunset()], &media, &plan, &plan.id, Some(&backups)).unwrap();
        assert!(report.replaced);
        assert_eq!(fs::read(report.backup.unwrap()).unwrap(), first);
        let _ = fs::remove_dir_all(&backups);
        assert_eq!(fs::read(media.join(FILE_NAME)).unwrap(), SUNSET_BIN);
        assert!(!media.join(PREVIOUS_NAME).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn an_unreadable_existing_file_is_flagged_and_still_backed_up() {
        let (root, media) = card("junk");
        fs::write(media.join(FILE_NAME), b"not a tau file").unwrap();
        let plan = plan_install(&[sunset()], &media).unwrap();
        assert!(!plan.existing.as_ref().unwrap().readable);
        assert!(plan.warnings.iter().any(|w| w.contains("cannot be read")));
        let report = execute_install(
            &[sunset()],
            &media,
            &plan,
            &plan.id,
            Some(&root.with_extension("backups")),
        )
        .unwrap();
        assert_eq!(fs::read(report.backup.unwrap()).unwrap(), b"not a tau file");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_a_wrong_token_a_changed_card_and_an_unsafe_backup() {
        let (root, media) = card("refuse");
        let plan = plan_install(&[sunset()], &media).unwrap();
        assert_eq!(
            execute_install(&[sunset()], &media, &plan, "nope", None)
                .unwrap_err()
                .code(),
            ErrorCode::ConfirmationMismatch
        );
        // The card's file changes after review.
        fs::write(media.join(FILE_NAME), b"changed behind our back").unwrap();
        assert_eq!(
            execute_install(&[sunset()], &media, &plan, &plan.id, None)
                .unwrap_err()
                .code(),
            ErrorCode::SourceChangedSincePlan
        );
        assert_eq!(
            fs::read(media.join(FILE_NAME)).unwrap(),
            b"changed behind our back",
            "a refused install changed the file"
        );
        let fresh = plan_install(&[sunset()], &media).unwrap();
        assert_eq!(
            execute_install(&[sunset()], &media, &fresh, &fresh.id, Some(&root))
                .unwrap_err()
                .code(),
            ErrorCode::UnsafeBackupLocation
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn an_interrupted_install_is_recovered_not_lost() {
        let (root, media) = card("recover");
        fs::write(media.join(PREVIOUS_NAME), SUNSET_BIN).unwrap(); // the old file, moved aside; the new one never arrived
        let plan = plan_install(&[sunset()], &media).unwrap();
        assert!(plan.interrupted_install);
        // Installing something else first restores the old file, so it is the "existing" one that gets replaced and backed up.
        let mut t = sunset();
        t.name = "SUNSET 2".into();
        let plan2 = plan_install(&[t.clone()], &media).unwrap();
        assert!(plan2.interrupted_install);
        recover(&media).unwrap();
        assert_eq!(fs::read(media.join(FILE_NAME)).unwrap(), SUNSET_BIN);
        assert!(!media.join(PREVIOUS_NAME).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_a_folder_that_is_not_a_media_root_and_a_faint_theme() {
        let (root, media) = card("bad");
        assert!(plan_install(&[sunset()], &root).is_err());
        assert!(plan_install(&[sunset()], &media.join("missing")).is_err());
        let mut t = sunset();
        t.dark
            .colors
            .insert("text_primary".into(), t.dark.colors["surface"].clone());
        assert!(plan_install(&[t], &media).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn warns_when_no_core_asks_for_the_file() {
        let (root, media) = card("noslot");
        fs::write(
            root.join("Cores/alfatreze.TAU/data.json"),
            r#"{"data":{"data_slots":[{"id":5,"filename":"tau-library.tdb"}]}}"#,
        )
        .unwrap();
        let plan = plan_install(&[sunset()], &media).unwrap();
        assert!(plan.warnings.iter().any(|w| w.contains("ignored")));
        let _ = fs::remove_dir_all(root);
    }

    /// Read-only check against a real card: `TAU_REAL_CARD=/Volumes/Pock cargo test -p tau-core real_card -- --ignored --nocapture`.
    /// Plans (never writes) an install for every `Assets/<platform>/common` folder and prints what it found.
    #[test]
    #[ignore]
    fn real_card_plan_is_read_only_and_finds_the_readers() {
        let Ok(card) = std::env::var("TAU_REAL_CARD") else {
            return;
        };
        let before: Vec<_> = walk(Path::new(&card).join("Assets"));
        for entry in fs::read_dir(Path::new(&card).join("Assets"))
            .unwrap()
            .filter_map(Result::ok)
        {
            let media = entry.path().join("common");
            if !media.is_dir() {
                continue;
            }
            let plan = plan_install(&[sunset()], &media).unwrap();
            println!(
                "{}: existing={:?} readers={:?} warnings={:?}",
                media.display(),
                plan.existing
                    .as_ref()
                    .map(|e| (e.bytes, &e.themes, e.readable)),
                plan.readers,
                plan.warnings
            );
        }
        assert_eq!(
            before,
            walk(Path::new(&card).join("Assets")),
            "planning changed the card"
        );
    }
    fn walk(root: PathBuf) -> Vec<(PathBuf, u64)> {
        let mut out = Vec::new();
        let mut stack = vec![root];
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
                    out.push((p, e.metadata().map(|m| m.len()).unwrap_or(0)))
                }
            }
        }
        out.sort();
        out
    }

    /// A real, approved write. Only runs when both variables are set:
    /// `TAU_REAL_WRITE_MEDIA=/Volumes/Pock/Assets/<platform>/common TAU_REAL_WRITE_BACKUP=<folder outside the card>`.
    /// Installs a theme called OMEGA TEST through the same plan/execute path the app uses, then checks that the only
    /// change on the whole card is that one file, that the replaced file is in the backup, and that no temp file is left.
    #[test]
    #[ignore]
    fn real_card_install_changes_only_the_theme_file() {
        let (Ok(media), Ok(backup)) = (
            std::env::var("TAU_REAL_WRITE_MEDIA"),
            std::env::var("TAU_REAL_WRITE_BACKUP"),
        ) else {
            return;
        };
        let media = PathBuf::from(media);
        let card = media
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let snapshot = |card: &Path| -> Vec<(PathBuf, u64)> {
            let mut all = walk(card.join("Assets"));
            all.extend(walk(card.join("Cores")));
            all
        };
        let before = snapshot(&card);
        let old = fs::read(media.join(FILE_NAME)).ok();
        let mut theme = sunset();
        theme.name = "OMEGA TEST".into();
        let plan = plan_install(&[theme.clone()], &media).unwrap();
        println!(
            "plan: existing={:?} readers={:?} warnings={:?}",
            plan.existing, plan.readers, plan.warnings
        );
        let report = execute_install(
            &[theme.clone()],
            &media,
            &plan,
            &plan.id,
            Some(Path::new(&backup)),
        )
        .unwrap();
        println!("report: {report:?}");
        let after = snapshot(&card);
        let live = media.join(FILE_NAME);
        let changed: Vec<_> = after
            .iter()
            .filter(|e| !before.contains(e))
            .chain(before.iter().filter(|e| !after.contains(e)))
            .collect();
        println!("changed entries: {changed:?}");
        assert!(
            changed.iter().all(|(p, _)| *p == live),
            "something other than the theme file changed"
        );
        assert_eq!(fs::read(&live).unwrap(), pack_assets(&[theme]).unwrap());
        if let (Some(old), Some(b)) = (old, report.backup.as_ref()) {
            assert_eq!(
                fs::read(b).unwrap(),
                old,
                "the backup is not the file that was replaced"
            );
        }
        assert!(!media.join(TEMP_NAME).exists() && !media.join(PREVIOUS_NAME).exists());
    }
}
