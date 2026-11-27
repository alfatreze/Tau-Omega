//! Game Bub core directory catalog, core metadata (`core.json`, `files.json`, `settings.json`), and read-only card inspector.
//!
//! Written from `https://docs.gamebub.net/developing-cores/reference-metadata/` (Framework v1.1-beta) and
//! `https://cores.gamebub.net/cores.json` (`schema_version: 1`). Pure logic / read-only filesystem inspection; WASI-safe.

use crate::{
    ErrorCode, TauError,
    catalog::{Catalog, CatalogCore, ReleaseSummary, Repository},
    core_update::{CoreStatus, Status},
    core_version::{self, Verdict},
    device::DeviceKind,
};
use serde_json::Value;
use std::{fs, path::Path};

/// Public machine-readable Game Bub Core Directory catalog (`schema_version: 1`).
pub const GAMEBUB_CATALOG_URL: &str = "https://cores.gamebub.net/cores.json";
/// Source identifier used in `CatalogCore.source`.
pub const GAMEBUB_SOURCE: &str = "gamebub-directory";
/// Default commercial hardware target.
pub const DEFAULT_TARGET: &str = "gamebub_rev4";

/// Built-in Game Bub BIOS files under `/system/` (name, display label, exact byte size).
pub const SYSTEM_BIOS: [(&str, &str, u64); 3] = [
    ("gameboy.bios-dmg.bin", "Game Boy (DMG) BIOS", 256),
    ("gameboy.bios-cgb.bin", "Game Boy Color (CGB) BIOS", 2304),
    ("gba.bios.bin", "Game Boy Advance (GBA) BIOS", 16384),
];

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubBitstream {
    pub target: String,
    pub filename: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubHardware {
    pub cartridge_enable: String,
    pub cartridge_selectable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubCoreJson {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub version: String,
    pub release_date: String,
    pub url: String,
    pub license: String,
    pub min_firmware: String,
    pub bitstreams: Vec<GameBubBitstream>,
    pub hardware: GameBubHardware,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubFileEntry {
    pub id: u16,
    pub label: String,
    pub filename: Option<String>,
    pub user_selected: bool,
    pub dependent_on_0: bool,
    pub extensions: Vec<String>,
    pub optional: bool,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubSettingEntry {
    pub id: u16,
    pub label: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubBiosStatus {
    pub filename: String,
    pub label: String,
    pub expected_bytes: u64,
    pub present: bool,
    pub actual_bytes: Option<u64>,
    pub size_ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubInstalledCore {
    pub folder: String,
    pub core: GameBubCoreJson,
    pub missing_bitstreams: Vec<String>,
    pub files: Vec<GameBubFileEntry>,
    pub missing_required_files: Vec<String>,
    pub settings: Vec<GameBubSettingEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GameBubCardView {
    pub cores: Vec<GameBubInstalledCore>,
    pub bios: Vec<GameBubBiosStatus>,
}

fn bad(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidPathReference, msg)
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Validates a Game Bub `core.json` (must have `metadata.{id, name, author}`, `bitstreams[]`, and `min_firmware`).
pub fn parse_core_json(bytes: &[u8]) -> Result<GameBubCoreJson, TauError> {
    let v: Value =
        serde_json::from_slice(bytes).map_err(|_| bad("Game Bub core.json is not valid JSON."))?;
    let meta = v
        .get("metadata")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("Game Bub core.json is missing 'metadata'."))?;
    let meta_v = Value::Object(meta.clone());
    let id = str_field(&meta_v, "id");
    let name = str_field(&meta_v, "name");
    let author = str_field(&meta_v, "author");
    if id.is_empty() || id.len() > 32 || id.contains('/') || id.contains('\\') || id.contains("..")
    {
        return Err(bad(format!("Invalid Game Bub core id: '{id}'.")));
    }
    if name.is_empty() {
        return Err(bad("Game Bub core.json is missing metadata.name."));
    }
    if author.is_empty() {
        return Err(bad("Game Bub core.json is missing metadata.author."));
    }
    let min_firmware = str_field(&v, "min_firmware");
    let bs_arr = v
        .get("bitstreams")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("Game Bub core.json is missing 'bitstreams'."))?;
    let mut bitstreams = Vec::new();
    for b in bs_arr {
        let target = str_field(b, "target");
        let filename = str_field(b, "filename");
        if target.is_empty() || filename.is_empty() || filename.contains("..") {
            return Err(bad("Game Bub core.json has an invalid bitstream entry."));
        }
        bitstreams.push(GameBubBitstream { target, filename });
    }
    if bitstreams.is_empty() {
        return Err(bad("Game Bub core.json declares no bitstreams."));
    }
    let hw = v.get("hardware");
    let hardware = GameBubHardware {
        cartridge_enable: hw
            .map(|h| str_field(h, "cartridge_enable"))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "no".into()),
        cartridge_selectable: hw
            .and_then(|h| h.get("cartridge_selectable"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    Ok(GameBubCoreJson {
        id,
        name,
        author,
        description: str_field(&meta_v, "description"),
        version: str_field(&meta_v, "version"),
        release_date: str_field(&meta_v, "release_date"),
        url: str_field(&meta_v, "url"),
        license: str_field(&meta_v, "license"),
        min_firmware,
        bitstreams,
        hardware,
    })
}

/// Parses an optional Game Bub `files.json` (max 16 entries).
pub fn parse_files_json(bytes: &[u8]) -> Result<Vec<GameBubFileEntry>, TauError> {
    let v: Value =
        serde_json::from_slice(bytes).map_err(|_| bad("Game Bub files.json is not valid JSON."))?;
    let Some(arr) = v.get("files").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for f in arr.iter().take(16) {
        let id = f.get("id").and_then(Value::as_u64).unwrap_or(0) as u16;
        let label = str_field(f, "label");
        let filename = f
            .get("filename")
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let user_selected = f
            .get("user_selected")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let dependent_on_0 = f
            .get("dependent_on_0")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let extensions = f
            .get("extensions")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.trim_start_matches('.').to_string())
                    .collect()
            })
            .unwrap_or_default();
        let optional = f.get("optional").and_then(Value::as_bool).unwrap_or(true);
        let read_only = f.get("read_only").and_then(Value::as_bool).unwrap_or(false);
        out.push(GameBubFileEntry {
            id,
            label,
            filename,
            user_selected,
            dependent_on_0,
            extensions,
            optional,
            read_only,
        });
    }
    Ok(out)
}

/// Parses an optional Game Bub `settings.json` (max 16 entries).
pub fn parse_settings_json(bytes: &[u8]) -> Result<Vec<GameBubSettingEntry>, TauError> {
    let v: Value = serde_json::from_slice(bytes)
        .map_err(|_| bad("Game Bub settings.json is not valid JSON."))?;
    let Some(arr) = v.get("settings").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    Ok(arr
        .iter()
        .take(16)
        .map(|s| GameBubSettingEntry {
            id: s.get("id").and_then(Value::as_u64).unwrap_or(0) as u16,
            label: str_field(s, "label"),
            kind: str_field(s, "type"),
        })
        .collect())
}

fn parse_repo_slug(raw: &str) -> Repository {
    let trimmed = raw
        .trim()
        .trim_start_matches("https://github.com/")
        .trim_end_matches('/');
    let mut parts = trimmed.split('/');
    match (parts.next(), parts.next()) {
        (Some(owner), Some(name)) if !owner.is_empty() && !name.is_empty() => Repository {
            owner: owner.to_string(),
            name: name.to_string(),
        },
        _ => Repository::default(),
    }
}

fn classify_system(id: &str, name: &str) -> (&'static str, &'static str, u32) {
    let h = format!("{id} {name}").to_ascii_lowercase();
    if h.contains("3do") {
        ("Console", "The 3DO Company", 1993)
    } else if h.contains("appleiigs") || h.contains("apple iigs") {
        ("Computer", "Apple", 1986)
    } else if h.contains("maclc") || h.contains("macintosh") {
        ("Computer", "Apple", 1990)
    } else if h.contains("minimig") || h.contains("amiga") {
        ("Computer", "Commodore", 1985)
    } else if h.contains("2600") {
        ("Console", "Atari", 1977)
    } else if h.contains("lynx") {
        ("Handheld", "Atari", 1989)
    } else if h.contains("mcr") || h.contains("midway") {
        ("Arcade", "Bally Midway", 1981)
    } else if h.contains("coleco") {
        ("Console", "Coleco", 1982)
    } else if h.contains("game gear") || h.ends_with(".gg") || h.contains(".gg ") {
        ("Handheld", "Sega", 1990)
    } else if h.contains("n64") || h.contains("mirlo64") || h.contains("nintendo 64") {
        ("Console", "Nintendo", 1996)
    } else if h.contains("snes") || h.contains("super nintendo") {
        ("Console", "Nintendo", 1990)
    } else if h.contains("psx") || h.contains("playstation") {
        ("Console", "Sony", 1994)
    } else if h.contains("pcengine") || h.contains("pc engine") {
        ("Console", "NEC", 1987)
    } else if h.contains("gb") || h.contains("handheld") {
        ("Handheld", "Game Bub", 0)
    } else if h.contains("arcade") {
        ("Arcade", "Game Bub", 0)
    } else if h.contains("computer") || h.contains("c64") || h.contains("msx") {
        ("Computer", "Game Bub", 0)
    } else {
        ("Console", "Game Bub", 0)
    }
}

/// Parses `https://cores.gamebub.net/cores.json` (`schema_version: 1`) into Omega's `Catalog` format.
pub fn parse_catalog(bytes: &[u8]) -> Result<Catalog, TauError> {
    let v: Value =
        serde_json::from_slice(bytes).map_err(|_| bad("Game Bub cores.json is not valid JSON."))?;
    let arr = v
        .get("cores")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("Game Bub cores.json is missing the 'cores' array."))?;

    let mut cores = Vec::with_capacity(arr.len());
    for item in arr {
        let id = str_field(item, "id");
        if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
            continue;
        }
        // Live cores.gamebub.net/cores.json nests release fields under `latest`; fall back to top-level for flat JSON.
        let latest = item.get("latest").filter(|v| v.is_object()).unwrap_or(item);
        let pick_str = |k: &str| {
            let s = str_field(latest, k);
            if s.is_empty() { str_field(item, k) } else { s }
        };

        let name = str_field(item, "name");
        let author = str_field(item, "author");
        let description = str_field(item, "description");
        let version = pick_str("version");
        let release_date = pick_str("release_date");
        let license = str_field(item, "license");
        let min_firmware = pick_str("min_firmware");
        let download_url = pick_str("download_url");
        let asset_sha256 = pick_str("asset_sha256").to_ascii_lowercase();
        let asset_size = latest
            .get("asset_size")
            .or_else(|| item.get("asset_size"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let zip_layout = pick_str("zip_layout");
        let repo_raw = item
            .get("repo")
            .and_then(Value::as_str)
            .or_else(|| item.get("repo_url").and_then(Value::as_str))
            .unwrap_or_default();
        let repository = parse_repo_slug(repo_raw);
        let targets: Vec<String> = latest
            .get("targets")
            .or_else(|| item.get("targets"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();

        let history = if !download_url.is_empty() {
            vec![ReleaseSummary {
                version: version.clone(),
                release_date: release_date.clone(),
                download_url: download_url.clone(),
            }]
        } else {
            Vec::new()
        };

        let display_name = if name.is_empty() { id.clone() } else { name };
        let (category, manufacturer, year) = classify_system(&id, &display_name);

        cores.push(CatalogCore {
            id,
            author,
            name: display_name,
            repository,
            category: category.into(),
            manufacturer: manufacturer.into(),
            year,
            version,
            release_date,
            download_url,
            firmware_required: min_firmware,
            history,
            source: GAMEBUB_SOURCE.into(),
            device: DeviceKind::GameBub,
            description,
            license,
            targets,
            asset_sha256,
            asset_size,
            zip_layout,
            ..CatalogCore::default()
        });
    }

    Ok(Catalog { cores })
}

/// Inspects a Game Bub microSD card (read-only): installed cores under `/cores/` and built-in BIOS files under `/system/`.
pub fn inspect_card(card: &Path) -> GameBubCardView {
    let mut cores = Vec::new();
    let cores_dir = if card.join("cores").is_dir() {
        card.join("cores")
    } else {
        card.join("Cores")
    };
    if let Ok(entries) = fs::read_dir(&cores_dir) {
        let mut dirs: Vec<_> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .collect();
        dirs.sort_by_key(|e| e.file_name());
        for e in dirs {
            let folder = e.file_name().to_string_lossy().into_owned();
            if folder.starts_with('.') {
                continue;
            }
            let dir = e.path();
            let Ok(cj_bytes) = fs::read(dir.join("core.json")) else {
                continue;
            };
            let Ok(core) = parse_core_json(&cj_bytes) else {
                continue;
            };
            let missing_bitstreams: Vec<String> = core
                .bitstreams
                .iter()
                .filter(|b| !dir.join(&b.filename).is_file())
                .map(|b| b.filename.clone())
                .collect();
            let files = fs::read(dir.join("files.json"))
                .ok()
                .and_then(|b| parse_files_json(&b).ok())
                .unwrap_or_default();
            let missing_required_files: Vec<String> = files
                .iter()
                .filter(|f| !f.optional)
                .filter_map(|f| f.filename.as_ref())
                .filter(|fname| !dir.join(fname).is_file())
                .cloned()
                .collect();
            let settings = fs::read(dir.join("settings.json"))
                .ok()
                .and_then(|b| parse_settings_json(&b).ok())
                .unwrap_or_default();
            cores.push(GameBubInstalledCore {
                folder,
                core,
                missing_bitstreams,
                files,
                missing_required_files,
                settings,
            });
        }
    }

    let sys = card.join("system");
    let bios = SYSTEM_BIOS
        .iter()
        .map(|(filename, label, expected_bytes)| {
            let p = sys.join(filename);
            let actual_bytes = fs::metadata(&p)
                .ok()
                .filter(|m| m.is_file())
                .map(|m| m.len());
            GameBubBiosStatus {
                filename: (*filename).to_string(),
                label: (*label).to_string(),
                expected_bytes: *expected_bytes,
                present: actual_bytes.is_some(),
                actual_bytes,
                size_ok: actual_bytes == Some(*expected_bytes),
            }
        })
        .collect();

    GameBubCardView { cores, bios }
}

/// Compares installed Game Bub cores on `card` against the Game Bub catalog entries.
pub fn check_updates(card: &Path, catalog: &Catalog) -> Vec<CoreStatus> {
    let view = inspect_card(card);
    view.cores
        .into_iter()
        .map(|inst| {
            let id = inst.core.id.clone();
            let installed_version = inst.core.version.clone();
            let Some(cat) = catalog
                .cores
                .iter()
                .find(|c| c.device == DeviceKind::GameBub && c.id == id)
            else {
                return CoreStatus {
                    id,
                    installed_version,
                    latest_version: String::new(),
                    release_date: String::new(),
                    status: Status::NotInCatalog,
                    manual_url: None,
                };
            };
            let status = match core_version::compare(&installed_version, &cat.version) {
                Verdict::Newer => Status::UpdateAvailable,
                Verdict::Same => Status::UpToDate,
                Verdict::Older => Status::NewerThanCatalog,
                Verdict::Unknown => Status::Unknown,
            };
            CoreStatus {
                id,
                installed_version,
                latest_version: cat.version.clone(),
                release_date: cat.release_date.clone(),
                status,
                manual_url: None,
            }
        })
        .collect()
}
