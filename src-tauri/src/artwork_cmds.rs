use crate::{PREFS_FILE, artwork_backup_root, begin_card_write, config_dir, data_dir, read_prefs};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};
use tau_core::{
    ErrorCode, TauError,
    artwork::{
        self, ArtworkApplyPlan, ArtworkApplyReport, ArtworkSlot, ArtworkSource, PackApplyPlan,
        PackApplyReport,
    },
    corehub,
    icon::{self, Fit},
};
use tau_net::image_packs::{self, ImagePackEntry};
use tauri::Manager;

#[derive(Serialize)]
pub struct ArtworkPacksState {
    pub packs: Vec<ImagePackEntry>,
    pub enabled_ids: Vec<String>,
}

#[derive(Serialize)]
pub struct ArtworkSample {
    pub platform_id: String,
    pub name: String,
    pub category: String,
    pub manufacturer: String,
    pub installed_platform_id: Option<String>,
    pub replaces_existing: bool,
    pub data_url: String,
}

#[derive(Serialize)]
pub struct ArtworkAlternative {
    pub pack_id: String,
    pub pack_name: String,
    pub owner: String,
    pub variant: Option<String>,
    pub matched_platform: String,
    pub data_url: String,
}

fn cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(|e| TauError {
            code: ErrorCode::Io,
            message: e.to_string(),
        })?
        .join("image-packs"))
}

fn overrides_dir(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    Ok(data_dir(app)?.join("artwork-overrides"))
}

fn decoded_pack_dir(cache: &Path, pack_id: &str) -> PathBuf {
    let safe: String = pack_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    cache.join("decoded").join(safe)
}

fn valid_cached_stem(stem: &str) -> bool {
    !stem.is_empty()
        && !stem.contains("..")
        && stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn png_data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(bytes))
}

fn read_cached_banner_data_url(
    decoded_dir: &Path,
    zip_mtime: Option<SystemTime>,
    stem: &str,
) -> Option<String> {
    if !valid_cached_stem(stem) {
        return None;
    }
    let png_path = decoded_dir.join(format!("{stem}.png"));
    let meta = std::fs::metadata(&png_path).ok()?;
    if !meta.is_file() {
        return None;
    }
    if let (Some(zm), Ok(pm)) = (zip_mtime, meta.modified())
        && pm < zm
    {
        return None;
    }
    let bytes = std::fs::read(&png_path).ok()?;
    Some(png_data_url(&bytes))
}

fn write_cached_banner_png(decoded_dir: &Path, stem: &str, png_bytes: &[u8]) {
    if !valid_cached_stem(stem) {
        return;
    }
    let _ = std::fs::create_dir_all(decoded_dir);
    let dest = decoded_dir.join(format!("{stem}.png"));
    let part = dest.with_extension("png.part");
    if std::fs::write(&part, png_bytes).is_ok() {
        let _ = std::fs::rename(&part, &dest);
    }
}

fn find_pack(cache: &Path, pack_id: &str) -> Result<ImagePackEntry, TauError> {
    let packs = image_packs::fetch_index(cache, false)?;
    packs
        .into_iter()
        .find(|p| p.id == pack_id)
        .ok_or_else(|| TauError {
            code: ErrorCode::NotFound,
            message: format!("Image pack “{pack_id}” is not in the directory."),
        })
}

fn resolve_source(
    app: &tauri::AppHandle,
    pack_id: Option<String>,
    platform_id: &str,
    source_path: Option<String>,
    fit: Option<Fit>,
) -> Result<ArtworkSource, TauError> {
    if let Some(pid) = pack_id.filter(|s| !s.trim().is_empty()) {
        let cache = cache_dir(app)?;
        let entry = find_pack(&cache, pid.trim())?;
        let zip = image_packs::ensure_pack_zip(&cache, &entry, false)?;
        return Ok(ArtworkSource::Pack {
            pack_zip: zip,
            platform_id: platform_id.to_string(),
        });
    }
    if let Some(path) = source_path.filter(|s| !s.trim().is_empty()) {
        return Ok(ArtworkSource::LocalFile {
            path: PathBuf::from(path),
            fit: fit.unwrap_or_default(),
        });
    }
    Err(TauError {
        code: ErrorCode::InvalidPathReference,
        message: "Choose an image pack entry or a local image file first.".into(),
    })
}

