//! Host-side device knowledge: which mounted volumes look like a Pocket card,
//! and whether a card is connected through the Pocket's own USB mode (slow)
//! or a card reader. Everything here is best-effort and read-only; when the
//! OS will not say, the answer is `Unknown` and the UI falls back to measuring
//! real transfer speed and to a per-card setting.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionKind {
    /// Plugged in as the Pocket itself (its USB storage mode): slow transfers.
    DirectUsb,
    /// A USB or built-in card reader: fast transfers.
    CardReader,
    Unknown,
}

#[derive(Serialize, Clone, Debug)]
pub struct ConnectionInfo {
    pub kind: ConnectionKind,
    /// What the OS reported (device name / bus), for the tooltip and support.
    pub detail: String,
}

/// Whether a device name/product string is the Analogue Pocket itself.
fn looks_like_pocket(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("pocket") || n.contains("analogue")
}

fn classify(name: &str, bus: &str) -> ConnectionInfo {
    let kind = if looks_like_pocket(name) {
        ConnectionKind::DirectUsb
    } else if bus.eq_ignore_ascii_case("usb") || bus.to_ascii_lowercase().contains("sd") {
        ConnectionKind::CardReader
    } else {
        ConnectionKind::Unknown
    };
    ConnectionInfo { kind, detail: format!("{} ({})", name.trim(), bus.trim()) }
}

/// Parses `diskutil info <volume>` output (macOS).
#[allow(dead_code)] // only called on macOS; unit-tested everywhere
pub fn parse_diskutil(text: &str) -> ConnectionInfo {
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(key).map(|v| v.trim_start_matches(':').trim().to_string()))
            .unwrap_or_default()
    };
    let name = format!("{} {}", field("Device / Media Name"), field("Volume Name"));
    classify(&name, &field("Protocol"))
}

/// Parses `FriendlyName|BusType` output from PowerShell `Get-Disk` (Windows).
#[allow(dead_code)] // only called on Windows; unit-tested everywhere
pub fn parse_windows_disk(text: &str) -> ConnectionInfo {
    match text.trim().split_once('|') {
        Some((name, bus)) => classify(name, bus),
        None => ConnectionInfo { kind: ConnectionKind::Unknown, detail: text.trim().to_string() },
    }
}

/// Finds the block device holding `path` from `/proc/mounts` text (Linux).
#[allow(dead_code)] // only called on Linux; unit-tested everywhere
pub fn linux_device_for(mounts: &str, path: &Path) -> Option<String> {
    mounts
        .lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            Some((parts.next()?.to_string(), parts.next()?.replace("\\040", " ")))
        })
        .filter(|(dev, mount)| dev.starts_with("/dev/") && path.starts_with(mount))
        .max_by_key(|(_, mount)| mount.len())
        .map(|(dev, _)| dev)
}

#[cfg(target_os = "linux")]
fn detect(path: &Path) -> ConnectionInfo {
    let unknown = |why: &str| ConnectionInfo { kind: ConnectionKind::Unknown, detail: why.into() };
    let Ok(mounts) = std::fs::read_to_string("/proc/mounts") else { return unknown("no /proc/mounts") };
    let Some(dev) = linux_device_for(&mounts, path) else { return unknown("volume not found") };
    let name = dev.trim_start_matches("/dev/");
    if name.starts_with("mmcblk") {
        return classify("Built-in SD card reader", "sd");
    }
    // /sys/class/block/sdb1 resolves into the USB device tree; the USB device
    // directory (a few levels up) carries `manufacturer` and `product`.
    let Ok(mut dir) = std::fs::canonicalize(format!("/sys/class/block/{name}")) else { return unknown("no sysfs entry") };
    for _ in 0..8 {
        let read = |f: &str| std::fs::read_to_string(dir.join(f)).map(|s| s.trim().to_string()).ok();
        if let Some(product) = read("product") {
            let maker = read("manufacturer").unwrap_or_default();
            return classify(&format!("{maker} {product}"), "usb");
        }
        if !dir.pop() {
            break;
        }
    }
    unknown("not a USB device")
}

