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

fn copy_dir_or_file(src: &Path, dst: &Path) -> Result<(), TauError> {
    if src.is_file() {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dst)?;
        return Ok(());
    }
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_symlink() {
                continue;
            }
            copy_dir_or_file(&entry.path(), &dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

#[tauri::command(async)]
pub fn execute_remove_core(app: tauri::AppHandle, 
    card: String,
    core_id: String,
    keep_media: Option<bool>,
    confirmation: String,
) -> Result<tau_core::remove::RemoveReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let card_root = Path::new(&card);
    let card_info = tau_core::inspect_card(card_root)?;
    let docs = tau_core::release_check::load_cached(&manifest_cache(&app)?);
    let plan = tau_core::remove::plan_remove_with(&card_info, &core_id, keep_media.unwrap_or(true), &docs)?;
    if confirmation != plan.id {
        return Err(TauError {
            code: tau_core::ErrorCode::ConfirmationMismatch,
            message: "confirmation token does not match the current plan".to_owned(),
        });
    }
    if !plan.paths.is_empty() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let safe_id: String = core_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' })
            .collect();
        let backup_root = data_dir(&app)?
            .join("removed-backups")
            .join(format!("core-{safe_id}-{ts}"));
        for rel in &plan.paths {
            let src = card_root.join(rel);
            if src.exists() {
                copy_dir_or_file(&src, &backup_root.join(rel))?;
            }
        }
    }
    tau_core::remove::execute_remove(&plan, &confirmation)
}