/// Lists community image packs along with which ones the user has enabled for per-core lookups.
#[tauri::command(async)]
pub fn artwork_list_packs(
    app: tauri::AppHandle,
    force: bool,
) -> Result<ArtworkPacksState, TauError> {
    let cache = cache_dir(&app)?;
    let packs = image_packs::fetch_index(&cache, force)?;
    let enabled_ids = read_prefs(&app)?.enabled_image_packs;
    Ok(ArtworkPacksState { packs, enabled_ids })
}

/// Updates the user's enabled community image pack IDs in preferences.
#[tauri::command(async)]
pub fn artwork_set_enabled_packs(
    app: tauri::AppHandle,
    enabled_ids: Vec<String>,
) -> Result<Vec<String>, TauError> {
    let mut prefs = read_prefs(&app)?;
    let mut clean: Vec<String> = Vec::new();
    for id in enabled_ids {
        let t = id.trim().to_string();
        if !t.is_empty() && !clean.contains(&t) {
            clean.push(t);
        }
    }
    prefs.enabled_image_packs = clean.clone();
    let dir = config_dir(&app)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join(PREFS_FILE),
        serde_json::to_vec_pretty(&prefs).map_err(|e| TauError {
            code: ErrorCode::Json,
            message: e.to_string(),
        })?,
    )?;
    Ok(clean)
}

/// Downloads (or refreshes) one community image pack zip into the local cache.
#[tauri::command(async)]
pub fn artwork_download_pack(
    app: tauri::AppHandle,
    pack_id: String,
    force: bool,
) -> Result<ImagePackEntry, TauError> {
    let cache = cache_dir(&app)?;
    let mut entry = find_pack(&cache, &pack_id)?;
    if force {
        let _ = std::fs::remove_dir_all(decoded_pack_dir(&cache, &pack_id));
    }
    let zip = image_packs::ensure_pack_zip(&cache, &entry, force)?;
    let count = artwork::list_pack_platforms(&zip).map(|v| v.len()).unwrap_or(0);
    entry.cached = true;
    entry.cached_platforms = count;
    Ok(entry)
}

/// Returns decoded banners from one image pack with canonical category & manufacturer metadata.
/// Uses `<cache>/decoded/<pack>/` PNGs so repeat opens never re-decompress or re-encode from the zip.
#[tauri::command(async)]
pub fn artwork_pack_samples(
    app: tauri::AppHandle,
    pack_id: String,
    limit: usize,
    card: Option<String>,
) -> Result<Vec<ArtworkSample>, TauError> {
    let cache = cache_dir(&app)?;
    let entry = find_pack(&cache, &pack_id)?;
    let zip = image_packs::ensure_pack_zip(&cache, &entry, false)?;
    let zip_mtime = std::fs::metadata(&zip).and_then(|m| m.modified()).ok();
    let d_dir = decoded_pack_dir(&cache, &pack_id);
    let cap = limit.clamp(1, 500);

    // Fast path: if every stem in the pack already has a cached PNG newer than the zip, read straight from disk.
    let stems = artwork::list_pack_platforms(&zip)?;
    let mut banner_pairs: Vec<(String, String)> = Vec::with_capacity(stems.len().min(cap));
    let mut all_cached = !stems.is_empty();
    for stem in stems.iter().take(cap) {
        if let Some(url) = read_cached_banner_data_url(&d_dir, zip_mtime, stem) {
            banner_pairs.push((stem.clone(), url));
        } else {
            all_cached = false;
            break;
        }
    }

    if !all_cached {
        banner_pairs.clear();
        for (stem, bin) in artwork::read_all_pack_banners(&zip)?.into_iter().take(cap) {
            if let Some(cached_url) = read_cached_banner_data_url(&d_dir, zip_mtime, &stem) {
                banner_pairs.push((stem, cached_url));
                continue;
            }
            if let Ok(png) = icon::decode_platform_image(&bin) {
                write_cached_banner_png(&d_dir, &stem, &png);
                banner_pairs.push((stem, png_data_url(&png)));
            }
        }
    }

    let installed_nodes = card
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .and_then(|c| corehub::platform_tree(Path::new(c)).ok())
        .unwrap_or_default();

    let installed_candidates: Vec<(&corehub::PlatformInfo, Vec<String>)> = installed_nodes
        .iter()
        .filter(|n| n.platform.id != corehub::NO_PLATFORM && n.platform.id != "gamebub")
        .map(|n| (&n.platform, artwork::candidate_platform_stems(&n.platform.id)))
        .collect();

    let mut samples = Vec::with_capacity(banner_pairs.len());
    for (stem, data_url) in banner_pairs {
        let matched_node = installed_candidates
            .iter()
            .find(|(p, _)| p.id.eq_ignore_ascii_case(&stem))
            .or_else(|| {
                installed_candidates
                    .iter()
                    .find(|(_, cands)| cands.contains(&stem))
            })
            .map(|(p, _)| *p);

        let spec = matched_node
            .and_then(|p| artwork::resolve_platform_spec(&p.id))
            .or_else(|| artwork::resolve_platform_spec(&stem));

        let name = matched_node
            .map(|p| p.name.as_str())
            .filter(|n| !n.is_empty() && *n != stem)
            .or_else(|| spec.map(|s| s.name))
            .map(str::to_string)
            .unwrap_or_else(|| stem.to_uppercase());

        let category = matched_node
            .map(|p| p.category.as_str())
            .filter(|c| !c.is_empty())
            .or_else(|| spec.map(|s| s.category))
            .unwrap_or("Other")
            .to_string();

        let manufacturer = matched_node
            .map(|p| p.manufacturer.as_str())
            .filter(|m| !m.is_empty())
            .or_else(|| spec.map(|s| s.manufacturer))
            .unwrap_or("")
            .to_string();

        samples.push(ArtworkSample {
            platform_id: stem,
            name,
            category,
            manufacturer,
            installed_platform_id: matched_node.map(|p| p.id.clone()),
            replaces_existing: matched_node.is_some_and(|p| p.has_image),
            data_url,
        });
    }
    Ok(samples)
}

