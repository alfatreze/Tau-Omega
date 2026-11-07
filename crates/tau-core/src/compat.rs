//! Tau release manifests (`tau-compat.json`, schemas 1 and 2): what a release expects on a card, read from the release
//! itself instead of from rules kept in Omega.
//!
//! Source of truth: tau-alpha `tools/tau_compat.py` (generator, verifier and the reference `check-card`), its JSON Schema
//! `docs/schemas/tau-compat.schema.json` and `RELEASE_SYSTEM_SPEC.md` sections 10-11. Rules this module follows:
//! - **refuse, don't guess:** a schema number this build does not know is refused ([`ErrorCode::InvalidReleaseManifest`]);
//!   unknown keys inside a known schema are ignored (they are additive);
//! - **identity by hash:** the installed release is the one whose owned files all match the card ([`identify_installed`]);
//!   `core.json` carries `0.6.0` for every alpha, so the version alone cannot say which build is installed;
//! - **union rule:** a card on release R must treat every persisted id whose registry `since` is later than R as changed
//!   ([`persist_changed_since`]), which also covers releases the user skipped;
//! - **card check:** owned files by hash, shared files present, generated and user files in a format the release reads,
//!   obsolete files gone, no stray files in the core's folders ([`check_card`]), as the reference tool does.

