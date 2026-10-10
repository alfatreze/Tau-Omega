//! Tauri commands moved out of `main.rs` unchanged; shared helpers and views stay in `main.rs`.

use super::*;

#[tauri::command(async)]
pub fn inspect_package(path: String) -> Result<tau_core::package::PackageManifest, TauError> {
    tau_core::package::inspect(Path::new(&path))
}

#[tauri::command(async)]
pub fn plan_package_install(path: String, card: String) -> Result<tau_core::package::PackagePlan, TauError> {
    tau_core::package::plan_install(Path::new(&path), Path::new(&card))
}

#[tauri::command(async)]
pub fn execute_package_install(app: tauri::AppHandle, 
    path: String,
    card: String,
    confirmation: String,
) -> Result<tau_core::package::PackageReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let zip_path = Path::new(&path);
    let card_root = Path::new(&card);
    let plan = tau_core::package::plan_install(zip_path, card_root)?;
    tau_core::package::execute_install(zip_path, card_root, &plan, &confirmation)
}

/// Plans removing an installed core; re-inspects the card so the plan always
/// reflects every currently-installed core, not a snapshot the caller might
/// be holding stale (the same "safe to re-derive, cheap to re-check" choice
/// `execute_package_install` already makes for its own plan).
#[tauri::command(async)]
pub fn plan_remove_core(app: tauri::AppHandle, card: String, core_id: String, keep_media: Option<bool>) -> Result<tau_core::remove::RemovePlan, TauError> {
    let card = tau_core::inspect_card(Path::new(&card))?;
    let docs = tau_core::release_check::load_cached(&manifest_cache(&app)?);
    // Default: keep the library, music and theme file; only the core's own files go.
    tau_core::remove::plan_remove_with(&card, &core_id, keep_media.unwrap_or(true), &docs)
}

#[tauri::command(async)]
pub fn execute_remove_core(app: tauri::AppHandle, 
    card: String,
    core_id: String,
    keep_media: Option<bool>,
    confirmation: String,
) -> Result<tau_core::remove::RemoveReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let card = tau_core::inspect_card(Path::new(&card))?;
    let docs = tau_core::release_check::load_cached(&manifest_cache(&app)?);
    let plan = tau_core::remove::plan_remove_with(&card, &core_id, keep_media.unwrap_or(true), &docs)?;
    tau_core::remove::execute_remove(&plan, &confirmation)
}
