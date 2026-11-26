//! External meter packages (`<id>.tmeter`, a zip archive): reading and vetting one. Pure bytes, no disk I/O or network.
//!
//! Enforces every rule of Tau Alpha's `EXTERNAL_METERS_SPEC.md` section 1 (R-021/9):
//! - Zip size cap: 4 MiB ([`MAX_PACKAGE_BYTES`]).
//! - Path safety: no `..`, no absolute paths, no backslashes, no duplicate entries.
//! - Allowed paths only: `meter.json`, `meter.bin`, `LICENSE`, `README.md`, `preview.png`, `presets/index.tpi`, `presets/p<n>.tpr`, and `source/...`.
//! - Required files: `meter.json`, `meter.bin`, `LICENSE`.
//! - Every file in the zip (except `meter.json`) must be pinned in `meter.json`'s `files` map with a matching SHA-256, and every pinned file must exist.
//! - `meter.json` fields (`schema == 1`, 32-char lowercase hex `id`, 1–23 printable ASCII `name`, semver `version`, `abi` matching `meter.bin`'s `TMPK` header,
//!   optional `tau_min`, `params_rev`, `author`, `licence`), plus full `TMPK` pack validation and preset validation when `presets/` is present.

use crate::{
    ErrorCode, TauError,
    meter_bin::{self, ComposeEntry, FLAG_BINARY_ONLY, FLAG_VISIBLE},
    preset_files,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
};

/// Maximum allowed `.tmeter` archive size (4 MiB, R-021/9).
pub const MAX_PACKAGE_BYTES: usize = 4 * 1024 * 1024;
/// Maximum total uncompressed size across all files in a `.tmeter` (16 MiB zip-bomb guard).
const MAX_UNCOMPRESSED_BYTES: u64 = 16 * 1024 * 1024;

/// Plain-language warning shown before every external meter install and update (D-M22).
pub const FULL_ACCESS_NOTICE: &str = "A meter runs with full access to the player.";
/// Trust label for any meter installed directly from a file (not from a signed verified catalogue).
pub const INDEPENDENT_TRUST_LABEL: &str = "Independent: not reviewed";

/// Human-facing review information for a vetted `.tmeter` package (safe to pass to UI/CLI).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PackageSummary {
    pub id: String,
    pub id_hash: u32,
    /// Lower-case 8-hex folder name (`tau-presets/<id_hash_hex>/`).
    pub id_hash_hex: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub licence: String,
    pub homepage: String,
    pub abi: String,
    pub tau_min: String,
    pub params_rev: u16,
    pub binary_only: bool,
    pub preset_count: usize,
    pub package_sha256: String,
    pub pack_sha256: String,
    pub pack_bytes: usize,
    pub readme: Option<String>,
    pub license_text: String,
    pub trust_label: String,
    pub warning_notice: String,
}

/// A fully validated `.tmeter` package, ready to display or compose into `tau-meters.bin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeterPackage {
    pub summary: PackageSummary,
    pub entry: ComposeEntry,
    /// Factory preset files (`index.tpi`, `p0.tpr`, ...) keyed by bare file name. Empty when the package ships no presets.
    pub presets: BTreeMap<String, Vec<u8>>,
}

