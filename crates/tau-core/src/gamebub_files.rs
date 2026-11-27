//! Game Bub ROM & companion save management (`D-026`): extension-based ROM discovery,
//! stem-matched saves/save states, optional per-core pinned folders, and reviewed Add/Remove
//! with verified external backups. Pure filesystem logic; WASI-safe.

use crate::{ErrorCode, TauError, gamebub, platforms, sync};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_SCAN_DEPTH: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubRomEntry {
    pub rel_path: String,
    pub folder_rel: String,
    pub name: String,
    pub stem: String,
    pub ext: String,
    pub bytes: u64,
    pub modified: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum GameBubSaveKind {
    Save,
    SaveState,
    Companion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubSaveEntry {
    pub rel_path: String,
    pub folder_rel: String,
    pub name: String,
    pub game_stem: String,
    pub slot_label: String,
    pub kind: GameBubSaveKind,
    pub bytes: u64,
    pub modified: u64,
    pub matched_rom: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubCoreFilesView {
    pub core_id: String,
    pub pinned_folder: Option<String>,
    pub rom_extensions: Vec<String>,
    pub save_extensions: Vec<String>,
    pub roms: Vec<GameBubRomEntry>,
    pub saves: Vec<GameBubSaveEntry>,
    pub unmatched_saves: Vec<GameBubSaveEntry>,
    pub total_rom_bytes: u64,
    pub total_save_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubAddItem {
    pub source: String,
    pub dest_rel: String,
    pub bytes: u64,
    pub replaces_existing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubAddPlan {
    pub id: String,
    pub core_id: String,
    pub dest_folder: String,
    pub items: Vec<GameBubAddItem>,
    pub new_files: usize,
    pub replaced_files: usize,
    pub skipped: Vec<String>,
    pub bytes_to_write: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubAddReport {
    pub written: usize,
    pub replaced: usize,
    pub bytes_written: u64,
    pub backup_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubDeletePlan {
    pub id: String,
    pub core_id: String,
    pub rel_paths: Vec<String>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubDeleteReport {
    pub removed: usize,
    pub bytes_removed: u64,
    pub backup_dir: String,
}

fn bad_path(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidPathReference, msg)
}

fn is_reserved_top(segment: &str) -> bool {
    let lower = segment.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "cores" | "system" | ".trashes" | ".spotlight-v100" | ".fseventsd" | ".git"
    )
}

/// Validates and normalizes a card-relative folder path (empty string = card root).
/// Rejects `..`, NUL, and reserved top-level folders (`/cores`, `/system`).
pub fn normalize_rel_folder(raw: &str) -> Result<String, TauError> {
    if raw.contains('\0') {
        return Err(bad_path("Folder path contains NUL byte."));
    }
    let trimmed = raw.replace('\\', "/");
    let parts: Vec<&str> = trimmed
        .split('/')
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    for part in &parts {
        if *part == ".." {
            return Err(bad_path("Folder path cannot contain '..'."));
        }
    }
    if let Some(first) = parts.first()
        && is_reserved_top(first)
    {
        return Err(bad_path(format!(
            "Cannot target reserved Game Bub system folder '/{first}'."
        )));
    }
    Ok(parts.join("/"))
}

/// Validates and normalizes a card-relative file path (must be non-empty, not under `/cores` or `/system`).
pub fn normalize_rel_file(raw: &str) -> Result<String, TauError> {
    if raw.contains('\0') {
        return Err(bad_path("File path contains NUL byte."));
    }
    let trimmed = raw.replace('\\', "/");
    let parts: Vec<&str> = trimmed
        .split('/')
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    if parts.is_empty() {
        return Err(bad_path("File path cannot be empty."));
    }
    for part in &parts {
        if *part == ".." {
            return Err(bad_path("File path cannot contain '..'."));
        }
    }
    if let Some(first) = parts.first()
        && is_reserved_top(first)
    {
        return Err(bad_path(format!(
            "Cannot remove files inside reserved folder '/{first}'."
        )));
    }
    Ok(parts.join("/"))
}

fn find_core(card: &Path, core_id: &str) -> Result<gamebub::GameBubInstalledCore, TauError> {
    let view = gamebub::inspect_card(card);
    view.cores
        .into_iter()
        .find(|c| c.core.id.eq_ignore_ascii_case(core_id))
        .ok_or_else(|| {
            TauError::e(
                ErrorCode::NotFound,
                format!("Game Bub core “{core_id}” is not installed on this card."),
            )
        })
}

fn core_rom_extensions(core: &gamebub::GameBubInstalledCore) -> Vec<String> {
    let mut exts = Vec::new();
    for entry in core.files.iter().filter(|f| f.user_selected) {
        for raw in &entry.extensions {
            let clean = raw.trim().trim_start_matches('.').to_ascii_lowercase();
            if !clean.is_empty() && !exts.contains(&clean) {
                exts.push(clean);
            }
        }
    }
    if exts.is_empty()
        && let Some(spec) = platforms::infer_from_core(&core.core.id, &core.core.name, &[])
    {
        for raw in spec.extensions {
            let clean = raw.trim().trim_start_matches('.').to_ascii_lowercase();
            if !clean.is_empty() && !exts.contains(&clean) {
                exts.push(clean);
            }
        }
    }
    exts
}

#[derive(Clone)]
struct CompanionSpec {
    ext: String,
    label: String,
    kind: GameBubSaveKind,
}

fn classify_companion(ext: &str, label: &str) -> GameBubSaveKind {
    let l = label.to_ascii_lowercase();
    let e = ext.to_ascii_lowercase();
    if l.contains("state") || matches!(e.as_str(), "ss" | "sta" | "state") {
        GameBubSaveKind::SaveState
    } else if l.contains("save") || matches!(e.as_str(), "srm" | "sav" | "eep" | "fla") {
        GameBubSaveKind::Save
    } else {
        GameBubSaveKind::Companion
    }
}

fn core_companions(core: &gamebub::GameBubInstalledCore) -> Vec<CompanionSpec> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for entry in core
        .files
        .iter()
        .filter(|f| f.dependent_on_0 && f.filename.is_none() && !f.user_selected)
    {
        for raw in &entry.extensions {
            let clean = raw.trim().trim_start_matches('.').to_ascii_lowercase();
            if clean.is_empty() || !seen.insert(clean.clone()) {
                continue;
            }
            let kind = classify_companion(&clean, &entry.label);
            let label = if entry.label.trim().is_empty() {
                match kind {
                    GameBubSaveKind::Save => "Save".into(),
                    GameBubSaveKind::SaveState => "Save state".into(),
                    GameBubSaveKind::Companion => "Companion".into(),
                }
            } else {
                entry.label.trim().to_string()
            };
            out.push(CompanionSpec {
                ext: clean,
                label,
                kind,
            });
        }
    }
    out
}

fn modified_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

struct ScannedRawFile {
    rel_path: String,
    folder_rel: String,
    name: String,
    stem: String,
    ext: String,
    bytes: u64,
    modified: u64,
}

fn walk_card_files(
    dir: &Path,
    rel_prefix: &str,
    depth: usize,
    out: &mut Vec<ScannedRawFile>,
) -> Result<(), TauError> {
    if depth > MAX_SCAN_DEPTH || !dir.is_dir() {
        return Ok(());
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    let mut list: Vec<fs::DirEntry> = entries.filter_map(Result::ok).collect();
    list.sort_by_key(|e| e.file_name());

    for entry in list {
        let name_os = entry.file_name();
        let name = name_os.to_string_lossy().into_owned();
        if name.starts_with("._") || name.starts_with('.') {
            continue;
        }
        if depth == 0 && is_reserved_top(&name) {
            continue;
        }
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        let rel = if rel_prefix.is_empty() {
            name.clone()
        } else {
            format!("{rel_prefix}/{name}")
        };
        if ft.is_dir() {
            walk_card_files(&entry.path(), &rel, depth + 1, out)?;
        } else if ft.is_file() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let (stem, ext) = match name.rsplit_once('.') {
                Some((s, e)) if !s.is_empty() => (s.to_string(), e.to_ascii_lowercase()),
                _ => (name.clone(), String::new()),
            };
            out.push(ScannedRawFile {
                rel_path: rel,
                folder_rel: rel_prefix.to_string(),
                name,
                stem,
                ext,
                bytes: meta.len(),
                modified: modified_secs(&meta),
            });
        }
    }
    Ok(())
}

/// Attempts to match a companion save/state stem in `folder_lower` against installed ROMs in the same folder.
/// Supports exact matches (`Game.srm` -> `Game`) and numeric save-state slot suffixes (`Game.0.ss` / `Game_0.ss` -> `Game`).
fn match_rom_in_folder(
    rom_map: &BTreeMap<(String, String), String>,
    folder_lower: &str,
    stem: &str,
    allow_slot_suffix: bool,
) -> Option<(String, Option<String>)> {
    let stem_lower = stem.to_ascii_lowercase();
    if let Some(rom_rel) = rom_map.get(&(folder_lower.to_string(), stem_lower.clone())) {
        return Some((rom_rel.clone(), None));
    }
    if allow_slot_suffix {
        for sep in ['.', '_'] {
            if let Some((base, suffix)) = stem.rsplit_once(sep)
                && !base.is_empty()
                && !suffix.is_empty()
                && suffix.len() <= 2
                && suffix.chars().all(|c| c.is_ascii_digit())
                && let Some(rom_rel) =
                    rom_map.get(&(folder_lower.to_string(), base.to_ascii_lowercase()))
            {
                return Some((rom_rel.clone(), Some(format!("Slot {suffix}"))));
            }
        }
    }
    None
}

/// Scans a Game Bub card for ROMs and stem-matched companion saves/save states belonging to `core_id`.
pub fn scan_core_files(
    card: &Path,
    core_id: &str,
    pinned_folder: Option<&str>,
) -> Result<GameBubCoreFilesView, TauError> {
    let core = find_core(card, core_id)?;
    let rom_extensions = core_rom_extensions(&core);
    let companions = core_companions(&core);
    let save_extensions: Vec<String> = companions.iter().map(|c| c.ext.clone()).collect();

    let clean_pin = match pinned_folder.map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => Some(normalize_rel_folder(raw)?),
        None => None,
    };

    let mut raw_files = Vec::new();
    match &clean_pin {
        Some(pin) => {
            let sub = card.join(pin);
            walk_card_files(&sub, pin, 1, &mut raw_files)?;
        }
        None => {
            walk_card_files(card, "", 0, &mut raw_files)?;
        }
    }

    let companion_by_ext: BTreeMap<String, CompanionSpec> =
        companions.into_iter().map(|c| (c.ext.clone(), c)).collect();
    let rom_ext_set: BTreeSet<String> = rom_extensions.iter().cloned().collect();
    let include_any_non_save_in_pin = rom_ext_set.is_empty() && clean_pin.is_some();

    let mut roms = Vec::new();
    let mut rom_map: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut candidate_companions: Vec<(ScannedRawFile, CompanionSpec)> = Vec::new();

    for f in raw_files {
        if let Some(comp) = companion_by_ext.get(&f.ext) {
            candidate_companions.push((f, comp.clone()));
            continue;
        }
        if rom_ext_set.contains(&f.ext) || (include_any_non_save_in_pin && !f.ext.is_empty()) {
            rom_map.insert(
                (
                    f.folder_rel.to_ascii_lowercase(),
                    f.stem.to_ascii_lowercase(),
                ),
                f.rel_path.clone(),
            );
            roms.push(GameBubRomEntry {
                rel_path: f.rel_path,
                folder_rel: f.folder_rel,
                name: f.name,
                stem: f.stem,
                ext: f.ext,
                bytes: f.bytes,
                modified: f.modified,
            });
        }
    }

    let mut saves = Vec::new();
    let mut unmatched_saves = Vec::new();

    for (f, comp) in candidate_companions {
        let folder_lower = f.folder_rel.to_ascii_lowercase();
        let allow_slot = comp.kind == GameBubSaveKind::SaveState;
        if let Some((matched_rom, slot_override)) =
            match_rom_in_folder(&rom_map, &folder_lower, &f.stem, allow_slot)
        {
            let game_stem = if slot_override.is_some() {
                f.stem
                    .rsplit_once(['.', '_'])
                    .map(|(b, _)| b.to_string())
                    .unwrap_or_else(|| f.stem.clone())
            } else {
                f.stem.clone()
            };
            saves.push(GameBubSaveEntry {
                rel_path: f.rel_path,
                folder_rel: f.folder_rel,
                name: f.name,
                game_stem,
                slot_label: slot_override.unwrap_or(comp.label),
                kind: comp.kind,
                bytes: f.bytes,
                modified: f.modified,
                matched_rom: Some(matched_rom),
            });
        } else {
            unmatched_saves.push(GameBubSaveEntry {
                rel_path: f.rel_path,
                folder_rel: f.folder_rel,
                name: f.name,
                game_stem: f.stem,
                slot_label: comp.label,
                kind: comp.kind,
                bytes: f.bytes,
                modified: f.modified,
                matched_rom: None,
            });
        }
    }

    roms.sort_by(|a, b| {
        (a.folder_rel.as_str(), a.name.as_str()).cmp(&(b.folder_rel.as_str(), b.name.as_str()))
    });
    saves.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.name.cmp(&b.name))
    });
    unmatched_saves.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.name.cmp(&b.name))
    });

    let total_rom_bytes = roms.iter().map(|r| r.bytes).sum();
    let total_save_bytes = saves.iter().map(|s| s.bytes).sum();

    Ok(GameBubCoreFilesView {
        core_id: core.core.id,
        pinned_folder: clean_pin,
        rom_extensions,
        save_extensions,
        roms,
        saves,
        unmatched_saves,
        total_rom_bytes,
        total_save_bytes,
    })
}

