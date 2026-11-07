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
    // The source is a folder image, or an audio file whose embedded picture is the cover.
    let is_audio = matches!(
        Path::new(&cover_source).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(),
        Some("mp3" | "flac")
    );
    let bytes = if is_audio {
        tau_core::cover::extract_embedded_cover(Path::new(&cover_source)).unwrap_or_default()
    } else {
        std::fs::read(&cover_source)?
    };
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

/// A Tau core's persisted settings decoded by their layout (legacy or the new area words), or `None` for any other core: the words
/// of another core mean something else. The core is the folder after `Settings` in the path (`Settings/<core id>/Interact/_core/...`).
/// Reads only; Omega never converts a file (the player migrates a legacy one itself) and never copies words between layouts.
#[tauri::command(async)]
pub fn describe_persisted_layout(path: String) -> Result<Option<tau_core::persist_layout::LayoutView>, TauError> {
    let components: Vec<String> = std::path::Path::new(&path).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    let core = components.iter().position(|c| c == "Settings").and_then(|i| components.get(i + 1));
    if !core.is_some_and(|c| tau_core::core_update::is_tau_core(c)) {
        return Ok(None);
    }
    let settings = tau_core::diag::read_persisted_settings(&path)?;
    let card: std::path::PathBuf = components.iter().take_while(|c| *c != "Settings").collect();
    let namer = preset_namer(&card, core.map(String::as_str).unwrap_or_default());
    Ok(Some(tau_core::persist_layout::describe_with_names(&settings, &|slot, id| namer(slot, id))))
}

