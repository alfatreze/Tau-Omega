//! The Cores section from the command line (same engine as the app): the platform tree, one core's details, files, saved settings and
//! screenshots, and adding files (plan first, then the same command with `--confirm <plan-id> --yes`). Reads never write; `core-add`
//! is the only verb that does, and it refuses without the plan's token.

use crate::{CliError, value, yes};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tau_core::{
    core_add::{self, AddState, Destination},
    core_files, core_pack, core_preview, core_remove, core_screens, core_settings, corehub,
    manual_source::{self, Source},
};

fn emit<T: Serialize>(j: bool, data: &T, text: impl FnOnce() -> String) -> Result<(), CliError> {
    if j {
        println!(
            "{}",
            serde_json::to_string_pretty(data).map_err(|e| CliError::Usage(e.to_string()))?
        );
    } else {
        println!("{}", text());
    }
    Ok(())
}

fn need<'a>(a: &'a [String], key: &str, verb: &str) -> Result<&'a str, CliError> {
    value(a, key).ok_or_else(|| CliError::Usage(format!("{verb} needs {key}")))
}

fn size(b: u64) -> String {
    match b {
        0..=1023 => format!("{b} B"),
        1024..=1_048_575 => format!("{:.0} KB", b as f64 / 1024.0),
        1_048_576..=1_073_741_823 => format!("{:.1} MB", b as f64 / 1_048_576.0),
        _ => format!("{:.2} GB", b as f64 / 1_073_741_824.0),
    }
}

/// `tau cores <card-root>`: every core on the card grouped by platform.
pub fn cores(a: &[String], j: bool) -> Result<(), CliError> {
    let card = a.first().ok_or("cores needs a card root")?;
    let tree = corehub::platform_tree(Path::new(card))?;
    emit(j, &tree, || {
        // The Library's grouping: category > manufacturer > platform > core (the tree is already in that order).
        let mut s = String::new();
        let (mut cat, mut maker) = (String::new(), String::new());
        for n in &tree {
            if n.platform.category != cat {
                cat = n.platform.category.clone();
                maker.clear();
                s += &format!("{cat}\n");
            }
            if n.platform.manufacturer != maker {
                maker = n.platform.manufacturer.clone();
                s += &format!("  {maker}\n");
            }
            s += &format!(
                "    {} ({}, {} core{})\n",
                n.platform.name,
                n.platform.id,
                n.cores.len(),
                if n.cores.len() == 1 { "" } else { "s" }
            );
            for c in &n.cores {
                s += &format!("      {:<34} {:<10} {}\n", c.id, c.version, c.shortname);
            }
        }
        s
    })
}

