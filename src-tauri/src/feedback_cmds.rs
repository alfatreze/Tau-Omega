//! Feedback to the Tau Omega project (a message that becomes a public GitHub issue, filed by a small service of the owner's).
//! Only the screen exists so far: this reports that sending is **not connected**, so the page lets a person write and review a message
//! but cannot send it. The service, the send command and opening the finished issue come with it (`docs/FEEDBACK_SERVICE.md`).

use serde::Serialize;

#[derive(Serialize)]
pub struct FeedbackInfo {
    /// False until the feedback service is connected: the page then shows a notice and never offers Send.
    pub enabled: bool,
    /// The few facts a sender may choose to add to a message.
    pub version: String,
    pub os: String,
    pub arch: String,
}

#[tauri::command]
pub fn feedback_info() -> FeedbackInfo {
    FeedbackInfo {
        enabled: false,
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}
