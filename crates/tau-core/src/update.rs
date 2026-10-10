//! Update semantics for a core release package, and the shared post-install
//! check. Read-only: nothing here writes to a card or a zip.
//!
//! [`assess_update`] answers "what would installing this zip do to this card"
//! in one of six plain verdicts ([`UpdateVerdict`]), comparing the installed
//! core's identity with the package's. A package does not carry the alpha
//! number (`core.json` says `0.6.0` for every alpha, only `date_release`
//! differs), so identity is the version, the date and the SHA-256 of the three
//! firmware files (`bitstream.rbf_r`, `tau.rom`, `tau-cold.bin`).
//!
//! A package never contains the user's data (`tau-library.tdb`,
//! `tau-assets.bin`, media, saves), and [`crate::package::plan_install`] only
//! writes entries the package lists, so an update keeps all of it by
//! construction; the report names the user files it found so the front end
//! can say so.
//!
//! [`post_install_check`] is the executable form of the manual checklist:
//! files match the package, the ROM and bitstream are a pair the Pocket will
//! accept (the ROM carries `TAUFWPAIR:<core versions>;` and
//! `TAUFWNEED:<features>;` in plain text; a bad pair black-screens with no
//! message), `core.json` is valid and inside the Analogue limits, the declared
//! data slots have their files, the library index is valid and rooted at this
//! core, the catalog caches are cleared, and no `._` or temp files are left.

use crate::{
    ErrorCode, TauError,
    compare::DifferenceState,
    compat::{self, CompatDoc, CompatPackage},
    package::{self, PackagePlan},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Bitstreams whose `CORE_VERSION` is known, by SHA-256 of `bitstream.rbf_r`.
/// The bitstream does not carry its version in a readable place, so this table
/// is seeded from shipped releases whose ROM was gated against that bitstream
/// by Tau Alpha's `check_fw_bitstream_pair.py` at release time (the ROM marker
/// of the release lists the same version). An unknown bitstream is reported as
/// "cannot verify", never as a failure.
pub const KNOWN_BITSTREAMS: &[(&str, &str)] = &[
    // v0.6.0 releases of 2026-10-04 and 2026-10-07 (cymo-feed, clut-rtl fit).
    (
        "ed977c7189283a07ce3d653ad6ccce5372370e8e99af5808bc7a64bd0d7f6727",
        "4D50331A",
    ),
];

/// The Analogue limits (`core.json`) checked by the post-install check.
const MAX_SHORTNAME: usize = 31;
const MAX_DESCRIPTION: usize = 63;
const MAX_AUTHOR: usize = 31;
const MAX_VERSION: usize = 31;
const MAX_PLATFORM_IDS: usize = 4;
const MAX_DATA_SLOTS: usize = 32;
const MAX_SLOT_FILENAME: usize = 31;

/// The five catalog caches the Pocket rebuilds on its next scan; stale ones
/// hide a newly installed core.
pub const CATALOG_CACHES: [&str; 5] = [
    "System/core_viewby_platform.bin",
    "System/corelist_cache.bin",
    "System/cores_cache.bin",
    "System/platform_viewby_category.bin",
    "System/platforms_cache.bin",
];

/// User files an update never touches because no package contains them.
const USER_FILES: [&str; 2] = ["tau-library.tdb", "tau-assets.bin"];

/// What identifies one build of a core, read from either a package or a card.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BuildIdentity {
    pub core_id: String,
    pub shortname: String,
    pub version: String,
    pub date_release: String,
    pub platform: String,
    /// Platform whose `common/` holds the library and media (see
    /// [`crate::cardlayout::media_platform`]); equals `platform` for a plain core.
    #[cfg_attr(feature = "serde", serde(default))]
    pub media_platform: String,
    /// Every declared platform id, in order.
    #[cfg_attr(feature = "serde", serde(default))]
    pub platforms: Vec<String>,
    pub bitstream_sha256: Option<String>,
    pub rom_sha256: Option<String>,
    pub cold_sha256: Option<String>,
    /// `CORE_VERSION`s the ROM accepts (`TAUFWPAIR`), `None` for a ROM built
    /// before the marker existed.
    pub rom_accepts: Option<Vec<String>>,
    /// Bitstream features the ROM uses (`TAUFWNEED`), `None` without a marker.
    pub rom_needs: Option<Vec<String>>,
}

