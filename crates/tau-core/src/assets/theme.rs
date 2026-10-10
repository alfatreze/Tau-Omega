//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

pub(super) const LIGHT_ACC_MAX_L: i32 = 110;

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
pub(super) const PALETTE: [u16; 19] = [
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

pub(super) fn invalid(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidTheme, msg)
}
pub(super) fn bad_file(msg: impl Into<String>) -> TauError {
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

pub(super) fn rgb8(c: u16) -> (i32, i32, i32) {
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

pub(super) fn pack565(r: i32, g: i32, b: i32) -> u16 {
    ((((r * 31 + 127) / 255) << 11) | (((g * 63 + 127) / 255) << 5) | ((b * 31 + 127) / 255)) as u16
}

pub(super) fn luminance(c: u16) -> f64 {
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

pub(super) fn contrast(a: u16, b: u16) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Mirror of `th_accent_of()`: on Light, an accent brighter than the cap is scaled down to it.
pub(super) fn acc_eff(a: u16, light: bool) -> u16 {
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
pub(super) fn grad_top(accent: u16, luma: i32) -> u16 {
    let (r, g, b) = rgb8(accent);
    let l = ((2126 * r + 7152 * g + 722 * b) / 10000).max(1);
    let scale = |v: i32| (v * luma + l / 2) / l;
    let mix = |v: i32| ((v + luma + 1) / 2).min(255);
    pack565(mix(scale(r)), mix(scale(g)), mix(scale(b)))
}

/// The firmware's per-channel blend (`ui_mix`); the division floors, as the reference does.
pub(super) fn mix565(a: u16, b: u16, t: i32, n: i32) -> u16 {
    let (a, b) = (a as i32, b as i32);
    let ch = |x: i32, y: i32| x + ((y - x) * t).div_euclid(n);
    ((ch(a >> 11, b >> 11) << 11)
        | (ch((a >> 5) & 0x3F, (b >> 5) & 0x3F) << 5)
        | ch(a & 0x1F, b & 0x1F)) as u16
}

/// Contrast rules: text role, backgrounds ("ramp" = the background ramp behind every device accent), minimum ratio.
pub(super) const RULES: [(&str, &[&str], f64); 3] = [
    ("text_primary", &["surface", "base", "ramp"], 4.5),
    ("text_secondary", &["surface", "base"], 3.0),
    ("text_secondary", &["ramp"], 2.6),
];

pub(super) fn read_polarity(
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
