//! Game Bub zip vetting and safe card install / update / remove (`D-026`, G3).
//!
//! Supports both documented Game Bub zip layouts (`cores/<id>/core.json` and `<id>/core.json`) as well as multi-core
//! archives (e.g. Minimig + Minimig040, PC Engine + PC Engine CD). Every card write follows Omega's safety model:
//! plan -> review -> confirmation token -> SHA-256-verified backup outside the card -> atomic write + read-back ->
//! rollback on error -> AppleDouble `._*` sweep. User-supplied BIOS/asset files in `/cores/<id>/` that are not part
//! of the zip are preserved on update.

use crate::{
    ErrorCode, TauError,
    device::{self, DeviceKind},
    gamebub::{self, GameBubCoreJson},
    sync,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_ZIP_BYTES: usize = 64 * 1024 * 1024;
const MAX_UNCOMPRESSED_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct BundledGameBubCore {
    pub core: GameBubCoreJson,
    /// Files relative to `cores/<core.id>/` -> raw bytes.
    pub files: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct VettedGameBubZip {
    pub zip_sha256: String,
    pub cores: Vec<BundledGameBubCore>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlannedCoreSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub min_firmware: String,
    /// `"install"` or `"update"`.
    pub action: String,
    pub previous_version: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubInstallPlan {
    pub id: String,
    pub zip_sha256: String,
    pub cores: Vec<PlannedCoreSummary>,
    pub files_to_write: Vec<String>,
    pub backup_files: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubInstallReport {
    pub cores_installed: Vec<String>,
    pub files_written: usize,
    pub backup_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubRemovePlan {
    pub id: String,
    pub core_id: String,
    pub name: String,
    pub files_to_remove: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubRemoveReport {
    pub core_id: String,
    pub files_removed: usize,
    pub backup_dir: Option<PathBuf>,
}

fn bad(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidPathReference, msg)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_safe_zip_path(raw: &str) -> bool {
    if raw.is_empty()
        || raw.starts_with('/')
        || raw.ends_with('/')
        || raw.contains('\\')
        || raw.contains('\0')
    {
        return false;
    }
    raw.split('/')
        .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

/// Vets a Game Bub release zip, normalizing both `cores/<id>/...` and `<id>/...` layouts and supporting multi-core zips.
pub fn inspect_zip(
    zip_bytes: &[u8],
    expected_sha256: Option<&str>,
) -> Result<VettedGameBubZip, TauError> {
    if zip_bytes.is_empty() {
        return Err(bad("The Game Bub core archive is empty."));
    }
    if zip_bytes.len() > MAX_ZIP_BYTES {
        return Err(bad(
            "The Game Bub core archive exceeds the 64 MiB safety limit.",
        ));
    }
    let zip_sha256 = sha256_hex(zip_bytes);
    if let Some(exp) = expected_sha256
        && !exp.is_empty()
        && !zip_sha256.eq_ignore_ascii_case(exp)
    {
        return Err(bad(format!(
            "Archive SHA-256 mismatch: expected {exp}, got {zip_sha256}."
        )));
    }

    let mut za = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|_| bad("This file is not a valid zip archive."))?;
    let mut raw_files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut total_uncompressed = 0u64;
    for i in 0..za.len() {
        let mut entry = za
            .by_index(i)
            .map_err(|_| bad("Could not read an entry inside the zip archive."))?;
        let name = entry.name().to_string();
        if entry.is_dir() {
            let trimmed = name.trim_end_matches('/');
            if !trimmed.is_empty() && !is_safe_zip_path(trimmed) {
                return Err(bad(format!("Unsafe path in zip: {name}")));
            }
            continue;
        }
        if !is_safe_zip_path(&name) {
            return Err(bad(format!("Unsafe path in zip: {name}")));
        }
        // Skip macOS AppleDouble / Finder metadata stubs.
        if name.starts_with("__MACOSX/")
            || name
                .rsplit('/')
                .next()
                .is_some_and(|f| f.starts_with("._") || f == ".DS_Store")
        {
            continue;
        }
        total_uncompressed = total_uncompressed.saturating_add(entry.size());
        if total_uncompressed > MAX_UNCOMPRESSED_BYTES {
            return Err(bad("Zip expands beyond the 128 MiB safety limit."));
        }
        let mut buf = Vec::with_capacity(entry.size().min(MAX_ZIP_BYTES as u64) as usize);
        entry
            .by_ref()
            .take(MAX_UNCOMPRESSED_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(|_| bad(format!("Failed to read '{name}' from zip.")))?;
        raw_files.insert(name, buf);
    }

    // Locate every Game Bub core.json at `cores/<dir>/core.json` or `<dir>/core.json`.
    let mut prefixes: Vec<(String, GameBubCoreJson)> = Vec::new();
    for (path, bytes) in &raw_files {
        let parts: Vec<&str> = path.split('/').collect();
        let prefix = match parts.as_slice() {
            ["cores", folder, "core.json"] | ["Cores", folder, "core.json"] => {
                format!("{}/{folder}/", parts[0])
            }
            [folder, "core.json"] if !folder.eq_ignore_ascii_case("cores") => {
                format!("{folder}/")
            }
            _ => continue,
        };
        let core = gamebub::parse_core_json(bytes)?;
        prefixes.push((prefix, core));
    }

    if prefixes.is_empty() {
        return Err(bad(
            "No Game Bub core.json was found in this archive (expected cores/<id>/core.json or <id>/core.json).",
        ));
    }

    let mut seen_ids = BTreeSet::new();
    let mut cores = Vec::with_capacity(prefixes.len());
    for (prefix, core) in prefixes {
        if !seen_ids.insert(core.id.clone()) {
            return Err(bad(format!(
                "Archive contains duplicate Game Bub core '{}'.",
                core.id
            )));
        }
        let mut files = BTreeMap::new();
        for (path, bytes) in &raw_files {
            if let Some(rel) = path.strip_prefix(&prefix)
                && !rel.is_empty()
            {
                files.insert(rel.to_string(), bytes.clone());
            }
        }
        for bs in &core.bitstreams {
            if !files.contains_key(&bs.filename) {
                return Err(bad(format!(
                    "Core '{}' declares bitstream '{}', which is missing from the archive.",
                    core.id, bs.filename
                )));
            }
        }
        cores.push(BundledGameBubCore { core, files });
    }

    Ok(VettedGameBubZip { zip_sha256, cores })
}

struct PreparedInstall {
    plan: GameBubInstallPlan,
    writes: BTreeMap<String, Vec<u8>>,
    core_ids: Vec<String>,
}

fn prepare_install(
    card: &Path,
    zip_bytes: &[u8],
    only_core: Option<&str>,
    expected_sha256: Option<&str>,
) -> Result<PreparedInstall, TauError> {
    let detected = device::detect_card(card);
    if detected.device == Some(DeviceKind::AnaloguePocket) && detected.gamebub_cores == 0 {
        return Err(bad(
            "This card is an Analogue Pocket card. Open a Game Bub (FAT32) card to install Game Bub cores.",
        ));
    }

    let vetted = inspect_zip(zip_bytes, expected_sha256)?;
    let selected: Vec<&BundledGameBubCore> = match only_core {
        Some(id) if !id.is_empty() => {
            let found: Vec<_> = vetted.cores.iter().filter(|c| c.core.id == id).collect();
            if found.is_empty() {
                return Err(bad(format!(
                    "Core '{id}' was not found inside the archive."
                )));
            }
            found
        }
        _ => vetted.cores.iter().collect(),
    };

    let installed = gamebub::inspect_card(card);
    let mut summaries = Vec::with_capacity(selected.len());
    let mut writes = BTreeMap::new();
    let mut backup_files = Vec::new();
    let mut warnings = Vec::new();
    let mut core_ids = Vec::with_capacity(selected.len());

    if detected.device.is_none() {
        warnings.push(
            "This card has no Game Bub files yet; make sure it is formatted as FAT32 (MBR).".into(),
        );
    }

    for bundled in selected {
        let id = &bundled.core.id;
        core_ids.push(id.clone());
        let prev = installed.cores.iter().find(|c| c.core.id == *id);
        let action = if prev.is_some() { "update" } else { "install" }.to_string();
        let previous_version = prev.map(|p| p.core.version.clone());
        let mut total_bytes = 0u64;
        for (rel, data) in &bundled.files {
            let card_rel = format!("cores/{id}/{rel}");
            total_bytes += data.len() as u64;
            if card.join(&card_rel).is_file() {
                backup_files.push(card_rel.clone());
            }
            writes.insert(card_rel, data.clone());
        }
        summaries.push(PlannedCoreSummary {
            id: id.clone(),
            name: bundled.core.name.clone(),
            version: bundled.core.version.clone(),
            min_firmware: bundled.core.min_firmware.clone(),
            action,
            previous_version,
            file_count: bundled.files.len(),
            total_bytes,
        });
    }

    let files_to_write: Vec<String> = writes.keys().cloned().collect();
    let token_src = format!(
        "gb-install|{}|{}|{}|{}",
        card.display(),
        vetted.zip_sha256,
        core_ids.join(","),
        backup_files.join(",")
    );
    let id = sha256_hex(token_src.as_bytes())[..16].to_string();

    Ok(PreparedInstall {
        plan: GameBubInstallPlan {
            id,
            zip_sha256: vetted.zip_sha256,
            cores: summaries,
            files_to_write,
            backup_files,
            warnings,
        },
        writes,
        core_ids,
    })
}

/// Plans installing one or all cores from a Game Bub release zip onto `card`. Read-only.
pub fn plan_install(
    card: &Path,
    zip_bytes: &[u8],
    only_core: Option<&str>,
    expected_sha256: Option<&str>,
) -> Result<GameBubInstallPlan, TauError> {
    Ok(prepare_install(card, zip_bytes, only_core, expected_sha256)?.plan)
}

fn backup_file(card: &Path, rel: &str, backup_dir: &Path) -> Result<(), TauError> {
    let src = card.join(rel);
    if !src.is_file() {
        return Ok(());
    }
    let dst = backup_dir.join(rel);
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p)?;
    }
    let src_hash = sync::sha256_file(&src)?;
    fs::copy(&src, &dst)?;
    let dst_hash = sync::sha256_file(&dst)?;
    if src_hash != dst_hash {
        return Err(bad(format!(
            "Backup verification failed for '{rel}': nothing on the card was changed."
        )));
    }
    Ok(())
}

fn write_atomic_verified(path: &Path, bytes: &[u8]) -> Result<(), TauError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let part = path.with_file_name(format!(
        "{}.part",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let expected = sha256_hex(bytes);
    {
        let mut f = fs::File::create(&part)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    let back = sync::read_back_bytes(&part)?;
    if sha256_hex(&back) != expected {
        let _ = fs::remove_file(&part);
        return Err(bad(format!("Read-back failed for '{}'.", path.display())));
    }
    fs::rename(&part, path)?;
    Ok(())
}

fn sweep_appledouble(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("._") && e.file_type().is_ok_and(|t| t.is_file()) {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}

/// Executes a reviewed Game Bub core install/update onto `card`.
pub fn execute_install(
    card: &Path,
    zip_bytes: &[u8],
    only_core: Option<&str>,
    expected_sha256: Option<&str>,
    confirmation: &str,
    backup_root: &Path,
) -> Result<GameBubInstallReport, TauError> {
    let prep = prepare_install(card, zip_bytes, only_core, expected_sha256)?;
    if prep.plan.id != confirmation {
        return Err(bad(
            "Confirmation token does not match the current card state. Review the plan again.",
        ));
    }

    let backup_dir = if !prep.plan.backup_files.is_empty() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let dir = backup_root.join(format!("gamebub-{stamp}-{}", prep.plan.id));
        fs::create_dir_all(&dir)?;
        for rel in &prep.plan.backup_files {
            backup_file(card, rel, &dir)?;
        }
        Some(dir)
    } else {
        None
    };

    let mut written = 0usize;
    for (rel, data) in &prep.writes {
        if let Err(e) = write_atomic_verified(&card.join(rel), data) {
            // Roll back any overwritten files from backup.
            if let Some(bdir) = &backup_dir {
                for brel in &prep.plan.backup_files {
                    let _ = fs::copy(bdir.join(brel), card.join(brel));
                }
            }
            return Err(e);
        }
        written += 1;
    }

    for cid in &prep.core_ids {
        sweep_appledouble(&card.join("cores").join(cid));
    }

    Ok(GameBubInstallReport {
        cores_installed: prep.core_ids,
        files_written: written,
        backup_dir,
    })
}

fn collect_rel_files(root: &Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut items: Vec<_> = entries.flatten().collect();
    items.sort_by_key(|e| e.file_name());
    for e in items {
        let name = e.file_name().to_string_lossy().into_owned();
        let rel = format!("{prefix}/{name}");
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            collect_rel_files(&e.path(), &rel, out);
        } else if e.file_type().is_ok_and(|t| t.is_file()) {
            out.push(rel);
        }
    }
}

/// Plans removing one installed Game Bub core (`/cores/<core_id>/`). Read-only.
pub fn plan_remove(card: &Path, core_id: &str) -> Result<GameBubRemovePlan, TauError> {
    if core_id.is_empty()
        || core_id.contains('/')
        || core_id.contains('\\')
        || core_id.contains("..")
    {
        return Err(bad("Invalid Game Bub core id."));
    }
    let dir = card.join("cores").join(core_id);
    let cj = fs::read(dir.join("core.json")).map_err(|_| {
        bad(format!(
            "Game Bub core '{core_id}' was not found on this card."
        ))
    })?;
    let parsed = gamebub::parse_core_json(&cj)?;
    let mut files_to_remove = Vec::new();
    collect_rel_files(&dir, &format!("cores/{core_id}"), &mut files_to_remove);
    let token_src = format!(
        "gb-remove|{}|{}|{}",
        card.display(),
        core_id,
        files_to_remove.join(",")
    );
    Ok(GameBubRemovePlan {
        id: sha256_hex(token_src.as_bytes())[..16].to_string(),
        core_id: core_id.to_string(),
        name: parsed.name,
        files_to_remove,
    })
}

/// Executes a reviewed removal of one Game Bub core after verified backup outside the card.
pub fn execute_remove(
    card: &Path,
    core_id: &str,
    confirmation: &str,
    backup_root: &Path,
) -> Result<GameBubRemoveReport, TauError> {
    let plan = plan_remove(card, core_id)?;
    if plan.id != confirmation {
        return Err(bad(
            "Confirmation token does not match the current card state. Review the plan again.",
        ));
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let bdir = backup_root.join(format!("gamebub-remove-{stamp}-{}", plan.id));
    fs::create_dir_all(&bdir)?;
    for rel in &plan.files_to_remove {
        backup_file(card, rel, &bdir)?;
    }
    fs::remove_dir_all(card.join("cores").join(core_id))?;
    Ok(GameBubRemoveReport {
        core_id: core_id.to_string(),
        files_removed: plan.files_to_remove.len(),
        backup_dir: Some(bdir),
    })
}
