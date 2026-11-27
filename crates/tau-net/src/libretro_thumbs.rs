//! Libretro Thumbnails fetcher and local disk cache via official `libretro-thumbnails`
//! GitHub repositories (`raw.githubusercontent.com` + `api.github.com`).
//!
//! Only `https://raw.githubusercontent.com/libretro-thumbnails` and
//! `https://api.github.com/repos/libretro-thumbnails` are ever requested.
//! Directory indexes, decoded PNG images, and genuine negative-lookup markers are cached
//! locally so repeat visits are instant and offline-safe. Transient / rate-limit errors
//! never write a negative miss marker or overwrite a valid index cache.

use crate::{ErrorCode, TauError};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime},
};
use tau_core::game_art::{
    self, GameArtCandidate, GameArtKind, GameArtResult, GameArtSourceKind, GameDetailsView,
    GameMeta, LIBRETRO_API_BASE_URL, LIBRETRO_DB_BASE_URL, LIBRETRO_RAW_BASE_URL,
};

const INDEX_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const MISS_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const INDEX_LIMIT: u64 = 24 * 1024 * 1024;
const IMAGE_LIMIT: u64 = 4 * 1024 * 1024;
const MAX_SEARCH_LIMIT: usize = 30;

fn err(code: ErrorCode, message: impl Into<String>) -> TauError {
    TauError {
        code,
        message: message.into(),
    }
}

fn ensure_allowed_url(url: &str) -> Result<(), TauError> {
    let raw_ok =
        url == LIBRETRO_RAW_BASE_URL || url.starts_with(&format!("{LIBRETRO_RAW_BASE_URL}/"));
    let api_ok =
        url == LIBRETRO_API_BASE_URL || url.starts_with(&format!("{LIBRETRO_API_BASE_URL}/"));
    let db_ok = url == LIBRETRO_DB_BASE_URL || url.starts_with(&format!("{LIBRETRO_DB_BASE_URL}/"));
    if !raw_ok && !api_ok && !db_ok {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "Game artwork lookup only allows official Libretro GitHub hosts.",
        ));
    }
    Ok(())
}

fn is_fresh_nonempty(path: &Path, ttl: Duration) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() == 0 {
        return false;
    }
    let Ok(modified) = meta.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|age| age < ttl)
        .unwrap_or(false)
}

fn is_fresh(path: &Path, ttl: Duration) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    let Ok(modified) = meta.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|age| age < ttl)
        .unwrap_or(false)
}

fn safe_segment(raw: &str) -> String {
    let clean: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() { "_".into() } else { clean }
}

fn index_cache_path(cache_dir: &Path, system: &str, kind: GameArtKind) -> PathBuf {
    cache_dir.join("indexes").join(format!(
        "{}__{}.txt",
        safe_segment(system),
        kind.libretro_dir()
    ))
}

fn image_cache_path(
    cache_dir: &Path,
    system: &str,
    kind: GameArtKind,
    entry_name: &str,
) -> PathBuf {
    let sanitized = game_art::libretro_sanitize(entry_name);
    let hash = game_art::game_key_hash(&format!("{system}|{}|{sanitized}", kind.libretro_dir()));
    cache_dir
        .join("images")
        .join(safe_segment(system))
        .join(kind.libretro_dir())
        .join(format!("{hash}.png"))
}

fn miss_cache_path(cache_dir: &Path, platform_id: &str, rom_name: &str) -> PathBuf {
    let canon = game_art::canonical_platform_id(platform_id);
    let key = game_art::canonical_game_key(&canon, rom_name);
    let hash = game_art::game_key_hash(&key);
    cache_dir
        .join("misses")
        .join(format!("{}__{hash}.miss", safe_segment(&canon)))
}

fn write_atomic(dest: &Path, bytes: &[u8]) -> Result<(), TauError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let part_name = format!(
        "{}.part.{}",
        dest.file_name().and_then(|s| s.to_str()).unwrap_or("cache"),
        std::process::id()
    );
    let part = dest.with_file_name(part_name);
    fs::write(&part, bytes)?;
    fs::rename(&part, dest)?;
    Ok(())
}