fn bad(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidPathReference, msg)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_safe_path(name: &str) -> bool {
    if name.is_empty()
        || name.starts_with('/')
        || name.ends_with('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        return false;
    }
    name.split('/')
        .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

fn is_preset_file_name(name: &str) -> bool {
    name == preset_files::INDEX_NAME
        || name
            .strip_prefix('p')
            .and_then(|n| n.strip_suffix(".tpr"))
            .is_some_and(|n| !n.is_empty() && n.len() <= 3 && n.bytes().all(|b| b.is_ascii_digit()))
}

fn is_allowed_path(name: &str) -> bool {
    match name {
        "meter.json" | "meter.bin" | "LICENSE" | "README.md" | "preview.png" => true,
        _ if name.starts_with("source/") => name.len() > "source/".len(),
        _ if name.starts_with("presets/") => {
            let rest = &name["presets/".len()..];
            !rest.contains('/') && is_preset_file_name(rest)
        }
        _ => false,
    }
}

fn parse_semver(s: &str) -> Result<(u16, u16, u16), TauError> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 {
        return Err(bad(format!(
            "Meter version '{s}' is not major.minor.patch."
        )));
    }
    let a = parts[0]
        .parse::<u16>()
        .map_err(|_| bad(format!("Meter version '{s}' is invalid.")))?;
    let b = parts[1]
        .parse::<u16>()
        .map_err(|_| bad(format!("Meter version '{s}' is invalid.")))?;
    let c = parts[2]
        .parse::<u16>()
        .map_err(|_| bad(format!("Meter version '{s}' is invalid.")))?;
    Ok((a, b, c))
}

fn parse_abi(s: &str) -> Result<u16, TauError> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 2 {
        return Err(bad(format!("Meter ABI '{s}' must be major.minor.")));
    }
    let major = parts[0]
        .parse::<u8>()
        .map_err(|_| bad(format!("Meter ABI '{s}' is invalid.")))?;
    let minor = parts[1]
        .parse::<u8>()
        .map_err(|_| bad(format!("Meter ABI '{s}' is invalid.")))?;
    Ok(u16::from(major) << 8 | u16::from(minor))
}

fn parse_tau_min(s: &str) -> Result<u16, TauError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(0);
    }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return Err(bad(format!("Minimum Tau version '{s}' is invalid.")));
    }
    let major = parts[0]
        .parse::<u8>()
        .map_err(|_| bad(format!("Minimum Tau version '{s}' is invalid.")))?;
    let minor = parts[1]
        .parse::<u8>()
        .map_err(|_| bad(format!("Minimum Tau version '{s}' is invalid.")))?;
    if parts.len() == 3 {
        let _ = parts[2]
            .parse::<u16>()
            .map_err(|_| bad(format!("Minimum Tau version '{s}' is invalid.")))?;
    }
    Ok(u16::from(major) << 8 | u16::from(minor))
}

fn req_str<'a>(m: &'a Value, key: &str) -> Result<&'a str, TauError> {
    m.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad(format!("meter.json is missing '{key}'.")))
}