fn plan_id_from_parts(parts: &[String]) -> String {
    let mut hasher = Sha256::new();
    for p in parts {
        hasher.update(p.as_bytes());
        hasher.update([0u8]);
    }
    let hex = format!("{:x}", hasher.finalize());
    hex[..16].to_string()
}

fn ensure_backup_safe(backup_root: &Path, card: &Path) -> Result<(), TauError> {
    if backup_root.as_os_str().is_empty() || sync::backup_is_inside(backup_root, card) {
        return Err(TauError::e(
            ErrorCode::UnsafeBackupLocation,
            "Backup folder must be outside the card.",
        ));
    }
    Ok(())
}

fn unique_backup_dir(backup_root: &Path, prefix: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let base = backup_root.join(format!("{prefix}-{millis}"));
    if !base.exists() {
        return base;
    }
    let mut n = 1usize;
    loop {
        let candidate = backup_root.join(format!("{prefix}-{millis}-{n}"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

fn backup_and_verify(card: &Path, rel: &str, backup_dir: &Path) -> Result<(), TauError> {
    let src = card.join(rel);
    if !src.is_file() {
        return Ok(());
    }
    let bytes = fs::read(&src)?;
    let dest = backup_dir.join(rel);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    sync::write_durable(&dest, &bytes)?;
    if sync::sha256_bytes(&fs::read(&dest)?) != sync::sha256_bytes(&bytes) {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            format!("Backup of {rel} failed read-back verification; nothing was changed."),
        ));
    }
    Ok(())
}

/// Plans copying ROM files into `dest_folder` on a Game Bub card. Read-only.
pub fn plan_add_files(
    card: &Path,
    core_id: &str,
    dest_folder: &str,
    sources: &[String],
    replace: bool,
) -> Result<GameBubAddPlan, TauError> {
    let core = find_core(card, core_id)?;
    let clean_dest = normalize_rel_folder(dest_folder)?;
    let rom_exts: BTreeSet<String> = core_rom_extensions(&core).into_iter().collect();

    let mut items = Vec::new();
    let mut skipped = Vec::new();
    let mut new_files = 0usize;
    let mut replaced_files = 0usize;
    let mut bytes_to_write = 0u64;
    let mut seen_dests = BTreeSet::new();

    for raw_src in sources {
        let src_path = Path::new(raw_src);
        let Some(fname) = src_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
        else {
            skipped.push(format!("{raw_src} (invalid file name)"));
            continue;
        };
        let Ok(meta) = fs::metadata(src_path) else {
            skipped.push(format!("{fname} (file not found)"));
            continue;
        };
        if !meta.is_file() {
            skipped.push(format!("{fname} (not a file)"));
            continue;
        }
        let ext = fname
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        if !rom_exts.is_empty() && !rom_exts.contains(&ext) {
            skipped.push(format!(
                "{fname} (extension .{ext} not supported by {})",
                core.core.id
            ));
            continue;
        }
        let dest_rel = if clean_dest.is_empty() {
            fname.clone()
        } else {
            format!("{clean_dest}/{fname}")
        };
        if !seen_dests.insert(dest_rel.clone()) {
            continue;
        }
        let exists = card.join(&dest_rel).exists();
        if exists && !replace {
            skipped.push(format!("{fname} (already exists at {dest_rel})"));
            continue;
        }
        if exists {
            replaced_files += 1;
        } else {
            new_files += 1;
        }
        bytes_to_write += meta.len();
        items.push(GameBubAddItem {
            source: raw_src.clone(),
            dest_rel,
            bytes: meta.len(),
            replaces_existing: exists,
        });
    }

    let mut parts = vec![
        core.core.id.clone(),
        clean_dest.clone(),
        replace.to_string(),
        bytes_to_write.to_string(),
    ];
    for item in &items {
        parts.push(format!("{}:{}:{}", item.source, item.dest_rel, item.bytes));
    }
    let id = plan_id_from_parts(&parts);

    Ok(GameBubAddPlan {
        id,
        core_id: core.core.id,
        dest_folder: clean_dest,
        items,
        new_files,
        replaced_files,
        skipped,
        bytes_to_write,
    })
}

/// Executes a reviewed ROM copy plan onto a Game Bub card with atomic `.part` + read-back verification.
pub fn execute_add_files(
    card: &Path,
    core_id: &str,
    dest_folder: &str,
    sources: &[String],
    replace: bool,
    confirmation: &str,
    backup_root: &Path,
) -> Result<GameBubAddReport, TauError> {
    ensure_backup_safe(backup_root, card)?;
    let plan = plan_add_files(card, core_id, dest_folder, sources, replace)?;
    if plan.id != confirmation {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "Confirmation token does not match the current add plan.",
        ));
    }
    if plan.items.is_empty() {
        return Ok(GameBubAddReport {
            written: 0,
            replaced: 0,
            bytes_written: 0,
            backup_dir: None,
        });
    }

    let need_backup = plan.items.iter().any(|i| i.replaces_existing);
    let backup_dir = if need_backup {
        let dir = unique_backup_dir(backup_root, "gamebub-add");
        for item in plan.items.iter().filter(|i| i.replaces_existing) {
            backup_and_verify(card, &item.dest_rel, &dir)?;
        }
        Some(dir)
    } else {
        None
    };

    let mut written = 0usize;
    let mut replaced = 0usize;
    let mut bytes_written = 0u64;

    for item in &plan.items {
        let bytes = fs::read(&item.source)?;
        let expected_hash = sync::sha256_bytes(&bytes);
        let dest = card.join(&item.dest_rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let part = dest.with_file_name(format!(
            ".{}.tau-part",
            dest.file_name().unwrap().to_string_lossy()
        ));
        sync::write_durable(&part, &bytes)?;
        fs::rename(&part, &dest)?;
        let read_back = sync::read_back_bytes(&dest)?;
        if sync::sha256_bytes(&read_back) != expected_hash {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                format!("Read-back verification failed for {}", item.dest_rel),
            ));
        }
        written += 1;
        if item.replaces_existing {
            replaced += 1;
        }
        bytes_written += bytes.len() as u64;
    }

    Ok(GameBubAddReport {
        written,
        replaced,
        bytes_written,
        backup_dir: backup_dir.map(|d| d.to_string_lossy().into_owned()),
    })
}