use crate::{ErrorCode, TauError};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Schema numbers this build reads.
pub const SUPPORTED_SCHEMAS: [u64; 2] = [1, 2];
/// Files examined per pattern entry (covers), so a large library does not stall the check.
const MAX_PATTERN_FILES: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Role {
    /// Install and replace exactly; the hash must match.
    Owned,
    /// Install if missing; other cores may use it, so a different hash is only a warning.
    Shared,
    /// Never in the zip; the companion builds it in the stated format.
    Generated,
    /// Never overwritten; checked for a format the release reads.
    User,
    /// Removed on update.
    Obsolete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Format {
    pub name: String,
    pub version: u64,
    /// tau-library: the absolute card path the index must embed as its root.
    pub root: Option<String>,
    /// TAUA: the sections the firmware reads.
    pub sections: Vec<String>,
    /// TAUA: the largest file the firmware reads.
    pub max_bytes: Option<u64>,
    /// TAUA: a writer must carry every section it does not write, byte for byte.
    pub preserve_unknown_sections: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayoutEntry {
    pub path: String,
    pub role: Role,
    pub sha256: Option<String>,
    pub slot: Option<u64>,
    pub required: bool,
    /// `path` uses `**` (any folders) and `*.{a,b}`.
    pub pattern: bool,
    pub format: Option<Format>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CompatPackage {
    pub zip: String,
    pub zip_sha256: String,
    pub core_id: String,
    pub bitstream_sha256: String,
    pub bitstream_core_version: String,
    pub bitstream_features: Option<Vec<String>>,
    pub rom_sha256: String,
    pub cold_sha256: String,
    pub rom_accepts: Vec<String>,
    pub rom_needs: Vec<String>,
    /// Core ids this package replaces (e.g. `alfatreze.TAU_DIAGNOSTIC` for `alfatreze.TAU Diagnostics`).
    #[cfg_attr(feature = "serde", serde(default))]
    pub replaces: Vec<String>,
    /// The firmware's full stamped version (`0.7.0-preview.1+6423e42`), when the release records it.
    #[cfg_attr(feature = "serde", serde(default))]
    pub rom_version: Option<String>,
    /// Empty for schema 1.
    pub layout: Vec<LayoutEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PersistMeaning {
    pub name: String,
    pub meaning: u64,
    pub since: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CompatDoc {
    pub schema: u64,
    pub release: String,
    pub date_release: String,
    pub prerelease: bool,
    pub previous_release: Option<String>,
    pub packages: Vec<CompatPackage>,
    pub persist_ids_changed: Vec<u64>,
    /// Empty for schema 1 and for files written before the registry existed.
    pub persist_registry: BTreeMap<u64, PersistMeaning>,
    pub min_omega: String,
    /// `requires_omega.persist_layout`: the persisted-settings layout version this release writes (word 0 bits 30..27); absent = unknown.
    #[cfg_attr(feature = "serde", serde(default))]
    pub persist_layout: Option<u64>,
    /// `requires_omega.packs_abi`: the meter pack ABI major version inside `tau-packs.bin`; absent = unknown.
    #[cfg_attr(feature = "serde", serde(default))]
    pub packs_abi: Option<u64>,
    pub notes: String,
    /// The commit the release was built from, and whether the tree had uncommitted changes.
    #[cfg_attr(feature = "serde", serde(default))]
    pub source_commit: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub source_dirty: bool,
}

fn bad(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidReleaseManifest, msg)
}

fn s(v: &Value, key: &str) -> Result<String, TauError> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| bad(format!("tau-compat.json: missing text field \"{key}\".")))
}

fn str_list(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Parses a `tau-compat.json`. Refuses a schema this build does not know and anything structurally wrong; ignores
/// unknown keys.
pub fn parse_compat(bytes: &[u8]) -> Result<CompatDoc, TauError> {
    let v: Value = serde_json::from_slice(bytes)
        .map_err(|e| bad(format!("tau-compat.json is not valid JSON: {e}.")))?;
    let schema = v
        .get("schema")
        .and_then(Value::as_u64)
        .ok_or_else(|| bad("tau-compat.json has no schema number."))?;
    if !SUPPORTED_SCHEMAS.contains(&schema) {
        return Err(bad(format!(
            "This release manifest uses schema {schema}; this Omega reads schemas 1 and 2. Update Omega before installing this release."
        )));
    }
    let release = s(&v, "release")?;
    tag_key(&release).ok_or_else(|| {
        bad(format!(
            "tau-compat.json: \"{release}\" is not a release tag."
        ))
    })?;
    let mut packages = Vec::new();
    for p in v
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("tau-compat.json has no packages."))?
    {
        let mut layout = Vec::new();
        for e in p
            .get("layout")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let role = match e.get("role").and_then(Value::as_str) {
                Some("owned") => Role::Owned,
                Some("shared") => Role::Shared,
                Some("generated") => Role::Generated,
                Some("user") => Role::User,
                Some("obsolete") => Role::Obsolete,
                other => {
                    return Err(bad(format!(
                        "tau-compat.json: unknown layout role {other:?}."
                    )));
                }
            };
            let path = s(e, "path")?;
            if path.split('/').any(|c| c == ".." || c.is_empty()) || path.starts_with('/') {
                return Err(bad(format!(
                    "tau-compat.json: unsafe layout path \"{path}\"."
                )));
            }
            let format = e.get("format").map(|f| -> Result<Format, TauError> {
                Ok(Format {
                    name: s(f, "name")?,
                    version: f
                        .get("version")
                        .and_then(Value::as_u64)
                        .ok_or_else(|| bad("format without version"))?,
                    root: f.get("root").and_then(Value::as_str).map(str::to_string),
                    sections: str_list(f.get("sections")),
                    max_bytes: f.get("max_bytes").and_then(Value::as_u64),
                    preserve_unknown_sections: f
                        .get("preserve_unknown_sections")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                })
            });
            layout.push(LayoutEntry {
                path,
                role,
                sha256: e.get("sha256").and_then(Value::as_str).map(str::to_string),
                slot: e.get("slot").and_then(Value::as_u64),
                required: e.get("required").and_then(Value::as_bool).unwrap_or(false),
                pattern: e.get("pattern").and_then(Value::as_bool).unwrap_or(false),
                format: format.transpose()?,
            });
        }
        packages.push(CompatPackage {
            zip: s(p, "zip")?,
            zip_sha256: s(p, "zip_sha256")?,
            core_id: s(p, "core_id")?,
            bitstream_sha256: s(p, "bitstream_sha256")?,
            bitstream_core_version: s(p, "bitstream_core_version")?,
            bitstream_features: p
                .get("bitstream_features")
                .filter(|f| !f.is_null())
                .map(|f| str_list(Some(f))),
            rom_sha256: s(p, "rom_sha256")?,
            cold_sha256: s(p, "cold_sha256")?,
            rom_accepts: str_list(p.get("rom_accepts")),
            rom_needs: str_list(p.get("rom_needs")),
            replaces: str_list(p.get("replaces")),
            rom_version: p
                .get("rom_version")
                .and_then(Value::as_str)
                .map(str::to_string),
            layout,
        });
    }
    if schema >= 2 && packages.iter().any(|p| p.layout.is_empty()) {
        return Err(bad("tau-compat.json schema 2 without a layout."));
    }
    let req = v
        .get("requires_omega")
        .ok_or_else(|| bad("tau-compat.json has no requires_omega."))?;
    let mut persist_registry = BTreeMap::new();
    if let Some(reg) = v.get("persist_registry").and_then(Value::as_object) {
        for (id, m) in reg {
            let id: u64 = id
                .parse()
                .map_err(|_| bad(format!("persist_registry id \"{id}\" is not a number")))?;
            persist_registry.insert(
                id,
                PersistMeaning {
                    name: s(m, "name")?,
                    meaning: m.get("meaning").and_then(Value::as_u64).unwrap_or(1),
                    since: s(m, "since")?,
                },
            );
        }
    }
    Ok(CompatDoc {
        schema,
        date_release: s(&v, "date_release")?,
        prerelease: v
            .get("prerelease")
            .and_then(Value::as_bool)
            .unwrap_or(release.contains('-')),
        previous_release: v
            .get("previous_release")
            .and_then(Value::as_str)
            .map(str::to_string),
        packages,
        persist_ids_changed: req
            .get("persist_ids_changed")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default(),
        persist_registry,
        min_omega: s(req, "min_omega")?,
        persist_layout: req.get("persist_layout").and_then(Value::as_u64),
        packs_abi: req.get("packs_abi").and_then(Value::as_u64),
        notes: v
            .get("notes")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        source_commit: v
            .pointer("/source/commit")
            .and_then(Value::as_str)
            .map(str::to_string),
        source_dirty: v
            .pointer("/source/dirty")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        release,
    })
}

/// SemVer precedence key of a `vX.Y.Z[-pre]` tag (identifiers compared numerically when both are numbers, else as text;
/// a release sorts after its pre-releases). `None` when the text is not a tag.
/// (major, minor, patch, is a release, pre-release identifiers as (is text, number, text)).
type TagKey = (u64, u64, u64, bool, Vec<(bool, u64, String)>);

/// Whether `tag` is a release tag (`vX.Y.Z[-pre]`).
pub fn is_release_tag(tag: &str) -> bool {
    tag_key(tag).is_some()
}

fn tag_key(tag: &str) -> Option<TagKey> {
    let rest = tag.strip_prefix('v')?;
    let (core, pre) = rest
        .split_once('-')
        .map_or((rest, None), |(c, p)| (c, Some(p)));
    let mut n = core.split('.').map(|x| x.parse::<u64>().ok());
    let (a, b, c) = (n.next()??, n.next()??, n.next()??);
    if n.next().is_some() {
        return None;
    }
    let ids = match pre {
        None => Vec::new(),
        Some(p)
            if !p.is_empty()
                && p.split('.')
                    .all(|x| !x.is_empty() && x.bytes().all(|ch| ch.is_ascii_alphanumeric())) =>
        {
            p.split('.')
                .map(|x| {
                    x.parse::<u64>()
                        .map_or((true, 0, x.to_string()), |num| (false, num, String::new()))
                })
                .collect()
        }
        Some(_) => return None,
    };
    Some((a, b, c, pre.is_none(), ids))
}

/// Orders two release tags by SemVer precedence (`v0.6.0-alpha.4 < v0.6.0-alpha.10 < v0.6.0-preview.1 < v0.6.0`).
/// Unparseable tags sort first.
pub fn compare_tags(a: &str, b: &str) -> Ordering {
    tag_key(a).cmp(&tag_key(b))
}

/// What this Tau Omega understands of the capability keys in `requires_omega` (Tau Alpha's interop message 001, Q-002): it reads
/// persisted-settings layout 1 and leaves a meter pack file of ABI 1 alone. A release that needs more is refused, not guessed.
pub const OMEGA_PERSIST_LAYOUT: u64 = 1;
pub const OMEGA_PACKS_ABI: u64 = 1;

/// Why this Tau Omega must not install `doc`'s release: `min_omega` newer than `omega_version`, or a capability key higher than
/// what it supports. A missing key is unknown and never blocks. Empty when nothing stands in the way.
pub fn requirements_unmet(doc: &CompatDoc, omega_version: &str) -> Vec<String> {
    use crate::core_version::{Verdict, compare};
    let mut out = Vec::new();
    if !doc.min_omega.trim().is_empty() && compare(omega_version, &doc.min_omega) == Verdict::Newer
    {
        out.push(format!(
            "{} needs Tau Omega {} or newer; this is {omega_version}. Update Tau Omega first.",
            doc.release, doc.min_omega
        ));
    }
    if let Some(n) = doc.persist_layout.filter(|n| *n > OMEGA_PERSIST_LAYOUT) {
        out.push(format!(
            "{} writes persisted settings in layout {n}; this Tau Omega reads layout {OMEGA_PERSIST_LAYOUT}. Update Tau Omega first.",
            doc.release
        ));
    }
    if let Some(n) = doc.packs_abi.filter(|n| *n > OMEGA_PACKS_ABI) {
        out.push(format!(
            "{} carries meter packs of ABI {n}; this Tau Omega knows ABI {OMEGA_PACKS_ABI}. Update Tau Omega first.",
            doc.release
        ));
    }
    out
}

/// Persisted ids a card on `card_release` must treat as changed when this release is installed (the union rule).
/// `None` when it cannot be known: no registry and the manifest was not computed against `card_release`.
pub fn persist_changed_since(doc: &CompatDoc, card_release: &str) -> Option<Vec<u64>> {
    let mut out: BTreeSet<u64> = BTreeSet::new();
    let exact = doc.previous_release.as_deref() == Some(card_release);
    if exact {
        out.extend(doc.persist_ids_changed.iter().copied());
    }
    if doc.persist_registry.is_empty() {
        return exact.then(|| out.into_iter().collect());
    }
    for (id, m) in &doc.persist_registry {
        if compare_tags(&m.since, card_release) == Ordering::Greater {
            out.insert(*id);
        }
    }
    Some(out.into_iter().collect())
}

fn sha256_file(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(
        Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

/// The releases (from `docs`) whose package for `core_id` matches the card exactly: every owned file of its layout
/// present with its hash (schema 2), or the bitstream, ROM and cold image (schema 1, either the core folder or the old
/// `common/` place). Empty means an unknown build.
pub fn identify_installed(docs: &[CompatDoc], card_root: &Path, core_id: &str) -> Vec<String> {
    let mut out = Vec::new();
    for doc in docs {
        for p in doc.packages.iter().filter(|p| p.core_id == core_id) {
            let matches = if p.layout.is_empty() {
                let platform = platform_of(card_root, core_id).unwrap_or_default();
                let either = |name: &str| {
                    [
                        format!("Assets/{platform}/{core_id}/{name}"),
                        format!("Assets/{platform}/common/{name}"),
                    ]
                    .iter()
                    .find_map(|rel| sha256_file(&card_root.join(rel)))
                };
                sha256_file(&card_root.join(format!("Cores/{core_id}/bitstream.rbf_r"))).as_deref()
                    == Some(&p.bitstream_sha256[..])
                    && either("tau.rom").as_deref() == Some(&p.rom_sha256[..])
                    && either("tau-cold.bin").as_deref() == Some(&p.cold_sha256[..])
            } else {
                p.layout
                    .iter()
                    .filter(|e| e.role == Role::Owned && !e.pattern)
                    .all(|e| {
                        e.sha256.is_some() && sha256_file(&card_root.join(&e.path)) == e.sha256
                    })
            };
            if matches {
                out.push(doc.release.clone());
            }
        }
    }
    out
}

fn platform_of(card_root: &Path, core_id: &str) -> Option<String> {
    let j: Value = serde_json::from_slice(
        &fs::read(card_root.join("Cores").join(core_id).join("core.json")).ok()?,
    )
    .ok()?;
    j.pointer("/core/metadata/platform_ids/0")
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Finding {
    /// True for an error (do not rely on the install); false for a warning.
    pub error: bool,
    pub message: String,
}

/// Why `bytes` is not in `fmt`, or `None` when it is.
fn format_problem(bytes: &[u8], fmt: &Format) -> Option<String> {
    match fmt.name.as_str() {
        "tau-library" => {
            if bytes.len() < 8 || bytes[..4] != 0x4249_4C54u32.to_le_bytes() {
                return Some("not a Tau library index".into());
            }
            let needs = u64::from(u16::from_le_bytes([bytes[6], bytes[7]]));
            if needs > fmt.version {
                return Some(format!(
                    "the index needs reader version {needs}; this release reads up to {}",
                    fmt.version
                ));
            }
            if let Some(root) = &fmt.root {
                let ix = match crate::parse(bytes) {
                    Ok(ix) => ix,
                    Err(e) => return Some(format!("the index does not parse ({})", e.message)),
                };
                match crate::string_at(&ix, ix.root) {
                    Ok(r) if r == *root => {}
                    Ok(r) => {
                        return Some(format!(
                            "the index root is {r}; this core's music is under {root}, so tracks would not open"
                        ));
                    }
                    Err(e) => {
                        return Some(format!("the index root cannot be read ({})", e.message));
                    }
                }
            }
            None
        }
        "TAUA" => {
            if bytes.len() < 12 || &bytes[..4] != b"TAUA" {
                return Some("not a Tau assets file".into());
            }
            let v = u64::from(u16::from_le_bytes([bytes[4], bytes[5]]));
            if v != fmt.version {
                return Some(format!(
                    "assets file version {v}; this release reads version {}",
                    fmt.version
                ));
            }
            match fmt.max_bytes {
                Some(max) if bytes.len() as u64 > max => Some(format!(
                    "{} bytes; this release reads at most {max}",
                    bytes.len()
                )),
                _ => None,
            }
        }
        "tau-names" => {
            // The firmware's own check (NAME_SIDECAR_SPEC 5): magic, version, header CRC.
            // Binding to a particular index is not an error here: the firmware just ignores a stale file.
            if bytes.len() < 48 || &bytes[..4] != b"TNMS" {
                return Some("not a Tau name sidecar".into());
            }
            let v = u64::from(u16::from_le_bytes([bytes[4], bytes[5]]));
            if v > fmt.version {
                return Some(format!(
                    "sidecar version {v}; this release reads up to {}",
                    fmt.version
                ));
            }
            let crc = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
            (crc32fast::hash(&bytes[..32]) != crc).then(|| "the sidecar header is damaged".into())
        }
        "tau-names-manifest" => {
            let Ok(v) = serde_json::from_slice::<Value>(bytes) else {
                return Some("not valid JSON".into());
            };
            if v["format"] != "tau-names" {
                return Some("not a name sidecar manifest".into());
            }
            let version = v["version"].as_u64().unwrap_or(0);
            (version > fmt.version).then(|| {
                format!(
                    "manifest version {version}; this release reads up to {}",
                    fmt.version
                )
            })
        }
        "TIM1" => (bytes.get(..4) != Some(b"TIM1")).then(|| "not a TIM1 cover image".into()),
        "interact_persist" => serde_json::from_slice::<Value>(bytes)
            .err()
            .map(|_| "not valid JSON".into()),
        _ => None, // a format this build does not know: the release reader decides, not Omega
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("._") || n == ".DS_Store")
        {
            continue;
        }
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// Files under `card_root` matching a layout pattern (`base/**/*.{a,b}` or `base/**/tail`).
fn pattern_files(card_root: &Path, pattern: &str) -> Vec<PathBuf> {
    let Some((base, rest)) = pattern.split_once("**/") else {
        return Vec::new();
    };
    let mut all = Vec::new();
    walk(&card_root.join(base), &mut all);
    let keep = |p: &PathBuf| -> bool {
        if let Some(exts) = rest.strip_prefix("*.{").and_then(|r| r.strip_suffix('}')) {
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            exts.split(',').any(|x| x == ext)
        } else {
            p.to_string_lossy()
                .replace('\\', "/")
                .ends_with(&format!("/{rest}"))
        }
    };
    all.into_iter()
        .filter(keep)
        .take(MAX_PATTERN_FILES)
        .collect()
}

/// Checks a card (or an unpacked install) against the manifest's package for `core_id`. An empty list means the card
/// matches. Same rules as tau-alpha's `tau_compat.check_card`.
pub fn check_card(doc: &CompatDoc, card_root: &Path, core_id: &str) -> Vec<Finding> {
    let err = |m: String| Finding {
        error: true,
        message: m,
    };
    let warn = |m: String| Finding {
        error: false,
        message: m,
    };
    let Some(p) = doc.packages.iter().find(|p| p.core_id == core_id) else {
        return vec![err(format!(
            "{} has no package for {core_id}.",
            doc.release
        ))];
    };
    if !card_root.join("Cores").join(core_id).is_dir() {
        return vec![err(format!("{core_id} is not installed on this card."))];
    }
    let mut out = Vec::new();
    let mut listed: BTreeSet<&str> = BTreeSet::new();
    let (core_platforms, _) = crate::cardlayout::core_platforms(card_root, core_id);
    let owned: Vec<String> = p
        .layout
        .iter()
        .filter(|e| e.role == Role::Owned)
        .map(|e| e.path.clone())
        .collect();
    for e in &p.layout {
        listed.insert(&e.path);
        let f = card_root.join(&e.path);
        match e.role {
            Role::Owned | Role::Shared => match sha256_file(&f) {
                // An optional owned DATA-SLOT file (`required: false`, a slot: tau-cold.bin, tau-loading.bin, tau-packs.bin) is a
                // warning (interop Q-001): the core falls back without it. Deliberately narrower than Tau's reference, which
                // downgrades every `required: false` owned file: the rehearsal manifest marks core.json, data.json and
                // bitstream.rbf_r `required: false` too (no data slot), and a core without those does not load.
                None if !e.required && e.role == Role::Owned && e.slot.is_some() => out.push(warn(
                    format!("{} is missing; the core falls back without it.", e.path),
                )),
                None => out.push(err(format!("{} is missing.", e.path))),
                Some(h) if Some(&h) != e.sha256.as_ref() => out.push(if e.role == Role::Owned {
                    err(format!("{} differs from {}.", e.path, doc.release))
                } else {
                    warn(format!(
                        "{} differs from {} (shared with other cores).",
                        e.path, doc.release
                    ))
                }),
                Some(_) => {}
            },
            Role::Obsolete => {
                if !crate::cardlayout::obsolete_path_allowed(
                    card_root,
                    core_id,
                    &core_platforms,
                    &owned,
                    &e.path,
                ) {
                    if f.exists() {
                        out.push(warn(format!(
                            "{} is listed obsolete but is outside this core's own files; it is not touched.",
                            e.path
                        )));
                    }
                    continue;
                }
                if f.exists() {
                    out.push(warn(format!(
                        "{} is obsolete and should be removed.",
                        e.path
                    )));
                }
            }
            Role::Generated | Role::User => {
                let Some(fmt) = &e.format else { continue };
                let files = if e.pattern {
                    pattern_files(card_root, &e.path)
                } else {
                    vec![f.clone()]
                };
                for g in files {
                    match fs::read(&g) {
                        Ok(bytes) => {
                            if let Some(why) = format_problem(&bytes, fmt) {
                                let rel = g
                                    .strip_prefix(card_root)
                                    .unwrap_or(&g)
                                    .to_string_lossy()
                                    .replace('\\', "/");
                                out.push(err(format!("{rel}: {why}.")));
                            }
                        }
                        Err(_) if e.required && !e.pattern => {
                            out.push(err(format!("required {} is missing.", e.path)))
                        }
                        Err(_) => {}
                    }
                }
            }
        }
    }
    // Stray files in the core's own folders (not part of this release).
    let platform = p.layout.iter().find_map(|e| {
        e.path
            .strip_prefix("Assets/")
            .and_then(|r| r.split('/').next())
            .map(str::to_string)
    });
    let mut dirs = vec![card_root.join("Cores").join(core_id)];
    if let Some(pl) = platform {
        dirs.push(card_root.join("Assets").join(pl).join(core_id));
    }
    for d in dirs {
        let mut files = Vec::new();
        walk(&d, &mut files);
        for f in files {
            let rel = f
                .strip_prefix(card_root)
                .unwrap_or(&f)
                .to_string_lossy()
                .replace('\\', "/");
            if !listed.contains(rel.as_str()) {
                out.push(warn(format!(
                    "{rel} is not part of {} (stale file).",
                    doc.release
                )));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_sidecar_formats_are_checked_like_the_firmware_does() {
        let bin_fmt = Format {
            name: "tau-names".into(),
            version: 1,
            root: None,
            sections: vec![],
            max_bytes: None,
            preserve_unknown_sections: false,
        };
        let man_fmt = Format {
            name: "tau-names-manifest".into(),
            ..bin_fmt.clone()
        };
        // A real sidecar made by the writer passes; damage to magic, version or header does not.
        let side = crate::name_sidecar::NameSide {
            tracks: vec![crate::name_sidecar::TrackSide {
                rel: "A/01.mp3".into(),
                file: "01.mp3".into(),
                title: Some("Песня".into()),
                idx_title: "Pesnia".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let s =
            crate::name_sidecar::build_sidecar(1, &side, &Default::default(), (1, 0, 0)).unwrap();
        let bin = s.binary.unwrap();
        assert_eq!(format_problem(&bin, &bin_fmt), None);
        let mut bad = bin.clone();
        bad[0] = b'X';
        assert!(format_problem(&bad, &bin_fmt).is_some());
        let mut bad = bin.clone();
        bad[4] = 9;
        assert!(format_problem(&bad, &bin_fmt).unwrap().contains("version"));
        let mut bad = bin.clone();
        bad[12] ^= 1;
        assert!(format_problem(&bad, &bin_fmt).unwrap().contains("damaged"));
        let manifest = crate::name_sidecar::manifest_bytes(&s.manifest);
        assert_eq!(format_problem(&manifest, &man_fmt), None);
        assert!(format_problem(b"{\"format\":\"other\"}", &man_fmt).is_some());
        assert!(format_problem(b"{\"format\":\"tau-names\",\"version\":2}", &man_fmt).is_some());
        assert!(format_problem(b"nope", &man_fmt).is_some());
    }

    fn sha(b: &[u8]) -> String {
        Sha256::digest(b)
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect()
    }

    /// A schema-2 manifest for a small release with the core-specific layout, and the card files it describes.
    fn release(card: &Path) -> CompatDoc {
        let files: [(&str, &[u8]); 5] = [
            (
                "Cores/alfatreze.TAU/core.json",
                br#"{"core":{"metadata":{"platform_ids":["tau"]}}}"#,
            ),
            ("Cores/alfatreze.TAU/bitstream.rbf_r", b"bits"),
            ("Assets/tau/alfatreze.TAU/tau.rom", b"rom"),
            ("Assets/tau/alfatreze.TAU/tau-cold.bin", b"cold"),
            ("Platforms/tau.json", b"{}"),
        ];
        let mut layout = Vec::new();
        for (rel, data) in files {
            let f = card.join(rel);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(&f, data).unwrap();
            let role = if rel.starts_with("Platforms/") {
                "shared"
            } else {
                "owned"
            };
            layout.push(serde_json::json!({"path": rel, "role": role, "sha256": sha(data), "slot": null, "required": rel.ends_with("tau.rom")}));
        }
        fs::create_dir_all(card.join("Assets/tau/common")).unwrap();
        layout.push(serde_json::json!({"path": "Assets/tau/common/tau-assets.bin", "role": "user", "slot": 8, "required": false,
            "format": {"name": "TAUA", "version": 1, "sections": ["THEM", "METR", "PRST"], "max_bytes": 65536, "preserve_unknown_sections": true}}));
        layout.push(serde_json::json!({"path": "Assets/tau/common/**/tau-art/cover_128.pal256.timg", "pattern": true, "role": "generated",
            "slot": 7, "required": false, "format": {"name": "TIM1", "version": 1}}));
        layout.push(serde_json::json!({"path": "Assets/tau/common/tau.rom", "role": "obsolete", "slot": null, "required": false}));
        let doc = serde_json::json!({
            "schema": 2, "release": "v0.6.0-preview.1", "date_release": "2026-10-08", "prerelease": true,
            "packages": [{"zip": "alfatreze.TAU_0.6.0_2026-10-08.zip", "zip_sha256": "00", "core_id": "alfatreze.TAU",
                "bitstream_sha256": sha(b"bits"), "bitstream_core_version": "4D50331A", "bitstream_features": ["HALCYON"],
                "rom_sha256": sha(b"rom"), "cold_sha256": sha(b"cold"), "rom_accepts": ["4D50331A"], "rom_needs": ["HALCYON"],
                "layout": layout, "a_future_key": 1}],
            "requires_omega": {"library_index_version": 1, "assets_sections": ["THEM"], "assets_read_limit_bytes": 65536,
                "report_tags_max": 27, "persist_ids_changed": [12, 16], "min_omega": "0.3.0"},
            "notes": "", "previous_release": "v0.6.0-alpha.4",
            "persist_registry": {"10": {"name": "Volume", "meaning": 2, "since": "v0.6.0-alpha.4"},
                                 "16": {"name": "Halcyon EQ preset", "meaning": 2, "since": "v0.6.0-preview.1"},
                                 "28": {"name": "(internal) theme mode", "meaning": 2, "since": "v0.6.0-alpha.4"},
                                 "11": {"name": "Color", "meaning": 1, "since": "v0.3.0"}}
        });
        parse_compat(&serde_json::to_vec(&doc).unwrap()).unwrap()
    }

    fn card(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "tau-compat-{name}-{}-{}",
            std::process::id(),
            crate::test_uniq()
        ));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn unknown_schema_is_refused_unknown_keys_are_not() {
        let doc = br#"{"schema": 3, "release": "v1.0.0"}"#;
        let e = parse_compat(doc).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidReleaseManifest);
        assert!(e.message.contains("schema 3"), "{}", e.message);
        let c = card("keys");
        assert_eq!(
            release(&c).packages[0].bitstream_features,
            Some(vec!["HALCYON".to_string()])
        );
        let _ = fs::remove_dir_all(c);
    }

    #[test]
    fn tag_order_follows_semver() {
        for (a, b) in [
            ("v0.6.0-alpha.4", "v0.6.0-alpha.10"),
            ("v0.6.0-alpha.10", "v0.6.0-preview.1"),
            ("v0.6.0-dev.385", "v0.6.0-preview.1"),
            ("v0.6.0-preview.1", "v0.6.0-rc.1"),
            ("v0.6.0-rc.1", "v0.6.0"),
            ("v0.6.0", "v0.6.1"),
            ("v0.9.9", "v0.10.0"),
        ] {
            assert_eq!(compare_tags(a, b), Ordering::Less, "{a} < {b}");
        }
    }

    #[test]
    fn the_union_rule_covers_skipped_releases() {
        let c = card("union");
        let doc = release(&c);
        assert_eq!(
            persist_changed_since(&doc, "v0.6.0-alpha.4"),
            Some(vec![12, 16]),
            "exact previous: listed ids plus newer meanings"
        );
        assert_eq!(
            persist_changed_since(&doc, "v0.6.0-alpha.3"),
            Some(vec![10, 16, 28]),
            "skipped alpha.4: its meanings count too"
        );
        let mut old = doc.clone();
        old.persist_registry.clear();
        assert_eq!(
            persist_changed_since(&old, "v0.6.0-alpha.3"),
            None,
            "no registry and not the previous release: unknown"
        );
        let _ = fs::remove_dir_all(c);
    }

    #[test]
    fn the_installed_release_is_identified_by_hash() {
        let c = card("identify");
        let doc = release(&c);
        assert_eq!(
            identify_installed(std::slice::from_ref(&doc), &c, "alfatreze.TAU"),
            vec!["v0.6.0-preview.1"]
        );
        fs::write(c.join("Assets/tau/alfatreze.TAU/tau.rom"), b"another build").unwrap();
        assert!(
            identify_installed(&[doc], &c, "alfatreze.TAU").is_empty(),
            "one changed owned file: unknown build"
        );
        let _ = fs::remove_dir_all(c);
    }

    #[test]
    fn check_card_matches_the_reference_rules() {
        let c = card("check");
        let doc = release(&c);
        assert_eq!(
            check_card(&doc, &c, "alfatreze.TAU"),
            vec![],
            "a clean install"
        );
        let rom = c.join("Assets/tau/alfatreze.TAU/tau.rom");
        fs::write(&rom, b"other").unwrap();
        assert!(
            check_card(&doc, &c, "alfatreze.TAU")
                .iter()
                .any(|f| f.error && f.message.contains("tau.rom differs"))
        );
        fs::write(&rom, b"rom").unwrap();
        fs::write(c.join("Platforms/tau.json"), b"{\"x\":1}").unwrap();
        let r = check_card(&doc, &c, "alfatreze.TAU");
        assert!(
            r.len() == 1 && !r[0].error,
            "a shared file from another release is a warning: {r:?}"
        );
        fs::write(c.join("Platforms/tau.json"), b"{}").unwrap();
        fs::write(
            c.join("Assets/tau/common/tau-assets.bin"),
            [b"TAUA".as_slice(), &[2, 0], &[0; 6]].concat(),
        )
        .unwrap();
        assert!(
            check_card(&doc, &c, "alfatreze.TAU")
                .iter()
                .any(|f| f.error && f.message.contains("version 2"))
        );
        fs::write(
            c.join("Assets/tau/common/tau-assets.bin"),
            [b"TAUA".as_slice(), &[1, 0], &vec![0; 70000]].concat(),
        )
        .unwrap();
        assert!(
            check_card(&doc, &c, "alfatreze.TAU")
                .iter()
                .any(|f| f.error && f.message.contains("at most 65536"))
        );
        fs::remove_file(c.join("Assets/tau/common/tau-assets.bin")).unwrap();
        let art = c.join("Assets/tau/common/Album/tau-art");
        fs::create_dir_all(&art).unwrap();
        fs::write(art.join("cover_128.pal256.timg"), b"JPEG").unwrap();
        assert!(
            check_card(&doc, &c, "alfatreze.TAU")
                .iter()
                .any(|f| f.error && f.message.contains("TIM1"))
        );
        fs::write(art.join("cover_128.pal256.timg"), b"TIM1....").unwrap();
        fs::write(c.join("Assets/tau/common/tau.rom"), b"old layout").unwrap();
        fs::write(c.join("Cores/alfatreze.TAU/old.bin"), b"x").unwrap();
        let r = check_card(&doc, &c, "alfatreze.TAU");
        assert!(r.iter().all(|f| !f.error), "{r:?}");
        assert!(
            r.iter().any(|f| f.message.contains("obsolete"))
                && r.iter().any(|f| f.message.contains("stale file")),
            "{r:?}"
        );
        let _ = fs::remove_dir_all(c);
    }

    /// Cross-implementation check against tau-alpha's own generator: a real `tau-compat.json` and the unpacked package it
    /// describes (`TAU_COMPAT_CASE=<dir with tau-compat.json and pocket/>`). Skipped without it (D-015: Omega reads tau-alpha
    /// artefacts in place, never copies them).
    #[test]
    fn a_real_tau_alpha_manifest_parses_and_its_package_checks_clean() {
        let Ok(dir) = std::env::var("TAU_COMPAT_CASE") else {
            eprintln!("skipped: set TAU_COMPAT_CASE to a folder with tau-compat.json and pocket/");
            return;
        };
        let dir = PathBuf::from(dir);
        let doc = parse_compat(&fs::read(dir.join("tau-compat.json")).unwrap()).unwrap();
        let pocket = dir.join("pocket");
        for p in &doc.packages {
            let r = check_card(&doc, &pocket, &p.core_id);
            assert!(r.iter().all(|f| !f.error), "{}: {r:?}", p.core_id);
            assert_eq!(
                identify_installed(std::slice::from_ref(&doc), &pocket, &p.core_id),
                vec![doc.release.clone()]
            );
        }
    }

    fn doc_with_requirements(
        min_omega: &str,
        layout: Option<u64>,
        packs: Option<u64>,
    ) -> CompatDoc {
        let mut d = release(&card("req"));
        d.min_omega = min_omega.into();
        d.persist_layout = layout;
        d.packs_abi = packs;
        d
    }

    #[test]
    fn requirements_compare_min_omega_and_the_capability_keys() {
        let ok = |d: &CompatDoc| requirements_unmet(d, "0.6.0");
        assert!(
            ok(&doc_with_requirements("0.6.0", Some(1), Some(1))).is_empty(),
            "equal is fine"
        );
        assert!(
            ok(&doc_with_requirements("0.3.0", None, None)).is_empty(),
            "missing keys are unknown, never a block"
        );
        let newer = requirements_unmet(&doc_with_requirements("0.6.0", None, None), "0.5.2");
        assert!(
            newer.len() == 1 && newer[0].contains("needs Tau Omega 0.6.0 or newer"),
            "{newer:?}"
        );
        let layout = ok(&doc_with_requirements("0.3.0", Some(2), None));
        assert!(
            layout.len() == 1
                && layout[0].contains("layout 2")
                && layout[0].contains("reads layout 1"),
            "{layout:?}"
        );
        let packs = ok(&doc_with_requirements("0.3.0", None, Some(2)));
        assert!(packs.len() == 1 && packs[0].contains("ABI 2"), "{packs:?}");
        assert_eq!(
            ok(&doc_with_requirements("0.7.0", Some(3), Some(3))).len(),
            3,
            "every reason is listed"
        );
    }

    #[test]
    fn a_missing_optional_owned_file_is_a_warning_and_a_missing_required_one_an_error() {
        let c = card("optional");
        let mut doc = release(&c);
        let rom = "Assets/tau/alfatreze.TAU/tau.rom";
        // An owned file the release marks `required: false` (tau-packs.bin and friends).
        let cold = doc.packages[0]
            .layout
            .iter()
            .position(|e| e.role == Role::Owned && e.path != rom)
            .unwrap();
        doc.packages[0].layout[cold].slot = Some(9); // a data-slot file, like tau-packs.bin
        doc.packages[0].layout[cold].required = false;
        let path = doc.packages[0].layout[cold].path.clone();
        fs::remove_file(c.join(&path)).unwrap();
        let r = check_card(&doc, &c, "alfatreze.TAU");
        assert!(
            r.iter()
                .any(|f| !f.error && f.message.contains(&path) && f.message.contains("falls back")),
            "{r:?}"
        );
        assert!(!r.iter().any(|f| f.error && f.message.contains(&path)));
        // A core file without a data slot (core.json, bitstream.rbf_r) stays an error even when the manifest says `required: false`.
        let plain = doc.packages[0]
            .layout
            .iter()
            .position(|e| e.role == Role::Owned && e.slot.is_none() && e.path != rom);
        if let Some(i) = plain {
            doc.packages[0].layout[i].required = false;
            let pp = doc.packages[0].layout[i].path.clone();
            fs::remove_file(c.join(&pp)).ok();
            assert!(
                check_card(&doc, &c, "alfatreze.TAU")
                    .iter()
                    .any(|f| f.error && f.message.contains(&pp)),
                "{pp}"
            );
        }
        // The same file marked required is an error again.
        doc.packages[0].layout[cold].required = true;
        assert!(
            check_card(&doc, &c, "alfatreze.TAU")
                .iter()
                .any(|f| f.error && f.message.contains(&path))
        );
        let _ = fs::remove_dir_all(c);
    }
}
