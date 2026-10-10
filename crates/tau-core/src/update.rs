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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum UpdateVerdict {
    /// No such core on the card yet.
    NewInstall,
    /// The firmware files on the card are byte-identical to the package's.
    SameBuild,
    /// The package is newer (higher version, or later date).
    Update,
    /// Same version and date but different bytes: two builds of one day, which
    /// cannot be ordered from the package alone.
    SameDateDifferentBuild,
    /// The package is older than what is installed (a downgrade).
    Older,
    /// The package must not be installed over this core; see the reasons.
    Mismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UpdateAssessment {
    pub verdict: UpdateVerdict,
    /// Plain-language reasons (always at least one).
    pub reasons: Vec<String>,
    pub installed: Option<BuildIdentity>,
    pub package: BuildIdentity,
    pub pair: PairStatus,
    /// The release tag the installed files belong to, when a manifest names them.
    pub installed_release: Option<String>,
    /// The release tag of the package, when a manifest names its files.
    pub package_release: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UpdateReport {
    /// One assessment per core in the package (a normal release has one).
    pub cores: Vec<UpdateAssessment>,
    /// The install plan that would carry the update out (not executed).
    pub plan: PackagePlan,
    /// Installed files an update would overwrite: back these up first.
    pub files_replaced: Vec<String>,
    /// User files found on the card that the package does not contain and an
    /// update never touches (library index, theme file).
    pub user_files_kept: Vec<String>,
    /// Persisted settings ids the update may change (the union rule over every release between the installed one and
    /// the package's), when manifests allow knowing; `None` otherwise.
    pub persist_changed: Option<Vec<u64>>,
}

/// Compares what is installed with what a package would install. Never writes.
pub fn assess_update(zip_path: &Path, card_root: &Path) -> Result<UpdateReport, TauError> {
    assess_update_with(zip_path, card_root, &[])
}

/// [`assess_update`] with release manifests (`tau-compat.json`) for the releases the caller knows. Manifests name the
/// installed and packaged builds by release tag (ordering them exactly, where the package alone cannot), supply the
/// bitstream's `CORE_VERSION`, and give the persisted ids an update changes.
pub fn assess_update_with(
    zip_path: &Path,
    card_root: &Path,
    docs: &[CompatDoc],
) -> Result<UpdateReport, TauError> {
    let manifest = package::inspect(zip_path)?;
    let plan = package::plan_install(zip_path, card_root)?;
    if manifest.core_ids.is_empty() {
        return Err(TauError::e(
            ErrorCode::NotFound,
            "this zip has no Cores/<id> folder, so it is not a core release package",
        ));
    }
    let zip_reader = ZipReader::open(zip_path)?;
    let mut cores = Vec::new();
    let mut user_files_kept = Vec::new();
    for core_id in &manifest.core_ids {
        let package_identity =
            identity_from(core_id, |path| zip_reader.read(path))?.ok_or_else(|| {
                TauError::e(
                    ErrorCode::Json,
                    format!("{core_id}: package has no readable core.json"),
                )
            })?;
        let installed = identity_from(core_id, |path| fs::read(card_root.join(path)).ok())?;
        let mut assessment = assess_with(installed, package_identity.clone(), docs);
        // A schema-2 manifest identifies the installed build from every file it owns, not just the firmware.
        let on_card = compat::identify_installed(docs, card_root, core_id);
        if let Some(tag) = on_card.iter().max_by(|a, b| compat::compare_tags(a, b)) {
            assessment.installed_release = Some(tag.clone());
        }
        cores.push(assessment);
        let common = card_root
            .join("Assets")
            .join(&package_identity.media_platform)
            .join("common");
        for name in USER_FILES {
            let in_package = manifest
                .entries
                .iter()
                .any(|entry| entry.path.ends_with(&format!("/{name}")));
            let relative = format!("Assets/{}/common/{name}", package_identity.media_platform);
            if !in_package && common.join(name).is_file() && !user_files_kept.contains(&relative) {
                user_files_kept.push(relative);
            }
        }
    }
    let files_replaced = plan
        .items
        .iter()
        .filter(|item| item.state == DifferenceState::Different)
        .map(|item| item.path.clone())
        .collect();
    let persist_changed = cores.iter().find_map(|core| {
        let installed = core.installed_release.as_deref()?;
        let package = core.package_release.as_deref()?;
        let doc = docs.iter().find(|d| d.release == package)?;
        compat::persist_changed_since(doc, installed)
    });
    Ok(UpdateReport {
        cores,
        plan,
        files_replaced,
        user_files_kept,
        persist_changed,
    })
}

/// The pure comparison behind [`assess_update`], testable on identities alone.
pub fn assess(installed: Option<BuildIdentity>, package: BuildIdentity) -> UpdateAssessment {
    assess_with(installed, package, &[])
}

/// [`assess`] with release manifests: see [`assess_update_with`].
pub fn assess_with(
    installed: Option<BuildIdentity>,
    package: BuildIdentity,
    docs: &[CompatDoc],
) -> UpdateAssessment {
    let pair = pair_status_with(&package, docs);
    let package_release = release_of(&package, docs);
    let installed_release = installed.as_ref().and_then(|old| release_of(old, docs));
    let mut reasons = Vec::new();
    let finish = |verdict, reasons, installed, package, pair| UpdateAssessment {
        verdict,
        reasons,
        installed,
        package,
        pair,
        installed_release: installed_release.clone(),
        package_release: package_release.clone(),
    };

    if let PairStatus::Mismatch { accepts, bitstream } = &pair {
        reasons.push(format!(
            "The firmware in this package accepts core version {} but its bitstream is {bitstream}: \
             the Pocket would show a black screen. Do not install it.",
            accepts.join(", ")
        ));
        return finish(UpdateVerdict::Mismatch, reasons, installed, package, pair);
    }
    if let PairStatus::MissingFeature { missing, .. } = &pair {
        reasons.push(format!(
            "The firmware in this package uses {} but its bitstream does not have it: the Pocket would show \"NO UNIT\". Do not install it.",
            missing.join(", ")
        ));
        return finish(UpdateVerdict::Mismatch, reasons, installed, package, pair);
    }
    match &pair {
        PairStatus::NoMarker => reasons.push(
            "This firmware has no pairing marker (an older build), so its match with the bitstream cannot be checked."
                .into(),
        ),
        PairStatus::CannotVerify => reasons.push(
            "This bitstream is not in Omega's table of known builds, so its match with the firmware cannot be checked."
                .into(),
        ),
        _ => {}
    }

    let Some(old) = installed.clone() else {
        reasons.insert(0, format!("{} is not on this card yet.", package.shortname));
        return finish(UpdateVerdict::NewInstall, reasons, installed, package, pair);
    };
    if old.shortname != package.shortname {
        reasons.insert(
            0,
            format!(
                "The package is {} but the installed core with this id is {}.",
                package.shortname, old.shortname
            ),
        );
        return finish(UpdateVerdict::Mismatch, reasons, installed, package, pair);
    }

    let same_files = same_hash(&old.bitstream_sha256, &package.bitstream_sha256)
        && same_hash(&old.rom_sha256, &package.rom_sha256)
        && same_hash(&old.cold_sha256, &package.cold_sha256);
    if same_files {
        reasons.insert(
            0,
            format!(
                "The firmware files on the card are identical to this package ({} {}).",
                package.version, package.date_release
            ),
        );
        return finish(UpdateVerdict::SameBuild, reasons, installed, package, pair);
    }
    if old.bitstream_sha256.is_none() || old.rom_sha256.is_none() {
        reasons.insert(
            0,
            "The installed core is missing its bitstream or firmware, so this repairs it.".into(),
        );
        return finish(UpdateVerdict::Update, reasons, installed, package, pair);
    }

    // Release tags order two builds exactly; without them the package's version and date are all there is.
    let ordering = match (&package_release, &installed_release) {
        (Some(new), Some(old_tag)) if new != old_tag => compat::compare_tags(new, old_tag),
        // core.json's full SemVer orders same-day pre-release builds (`0.7.0-dev.385` before `.386`).
        _ => full_version_order(&package.version, &old.version)
            .then_with(|| package.date_release.cmp(&old.date_release)),
    };
    let verdict = match ordering {
        std::cmp::Ordering::Greater => UpdateVerdict::Update,
        std::cmp::Ordering::Less => UpdateVerdict::Older,
        std::cmp::Ordering::Equal => UpdateVerdict::SameDateDifferentBuild,
    };
    let label = |tag: &Option<String>, version: &str, date: &str| {
        tag.clone().unwrap_or_else(|| format!("{version} {date}"))
    };
    let (new_label, old_label) = (
        label(&package_release, &package.version, &package.date_release),
        label(&installed_release, &old.version, &old.date_release),
    );
    let line = match verdict {
        UpdateVerdict::Update if package_release.is_some() && installed_release.is_some() => {
            format!("Update: {new_label} replaces {old_label}.")
        }
        UpdateVerdict::Older if package_release.is_some() && installed_release.is_some() => {
            format!(
                "The package ({new_label}) is older than the installed core ({old_label}); installing it is a downgrade."
            )
        }
        UpdateVerdict::Update => format!(
            "Update: {} {} replaces {} {}.",
            package.version, package.date_release, old.version, old.date_release
        ),
        UpdateVerdict::Older => format!(
            "The package ({} {}) is older than the installed core ({} {}); installing it is a downgrade.",
            package.version, package.date_release, old.version, old.date_release
        ),
        _ => format!(
            "Both are {} {} but the files differ: two builds from the same day. \
             The package cannot say which is newer.",
            package.version, package.date_release
        ),
    };
    reasons.insert(0, line);
    reasons.extend(package_notes(&package, docs));
    finish(verdict, reasons, installed, package, pair)
}

/// What the release manifest says about the package's build: the firmware's stamped version and a dirty tree.
fn package_notes(package: &BuildIdentity, docs: &[CompatDoc]) -> Vec<String> {
    let Some((doc, p)) = manifest_package(package, docs) else {
        return Vec::new();
    };
    let mut notes = Vec::new();
    if let Some(version) = &p.rom_version {
        notes.push(format!("Firmware {version}."));
    }
    if doc.source_dirty {
        notes.push(format!(
            "This release was built from a working tree with uncommitted changes{}, so it cannot be reproduced from the repository.",
            doc.source_commit
                .as_ref()
                .map_or(String::new(), |c| format!(" (commit {c})"))
        ));
    }
    notes
}

fn same_hash(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a == b,
        // A file the package does not carry cannot make the build differ.
        (_, None) => true,
        (None, Some(_)) => false,
    }
}

/// SemVer order of two `core.json` versions when both are release-shaped (`X.Y.Z[-suffix]`), else the numeric
/// dotted-prefix comparison.
fn full_version_order(a: &str, b: &str) -> std::cmp::Ordering {
    let (ta, tb) = (format!("v{a}"), format!("v{b}"));
    if compat::is_release_tag(&ta) && compat::is_release_tag(&tb) {
        compat::compare_tags(&ta, &tb)
    } else {
        compare_versions(a, b)
    }
}

/// Numeric dotted-prefix comparison; a pre-release suffix is ignored (the
/// packages carry none), anything unparsable compares as equal.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    fn parts(v: &str) -> Vec<u64> {
        v.split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .next()
            .unwrap_or("")
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect()
    }
    parts(a).cmp(&parts(b))
}

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