/// Plans removing selected ROM/save files from a Game Bub card. Read-only.
pub fn plan_delete_files(
    card: &Path,
    core_id: &str,
    rel_paths: &[String],
) -> Result<GameBubDeletePlan, TauError> {
    let core = find_core(card, core_id)?;
    let mut clean_paths = Vec::new();
    let mut total_bytes = 0u64;
    let mut seen = BTreeSet::new();

    for raw in rel_paths {
        let rel = normalize_rel_file(raw)?;
        if !seen.insert(rel.clone()) {
            continue;
        }
        let full = card.join(&rel);
        let meta = fs::metadata(&full).map_err(|_| {
            TauError::e(
                ErrorCode::NotFound,
                format!("File not found on card: {rel}"),
            )
        })?;
        if !meta.is_file() {
            return Err(bad_path(format!("{rel} is not a file.")));
        }
        total_bytes += meta.len();
        clean_paths.push(rel);
    }

    clean_paths.sort();
    let mut parts = vec![core.core.id.clone(), total_bytes.to_string()];
    parts.extend(clean_paths.iter().cloned());
    let id = plan_id_from_parts(&parts);

    Ok(GameBubDeletePlan {
        id,
        core_id: core.core.id,
        rel_paths: clean_paths,
        total_bytes,
    })
}

