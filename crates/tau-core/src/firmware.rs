//! Per-system firmware detection, verification, and safe installation (`Analogue Pocket` + `Game Bub`).
//!
//! - **Analogue Pocket:** inspects `pocket_firmware*.bin` on the microSD card root, verifies downloaded
//!   firmware against Analogue's published MD5, backs up and removes older `.bin` files on the root,
//!   and writes the new `.bin` atomically with read-back verification.
//! - **Game Bub:** inspects installed cores' `min_firmware` requirements, detects mounted USB UF2
//!   bootloader volumes (`GAME BUB` / `GameBubDock`), validates `.uf2` block headers, and copies a
//!   reviewed `.uf2` file to the bootloader drive.

use crate::{
    ErrorCode, TauError,
    device::{DeviceKind, FirmwareMethod},
    gamebub, sync,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const UF2_MAGIC_START0: u32 = 0x0A32_4655; // "UF2\n"
const UF2_MAGIC_START1: u32 = 0x9E5D_5157;
const UF2_MAGIC_END: u32 = 0x0AB1_6F30;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CardFirmwareFile {
    pub file_name: String,
    pub version: Option<String>,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Uf2Drive {
    pub label: String,
    pub path: String,
    pub info: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FirmwareCardStatus {
    pub device: DeviceKind,
    pub method: FirmwareMethod,
    /// Firmware files currently sitting on the SD card root (Analogue Pocket).
    pub card_files: Vec<CardFirmwareFile>,
    /// Highest `min_firmware` required by installed cores on the card (Game Bub).
    pub required_min_firmware: Option<String>,
    /// Mounted USB UF2 bootloader drives currently connected (Game Bub).
    pub uf2_drives: Vec<Uf2Drive>,
    /// Plain-language warnings (e.g. multiple Pocket firmware `.bin` files on the card root).
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FirmwareInstallPlan {
    pub id: String,
    pub device: DeviceKind,
    pub target_path: String,
    pub file_name: String,
    pub bytes: u64,
    pub md5: String,
    pub md5_verified: bool,
    pub replaces: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FirmwareInstallReport {
    pub device: DeviceKind,
    pub destination: String,
    pub bytes_written: u64,
    pub removed_old: Vec<String>,
    pub backup_dir: Option<PathBuf>,
}

fn bad(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidPathReference, msg)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Zero-dependency RFC 1321 MD5 digest (lowercase hex), used to verify Analogue's published `.bin` MD5.
pub fn md5_hex(data: &[u8]) -> String {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76a_a478,
        0xe8c7_b756,
        0x2420_70db,
        0xc1bd_ceee,
        0xf57c_0faf,
        0x4787_c62a,
        0xa830_4613,
        0xfd46_9501,
        0x6980_98d8,
        0x8b44_f7af,
        0xffff_5bb1,
        0x895c_d7be,
        0x6b90_1122,
        0xfd98_7193,
        0xa679_438e,
        0x49b4_0821,
        0xf61e_2562,
        0xc040_b340,
        0x265e_5a51,
        0xe9b6_c7aa,
        0xd62f_105d,
        0x0244_1453,
        0xd8a1_e681,
        0xe7d3_fbc8,
        0x21e1_cde6,
        0xc337_07d6,
        0xf4d5_0d87,
        0x455a_14ed,
        0xa9e3_e905,
        0xfcef_a3f8,
        0x676f_02d9,
        0x8d2a_4c8a,
        0xfffa_3942,
        0x8771_f681,
        0x6d9d_6122,
        0xfde5_380c,
        0xa4be_ea44,
        0x4bde_cfa9,
        0xf6bb_4b60,
        0xbebf_bc70,
        0x289b_7ec6,
        0xeaa1_27fa,
        0xd4ef_3085,
        0x0488_1d05,
        0xd9d4_d039,
        0xe6db_99e5,
        0x1fa2_7cf8,
        0xc4ac_5665,
        0xf429_2244,
        0x432a_ff97,
        0xab94_23a7,
        0xfc93_a039,
        0x655b_59c3,
        0x8f0c_cc92,
        0xffef_f47d,
        0x8584_5dd1,
        0x6fa8_7e4f,
        0xfe2c_e6e0,
        0xa301_4314,
        0x4e08_11a1,
        0xf753_7e82,
        0xbd3a_f235,
        0x2ad7_d2bb,
        0xeb86_d391,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = Vec::with_capacity(data.len() + 72);
    msg.extend_from_slice(data);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    let (mut a0, mut b0, mut c0, mut d0) = (
        0x6745_2301u32,
        0xefcd_ab89u32,
        0x98ba_dcfeu32,
        0x1032_5476u32,
    );

    for chunk in msg.as_chunks::<64>().0 {
        let mut m = [0u32; 16];
        for (i, w) in chunk.as_chunks::<4>().0.iter().enumerate() {
            m[i] = u32::from_le_bytes(*w);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | ((!b) & d), i),
                16..=31 => ((d & b) | ((!d) & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | (!d)), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(K[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut digest = [0u8; 16];
    digest[0..4].copy_from_slice(&a0.to_le_bytes());
    digest[4..8].copy_from_slice(&b0.to_le_bytes());
    digest[8..12].copy_from_slice(&c0.to_le_bytes());
    digest[12..16].copy_from_slice(&d0.to_le_bytes());
    let mut out = String::with_capacity(32);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Validates that `bytes` is a non-empty, 512-byte-aligned UF2 image with valid start/end magic words.
pub fn validate_uf2(bytes: &[u8]) -> Result<(), TauError> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(512) {
        return Err(bad(
            "This file does not have a valid UF2 size (must be a multiple of 512 bytes).",
        ));
    }
    let m0 = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let m1 = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let mend = u32::from_le_bytes(bytes[508..512].try_into().unwrap());
    if m0 != UF2_MAGIC_START0 || m1 != UF2_MAGIC_START1 || mend != UF2_MAGIC_END {
        return Err(bad("This file does not start with a valid UF2 header."));
    }
    Ok(())
}

/// Extracts a human-readable version from an Analogue Pocket firmware filename
/// (e.g. `pocket_firmware_2_7.bin` → `"2.7"`, `pocket_firmware_2_2_beta.bin` → `"2.2-beta"`).
pub fn parse_pocket_firmware_version(file_name: &str) -> Option<String> {
    let lower = file_name.to_ascii_lowercase();
    let stem = lower
        .strip_prefix("pocket_firmware_")?
        .strip_suffix(".bin")?;
    if stem.is_empty() {
        return None;
    }
    let parts: Vec<&str> = stem.split('_').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    let mut nums = Vec::new();
    let mut suffix = Vec::new();
    for p in parts {
        if suffix.is_empty() && p.chars().all(|c| c.is_ascii_digit()) {
            nums.push(p);
        } else {
            suffix.push(p);
        }
    }
    if nums.is_empty() {
        return Some(stem.to_string());
    }
    let mut v = nums.join(".");
    if !suffix.is_empty() {
        v.push('-');
        v.push_str(&suffix.join("-"));
    }
    Some(v)
}

fn is_pocket_firmware_bin(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("pocket_firmware") && lower.ends_with(".bin")
}

fn safe_firmware_filename(device: DeviceKind, name: &str) -> Result<String, TauError> {
    let clean = name.trim();
    if clean.is_empty()
        || clean.contains('/')
        || clean.contains('\\')
        || clean.contains('\0')
        || clean == "."
        || clean == ".."
    {
        return Err(bad(format!("Invalid firmware filename: '{name}'.")));
    }
    match device {
        DeviceKind::AnaloguePocket => {
            if !is_pocket_firmware_bin(clean) {
                return Err(bad(
                    "Analogue Pocket firmware must be named pocket_firmware_*.bin.",
                ));
            }
        }
        DeviceKind::GameBub => {
            if !clean.to_ascii_lowercase().ends_with(".uf2") {
                return Err(bad("Game Bub firmware must be a .uf2 file."));
            }
        }
    }
    Ok(clean.to_string())
}

fn compare_semver_like(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u32> {
        s.trim_start_matches(['v', 'V'])
            .split(['.', '-', '_'])
            .filter_map(|p| p.parse::<u32>().ok())
            .collect()
    };
    parse(a).cmp(&parse(b))
}

/// Scans candidate mount directories for Game Bub UF2 bootloader drives (`GAME BUB` / `GameBubDock`).
pub fn detect_uf2_drives_in(roots: &[PathBuf]) -> Vec<Uf2Drive> {
    let mut out = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for e in entries.flatten() {
            let label = e.file_name().to_string_lossy().into_owned();
            let upper = label.to_ascii_uppercase();
            if upper != "GAME BUB" && upper != "GAMEBUBDOCK" {
                continue;
            }
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let info = fs::read_to_string(p.join("INFO_UF2.TXT"))
                .ok()
                .map(|s| s.lines().take(3).collect::<Vec<_>>().join(" · "));
            out.push(Uf2Drive {
                label,
                path: p.to_string_lossy().into_owned(),
                info,
            });
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// Inspects a card (and any connected UF2 drives) for firmware status. Strictly read-only.
pub fn inspect_card_firmware(
    device: DeviceKind,
    card: Option<&Path>,
    uf2_drives: Vec<Uf2Drive>,
) -> FirmwareCardStatus {
    let method = device.profile().firmware_method;
    let mut card_files = Vec::new();
    let mut required_min_firmware: Option<String> = None;
    let mut warnings = Vec::new();

    if let Some(card) = card {
        match device {
            DeviceKind::AnaloguePocket => {
                if let Ok(entries) = fs::read_dir(card) {
                    for e in entries.flatten() {
                        let name = e.file_name().to_string_lossy().into_owned();
                        if name.starts_with('.') || !e.file_type().is_ok_and(|t| t.is_file()) {
                            continue;
                        }
                        if is_pocket_firmware_bin(&name) {
                            let bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
                            card_files.push(CardFirmwareFile {
                                version: parse_pocket_firmware_version(&name),
                                file_name: name,
                                bytes,
                            });
                        }
                    }
                }
                card_files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
                if card_files.len() > 1 {
                    warnings.push(
                        "Multiple pocket_firmware*.bin files are on this card root. Installing a new version will back up and clean up the old ones so the Pocket flashes cleanly."
                            .into(),
                    );
                }
            }
            DeviceKind::GameBub => {
                let view = gamebub::inspect_card(card);
                for c in view.cores {
                    if c.core.min_firmware.is_empty() {
                        continue;
                    }
                    let take = match &required_min_firmware {
                        None => true,
                        Some(cur) => {
                            compare_semver_like(&c.core.min_firmware, cur)
                                == std::cmp::Ordering::Greater
                        }
                    };
                    if take {
                        required_min_firmware = Some(c.core.min_firmware);
                    }
                }
            }
        }
    }

    FirmwareCardStatus {
        device,
        method,
        card_files,
        required_min_firmware,
        uf2_drives,
        warnings,
    }
}

/// Plans installing a downloaded firmware image onto a Pocket SD card root or a Game Bub UF2 drive. Read-only.
pub fn plan_install(
    device: DeviceKind,
    target_root: &Path,
    downloaded_file: &Path,
    file_name: &str,
    expected_md5: Option<&str>,
) -> Result<FirmwareInstallPlan, TauError> {
    if !target_root.is_dir() {
        return Err(bad("Target drive or card folder is not available."));
    }
    let clean_name = safe_firmware_filename(device, file_name)?;
    let bytes = fs::read(downloaded_file)?;
    if bytes.is_empty() {
        return Err(bad("Downloaded firmware file is empty."));
    }
    let md5 = md5_hex(&bytes);
    let md5_verified = if let Some(exp) = expected_md5.filter(|s| !s.trim().is_empty()) {
        if !md5.eq_ignore_ascii_case(exp.trim()) {
            return Err(bad(format!(
                "Firmware checksum mismatch (expected MD5 {}, got {}).",
                exp.trim(),
                md5
            )));
        }
        true
    } else {
        false
    };

    if device == DeviceKind::GameBub {
        validate_uf2(&bytes)?;
    }

    let mut replaces = Vec::new();
    let mut warnings = Vec::new();
    if device == DeviceKind::AnaloguePocket
        && let Ok(entries) = fs::read_dir(target_root)
    {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with('.')
                && e.file_type().is_ok_and(|t| t.is_file())
                && is_pocket_firmware_bin(&name)
            {
                replaces.push(name);
            }
        }
        replaces.sort();
    } else if device == DeviceKind::GameBub {
        warnings.push(
            "Copying the .uf2 file to the GAME BUB drive will immediately flash the handheld and reboot it."
                .into(),
        );
    }

    let target_path = target_root.join(&clean_name);
    let id = sha256_hex(
        format!(
            "fw|{}|{}|{}|{}|{}",
            device.label(),
            target_root.display(),
            clean_name,
            md5,
            replaces.join(",")
        )
        .as_bytes(),
    )[..16]
        .to_string();

    Ok(FirmwareInstallPlan {
        id,
        device,
        target_path: target_path.to_string_lossy().into_owned(),
        file_name: clean_name,
        bytes: bytes.len() as u64,
        md5,
        md5_verified,
        replaces,
        warnings,
    })
}

/// Executes a reviewed firmware install plan.
pub fn execute_install(
    device: DeviceKind,
    target_root: &Path,
    downloaded_file: &Path,
    file_name: &str,
    expected_md5: Option<&str>,
    confirmation: &str,
    backup_root: &Path,
) -> Result<FirmwareInstallReport, TauError> {
    let plan = plan_install(
        device,
        target_root,
        downloaded_file,
        file_name,
        expected_md5,
    )?;
    if plan.id != confirmation {
        return Err(bad(
            "Confirmation token does not match the current card or drive state. Review the plan again.",
        ));
    }

    let bytes = fs::read(downloaded_file)?;
    let mut backup_dir = None;
    let mut removed_old = Vec::new();

    if device == DeviceKind::AnaloguePocket && !plan.replaces.is_empty() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let dir = backup_root.join(format!("pocket-firmware-{stamp}-{}", plan.id));
        fs::create_dir_all(&dir)?;
        for old in &plan.replaces {
            let src = target_root.join(old);
            let dst = dir.join(old);
            let h1 = sync::sha256_file(&src)?;
            fs::copy(&src, &dst)?;
            if sync::sha256_file(&dst)? != h1 {
                return Err(bad(format!("Backup verification failed for '{old}'.")));
            }
        }
        backup_dir = Some(dir);
    }

    let dst = target_root.join(&plan.file_name);
    if device == DeviceKind::AnaloguePocket {
        let part = target_root.join(format!("{}.part", plan.file_name));
        let expected_sha = sha256_hex(&bytes);
        {
            let mut f = fs::File::create(&part)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
        }
        let back = sync::read_back_bytes(&part)?;
        if sha256_hex(&back) != expected_sha {
            let _ = fs::remove_file(&part);
            return Err(bad("Read-back verification failed while writing firmware."));
        }
        // Only remove older firmware files after the new image is written and verified on the card.
        for old in &plan.replaces {
            let src = target_root.join(old);
            if src.is_file() {
                fs::remove_file(&src)?;
                removed_old.push(old.clone());
            }
        }
        fs::rename(&part, &dst)?;
    } else {
        // UF2 bootloader drives flash blocks on sequential write and reboot immediately on completion.
        let mut f = fs::File::create(&dst)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }

    Ok(FirmwareInstallReport {
        device,
        destination: dst.to_string_lossy().into_owned(),
        bytes_written: bytes.len() as u64,
        removed_old,
        backup_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_rfc1321_vectors() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            md5_hex(b"message digest"),
            "f96b697d7cb7938d525a2f31aaf161d0"
        );
    }

    #[test]
    fn parses_pocket_firmware_filenames() {
        assert_eq!(
            parse_pocket_firmware_version("pocket_firmware_2_7.bin").as_deref(),
            Some("2.7")
        );
        assert_eq!(
            parse_pocket_firmware_version("pocket_firmware_2_2_beta.bin").as_deref(),
            Some("2.2-beta")
        );
        assert_eq!(parse_pocket_firmware_version("other.bin"), None);
    }

    #[test]
    fn plans_and_installs_pocket_firmware_with_backup() {
        let root = std::env::temp_dir().join(format!("tau-fw-{}", crate::test_uniq()));
        let card = root.join("card");
        let bk = root.join("backup");
        fs::create_dir_all(&card).unwrap();
        fs::create_dir_all(&bk).unwrap();

        fs::write(card.join("pocket_firmware_2_6.bin"), b"old-fw").unwrap();
        let dl = root.join("pocket_firmware_2_7.bin");
        fs::write(&dl, b"new-fw-bytes").unwrap();
        let expected_md5 = md5_hex(b"new-fw-bytes");

        let status = inspect_card_firmware(DeviceKind::AnaloguePocket, Some(&card), vec![]);
        assert_eq!(status.card_files.len(), 1);
        assert_eq!(status.card_files[0].version.as_deref(), Some("2.6"));

        let plan = plan_install(
            DeviceKind::AnaloguePocket,
            &card,
            &dl,
            "pocket_firmware_2_7.bin",
            Some(&expected_md5),
        )
        .unwrap();
        assert!(plan.md5_verified);
        assert_eq!(plan.replaces, ["pocket_firmware_2_6.bin"]);

        let rep = execute_install(
            DeviceKind::AnaloguePocket,
            &card,
            &dl,
            "pocket_firmware_2_7.bin",
            Some(&expected_md5),
            &plan.id,
            &bk,
        )
        .unwrap();
        assert_eq!(rep.removed_old, ["pocket_firmware_2_6.bin"]);
        assert!(
            rep.backup_dir
                .unwrap()
                .join("pocket_firmware_2_6.bin")
                .is_file()
        );
        assert!(!card.join("pocket_firmware_2_6.bin").exists());
        assert_eq!(
            fs::read(card.join("pocket_firmware_2_7.bin")).unwrap(),
            b"new-fw-bytes"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validates_uf2_blocks() {
        assert!(validate_uf2(b"not uf2").is_err());
        let mut block = [0u8; 512];
        block[0..4].copy_from_slice(&UF2_MAGIC_START0.to_le_bytes());
        block[4..8].copy_from_slice(&UF2_MAGIC_START1.to_le_bytes());
        block[508..512].copy_from_slice(&UF2_MAGIC_END.to_le_bytes());
        assert!(validate_uf2(&block).is_ok());
    }
}