/// `tau core-detail <card-root> <core-id>`
pub fn core_detail(a: &[String], j: bool) -> Result<(), CliError> {
    let card = a.first().ok_or("core-detail needs a card root")?;
    let id = a.get(1).ok_or("core-detail needs a core id")?;
    let d = corehub::core_detail(Path::new(card), id)?;
    emit(j, &d, || {
        let yn = |v: Option<bool>| match v {
            Some(true) => "yes",
            Some(false) => "no",
            None => "not stated",
        };
        let mut s = format!(
            "{} {} ({})\n  {}\n  author {} · firmware {} · Dock {} · link port {} · sleep {}\n  platforms: {}\n  slots:\n",
            d.id,
            d.version,
            d.date_release,
            d.description,
            d.author,
            if d.firmware_required.is_empty() {
                "not stated"
            } else {
                &d.firmware_required
            },
            yn(d.dock_supported),
            yn(d.link_port),
            yn(d.sleep_supported),
            d.platforms
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        for sl in &d.slots {
            s += &format!(
                "    {:<14} {}{}{} [{}]{}\n",
                sl.name,
                if sl.required { "required " } else { "" },
                if sl.nonvolatile { "saves " } else { "" },
                if sl.filename.is_empty() {
                    sl.extensions
                        .iter()
                        .map(|e| format!(".{e}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    sl.filename.clone()
                },
                if sl.core_specific {
                    "this core only"
                } else {
                    "shared"
                },
                sl.size_exact
                    .map_or(String::new(), |n| format!(" exactly {}", size(n)))
            );
        }
        s
    })
}

/// `tau core-files <card-root> <core-id>`: the core's folders with their files, and what is missing.
pub fn core_files(a: &[String], j: bool) -> Result<(), CliError> {
    let card = a.first().ok_or("core-files needs a card root")?;
    let id = a.get(1).ok_or("core-files needs a core id")?;
    let f = core_files::core_files(Path::new(card), id)?;
    emit(j, &f, || {
        let mut s = String::new();
        for l in &f.folders {
            s += &format!(
                "{} ({}){}\n  {} file(s), {}{}\n",
                l.folder.label,
                l.folder.rel_path,
                if l.exists { "" } else { " [not created yet]" },
                l.total_files,
                size(l.total_bytes),
                if l.truncated { ", list cut" } else { "" }
            );
            for e in l.entries.iter().take(12) {
                s += &format!(
                    "    {:<60} {:>9} {:?}{}\n",
                    e.rel_path,
                    size(e.bytes),
                    e.kind,
                    e.slot.as_ref().map_or(String::new(), |s| format!(" ({s})"))
                );
            }
            if l.entries.len() > 12 {
                s += &format!("    … and {} more\n", l.entries.len() - 12);
            }
        }
        for m in &f.missing {
            s += &format!(
                "MISSING {} ({}): put it in {}\n",
                m.filename, m.slot, m.folder_rel
            );
        }
        if !f.save_backups.is_empty() {
            s += &format!(
                "{} Pocket Sync save backup zip(s) in Saves/ (not counted as saves)\n",
                f.save_backups.len()
            );
        }
        s
    })
}

/// `tau core-settings <card-root> <core-id>`
pub fn core_settings(a: &[String], j: bool) -> Result<(), CliError> {
    let card = a.first().ok_or("core-settings needs a card root")?;
    let id = a.get(1).ok_or("core-settings needs a core id")?;
    let st = core_settings::core_settings(Path::new(card), id)?;
    emit(j, &st, || {
        let mut s = String::new();
        for v in &st.values {
            s += &format!(
                "{:<28} {:<14} (default {})\n",
                v.name, v.display, v.default_display
            );
        }
        s += &format!(
            "{} saved file(s), {} preset(s) in {}\n",
            st.files.len(),
            st.presets,
            st.presets_folder
        );
        s
    })
}

/// `tau core-screens <card-root> <core-id>`: screenshots, with the ones taken near this core's save states marked.
pub fn core_screens(a: &[String], j: bool) -> Result<(), CliError> {
    let card = a.first().ok_or("core-screens needs a card root")?;
    let id = a.get(1).ok_or("core-screens needs a core id")?;
    let v = core_screens::screenshots(Path::new(card), id)?;
    emit(j, &v, || {
        format!(
            "{} screenshot(s) on the card: {} name this core, {} older file(s) without a recorded core are probably this core (taken within {} minutes of one of its {} save state(s))",
            v.total,
            v.recorded,
            v.likely,
            v.window_secs / 60,
            v.states
        )
    })
}

fn rels(a: &[String]) -> Vec<String> {
    sources(a)
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect()
}

/// `tau core-remove-plan <card-root> <core-id> <card-relative file paths...>`
pub fn core_remove_plan(a: &[String], j: bool) -> Result<(), CliError> {
    let card = a.first().ok_or("core-remove-plan needs a card root")?;
    let id = a.get(1).ok_or("core-remove-plan needs a core id")?;
    let p = core_remove::plan_remove_files(Path::new(card), id, &rels(a))?;
    emit(j, &p, || {
        let mut s = format!(
            "{} file(s), {} to remove (backed up outside the card first); plan id {}",
            p.items.len(),
            size(p.total_bytes),
            p.id
        );
        for w in &p.warnings {
            s.push_str(&format!("\nnote: {w}"));
        }
        if let Some(b) = &p.blocked {
            s.push_str(&format!("\nblocked: {b}"));
        }
        s
    })
}

/// `tau core-remove <card-root> <core-id> <card-relative file paths...> --confirm <plan-id> --backup-dir <outside the card> --yes`
pub fn core_remove(a: &[String], j: bool) -> Result<(), CliError> {
    yes(a)?;
    let card = a.first().ok_or("core-remove needs a card root")?;
    let id = a.get(1).ok_or("core-remove needs a core id")?;
    let confirm = need(a, "--confirm", "core-remove")?;
    let backup = need(a, "--backup-dir", "core-remove")?;
    let r = core_remove::execute_remove_files(
        Path::new(card),
        id,
        &rels(a),
        confirm,
        Path::new(backup),
    )?;
    emit(j, &r, || {
        format!(
            "removed {} file(s), {}; copies kept in {}",
            r.removed,
            size(r.bytes_removed),
            r.backup_dir.display()
        )
    })
}

fn destination(a: &[String]) -> Result<Destination, CliError> {
    match value(a, "--to") {
        Some("shared") | None => Ok(Destination::Shared),
        Some("core") => Ok(Destination::CoreOnly),
        Some(_) => Err("--to is shared or core".into()),
    }
}

fn sources(a: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut skip = false;
    for arg in a.iter().skip(2) {
        if skip {
            skip = false;
        } else if matches!(
            arg.as_str(),
            "--platform" | "--to" | "--confirm" | "--backup-dir"
        ) {
            skip = true;
        } else if !arg.starts_with("--") {
            out.push(PathBuf::from(arg));
        }
    }
    out
}

fn target(a: &[String], card: &str, id: &str) -> Result<String, CliError> {
    match value(a, "--platform") {
        Some(p) => Ok(p.to_string()),
        None => Ok(corehub::core_detail(Path::new(card), id)?
            .platforms
            .first()
            .map(|p| p.id.clone())
            .ok_or("this core names no platform; pass --platform <id>")?),
    }
}

/// `tau core-add-plan <card-root> <core-id> <files or folders...> [--to shared|core] [--platform id] [--replace]`
pub fn core_add_plan(a: &[String], j: bool) -> Result<(), CliError> {
    let card = a.first().ok_or("core-add-plan needs a card root")?;
    let id = a.get(1).ok_or("core-add-plan needs a core id")?;
    let platform = target(a, card, id)?;
    let replace = a.iter().any(|x| x == "--replace");
    let p = core_add::plan_add(
        Path::new(card),
        id,
        &platform,
        destination(a)?,
        &sources(a),
        replace,
    )?;
    emit(j, &p, || {
        let mut s = format!(
            "plan {}\n  into {}\n  {} new · {} identical · {} different{} · {} refused · {} to write\n",
            p.id,
            p.folder_rel,
            p.new_files,
            p.identical,
            p.different,
            if p.replace {
                " (will be replaced)"
            } else {
                " (kept as they are)"
            },
            p.refused,
            size(p.bytes_to_write)
        );
        for i in p.items.iter().take(20) {
            s += &format!(
                "    {:<64} {:<10} {}\n",
                i.dest_rel,
                size(i.bytes),
                match &i.state {
                    AddState::New => "new".to_string(),
                    AddState::Identical => "identical".into(),
                    AddState::Different => "different".into(),
                    AddState::Refused(w) => format!("REFUSED: {w}"),
                }
            );
        }
        for w in &p.warnings {
            s += &format!("  note: {w}\n");
        }
        if let Some(b) = &p.blocked {
            s += &format!("  cannot run: {b}\n");
        } else {
            s += &format!(
                "To add them: tau core-add {card} {id} … --confirm {} --yes\n",
                p.id
            );
        }
        s
    })
}

/// `tau core-add <card-root> <core-id> <files or folders...> --confirm <plan-id> --yes [--to ...] [--platform id] [--replace --backup-dir <outside the card>]`
pub fn core_add(a: &[String], j: bool) -> Result<(), CliError> {
    yes(a)?;
    let card = a.first().ok_or("core-add needs a card root")?;
    let id = a.get(1).ok_or("core-add needs a core id")?;
    let platform = target(a, card, id)?;
    let replace = a.iter().any(|x| x == "--replace");
    let confirm = need(a, "--confirm", "core-add")?;
    let backup = value(a, "--backup-dir").map(PathBuf::from);
    let r = core_add::execute_add(
        Path::new(card),
        id,
        &platform,
        destination(a)?,
        &sources(a),
        replace,
        confirm,
        backup.as_deref(),
    )?;
    emit(j, &r, || {
        format!(
            "added {} file(s), replaced {}, skipped {} identical and {} different, {}{}",
            r.written,
            r.replaced,
            r.skipped_identical,
            r.skipped_different,
            size(r.bytes_written),
            r.backup_dir.as_ref().map_or(String::new(), |d| format!(
                "; replaced files backed up in {}",
                d.display()
            ))
        )
    })
}

/// `tau core-source <zip | folder | github-address> [--json]`: what a core that is not in the Library says about itself (the preview of a manual add).
/// A folder is packed into a temporary zip first; a GitHub address is validated here and fetched in a later step (nothing is downloaded yet).
pub fn core_source(a: &[String], j: bool) -> Result<(), CliError> {
    let input = a
        .first()
        .ok_or("core-source needs a zip, a folder or a GitHub address")?;
    match manual_source::classify(input)? {
        Source::GitHub(g) => emit(j, &g, || {
            format!("a GitHub address, valid: {g:?}\n(fetching it is not built yet)")
        }),
        Source::Zip(z) => show_preview(&z, None, j),
        Source::Folder(f) => {
            let out = std::env::temp_dir().join(format!("tau-core-source-{}", std::process::id()));
            let packed =
                core_pack::pack_folder(&f, &out, tau_core::core_package::Limits::default())?;
            show_preview(&packed.zip, Some(&packed), j)
        }
    }
}

fn show_preview(zip: &Path, packed: Option<&core_pack::Packed>, j: bool) -> Result<(), CliError> {
    let p = core_preview::preview_zip(zip, tau_core::core_package::Limits::default())?;
    emit(j, &p, || {
        let mut s = format!(
            "{} {} ({})\n{}\nfirmware {} · dock {:?} · {} files, {} · sha256 {}\n",
            p.core_id,
            p.version,
            p.date_release,
            p.description,
            if p.firmware_required.is_empty() {
                "not stated"
            } else {
                &p.firmware_required
            },
            p.dock_supported,
            p.files,
            size(p.uncompressed_bytes),
            p.sha256
        );
        if let Some(pk) = packed {
            s += &format!(
                "(packed from a {:?} folder{})\n",
                pk.layout,
                pk.descended_into
                    .as_ref()
                    .map_or(String::new(), |d| format!(", inside {d}"))
            );
        }
        for pl in &p.platforms {
            s += &format!(
                "platform: {} > {} > {}{}\n",
                pl.category,
                pl.manufacturer,
                pl.name,
                if pl.in_package {
                    ""
                } else {
                    " (no platform file in the package)"
                }
            );
        }
        for n in &p.needs {
            s += &format!(
                "needs: {}{} -> {}{}\n",
                n.slot,
                if n.filename.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", n.filename)
                },
                n.folder_rel,
                if n.you_supply {
                    "  <- you supply this"
                } else if n.in_package {
                    "  (in the package)"
                } else {
                    ""
                }
            );
        }
        if !p.settings.is_empty() {
            s += &format!("settings: {}\n", p.settings.join(", "));
        }
        for w in &p.warnings {
            s += &format!("note: {w}\n");
        }
        s
    })
}
