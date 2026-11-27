//! Alternative artwork and image pack support for Analogue Pocket banners/icons
//! and app-local overrides for other systems (Game Bub, MiSTer, DSpico, etc.).
//!
//! Community image packs distribute `Platforms/_images/<platform>.bin` (521x165,
//! 16 bpp monochrome stored rotated 90 deg CCW). Pocket card writes always go
//! through a reviewed plan -> verified external backup -> atomic `.part` + read-back
//! write flow. Non-Pocket systems store chosen artwork in the app's local override
//! directory without touching the SD card.

use crate::{
    ErrorCode, TauError, corehub,
    icon::{self, Fit},
    image, platforms, sync,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zip::ZipArchive;

/// Exact byte length of an Analogue Pocket platform banner (`Platforms/_images/<id>.bin`).
pub const BANNER_BYTES: usize = 521 * 165 * 2;
/// Maximum source image size accepted for local artwork conversion/override (16 MiB).
const MAX_LOCAL_IMAGE_BYTES: usize = 16 * 1024 * 1024;
/// Maximum size for an already-saved local PNG override read back for display.
const MAX_OVERRIDE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ArtworkSlot {
    Banner,
    Icon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", rename_all = "snake_case"))]
pub enum ArtworkSource {
    Pack {
        pack_zip: PathBuf,
        platform_id: String,
    },
    LocalFile {
        path: PathBuf,
        fit: Fit,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArtworkApplyPlan {
    pub id: String,
    pub target_rel: String,
    pub target_exists: bool,
    pub existing_bytes: u64,
    pub new_bytes: u64,
    pub slot: ArtworkSlot,
    pub preview_data_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArtworkApplyReport {
    pub target_rel: String,
    pub bytes_written: u64,
    pub backup_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PackApplyItem {
    pub platform_id: String,
    pub platform_name: String,
    pub category: String,
    pub pack_entry: String,
    pub target_rel: String,
    pub replaces_existing: bool,
    pub preview_data_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PackApplyPlan {
    pub id: String,
    pub pack_id: String,
    pub items: Vec<PackApplyItem>,
    pub will_replace: usize,
    pub will_add: usize,
    pub skipped_not_installed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PackApplyReport {
    pub pack_id: String,
    pub written: usize,
    pub backed_up: usize,
    pub backup_dir: Option<String>,
}

fn png_to_data_url(png_bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(png_bytes))
}

/// Decodes a 521x165 platform banner `.bin` into a `data:image/png;base64,...` URL.
pub fn bin_to_data_url(bytes: &[u8]) -> Result<String, TauError> {
    let png = icon::decode_platform_image(bytes)?;
    Ok(png_to_data_url(&png))
}

/// Decodes a 36x36 core `icon.bin` into a `data:image/png;base64,...` URL.
pub fn icon_bin_to_data_url(bytes: &[u8]) -> Result<String, TauError> {
    let png = icon::decode_icon_bin(bytes)?;
    Ok(png_to_data_url(&png))
}

fn normalize_zip_entry_name(name: &str) -> Option<String> {
    let normalized = name.replace('\\', "/");
    if normalized.ends_with('/') {
        return None;
    }
    let lower = normalized.to_ascii_lowercase();
    if lower.starts_with("__macosx/") || lower.contains("/__macosx/") {
        return None;
    }
    let file_name = normalized.rsplit('/').next().unwrap_or("");
    if file_name.is_empty() || file_name.starts_with("._") {
        return None;
    }
    Some(normalized)
}

/// Extracts the `<stem>` if `entry_name` is a `Platforms/_images/<stem>.bin` (or `_images/<stem>.bin`) entry.
fn extract_platform_bin_stem(normalized: &str) -> Option<String> {
    let lower = normalized.to_ascii_lowercase();
    let stem = if let Some(rest) = lower.strip_prefix("platforms/_images/") {
        rest.strip_suffix(".bin")?
    } else if let Some((_, tail)) = lower.rsplit_once("/platforms/_images/") {
        tail.strip_suffix(".bin")?
    } else if let Some(rest) = lower.strip_prefix("_images/") {
        rest.strip_suffix(".bin")?
    } else if let Some((_, tail)) = lower.rsplit_once("/_images/") {
        tail.strip_suffix(".bin")?
    } else {
        return None;
    };
    if stem.is_empty() || stem.contains('/') {
        return None;
    }
    Some(stem.to_string())
}

/// Scans a pack zip and returns `(archive, stem -> zip_entry_index)` for all valid `Platforms/_images/<stem>.bin` entries.
fn open_pack_banner_index(
    zip_path: &Path,
) -> Result<(ZipArchive<fs::File>, BTreeMap<String, usize>), TauError> {
    let file = fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| TauError::e(ErrorCode::Io, format!("Could not open image pack zip: {e}")))?;
    let mut matched = BTreeMap::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| TauError::e(ErrorCode::Io, format!("Invalid entry in zip: {e}")))?;
        if entry.is_dir() || entry.size() as usize != BANNER_BYTES {
            continue;
        }
        let Some(norm) = normalize_zip_entry_name(entry.name()) else {
            continue;
        };
        if let Some(stem) = extract_platform_bin_stem(&norm) {
            matched.entry(stem).or_insert(i);
        }
    }
    Ok((archive, matched))
}

/// Lists every valid platform `<stem>` contained in a community image pack zip.
pub fn list_pack_platforms(zip_path: &Path) -> Result<Vec<String>, TauError> {
    let (_, matched) = open_pack_banner_index(zip_path)?;
    Ok(matched.into_keys().collect())
}

/// Resolves a platform ID or community image pack `.bin` stem to a canonical [`platforms::PlatformSpec`].
pub fn resolve_platform_spec(stem: &str) -> Option<&'static platforms::PlatformSpec> {
    let raw_lower = stem.trim().to_ascii_lowercase();
    let normalized_id = match raw_lower.as_str() {
        "gameboy" => "gb",
        "gameboycolor" => "gbc",
        "gameboyadvance" => "gba",
        "pokemon_mini" => "poke_mini",
        "ngmvs" | "ngaes" => "ng",
        "pcengine" => "pce",
        "tgcd" | "turbografx_cd" => "pcecd",
        "486pc" => "dos",
        "ps1" => "psx",
        "nintendo_64" => "n64",
        "pc_engine" => "pce",
        "master_system" => "sms",
        "mega_drive" => "genesis",
        "game_gear" => "gg",
        other => other,
    };
    platforms::find(normalized_id)
}

/// Candidate `.bin` stems for `platform_id` in priority order (exact first, then canonical + aliases).
pub fn candidate_platform_stems(platform_id: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |s: &str| {
        let clean = s.trim().to_ascii_lowercase();
        if !clean.is_empty() && !out.contains(&clean) {
            out.push(clean);
        }
    };
    let raw_lower = platform_id.trim().to_ascii_lowercase();
    push(&raw_lower);

    if let Some(spec) = resolve_platform_spec(&raw_lower) {
        push(spec.id);
        let extra: &[&str] = match spec.id {
            "psx" => &["playstation", "ps1"],
            "2600" => &["atari2600"],
            "5200" => &["atari5200"],
            "7800" => &["atari7800"],
            "genesis" => &["megadrive"],
            "32x" => &["sega32x"],
            "megacd" => &["segacd"],
            "gg" => &["gamegear"],
            "pce" => &["tg16", "pcengine"],
            "pcecd" => &["tgcd", "turbografx_cd"],
            "sgx" => &["supergrafx"],
            "poke_mini" => &["pokemonmini", "pokemon_mini"],
            "amstrad" => &["cpc"],
            "dos" => &["ao486", "486pc"],
            "wonderswan" => &["ws", "wsc"],
            "zxspectrum" => &["zx"],
            "ng" => &["neogeo", "ngmvs", "ngaes"],
            "arcade" => &["mame"],
            "gb" => &["gameboy"],
            "gbc" => &["gameboycolor"],
            "gba" => &["gameboyadvance"],
            _ => &[],
        };
        for alias in extra {
            push(alias);
        }
    }
    out
}

/// Finds and reads the `.bin` payload in `zip_path` matching `platform_id` or a canonical alias.
/// Returns `Ok(Some((matched_stem, raw_bin_bytes)))` if found.
pub fn read_pack_platform_bin(
    zip_path: &Path,
    platform_id: &str,
) -> Result<Option<(String, Vec<u8>)>, TauError> {
    let candidates = candidate_platform_stems(platform_id);
    if candidates.is_empty() {
        return Ok(None);
    }
    let (mut archive, matched) = open_pack_banner_index(zip_path)?;

    for cand in &candidates {
        if let Some(&idx) = matched.get(cand) {
            let mut entry = archive
                .by_index(idx)
                .map_err(|e| TauError::e(ErrorCode::Io, format!("Invalid entry in zip: {e}")))?;
            let mut buf = Vec::with_capacity(BANNER_BYTES);
            entry
                .by_ref()
                .take(BANNER_BYTES as u64 + 1)
                .read_to_end(&mut buf)?;
            if buf.len() == BANNER_BYTES {
                return Ok(Some((cand.clone(), buf)));
            }
        }
    }
    Ok(None)
}

/// Opens a pack zip once and reads every valid `Platforms/_images/<stem>.bin` banner sorted by `<stem>`.
pub fn read_all_pack_banners(zip_path: &Path) -> Result<Vec<(String, Vec<u8>)>, TauError> {
    let (mut archive, matched) = open_pack_banner_index(zip_path)?;
    let mut out = Vec::with_capacity(matched.len());
    for (stem, idx) in matched {
        let mut entry = archive
            .by_index(idx)
            .map_err(|e| TauError::e(ErrorCode::Io, format!("Invalid entry in zip: {e}")))?;
        let mut buf = Vec::with_capacity(BANNER_BYTES);
        entry
            .by_ref()
            .take(BANNER_BYTES as u64 + 1)
            .read_to_end(&mut buf)?;
        if buf.len() == BANNER_BYTES {
            out.push((stem, buf));
        }
    }
    Ok(out)
}

fn read_local_image_bytes(source_path: &Path) -> Result<Vec<u8>, TauError> {
    let meta = fs::metadata(source_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            TauError::e(
                ErrorCode::NotFound,
                format!("Image file not found: {}", source_path.display()),
            )
        } else {
            TauError::from(e)
        }
    })?;
    if meta.len() > MAX_LOCAL_IMAGE_BYTES as u64 {
        return Err(TauError::e(
            ErrorCode::Io,
            "Image file is larger than 16 MB; choose a smaller PNG or JPEG.",
        ));
    }
    let bytes = fs::read(source_path)?;
    if bytes.is_empty() {
        return Err(TauError::e(
            ErrorCode::Io,
            "The selected image file is empty.",
        ));
    }
    Ok(bytes)
}

fn encode_rgb_as_png(width: u32, height: u32, rgb: &[u8]) -> Result<Vec<u8>, TauError> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| TauError::e(ErrorCode::Io, format!("Could not encode PNG: {e}")))?;
        writer
            .write_image_data(rgb)
            .map_err(|e| TauError::e(ErrorCode::Io, format!("Could not encode PNG: {e}")))?;
    }
    Ok(out)
}