/// Fetches alternative banner artwork across community packs (defaults to all packs when `include_all` is true/omitted).
#[tauri::command(async)]
pub fn artwork_alternatives(
    app: tauri::AppHandle,
    platform_id: String,
    pack_ids: Option<Vec<String>>,
    include_all: Option<bool>,
) -> Result<Vec<ArtworkAlternative>, TauError> {
    let cache = cache_dir(&app)?;
    let index = image_packs::fetch_index(&cache, false)?;
    let all = include_all.unwrap_or(true);

    let targets: Vec<&ImagePackEntry> = if let Some(ids) = pack_ids {
        ids.iter()
            .filter_map(|id| index.iter().find(|p| &p.id == id))
            .collect()
    } else if all {
        index.iter().collect()
    } else {
        let enabled = read_prefs(&app)?.enabled_image_packs;
        enabled
            .iter()
            .filter_map(|id| index.iter().find(|p| &p.id == id))
            .collect()
    };

    // Download uncached zips in parallel (up to 4 at once) so first-time lookups across all packs are fast.
    let uncached: Vec<&ImagePackEntry> = targets.iter().copied().filter(|e| !e.cached).collect();
    for chunk in uncached.chunks(4) {
        std::thread::scope(|s| {
            for entry in chunk {
                let cache_path = cache.as_path();
                s.spawn(move || {
                    let _ = image_packs::ensure_pack_zip(cache_path, entry, false);
                });
            }
        });
    }

    let cands = artwork::candidate_platform_stems(&platform_id);
    let mut out = Vec::new();
    for entry in targets {
        let zip = image_packs::pack_zip_path(&cache, &entry.id);
        if !zip.is_file() {
            continue;
        }
        let zip_mtime = std::fs::metadata(&zip).and_then(|m| m.modified()).ok();
        let d_dir = decoded_pack_dir(&cache, &entry.id);

        if let Some((matched, data_url)) = cands.iter().find_map(|c| {
            read_cached_banner_data_url(&d_dir, zip_mtime, c).map(|url| (c.clone(), url))
        }) {
            out.push(ArtworkAlternative {
                pack_id: entry.id.clone(),
                pack_name: entry.name.clone(),
                owner: entry.owner.clone(),
                variant: entry.variant.clone(),
                matched_platform: matched,
                data_url,
            });
            continue;
        }

        if let Ok(Some((matched, bin))) = artwork::read_pack_platform_bin(&zip, &platform_id)
            && let Ok(png) = icon::decode_platform_image(&bin)
        {
            write_cached_banner_png(&d_dir, &matched, &png);
            out.push(ArtworkAlternative {
                pack_id: entry.id.clone(),
                pack_name: entry.name.clone(),
                owner: entry.owner.clone(),
                variant: entry.variant.clone(),
                matched_platform: matched,
                data_url: png_data_url(&png),
            });
        }
    }
    Ok(out)
}

