//! The verbs that change a card (or a core's files), the command-line twins of the app's reviewed flows. All of them follow the
//! rule of the existing `sync` verb: run the `*-plan` form first (it writes nothing and prints a plan id), then run the write
//! form with `--confirm <plan-id> --yes`. Anything that replaces files also needs `--backup-dir <folder outside the card>`.
//! Plans and results are the engine's own structures, printed with `--json`.

use crate::{CliError, value, yes};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tau_core::{
    TauError, assets, changes, halcyon, install_exec, install_plan, refresh, release_check, remove,
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

fn first<'a>(a: &'a [String], what: &str) -> Result<&'a Path, CliError> {
    a.first()
        .map(|s| Path::new(s.as_str()))
        .ok_or_else(|| CliError::Usage(format!("missing {what}")))
}

fn need<'a>(a: &'a [String], key: &str, verb: &str) -> Result<&'a str, CliError> {
    value(a, key).ok_or_else(|| CliError::Usage(format!("{verb} needs {key}")))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, CliError> {
    let bytes = fs::read(path).map_err(TauError::from)?;
    serde_json::from_slice(&bytes).map_err(|e| CliError::Usage(format!("{path}: {e}")))
}

/// `--backup-dir`, which must be outside `card` (a backup on the card is not a backup).
fn backup_dir(a: &[String], card: &Path, verb: &str) -> Result<PathBuf, CliError> {
    let dir = PathBuf::from(need(a, "--backup-dir", verb)?);
    let card = card.canonicalize().map_err(TauError::from)?;
    if tau_core::sync::backup_is_inside(&dir, &card) {
        return Err(CliError::Usage(
            "--backup-dir must be outside the card".into(),
        ));
    }
    Ok(dir)
}

fn docs_for(zip: &Path) -> Vec<tau_core::compat::CompatDoc> {
    release_check::sibling_manifest(zip).into_iter().collect()
}

// ---- remove a core ----

/// `tau remove-plan <card> --core <id> [--delete-media]`: what removing a core would delete. By default the library, music, covers and
/// theme file are kept. Writes nothing.
pub fn remove_plan(a: &[String], j: bool) -> Result<(), CliError> {
    let card = tau_core::inspect_card(first(a, "a card root")?)?;
    let plan = remove::plan_remove_with(
        &card,
        need(a, "--core", "remove-plan")?,
        !a.iter().any(|x| x == "--delete-media"),
        &[],
    )?;
    emit(j, &plan, || {
        format!(
            "plan {}: {} files, {} bytes{}",
            plan.id,
            plan.files_to_remove,
            plan.bytes_to_remove,
            if plan.media_kept { " (media kept)" } else { "" }
        )
    })
}

/// `tau remove <card> --core <id> [--delete-media] --confirm <plan-id> --yes`.
pub fn remove_core(a: &[String], j: bool) -> Result<(), CliError> {
    yes(a)?;
    let card = tau_core::inspect_card(first(a, "a card root")?)?;
    let plan = remove::plan_remove_with(
        &card,
        need(a, "--core", "remove")?,
        !a.iter().any(|x| x == "--delete-media"),
        &[],
    )?;
    let r = remove::execute_remove(&plan, need(a, "--confirm", "remove")?)?;
    emit(j, &r, || format!("removed: {r:?}"))
}

// ---- update a core from a zip ----

/// `tau update-plan <zip> --card <root> [--allow-downgrade]`: the reviewed update plan (what is new, replaced, kept, refused). Writes nothing.
pub fn update_plan(a: &[String], j: bool) -> Result<(), CliError> {
    let zip = first(a, "a package zip")?;
    let card = Path::new(need(a, "--card", "update-plan")?);
    let plan = install_plan::plan(
        zip,
        card,
        &docs_for(zip),
        a.iter().any(|x| x == "--allow-downgrade"),
    )?;
    emit(j, &plan, || {
        format!(
            "plan {}: {} new, {} replaced",
            plan.id, plan.files.new_files, plan.files.updated_files
        )
    })
}

/// `tau update <zip> --card <root> --confirm <plan-id> --backup-dir <outside> --yes [--allow-downgrade]`: backed-up install with a post-install check;
/// rolls back by itself on a failed step.
pub fn update_core(a: &[String], j: bool) -> Result<(), CliError> {
    yes(a)?;
    let zip = first(a, "a package zip")?;
    let card = Path::new(need(a, "--card", "update")?);
    let docs = docs_for(zip);
    let plan = install_plan::plan(zip, card, &docs, a.iter().any(|x| x == "--allow-downgrade"))?;
    let backup = backup_dir(a, card, "update")?;
    let r = install_exec::execute(
        zip,
        card,
        &plan,
        need(a, "--confirm", "update")?,
        &backup,
        &docs,
    )?;
    emit(j, &r, || format!("updated: {r:?}"))
}

// ---- library refresh ----

/// `tau library-refresh <media-root> --confirm <plan-id> --backup-dir <outside> --yes`: runs a reviewed refresh (see `library-refresh-plan`).
pub fn library_refresh(a: &[String], j: bool) -> Result<(), CliError> {
    yes(a)?;
    let root = first(a, "a media root")?;
    let plan = refresh::plan_refresh(root)?;
    let backup = backup_dir(a, root, "library-refresh")?;
    let r = refresh::execute_refresh(
        root,
        &plan,
        need(a, "--confirm", "library-refresh")?,
        &backup,
    )?;
    emit(j, &r, || format!("refreshed: {r:?}"))
}

// ---- workbench: add / remove / edit albums ----

