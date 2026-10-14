//! Conversion settings support: where `lame` is, and installing it when the user says yes.
//!
//! `lame` makes the MP3s (for lossy files the Pocket cannot play). It is run as a separate program and
//! never linked, so its licence stays its own. Nothing is downloaded or installed unless the user asks.

use super::*;

#[derive(Serialize)]
pub struct LameInstallOption {
    /// `"brew"` is the only method the app runs itself; the others are instructions.
    method: String,
    label: String,
    /// What would be run (or the user should run).
    command: String,
    runnable: bool,
}

#[derive(Serialize)]
pub struct LameStatus {
    /// The `lame` that will be used, if one was found.
    path: Option<String>,
    version: Option<String>,
    options: Vec<LameInstallOption>,
}

fn find_in_path(name: &str, extra: &[&str]) -> Option<PathBuf> {
    let from_path = std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>());
    from_path.chain(extra.iter().map(PathBuf::from)).map(|d| d.join(name)).find(|p| p.is_file())
}

fn install_options() -> Vec<LameInstallOption> {
    let mut out = Vec::new();
    if cfg!(target_os = "macos") {
        let brew = find_in_path("brew", &["/opt/homebrew/bin", "/usr/local/bin"]);
        out.push(LameInstallOption {
            method: "brew".into(),
            label: if brew.is_some() { "Install with Homebrew".into() } else { "Homebrew is not installed".into() },
            command: "brew install lame".into(),
            runnable: brew.is_some(),
        });
        out.push(LameInstallOption {
            method: "manual".into(),
            label: "Or install Homebrew first from brew.sh, then come back".into(),
            command: "https://brew.sh".into(),
            runnable: false,
        });
    } else if cfg!(windows) {
        if let Some(source) = tau_core::convert::lame_source() {
            out.push(LameInstallOption {
                method: "download".into(),
                label: format!("Download {} (about {} KB, checked against a pinned checksum)", source.label, source.bytes_about / 1000),
                command: source.url.into(),
                runnable: true,
            });
        }
        out.push(LameInstallOption {
            method: "manual".into(),
            label: "Or use a package manager".into(),
            command: "winget install LAME.LAME   |   scoop install lame".into(),
            runnable: false,
        });
    } else {
        out.push(LameInstallOption {
            method: "manual".into(),
            label: "Install LAME with your package manager (it needs administrator rights, so this app does not run it)".into(),
            command: "sudo apt install lame   |   sudo dnf install lame   |   sudo pacman -S lame".into(),
            runnable: false,
        });
    }
    out
}

fn lame_dir(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    Ok(data_dir(app)?.join("tools"))
}

fn status_for(app: &tauri::AppHandle) -> Result<LameStatus, TauError> {
    let prefs = read_prefs(app)?;
    let found = tau_audio_find(&prefs.convert.lame_path, &lame_dir(app)?);
    let version = found.as_deref().and_then(|p| tau_core::convert::lame_version(p).ok());
    Ok(LameStatus { path: found.map(|p| p.to_string_lossy().into_owned()), version, options: install_options() })
}

fn tau_audio_find(custom: &Option<PathBuf>, managed: &Path) -> Option<PathBuf> {
    tau_core::convert::ConvertSettings { lame_path: custom.clone(), lame_dir: Some(managed.to_path_buf()), ..Default::default() }.find_lame()
}

/// Whether a usable `lame` exists, and what the user could do about it if not.
#[tauri::command(async)]
pub fn lame_status(app: tauri::AppHandle) -> Result<LameStatus, TauError> {
    status_for(&app)
}

/// Checks a `lame` the user picked and returns its version line.
#[tauri::command(async)]
pub fn check_lame(path: String) -> Result<String, TauError> {
    tau_core::convert::lame_version(Path::new(&path))
}

/// Installs `lame` with the chosen method. Only Homebrew is run by the app (the user was asked first in the
/// window); every other method is shown as instructions.
#[tauri::command(async)]
pub fn install_lame(app: tauri::AppHandle, method: String) -> Result<LameStatus, TauError> {
    if method == "download" {
        let source = tau_core::convert::lame_source().ok_or_else(|| TauError { code: ErrorCode::UnsupportedTags, message: "there is no download for this system".into() })?;
        let bytes = tau_net::download_lame(&source)?;
        let exe = tau_core::convert::install_lame_zip(&bytes, source.sha256, &lame_dir(&app)?)?;
        tau_core::convert::lame_version(&exe)?;
        return status_for(&app);
    }
    if method != "brew" {
        return Err(TauError { code: ErrorCode::UnsupportedTags, message: "this method is done by hand".into() });
    }
    let brew = find_in_path("brew", &["/opt/homebrew/bin", "/usr/local/bin"])
        .ok_or_else(|| TauError { code: ErrorCode::Io, message: "Homebrew was not found".into() })?;
    let output = std::process::Command::new(&brew)
        .args(["install", "lame"])
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .output()
        .map_err(|e| TauError { code: ErrorCode::Io, message: format!("could not run Homebrew: {e}") })?;
    if !output.status.success() {
        let tail: String = String::from_utf8_lossy(&output.stderr).lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" ");
        return Err(TauError { code: ErrorCode::Io, message: format!("Homebrew could not install LAME: {tail}") });
    }
    status_for(&app)
}
