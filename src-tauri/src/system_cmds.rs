//! Tauri commands for the canonical platform catalog and unified device-agnostic core profiles.

use crate::TauError;
use std::path::Path;
use tau_core::{
    device::DeviceKind,
    platforms::{self, PlatformSpec},
    system::{self, CoreProfile, CoreSummary},
};

/// Returns every canonical gaming platform with standard extensions and artwork mappings.
#[tauri::command]
pub fn platforms_catalog() -> Vec<PlatformSpec> {
    platforms::all()
}

/// Lists normalized core summaries for `device` on `card`. Read-only.
#[tauri::command(async)]
pub fn system_list_cores(card: String, device: DeviceKind) -> Result<Vec<CoreSummary>, TauError> {
    system::list_cores(Path::new(&card), device)
}

/// Returns the normalized profile (capabilities, facts, targets, ROM/artwork routes) for one core. Read-only.
#[tauri::command(async)]
pub fn system_core_profile(
    card: String,
    device: DeviceKind,
    core_id: String,
) -> Result<CoreProfile, TauError> {
    system::inspect_core(Path::new(&card), device, &core_id)
}
