//! `tau-assets.bin`, the optional theme file a Tau core reads (data slot 8).
//!
//! Layout and rules are the firmware's (`docs/features/THEME_FILE_FORMAT.md` in tau-alpha, written by
//! `tools/tau_assets.py`); this module is an independent Rust writer/reader checked byte for byte against a file that
//! reference tool produced (`testdata/assets/`). Only the `THEM` section is *edited* here. Every other section (`METR` meter
//! presets, `PRST` Halcyon user EQ presets, anything newer) is carried byte for byte, in its original place, whenever an
//! existing file is replaced or re-saved ([`pack_assets_keeping`]); the Tau release manifest marks the format
//! `preserve_unknown_sections` for exactly this reason. A file whose container version is newer than this reader is
//! refused rather than overwritten.
//!
//! The firmware trusts only the CRCs, so the *writer* owns the safety rules: both polarities, a name the device font
//! can draw, and the contrast rules the firmware's own theme generator enforces (text on every surface and on the
//! background ramp behind every accent the device offers). Colours are RGB565 on the device, so every colour is
//! snapped first and the report shows what will really appear.

use crate::halcyon::{self, HalcyonPreset};
use crate::{ErrorCode, TauError};
use std::collections::BTreeMap;

const MAGIC: &[u8; 4] = b"TAUA";
/// Section tag in the container table, and the magic inside the section (they differ in the firmware format).
const SECTION_THEM: &[u8; 4] = b"THEM";
const SECTION_PRST: &[u8; 4] = b"PRST";
const THEM_MAGIC: &[u8; 4] = b"TTHM";
const VERSION: u16 = 1;
const NAME_LEN: usize = 16;
/// Role count the firmware knows (`TR_COUNT` in `fw/theme.h`). The file carries all of them; roles 0, 6 and 8 are not themeable.
const ROLE_COUNT: usize = 21;
pub const MAX_THEMES: usize = 4;
/// The firmware's limits (`AS_MAX_SECTIONS`, `AS_MAX_FILE` in `fw/assets_core.h`): a bigger file is not read at all.
const MAX_SECTIONS: usize = 8;
pub const MAX_FILE_BYTES: usize = 65536;
/// Themes the firmware already has; a file may not reuse their names.
pub const BUILTIN_NAMES: [&str; 2] = ["TAU", "OCEAN"];
mod theme;
pub use theme::*;

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

/// The container: header, section table, then the sections back to back in table order (the reference
/// `tools/tau_assets.py pack_container`, byte for byte).
fn pack_container(sections: &[([u8; 4], &[u8])]) -> Vec<u8> {
    let mut table = Vec::new();
    let mut off = 12 + 16 * sections.len();
    for (tag, data) in sections {
        table.extend_from_slice(tag);
        table.extend_from_slice(&(off as u32).to_le_bytes());
        table.extend_from_slice(&(data.len() as u32).to_le_bytes());
        table.extend_from_slice(&crc(data).to_le_bytes());
        off += data.len();
    }
    let mut out = Vec::with_capacity(off);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(sections.len() as u16).to_le_bytes());
    out.extend_from_slice(&crc(&table).to_le_bytes());
    out.extend_from_slice(&table);
    for (_, data) in sections {
        out.extend_from_slice(data);
    }
    out
}

/// The whole `tau-assets.bin` for these themes alone (a container with one `THEM` section). Refuses any theme that
/// fails `check_theme`. Use [`pack_assets_keeping`] whenever a file is being replaced or re-saved.
pub fn pack_assets(themes: &[ThemeInput]) -> Result<Vec<u8>, TauError> {
    let them = pack_them(themes)?;
    Ok(pack_container(&[(*SECTION_THEM, &them)]))
}

/// True when `blob` is a Tau assets container of a version this reader does not know (made by a newer Tau).
fn is_newer_container(blob: &[u8]) -> bool {
    blob.len() >= 6 && &blob[..4] == MAGIC && u16le(blob, 4) != VERSION
}

/// The themes and presets an [`AssetsEdit`] reads back from a file (`None` where the edit leaves that part alone).
type EditContent = (Option<Vec<ThemeInput>>, Option<Vec<HalcyonPreset>>);

/// What to change in a `tau-assets.bin`. `None` leaves that part of the existing file exactly as it is.
#[derive(Debug, Clone, Copy, Default)]
pub struct AssetsEdit<'a> {
    /// Replace the `THEM` section with these themes.
    pub themes: Option<&'a [ThemeInput]>,
    /// Replace the `PRST` section with these Halcyon presets (an empty list removes the section).
    pub presets: Option<&'a [HalcyonPreset]>,
}

impl<'a> AssetsEdit<'a> {
    pub fn themes(themes: &'a [ThemeInput]) -> Self {
        Self {
            themes: Some(themes),
            presets: None,
        }
    }
    pub fn presets(presets: &'a [HalcyonPreset]) -> Self {
        Self {
            themes: None,
            presets: Some(presets),
        }
    }
    /// The edit that would reproduce `blob` from `old`: what the install reads back and checks.
    fn read_back(&self, blob: &[u8]) -> Result<EditContent, TauError> {
        let themes = if self.themes.is_some() {
            Some(parse_assets(blob)?)
        } else {
            None
        };
        let presets = if self.presets.is_some() {
            Some(read_presets(blob)?)
        } else {
            None
        };
        Ok((themes, presets))
    }
}

