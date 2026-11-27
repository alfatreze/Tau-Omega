//! Feedback sender (`docs/FEEDBACK_SERVICE.md`, D-025): validates fields and attachment limits/bytes,
//! POSTs JSON to the owner's Cloudflare Worker (`POST /v1/feedback`, no redirects, 60 s timeout),
//! and maps HTTP errors to plain-language messages. Tested with a scripted [`FeedbackTransport`].

use serde::{Deserialize, Serialize};
use std::{io::Read, time::Duration};
use tau_core::{ErrorCode, TauError};

pub const MAX_TITLE: usize = 80;
pub const MIN_TEXT: usize = 10;
pub const MAX_TEXT: usize = 4000;
pub const MAX_FILES: usize = 4;
pub const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: usize = 12 * 1024 * 1024;
pub const MAX_BODY_BYTES: usize = 18 * 1024 * 1024;

const KINDS: [&str; 3] = ["bug", "idea", "question"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackBasics {
    pub version: String,
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackFile {
    pub name: String,
    pub mime: String,
    /// Standard base64.
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackSend {
    pub kind: String,
    pub area: String,
    pub title: String,
    pub text: String,
    pub basics: Option<FeedbackBasics>,
    pub github: String,
    pub files: Vec<FeedbackFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackSent {
    pub number: u64,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct PostResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

pub trait FeedbackTransport {
    fn post_json(&self, url: &str, body: &[u8]) -> Result<PostResponse, TauError>;
}

pub struct Http;

fn err(code: ErrorCode, msg: impl Into<String>) -> TauError {
    TauError {
        code,
        message: msg.into(),
    }
}

impl FeedbackTransport for Http {
    fn post_json(&self, url: &str, body: &[u8]) -> Result<PostResponse, TauError> {
        if !url.starts_with("https://") {
            return Err(err(
                ErrorCode::InvalidPathReference,
                "The feedback service address must use HTTPS.",
            ));
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_global(Some(Duration::from_secs(60)))
            .max_redirects(0)
            .http_status_as_error(false)
            .user_agent(concat!("Tau-Omega/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        let mut resp = agent
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send(body)
            .map_err(|e| match e {
                ureq::Error::Timeout(_) => err(
                    ErrorCode::Io,
                    "The feedback service did not answer in time. Try again in a moment.",
                ),
                ureq::Error::Io(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => {
                    err(
                        ErrorCode::Io,
                        "Could not reach the feedback service. Are you offline?",
                    )
                }
                other => err(ErrorCode::Io, format!("Could not send feedback: {other}")),
            })?;
        let status = resp.status().as_u16();
        let mut out = Vec::new();
        resp.body_mut()
            .as_reader()
            .take(64 * 1024)
            .read_to_end(&mut out)
            .map_err(|e| err(ErrorCode::Io, format!("Could not read response: {e}")))?;
        Ok(PostResponse { status, body: out })
    }
}

/// Minimal standard base64 decoder (rejects non-base64 bytes) so `tau-net` needs no extra crate.
fn decode_base64(input: &str) -> Result<Vec<u8>, TauError> {
    let bytes: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "An attachment is not valid base64.",
        ));
    }
    let val = |c: u8| -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.as_chunks::<4>().0 {
        let a = val(chunk[0]).ok_or_else(|| {
            err(
                ErrorCode::InvalidPathReference,
                "An attachment is not valid base64.",
            )
        })?;
        let b = val(chunk[1]).ok_or_else(|| {
            err(
                ErrorCode::InvalidPathReference,
                "An attachment is not valid base64.",
            )
        })?;
        out.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            let c = val(chunk[2]).ok_or_else(|| {
                err(
                    ErrorCode::InvalidPathReference,
                    "An attachment is not valid base64.",
                )
            })?;
            out.push((b << 4) | (c >> 2));
            if chunk[3] != b'=' {
                let d = val(chunk[3]).ok_or_else(|| {
                    err(
                        ErrorCode::InvalidPathReference,
                        "An attachment is not valid base64.",
                    )
                })?;
                out.push((c << 6) | d);
            }
        } else if chunk[3] != b'=' {
            return Err(err(
                ErrorCode::InvalidPathReference,
                "An attachment is not valid base64.",
            ));
        }
    }
    Ok(out)
}

fn check_magic(ext: &str, bytes: &[u8]) -> bool {
    match ext {
        "png" => bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
        "jpg" | "jpeg" => {
            bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF
        }
        "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        "zip" => bytes.starts_with(b"PK"),
        "txt" | "log" | "json" => !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok(),
        _ => false,
    }
}

/// Validates a feedback message against all `docs/FEEDBACK_SERVICE.md` rules before anything leaves the machine.
pub fn validate(msg: &FeedbackSend) -> Result<(), TauError> {
    if !KINDS.contains(&msg.kind.as_str()) {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "Choose what kind of feedback this is (bug, idea, or question).",
        ));
    }
    if msg.area.trim().is_empty() {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "Choose which area of Tau Omega this is about.",
        ));
    }
    let title_len = msg.title.trim().chars().count();
    if title_len == 0 || title_len > MAX_TITLE {
        return Err(err(
            ErrorCode::InvalidPathReference,
            format!("The title must be 1 to {MAX_TITLE} characters."),
        ));
    }
    let text_len = msg.text.trim().chars().count();
    if !(MIN_TEXT..=MAX_TEXT).contains(&text_len) {
        return Err(err(
            ErrorCode::InvalidPathReference,
            format!("The description must be {MIN_TEXT} to {MAX_TEXT} characters."),
        ));
    }
    let gh = msg.github.trim().trim_start_matches('@');
    if !gh.is_empty()
        && (gh.len() > 39
            || gh.starts_with('-')
            || gh.ends_with('-')
            || gh.contains("--")
            || !gh.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
    {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "That does not look like a valid GitHub username.",
        ));
    }
    if msg.files.len() > MAX_FILES {
        return Err(err(
            ErrorCode::InvalidPathReference,
            format!("You can attach at most {MAX_FILES} files."),
        ));
    }
    let mut total = 0usize;
    for f in &msg.files {
        let ext = f
            .name
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let raw = decode_base64(&f.data)?;
        if raw.len() > MAX_FILE_BYTES {
            return Err(err(
                ErrorCode::InvalidPathReference,
                format!("'{}' is larger than the 5 MB per-file limit.", f.name),
            ));
        }
        total = total.saturating_add(raw.len());
        if total > MAX_TOTAL_BYTES {
            return Err(err(
                ErrorCode::InvalidPathReference,
                "Attachments exceed the 12 MB total limit.",
            ));
        }
        if !check_magic(&ext, &raw) {
            return Err(err(
                ErrorCode::InvalidPathReference,
                format!(
                    "'{}' does not match an allowed file type (png, jpg, gif, webp, txt, log, json, zip).",
                    f.name
                ),
            ));
        }
    }
    Ok(())
}

