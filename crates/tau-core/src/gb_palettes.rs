//! Game Boy colour palettes (`Assets/gb/common/Palettes/**/*.pal`, 56-byte openFPGA `.pal` files):
//! read-only listing, reviewed plan, verified external backup, atomic write, and safe removal.

use crate::{ErrorCode, TauError, sync};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const PALETTE_ROOT: &str = "Assets/gb/common/Palettes";
pub const PALETTE_BYTES: usize = 56;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaletteItem {
    /// Relative path inside `Assets/gb/common/Palettes/`, e.g. `Analogue Palettes/Analogue Green.pal`.
    pub rel_path: String,
    /// 56 raw palette bytes as 112 hex characters.
    pub hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaletteInstallPlan {
    pub id: String,
    pub new_files: Vec<String>,
    pub updated_files: Vec<String>,
    pub unchanged: usize,
    pub backup_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaletteInstallReport {
    pub written: usize,
    pub backup_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaletteRemovePlan {
    pub id: String,
    pub rel_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaletteRemoveReport {
    pub rel_path: String,
    pub backup_dir: Option<PathBuf>,
}

fn bad(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidPathReference, msg)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn safe_pal_rel(raw: &str) -> Result<String, TauError> {
    let rel = raw.trim().replace('\\', "/");
    if rel.is_empty()
        || rel.starts_with('/')
        || rel.ends_with('/')
        || rel.contains('\0')
        || !rel.to_ascii_lowercase().ends_with(".pal")
    {
        return Err(bad(format!("Invalid palette path: '{raw}'.")));
    }
    if !rel
        .split('/')
        .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
    {
        return Err(bad(format!("Unsafe palette path: '{raw}'.")));
    }
    Ok(rel)
}

fn decode_pal_hex(hex: &str) -> Result<Vec<u8>, TauError> {
    let h = hex.trim();
    if h.len() != PALETTE_BYTES * 2 {
        return Err(bad(format!(
            "A Game Boy palette must be exactly {PALETTE_BYTES} bytes."
        )));
    }
    let mut out = Vec::with_capacity(PALETTE_BYTES);
    for pair in h.as_bytes().as_chunks::<2>().0 {
        let s = std::str::from_utf8(pair).map_err(|_| bad("Palette hex is invalid."))?;
        let v = u8::from_str_radix(s, 16).map_err(|_| bad("Palette hex is invalid."))?;
        out.push(v);
    }
    Ok(out)
}

fn walk_pals(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<_> = entries.flatten().collect();
    items.sort_by_key(|e| e.file_name());
    for e in items {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            walk_pals(&e.path(), &rel, out);
        } else if e.file_type().is_ok_and(|t| t.is_file())
            && name.to_ascii_lowercase().ends_with(".pal")
        {
            out.push(rel);
        }
    }
}

/// Lists all `.pal` files currently on the card under `Assets/gb/common/Palettes/` (relative paths). Read-only.
pub fn list_on_card(card: &Path) -> Vec<String> {
    let mut out = Vec::new();
    walk_pals(&card.join(PALETTE_ROOT), "", &mut out);
    out
}

/// Plans writing one or more 56-byte `.pal` files to `Assets/gb/common/Palettes/`. Read-only.
pub fn plan_install(card: &Path, items: &[PaletteItem]) -> Result<PaletteInstallPlan, TauError> {
    if items.is_empty() {
        return Err(bad("Choose at least one palette to install."));
    }
    let root = card.join(PALETTE_ROOT);
    let mut new_files = Vec::new();
    let mut updated_files = Vec::new();
    let mut backup_files = Vec::new();
    let mut unchanged = 0usize;
    let mut token_parts = vec![format!("gb-pal|{}", card.display())];

    for item in items {
        let rel = safe_pal_rel(&item.rel_path)?;
        let bytes = decode_pal_hex(&item.hex)?;
        let dst = root.join(&rel);
        let full_rel = format!("{PALETTE_ROOT}/{rel}");
        if dst.is_file() {
            let cur = sync::read_back_bytes(&dst)?;
            if cur == bytes {
                unchanged += 1;
                continue;
            }
            updated_files.push(rel.clone());
            backup_files.push(full_rel);
        } else {
            new_files.push(rel.clone());
        }
        token_parts.push(format!("{rel}:{}", sha256_hex(&bytes)));
    }
    let id = sha256_hex(token_parts.join("|").as_bytes())[..16].to_string();
    Ok(PaletteInstallPlan {
        id,
        new_files,
        updated_files,
        unchanged,
        backup_files,
    })
}

fn write_verified(path: &Path, bytes: &[u8]) -> Result<(), TauError> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
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
        return Err(bad("Read-back verification failed while writing palette."));
    }
    fs::rename(&part, path)?;
    Ok(())
}

/// Executes a reviewed palette install plan after backing up any replaced `.pal` files outside the card.
pub fn execute_install(
    card: &Path,
    items: &[PaletteItem],
    confirmation: &str,
    backup_root: &Path,
) -> Result<PaletteInstallReport, TauError> {
    let plan = plan_install(card, items)?;
    if plan.id != confirmation {
        return Err(bad(
            "Confirmation token does not match the current card state. Review the plan again.",
        ));
    }

    let backup_dir = if !plan.backup_files.is_empty() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let dir = backup_root.join(format!("gb-palettes-{stamp}-{}", plan.id));
        fs::create_dir_all(&dir)?;
        for rel in &plan.backup_files {
            let src = card.join(rel);
            let dst = dir.join(rel);
            if let Some(p) = dst.parent() {
                fs::create_dir_all(p)?;
            }
            let h1 = sync::sha256_file(&src)?;
            fs::copy(&src, &dst)?;
            if sync::sha256_file(&dst)? != h1 {
                return Err(bad(format!("Backup verification failed for '{rel}'.")));
            }
        }
        Some(dir)
    } else {
        None
    };

    let root = card.join(PALETTE_ROOT);
    let mut written = 0usize;
    for item in items {
        let rel = safe_pal_rel(&item.rel_path)?;
        if !plan.new_files.contains(&rel) && !plan.updated_files.contains(&rel) {
            continue;
        }
        let bytes = decode_pal_hex(&item.hex)?;
        write_verified(&root.join(&rel), &bytes)?;
        written += 1;
    }

    Ok(PaletteInstallReport {
        written,
        backup_dir,
    })
}

