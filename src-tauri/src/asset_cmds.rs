//! Tauri commands for the theme file (Appearance) and the Halcyon user presets: both read and write `tau-assets.bin` through
//! `tau_core::assets`. Moved out of `main.rs` unchanged.

use crate::{ErrorCode, TauError, begin_card_write};
use std::path::Path;

/// Appearance: checks a theme (names, ranges, contrast on every surface) and shows each colour as the Pocket will
/// show it. Writes nothing.
#[tauri::command(async)]
pub fn appearance_check(theme: tau_core::assets::ThemeInput) -> tau_core::assets::ThemeReport {
    tau_core::assets::check_theme(&theme)
}

/// Appearance: opens a `tau-assets.bin` for editing. Every checksum is verified before anything is shown.
#[tauri::command(async)]
pub fn appearance_open(path: String) -> Result<Vec<tau_core::assets::ThemeInput>, TauError> {
    tau_core::assets::parse_assets(&std::fs::read(&path)?)
}

/// Appearance: writes `tau-assets.bin` to a file the user chose (not to the card), then reads it back and checks it
/// parses to the same themes. Every non-theme section (meter presets, EQ presets) is kept: from `keep_from` (the file
/// the themes were opened from) when given, else from the file being overwritten. Returns the file's size in bytes.
#[tauri::command(async)]
pub fn appearance_export(
    themes: Vec<tau_core::assets::ThemeInput>,
    path: String,
    keep_from: Option<String>,
) -> Result<u64, TauError> {
    let source = match &keep_from {
        Some(k) => Some(std::fs::read(k)?),
        None => std::fs::read(&path).ok(),
    };
    let bytes = tau_core::assets::pack_assets_keeping(&themes, source.as_deref())?;
    std::fs::write(&path, &bytes)?;
    let written = std::fs::read(&path)?;
    let back = tau_core::assets::parse_assets(&written)?;
    let kept = source
        .as_deref()
        .map(tau_core::assets::kept_sections)
        .unwrap_or_default();
    if written != bytes
        || tau_core::assets::pack_assets_keeping(&back, source.as_deref())? != bytes
        || tau_core::assets::kept_sections(&written) != kept
    {
        return Err(TauError {
            code: ErrorCode::VerificationFailed,
            message: "The saved file did not read back the same, so it should not be used.".into(),
        });
    }
    Ok(bytes.len() as u64)
}

/// Appearance: plans putting these themes on a card at `<media root>/tau-assets.bin`. Writes nothing.
#[tauri::command(async)]
pub fn appearance_plan_install(
    themes: Vec<tau_core::assets::ThemeInput>,
    media_root: String,
) -> Result<tau_core::assets::AssetsInstallPlan, TauError> {
    tau_core::assets::plan_install(&themes, Path::new(&media_root))
}

/// Appearance: writes a reviewed theme-file install. Takes the card write lock, backs up any file it replaces
/// (when `backup` is given), verifies by reading back from the device, and swaps the file in recoverably.
#[tauri::command(async)]
pub fn appearance_install(
    app: tauri::AppHandle,
    themes: Vec<tau_core::assets::ThemeInput>,
    media_root: String,
    confirmation: String,
    backup: Option<String>,
) -> Result<tau_core::assets::AssetsInstallReport, TauError> {
    let _write = begin_card_write(&app, &media_root)?;
    let root = Path::new(&media_root);
    let plan = tau_core::assets::plan_install(&themes, root)?;
    tau_core::assets::execute_install(
        &themes,
        root,
        &plan,
        &confirmation,
        backup.as_deref().map(Path::new),
    )
}

/// Halcyon presets: reads the user presets out of a `tau-assets.bin` (empty when it has none). Every checksum is verified.
#[tauri::command(async)]
pub fn halcyon_open(path: String) -> Result<Vec<tau_core::halcyon::HalcyonPreset>, TauError> {
    tau_core::assets::read_presets(&std::fs::read(&path)?)
}

/// Halcyon presets: imports an Equalizer APO / AutoEQ profile text as a raw preset. Writes nothing.
#[tauri::command(async)]
pub fn halcyon_import_apo(
    name: String,
    text: String,
) -> Result<tau_core::halcyon::ApoImport, TauError> {
    tau_core::halcyon::import_apo(&name, &text)
}

/// Halcyon presets: the response curve of a raw preset, for drawing. `None` for a control preset.
#[tauri::command(async)]
pub fn halcyon_curve(preset: tau_core::halcyon::HalcyonPreset) -> Option<Vec<(f64, f64)>> {
    tau_core::halcyon::response_curve(&preset)
}

/// Halcyon presets: runs every writer rule over the list and returns the size of the section it would write.
#[tauri::command(async)]
pub fn halcyon_validate(presets: Vec<tau_core::halcyon::HalcyonPreset>) -> Result<usize, TauError> {
    Ok(tau_core::halcyon::pack_presets(&presets)?.len())
}

/// Halcyon presets: writes `tau-assets.bin` to a file the user chose (not the card), keeping the themes and every other section
/// of `keep_from` (the file the presets were opened from) or of the file being overwritten, then reads it back and checks it.
#[tauri::command(async)]
pub fn halcyon_export(
    presets: Vec<tau_core::halcyon::HalcyonPreset>,
    path: String,
    keep_from: Option<String>,
) -> Result<u64, TauError> {
    let source = match &keep_from {
        Some(k) => Some(std::fs::read(k)?),
        None => std::fs::read(&path).ok(),
    };
    let edit = tau_core::assets::AssetsEdit::presets(&presets);
    let bytes = tau_core::assets::pack_assets_edit(edit, source.as_deref())?;
    std::fs::write(&path, &bytes)?;
    let written = std::fs::read(&path)?;
    if written != bytes || tau_core::assets::read_presets(&written)? != presets {
        return Err(TauError {
            code: ErrorCode::VerificationFailed,
            message: "The saved file did not read back the same, so it should not be used.".into(),
        });
    }
    Ok(bytes.len() as u64)
}

/// Halcyon presets: plans putting these presets on a card at `<media root>/tau-assets.bin` (themes and other sections kept). Writes nothing.
#[tauri::command(async)]
pub fn halcyon_plan_install(
    presets: Vec<tau_core::halcyon::HalcyonPreset>,
    media_root: String,
) -> Result<tau_core::assets::AssetsInstallPlan, TauError> {
    tau_core::assets::plan_install_edit(
        tau_core::assets::AssetsEdit::presets(&presets),
        Path::new(&media_root),
    )
}

/// Halcyon presets: writes a reviewed install (card write lock, verified backup outside the card, read-back, recoverable swap).
#[tauri::command(async)]
pub fn halcyon_install(
    app: tauri::AppHandle,
    presets: Vec<tau_core::halcyon::HalcyonPreset>,
    media_root: String,
    confirmation: String,
    backup: Option<String>,
) -> Result<tau_core::assets::AssetsInstallReport, TauError> {
    let _write = begin_card_write(&app, &media_root)?;
    let root = Path::new(&media_root);
    let edit = tau_core::assets::AssetsEdit::presets(&presets);
    let plan = tau_core::assets::plan_install_edit(edit, root)?;
    tau_core::assets::execute_install_edit(
        edit,
        root,
        &plan,
        &confirmation,
        backup.as_deref().map(Path::new),
    )
}