/// A lookup from (saved meter slot, preset file id) to the preset's name on this card. The slot is the meter's table slot (`MPRE`/`MPID`
/// are indexed by it); the folder is the meter's manifest id. Pack-only meters are found through the descriptors in the core's own
/// `tau-packs.bin` (`Assets/<platform>/<core id>/`), the built-in meter (slot 0, Winamp Bars, id 12) when no pack says otherwise.
/// Reads only; a name that cannot be found is simply not shown.
fn preset_namer(card: &std::path::Path, core_id: &str) -> impl Fn(usize, u8) -> Option<String> {
    let mut slot_ids: std::collections::BTreeMap<u16, u16> = std::collections::BTreeMap::from([(0, 12)]);
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    for platform in std::fs::read_dir(card.join("Assets")).into_iter().flatten().flatten() {
        if let Ok(bundle) = std::fs::read(platform.path().join(core_id).join("tau-packs.bin"))
            && let Ok(found) = tau_core::meter_packs::slot_ids(&bundle)
        {
            slot_ids.extend(found);
        }
        let common = platform.path().join("common");
        if common.join(tau_core::preset_files::DIR).is_dir() {
            roots.push(common);
        }
    }
    move |slot, id| {
        let manifest = slot_ids.get(&u16::try_from(slot).ok()?)?;
        roots.iter().find_map(|root| {
            let files = tau_core::preset_install::read_folder(root, *manifest);
            let (_, entries) = tau_core::preset_files::parse_index(files.get(tau_core::preset_files::INDEX_NAME)?).ok()?;
            entries.into_iter().find(|e| e.id == id).map(|e| e.name)
        })
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    fn put(core: &str) -> String {
        let dir = std::env::temp_dir().join(format!("tau-layout-{}-{}", std::process::id(), core.replace(' ', "_")));
        let d = dir.join("Settings").join(core).join("Interact/_core");
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("interact_persist.json");
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/persist_layout/dev181_interact_persist.json"), &f).unwrap();
        f.to_string_lossy().into_owned()
    }

    #[test]
    fn only_a_tau_cores_settings_are_decoded_by_the_tau_layout() {
        let tau = describe_persisted_layout(put("alfatreze.TAU DEV 181")).unwrap().expect("a Tau core is decoded");
        assert_eq!(tau.layout, tau_core::persist_layout::Layout::Legacy);
        assert!(tau.values.iter().any(|v| v.label == "Volume" && v.value == 73));
        // Another core's words mean something else: nothing is decoded and nothing is guessed.
        assert!(describe_persisted_layout(put("agg23.NES")).unwrap().is_none());
        // A file outside any Settings/<core> folder is not a core's settings either.
        let loose = std::env::temp_dir().join("loose_interact_persist.json");
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/persist_layout/dev181_interact_persist.json"), &loose).unwrap();
        assert!(describe_persisted_layout(loose.to_string_lossy().into_owned()).unwrap().is_none());
    }

    #[test]
    fn the_chosen_preset_is_shown_by_name_found_through_the_packs_descriptors() {
        let root = std::env::temp_dir().join(format!("tau-namer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let core = "alfatreze.TAU DEV 213";
        let d = root.join("Settings").join(core).join("Interact/_core");
        std::fs::create_dir_all(&d).unwrap();
        // The real DEV 213 file with one change: slot 3 is set to preset id 4 (word 15 = converted mark | id + 1; persist id 30).
        let mut json: serde_json::Value = serde_json::from_slice(&std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/persist_layout/dev213_interact_persist.json")).unwrap()).unwrap();
        for v in json["interact_persist"]["variables"].as_array_mut().unwrap() {
            if v["id"] == 30 { v["val"] = serde_json::json!((1u64 << 30) | 5); }
        }
        let f = d.join("interact_persist.json");
        std::fs::write(&f, serde_json::to_vec(&json).unwrap()).unwrap();
        // The core's packs (real DEV 184 bundle: slot 3 is Layered Wave, manifest id 16) and the card's real index for meter 16.
        let packs = root.join("Assets/tau").join(core);
        std::fs::create_dir_all(&packs).unwrap();
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/meter_packs/tau-packs_dev184.bin"), packs.join("tau-packs.bin")).unwrap();
        let idx = root.join("Assets/tau/common/tau-presets/16");
        std::fs::create_dir_all(&idx).unwrap();
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/preset_files/card/tau-presets/16/index.tpi"), idx.join("index.tpi")).unwrap();
        let view = describe_persisted_layout(f.to_string_lossy().into_owned()).unwrap().unwrap();
        let row = view.values.iter().find(|v| v.label == "Meter slot 3 preset").expect("the chosen preset is listed");
        assert_eq!(row.text, "NEON (id 4)");
        // Without the index the id is still shown, just not its name.
        std::fs::remove_dir_all(root.join("Assets/tau/common")).unwrap();
        let view = describe_persisted_layout(f.to_string_lossy().into_owned()).unwrap().unwrap();
        assert_eq!(view.values.iter().find(|v| v.label == "Meter slot 3 preset").unwrap().text, "preset id 4");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The first real capture of a preset-file id: the file TAU DEV 213 wrote on the owner's Pocket after Omega wrote meter 12's presets and
    /// the owner chose TEST ONE (id 5) on WINAMP BARS (slot 0, built in, manifest id 12) and rebooted.
    #[test]
    fn a_real_capture_from_a_pocket_names_the_preset_omega_wrote() {
        let root = std::env::temp_dir().join(format!("tau-namer-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let d = root.join("Settings/alfatreze.TAU DEV 213/Interact/_core");
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("interact_persist.json");
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/persist_layout/dev213_after_omega_presets_interact_persist.json"), &f).unwrap();
        let idx = root.join("Assets/tau/common/tau-presets/12");
        std::fs::create_dir_all(&idx).unwrap();
        for name in ["index.tpi", "p5.tpr"] {
            std::fs::copy(format!("{}/../testdata/preset_files/card_after_omega_write/tau-presets/12/{name}", env!("CARGO_MANIFEST_DIR")), idx.join(name)).unwrap();
        }
        let view = describe_persisted_layout(f.to_string_lossy().into_owned()).unwrap().unwrap();
        assert_eq!(view.layout, tau_core::persist_layout::Layout::New);
        let row = view.values.iter().find(|v| v.label == "Meter slot 0 preset").expect("the chosen preset is decoded from the real file");
        assert_eq!(row.text, "TEST ONE (id 5)");
        let _ = std::fs::remove_dir_all(&root);
    }
}
