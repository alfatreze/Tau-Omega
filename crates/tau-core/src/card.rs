//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

/// Derives the on-card path prefix baked into every entry of a media root's
/// index: `Assets/<platform>/common` becomes `/Assets/<platform>/common/`.
/// This is the single implementation of that rule; front-ends call it rather
/// than re-deriving the prefix themselves (it determines paths written into
/// the index, and any divergence between hosts corrupts a real card).
pub fn root_prefix(common: &Path) -> Result<String, TauError> {
    let platform = common
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            TauError::e(
                ErrorCode::InvalidMediaRoot,
                "destination must be Assets/<platform>/common",
            )
        })?;
    Ok(format!("/Assets/{platform}/common/"))
}

/// Whether a core's on-card media root has a valid, absent, or broken index.
/// Computed once here so no front-end re-derives the media-root path or
/// re-reads the index file itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum IndexStatus {
    NoIndex,
    Ready { tracks: u16 },
    NeedsRepair,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Core {
    pub id: String,
    pub author: String,
    pub shortname: String,
    pub version: String,
    /// The build platform: the first `platform_ids` entry.
    pub platform: String,
    /// Every declared platform id, in order.
    pub platforms: Vec<String>,
    /// Platform whose `common/` holds this core's library, covers and music
    /// (data-slot bits [25:24] of the library slot; equals `platform` for a
    /// plain core).
    pub media_platform: String,
    pub library_capable: bool,
    pub index_status: IndexStatus,
    /// The declaring platform's own `category` (`Platforms/<platform>.json`'s
    /// `platform.category`, e.g. `"Media Players"`), when that file exists
    /// and parses. This is the real APF signal for "is this a media player
    /// core" -- general across authors, not a guess from the core's id or
    /// name.
    pub platform_category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Card {
    pub root: PathBuf,
    pub is_pocket_card: bool,
    pub cores: Vec<Core>,
    pub warnings: Vec<Warning>,
}

/// Reads card metadata only; no filesystem mutation is performed.
pub fn inspect_card(root: impl AsRef<Path>) -> Result<Card, TauError> {
    let root = root.as_ref().canonicalize().map_err(TauError::from)?;
    let is_pocket_card = root.join("Cores").is_dir() && root.join("Assets").is_dir();
    let mut warnings = Vec::new();
    if !is_pocket_card {
        warnings.push(Warning::new(
            WarningCode::NotAPocketCard,
            "This folder does not contain both Cores and Assets.",
        ));
    }
    let mut cores = Vec::new();
    let core_root = root.join("Cores");
    if core_root.is_dir() {
        let mut folders: Vec<_> = fs::read_dir(&core_root)?
            .filter_map(Result::ok)
            .filter(|p| p.path().is_dir())
            .collect();
        folders.sort_by_key(|p| p.file_name());
        for folder in folders {
            let id = folder.file_name().to_string_lossy().to_string();
            let core_json = folder.path().join("core.json");
            if !core_json.is_file() {
                warnings.push(Warning::new(
                    WarningCode::MissingCoreJson,
                    format!("{id}: missing core.json"),
                ));
                continue;
            }
            let json: Value = serde_json::from_slice(&fs::read(&core_json)?)
                .map_err(|e| TauError::e(ErrorCode::Json, format!("{id}/core.json: {e}")))?;
            let metadata = json.pointer("/core/metadata").ok_or_else(|| {
                TauError::e(
                    ErrorCode::Json,
                    format!("{id}/core.json: missing core.metadata"),
                )
            })?;
            let string = |key: &str| {
                metadata
                    .get(key)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let platforms = cardlayout::platform_ids(&json);
            let platform = platforms.first().cloned().unwrap_or_default();
            let data_json = folder.path().join("data.json");
            let data_value = fs::read(&data_json)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
            let library_capable = data_value.as_ref().map(slots_have_library).unwrap_or(false);
            let media_platform = cardlayout::media_platform(&platforms, data_value.as_ref());
            let index_status = index_status_for(&root, &media_platform);
            let platform_category = platform_category_for(&root, &platform);
            cores.push(Core {
                id,
                author: string("author"),
                shortname: string("shortname"),
                version: string("version"),
                platform,
                platforms,
                media_platform,
                library_capable,
                index_status,
                platform_category,
            });
        }
    }
    Ok(Card {
        root,
        is_pocket_card,
        cores,
        warnings,
    })
}

/// The media root a core's index lives under, and its current status. Moved
/// out of the Tauri adapter, which used to derive this path and status itself
/// (D-001): a front-end reads `Core::index_status`, it never recomputes it.
pub(super) fn index_status_for(card_root: &Path, platform: &str) -> IndexStatus {
    if platform.is_empty() {
        return IndexStatus::NoIndex;
    }
    let index_path = card_root
        .join("Assets")
        .join(platform)
        .join("common")
        .join("tau-library.tdb");
    match fs::read(&index_path) {
        Err(_) => IndexStatus::NoIndex,
        Ok(bytes) => match parse(&bytes) {
            Ok(parsed) => IndexStatus::Ready {
                tracks: parsed.counts.tracks,
            },
            Err(_) => IndexStatus::NeedsRepair,
        },
    }
}

/// Reads `Platforms/<platform>.json`'s `platform.category` (e.g.
/// `"Media Players"`), the real APF signal a front-end can use to tell a
/// media-player core apart from any other kind, without guessing from the
/// core's id, author or name. `None` when the platform id is empty, the file
/// is missing, or it doesn't parse -- never an error, since this is display
/// metadata, not something `inspect_card` should fail over.
pub(super) fn platform_category_for(card_root: &Path, platform: &str) -> Option<String> {
    if platform.is_empty() {
        return None;
    }
    let path = card_root.join("Platforms").join(format!("{platform}.json"));
    let json: Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    json.pointer("/platform/category")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// A core supports the media library if any data slot serves `tau-library.tdb`.
///
/// Matched by filename, never by slot id: the id is the core author's choice and
/// the shipped Tau core has already moved its other slots around. The real APF
/// layout is `{"data": {"data_slots": [...]}}`; the flatter shapes are accepted
/// because older hand-written cores use them.
pub(super) fn slots_have_library(json: &Value) -> bool {
    let serves_library = |items: Option<&Vec<Value>>| {
        items
            .into_iter()
            .flatten()
            .any(|v| v.get("filename").and_then(Value::as_str) == Some("tau-library.tdb"))
    };
    let slots = |pointer: &str| serves_library(json.pointer(pointer).and_then(Value::as_array));
    slots("/data/data_slots")
        || slots("/core/data/data_slots")
        || slots("/data")
        || slots("/core/data")
}