/// Validates and sends `msg` to `<base_url>/v1/feedback`.
pub fn send(
    t: &dyn FeedbackTransport,
    base_url: &str,
    msg: &FeedbackSend,
) -> Result<FeedbackSent, TauError> {
    validate(msg)?;
    let url = format!("{}/v1/feedback", base_url.trim_end_matches('/'));
    let body =
        serde_json::to_vec(msg).map_err(|e| err(ErrorCode::InvalidPathReference, e.to_string()))?;
    if body.len() > MAX_BODY_BYTES {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "The feedback request is too large to send.",
        ));
    }
    let resp = t.post_json(&url, &body)?;
    if resp.status == 201 || resp.status == 200 {
        return serde_json::from_slice::<FeedbackSent>(&resp.body).map_err(|_| {
            err(
                ErrorCode::Io,
                "The feedback service answered, but the reply could not be read.",
            )
        });
    }
    let server_msg = serde_json::from_slice::<serde_json::Value>(&resp.body)
        .ok()
        .and_then(|v| {
            v.get("message")
                .and_then(|m| m.as_str())
                .map(str::to_string)
        });
    let message = match resp.status {
        400 => server_msg.unwrap_or_else(|| "Some fields in the message were not accepted.".into()),
        409 => server_msg.unwrap_or_else(|| "This exact message was already sent today.".into()),
        413 => server_msg.unwrap_or_else(|| "The message or attachments are too large.".into()),
        429 => server_msg
            .unwrap_or_else(|| "Too many messages were sent recently. Try again later.".into()),
        502 => server_msg.unwrap_or_else(|| {
            "GitHub could not be reached right now. Try again in a moment.".into()
        }),
        503 => server_msg.unwrap_or_else(|| "Feedback is paused right now.".into()),
        code => server_msg
            .unwrap_or_else(|| format!("The feedback service answered with an error ({code}).")),
    };
    Err(err(ErrorCode::Io, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(u16, String);
    impl FeedbackTransport for Fake {
        fn post_json(&self, _url: &str, _body: &[u8]) -> Result<PostResponse, TauError> {
            Ok(PostResponse {
                status: self.0,
                body: self.1.as_bytes().to_vec(),
            })
        }
    }

    fn sample() -> FeedbackSend {
        FeedbackSend {
            kind: "bug".into(),
            area: "Marketplace".into(),
            title: "Something broke".into(),
            text: "Steps to reproduce the problem in detail.".into(),
            basics: Some(FeedbackBasics {
                version: "0.6.1".into(),
                os: "macos".into(),
                arch: "aarch64".into(),
            }),
            github: "alice-1".into(),
            files: vec![FeedbackFile {
                name: "note.txt".into(),
                mime: "text/plain".into(),
                data: "aGVsbG8=".into(), // "hello"
            }],
        }
    }

    #[test]
    fn valid_message_posts_and_parses_issue() {
        let fake = Fake(
            201,
            r#"{"number":42,"url":"https://github.com/alfatreze/Tau-Omega/issues/42"}"#.into(),
        );
        let sent = send(&fake, "https://example.workers.dev", &sample()).unwrap();
        assert_eq!(sent.number, 42);
    }

    #[test]
    fn maps_every_error_status_and_blocks_bad_attachments_before_network() {
        let mut bad_file = sample();
        bad_file.files[0].name = "fake.png".into();
        let fake = Fake(201, "{}".into());
        assert!(send(&fake, "https://example.workers.dev", &bad_file).is_err());

        for (code, expected) in [
            (409, "already sent"),
            (429, "Too many"),
            (503, "paused"),
            (502, "GitHub"),
        ] {
            let f = Fake(code, "{}".into());
            let e = send(&f, "https://example.workers.dev", &sample()).unwrap_err();
            assert!(e.message.contains(expected), "{code}: {}", e.message);
        }
    }
}