#[cfg(target_os = "macos")]
fn detect(path: &Path) -> ConnectionInfo {
    match std::process::Command::new("diskutil").arg("info").arg(path).output() {
        Ok(out) if out.status.success() => parse_diskutil(&String::from_utf8_lossy(&out.stdout)),
        _ => ConnectionInfo { kind: ConnectionKind::Unknown, detail: "diskutil unavailable".into() },
    }
}

#[cfg(target_os = "windows")]
fn detect(path: &Path) -> ConnectionInfo {
    let letter = path.to_string_lossy().chars().next().filter(char::is_ascii_alphabetic);
    let Some(letter) = letter else {
        return ConnectionInfo { kind: ConnectionKind::Unknown, detail: "no drive letter".into() };
    };
    let script = format!("$d = Get-Partition -DriveLetter {letter} | Get-Disk; \"$($d.FriendlyName)|$($d.BusType)\"");
    match std::process::Command::new("powershell").args(["-NoProfile", "-Command", &script]).output() {
        Ok(out) if out.status.success() => parse_windows_disk(&String::from_utf8_lossy(&out.stdout)),
        _ => ConnectionInfo { kind: ConnectionKind::Unknown, detail: "PowerShell unavailable".into() },
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn detect(_path: &Path) -> ConnectionInfo {
    ConnectionInfo { kind: ConnectionKind::Unknown, detail: "unsupported OS".into() }
}

pub fn detect_connection(path: &Path) -> ConnectionInfo {
    detect(path)
}

/// Mounted volumes that look like a Pocket card: a top-level folder holding
/// both `Cores` and `Assets`. A plain directory check, not a full inspection.
pub fn mounted_cards() -> Vec<String> {
    let mut roots: Vec<PathBuf> = Vec::new();
    #[cfg(target_os = "macos")]
    if let Ok(entries) = std::fs::read_dir("/Volumes") {
        roots.extend(entries.flatten().map(|e| e.path()));
    }
    #[cfg(target_os = "linux")]
    {
        let user = std::env::var("USER").unwrap_or_default();
        for base in [format!("/media/{user}"), format!("/run/media/{user}"), "/mnt".to_string()] {
            if let Ok(entries) = std::fs::read_dir(base) {
                roots.extend(entries.flatten().map(|e| e.path()));
            }
        }
    }
    #[cfg(target_os = "windows")]
    roots.extend(('D'..='Z').map(|l| PathBuf::from(format!("{l}:\\"))));
    let mut found: Vec<String> = roots
        .into_iter()
        .filter(|p| p.join("Cores").is_dir() && p.join("Assets").is_dir())
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pocket_itself_is_direct_usb() {
        let text = "   Device / Media Name:       Analogue Pocket\n   Volume Name:               POCKET\n   Protocol:                  USB\n";
        assert_eq!(parse_diskutil(text).kind, ConnectionKind::DirectUsb);
    }
    #[test]
    fn a_generic_usb_reader_is_a_card_reader() {
        let text = "   Device / Media Name:       Generic STORAGE DEVICE\n   Volume Name:               SD\n   Protocol:                  USB\n";
        assert_eq!(parse_diskutil(text).kind, ConnectionKind::CardReader);
    }
    #[test]
    fn unrecognised_output_is_unknown_not_a_guess() {
        assert_eq!(parse_diskutil("nonsense").kind, ConnectionKind::Unknown);
        assert_eq!(parse_windows_disk("garbage").kind, ConnectionKind::Unknown);
    }
    #[test]
    fn windows_disk_lines_are_classified() {
        assert_eq!(parse_windows_disk("Analogue Pocket|USB\n").kind, ConnectionKind::DirectUsb);
        assert_eq!(parse_windows_disk("Mass Storage Device|USB").kind, ConnectionKind::CardReader);
        assert_eq!(parse_windows_disk("Samsung SSD|NVMe").kind, ConnectionKind::Unknown);
    }
    #[test]
    fn the_longest_matching_mount_wins_on_linux() {
        let mounts = "/dev/sda1 / ext4 rw 0 0\n/dev/sdb1 /media/u/POCKET\\040CARD vfat rw 0 0\n";
        assert_eq!(
            linux_device_for(mounts, Path::new("/media/u/POCKET CARD/Assets")).as_deref(),
            Some("/dev/sdb1")
        );
        assert_eq!(linux_device_for(mounts, Path::new("/home/u")).as_deref(), Some("/dev/sda1"));
    }
}
