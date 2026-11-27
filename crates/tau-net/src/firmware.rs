//! Per-system official firmware discovery and download (`Analogue Pocket` + `Game Bub`).
//!
//! Only fetches from pinned, official endpoints:
//! - Analogue Pocket: `https://www.analogue.co/support/pocket/firmware/latest/details`
//!   and downloads from `https://assets.analogue.co/firmware/` or `https://www.analogue.co/support/pocket/firmware/`.
//! - Game Bub: `https://api.github.com/repos/elipsitz/gamebub/releases/latest`
//!   and downloads `.uf2` assets from `https://github.com/elipsitz/gamebub/releases/download/`.

use crate::{ErrorCode, TauError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tau_core::{device::DeviceKind, firmware as fw_core};

pub const POCKET_FIRMWARE_DETAILS_URL: &str =
    "https://www.analogue.co/support/pocket/firmware/latest/details";
pub const GAMEBUB_FIRMWARE_RELEASE_URL: &str =
    "https://api.github.com/repos/elipsitz/gamebub/releases/latest";

const CACHE_TTL_SECS: u64 = 3600;
const META_LIMIT: u64 = 2 * 1024 * 1024;
const FIRMWARE_LIMIT: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareRelease {
    pub device: DeviceKind,
    pub version: String,
    pub published_at: String,
    pub file_name: String,
    pub download_url: String,
    pub bytes: Option<u64>,
    pub size_label: String,
    pub md5: Option<String>,
    pub page_url: String,
    pub release_notes: String,
    pub fetched_at: u64,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareDownload {
    pub path: String,
    pub file_name: String,
    pub bytes: u64,
    pub md5: String,
    pub verified: bool,
}

fn err(code: ErrorCode, msg: impl Into<String>) -> TauError {
    TauError {
        code,
        message: msg.into(),
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(180)))
        .user_agent(concat!("Tau-Omega/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn cache_file(cache_dir: &Path, device: DeviceKind) -> std::path::PathBuf {
    let name = match device {
        DeviceKind::AnaloguePocket => "pocket-latest.json",
        DeviceKind::GameBub => "gamebub-latest.json",
    };
    cache_dir.join("firmware").join(name)
}

fn read_cached(path: &Path) -> Option<FirmwareRelease> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_cached(path: &Path, release: &FirmwareRelease) {
    if let Some(p) = path.parent() {
        let _ = fs::create_dir_all(p);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(release) {
        let _ = fs::write(path, bytes);
    }
}

fn parse_pocket_details(v: &Value, now: u64) -> Result<FirmwareRelease, TauError> {
    let version = v
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let download_url = v
        .get("download_url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let file_name = v
        .get("file_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if version.is_empty() || download_url.is_empty() || file_name.is_empty() {
        return Err(err(
            ErrorCode::Json,
            "Analogue firmware response was missing version or download fields.",
        ));
    }
    Ok(FirmwareRelease {
        device: DeviceKind::AnaloguePocket,
        version,
        published_at: v
            .get("published_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        file_name,
        download_url,
        bytes: None,
        size_label: v
            .get("file_size")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        md5: v
            .get("md5")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        page_url: v
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("https://www.analogue.co/support/pocket/firmware/latest")
            .to_string(),
        release_notes: v
            .get("release_notes_markdown")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        fetched_at: now,
        from_cache: false,
    })
}

fn parse_gamebub_release(v: &Value, now: u64) -> Result<FirmwareRelease, TauError> {
    let tag = v
        .get("tag_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let assets = v.get("assets").and_then(Value::as_array);
    let uf2 = assets.and_then(|arr| {
        arr.iter().find(|a| {
            a.get("name")
                .and_then(Value::as_str)
                .is_some_and(|n| n.to_ascii_lowercase().ends_with(".uf2"))
        })
    });
    let Some(uf2) = uf2 else {
        return Err(err(
            ErrorCode::NotFound,
            "Latest Game Bub release has no .uf2 firmware asset.",
        ));
    };
    let file_name = uf2
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let download_url = uf2
        .get("browser_download_url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let bytes = uf2.get("size").and_then(Value::as_u64);
    let size_label = bytes
        .map(|b| format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)))
        .unwrap_or_default();
    Ok(FirmwareRelease {
        device: DeviceKind::GameBub,
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        published_at: v
            .get("published_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        file_name,
        download_url,
        bytes,
        size_label,
        md5: None,
        page_url: v
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or("https://github.com/elipsitz/gamebub/releases")
            .to_string(),
        release_notes: v
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        fetched_at: now,
        from_cache: false,
    })
}

/// Fetches the latest official firmware metadata for `device`, using a 1-hour cache unless `force` is set.
pub fn fetch_latest_firmware(
    device: DeviceKind,
    cache_dir: &Path,
    force: bool,
) -> Result<FirmwareRelease, TauError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let cf = cache_file(cache_dir, device);
    let cached = read_cached(&cf);
    if !force
        && let Some(mut c) = cached.clone()
        && now.saturating_sub(c.fetched_at) < CACHE_TTL_SECS
    {
        c.from_cache = true;
        return Ok(c);
    }

    let url = match device {
        DeviceKind::AnaloguePocket => POCKET_FIRMWARE_DETAILS_URL,
        DeviceKind::GameBub => GAMEBUB_FIRMWARE_RELEASE_URL,
    };

    let req = agent().get(url);
    let req = if device == DeviceKind::GameBub {
        req.header("Accept", "application/vnd.github+json")
    } else {
        req.header("Accept", "application/json")
    };

    let fetched = req.call().and_then(|mut resp| {
        resp.body_mut()
            .with_config()
            .limit(META_LIMIT)
            .read_to_vec()
    });

    let body = match fetched {
        Ok(b) => b,
        Err(e) => {
            if let Some(mut c) = cached {
                c.from_cache = true;
                return Ok(c);
            }
            return Err(err(
                ErrorCode::Io,
                format!("Could not check for {} firmware: {e}", device.label()),
            ));
        }
    };

    let v: Value = serde_json::from_slice(&body)
        .map_err(|e| err(ErrorCode::Json, format!("Invalid firmware metadata: {e}")))?;
    let release = match device {
        DeviceKind::AnaloguePocket => parse_pocket_details(&v, now)?,
        DeviceKind::GameBub => parse_gamebub_release(&v, now)?,
    };
    write_cached(&cf, &release);
    Ok(release)
}

fn check_download_url(device: DeviceKind, url: &str) -> Result<(), TauError> {
    let ok = match device {
        DeviceKind::AnaloguePocket => {
            url.starts_with("https://assets.analogue.co/firmware/")
                || url.starts_with("https://www.analogue.co/support/pocket/firmware/")
        }
        DeviceKind::GameBub => {
            url.starts_with("https://github.com/elipsitz/gamebub/releases/download/")
        }
    };
    if !ok {
        return Err(err(
            ErrorCode::InvalidPathReference,
            format!(
                "Refused firmware download URL outside the official {} host.",
                device.label()
            ),
        ));
    }
    Ok(())
}

/// Downloads `release` into `cache_dir/firmware/downloads/<file_name>` and verifies MD5 (Pocket) or UF2 magic (Game Bub).
pub fn download_firmware(
    device: DeviceKind,
    release: &FirmwareRelease,
    cache_dir: &Path,
) -> Result<FirmwareDownload, TauError> {
    check_download_url(device, &release.download_url)?;
    let safe_name = Path::new(&release.file_name)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| {
            err(
                ErrorCode::InvalidPathReference,
                "Invalid firmware filename.",
            )
        })?;
    let dl_dir = cache_dir.join("firmware").join("downloads");
    fs::create_dir_all(&dl_dir)?;
    let dest = dl_dir.join(safe_name);

    // Reuse an already-downloaded file if it passes verification.
    if dest.is_file()
        && let Ok(existing) = fs::read(&dest)
        && !existing.is_empty()
    {
        let md5 = fw_core::md5_hex(&existing);
        let ok = match (&release.md5, device) {
            (Some(exp), _) => md5.eq_ignore_ascii_case(exp.trim()),
            (None, DeviceKind::GameBub) => {
                fw_core::validate_uf2(&existing).is_ok()
                    && release.bytes.is_none_or(|b| b == existing.len() as u64)
            }
            (None, DeviceKind::AnaloguePocket) => false,
        };
        if ok {
            return Ok(FirmwareDownload {
                path: dest.to_string_lossy().into_owned(),
                file_name: safe_name.to_string(),
                bytes: existing.len() as u64,
                md5,
                verified: release.md5.is_some() || device == DeviceKind::GameBub,
            });
        }
    }

    let mut resp = agent().get(&release.download_url).call().map_err(|e| {
        err(
            ErrorCode::Io,
            format!("Could not download {} firmware: {e}", device.label()),
        )
    })?;
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(FIRMWARE_LIMIT)
        .read_to_vec()
        .map_err(|e| err(ErrorCode::Io, format!("Download interrupted: {e}")))?;

    let md5 = fw_core::md5_hex(&bytes);
    let mut verified = false;
    if let Some(exp) = release.md5.as_deref().filter(|s| !s.trim().is_empty()) {
        if !md5.eq_ignore_ascii_case(exp.trim()) {
            return Err(err(
                ErrorCode::InvalidPathReference,
                format!(
                    "Downloaded firmware checksum mismatch (expected {}, got {}).",
                    exp.trim(),
                    md5
                ),
            ));
        }
        verified = true;
    }
    if device == DeviceKind::GameBub {
        fw_core::validate_uf2(&bytes)?;
        verified = true;
    }

    let part = dl_dir.join(format!("{safe_name}.part"));
    fs::write(&part, &bytes)?;
    fs::rename(&part, &dest)?;

    Ok(FirmwareDownload {
        path: dest.to_string_lossy().into_owned(),
        file_name: safe_name.to_string(),
        bytes: bytes.len() as u64,
        md5,
        verified,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_unofficial_firmware_urls() {
        assert!(
            check_download_url(DeviceKind::AnaloguePocket, "https://evil.example/fw.bin").is_err()
        );
        assert!(
            check_download_url(
                DeviceKind::AnaloguePocket,
                "https://assets.analogue.co/firmware/abc/pocket_firmware_2_7.bin"
            )
            .is_ok()
        );
        assert!(check_download_url(DeviceKind::GameBub, "https://evil.example/fw.uf2").is_err());
        assert!(
            check_download_url(
                DeviceKind::GameBub,
                "https://github.com/elipsitz/gamebub/releases/download/v1.0.3/gamebub-rev4_v1.0.3.uf2"
            )
            .is_ok()
        );
    }
}
