//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

// ---- installing to a card -------------------------------------------------------------------------------------

use crate::sync;
use std::fs;
use std::path::{Path, PathBuf};

/// File name on the card (data slot 8 of a Tau core).
pub const FILE_NAME: &str = "tau-assets.bin";
pub(super) const TEMP_NAME: &str = ".tau-assets.bin.tmp";
pub(super) const PREVIOUS_NAME: &str = ".tau-assets.bin.prev";

/// What is already at the destination, so the review can say what an install would replace.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExistingAssets {
    pub bytes: u64,
    pub sha256: String,
    /// Names of the themes in it (empty when it has none or cannot be read).
    pub themes: Vec<String>,
    /// Sections other than `THEM` (for example `METR` meter presets, `PRST` EQ presets): an install keeps them
    /// unchanged.
    pub other_sections: Vec<String>,
    /// False when the file does not parse; it is still backed up before being replaced.
    pub readable: bool,
}

/// A core on this card that uses the same media folder, and whether it asks for the file at all.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ThemeFileReader {
    pub core_id: String,
    pub version: String,
    /// True when the core's `data.json` declares a slot for `tau-assets.bin`.
    pub declares_slot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssetsInstallPlan {
    /// Confirmation token: changes when the destination, the new file or the existing file changes.
    pub id: String,
    pub destination: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub themes: Vec<String>,
    /// Names of the Halcyon presets being written (empty when the edit leaves them alone).
    pub presets: Vec<String>,
    pub existing: Option<ExistingAssets>,
    pub readers: Vec<ThemeFileReader>,
    /// A previous install was interrupted; running this one first puts the old file back.
    pub interrupted_install: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssetsInstallReport {
    pub destination: PathBuf,
    pub bytes_written: u64,
    pub replaced: bool,
    pub backup: Option<PathBuf>,
}

pub(super) fn describe_existing(bytes: &[u8]) -> ExistingAssets {
    let mut e = ExistingAssets {
        bytes: bytes.len() as u64,
        sha256: sync::sha256_bytes(bytes),
        themes: Vec::new(),
        other_sections: Vec::new(),
        readable: false,
    };
    if let Ok(sections) = read_sections(bytes) {
        e.readable = true;
        for (tag, _) in &sections {
            if tag != SECTION_THEM {
                e.other_sections
                    .push(String::from_utf8_lossy(tag).into_owned());
            }
        }
        match parse_assets(bytes) {
            Ok(t) => e.themes = t.into_iter().map(|t| t.name).collect(),
            Err(_) => e.readable = false,
        }
    }
    e
}

/// Cores under `<card>/Cores` whose platform is the media root's platform. `media_root` is `<card>/Assets/<platform>/common`.
pub(super) fn readers_for(media_root: &Path) -> Vec<ThemeFileReader> {
    let platform = media_root
        .parent()
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned());
    let card = media_root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent);
    let (Some(platform), Some(card)) = (platform, card) else {
        return Vec::new();
    };
    let Ok(dir) = fs::read_dir(card.join("Cores")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for folder in dir.filter_map(Result::ok).filter(|f| f.path().is_dir()) {
        let read = |name: &str| -> Option<serde_json::Value> {
            serde_json::from_slice(&fs::read(folder.path().join(name)).ok()?).ok()
        };
        let Some(core) = read("core.json") else {
            continue;
        };
        let meta = core.pointer("/core/metadata");
        let plat = meta
            .and_then(|m| m.pointer("/platform_ids/0"))
            .and_then(|v| v.as_str());
        if plat != Some(platform.as_str()) {
            continue;
        }
        let version = meta
            .and_then(|m| m.get("version"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let declares_slot = read("data.json").is_some_and(|d| {
            [
                "/data/data_slots",
                "/core/data/data_slots",
                "/data",
                "/core/data",
            ]
            .iter()
            .any(|p| {
                d.pointer(p).and_then(|v| v.as_array()).is_some_and(|a| {
                    a.iter()
                        .any(|s| s.get("filename").and_then(|f| f.as_str()) == Some(FILE_NAME))
                })
            })
        });
        out.push(ThemeFileReader {
            core_id: folder.file_name().to_string_lossy().into_owned(),
            version,
            declares_slot,
        });
    }
    out.sort_by(|a, b| a.core_id.cmp(&b.core_id));
    out
}

/// Plans writing these themes to `<media_root>/tau-assets.bin`. Writes nothing. Refuses any theme that fails
/// `check_theme`, a media root that is not an `Assets/<platform>/common` folder, and a missing folder.
pub fn plan_install(
    themes: &[ThemeInput],
    media_root: &Path,
) -> Result<AssetsInstallPlan, TauError> {
    plan_install_edit(AssetsEdit::themes(themes), media_root)
}

/// [`plan_install`] for any [`AssetsEdit`] (themes, Halcyon presets, or both).
pub fn plan_install_edit(
    edit: AssetsEdit,
    media_root: &Path,
) -> Result<AssetsInstallPlan, TauError> {
    sync::validate_media_root(media_root)?;
    let destination = media_root.join(FILE_NAME);
    let old = fs::read(&destination).ok();
    let blob = pack_assets_edit(edit, old.as_deref())?;
    let existing = old.as_deref().map(describe_existing);
    let sha256 = sync::sha256_bytes(&blob);
    let readers = readers_for(media_root);
    let interrupted_install = !destination.is_file() && media_root.join(PREVIOUS_NAME).is_file();
    let mut warnings = Vec::new();
    if readers.is_empty() {
        warnings.push(
            "No core on this card uses this media folder, so nothing would read the file.".into(),
        );
    } else if !readers.iter().any(|r| r.declares_slot) {
        warnings.push("None of the cores that use this folder ask for a theme file (they need Tau 0.5.0 or later), so it will be ignored until one is installed.".into());
    }
    if let Some(e) = &existing {
        if !e.other_sections.is_empty() {
            warnings.push(format!(
                "The file already there also holds {}: kept unchanged.",
                e.other_sections.join(", ")
            ));
        }
        if !e.readable {
            warnings.push("The file already there cannot be read as a Tau assets file. It will still be backed up before it is replaced.".into());
        }
    }
    let id = sync::sha256_bytes(
        format!(
            "assets|{}|{}|{}",
            destination.display(),
            sha256,
            existing.as_ref().map_or("none", |e| e.sha256.as_str())
        )
        .as_bytes(),
    );
    Ok(AssetsInstallPlan {
        id,
        destination,
        bytes: blob.len() as u64,
        sha256,
        themes: edit
            .themes
            .unwrap_or(&[])
            .iter()
            .map(|t| t.name.clone())
            .collect(),
        presets: edit
            .presets
            .unwrap_or(&[])
            .iter()
            .map(|p| p.name().to_string())
            .collect(),
        existing,
        readers,
        interrupted_install,
        warnings,
    })
}

/// Puts back an old file left by an install that was interrupted between its two renames (never invents one).
pub(super) fn recover(media_root: &Path) -> Result<(), TauError> {
    let live = media_root.join(FILE_NAME);
    let previous = media_root.join(PREVIOUS_NAME);
    if previous.is_file() {
        if live.is_file() {
            let _ = fs::remove_file(&previous);
        } else {
            fs::rename(&previous, &live)?;
        }
    }
    Ok(())
}

/// Writes a reviewed [`AssetsInstallPlan`]. Refuses unless `confirmation` is the plan's id **and** the plan is
/// still what a fresh plan would be (the existing file or the themes changed since review). Order: recover an
/// interrupted install, back up the existing file (when `backup_root` is given; it must be outside the card),
/// write a temporary file beside the target, read it back through the cache-bypassing path and check it parses
/// to the same bytes, swap it in keeping the old file until the new one is in place, then read the result back.
pub fn execute_install(
    themes: &[ThemeInput],
    media_root: &Path,
    plan: &AssetsInstallPlan,
    confirmation: &str,
    backup_root: Option<&Path>,
) -> Result<AssetsInstallReport, TauError> {
    execute_install_edit(
        AssetsEdit::themes(themes),
        media_root,
        plan,
        confirmation,
        backup_root,
    )
}

/// [`execute_install`] for any [`AssetsEdit`].
pub fn execute_install_edit(
    edit: AssetsEdit,
    media_root: &Path,
    plan: &AssetsInstallPlan,
    confirmation: &str,
    backup_root: Option<&Path>,
) -> Result<AssetsInstallReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    // A backup on the card itself is not a backup, so the whole card is off limits, not just the media folder.
    let card = media_root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap_or(media_root);
    if let Some(backup) = backup_root
        && (backup.as_os_str().is_empty() || sync::backup_is_inside(backup, card))
    {
        return Err(TauError::e(
            ErrorCode::UnsafeBackupLocation,
            "backup folder must be outside the card",
        ));
    }
    recover(media_root)?;
    let fresh = plan_install_edit(edit, media_root)?;
    if fresh.id != plan.id {
        return Err(TauError::e(
            ErrorCode::SourceChangedSincePlan,
            "the theme file on the card or the themes changed since the plan was reviewed",
        ));
    }
    let live = media_root.join(FILE_NAME);
    let old = fs::read(&live).ok();
    let blob = pack_assets_edit(edit, old.as_deref())?;
    let mut backup = None;
    if live.is_file()
        && let Some(root) = backup_root
    {
        let old = fs::read(&live)?;
        let dest = root.join(&plan.id).join(FILE_NAME);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        sync::write_durable(&dest, &old)?;
        if sync::sha256_bytes(&fs::read(&dest)?) != sync::sha256_bytes(&old) {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                "the backup of the existing theme file did not read back the same, so nothing was changed",
            ));
        }
        backup = Some(dest);
    }
    let temp = media_root.join(TEMP_NAME);
    let result = (|| -> Result<(), TauError> {
        sync::write_durable(&temp, &blob)?;
        let back = sync::read_back_bytes(&temp)?;
        let kept = old
            .as_deref()
            .map(|o| carried_sections(o, &edit))
            .unwrap_or_default();
        let (themes_back, presets_back) = edit.read_back(&back)?;
        let again = AssetsEdit {
            themes: themes_back.as_deref(),
            presets: presets_back.as_deref(),
        };
        if back != blob
            || pack_assets_edit(again, old.as_deref())? != blob
            || carried_sections(&back, &edit) != kept
        {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                "the theme file written to the card did not read back the same",
            ));
        }
        sync::swap_in_file(&temp, &live, PREVIOUS_NAME)?;
        if sync::read_back_bytes(&live)? != blob {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                "the installed theme file did not read back the same",
            ));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    // macOS writes an AppleDouble `._<name>` beside what it renames on exFAT/FAT (SAFETY_RULES 7). Remove only the
    // ones that belong to the three names this install used.
    for name in [FILE_NAME, TEMP_NAME, PREVIOUS_NAME] {
        let _ = fs::remove_file(media_root.join(format!("._{name}")));
    }
    result?;
    Ok(AssetsInstallReport {
        destination: live,
        bytes_written: blob.len() as u64,
        replaced: plan.existing.is_some(),
        backup,
    })
}