fn request(a: &[String]) -> Result<changes::ChangeRequest, CliError> {
    let mut r: changes::ChangeRequest = match value(a, "--request") {
        Some(path) => read_json(path)?,
        None => changes::ChangeRequest::default(),
    };
    if let Some(l) = value(a, "--library") {
        r.library_root = Some(PathBuf::from(l));
    }
    for (i, x) in a.iter().enumerate() {
        match x.as_str() {
            "--add" => r.add_albums.extend(a.get(i + 1).cloned()),
            "--remove" => r.remove_albums.extend(a.get(i + 1).cloned()),
            _ => {}
        }
    }
    Ok(r)
}

/// `tau changes-plan <media-root> [--request request.json] [--library <dir>] [--add <album-id>]... [--remove <album-id>]...`: the combined add/remove/tag-edit
/// plan the app's workbench builds (the request file is a serialised `ChangeRequest`, which also carries tag and cover edits). Writes nothing.
pub fn changes_plan(a: &[String], j: bool) -> Result<(), CliError> {
    let plan = changes::plan_changes(first(a, "a media root")?, &request(a)?, &mut None)?;
    emit(j, &plan, || {
        format!(
            "plan {}: {} bytes to write, {} to remove, {} files",
            plan.id, plan.bytes_to_write, plan.bytes_to_remove, plan.files_total
        )
    })
}

/// `tau changes <media-root> <same options> --confirm <plan-id> --manifest <journal.json> [--backup-dir <outside>] --yes`.
pub fn changes_apply(a: &[String], j: bool) -> Result<(), CliError> {
    yes(a)?;
    let root = first(a, "a media root")?;
    let plan = changes::plan_changes(root, &request(a)?, &mut None)?;
    let backup = match value(a, "--backup-dir") {
        Some(_) => Some(backup_dir(a, root, "changes")?),
        None => None,
    };
    let journal = Path::new(need(a, "--manifest", "changes")?);
    let r = tau_core::journal::execute_changes_to_journal(
        &plan,
        need(a, "--confirm", "changes")?,
        backup.as_deref(),
        journal,
        None,
        &mut None,
    )?;
    emit(j, &r, || format!("applied: {r:?}"))
}

// ---- themes and Halcyon presets (tau-assets.bin) ----

/// `tau theme-check <theme.json>`: the readability report for one theme (the file is a serialised `ThemeInput`).
pub fn theme_check(a: &[String], j: bool) -> Result<(), CliError> {
    let t: assets::ThemeInput = read_json(&first(a, "a theme file")?.to_string_lossy())?;
    let r = assets::check_theme(&t);
    emit(j, &r, || {
        if r.problems.is_empty() {
            "passes every readability check".into()
        } else {
            r.problems.join("\n")
        }
    })
}

/// `tau theme-show <tau-assets.bin>`: the themes in an assets file.
pub fn theme_show(a: &[String], j: bool) -> Result<(), CliError> {
    let themes =
        assets::parse_assets(&fs::read(first(a, "a tau-assets.bin")?).map_err(TauError::from)?)?;
    emit(j, &themes, || {
        themes
            .iter()
            .map(|t| t.name.clone())
            .collect::<Vec<_>>()
            .join("\n")
    })
}

struct Edit {
    themes: Option<Vec<assets::ThemeInput>>,
    presets: Option<Vec<halcyon::HalcyonPreset>>,
}

fn edit(a: &[String], verb: &str) -> Result<Edit, CliError> {
    let themes = value(a, "--themes").map(read_json).transpose()?;
    let presets = value(a, "--presets").map(read_json).transpose()?;
    if themes.is_none() && presets.is_none() {
        return Err(CliError::Usage(format!(
            "{verb} needs --themes themes.json and/or --presets presets.json (an empty presets list removes the section)"
        )));
    }
    Ok(Edit { themes, presets })
}

impl Edit {
    fn as_edit(&self) -> assets::AssetsEdit<'_> {
        assets::AssetsEdit {
            themes: self.themes.as_deref(),
            presets: self.presets.as_deref(),
        }
    }
}

/// `tau assets-plan <media-root> [--themes themes.json] [--presets presets.json]`: plans replacing the themes and/or Halcyon presets in
/// `<media-root>/tau-assets.bin`, keeping every other section. The files are JSON lists of `ThemeInput` / `HalcyonPreset`. Writes nothing.
pub fn assets_plan(a: &[String], j: bool) -> Result<(), CliError> {
    let e = edit(a, "assets-plan")?;
    let plan = assets::plan_install_edit(e.as_edit(), first(a, "a media root")?)?;
    emit(j, &plan, || {
        format!(
            "plan {}: {} bytes, themes {:?}, presets {:?}",
            plan.id, plan.bytes, plan.themes, plan.presets
        )
    })
}

/// `tau assets-install <media-root> <same options> --confirm <plan-id> [--backup-dir <outside>] --yes`.
pub fn assets_install(a: &[String], j: bool) -> Result<(), CliError> {
    yes(a)?;
    let root = first(a, "a media root")?;
    let e = edit(a, "assets-install")?;
    let plan = assets::plan_install_edit(e.as_edit(), root)?;
    let backup = match value(a, "--backup-dir") {
        Some(_) => {
            let card = root.ancestors().nth(3).unwrap_or(root);
            Some(backup_dir(a, card, "assets-install")?)
        }
        None => None,
    };
    let r = assets::execute_install_edit(
        e.as_edit(),
        root,
        &plan,
        need(a, "--confirm", "assets-install")?,
        backup.as_deref(),
    )?;
    emit(j, &r, || {
        format!(
            "installed {} bytes to {}",
            r.bytes_written,
            r.destination.display()
        )
    })
}