fn build_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(8)))
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent(concat!("Tau-Omega/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn http_get_optional(
    agent: &ureq::Agent,
    url: &str,
    limit: u64,
) -> Result<Option<Vec<u8>>, TauError> {
    ensure_allowed_url(url)?;
    let mut req = agent.get(url);
    if url.starts_with(LIBRETRO_API_BASE_URL) {
        req = req.header("Accept", "application/vnd.github+json");
    }
    match req.call() {
        Ok(mut resp) => {
            let bytes = resp
                .body_mut()
                .with_config()
                .limit(limit)
                .read_to_vec()
                .map_err(|e| {
                    err(
                        ErrorCode::Io,
                        format!("Failed reading artwork response: {e}"),
                    )
                })?;
            Ok(Some(bytes))
        }
        Err(ureq::Error::StatusCode(404 | 410)) => Ok(None),
        Err(ureq::Error::StatusCode(403 | 429)) => Err(err(
            ErrorCode::Io,
            "GitHub is temporarily rate limiting Libretro index lookups. Exact-named ROMs still load directly; try again in a few minutes.",
        )),
        Err(ureq::Error::StatusCode(code)) => Err(err(
            ErrorCode::Io,
            format!("libretro-thumbnails returned HTTP {code}."),
        )),
        Err(e) => Err(err(
            ErrorCode::Io,
            format!("Could not reach GitHub libretro-thumbnails: {e}"),
        )),
    }
}

/// Fetches (or reads from disk cache) the sorted list of artwork entry names for one system/kind.
/// Uses the GitHub recursive tree endpoint so one API call populates Boxarts, Titles, and Snaps
/// on disk for 30 days.
pub fn fetch_system_index(
    cache_dir: &Path,
    system: &str,
    kind: GameArtKind,
    force: bool,
) -> Result<Vec<String>, TauError> {
    let path = index_cache_path(cache_dir, system, kind);
    if !force && is_fresh_nonempty(&path, INDEX_TTL) {
        let text = fs::read_to_string(&path)?;
        return Ok(text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(ToString::to_string)
            .collect());
    }

    let agent = build_agent();
    let tree_url = game_art::libretro_tree_url(system);
    let Some(bytes) = http_get_optional(&agent, &tree_url, INDEX_LIMIT)? else {
        return Ok(Vec::new());
    };

    let (truncated, mut map) = game_art::parse_github_tree_json(&bytes)?;
    if !truncated {
        let any_entries = map.values().any(|v| !v.is_empty());
        if any_entries {
            for (k, list) in &map {
                let dest = index_cache_path(cache_dir, system, *k);
                let _ = write_atomic(&dest, list.join("\n").as_bytes());
            }
        }
        return Ok(map.remove(&kind).unwrap_or_default());
    }

    // Repo tree exceeded GitHub's recursive cap: fetch just the requested directory tree.
    let dir_url = game_art::libretro_dir_tree_url(system, kind);
    let Some(dir_bytes) = http_get_optional(&agent, &dir_url, INDEX_LIMIT)? else {
        return Ok(Vec::new());
    };
    let entries = game_art::parse_github_dir_tree_json(&dir_bytes)?;
    if !entries.is_empty() {
        let _ = write_atomic(&path, entries.join("\n").as_bytes());
    }
    Ok(entries)
}

/// Fetches (or reads from disk cache) raw image bytes for a specific Libretro thumbnail entry
/// from `raw.githubusercontent.com/libretro-thumbnails`.
pub fn fetch_thumbnail_bytes(
    cache_dir: &Path,
    system: &str,
    kind: GameArtKind,
    entry_name: &str,
) -> Result<Option<Vec<u8>>, TauError> {
    let path = image_cache_path(cache_dir, system, kind, entry_name);
    if let Ok(meta) = fs::metadata(&path)
        && meta.is_file()
        && meta.len() > 0
        && meta.len() <= IMAGE_LIMIT
    {
        let bytes = fs::read(&path)?;
        if game_art::sniff_image_mime(&bytes).is_some() {
            return Ok(Some(bytes));
        }
    }

    let agent = build_agent();
    let url = game_art::libretro_image_url(system, kind, entry_name);
    match http_get_optional(&agent, &url, IMAGE_LIMIT)? {
        Some(bytes) => {
            if game_art::sniff_image_mime(&bytes).is_none() {
                return Ok(None);
            }
            write_atomic(&path, &bytes)?;
            Ok(Some(bytes))
        }
        None => Ok(None),
    }
}

/// Removes any cached negative-lookup marker for a game (used when overrides are applied/cleared).
pub fn clear_miss_marker(cache_dir: &Path, platform_id: &str, rom_name: &str) {
    let path = miss_cache_path(cache_dir, platform_id, rom_name);
    let _ = fs::remove_file(path);
}

fn write_miss_marker(cache_dir: &Path, platform_id: &str, rom_name: &str) {
    let path = miss_cache_path(cache_dir, platform_id, rom_name);
    let _ = write_atomic(&path, b"miss");
}

fn try_direct_stem_for_system(
    cache_dir: &Path,
    system: &str,
    rom_name: &str,
    kind: GameArtKind,
) -> Result<Option<(String, Vec<u8>)>, TauError> {
    let stem = game_art::display_title(rom_name);
    if stem.is_empty() {
        return Ok(None);
    }
    if let Some(bytes) = fetch_thumbnail_bytes(cache_dir, system, kind, &stem)? {
        return Ok(Some((stem, bytes)));
    }
    Ok(None)
}

fn try_index_match_for_system(
    cache_dir: &Path,
    system: &str,
    rom_name: &str,
    kind: GameArtKind,
) -> Result<Option<(String, Vec<u8>)>, TauError> {
    let index = fetch_system_index(cache_dir, system, kind, false)?;
    if let Some(matched) = game_art::match_in_index(rom_name, &index)
        && let Some(bytes) = fetch_thumbnail_bytes(cache_dir, system, kind, &matched)?
    {
        return Ok(Some((matched, bytes)));
    }
    Ok(None)
}

fn make_auto_result(
    canon_plat: &str,
    rom_name: &str,
    system: &str,
    kind: GameArtKind,
    matched_name: String,
    bytes: &[u8],
) -> Option<GameArtResult> {
    let data_url = game_art::image_data_url(bytes).ok()?;
    Some(GameArtResult {
        rom_name: rom_name.to_string(),
        platform_id: canon_plat.to_string(),
        game_key: game_art::canonical_game_key(canon_plat, rom_name),
        data_url,
        kind,
        source: GameArtSourceKind::LibretroAuto,
        matched_name: Some(matched_name),
        libretro_system: Some(system.to_string()),
        has_override: false,
    })
}

/// Resolves automatic artwork (override -> miss cache -> direct raw PNG -> cached/fetched index)
/// for one ROM.
pub fn resolve_one(
    cache_dir: &Path,
    overrides_dir: &Path,
    platform_id: &str,
    rom_name: &str,
    preferred_kind: GameArtKind,
    force: bool,
) -> Result<Option<GameArtResult>, TauError> {
    if let Some(ovr) = game_art::read_override(overrides_dir, platform_id, rom_name)? {
        return Ok(Some(ovr));
    }

    let canon_plat = game_art::canonical_platform_id(platform_id);
    if !force && is_fresh(&miss_cache_path(cache_dir, &canon_plat, rom_name), MISS_TTL) {
        return Ok(None);
    }

    let systems = game_art::libretro_systems_for_platform(&canon_plat);
    if systems.is_empty() {
        return Ok(None);
    }

    let mut kinds_to_try = vec![preferred_kind];
    if preferred_kind == GameArtKind::Boxart {
        kinds_to_try.push(GameArtKind::Title);
    }

    let mut had_network_error = false;
    // Pass 1: zero-API-call direct exact-stem fetch on raw.githubusercontent.com.
    for &kind in &kinds_to_try {
        for &system in &systems {
            match try_direct_stem_for_system(cache_dir, system, rom_name, kind) {
                Ok(Some((matched_name, bytes))) => {
                    if let Some(res) =
                        make_auto_result(&canon_plat, rom_name, system, kind, matched_name, &bytes)
                    {
                        clear_miss_marker(cache_dir, &canon_plat, rom_name);
                        return Ok(Some(res));
                    }
                }
                Ok(None) => {}
                Err(_) => had_network_error = true,
            }
        }
    }

    // Pass 2: fuzzy/normalized matching against cached GitHub directory tree.
    for &kind in &kinds_to_try {
        for &system in &systems {
            match try_index_match_for_system(cache_dir, system, rom_name, kind) {
                Ok(Some((matched_name, bytes))) => {
                    if let Some(res) =
                        make_auto_result(&canon_plat, rom_name, system, kind, matched_name, &bytes)
                    {
                        clear_miss_marker(cache_dir, &canon_plat, rom_name);
                        return Ok(Some(res));
                    }
                }
                Ok(None) => {}
                Err(_) => had_network_error = true,
            }
        }
    }

    if !had_network_error {
        write_miss_marker(cache_dir, &canon_plat, rom_name);
    }
    Ok(None)
}

/// Resolves artwork for a batch of ROMs on one platform.
/// First attempts direct exact-stem PNG fetches on `raw.githubusercontent.com` across up to 6
/// threads (zero GitHub API quota cost), then warms the primary system tree once and resolves any
/// remaining non-exact ROMs via normalized title/region matching.
pub fn resolve_batch(
    cache_dir: &Path,
    overrides_dir: &Path,
    platform_id: &str,
    rom_names: &[String],
    preferred_kind: GameArtKind,
    force: bool,
) -> Vec<GameArtResult> {
    if rom_names.is_empty() {
        return Vec::new();
    }

    let canon_plat = game_art::canonical_platform_id(platform_id);
    let systems = game_art::libretro_systems_for_platform(&canon_plat);

    let mut immediate: HashMap<usize, GameArtResult> = HashMap::new();
    let mut pending: Vec<(usize, String)> = Vec::new();

    for (idx, rom_name) in rom_names.iter().enumerate() {
        if let Ok(Some(ovr)) = game_art::read_override(overrides_dir, &canon_plat, rom_name) {
            immediate.insert(idx, ovr);
            continue;
        }
        if !force && is_fresh(&miss_cache_path(cache_dir, &canon_plat, rom_name), MISS_TTL) {
            continue;
        }
        if !systems.is_empty() {
            pending.push((idx, rom_name.clone()));
        }
    }

    let results_mutex: Mutex<HashMap<usize, GameArtResult>> = Mutex::new(immediate);

    // Phase B: parallel direct raw.githubusercontent.com lookup for preferred_kind (zero API calls).
    let direct_queue: Mutex<VecDeque<(usize, String)>> = Mutex::new(pending.clone().into());
    let unresolved_mutex: Mutex<Vec<(usize, String)>> = Mutex::new(Vec::new());
    if !pending.is_empty() {
        let workers = 6usize.min(pending.len());
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        let next = {
                            let Ok(mut q) = direct_queue.lock() else {
                                return;
                            };
                            q.pop_front()
                        };
                        let Some((idx, rom_name)) = next else {
                            return;
                        };
                        let mut hit: Option<GameArtResult> = None;
                        for &system in &systems {
                            if let Ok(Some((matched, bytes))) = try_direct_stem_for_system(
                                cache_dir,
                                system,
                                &rom_name,
                                preferred_kind,
                            ) && let Some(res) = make_auto_result(
                                &canon_plat,
                                &rom_name,
                                system,
                                preferred_kind,
                                matched,
                                &bytes,
                            ) {
                                clear_miss_marker(cache_dir, &canon_plat, &rom_name);
                                hit = Some(res);
                                break;
                            }
                        }
                        if let Some(res) = hit {
                            if let Ok(mut map) = results_mutex.lock() {
                                map.insert(idx, res);
                            }
                        } else if let Ok(mut missed) = unresolved_mutex.lock() {
                            missed.push((idx, rom_name));
                        }
                    }
                });
            }
        });
    }

    // Phase C: warm the primary system tree ONCE, then fuzzy-match remaining ROMs.
    let remaining = unresolved_mutex.into_inner().unwrap_or_default();
    if !remaining.is_empty() {
        if let Some(first_sys) = systems.first() {
            let _ = fetch_system_index(cache_dir, first_sys, preferred_kind, false);
        }
        let fuzzy_queue: Mutex<VecDeque<(usize, String)>> = Mutex::new(remaining.clone().into());
        let workers = 4usize.min(remaining.len());
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        let next = {
                            let Ok(mut q) = fuzzy_queue.lock() else {
                                return;
                            };
                            q.pop_front()
                        };
                        let Some((idx, rom_name)) = next else {
                            return;
                        };
                        if let Ok(Some(res)) = resolve_one(
                            cache_dir,
                            overrides_dir,
                            &canon_plat,
                            &rom_name,
                            preferred_kind,
                            force,
                        ) && let Ok(mut map) = results_mutex.lock()
                        {
                            map.insert(idx, res);
                        }
                    }
                });
            }
        });
    }

    let map = results_mutex.into_inner().unwrap_or_default();
    let mut ordered = Vec::with_capacity(map.len());
    for idx in 0..rom_names.len() {
        if let Some(item) = map.get(&idx) {
            ordered.push(item.clone());
        }
    }
    ordered
}

