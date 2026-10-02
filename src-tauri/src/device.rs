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

/// The real Analogue Pocket's USB descriptor, read from a real device on
/// 2026-09-26 (not documented by Analogue): vendor 0x04D8 (Microchip
/// Technology), product 0xCA19, product name "Analogue Pocket".
#[allow(dead_code)] // only used on macOS; unit-tested everywhere
const POCKET_USB_VENDOR: i64 = 0x04D8;
#[allow(dead_code)]
const POCKET_USB_PRODUCT: i64 = 0xCA19;

/// One `Key: value` field from `diskutil info` output.
#[allow(dead_code)] // only called on macOS; unit-tested everywhere
fn diskutil_field(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix(name)
            .and_then(|rest| rest.trim().strip_prefix(':'))
            .map(|value| value.trim().to_string())
    })
}

/// Scans `ioreg -l -w0` tree text for `whole_disk` (for example `"disk6"`) and
/// reports whether the nearest preceding `IOUSBHostDevice` is the real Pocket.
/// The tree is depth-first, so the last device header seen before a disk's own
/// properties is that disk's real parent; each new header resets the match, so
/// a hub or unrelated sibling is never mistaken for it. The default IOService
/// plane is required (`-p IOUSB` stops above the storage nodes). ioreg's tree
/// characters (`|`, `+-o`) are not whitespace, so lines are matched with
/// `contains`/`split_once`, never `trim()` plus equality -- the bug found by
/// testing against a live Pocket.
#[allow(dead_code)] // only called on macOS; unit-tested everywhere
pub fn usb_disk_is_pocket(usb_tree_text: &str, whole_disk: &str) -> bool {
    let bsd_unit = whole_disk.trim_start_matches("disk");
    let mut pending_vendor: Option<i64> = None;
    let mut pending_product: Option<String> = None;
    let is_pocket = |vendor: Option<i64>, product: &Option<String>| {
        vendor == Some(POCKET_USB_VENDOR) && product.as_deref() == Some("Analogue Pocket")
    };
    for line in usb_tree_text.lines() {
        if line.contains("<class IOUSBHostDevice") {
            pending_vendor = None;
            pending_product = None;
        } else if let Some((_, value)) = line.split_once("\"idVendor\" = ") {
            pending_vendor = value.trim().parse().ok();
        } else if let Some((_, value)) = line.split_once("\"USB Product Name\" = ") {
            pending_product = Some(value.trim().trim_matches('"').to_string());
        } else if let Some((_, value)) = line.split_once("\"BSD Name\" = ")
            && value.trim().trim_matches('"') == whole_disk
        {
            return is_pocket(pending_vendor, &pending_product);
        } else if let Some((_, value)) = line.split_once("\"BSD Unit\" = ")
            && value.trim() == bsd_unit
        {
            return is_pocket(pending_vendor, &pending_product);
        }
    }
    false
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
    let text = match std::process::Command::new("diskutil").arg("info").arg(path).output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).into_owned(),
        _ => return ConnectionInfo { kind: ConnectionKind::Unknown, detail: "diskutil unavailable".into() },
    };
    let mut info = parse_diskutil(&text);
    // The name-based reading above is a guess. For USB disks, confirm it (or
    // catch a Pocket whose name differs) against the real USB descriptor.
    if diskutil_field(&text, "Protocol").as_deref() == Some("USB")
        && let Some(whole_disk) = diskutil_field(&text, "Part of Whole").or_else(|| diskutil_field(&text, "Device Identifier"))
        && let Ok(tree) = std::process::Command::new("ioreg").args(["-l", "-w0"]).output()
    {
        if usb_disk_is_pocket(&String::from_utf8_lossy(&tree.stdout), &whole_disk) {
            info.kind = ConnectionKind::DirectUsb;
            info.detail = format!("{} - USB {:04X}:{:04X}", info.detail, POCKET_USB_VENDOR, POCKET_USB_PRODUCT);
        } else if info.kind == ConnectionKind::DirectUsb {
            // Name said Pocket but the descriptor does not: do not warn on a guess.
            info.kind = ConnectionKind::CardReader;
        }
    }
    info
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
    #[test]
    fn matches_the_real_confirmed_pocket_descriptor() {
        // A trimmed real excerpt of `ioreg` output captured 2026-09-26 with a
        // real Pocket (USB SD Access on) and a real CalDigit card reader both
        // attached -- the real contrasting case, not an invented fixture.
        let tree = r#"
+-o Analogue Pocket@02100000  <class IOUSBHostDevice, id 0x100017a33, registered>
    {
      "idProduct" = 51737
      "USB Product Name" = "Analogue Pocket"
      "idVendor" = 1240
      "USB Vendor Name" = "Microchip Technology Inc."
    }
    +-o IOUSBHostInterface@0
      +-o IOMedia
        {
          "BSD Name" = "disk6"
          "BSD Unit" = 6
        }
+-o Card Reader@22800000  <class IOUSBHostDevice, id 0x10000cb47, registered>
    {
      "idProduct" = 1880
      "USB Product Name" = "Card Reader"
      "idVendor" = 8584
      "USB Vendor Name" = "CalDigit"
    }
    +-o IOUSBHostInterface@0
      +-o IOMedia
        {
          "BSD Name" = "disk7"
          "BSD Unit" = 7
        }
"#;
        assert!(usb_disk_is_pocket(tree, "disk6"));
        assert!(!usb_disk_is_pocket(tree, "disk7"));
        assert!(!usb_disk_is_pocket(tree, "disk8"));
    }
    #[test]
    fn diskutil_fields_are_read_by_name() {
        let text = "   Protocol:                  USB\n   Part of Whole:             disk6\n";
        assert_eq!(diskutil_field(text, "Protocol").as_deref(), Some("USB"));
        assert_eq!(diskutil_field(text, "Part of Whole").as_deref(), Some("disk6"));
        assert_eq!(diskutil_field(text, "Nope"), None);
    }
}

/// `#[ignore]`d: they need this exact machine's attached hardware. Run with
/// `cargo test --features tau-core/serde -- --ignored` when a Pocket (mounted
/// as `/Volumes/Pock`) and a different USB device (`/Volumes/DSPICO`) are
/// attached, to re-confirm detection has not regressed. Kept as permanent
/// regression coverage for the real parsing bug described on `usb_disk_is_pocket`.
#[cfg(all(test, target_os = "macos"))]
mod real_hardware_checks {
    use super::*;

    #[test]
    #[ignore]
    fn real_pocket_is_detected_right_now() {
        assert_eq!(detect_connection(Path::new("/Volumes/Pock")).kind, ConnectionKind::DirectUsb);
    }

    /// A Raspberry Pi Pico in mass-storage mode: the real contrasting device.
    #[test]
    #[ignore]
    fn a_different_real_usb_device_is_not_detected_as_pocket() {
        assert_ne!(detect_connection(Path::new("/Volumes/DSPICO")).kind, ConnectionKind::DirectUsb);
    }
}