/// Parses and validates a `.tmeter` archive from raw bytes.
pub fn parse(bytes: &[u8]) -> Result<MeterPackage, TauError> {
    if bytes.is_empty() {
        return Err(bad("The meter package is empty."));
    }
    if bytes.len() > MAX_PACKAGE_BYTES {
        return Err(bad(format!(
            "The meter package is too large ({} bytes; the limit is 4 MiB).",
            bytes.len()
        )));
    }
    let package_sha256 = sha256_hex(bytes);
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| bad("This file is not a valid meter package (.tmeter zip)."))?;

    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut total_uncompressed = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|_| bad("Could not read an entry inside the meter package."))?;
        let raw_name = entry.name().to_string();
        if entry.is_dir() {
            // Allow directory entries as long as they don't escape.
            let trimmed = raw_name.trim_end_matches('/');
            if !trimmed.is_empty() && !is_safe_path(trimmed) {
                return Err(bad(format!(
                    "The meter package contains an unsafe path: {raw_name}"
                )));
            }
            continue;
        }
        if !is_safe_path(&raw_name) {
            return Err(bad(format!(
                "The meter package contains an unsafe path: {raw_name}"
            )));
        }
        if !is_allowed_path(&raw_name) {
            return Err(bad(format!(
                "The meter package contains an unexpected file: {raw_name}"
            )));
        }
        if files.contains_key(&raw_name) {
            return Err(bad(format!(
                "The meter package lists '{raw_name}' more than once."
            )));
        }
        total_uncompressed = total_uncompressed.saturating_add(entry.size());
        if total_uncompressed > MAX_UNCOMPRESSED_BYTES {
            return Err(bad(
                "The meter package expands beyond the 16 MiB safety limit.",
            ));
        }
        let mut buf = Vec::with_capacity(entry.size().min(MAX_PACKAGE_BYTES as u64) as usize);
        entry
            .by_ref()
            .take(MAX_UNCOMPRESSED_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(|_| {
                bad(format!(
                    "Could not read '{raw_name}' from the meter package."
                ))
            })?;
        if buf.len() as u64 > MAX_UNCOMPRESSED_BYTES {
            return Err(bad(
                "The meter package expands beyond the 16 MiB safety limit.",
            ));
        }
        files.insert(raw_name, buf);
    }

    let manifest_bytes = files
        .get("meter.json")
        .ok_or_else(|| bad("The meter package has no meter.json."))?;
    let pack = files
        .get("meter.bin")
        .ok_or_else(|| bad("The meter package has no meter.bin."))?
        .clone();
    let license_bytes = files
        .get("LICENSE")
        .ok_or_else(|| bad("The meter package has no LICENSE file."))?;

    let manifest: Value =
        serde_json::from_slice(manifest_bytes).map_err(|_| bad("meter.json is not valid JSON."))?;
    if manifest.get("schema").and_then(Value::as_u64) != Some(1) {
        return Err(bad(
            "meter.json uses a schema version this Omega does not read.",
        ));
    }

    let id = req_str(&manifest, "id")?.to_string();
    if !meter_bin::valid_id_str(&id) {
        return Err(bad(
            "meter.json has an invalid id (expected 32 lower-case hex digits).",
        ));
    }
    let id_hash = meter_bin::id_hash_of_str(&id);
    let id_hash_hex = format!("{id_hash:08x}");

    let name = req_str(&manifest, "name")?.to_string();
    if !meter_bin::valid_name_str(&name) {
        return Err(bad(
            "meter.json has an invalid name (1 to 23 printable ASCII characters).",
        ));
    }

    let version_str = req_str(&manifest, "version")?.to_string();
    let version = parse_semver(&version_str)?;
    let author = req_str(&manifest, "author")?.to_string();
    let licence = req_str(&manifest, "licence")?.to_string();
    let homepage = manifest
        .get("homepage")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let abi_str = req_str(&manifest, "abi")?.to_string();
    let abi = parse_abi(&abi_str)?;
    let tau_min_str = manifest
        .get("tau_min")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let tau_min = parse_tau_min(&tau_min_str)?;
    let params_rev_u64 = manifest
        .get("params_rev")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let params_rev = u16::try_from(params_rev_u64)
        .map_err(|_| bad("meter.json has a params_rev outside 0..65535."))?;

    // Pin verification: every file in the zip (except meter.json) must be pinned, and every pin must match.
    let pins = manifest
        .get("files")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("meter.json is missing the 'files' checksum table."))?;

    let mut pinned_paths = BTreeSet::new();
    for (rel, val) in pins {
        if !is_safe_path(rel) || !is_allowed_path(rel) || rel == "meter.json" {
            return Err(bad(format!("meter.json pins an invalid path: {rel}")));
        }
        let expected = val
            .as_str()
            .filter(|s| s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
            .ok_or_else(|| {
                bad(format!(
                    "meter.json has an invalid SHA-256 pin for '{rel}'."
                ))
            })?;
        let actual_bytes = files.get(rel).ok_or_else(|| {
            bad(format!(
                "meter.json pins '{rel}', but that file is missing from the package."
            ))
        })?;
        let actual = sha256_hex(actual_bytes);
        if actual != expected {
            return Err(bad(format!(
                "'{rel}' does not match its checksum in meter.json."
            )));
        }
        pinned_paths.insert(rel.clone());
    }
    for rel in files.keys() {
        if rel != "meter.json" && !pinned_paths.contains(rel) {
            return Err(bad(format!(
                "'{rel}' is inside the package but is not listed in meter.json 'files'."
            )));
        }
    }

    // Validate TMPK v2 pack and check ABI agreement.
    let vp = meter_bin::validate_pack(&pack, None).map_err(|e| {
        bad(format!(
            "meter.bin failed validation ({}): {}",
            e.refusal.name(),
            e.plain()
        ))
    })?;
    if vp.header.abi != abi {
        return Err(bad(format!(
            "meter.json says ABI {abi_str}, but meter.bin was built for {}.{}.",
            vp.header.abi >> 8,
            vp.header.abi & 0xFF
        )));
    }
    let pack_sha256 = sha256_hex(&pack);

    // Validate bundled presets, if any.
    let mut presets = BTreeMap::new();
    for (rel, data) in &files {
        if let Some(bare) = rel.strip_prefix("presets/") {
            presets.insert(bare.to_string(), data.clone());
        }
    }
    let preset_count = validate_presets(&presets, (id_hash & 0xFFFF) as u16)?;

    let has_source = files.keys().any(|k| k.starts_with("source/"))
        || manifest
            .get("source")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.trim().is_empty());
    let binary_only = !has_source;
    let flags = FLAG_VISIBLE | if binary_only { FLAG_BINARY_ONLY } else { 0 };

    let readme = files
        .get("README.md")
        .map(|b| String::from_utf8_lossy(b).into_owned());
    let license_text = String::from_utf8_lossy(license_bytes).into_owned();

    let entry = ComposeEntry {
        id: id.clone(),
        name: name.clone(),
        version,
        pack,
        tau_min,
        flags,
        params_rev,
    };

    let summary = PackageSummary {
        id,
        id_hash,
        id_hash_hex,
        name,
        version: version_str,
        author,
        licence,
        homepage,
        abi: abi_str,
        tau_min: tau_min_str,
        params_rev,
        binary_only,
        preset_count,
        package_sha256,
        pack_sha256,
        pack_bytes: entry.pack.len(),
        readme,
        license_text,
        trust_label: INDEPENDENT_TRUST_LABEL.into(),
        warning_notice: FULL_ACCESS_NOTICE.into(),
    };

    Ok(MeterPackage {
        summary,
        entry,
        presets,
    })
}