// ---------------------------------------------------------------------------
// Post-install check
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CheckItem {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PostInstallReport {
    pub core_id: String,
    pub items: Vec<CheckItem>,
    /// The worst status of any item.
    pub verdict: CheckStatus,
    /// One line for the front end and the saved report.
    pub summary: String,
}

impl PostInstallReport {
    /// A plain-text rendering, suitable for saving next to the install journal.
    pub fn to_text(&self) -> String {
        let mut out = format!(
            "Post-install check for {}\n{}\n\n",
            self.core_id, self.summary
        );
        for item in &self.items {
            let mark = match item.status {
                CheckStatus::Pass => "PASS",
                CheckStatus::Warn => "WARN",
                CheckStatus::Fail => "FAIL",
            };
            out.push_str(&format!("[{mark}] {}: {}\n", item.name, item.detail));
        }
        out
    }
}

/// Checks the installed core at `card_root`. Pass the package zip that was just
/// installed (or `None` to check the card on its own, as after a hand copy).
/// Read-only.
pub fn post_install_check(
    card_root: &Path,
    core_id: &str,
    package_zip: Option<&Path>,
) -> Result<PostInstallReport, TauError> {
    post_install_check_with(card_root, core_id, package_zip, &[])
}

/// [`post_install_check`] with release manifests: pairing uses the manifest's `CORE_VERSION`, and when a manifest
/// names the installed build the whole card is checked against that release's layout ([`compat::check_card`]).
pub fn post_install_check_with(
    card_root: &Path,
    core_id: &str,
    package_zip: Option<&Path>,
    docs: &[CompatDoc],
) -> Result<PostInstallReport, TauError> {
    let mut items = Vec::new();
    let mut push = |name: &str, status, detail: String| {
        items.push(CheckItem {
            name: name.to_string(),
            status,
            detail,
        });
    };

    let core_dir = card_root.join("Cores").join(core_id);
    let core_json_path = core_dir.join("core.json");
    let core_json: Option<Value> = fs::read(&core_json_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let Some(core_json) = core_json else {
        push(
            "core.json",
            CheckStatus::Fail,
            format!("Cores/{core_id}/core.json is missing or is not valid JSON."),
        );
        return Ok(finish_report(core_id, items));
    };
    let identity = identity_from(core_id, |path| fs::read(card_root.join(path)).ok())?
        .expect("core.json was just read");
    let platform = identity.platform.clone();

    // 1. core.json against the Analogue limits.
    let problems = core_json_problems(core_id, &core_json);
    if problems.is_empty() {
        push(
            "core.json",
            CheckStatus::Pass,
            format!(
                "valid; {} {} ({}).",
                identity.shortname, identity.version, identity.date_release
            ),
        );
    } else {
        push("core.json", CheckStatus::Fail, problems.join(" "));
    }

    // 2. Every file of the package that belongs to this core is on the card, byte for byte.
    if let Some(zip) = package_zip {
        let manifest = package::inspect(zip)?;
        let mut missing = Vec::new();
        let mut different = Vec::new();
        let mut checked = 0usize;
        for entry in manifest
            .entries
            .iter()
            .filter(|e| belongs_to_core(&e.path, core_id, &platform))
        {
            checked += 1;
            match fs::read(card_root.join(&entry.path)) {
                Err(_) => missing.push(entry.path.clone()),
                Ok(bytes) if sha256_hex(&bytes) != entry.sha256 => {
                    different.push(entry.path.clone())
                }
                Ok(_) => {}
            }
        }
        if missing.is_empty() && different.is_empty() {
            push(
                "files match package",
                CheckStatus::Pass,
                format!("{checked} files are byte-identical to the package."),
            );
        } else {
            push(
                "files match package",
                CheckStatus::Fail,
                format!(
                    "{} missing ({}), {} different ({}).",
                    missing.len(),
                    missing.join(", "),
                    different.len(),
                    different.join(", ")
                ),
            );
        }
    }

    // 3. Firmware and bitstream are a pair the Pocket accepts.
    match pair_status_with(&identity, docs) {
        PairStatus::Verified { core_version } => push(
            "firmware pairing",
            CheckStatus::Pass,
            format!("the firmware accepts this bitstream (core version {core_version})."),
        ),
        PairStatus::NoMarker => push(
            "firmware pairing",
            CheckStatus::Warn,
            "the firmware has no pairing marker (an older build): its match cannot be checked."
                .into(),
        ),
        PairStatus::CannotVerify => push(
            "firmware pairing",
            CheckStatus::Warn,
            "this bitstream is not in Omega's table of known builds: its match cannot be checked."
                .into(),
        ),
        PairStatus::Mismatch { accepts, bitstream } => push(
            "firmware pairing",
            CheckStatus::Fail,
            format!(
                "the firmware accepts core version {} but the bitstream is {bitstream}: the Pocket would show a black screen.",
                accepts.join(", ")
            ),
        ),
        PairStatus::MissingFeature { missing, .. } => push(
            "firmware pairing",
            CheckStatus::Fail,
            format!(
                "the firmware uses {} but the bitstream does not have it: the Pocket would show \"NO UNIT\".",
                missing.join(", ")
            ),
        ),
    }

    // 4. Declared data slots and their files.
    let media_platform = identity.media_platform.clone();
    let common = card_root
        .join("Assets")
        .join(&media_platform)
        .join("common");
    let core_assets = card_root.join("Assets").join(&platform).join(core_id);
    let commons: Vec<PathBuf> = crate::cardlayout::core_platforms(card_root, core_id)
        .0
        .iter()
        .map(|p| card_root.join("Assets").join(p).join("common"))
        .collect();
    let slot_report = data_slot_problems(&core_dir, &commons, &core_assets);
    match slot_report {
        Err(detail) => push("data slots", CheckStatus::Fail, detail),
        Ok(report) if report.problems.is_empty() => push(
            "data slots",
            CheckStatus::Pass,
            format!(
                "{} slots declared; every required file is present.",
                report.slots
            ),
        ),
        Ok(report) => push("data slots", CheckStatus::Fail, report.problems.join(" ")),
    }

    // 5. The library index, when the core serves one.
    push_library_check(&mut push, &core_dir, &common, &media_platform);

    // 6. Catalog caches must be cleared so the Pocket rescans.
    let stale: Vec<&str> = CATALOG_CACHES
        .iter()
        .copied()
        .filter(|name| card_root.join(name).is_file())
        .collect();
    if stale.is_empty() {
        push(
            "catalog caches",
            CheckStatus::Pass,
            "cleared; the Pocket will rescan its cores on the next start.".into(),
        );
    } else {
        push(
            "catalog caches",
            CheckStatus::Warn,
            format!(
                "{} cache file(s) from before this install are still on the card ({}); the new core may not show until they are removed.",
                stale.len(),
                stale.join(", ")
            ),
        );
    }

    // 7. No `._` stubs or temp files in the trees this install writes.
    let mut junk = Vec::new();
    for root in [core_dir.clone(), card_root.join("Assets").join(&platform)] {
        collect_junk(&root, card_root, &mut junk);
    }
    if junk.is_empty() {
        push(
            "stray files",
            CheckStatus::Pass,
            "no `._` or temp files.".into(),
        );
    } else {
        junk.sort();
        let shown: Vec<_> = junk.iter().take(5).cloned().collect();
        push(
            "stray files",
            CheckStatus::Warn,
            format!("{} stray file(s), e.g. {}.", junk.len(), shown.join(", ")),
        );
    }

    // 8. Against the release manifest that names this build: the layout the release itself defines.
    // (A schema-1 manifest has no layout to check against: it only names the build.)
    if let Some(doc) = release_of(&identity, docs)
        .and_then(|tag| docs.iter().find(|doc| doc.release == tag))
        .filter(|doc| {
            doc.packages
                .iter()
                .any(|p| p.core_id == core_id && !p.layout.is_empty())
        })
    {
        let findings = compat::check_card(doc, card_root, core_id);
        let errors: Vec<_> = findings.iter().filter(|f| f.error).collect();
        let shown = |list: &[&compat::Finding]| {
            list.iter()
                .take(5)
                .map(|f| f.message.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };
        if findings.is_empty() {
            push(
                "release layout",
                CheckStatus::Pass,
                format!("the card matches release {}.", doc.release),
            );
        } else if !errors.is_empty() {
            push(
                "release layout",
                CheckStatus::Fail,
                format!("against release {}: {}", doc.release, shown(&errors)),
            );
        } else {
            let warnings: Vec<_> = findings.iter().collect();
            push(
                "release layout",
                CheckStatus::Warn,
                format!("against release {}: {}", doc.release, shown(&warnings)),
            );
        }
    }

    Ok(finish_report(core_id, items))
}

fn finish_report(core_id: &str, items: Vec<CheckItem>) -> PostInstallReport {
    let verdict = items
        .iter()
        .map(|item| item.status)
        .max()
        .unwrap_or(CheckStatus::Pass);
    let count = |s| items.iter().filter(|i| i.status == s).count();
    let summary = match verdict {
        CheckStatus::Pass => format!("{core_id}: all {} checks passed.", items.len()),
        CheckStatus::Warn => format!(
            "{core_id}: usable, with {} warning(s) to look at.",
            count(CheckStatus::Warn)
        ),
        CheckStatus::Fail => format!(
            "{core_id}: {} check(s) failed; do not rely on this install.",
            count(CheckStatus::Fail)
        ),
    };
    PostInstallReport {
        core_id: core_id.to_string(),
        items,
        verdict,
        summary,
    }
}

fn belongs_to_core(path: &str, core_id: &str, platform: &str) -> bool {
    path.starts_with(&format!("Cores/{core_id}/"))
        || path.starts_with(&format!("Assets/{platform}/"))
        || path == format!("Platforms/{platform}.json")
        || path == format!("Platforms/_images/{platform}.bin")
}

fn core_json_problems(core_id: &str, json: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let meta = json.pointer("/core/metadata");
    if json.pointer("/core/magic").and_then(Value::as_str) != Some("APF_VER_1") {
        problems.push("core.magic is not APF_VER_1.".to_string());
    }
    let Some(meta) = meta else {
        problems.push("core.metadata is missing.".into());
        return problems;
    };
    let text = |key: &str| meta.get(key).and_then(Value::as_str).unwrap_or_default();
    let limit = |key: &str, max: usize, problems: &mut Vec<String>| {
        let value = text(key);
        if value.is_empty() {
            problems.push(format!("{key} is empty."));
        } else if value.len() > max {
            problems.push(format!(
                "{key} is {} characters; the Pocket allows {max}.",
                value.len()
            ));
        }
    };
    limit("shortname", MAX_SHORTNAME, &mut problems);
    limit("author", MAX_AUTHOR, &mut problems);
    limit("version", MAX_VERSION, &mut problems);
    if text("description").len() > MAX_DESCRIPTION {
        problems.push(format!(
            "description is {} characters; the Pocket allows {MAX_DESCRIPTION}.",
            text("description").len()
        ));
    }
    let date = text("date_release");
    let date_ok = date.len() == 10
        && date.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    if !date_ok {
        problems.push("date_release is not YYYY-MM-DD.".into());
    }
    let platforms = meta
        .get("platform_ids")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if platforms == 0 || platforms > MAX_PLATFORM_IDS {
        problems.push(format!(
            "platform_ids has {platforms} entries; the Pocket needs 1 to {MAX_PLATFORM_IDS}."
        ));
    }
    let expected = format!("{}.{}", text("author"), text("shortname"));
    if core_id != expected {
        problems.push(format!(
            "the folder is {core_id} but author.shortname is {expected}; they must match."
        ));
    }
    problems
}

struct SlotReport {
    slots: usize,
    problems: Vec<String>,
}

use crate::cardlayout::{SLOT_CORE_SPECIFIC, slot_parameters};

fn data_slot_problems(
    core_dir: &Path,
    commons: &[PathBuf],
    core_assets: &Path,
) -> Result<SlotReport, String> {
    let bytes =
        fs::read(core_dir.join("data.json")).map_err(|_| "data.json is missing.".to_string())?;
    let json: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("data.json is not valid JSON: {e}."))?;
    let slots = json
        .pointer("/data/data_slots")
        .and_then(Value::as_array)
        .ok_or_else(|| "data.json has no data.data_slots list.".to_string())?;
    let mut problems = Vec::new();
    if slots.len() > MAX_DATA_SLOTS {
        problems.push(format!(
            "{} slots declared; the Pocket allows {MAX_DATA_SLOTS}.",
            slots.len()
        ));
    }
    let mut seen = Vec::new();
    for slot in slots {
        let id = slot.get("id").and_then(Value::as_u64);
        let name = slot.get("name").and_then(Value::as_str).unwrap_or_default();
        if let Some(id) = id {
            if seen.contains(&id) {
                problems.push(format!("slot id {id} is declared twice."));
            }
            seen.push(id);
        } else {
            problems.push(format!("slot \"{name}\" has no id."));
        }
        let filename = slot.get("filename").and_then(Value::as_str);
        if let Some(filename) = filename {
            if filename.len() > MAX_SLOT_FILENAME {
                problems.push(format!(
                    "slot filename \"{filename}\" is longer than {MAX_SLOT_FILENAME}."
                ));
            }
            let required = slot
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let folder = if slot_parameters(slot) & SLOT_CORE_SPECIFIC != 0 {
                core_assets
            } else {
                let index = crate::cardlayout::slot_platform_index(slot);
                commons
                    .get(index)
                    .or_else(|| commons.first())
                    .map_or(core_assets, PathBuf::as_path)
            };
            if required && !folder.join(filename).is_file() && !core_dir.join(filename).is_file() {
                problems.push(format!("required file {filename} is not on the card."));
            }
        }
    }
    Ok(SlotReport {
        slots: slots.len(),
        problems,
    })
}

fn push_library_check(
    push: &mut impl FnMut(&str, CheckStatus, String),
    core_dir: &Path,
    common: &Path,
    platform: &str,
) {
    let library_capable = fs::read(core_dir.join("data.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .is_some_and(|json| crate::slots_have_library(&json));
    if !library_capable {
        push(
            "library index",
            CheckStatus::Pass,
            "this core does not use a library index.".into(),
        );
        return;
    }
    let index_path = common.join("tau-library.tdb");
    let Ok(bytes) = fs::read(&index_path) else {
        push(
            "library index",
            CheckStatus::Warn,
            "no library yet: the core will say \"Library file not found\" until music is synced and an index is built.".into(),
        );
        return;
    };
    let index = match crate::parse(&bytes) {
        Ok(index) => index,
        Err(error) => {
            push(
                "library index",
                CheckStatus::Fail,
                format!("tau-library.tdb is damaged ({error}); rebuild it."),
            );
            return;
        }
    };
    let expected_root = format!("/Assets/{platform}/common/");
    match crate::string_at(&index, index.root) {
        Ok(root) if root == expected_root => {}
        Ok(root) => {
            push(
                "library index",
                CheckStatus::Fail,
                format!(
                    "the index is rooted at {root} but this core reads {expected_root}: tracks will not open. Rebuild it for this core."
                ),
            );
            return;
        }
        Err(error) => {
            push(
                "library index",
                CheckStatus::Fail,
                format!("the index root cannot be read ({error})."),
            );
            return;
        }
    }
    let missing = crate::verify(&bytes, Some(common)).map(|issues| issues.len());
    match missing {
        Ok(0) => push(
            "library index",
            CheckStatus::Pass,
            format!(
                "valid, rooted at this core, {} tracks all present.",
                index.counts.tracks
            ),
        ),
        Ok(count) => push(
            "library index",
            CheckStatus::Warn,
            format!(
                "valid and rooted at this core, but {count} of {} listed tracks are missing on the card.",
                index.counts.tracks
            ),
        ),
        Err(error) => push(
            "library index",
            CheckStatus::Fail,
            format!("the index could not be verified ({error})."),
        ),
    }
}

fn collect_junk(dir: &Path, card_root: &Path, out: &mut Vec<String>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            collect_junk(&path, card_root, out);
        } else if name.starts_with("._")
            || name == ".DS_Store"
            || name.ends_with(".tmp")
            || name.ends_with(".part")
        {
            out.push(
                path.strip_prefix(card_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

#[cfg(test)]
mod tests;
