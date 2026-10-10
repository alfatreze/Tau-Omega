//! Updating Tau Omega itself (`tauri-plugin-updater`). Separate from `updates.rs`, which checks for new *Tau core* releases.
//!
//! What leaves the machine: one GET of `latest.json` from this repository's GitHub releases, and, only when the user presses
//! Update, the signed installer bundle it names. The bundle is verified against the public key in `tauri.conf.json` before it is
//! installed; an unsigned or tampered download is refused. Nothing is ever downloaded or installed without the user's click.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tau_core::{ErrorCode, TauError};

/// True while an update is being installed; `begin_card_write` refuses to start a card write then.
pub static UPDATING: AtomicBool = AtomicBool::new(false);
use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
pub struct OmegaUpdate {
    pub current: String,
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

fn err(message: impl Into<String>) -> TauError {
    TauError { code: ErrorCode::Io, message: message.into() }
}

fn explain(error: &tauri_plugin_updater::Error) -> String {
    let text = error.to_string();
    if text.contains("404") || text.contains("Could not fetch") {
        "No Tau Omega update information was found on GitHub yet (there may be no release with an updater file).".into()
    } else if text.to_lowercase().contains("network") || text.contains("error sending request") {
        "Could not reach GitHub. Check the connection and try again.".into()
    } else {
        format!("The Tau Omega update check failed: {text}")
    }
}

/// Looks for a newer Tau Omega. `None` means this one is current. Downloads nothing.
#[tauri::command]
pub async fn omega_update_check(app: tauri::AppHandle) -> Result<Option<OmegaUpdate>, TauError> {
    let updater = app.updater().map_err(|e| err(explain(&e)))?;
    let found = updater.check().await.map_err(|e| err(explain(&e)))?;
    Ok(found.map(|u| OmegaUpdate { current: u.current_version.to_string(), version: u.version.clone(), notes: u.body.clone(), date: u.date.map(|d| d.to_string()) }))
}

/// Downloads the update found by a fresh check, verifies its signature, installs it and restarts Tau Omega.
/// Refused while a card write is running (the app would be killed halfway through it).
#[tauri::command]
pub async fn omega_update_install(app: tauri::AppHandle) -> Result<(), TauError> {
    // Refuse while a card write runs, and stop one starting during the install (the guard cannot be held across an await).
    drop(crate::card_write_guard().map_err(|_| err("A card write is running. Wait for it to finish before updating Tau Omega."))?);
    UPDATING.store(true, Ordering::SeqCst);
    let result = install(&app).await;
    UPDATING.store(false, Ordering::SeqCst);
    result
}

async fn install(app: &tauri::AppHandle) -> Result<(), TauError> {    let updater = app.updater().map_err(|e| err(explain(&e)))?;
    let Some(update) = updater.check().await.map_err(|e| err(explain(&e)))? else {
        return Err(err("Tau Omega is already up to date."));
    };
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| err(format!("The update could not be installed: {e}")))?;
    app.restart();
}