fn local_bytes_to_png(bytes: &[u8]) -> Result<Vec<u8>, TauError> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        // Validate PNG header before accepting it as an override.
        let _ = image::decode_source_image(bytes)?;
        return Ok(bytes.to_vec());
    }
    let (w, h, rgb) = image::decode_source_image(bytes)?;
    encode_rgb_as_png(w, h, &rgb)
}

/// Generates a PNG data URL preview of a local image file.
/// When `pocket_monochrome` is true, simulates the exact 521x165 / 36x36 monochrome `.bin` output.
pub fn preview_local_image(
    source_path: &Path,
    slot: ArtworkSlot,
    fit: Fit,
    pocket_monochrome: bool,
) -> Result<String, TauError> {
    let bytes = read_local_image_bytes(source_path)?;
    if pocket_monochrome {
        match slot {
            ArtworkSlot::Banner => {
                let bin = icon::encode_platform_image(&bytes, fit)?;
                bin_to_data_url(&bin)
            }
            ArtworkSlot::Icon => {
                let bin = icon::encode_icon_from_image(&bytes, fit)?;
                icon_bin_to_data_url(&bin)
            }
        }
    } else {
        let png = local_bytes_to_png(&bytes)?;
        Ok(png_to_data_url(&png))
    }
}

fn validate_target_id(id: &str) -> Result<&str, TauError> {
    let trimmed = id.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || trimmed.contains('\0')
    {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            "Invalid target identifier.",
        ));
    }
    Ok(trimmed)
}

