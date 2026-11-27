use crate::data_dir;
use std::path::{Path, PathBuf};
use tau_core::{
    ErrorCode, TauError,
    game_art::{
        self, GameArtCandidate, GameArtKind, GameArtResult, GameArtSourceKind, GameDetailsView,
        GameMeta,
    },
};
use tau_net::libretro_thumbs;
use tauri::Manager;

const MAX_BATCH_ROMS: usize = 48;
const MAX_META_ROMS: usize = 2000;

fn cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(|e| TauError {
            code: ErrorCode::Io,
            message: e.to_string(),
        })?
        .join("game-art-v2"))
}

fn overrides_dir(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    Ok(data_dir(app)?.join("game-art-overrides"))
}

/// Resolves automatic artwork (override -> local cache -> Libretro Thumbnails) for a batch of ROMs.
/// Never reads or writes SD cards.
#[tauri::command(async)]
pub fn game_art_resolve_batch(
    app: tauri::AppHandle,
    platform_id: String,
    rom_names: Vec<String>,
    kind: Option<GameArtKind>,
    force: Option<bool>,
) -> Result<Vec<GameArtResult>, TauError> {
    let cache = cache_dir(&app)?;
    let overrides = overrides_dir(&app)?;
    let trimmed: Vec<String> = rom_names
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .take(MAX_BATCH_ROMS)
        .collect();
    if trimmed.is_empty() || platform_id.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(libretro_thumbs::resolve_batch(
        &cache,
        &overrides,
        platform_id.trim(),
        &trimmed,
        kind.unwrap_or_default(),
        force.unwrap_or(false),
    ))
}

/// Searches Libretro Thumbnails for interactive picker candidates matching a query.
#[tauri::command(async)]
pub fn game_art_search(
    app: tauri::AppHandle,
    platform_id: String,
    query: String,
    kind: Option<GameArtKind>,
    limit: Option<usize>,
) -> Result<Vec<GameArtCandidate>, TauError> {
    let cache = cache_dir(&app)?;
    libretro_thumbs::search_candidates(
        &cache,
        platform_id.trim(),
        query.trim(),
        kind.unwrap_or_default(),
        limit.unwrap_or(18),
    )
}

/// Reads and validates a user-picked local image file for preview inside the game artwork picker.
#[tauri::command(async)]
pub fn game_art_preview_local(source_path: String) -> Result<String, TauError> {
    game_art::read_local_preview_data_url(Path::new(&source_path))
}

/// Saves an app-local per-game artwork override from a Libretro pick or a local image file.
#[tauri::command(async)]
pub fn game_art_set_override(
    app: tauri::AppHandle,
    platform_id: String,
    rom_name: String,
    libretro_system: Option<String>,
    libretro_kind: Option<GameArtKind>,
    libretro_name: Option<String>,
    source_path: Option<String>,
) -> Result<GameArtResult, TauError> {
    let cache = cache_dir(&app)?;
    let overrides = overrides_dir(&app)?;
    let plat = platform_id.trim();
    let rom = rom_name.trim();
    if rom.is_empty() {
        return Err(TauError {
            code: ErrorCode::InvalidPathReference,
            message: "Choose a game first.".into(),
        });
    }

    let result = if let Some(path) = source_path.filter(|s| !s.trim().is_empty()) {
        game_art::write_override_from_file(&overrides, plat, rom, Path::new(path.trim()))?
    } else if let (Some(sys), Some(name)) = (
        libretro_system.filter(|s| !s.trim().is_empty()),
        libretro_name.filter(|s| !s.trim().is_empty()),
    ) {
        let kind = libretro_kind.unwrap_or_default();
        let bytes = libretro_thumbs::fetch_thumbnail_bytes(&cache, sys.trim(), kind, name.trim())?
            .ok_or_else(|| TauError {
                code: ErrorCode::NotFound,
                message: format!("Could not download “{}” from Libretro Thumbnails.", name.trim()),
            })?;
        game_art::write_override_bytes(
            &overrides,
            plat,
            rom,
            &bytes,
            kind,
            GameArtSourceKind::OverrideLibretro,
            Some(name.trim()),
            Some(sys.trim()),
        )?
    } else {
        return Err(TauError {
            code: ErrorCode::InvalidPathReference,
            message: "Choose a Libretro thumbnail or a local image file first.".into(),
        });
    };

    libretro_thumbs::clear_miss_marker(&cache, plat, rom);
    Ok(result)
}

/// Clears a saved per-game artwork override (and any negative miss marker so auto lookup can run).
#[tauri::command(async)]
pub fn game_art_clear_override(
    app: tauri::AppHandle,
    platform_id: String,
    rom_name: String,
) -> Result<bool, TauError> {
    let cache = cache_dir(&app)?;
    let overrides = overrides_dir(&app)?;
    libretro_thumbs::clear_miss_marker(&cache, platform_id.trim(), rom_name.trim());
    game_art::clear_override(&overrides, platform_id.trim(), rom_name.trim())
}

/// Resolves game genres and display regions from cached `libretro-database` metadata.
#[tauri::command(async)]
pub fn game_meta_resolve_batch(
    app: tauri::AppHandle,
    platform_id: String,
    rom_names: Vec<String>,
) -> Result<Vec<GameMeta>, TauError> {
    let cache = cache_dir(&app)?;
    let trimmed: Vec<String> = rom_names
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .take(MAX_META_ROMS)
        .collect();
    if trimmed.is_empty() || platform_id.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(libretro_thumbs::resolve_genres_batch(
        &cache,
        platform_id.trim(),
        &trimmed,
    ))
}

/// Resolves detailed game metadata and available images (Boxart, Title screen, Screenshot) for one game.
#[tauri::command(async)]
pub fn game_details_read(
    app: tauri::AppHandle,
    platform_id: String,
    rom_name: String,
) -> Result<GameDetailsView, TauError> {
    let cache = cache_dir(&app)?;
    let overrides = overrides_dir(&app)?;
    libretro_thumbs::resolve_game_details(&cache, &overrides, platform_id.trim(), rom_name.trim())
}