/// Whether the ROM and bitstream of one build are a pair the Pocket accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PairStatus {
    /// The bitstream is in [`KNOWN_BITSTREAMS`] and the ROM accepts its version.
    Verified { core_version: String },
    /// The ROM has no `TAUFWPAIR` marker: an old or non-Tau ROM cannot be vouched for.
    NoMarker,
    /// The bitstream is not in the table (or there is no ROM or bitstream to compare).
    CannotVerify,
    /// The bitstream's version is known and the ROM does not accept it: black screen.
    Mismatch {
        accepts: Vec<String>,
        bitstream: String,
    },
    /// The ROM uses bitstream features (`TAUFWNEED`) the release says the bitstream lacks: the unit would answer
    /// "NO UNIT".
    MissingFeature {
        missing: Vec<String>,
        bitstream_has: Vec<String>,
    },
}

/// Pairing from the built-in table only; see [`pair_status_with`].
pub fn pair_status(identity: &BuildIdentity) -> PairStatus {
    pair_status_with(identity, &[])
}

/// The release manifest's package whose firmware files are exactly these (a manifest is the release's own statement,
/// so it outranks [`KNOWN_BITSTREAMS`]).
fn manifest_package<'a>(
    identity: &BuildIdentity,
    docs: &'a [CompatDoc],
) -> Option<(&'a CompatDoc, &'a CompatPackage)> {
    let bitstream = identity.bitstream_sha256.as_deref()?;
    docs.iter().find_map(|doc| {
        doc.packages
            .iter()
            .find(|p| {
                p.core_id == identity.core_id
                    && p.bitstream_sha256 == bitstream
                    && identity.rom_sha256.as_deref() == Some(&p.rom_sha256[..])
                    && identity
                        .cold_sha256
                        .as_deref()
                        .is_none_or(|c| c == p.cold_sha256)
            })
            .map(|p| (doc, p))
    })
}

/// The release tag these firmware files belong to, when a manifest names them (the newest if several do).
pub fn release_of(identity: &BuildIdentity, docs: &[CompatDoc]) -> Option<String> {
    docs.iter()
        .filter(|doc| manifest_package(identity, std::slice::from_ref(doc)).is_some())
        .map(|doc| doc.release.clone())
        .max_by(|a, b| compat::compare_tags(a, b))
}

/// Whether the ROM and bitstream are a pair the Pocket accepts. A release manifest that names these exact files gives
/// the bitstream's `CORE_VERSION` (and vouches for a ROM without a marker); otherwise the built-in table is used.
pub fn pair_status_with(identity: &BuildIdentity, docs: &[CompatDoc]) -> PairStatus {
    let status = pair_version_status(identity, docs);
    // The CORE_VERSION pairing decides first; features are judged only for a pair that otherwise passes, and only
    // when both sides state them (a manifest's `bitstream_features` and the ROM's `TAUFWNEED`).
    if matches!(status, PairStatus::Verified { .. })
        && let (Some(needs), Some((_, package))) =
            (&identity.rom_needs, manifest_package(identity, docs))
        && let Some(has) = &package.bitstream_features
    {
        let missing: Vec<String> = needs.iter().filter(|n| !has.contains(n)).cloned().collect();
        if !missing.is_empty() {
            return PairStatus::MissingFeature {
                missing,
                bitstream_has: has.clone(),
            };
        }
    }
    status
}

fn pair_version_status(identity: &BuildIdentity, docs: &[CompatDoc]) -> PairStatus {
    let (Some(_), Some(bitstream)) = (&identity.rom_sha256, &identity.bitstream_sha256) else {
        return PairStatus::CannotVerify;
    };
    let manifest_version =
        manifest_package(identity, docs).map(|(_, p)| p.bitstream_core_version.clone());
    let Some(accepts) = &identity.rom_accepts else {
        // The release itself says this ROM goes with this bitstream.
        return match manifest_version {
            Some(core_version) => PairStatus::Verified { core_version },
            None => PairStatus::NoMarker,
        };
    };
    let known = manifest_version.or_else(|| {
        KNOWN_BITSTREAMS
            .iter()
            .find(|(hash, _)| hash == bitstream)
            .map(|(_, version)| (*version).to_string())
    });
    match known {
        None => PairStatus::CannotVerify,
        Some(version) if accepts.contains(&version) => PairStatus::Verified {
            core_version: version,
        },
        Some(version) => PairStatus::Mismatch {
            accepts: accepts.clone(),
            bitstream: version,
        },
    }
}

mod assess;
pub use assess::*;
/// The identity of the core installed at `card_root` (`None` when it has no readable `core.json`).
pub fn installed_identity(card_root: &Path, core_id: &str) -> Option<BuildIdentity> {
    identity_from(core_id, |path| fs::read(card_root.join(path)).ok())
        .ok()
        .flatten()
}