/// Searches Libretro Thumbnails for interactive candidates matching `query` on `platform_id`.
pub fn search_candidates(
    cache_dir: &Path,
    platform_id: &str,
    query: &str,
    kind: GameArtKind,
    limit: usize,
) -> Result<Vec<GameArtCandidate>, TauError> {
    let systems = game_art::libretro_systems_for_platform(platform_id);
    if systems.is_empty() || query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let cap = limit.clamp(1, MAX_SEARCH_LIMIT);

    let mut targets: Vec<(String, String)> = Vec::new();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut last_err: Option<TauError> = None;

    for system in systems {
        match fetch_system_index(cache_dir, system, kind, false) {
            Ok(entries) => {
                for hit in game_art::search_index(query, &entries, cap) {
                    let pair = (system.to_string(), hit);
                    if seen.insert(pair.clone()) {
                        targets.push(pair);
                        if targets.len() >= cap {
                            break;
                        }
                    }
                }
            }
            Err(e) => last_err = Some(e),
        }
        if targets.len() >= cap {
            break;
        }
    }

    if targets.is_empty()
        && let Some(e) = last_err
    {
        return Err(e);
    }

    let indexed_targets: Vec<(usize, String, String)> = targets
        .into_iter()
        .enumerate()
        .map(|(i, (s, n))| (i, s, n))
        .collect();
    let total = indexed_targets.len();
    let queue_mutex: Mutex<VecDeque<(usize, String, String)>> = Mutex::new(indexed_targets.into());
    let results_mutex: Mutex<HashMap<usize, GameArtCandidate>> = Mutex::new(HashMap::new());

    std::thread::scope(|scope| {
        for _ in 0..4usize.min(total.max(1)) {
            scope.spawn(|| {
                loop {
                    let next = {
                        let Ok(mut q) = queue_mutex.lock() else {
                            return;
                        };
                        q.pop_front()
                    };
                    let Some((idx, system, name)) = next else {
                        return;
                    };
                    if let Ok(Some(bytes)) = fetch_thumbnail_bytes(cache_dir, &system, kind, &name)
                        && let Ok(data_url) = game_art::image_data_url(&bytes)
                        && let Ok(mut map) = results_mutex.lock()
                    {
                        map.insert(
                            idx,
                            GameArtCandidate {
                                system,
                                kind,
                                name,
                                data_url,
                            },
                        );
                    }
                }
            });
        }
    });

    let map = results_mutex.into_inner().unwrap_or_default();
    let mut out = Vec::with_capacity(map.len());
    for idx in 0..total {
        if let Some(c) = map.get(&idx) {
            out.push(c.clone());
        }
    }
    Ok(out)
}

