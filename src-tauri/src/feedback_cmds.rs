//! Feedback to the Tau Omega project (`docs/FEEDBACK_SERVICE.md`, D-025): a message becomes a public GitHub issue,
//! filed by the owner's Cloudflare Worker. Omega never holds a GitHub token.
//!
//! Sending stays disabled (`enabled: false`, menu entry hidden) until `TAU_OMEGA_FEEDBACK_URL` is configured
//! (either at build time via `option_env!` or at runtime via environment variable for staging tests).

use serde::Serialize;
use tau_core::{ErrorCode, TauError};
use tau_net::feedback::{self, FeedbackSend, FeedbackSent, Http};

#[derive(Serialize)]
pub struct FeedbackInfo {
    /// False until the feedback service URL is configured: the page then shows a notice and never offers Send.
    pub enabled: bool,
    /// The few facts a sender may choose to add to a message.
    pub version: String,
    pub os: String,
    pub arch: String,
}

const DEFAULT_FEEDBACK_URL: &str = "https://tau-omega-feedback.alfatreze.workers.dev";

fn endpoint() -> Option<String> {
    if let Ok(v) = std::env::var("TAU_OMEGA_FEEDBACK_URL") {
        let t = v.trim();
        if t.is_empty() || t == "off" {
            return None;
        }
        if t.starts_with("https://") {
            return Some(t.to_string());
        }
    }
    if let Some(v) = option_env!("TAU_OMEGA_FEEDBACK_URL") {
        let t = v.trim();
        if t.starts_with("https://") {
            return Some(t.to_string());
        }
    }
    Some(DEFAULT_FEEDBACK_URL.to_string())
}

#[tauri::command]
pub fn feedback_info() -> FeedbackInfo {
    FeedbackInfo {
        enabled: endpoint().is_some(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

/// Validates and sends a feedback message to the configured Cloudflare Worker.
#[tauri::command(async)]
pub fn feedback_send(message: FeedbackSend) -> Result<FeedbackSent, TauError> {
    let Some(url) = endpoint() else {
        return Err(TauError {
            code: ErrorCode::InvalidPathReference,
            message: "Sending feedback is not connected yet.".into(),
        });
    };
    feedback::send(&Http, &url, &message)
}

/// Opens a filed GitHub issue in the system browser. Strictly builds `https://github.com/alfatreze/Tau-Omega/issues/<number>`.
#[tauri::command(async)]
pub fn feedback_open_issue(number: u64) -> Result<(), TauError> {
    if number == 0 {
        return Err(TauError {
            code: ErrorCode::InvalidPathReference,
            message: "Invalid issue number.".into(),
        });
    }
    let url = format!("https://github.com/alfatreze/Tau-Omega/issues/{number}");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(&url)
        .spawn()
        .map(|_| ())
        .map_err(|e| TauError {
            code: ErrorCode::Io,
            message: format!("Could not open the browser: {e}"),
        })
}