/// The Halcyon user presets in a `tau-assets.bin` (empty when it has no `PRST` section), every CRC checked first.
pub fn read_presets(blob: &[u8]) -> Result<Vec<HalcyonPreset>, TauError> {
    for (tag, data) in read_sections(blob)? {
        if &tag == SECTION_PRST {
            return halcyon::parse_presets(data);
        }
    }
    Ok(Vec::new())
}

/// The `tau-assets.bin` for these themes, keeping every other section of `existing` byte for byte and in place
/// (the `THEM` section is replaced where it was, or put first when there was none). `existing` that is absent or
/// damaged gives the themes-only file (a damaged file is backed up by the install, nothing in it can be carried).
/// Refuses an `existing` file of a newer container version (it may hold data this reader cannot keep) and a result
/// over the firmware's limits (8 sections, 64 KiB), which the device would not read at all.
pub fn pack_assets_keeping(
    themes: &[ThemeInput],
    existing: Option<&[u8]>,
) -> Result<Vec<u8>, TauError> {
    pack_assets_edit(AssetsEdit::themes(themes), existing)
}

/// Generalises [`pack_assets_keeping`]: replaces the `THEM` and/or `PRST` section as asked and carries every other
/// section byte for byte. A section that was not asked for is left exactly as it was; a requested one is replaced in
/// place, or added (`THEM` first, `PRST` last) when absent.
pub fn pack_assets_edit(edit: AssetsEdit, existing: Option<&[u8]>) -> Result<Vec<u8>, TauError> {
    let them = edit.themes.map(pack_them).transpose()?;
    let prst = match edit.presets {
        Some([]) => Some(None),
        Some(p) => Some(Some(halcyon::pack_presets(p)?)),
        None => None,
    };
    let fresh = |them: &Option<Vec<u8>>, prst: &Option<Option<Vec<u8>>>| {
        let mut out: Vec<([u8; 4], &[u8])> = Vec::new();
        if let Some(t) = them {
            out.push((*SECTION_THEM, t));
        }
        if let Some(Some(p)) = prst {
            out.push((*SECTION_PRST, p));
        }
        pack_container(&out)
    };
    let Some(old) = existing else {
        return Ok(fresh(&them, &prst));
    };
    if is_newer_container(old) {
        return Err(bad_file(format!(
            "The theme file already there was made by a newer Tau (assets version {}). Omega cannot keep what is in it, so it was not replaced. Update Omega first.",
            u16le(old, 4)
        )));
    }
    let Ok(sections) = read_sections(old) else {
        return Ok(fresh(&them, &prst));
    };
    let mut out: Vec<([u8; 4], &[u8])> = Vec::with_capacity(sections.len() + 2);
    if let Some(t) = &them
        && !sections.iter().any(|(tag, _)| tag == SECTION_THEM)
    {
        out.push((*SECTION_THEM, t));
    }
    for (tag, data) in &sections {
        if tag == SECTION_THEM {
            out.push((*tag, them.as_deref().unwrap_or(data)));
        } else if tag == SECTION_PRST {
            match &prst {
                Some(Some(p)) => out.push((*tag, p)),
                Some(None) => {}
                None => out.push((*tag, data)),
            }
        } else {
            out.push((*tag, *data));
        }
    }
    if let Some(Some(p)) = &prst
        && !sections.iter().any(|(tag, _)| tag == SECTION_PRST)
    {
        out.push((*SECTION_PRST, p));
    }
    if out.len() > MAX_SECTIONS {
        return Err(bad_file(format!(
            "The file would have {} sections; the Pocket reads at most {MAX_SECTIONS}.",
            out.len()
        )));
    }
    let blob = pack_container(&out);
    if blob.len() > MAX_FILE_BYTES {
        return Err(bad_file(format!(
            "The file would be {} bytes; the Pocket reads at most {MAX_FILE_BYTES}. Remove a theme or some presets.",
            blob.len()
        )));
    }
    Ok(blob)
}

/// The sections of a readable file that an edit of this kind would carry unchanged, as (tag, bytes), in file order.
fn carried_sections(blob: &[u8], edit: &AssetsEdit) -> Vec<([u8; 4], Vec<u8>)> {
    read_sections(blob)
        .map(|s| {
            s.into_iter()
                .filter(|(t, _)| {
                    !(t == SECTION_THEM && edit.themes.is_some())
                        && !(t == SECTION_PRST && edit.presets.is_some())
                })
                .map(|(t, d)| (t, d.to_vec()))
                .collect()
        })
        .unwrap_or_default()
}

/// The non-`THEM` sections of a readable file, as (tag, bytes), in file order (empty for anything unreadable).
pub fn kept_sections(blob: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
    read_sections(blob)
        .map(|s| {
            s.into_iter()
                .filter(|(t, _)| t != SECTION_THEM)
                .map(|(t, d)| (t, d.to_vec()))
                .collect()
        })
        .unwrap_or_default()
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

mod install;
pub use install::*;

#[cfg(test)]
mod tests;