fn metadata_cache_path(cache_dir: &Path, system: &str, field: &str) -> PathBuf {
    cache_dir
        .join("metadat")
        .join(format!("{}__{field}.tsv", safe_segment(system)))
}

fn metadata_none_path(cache_dir: &Path, system: &str, field: &str) -> PathBuf {
    cache_dir
        .join("metadat")
        .join(format!("{}__{field}.none", safe_segment(system)))
}

/// Fetches (or reads from 30-day TSV cache) one `libretro-database` metadata table (`genre`, `developer`, `publisher`, `releaseyear`).
pub fn fetch_metadata_map(
    cache_dir: &Path,
    system: &str,
    field: &str,
) -> Result<BTreeMap<String, String>, TauError> {
    let tsv_path = metadata_cache_path(cache_dir, system, field);
    if is_fresh_nonempty(&tsv_path, INDEX_TTL) {
        let text = fs::read_to_string(&tsv_path)?;
        let mut map = BTreeMap::new();
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('\t')
                && !k.is_empty()
                && !v.is_empty()
            {
                map.insert(k.to_string(), v.to_string());
            }
        }
        return Ok(map);
    }

    let none_path = metadata_none_path(cache_dir, system, field);
    if is_fresh(&none_path, INDEX_TTL) {
        return Ok(BTreeMap::new());
    }

    let agent = build_agent();
    let url = game_art::libretro_db_dat_url(field, system);
    let Some(bytes) = http_get_optional(&agent, &url, INDEX_LIMIT)? else {
        let _ = write_atomic(&none_path, b"none");
        return Ok(BTreeMap::new());
    };

    let text = String::from_utf8_lossy(&bytes);
    let map = game_art::parse_libretro_dat(&text, field);
    if map.is_empty() {
        let _ = write_atomic(&none_path, b"empty");
        return Ok(map);
    }

    let mut tsv = String::new();
    for (k, v) in &map {
        let clean_v = v.replace(['\t', '\r', '\n'], " ");
        tsv.push_str(k);
        tsv.push('\t');
        tsv.push_str(&clean_v);
        tsv.push('\n');
    }
    let _ = fs::remove_file(&none_path);
    let _ = write_atomic(&tsv_path, tsv.as_bytes());
    Ok(map)
}