fn slot_target_rel(slot: ArtworkSlot, target_id: &str) -> Result<String, TauError> {
    let clean = validate_target_id(target_id)?;
    Ok(match slot {
        ArtworkSlot::Banner => format!("Platforms/_images/{clean}.bin"),
        ArtworkSlot::Icon => format!("Cores/{clean}/icon.bin"),
    })
}

fn prepare_slot_bin(
    slot: ArtworkSlot,
    source: &ArtworkSource,
) -> Result<(Vec<u8>, String), TauError> {
    match source {
        ArtworkSource::Pack {
            pack_zip,
            platform_id,
        } => {
            if slot != ArtworkSlot::Banner {
                return Err(TauError::e(
                    ErrorCode::InvalidPathReference,
                    "Image pack items can only be applied to platform banners.",
                ));
            }
            let Some((_, bin)) = read_pack_platform_bin(pack_zip, platform_id)? else {
                return Err(TauError::e(
                    ErrorCode::NotFound,
                    format!("This image pack has no artwork for “{platform_id}”."),
                ));
            };
            let preview = bin_to_data_url(&bin)?;
            Ok((bin, preview))
        }
        ArtworkSource::LocalFile { path, fit } => {
            let bytes = read_local_image_bytes(path)?;
            match slot {
                ArtworkSlot::Banner => {
                    let bin = icon::encode_platform_image(&bytes, *fit)?;
                    let preview = bin_to_data_url(&bin)?;
                    Ok((bin, preview))
                }
                ArtworkSlot::Icon => {
                    let bin = icon::encode_icon_from_image(&bytes, *fit)?;
                    let preview = icon_bin_to_data_url(&bin)?;
                    Ok((bin, preview))
                }
            }
        }
    }
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

/// Plans writing an alternative banner or icon onto an Analogue Pocket card without changing any files.
pub fn plan_apply(
    card: &Path,
    slot: ArtworkSlot,
    target_id: &str,
    source: &ArtworkSource,
) -> Result<ArtworkApplyPlan, TauError> {
    let target_rel = slot_target_rel(slot, target_id)?;
    let (new_bin, preview_data_url) = prepare_slot_bin(slot, source)?;
    let target_path = card.join(&target_rel);
    let (target_exists, existing_bytes) = match fs::metadata(&target_path) {
        Ok(m) => (m.is_file(), m.len()),
        Err(_) => (false, 0),
    };
    let bin_hash = sync::sha256_bytes(&new_bin);
    let id = plan_id_from_parts(&[
        target_rel.clone(),
        target_exists.to_string(),
        existing_bytes.to_string(),
        bin_hash,
    ]);
    Ok(ArtworkApplyPlan {
        id,
        target_rel,
        target_exists,
        existing_bytes,
        new_bytes: new_bin.len() as u64,
        slot,
        preview_data_url,
    })
}

fn ensure_backup_root_safe(backup_root: &Path, card: &Path) -> Result<(), TauError> {
    if backup_root.as_os_str().is_empty() || sync::backup_is_inside(backup_root, card) {
        return Err(TauError::e(
            ErrorCode::UnsafeBackupLocation,
            "Artwork backup folder must be outside the card.",
        ));
    }
    Ok(())
}

fn unique_artwork_backup_dir(backup_root: &Path) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let base = backup_root.join(format!("artwork-{millis}"));
    if !base.exists() {
        return base;
    }
    let mut n = 1usize;
    loop {
        let candidate = backup_root.join(format!("artwork-{millis}-{n}"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

fn backup_existing_file(
    card: &Path,
    rel: &str,
    backup_dir: &Path,
) -> Result<Option<PathBuf>, TauError> {
    let source = card.join(rel);
    if !source.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(&source)?;
    let dest = backup_dir.join(rel);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    sync::write_durable(&dest, &bytes)?;
    let hash = sync::sha256_bytes(&bytes);
    if sync::sha256_bytes(&fs::read(&dest)?) != hash {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            format!("Backup of {rel} did not read back identically; nothing was changed."),
        ));
    }
    Ok(Some(dest))
}

fn write_card_bin_atomic(dest: &Path, bytes: &[u8], rel: &str) -> Result<(), TauError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let part = dest.with_extension("bin.part");
    sync::write_durable(&part, bytes)?;
    fs::rename(&part, dest)?;
    let read_back = sync::read_back_bytes(dest)?;
    if sync::sha256_bytes(&read_back) != sync::sha256_bytes(bytes) {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            format!("Read-back verification failed after writing {rel}."),
        ));
    }
    Ok(())
}