/// Plans removing one `.pal` file from `Assets/gb/common/Palettes/`. Read-only.
pub fn plan_remove(card: &Path, rel_path: &str) -> Result<PaletteRemovePlan, TauError> {
    let rel = safe_pal_rel(rel_path)?;
    let full = card.join(PALETTE_ROOT).join(&rel);
    if !full.is_file() {
        return Err(bad(format!("Palette '{rel}' is not on this card.")));
    }
    let h = sync::sha256_file(&full)?;
    let id = sha256_hex(format!("gb-pal-rm|{}|{}|{h}", card.display(), rel).as_bytes())[..16]
        .to_string();
    Ok(PaletteRemovePlan { id, rel_path: rel })
}

/// Removes a reviewed `.pal` file after a verified backup outside the card.
pub fn execute_remove(
    card: &Path,
    rel_path: &str,
    confirmation: &str,
    backup_root: &Path,
) -> Result<PaletteRemoveReport, TauError> {
    let plan = plan_remove(card, rel_path)?;
    if plan.id != confirmation {
        return Err(bad(
            "Confirmation token does not match the current card state. Review the plan again.",
        ));
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dir = backup_root.join(format!("gb-palettes-rm-{stamp}-{}", plan.id));
    let full_rel = format!("{PALETTE_ROOT}/{}", plan.rel_path);
    let src = card.join(&full_rel);
    let dst = dir.join(&full_rel);
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p)?;
    }
    let h1 = sync::sha256_file(&src)?;
    fs::copy(&src, &dst)?;
    if sync::sha256_file(&dst)? != h1 {
        return Err(bad("Backup verification failed; nothing was removed."));
    }
    fs::remove_file(&src)?;
    Ok(PaletteRemoveReport {
        rel_path: plan.rel_path,
        backup_dir: Some(dir),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_plans_installs_and_removes_56_byte_palettes() {
        let card = std::env::temp_dir().join(format!("tau-gbpal-{}", std::process::id()));
        let backup = std::env::temp_dir().join(format!("tau-gbpal-bk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&card);
        let _ = fs::remove_dir_all(&backup);
        fs::create_dir_all(&card).unwrap();
        fs::create_dir_all(&backup).unwrap();

        let hex56 = "00".repeat(PALETTE_BYTES);
        let item = PaletteItem {
            rel_path: "Analogue Palettes/Analogue Green.pal".into(),
            hex: hex56.clone(),
        };
        // Rejects wrong length and path traversal
        assert!(
            plan_install(
                &card,
                &[PaletteItem {
                    rel_path: "a.pal".into(),
                    hex: "0011".into()
                }]
            )
            .is_err()
        );
        assert!(
            plan_install(
                &card,
                &[PaletteItem {
                    rel_path: "../escape.pal".into(),
                    hex: hex56
                }]
            )
            .is_err()
        );

        let p = plan_install(&card, std::slice::from_ref(&item)).unwrap();
        assert_eq!(p.new_files, ["Analogue Palettes/Analogue Green.pal"]);
        let r = execute_install(&card, std::slice::from_ref(&item), &p.id, &backup).unwrap();
        assert_eq!(r.written, 1);
        assert_eq!(
            list_on_card(&card),
            ["Analogue Palettes/Analogue Green.pal"]
        );

        let rm_p = plan_remove(&card, "Analogue Palettes/Analogue Green.pal").unwrap();
        let rm_r = execute_remove(
            &card,
            "Analogue Palettes/Analogue Green.pal",
            &rm_p.id,
            &backup,
        )
        .unwrap();
        assert!(rm_r.backup_dir.is_some());
        assert!(list_on_card(&card).is_empty());
    }
}