/// The identity of a core inside a package zip (`None` when it has no readable `core.json`).
pub fn package_identity(zip_path: &Path, core_id: &str) -> Option<BuildIdentity> {
    let reader = ZipReader::open(zip_path).ok()?;
    identity_from(core_id, |path| reader.read(path))
        .ok()
        .flatten()
}

/// Reads a core's identity through `read`, which maps a card-relative path to
/// its bytes (a zip entry or a card file). `None` when the core has no readable
/// `core.json`.
fn identity_from(
    core_id: &str,
    read: impl Fn(&str) -> Option<Vec<u8>>,
) -> Result<Option<BuildIdentity>, TauError> {
    let Some(core_json) = read(&format!("Cores/{core_id}/core.json")) else {
        return Ok(None);
    };
    let json: Value = serde_json::from_slice(&core_json)
        .map_err(|e| TauError::e(ErrorCode::Json, format!("{core_id}/core.json: {e}")))?;
    let text = |key: &str| {
        json.pointer(&format!("/core/metadata/{key}"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let platform = json
        .pointer("/core/metadata/platform_ids/0")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let media_platform = crate::cardlayout::media_platform(
        &crate::cardlayout::platform_ids(&json),
        read(&format!("Cores/{core_id}/data.json"))
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .as_ref(),
    );
    let hash = |path: String| read(&path).map(|bytes| sha256_hex(&bytes));
    // Build-bound files are core-specific since Tau's layout change (Assets/<platform>/<core>/, data slots 1/4/6 with
    // parameter bit 1); packages and cards made before keep them in common/.
    let build_file = |name: &str| {
        read(&format!("Assets/{platform}/{core_id}/{name}"))
            .or_else(|| read(&format!("Assets/{platform}/common/{name}")))
    };
    let rom = build_file("tau.rom");
    Ok(Some(BuildIdentity {
        core_id: core_id.to_string(),
        shortname: text("shortname"),
        version: text("version"),
        date_release: text("date_release"),
        bitstream_sha256: hash(format!("Cores/{core_id}/bitstream.rbf_r")),
        rom_sha256: rom.as_ref().map(|bytes| sha256_hex(bytes)),
        cold_sha256: build_file("tau-cold.bin").map(|bytes| sha256_hex(&bytes)),
        rom_accepts: rom.as_deref().and_then(rom_pair_marker),
        rom_needs: rom.as_deref().and_then(rom_needs_marker),
        platform,
        media_platform,
        platforms: crate::cardlayout::platform_ids(&json),
    }))
}

/// `TAUFWPAIR:4D50331A,...;` -> the listed versions.
pub fn rom_pair_marker(rom: &[u8]) -> Option<Vec<String>> {
    let body = marker_body(rom, b"TAUFWPAIR:")?;
    let list: Vec<String> = body.split(',').map(str::to_string).collect();
    let valid = !list.is_empty()
        && list.iter().all(|v| {
            v.len() == 8
                && v.bytes()
                    .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
        });
    valid.then_some(list)
}

/// `TAUFWNEED:HALCYON,LPC;` -> the listed features (possibly none).
pub fn rom_needs_marker(rom: &[u8]) -> Option<Vec<String>> {
    let body = marker_body(rom, b"TAUFWNEED:")?;
    let valid = body
        .bytes()
        .all(|b| b.is_ascii_uppercase() || b == b'_' || b == b',');
    valid.then(|| {
        body.split(',')
            .filter(|f| !f.is_empty())
            .map(str::to_string)
            .collect()
    })
}

fn marker_body<'a>(rom: &'a [u8], tag: &[u8]) -> Option<&'a str> {
    let start = rom.windows(tag.len()).position(|w| w == tag)? + tag.len();
    let end = start + rom[start..].iter().take(256).position(|&b| b == b';')?;
    std::str::from_utf8(&rom[start..end]).ok()
}

fn sha256_hex(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

/// A zip opened once for repeated by-name reads (the package module reads every
/// entry; the identity needs only five).
struct ZipReader {
    path: PathBuf,
}

impl ZipReader {
    fn open(path: &Path) -> Result<Self, TauError> {
        fs::File::open(path)?;
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    fn read(&self, name: &str) -> Option<Vec<u8>> {
        use std::io::Read;
        let mut archive = zip::ZipArchive::new(fs::File::open(&self.path).ok()?).ok()?;
        let mut entry = archive.by_name(name).ok()?;
        let mut data = Vec::new();
        entry.read_to_end(&mut data).ok()?;
        Some(data)
    }
}

mod check;
pub use check::*;
#[cfg(test)]
mod tests;