/// Applies a reviewed single-artwork plan to an Analogue Pocket card with verified backup + read-back.
pub fn execute_apply(
    card: &Path,
    slot: ArtworkSlot,
    target_id: &str,
    source: &ArtworkSource,
    confirmation: &str,
    backup_root: &Path,
) -> Result<ArtworkApplyReport, TauError> {
    ensure_backup_root_safe(backup_root, card)?;
    let plan = plan_apply(card, slot, target_id, source)?;
    if plan.id != confirmation {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "Confirmation token does not match the current artwork plan; review again.",
        ));
    }
    let (new_bin, _) = prepare_slot_bin(slot, source)?;
    let backup_path = if plan.target_exists {
        let dir = unique_artwork_backup_dir(backup_root);
        backup_existing_file(card, &plan.target_rel, &dir)?
            .map(|p| p.to_string_lossy().into_owned())
    } else {
        None
    };
    let dest = card.join(&plan.target_rel);
    write_card_bin_atomic(&dest, &new_bin, &plan.target_rel)?;
    Ok(ArtworkApplyReport {
        target_rel: plan.target_rel,
        bytes_written: new_bin.len() as u64,
        backup_path,
    })
}

/// Plans applying matching platform banners from a cached pack zip to installed Pocket platforms.
/// When `only_platforms` is `Some`, only platforms whose ID is in that list are included.
pub fn plan_pack_apply(
    card: &Path,
    pack_id: &str,
    pack_zip: &Path,
    only_platforms: Option<&[String]>,
) -> Result<PackApplyPlan, TauError> {
    let pack_platforms = list_pack_platforms(pack_zip)?;
    let tree = corehub::platform_tree(card)?;
    let filter_set: Option<BTreeSet<String>> = only_platforms.map(|list| {
        list.iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    });
    let mut items = Vec::new();
    let mut matched_stems = BTreeSet::new();
    let mut will_replace = 0usize;
    let mut will_add = 0usize;

    for node in &tree {
        let pid = node.platform.id.as_str();
        if pid == corehub::NO_PLATFORM || pid == "gamebub" {
            continue;
        }
        if let Some(ref allowed) = filter_set
            && !allowed.contains(&pid.to_ascii_lowercase())
        {
            continue;
        }
        if let Some((stem, bin)) = read_pack_platform_bin(pack_zip, pid)? {
            matched_stems.insert(stem.clone());
            let target_rel = format!("Platforms/_images/{pid}.bin");
            let replaces_existing = card.join(&target_rel).is_file();
            if replaces_existing {
                will_replace += 1;
            } else {
                will_add += 1;
            }
            let spec = platforms::find(pid);
            let platform_name = if !node.platform.name.is_empty() && node.platform.name != pid {
                node.platform.name.clone()
            } else if let Some(s) = spec {
                s.name.to_string()
            } else {
                pid.to_string()
            };
            let category = if !node.platform.category.is_empty() {
                node.platform.category.clone()
            } else if let Some(s) = spec {
                s.category.to_string()
            } else {
                "Other".to_string()
            };
            let preview_data_url = bin_to_data_url(&bin)?;
            items.push(PackApplyItem {
                platform_id: pid.to_string(),
                platform_name,
                category,
                pack_entry: stem,
                target_rel,
                replaces_existing,
                preview_data_url,
            });
        }
    }

    let skipped_not_installed = pack_platforms
        .iter()
        .filter(|s| !matched_stems.contains(*s))
        .count();

    let mut hash_parts = vec![
        pack_id.to_string(),
        pack_zip.to_string_lossy().into_owned(),
        will_replace.to_string(),
        will_add.to_string(),
    ];
    if let Some(ref allowed) = filter_set {
        let joined: Vec<String> = allowed.iter().cloned().collect();
        hash_parts.push(format!("filter:{}", joined.join(",")));
    }
    for item in &items {
        hash_parts.push(format!(
            "{}:{}:{}:{}",
            item.platform_id, item.pack_entry, item.target_rel, item.replaces_existing
        ));
    }
    let id = plan_id_from_parts(&hash_parts);

    Ok(PackApplyPlan {
        id,
        pack_id: pack_id.to_string(),
        items,
        will_replace,
        will_add,
        skipped_not_installed,
    })
}

