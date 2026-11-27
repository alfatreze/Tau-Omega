//! Tauri commands for Game Boy colour palettes (`Assets/gb/common/Palettes/**/*.pal`):
//! read-only listing, reviewed plan/execute install, and reviewed plan/execute removal.

use crate::{TauError, begin_card_write, install_backup_root};
use std::path::Path;
use tau_core::gb_palettes::{
    self, PaletteInstallPlan, PaletteInstallReport, PaletteItem, PaletteRemovePlan,
    PaletteRemoveReport,
};

/// Lists all `.pal` files currently on the card under `Assets/gb/common/Palettes/`. Read-only.
#[tauri::command(async)]
pub fn gb_palettes_list(card: String) -> Vec<String> {
    gb_palettes::list_on_card(Path::new(&card))
}

/// Plans writing one or more 56-byte `.pal` files to `Assets/gb/common/Palettes/`. Read-only.
#[tauri::command(async)]
pub fn gb_palettes_plan_install(
    card: String,
    items: Vec<PaletteItem>,
) -> Result<PaletteInstallPlan, TauError> {
    gb_palettes::plan_install(Path::new(&card), &items)
}

/// Executes a reviewed palette install plan after backing up any replaced `.pal` files outside the card.
#[tauri::command(async)]
pub fn gb_palettes_execute_install(
    app: tauri::AppHandle,
    card: String,
    items: Vec<PaletteItem>,
    confirmation: String,
) -> Result<PaletteInstallReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let backup = install_backup_root(&app)?;
    gb_palettes::execute_install(Path::new(&card), &items, &confirmation, &backup)
}

/// Plans removing one `.pal` file from `Assets/gb/common/Palettes/`. Read-only.
#[tauri::command(async)]
pub fn gb_palettes_plan_remove(card: String, rel_path: String) -> Result<PaletteRemovePlan, TauError> {
    gb_palettes::plan_remove(Path::new(&card), &rel_path)
}

/// Executes a reviewed `.pal` removal after a verified backup outside the card.
#[tauri::command(async)]
pub fn gb_palettes_execute_remove(
    app: tauri::AppHandle,
    card: String,
    rel_path: String,
    confirmation: String,
) -> Result<PaletteRemoveReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let backup = install_backup_root(&app)?;
    gb_palettes::execute_remove(Path::new(&card), &rel_path, &confirmation, &backup)
}
