//! Tauri commands moved out of `main.rs` unchanged; shared helpers and views stay in `main.rs`.

use super::*;

#[tauri::command(async)]
pub fn read_persisted_settings(path: String) -> Result<Vec<tau_core::diag::PersistedSetting>, TauError> {
    tau_core::diag::read_persisted_settings(path)
}

/// `None` means these persist ids don't currently hold a Check summary --
/// the normal case (no Check has been run, or they're this core's legacy
/// playlist state, per `docs/FIRMWARE_SYNC.md`'s overloaded-ids trap), not
/// an error the front-end needs to display as one.
#[tauri::command(async)]
pub fn read_check_summary(path: String) -> Result<Option<tau_core::diag::CheckSummary>, TauError> {
    match tau_core::diag::read_check_summary(path) {
        Ok(summary) => Ok(Some(summary)),
        Err(error) if error.code() == ErrorCode::NotACheckSummary => Ok(None),
        Err(error) => Err(error),
    }
}

/// Decodes a screenshot's Check QR page into the full TAUD1 report. Unlike
/// `read_check_summary` (the tiny 4-word persisted summary), this needs a
/// real screenshot PNG, not a card path -- `None` when the image simply has
/// no QR code in it (a normal screenshot of something else), an error for
/// anything that looks like a QR but fails to decode as a valid report.
#[tauri::command(async)]
pub fn read_qr_report(path: String) -> Result<Option<tau_core::taud::TaudReport>, TauError> {
    match tau_core::taud::read_screenshot_report(Path::new(&path)) {
        Ok(report) => Ok(Some(report)),
        Err(error) if error.code() == ErrorCode::NoQrCodeFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Lists every screenshot the Pocket has saved on this card
/// (`Memories/Screenshots/`), newest first -- the "find it automatically"
/// complement to `read_qr_report`, which decodes one the caller already has
/// a path for.
#[tauri::command(async)]
pub fn list_screenshots(card: String) -> Result<Vec<tau_core::screenshots::ScreenshotEntry>, TauError> {
    tau_core::screenshots::list_screenshots(Path::new(&card))
}

/// Reads one screenshot's raw bytes and returns them as a `data:` URL, so
/// the UI can show the actual image inline (a real preview, not just its
/// filename) without granting the webview broader filesystem access via
/// Tauri's asset protocol -- this only ever serves a path `list_screenshots`
/// or the user's own file picker already produced.
#[tauri::command(async)]
pub fn read_image_data_url(path: String) -> Result<String, TauError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = std::fs::read(&path)?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

/// Send diagnostics: reads what a Check run left on a card (saved summaries, Check QR screenshots, core file
/// checksums). Read-only; also returns the readable summary for "Copy summary".
#[derive(Serialize)]
pub(super) struct DiagView {
    reading: tau_core::diagnostics::DiagReading,
    summary: String,
}

#[tauri::command(async)]
pub fn diag_read(card: String) -> Result<DiagView, TauError> {
    let reading = tau_core::diagnostics::read(Path::new(&card), 0)?;
    let summary = tau_core::diagnostics::summary_markdown(&reading);
    Ok(DiagView { reading, summary })
}

/// Send diagnostics: writes the zip into a folder outside the card. Nothing is uploaded and nothing is written to the card.
#[tauri::command(async)]
pub fn diag_zip(card: String, dest_dir: String) -> Result<String, TauError> {
    let reading = tau_core::diagnostics::read(Path::new(&card), 0)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = tau_core::diagnostics::create_zip(&reading, Path::new(&card), Path::new(&dest_dir), now)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Previews what `art_sidecar_pal256` will encode for one album's cover
/// (`cover_source`, from a `plan_sync` result's `art_sidecar_previews`),
/// without writing anything: runs the exact same quantizer `execute_sync`
/// will use, so what's shown here matches what actually ends up on the card.
#[tauri::command(async)]
pub fn preview_art_sidecar(cover_source: String) -> Result<String, TauError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = std::fs::read(&cover_source)?;
    let png = tau_core::image::preview_pal256_png(&bytes, 128)?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

/// Decodes a core's `icon.bin` (`Cores/<core_id>/icon.bin`) into a PNG data
/// URL. `None`, not an error, when the file simply doesn't exist -- not
/// every core ships one.
#[tauri::command(async)]
pub fn read_core_icon(card: String, core_id: String) -> Result<Option<String>, TauError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let path = Path::new(&card).join("Cores").join(&core_id).join("icon.bin");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(TauError::from(error)),
    };
    let png_bytes = tau_core::icon::decode_icon_bin(&bytes)?;
    Ok(Some(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(png_bytes)
    )))
}

/// Decodes a platform's banner (`Platforms/_images/<platform>.bin`) into a
/// PNG data URL -- the real per-platform artwork (521x165, shared by every
/// core on that platform), distinct from a core's own small `icon.bin`.
/// `None`, not an error, when the file doesn't exist.
#[tauri::command(async)]
pub fn read_platform_image(card: String, platform: String) -> Result<Option<String>, TauError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let path = Path::new(&card)
        .join("Platforms")
        .join("_images")
        .join(format!("{platform}.bin"));
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(TauError::from(error)),
    };
    let png_bytes = tau_core::icon::decode_platform_image(&bytes)?;
    Ok(Some(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(png_bytes)
    )))
}
