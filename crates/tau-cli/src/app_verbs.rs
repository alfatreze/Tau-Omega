//! The verbs that give the command line the same reach as the app (roadmap item 5, "`tau-cli` parity with `--json` everywhere"):
//! read a screenshot's report code, list screenshots, read saved settings, inspect and plan packages, library health, Halcyon presets.
//! Each prints a short summary, or with `--json` the engine's own serialised result (the same shape the app's front end receives).
//! Writes need `--yes` and the plan id from a plan run, like the existing sync verbs.

use crate::{CliError, value};
use serde::Serialize;
use std::{fs, path::Path};
use tau_core::TauError;

/// Prints `data` as pretty JSON when `j`, else the one-line `text`.
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

fn arg<'a>(a: &'a [String], what: &str) -> Result<&'a str, CliError> {
    a.first()
        .map(String::as_str)
        .ok_or_else(|| CliError::Usage(format!("missing {what}")))
}

/// `tau report-code <screenshot.png>`: the report in a Pocket screenshot, whichever view (pixel grid or QR) it shows.
pub fn report_code(a: &[String], j: bool) -> Result<(), CliError> {
    let r = tau_core::taud::read_screenshot_report(arg(a, "a screenshot file")?)?;
    emit(j, &r, || {
        format!(
            "{} · {} · {} tests{}",
            r.profile,
            r.verdict,
            r.tests.len(),
            r.entries
                .build
                .as_ref()
                .map(|b| format!(" · firmware {} bitstream {}", b.firmware, b.bitstream))
                .unwrap_or_default()
        )
    })
}

/// `tau screenshots <card-root>`: the Pocket screenshots on a card, newest first.
pub fn screenshots(a: &[String], j: bool) -> Result<(), CliError> {
    let list = tau_core::screenshots::list_screenshots(Path::new(arg(a, "a card root")?))?;
    emit(j, &list, || {
        list.iter()
            .map(|s| s.filename.clone())
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// `tau settings <interact_persist.json>`: the saved settings of a core (and the Check summary when they hold one).
pub fn settings(a: &[String], j: bool) -> Result<(), CliError> {
    #[derive(Serialize)]
    struct Out {
        settings: Vec<tau_core::diag::PersistedSetting>,
        check_summary: Option<tau_core::diag::CheckSummary>,
    }
    let path = arg(a, "an interact_persist.json file")?;
    let settings = tau_core::diag::read_persisted_settings(path)?;
    let check_summary = match tau_core::diag::read_check_summary(path) {
        Ok(s) => Some(s),
        Err(e) if e.code() == tau_core::ErrorCode::NotACheckSummary => None,
        Err(e) => return Err(e.into()),
    };
    let n = settings.len();
    emit(
        j,
        &Out {
            settings,
            check_summary,
        },
        || format!("{n} saved settings"),
    )
}

/// `tau package-inspect <zip>`.
pub fn package_inspect(a: &[String], j: bool) -> Result<(), CliError> {
    let m = tau_core::package::inspect(Path::new(arg(a, "a package zip")?))?;
    emit(j, &m, || {
        format!("{m:#?}")
            .lines()
            .next()
            .unwrap_or_default()
            .to_string()
    })
}

/// `tau package-plan <zip> --card <card-root>`: what installing the package would write. Writes nothing.
pub fn package_plan(a: &[String], j: bool) -> Result<(), CliError> {
    let zip = Path::new(arg(a, "a package zip")?);
    let card = Path::new(value(a, "--card").ok_or("package-plan needs --card CARD_ROOT")?);
    let p = tau_core::package::plan_install(zip, card)?;
    emit(j, &p, || {
        format!(
            "plan {}: {} new, {} updated, {} unchanged, {} bytes",
            p.id, p.new_files, p.updated_files, p.unchanged_files, p.bytes_to_write
        )
    })
}

/// `tau package-install <zip> --card <card-root> --confirm <plan-id> --yes`: writes the package (hash-checked, as the app does).
pub fn package_install(a: &[String], j: bool) -> Result<(), CliError> {
    crate::yes(a)?;
    let zip = Path::new(arg(a, "a package zip")?);
    let card = Path::new(value(a, "--card").ok_or("package-install needs --card CARD_ROOT")?);
    let token = value(a, "--confirm")
        .ok_or("package-install needs --confirm PLAN_ID (from package-plan)")?;
    let plan = tau_core::package::plan_install(zip, card)?;
    let r = tau_core::package::execute_install(zip, card, &plan, token)?;
    emit(j, &r, || format!("installed: {r:?}"))
}

/// `tau library-health <media-root>`: whether the core's library index is present and current.
pub fn library_health(a: &[String], j: bool) -> Result<(), CliError> {
    let s = tau_core::refresh::index_state(Path::new(arg(a, "a media root")?));
    emit(j, &s, || format!("{s:?}"))
}

/// `tau library-refresh-plan <media-root>`: what a refresh would fix and rebuild. Reads tags; writes nothing.
pub fn library_refresh_plan(a: &[String], j: bool) -> Result<(), CliError> {
    let p = tau_core::refresh::plan_refresh(Path::new(arg(a, "a media root")?))?;
    emit(j, &p, || format!("{p:?}").chars().take(200).collect())
}

/// `tau halcyon-import <profile.txt> --name NAME`: imports an Equalizer APO / AutoEQ profile and reports the filters, the preamp and the peak boost.
pub fn halcyon_import(a: &[String], j: bool) -> Result<(), CliError> {
    let text = fs::read_to_string(arg(a, "a profile text file")?).map_err(TauError::from)?;
    let name = value(a, "--name")
        .ok_or("halcyon-import needs --name NAME (1-15 characters of A-Z 0-9 space _ -)")?;
    let r = tau_core::halcyon::import_apo(name, &text)?;
    emit(j, &r, || {
        format!(
            "{} filters, volume {} dB for headroom (loudest boost {} dB)",
            r.filters, r.preamp_db, r.peak_gain_db
        )
    })
}

/// `tau halcyon-show <tau-assets.bin>`: the Halcyon presets in a theme/assets file.
pub fn halcyon_show(a: &[String], j: bool) -> Result<(), CliError> {
    let presets = tau_core::assets::read_presets(
        &fs::read(arg(a, "a tau-assets.bin")?).map_err(TauError::from)?,
    )?;
    emit(j, &presets, || {
        presets
            .iter()
            .map(|p| p.name().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })
}