/// Applies a reviewed pack batch plan onto an Analogue Pocket card, backing up all overwritten `.bin` files first.
pub fn execute_pack_apply(
    card: &Path,
    pack_id: &str,
    pack_zip: &Path,
    only_platforms: Option<&[String]>,
    confirmation: &str,
    backup_root: &Path,
) -> Result<PackApplyReport, TauError> {
    ensure_backup_root_safe(backup_root, card)?;
    let plan = plan_pack_apply(card, pack_id, pack_zip, only_platforms)?;
    if plan.id != confirmation {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "Confirmation token does not match the current pack plan; review again.",
        ));
    }
    if plan.items.is_empty() {
        return Ok(PackApplyReport {
            pack_id: pack_id.to_string(),
            written: 0,
            backed_up: 0,
            backup_dir: None,
        });
    }

    // Read all pack payloads up front so we never leave partial card state on zip read errors.
    let mut payloads: Vec<(String, Vec<u8>, bool)> = Vec::with_capacity(plan.items.len());
    for item in &plan.items {
        let Some((_, bin)) = read_pack_platform_bin(pack_zip, &item.pack_entry)? else {
            return Err(TauError::e(
                ErrorCode::SourceChangedSincePlan,
                format!(
                    "Pack entry “{}” disappeared since planning.",
                    item.pack_entry
                ),
            ));
        };
        payloads.push((item.target_rel.clone(), bin, item.replaces_existing));
    }

    let need_backup = payloads.iter().any(|(_, _, replaces)| *replaces);
    let backup_dir = if need_backup {
        Some(unique_artwork_backup_dir(backup_root))
    } else {
        None
    };

    let mut backed_up = 0usize;
    if let Some(dir) = &backup_dir {
        for (rel, _, replaces) in &payloads {
            if *replaces && backup_existing_file(card, rel, dir)?.is_some() {
                backed_up += 1;
            }
        }
    }

    let mut written = 0usize;
    for (rel, bytes, _) in &payloads {
        write_card_bin_atomic(&card.join(rel), bytes, rel)?;
        written += 1;
    }

    Ok(PackApplyReport {
        pack_id: pack_id.to_string(),
        written,
        backed_up,
        backup_dir: backup_dir.map(|d| d.to_string_lossy().into_owned()),
    })
}