/// Previews a local PNG or JPEG as either an Analogue Pocket 521x165 monochrome banner/icon or colour PNG.
#[tauri::command(async)]
pub fn artwork_preview_local(
    source_path: String,
    slot: ArtworkSlot,
    fit: Fit,
    pocket_monochrome: bool,
) -> Result<String, TauError> {
    artwork::preview_local_image(Path::new(&source_path), slot, fit, pocket_monochrome)
}

/// Plans writing one alternative banner or icon to an Analogue Pocket card. Writes nothing.
#[tauri::command(async)]
pub fn artwork_plan_apply(
    app: tauri::AppHandle,
    card: String,
    slot: ArtworkSlot,
    target_id: String,
    pack_id: Option<String>,
    pack_platform_id: Option<String>,
    source_path: Option<String>,
    fit: Option<Fit>,
) -> Result<ArtworkApplyPlan, TauError> {
    let lookup_platform = pack_platform_id.as_deref().unwrap_or(&target_id);
    let source = resolve_source(&app, pack_id, lookup_platform, source_path, fit)?;
    artwork::plan_apply(Path::new(&card), slot, &target_id, &source)
}

/// Executes a reviewed single-artwork plan on an Analogue Pocket card with backup & verification.
#[tauri::command(async)]
pub fn artwork_execute_apply(
    app: tauri::AppHandle,
    card: String,
    slot: ArtworkSlot,
    target_id: String,
    pack_id: Option<String>,
    pack_platform_id: Option<String>,
    source_path: Option<String>,
    fit: Option<Fit>,
    confirmation: String,
) -> Result<ArtworkApplyReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let lookup_platform = pack_platform_id.as_deref().unwrap_or(&target_id);
    let source = resolve_source(&app, pack_id, lookup_platform, source_path, fit)?;
    artwork::execute_apply(
        Path::new(&card),
        slot,
        &target_id,
        &source,
        &confirmation,
        &artwork_backup_root(&app)?,
    )
}

/// Plans applying matching banners in an image pack to installed platforms on a Pocket card.
#[tauri::command(async)]
pub fn artwork_plan_pack_apply(
    app: tauri::AppHandle,
    card: String,
    pack_id: String,
    only_platforms: Option<Vec<String>>,
) -> Result<PackApplyPlan, TauError> {
    let cache = cache_dir(&app)?;
    let entry = find_pack(&cache, &pack_id)?;
    let zip = image_packs::ensure_pack_zip(&cache, &entry, false)?;
    artwork::plan_pack_apply(
        Path::new(&card),
        &pack_id,
        &zip,
        only_platforms.as_deref(),
    )
}

/// Executes a reviewed batch image pack apply to a Pocket card with backup & verification.
#[tauri::command(async)]
pub fn artwork_execute_pack_apply(
    app: tauri::AppHandle,
    card: String,
    pack_id: String,
    confirmation: String,
    only_platforms: Option<Vec<String>>,
) -> Result<PackApplyReport, TauError> {
    let _write = begin_card_write(&app, &card)?;
    let cache = cache_dir(&app)?;
    let entry = find_pack(&cache, &pack_id)?;
    let zip = image_packs::ensure_pack_zip(&cache, &entry, false)?;
    artwork::execute_pack_apply(
        Path::new(&card),
        &pack_id,
        &zip,
        only_platforms.as_deref(),
        &confirmation,
        &artwork_backup_root(&app)?,
    )
}

/// Reads a saved app-local artwork override (e.g. for a Game Bub core) if present.
#[tauri::command(async)]
pub fn artwork_read_override(
    app: tauri::AppHandle,
    override_key: String,
) -> Result<Option<String>, TauError> {
    artwork::read_override_data_url(&overrides_dir(&app)?, &override_key)
}

/// Saves an app-local artwork override from a community image pack or local file.
#[tauri::command(async)]
pub fn artwork_set_override(
    app: tauri::AppHandle,
    override_key: String,
    pack_id: Option<String>,
    platform_id: Option<String>,
    source_path: Option<String>,
) -> Result<String, TauError> {
    let pid = platform_id.unwrap_or_default();
    let source = resolve_source(&app, pack_id, &pid, source_path, None)?;
    artwork::write_override_from_source(&overrides_dir(&app)?, &override_key, &source)
}

/// Deletes an app-local artwork override so the UI returns to its generated default banner.
#[tauri::command(async)]
pub fn artwork_clear_override(app: tauri::AppHandle, override_key: String) -> Result<bool, TauError> {
    artwork::clear_override(&overrides_dir(&app)?, &override_key)
}