/// Resolves genre & region metadata for a list of ROMs on a platform (1 cached DAT per system).
pub fn resolve_genres_batch(
    cache_dir: &Path,
    platform_id: &str,
    rom_names: &[String],
) -> Vec<GameMeta> {
    if rom_names.is_empty() {
        return Vec::new();
    }
    let canon_plat = game_art::canonical_platform_id(platform_id);
    let systems = game_art::libretro_systems_for_platform(&canon_plat);
    let mut genre_maps = Vec::new();
    for system in &systems {
        if let Ok(m) = fetch_metadata_map(cache_dir, system, "genre")
            && !m.is_empty()
        {
            genre_maps.push(m);
        }
    }

    rom_names
        .iter()
        .map(|rom_name| {
            let norm = game_art::normalize_title(rom_name);
            let genre = genre_maps.iter().find_map(|m| m.get(&norm).cloned());
            GameMeta {
                rom_name: rom_name.clone(),
                platform_id: canon_plat.clone(),
                genre,
                developer: None,
                publisher: None,
                release_year: None,
                region: game_art::extract_display_region(rom_name),
                matched_name: None,
            }
        })
        .collect()
}

/// Resolves full metadata (genre, developer, publisher, release year, region) and all available
/// Libretro image types (Boxart, Title screen, Screenshot) plus any custom override for one ROM.
pub fn resolve_game_details(
    cache_dir: &Path,
    overrides_dir: &Path,
    platform_id: &str,
    rom_name: &str,
) -> Result<GameDetailsView, TauError> {
    let canon_plat = game_art::canonical_platform_id(platform_id);
    let systems = game_art::libretro_systems_for_platform(&canon_plat);
    let norm = game_art::normalize_title(rom_name);

    let ovr = game_art::read_override(overrides_dir, &canon_plat, rom_name)?;

    // Fetch metadata maps & the 3 thumbnail types concurrently.
    let (genre_res, dev_res, pub_res, year_res) = std::thread::scope(|scope| {
        let g = scope.spawn(|| {
            for s in &systems {
                if let Ok(m) = fetch_metadata_map(cache_dir, s, "genre")
                    && let Some(v) = m.get(&norm)
                {
                    return Some(v.clone());
                }
            }
            None
        });
        let d = scope.spawn(|| {
            for s in &systems {
                if let Ok(m) = fetch_metadata_map(cache_dir, s, "developer")
                    && let Some(v) = m.get(&norm)
                {
                    return Some(v.clone());
                }
            }
            None
        });
        let p = scope.spawn(|| {
            for s in &systems {
                if let Ok(m) = fetch_metadata_map(cache_dir, s, "publisher")
                    && let Some(v) = m.get(&norm)
                {
                    return Some(v.clone());
                }
            }
            None
        });
        let y = scope.spawn(|| {
            for s in &systems {
                if let Ok(m) = fetch_metadata_map(cache_dir, s, "releaseyear")
                    && let Some(v) = m.get(&norm)
                {
                    return Some(v.clone());
                }
            }
            None
        });
        (
            g.join().unwrap_or(None),
            d.join().unwrap_or(None),
            p.join().unwrap_or(None),
            y.join().unwrap_or(None),
        )
    });

    let kinds = [GameArtKind::Boxart, GameArtKind::Title, GameArtKind::Snap];
    let resolved_kinds: Vec<Option<GameArtResult>> = std::thread::scope(|scope| {
        let handles: Vec<_> = kinds
            .iter()
            .map(|&kind| {
                let systems = &systems;
                let canon_plat = &canon_plat;
                scope.spawn(move || {
                    for &sys in systems {
                        if let Ok(Some((matched, bytes))) =
                            try_direct_stem_for_system(cache_dir, sys, rom_name, kind)
                            && let Some(res) =
                                make_auto_result(canon_plat, rom_name, sys, kind, matched, &bytes)
                        {
                            return Some(res);
                        }
                    }
                    for &sys in systems {
                        if let Ok(Some((matched, bytes))) =
                            try_index_match_for_system(cache_dir, sys, rom_name, kind)
                            && let Some(res) =
                                make_auto_result(canon_plat, rom_name, sys, kind, matched, &bytes)
                        {
                            return Some(res);
                        }
                    }
                    None
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or(None))
            .collect()
    });

    let mut images: Vec<GameArtCandidate> = Vec::new();
    let mut matched_name = ovr.as_ref().and_then(|o| o.matched_name.clone());

    if let Some(o) = &ovr {
        images.push(GameArtCandidate {
            system: o
                .libretro_system
                .clone()
                .unwrap_or_else(|| "Custom image".into()),
            kind: o.kind,
            name: o
                .matched_name
                .clone()
                .unwrap_or_else(|| game_art::display_title(rom_name)),
            data_url: o.data_url.clone(),
        });
    }

    for res in resolved_kinds.into_iter().flatten() {
        if matched_name.is_none() {
            matched_name = res.matched_name.clone();
        }
        let dup = ovr.as_ref().is_some_and(|o| {
            o.kind == res.kind
                && o.matched_name.as_deref() == res.matched_name.as_deref()
                && o.libretro_system.as_deref() == res.libretro_system.as_deref()
        });
        if !dup {
            images.push(GameArtCandidate {
                system: res.libretro_system.unwrap_or_else(|| canon_plat.clone()),
                kind: res.kind,
                name: res
                    .matched_name
                    .unwrap_or_else(|| game_art::display_title(rom_name)),
                data_url: res.data_url,
            });
        }
    }

    Ok(GameDetailsView {
        rom_name: rom_name.to_string(),
        platform_id: canon_plat.clone(),
        display_title: game_art::display_title(rom_name),
        meta: GameMeta {
            rom_name: rom_name.to_string(),
            platform_id: canon_plat,
            genre: genre_res,
            developer: dev_res,
            publisher: pub_res,
            release_year: year_res,
            region: game_art::extract_display_region(rom_name),
            matched_name,
        },
        images,
        has_override: ovr.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn scratch_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "tau-libretro-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn minimal_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]
    }

    #[test]
    fn url_allowlist_accepts_github_and_rejects_external_hosts() {
        assert!(
            ensure_allowed_url(
                "https://raw.githubusercontent.com/libretro-thumbnails/Nintendo_-_Game_Boy/master/Named_Boxarts/a.png"
            )
            .is_ok()
        );
        assert!(
            ensure_allowed_url(
                "https://api.github.com/repos/libretro-thumbnails/Nintendo_-_Game_Boy/git/trees/master?recursive=1"
            )
            .is_ok()
        );
        assert!(ensure_allowed_url("https://thumbnails.libretro.com/a.png").is_err());
        assert!(ensure_allowed_url("https://evil.example.com/a.png").is_err());
    }

    #[test]
    fn override_beats_miss_cache_and_cached_libretro_resolves_offline() {
        let root = scratch_dir("cache-hit");
        let cache = root.join("cache");
        let overrides = root.join("overrides");
        let png = minimal_png();

        // Populate cached index + cached PNG for Mother 3 (Japan)
        let idx_path = index_cache_path(&cache, "Nintendo - Game Boy Advance", GameArtKind::Boxart);
        write_atomic(&idx_path, b"Advance Wars (USA)\nMother 3 (Japan)\n").unwrap();

        let img_path = image_cache_path(
            &cache,
            "Nintendo - Game Boy Advance",
            GameArtKind::Boxart,
            "Mother 3 (Japan)",
        );
        write_atomic(&img_path, &png).unwrap();

        // Write a miss marker for Mother 3 first, then set an override: override must still win!
        write_miss_marker(&cache, "gba", "Mother 3.gba");
        game_art::write_override_bytes(
            &overrides,
            "gba",
            "Mother 3.gba",
            &png,
            GameArtKind::Title,
            GameArtSourceKind::OverrideLocal,
            Some("custom.png"),
            None,
        )
        .unwrap();

        let ovr_hit = resolve_one(
            &cache,
            &overrides,
            "gba",
            "Mother 3.gba",
            GameArtKind::Boxart,
            false,
        )
        .unwrap()
        .unwrap();
        assert!(ovr_hit.has_override);
        assert_eq!(ovr_hit.source, GameArtSourceKind::OverrideLocal);

        // Clearing override & miss marker falls back to offline cached Libretro hit!
        game_art::clear_override(&overrides, "gba", "Mother 3.gba").unwrap();
        clear_miss_marker(&cache, "gba", "Mother 3.gba");

        let auto_hit = resolve_one(
            &cache,
            &overrides,
            "gba",
            "Mother 3.gba",
            GameArtKind::Boxart,
            false,
        )
        .unwrap()
        .unwrap();
        assert!(!auto_hit.has_override);
        assert_eq!(auto_hit.source, GameArtSourceKind::LibretroAuto);
        assert_eq!(auto_hit.matched_name.as_deref(), Some("Mother 3 (Japan)"));

        // Cached search_candidates also works offline when index + PNG are cached.
        let search = search_candidates(&cache, "gba", "Mother", GameArtKind::Boxart, 5).unwrap();
        assert_eq!(search.len(), 1);
        assert_eq!(search[0].name, "Mother 3 (Japan)");

        // Cached metadata TSV resolves genres & details offline too.
        let genre_tsv = metadata_cache_path(&cache, "Nintendo - Game Boy Advance", "genre");
        write_atomic(&genre_tsv, b"mother 3\tRole Playing\n").unwrap();
        let meta = resolve_genres_batch(&cache, "gba", &["Mother 3 (Japan).gba".into()]);
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].genre.as_deref(), Some("Role Playing"));
        assert_eq!(meta[0].region.as_deref(), Some("Japan"));

        let _ = fs::remove_dir_all(&root);
    }
}