/// Turns an arbitrary override key (such as `"game_bub:core:Retro-Bub.Lynx"`) into a safe filename stem.
pub fn sanitize_override_key(key: &str) -> Result<String, TauError> {
    let trimmed = key.trim();
    if trimmed.is_empty() || trimmed.contains("..") {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            "Invalid artwork override key.",
        ));
    }
    let replaced = trimmed.replace(':', "__");
    if !replaced
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            "Artwork override key contains unsupported characters.",
        ));
    }
    Ok(replaced)
}

fn override_png_path(overrides_dir: &Path, key: &str) -> Result<PathBuf, TauError> {
    let stem = sanitize_override_key(key)?;
    Ok(overrides_dir.join(format!("{stem}.png")))
}

/// Reads an app-local artwork override PNG and returns it as a `data:image/png;base64,...` URL.
pub fn read_override_data_url(overrides_dir: &Path, key: &str) -> Result<Option<String>, TauError> {
    let path = override_png_path(overrides_dir, key)?;
    let meta = match fs::metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(TauError::from(e)),
    };
    if !meta.is_file() || meta.len() > MAX_OVERRIDE_BYTES {
        return Ok(None);
    }
    let bytes = fs::read(&path)?;
    Ok(Some(png_to_data_url(&bytes)))
}