fn validate_presets(
    presets: &BTreeMap<String, Vec<u8>>,
    expected_mid: u16,
) -> Result<usize, TauError> {
    if presets.is_empty() {
        return Ok(0);
    }
    let index = presets.get(preset_files::INDEX_NAME).ok_or_else(|| {
        bad("The package has preset files under presets/ but no presets/index.tpi.")
    })?;
    let (idx_mid, entries) = preset_files::parse_index(index)
        .map_err(|e| bad(format!("presets/index.tpi is invalid: {}", e.message)))?;
    if idx_mid != expected_mid {
        return Err(bad(format!(
            "presets/index.tpi is for meter 0x{idx_mid:04x}, not 0x{expected_mid:04x}."
        )));
    }
    let mut expected_files = BTreeSet::new();
    expected_files.insert(preset_files::INDEX_NAME.to_string());
    for e in &entries {
        let fname = preset_files::preset_file_name(e.id);
        let raw = presets.get(&fname).ok_or_else(|| {
            bad(format!(
                "presets/index.tpi lists '{fname}', which is missing from the package."
            ))
        })?;
        let parsed = preset_files::parse_preset(raw)
            .map_err(|err| bad(format!("presets/{fname} is invalid: {}", err.message)))?;
        if parsed.id != e.id || parsed.meter_id != expected_mid {
            return Err(bad(format!(
                "presets/{fname} does not match its entry in presets/index.tpi."
            )));
        }
        expected_files.insert(fname);
    }
    for name in presets.keys() {
        if !expected_files.contains(name) {
            return Err(bad(format!(
                "presets/{name} is not listed in presets/index.tpi."
            )));
        }
    }
    Ok(entries.len())
}
