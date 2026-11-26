//! Tauri commands for composing `tau-meters.bin` and installing `.tmeter` packages (`tau_core::meter_package`, `meter_install`).
//! Thin wrapper: all validation, composition, backup verification and atomic swap live in `tau-core`.

use crate::{ErrorCode, TauError, begin_card_write, data_dir, install_backup_root};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use tau_core::{
    meter_install::{self, MeterAction, MeterBinPlan, MeterBinReport, RowEdit},
    meter_package::{self, PackageSummary},
};

const AUTHORS_FILE: &str = "meter_package_authors.json";

type AuthorBook = BTreeMap<String, BTreeMap<String, String>>;

fn bad(msg: impl Into<String>) -> TauError {
    TauError {
        code: ErrorCode::InvalidPathReference,
        message: msg.into(),
    }
}

fn authors_path(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    Ok(data_dir(app)?.join(AUTHORS_FILE))
}

fn read_authors(app: &tauri::AppHandle) -> AuthorBook {
    authors_path(app)
        .ok()
        .and_then(|p| fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_authors(app: &tauri::AppHandle, book: &AuthorBook) {
    if let Ok(path) = authors_path(app) {
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(book) {
            let _ = fs::write(path, bytes);
        }
    }
}

fn card_key(card: &str) -> String {
    tau_core::ledger_host::card_key(Path::new(card))
        .unwrap_or_else(|| format!("path-{}", tau_core::ledger_host::library_key(Path::new(card))))
}

fn read_package_bytes(path: &str) -> Result<Vec<u8>, TauError> {
    let p = Path::new(path);
    let meta = fs::metadata(p).map_err(|e| bad(format!("Could not read meter package: {e}")))?;
    if meta.len() as usize > meter_package::MAX_PACKAGE_BYTES {
        return Err(bad(format!(
            "The meter package is too large ({} bytes; the limit is 4 MiB).",
            meta.len()
        )));
    }
    fs::read(p).map_err(|e| bad(format!("Could not read meter package: {e}")))
}

/// Inspects and vets a `.tmeter` package file on disk. Reads only.
#[tauri::command(async)]
pub fn meter_package_inspect(path: String) -> Result<PackageSummary, TauError> {
    let bytes = read_package_bytes(&path)?;
    Ok(meter_package::parse(&bytes)?.summary)
}

/// Plans reordering, hiding/showing, or removing meters in `tau-meters.bin`. Writes nothing.
#[tauri::command(async)]
pub fn meter_bin_plan_edit(
    app: tauri::AppHandle,
    card: String,
    rows: Vec<RowEdit>,
) -> Result<MeterBinPlan, TauError> {
    let book = read_authors(&app);
    let known = book.get(&card_key(&card)).cloned().unwrap_or_default();
    meter_install::plan(Path::new(&card), &MeterAction::EditList { rows }, &known)
}

/// Executes a reviewed meter-list edit on the card.
#[tauri::command(async)]
pub fn meter_bin_execute_edit(
    app: tauri::AppHandle,
    card: String,
    rows: Vec<RowEdit>,
    confirmation: String,
) -> Result<MeterBinReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let backup = install_backup_root(&app)?;
    let book = read_authors(&app);
    let known = book.get(&card_key(&card)).cloned().unwrap_or_default();
    meter_install::execute(
        Path::new(&card),
        &MeterAction::EditList { rows },
        &known,
        &confirmation,
        &backup,
    )
}

/// Plans installing or updating a `.tmeter` package onto the card. Writes nothing.
#[tauri::command(async)]
pub fn meter_bin_plan_install(
    app: tauri::AppHandle,
    card: String,
    package_path: String,
    allow_source_change: bool,
    overwrite_presets: bool,
) -> Result<MeterBinPlan, TauError> {
    let bytes = read_package_bytes(&package_path)?;
    let book = read_authors(&app);
    let known = book.get(&card_key(&card)).cloned().unwrap_or_default();
    meter_install::plan(
        Path::new(&card),
        &MeterAction::InstallPackage {
            package_bytes: bytes,
            allow_source_change,
            overwrite_presets,
        },
        &known,
    )
}

/// Executes a reviewed `.tmeter` package install onto the card and records its author for future source checks.
#[tauri::command(async)]
pub fn meter_bin_execute_install(
    app: tauri::AppHandle,
    card: String,
    package_path: String,
    allow_source_change: bool,
    overwrite_presets: bool,
    confirmation: String,
) -> Result<MeterBinReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let backup = install_backup_root(&app)?;
    let bytes = read_package_bytes(&package_path)?;
    let pkg = meter_package::parse(&bytes)?;
    let mut book = read_authors(&app);
    let key = card_key(&card);
    let known = book.get(&key).cloned().unwrap_or_default();
    let report = meter_install::execute(
        Path::new(&card),
        &MeterAction::InstallPackage {
            package_bytes: bytes,
            allow_source_change,
            overwrite_presets,
        },
        &known,
        &confirmation,
        &backup,
    )?;
    book.entry(key)
        .or_default()
        .insert(pkg.summary.id, pkg.summary.author);
    write_authors(&app, &book);
    Ok(report)
}

/// Plans removing `tau-meters.bin` so the Pocket falls back to the core's shipped meters. Writes nothing.
#[tauri::command(async)]
pub fn meter_bin_plan_reset(
    app: tauri::AppHandle,
    card: String,
) -> Result<MeterBinPlan, TauError> {
    let book = read_authors(&app);
    let known = book.get(&card_key(&card)).cloned().unwrap_or_default();
    meter_install::plan(Path::new(&card), &MeterAction::ResetToShipped, &known)
}

/// Executes a reviewed reset of `tau-meters.bin` (after verified backup outside the card).
#[tauri::command(async)]
pub fn meter_bin_execute_reset(
    app: tauri::AppHandle,
    card: String,
    confirmation: String,
) -> Result<MeterBinReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let backup = install_backup_root(&app)?;
    let book = read_authors(&app);
    let known = book.get(&card_key(&card)).cloned().unwrap_or_default();
    meter_install::execute(
        Path::new(&card),
        &MeterAction::ResetToShipped,
        &known,
        &confirmation,
        &backup,
    )
}