/// Saves a PNG artwork override inside `overrides_dir` from either a community pack banner or a local image file.
pub fn write_override_from_source(
    overrides_dir: &Path,
    key: &str,
    source: &ArtworkSource,
) -> Result<String, TauError> {
    let dest = override_png_path(overrides_dir, key)?;
    let png_bytes = match source {
        ArtworkSource::Pack {
            pack_zip,
            platform_id,
        } => {
            let Some((_, bin)) = read_pack_platform_bin(pack_zip, platform_id)? else {
                return Err(TauError::e(
                    ErrorCode::NotFound,
                    format!("This image pack has no artwork for “{platform_id}”."),
                ));
            };
            icon::decode_platform_image(&bin)?
        }
        ArtworkSource::LocalFile { path, .. } => {
            let raw = read_local_image_bytes(path)?;
            local_bytes_to_png(&raw)?
        }
    };
    fs::create_dir_all(overrides_dir)?;
    let part = dest.with_extension("png.part");
    sync::write_durable(&part, &png_bytes)?;
    fs::rename(&part, &dest)?;
    Ok(png_to_data_url(&png_bytes))
}

/// Removes an app-local artwork override PNG. Returns `true` if a file existed and was deleted.
pub fn clear_override(overrides_dir: &Path, key: &str) -> Result<bool, TauError> {
    let dest = override_png_path(overrides_dir, key)?;
    match fs::remove_file(&dest) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(TauError::from(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn sample_png(w: u32, h: u32) -> Vec<u8> {
        let mut px = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            for x in 0..w {
                px.push(if ((x + y) % 4) == 0 { 255 } else { 40 });
            }
        }
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, w, h);
            e.set_color(png::ColorType::Grayscale);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&px).unwrap();
        }
        out
    }

    fn sample_banner_bin(fill: u8) -> Vec<u8> {
        let mut bin = vec![0u8; BANNER_BYTES];
        for i in (0..BANNER_BYTES).step_by(2) {
            bin[i] = fill;
        }
        bin
    }

    fn create_pack_zip(path: &Path, entries: &[(&str, Vec<u8>)]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let file = fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opt = SimpleFileOptions::default();
        for (name, bytes) in entries {
            zip.start_file(*name, opt).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tau-artwork-{name}-{}", crate::test_uniq()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn pack_zip_lists_valid_banners_and_resolves_aliases() {
        let root = temp_dir("pack-list");
        let zip_path = root.join("pack.zip");
        create_pack_zip(
            &zip_path,
            &[
                ("Wrapper/Platforms/_images/gb.bin", sample_banner_bin(220)),
                (
                    "Wrapper/Platforms/_images/playstation.bin",
                    sample_banner_bin(180),
                ),
                ("__MACOSX/Platforms/_images/._gb.bin", sample_banner_bin(10)),
                ("Wrapper/Platforms/_images/bad.bin", vec![1, 2, 3]),
            ],
        );

        let platforms = list_pack_platforms(&zip_path).unwrap();
        assert_eq!(platforms, vec!["gb".to_string(), "playstation".to_string()]);

        let all_banners = read_all_pack_banners(&zip_path).unwrap();
        assert_eq!(all_banners.len(), 2);
        assert_eq!(all_banners[0].0, "gb");
        assert_eq!(all_banners[1].0, "playstation");

        let (stem, bin) = read_pack_platform_bin(&zip_path, "gameboy")
            .unwrap()
            .unwrap();
        assert_eq!(stem, "gb");
        assert_eq!(bin.len(), BANNER_BYTES);
        assert!(
            bin_to_data_url(&bin)
                .unwrap()
                .starts_with("data:image/png;base64,")
        );

        let (ps_stem, _) = read_pack_platform_bin(&zip_path, "psx").unwrap().unwrap();
        assert_eq!(ps_stem, "playstation");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn plan_and_execute_apply_with_verified_backup_and_token_check() {
        let root = temp_dir("apply");
        let card = root.join("card");
        let backup = root.join("backup");
        fs::create_dir_all(card.join("Platforms/_images")).unwrap();
        let original = sample_banner_bin(11);
        fs::write(card.join("Platforms/_images/gb.bin"), &original).unwrap();

        let png_path = root.join("custom.png");
        fs::write(&png_path, sample_png(260, 80)).unwrap();

        let src = ArtworkSource::LocalFile {
            path: png_path,
            fit: Fit::Cover,
        };
        let plan = plan_apply(&card, ArtworkSlot::Banner, "gb", &src).unwrap();
        assert!(plan.target_exists);
        assert_eq!(plan.target_rel, "Platforms/_images/gb.bin");
        assert_eq!(plan.new_bytes, BANNER_BYTES as u64);
        assert!(plan.preview_data_url.starts_with("data:image/png;base64,"));

        // Refuses mismatched confirmation without touching the card.
        assert!(
            execute_apply(
                &card,
                ArtworkSlot::Banner,
                "gb",
                &src,
                "wrong-token",
                &backup
            )
            .is_err()
        );
        assert_eq!(
            fs::read(card.join("Platforms/_images/gb.bin")).unwrap(),
            original
        );

        let report =
            execute_apply(&card, ArtworkSlot::Banner, "gb", &src, &plan.id, &backup).unwrap();
        assert_eq!(report.bytes_written, BANNER_BYTES as u64);
        let backup_file = PathBuf::from(report.backup_path.unwrap());
        assert_eq!(fs::read(&backup_file).unwrap(), original);
        assert_ne!(
            fs::read(card.join("Platforms/_images/gb.bin")).unwrap(),
            original
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn plan_and_execute_pack_apply_updates_only_installed_platforms() {
        let root = temp_dir("pack-apply");
        let card = root.join("card");
        let backup = root.join("backup");
        fs::create_dir_all(card.join("Cores/Spiritualized.GB")).unwrap();
        fs::create_dir_all(card.join("Assets")).unwrap();
        fs::create_dir_all(card.join("Platforms/_images")).unwrap();
        fs::write(
            card.join("Cores/Spiritualized.GB/core.json"),
            r#"{"core":{"metadata":{"platform_ids":["gb"],"shortname":"GB","author":"Spiritualized","version":"1.3.0"}}}"#,
        )
        .unwrap();
        let old_gb = sample_banner_bin(5);
        fs::write(card.join("Platforms/_images/gb.bin"), &old_gb).unwrap();

        let zip_path = root.join("pack.zip");
        let new_gb = sample_banner_bin(250);
        create_pack_zip(
            &zip_path,
            &[
                ("Platforms/_images/gb.bin", new_gb.clone()),
                ("Platforms/_images/snes.bin", sample_banner_bin(99)),
            ],
        );

        let plan = plan_pack_apply(&card, "test-pack", &zip_path, None).unwrap();
        assert_eq!(plan.will_replace, 1);
        assert_eq!(plan.will_add, 0);
        assert_eq!(plan.skipped_not_installed, 1);
        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.items[0].platform_name, "Game Boy");
        assert!(
            plan.items[0]
                .preview_data_url
                .starts_with("data:image/png;base64,")
        );

        // Filtering out `gb` yields zero items.
        let filtered =
            plan_pack_apply(&card, "test-pack", &zip_path, Some(&["snes".to_string()])).unwrap();
        assert!(filtered.items.is_empty());

        let report =
            execute_pack_apply(&card, "test-pack", &zip_path, None, &plan.id, &backup).unwrap();
        assert_eq!(report.written, 1);
        assert_eq!(report.backed_up, 1);
        assert_eq!(
            fs::read(card.join("Platforms/_images/gb.bin")).unwrap(),
            new_gb
        );
        assert!(!card.join("Platforms/_images/snes.bin").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn app_override_round_trip_and_sanitization() {
        let root = temp_dir("overrides");
        let key = "game_bub:core:Retro-Bub.Lynx";
        assert!(read_override_data_url(&root, key).unwrap().is_none());

        let png_path = root.join("lynx.png");
        fs::write(&png_path, sample_png(120, 40)).unwrap();
        let src = ArtworkSource::LocalFile {
            path: png_path,
            fit: Fit::Contain,
        };
        let written_url = write_override_from_source(&root, key, &src).unwrap();
        assert!(written_url.starts_with("data:image/png;base64,"));
        assert_eq!(
            read_override_data_url(&root, key).unwrap().as_deref(),
            Some(written_url.as_str())
        );

        assert!(clear_override(&root, key).unwrap());
        assert!(!clear_override(&root, key).unwrap());
        assert!(read_override_data_url(&root, key).unwrap().is_none());
        assert!(sanitize_override_key("../bad").is_err());

        let _ = fs::remove_dir_all(root);
    }
}