/// Removes planned files from a Game Bub card after backing each up outside the card and verifying.
pub fn execute_delete_files(
    card: &Path,
    core_id: &str,
    rel_paths: &[String],
    confirmation: &str,
    backup_root: &Path,
) -> Result<GameBubDeleteReport, TauError> {
    ensure_backup_safe(backup_root, card)?;
    let plan = plan_delete_files(card, core_id, rel_paths)?;
    if plan.id != confirmation {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "Confirmation token does not match the current remove plan.",
        ));
    }

    let backup_dir = unique_backup_dir(backup_root, "gamebub-remove");
    fs::create_dir_all(&backup_dir)?;
    for rel in &plan.rel_paths {
        backup_and_verify(card, rel, &backup_dir)?;
    }

    let mut removed = 0usize;
    for rel in &plan.rel_paths {
        fs::remove_file(card.join(rel))?;
        removed += 1;
    }

    Ok(GameBubDeleteReport {
        removed,
        bytes_removed: plan.total_bytes,
        backup_dir: backup_dir.to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tau-gb-files-{name}-{}", crate::test_uniq()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn setup_snes_core(card: &Path) {
        let core_dir = card.join("cores/jeremy.SNES");
        fs::create_dir_all(&core_dir).unwrap();
        fs::write(core_dir.join("snes.bit"), b"BIT").unwrap();
        fs::write(
            core_dir.join("core.json"),
            r#"{
              "metadata":{"id":"jeremy.SNES","name":"Super Nintendo","author":"jeremy","version":"0.2.1"},
              "bitstreams":[{"target":"gamebub_rev4","filename":"snes.bit"}]
            }"#,
        )
        .unwrap();
        fs::write(
            core_dir.join("files.json"),
            r#"{"files":[
              {"id":0,"label":"ROM","user_selected":true,"extensions":["sfc","smc"]},
              {"id":1,"label":"Save","dependent_on_0":true,"extensions":["srm"]},
              {"id":2,"label":"States","dependent_on_0":true,"extensions":["ss"]}
            ]}"#,
        )
        .unwrap();
    }

    #[test]
    fn shared_folder_filters_roms_and_matches_companion_saves_by_stem() {
        let root = temp_dir("shared");
        let card = root.join("card");
        setup_snes_core(&card);

        let roms = card.join("ROMs");
        fs::create_dir_all(&roms).unwrap();
        fs::write(roms.join("Super Metroid.sfc"), b"SNESROM1").unwrap();
        fs::write(roms.join("Super Metroid.srm"), b"SRM1").unwrap();
        fs::write(roms.join("Super Metroid.ss"), b"SS0").unwrap();
        fs::write(roms.join("Super Metroid.1.ss"), b"SS1").unwrap();
        // Other system in the same folder:
        fs::write(roms.join("Sonic 2.md"), b"GENROM").unwrap();
        fs::write(roms.join("Sonic 2.srm"), b"GENSRM").unwrap();

        let view = scan_core_files(&card, "jeremy.SNES", None).unwrap();
        assert_eq!(view.rom_extensions, vec!["sfc", "smc"]);
        assert_eq!(view.roms.len(), 1);
        assert_eq!(view.roms[0].name, "Super Metroid.sfc");

        assert_eq!(view.saves.len(), 3);
        assert!(
            view.saves
                .iter()
                .all(|s| s.matched_rom.as_deref() == Some("ROMs/Super Metroid.sfc"))
        );
        assert!(view.saves.iter().any(|s| s.slot_label == "Slot 1"));

        assert_eq!(view.unmatched_saves.len(), 1);
        assert_eq!(view.unmatched_saves[0].name, "Sonic 2.srm");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pinned_folder_scopes_scan_strictly() {
        let root = temp_dir("pin");
        let card = root.join("card");
        setup_snes_core(&card);
        fs::create_dir_all(card.join("SNES")).unwrap();
        fs::create_dir_all(card.join("Other")).unwrap();
        fs::write(card.join("SNES/A.sfc"), b"A").unwrap();
        fs::write(card.join("Other/B.sfc"), b"B").unwrap();

        let view = scan_core_files(&card, "jeremy.SNES", Some("/SNES/")).unwrap();
        assert_eq!(view.pinned_folder.as_deref(), Some("SNES"));
        assert_eq!(view.roms.len(), 1);
        assert_eq!(view.roms[0].rel_path, "SNES/A.sfc");
        assert!(normalize_rel_folder("../bad").is_err());
        assert!(normalize_rel_folder("cores/jeremy.SNES").is_err());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn add_and_delete_files_enforce_tokens_and_create_verified_backups() {
        let root = temp_dir("add-del");
        let card = root.join("card");
        let backup = root.join("backup");
        setup_snes_core(&card);

        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let good_src = src_dir.join("Chrono Trigger.sfc");
        let bad_src = src_dir.join("Sonic.md");
        fs::write(&good_src, b"CHRONO").unwrap();
        fs::write(&bad_src, b"SONIC").unwrap();

        let sources = vec![
            good_src.to_string_lossy().into_owned(),
            bad_src.to_string_lossy().into_owned(),
        ];
        let add_plan = plan_add_files(&card, "jeremy.SNES", "ROMs/SNES", &sources, false).unwrap();
        assert_eq!(add_plan.items.len(), 1);
        assert_eq!(add_plan.skipped.len(), 1);

        assert!(
            execute_add_files(
                &card,
                "jeremy.SNES",
                "ROMs/SNES",
                &sources,
                false,
                "bad",
                &backup
            )
            .is_err()
        );
        let add_rep = execute_add_files(
            &card,
            "jeremy.SNES",
            "ROMs/SNES",
            &sources,
            false,
            &add_plan.id,
            &backup,
        )
        .unwrap();
        assert_eq!(add_rep.written, 1);
        assert!(card.join("ROMs/SNES/Chrono Trigger.sfc").is_file());

        let del_paths = vec!["ROMs/SNES/Chrono Trigger.sfc".to_string()];
        let del_plan = plan_delete_files(&card, "jeremy.SNES", &del_paths).unwrap();
        let del_rep =
            execute_delete_files(&card, "jeremy.SNES", &del_paths, &del_plan.id, &backup).unwrap();
        assert_eq!(del_rep.removed, 1);
        assert!(!card.join("ROMs/SNES/Chrono Trigger.sfc").exists());
        assert!(
            PathBuf::from(del_rep.backup_dir)
                .join("ROMs/SNES/Chrono Trigger.sfc")
                .is_file()
        );

        let _ = fs::remove_dir_all(root);
    }
}
