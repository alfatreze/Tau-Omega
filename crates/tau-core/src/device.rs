//! Multi-device model (`D-026`): identifies which FPGA handheld a microSD card belongs to.
//!
//! Because macOS and Windows filesystems are case-insensitive (`Cores/` and `cores/` resolve to the same directory),
//! card detection inspects the structure of `core.json` files and device-specific folders rather than folder casing alone.

use serde_json::Value;
use std::{fs, path::Path};

/// Supported handheld targets in Tau Omega.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum DeviceKind {
    #[default]
    AnaloguePocket,
    GameBub,
}

/// How a handheld receives system firmware updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum FirmwareMethod {
    /// Placed as `pocket_firmware_*.bin` on the microSD card root; installed on next boot.
    SdCardRoot,
    /// Flashed as a `.uf2` file over USB to the bootloader drive (`GAME BUB` / `GameBubDock`).
    UsbUf2,
}

/// Global per-system descriptor shared by Marketplace, Firmware, and card inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SystemProfile {
    pub id: DeviceKind,
    pub name: &'static str,
    pub short_name: &'static str,
    pub beta: bool,
    pub firmware_method: FirmwareMethod,
    pub supports_music_library: bool,
}

impl DeviceKind {
    pub const ALL: &'static [DeviceKind] = &[DeviceKind::AnaloguePocket, DeviceKind::GameBub];

    pub fn label(self) -> &'static str {
        self.profile().name
    }

    pub fn profile(self) -> SystemProfile {
        match self {
            DeviceKind::AnaloguePocket => SystemProfile {
                id: DeviceKind::AnaloguePocket,
                name: "Analogue Pocket",
                short_name: "Pocket",
                beta: false,
                firmware_method: FirmwareMethod::SdCardRoot,
                supports_music_library: true,
            },
            DeviceKind::GameBub => SystemProfile {
                id: DeviceKind::GameBub,
                name: "Game Bub",
                short_name: "Game Bub",
                beta: true,
                firmware_method: FirmwareMethod::UsbUf2,
                supports_music_library: false,
            },
        }
    }
}

/// Returns every handheld system supported by Tau Omega in display order.
pub fn all_systems() -> Vec<SystemProfile> {
    DeviceKind::ALL.iter().map(|d| d.profile()).collect()
}

/// Result of inspecting a card root to determine its target device.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CardDeviceInfo {
    /// Detected device, or `None` for an empty or unrecognised card.
    pub device: Option<DeviceKind>,
    pub reason: String,
    pub pocket_cores: usize,
    pub gamebub_cores: usize,
}

fn classify_core_json(v: &Value) -> Option<DeviceKind> {
    if v.get("core").is_some() {
        Some(DeviceKind::AnaloguePocket)
    } else if v.get("bitstreams").is_some() || v.get("min_firmware").is_some() {
        Some(DeviceKind::GameBub)
    } else {
        None
    }
}

fn scan_cores_dir(dir: &Path) -> (usize, usize) {
    let mut pocket = 0usize;
    let mut gamebub = 0usize;
    let Ok(entries) = fs::read_dir(dir) else {
        return (0, 0);
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let cj = e.path().join("core.json");
        let Ok(bytes) = fs::read(&cj) else { continue };
        let Ok(v) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        match classify_core_json(&v) {
            Some(DeviceKind::AnaloguePocket) => pocket += 1,
            Some(DeviceKind::GameBub) => gamebub += 1,
            None => {}
        }
    }
    (pocket, gamebub)
}

/// Inspects a card root (read-only) to determine whether it is an Analogue Pocket card or a Game Bub card.
pub fn detect_card(card: &Path) -> CardDeviceInfo {
    // Check both `Cores` and `cores`, avoiding double-counting on case-insensitive filesystems.
    let upper = card.join("Cores");
    let lower = card.join("cores");
    let same_dir = match (fs::canonicalize(&upper), fs::canonicalize(&lower)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };

    let (mut pocket_cores, mut gamebub_cores) = scan_cores_dir(&upper);
    if !same_dir {
        let (p2, g2) = scan_cores_dir(&lower);
        pocket_cores += p2;
        gamebub_cores += g2;
    }

    if gamebub_cores > 0 && pocket_cores == 0 {
        return CardDeviceInfo {
            device: Some(DeviceKind::GameBub),
            reason: format!("Found {gamebub_cores} Game Bub core(s) under /cores/."),
            pocket_cores,
            gamebub_cores,
        };
    }
    if pocket_cores > 0 && gamebub_cores == 0 {
        return CardDeviceInfo {
            device: Some(DeviceKind::AnaloguePocket),
            reason: format!("Found {pocket_cores} Analogue Pocket core(s) under /Cores/."),
            pocket_cores,
            gamebub_cores,
        };
    }
    if pocket_cores > 0 && gamebub_cores > 0 {
        let device = if gamebub_cores > pocket_cores {
            DeviceKind::GameBub
        } else {
            DeviceKind::AnaloguePocket
        };
        return CardDeviceInfo {
            device: Some(device),
            reason: format!(
                "Card contains both Analogue Pocket ({pocket_cores}) and Game Bub ({gamebub_cores}) cores."
            ),
            pocket_cores,
            gamebub_cores,
        };
    }

    // No cores installed yet: check structural markers.
    let has_pocket_dirs = card.join("Assets").is_dir()
        || card.join("Platforms").is_dir()
        || card.join("System").join("Pocket").is_dir();
    if has_pocket_dirs {
        return CardDeviceInfo {
            device: Some(DeviceKind::AnaloguePocket),
            reason: "Found Analogue Pocket folders (Assets/Platforms).".into(),
            pocket_cores: 0,
            gamebub_cores: 0,
        };
    }

    let sys = card.join("system");
    let has_gb_bios = [
        "gameboy.bios-dmg.bin",
        "gameboy.bios-cgb.bin",
        "gba.bios.bin",
    ]
    .iter()
    .any(|f| sys.join(f).is_file());
    if has_gb_bios {
        return CardDeviceInfo {
            device: Some(DeviceKind::GameBub),
            reason: "Found Game Bub BIOS files under /system/.".into(),
            pocket_cores: 0,
            gamebub_cores: 0,
        };
    }

    CardDeviceInfo {
        device: None,
        reason: "No Analogue Pocket or Game Bub files found on this card yet.".into(),
        pocket_cores: 0,
        gamebub_cores: 0,
    }
}
