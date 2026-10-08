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
}

pub fn pair_status(identity: &BuildIdentity) -> PairStatus {
    let (Some(_), Some(bitstream)) = (&identity.rom_sha256, &identity.bitstream_sha256) else {
        return PairStatus::CannotVerify;
    };
    let Some(accepts) = &identity.rom_accepts else {
        return PairStatus::NoMarker;
    };
    match KNOWN_BITSTREAMS.iter().find(|(hash, _)| hash == bitstream) {
        None => PairStatus::CannotVerify,
        Some((_, version)) if accepts.iter().any(|a| a == version) => PairStatus::Verified {
            core_version: (*version).to_string(),
        },
        Some((_, version)) => PairStatus::Mismatch {
            accepts: accepts.clone(),
            bitstream: (*version).to_string(),
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
}

/// Compares what is installed with what a package would install. Never writes.
pub fn assess_update(zip_path: &Path, card_root: &Path) -> Result<UpdateReport, TauError> {
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
        cores.push(assess(installed, package_identity.clone()));
        let common = card_root
            .join("Assets")
            .join(&package_identity.platform)
            .join("common");
        for name in USER_FILES {
            let in_package = manifest
                .entries
                .iter()
                .any(|entry| entry.path.ends_with(&format!("/{name}")));
            let relative = format!("Assets/{}/common/{name}", package_identity.platform);
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
    Ok(UpdateReport {
        cores,
        plan,
        files_replaced,
        user_files_kept,
    })
}

/// The pure comparison behind [`assess_update`], testable on identities alone.
pub fn assess(installed: Option<BuildIdentity>, package: BuildIdentity) -> UpdateAssessment {
    let pair = pair_status(&package);
    let mut reasons = Vec::new();
    let finish = |verdict, reasons, installed, package, pair| UpdateAssessment {
        verdict,
        reasons,
        installed,
        package,
        pair,
    };

    if let PairStatus::Mismatch { accepts, bitstream } = &pair {
        reasons.push(format!(
            "The firmware in this package accepts core version {} but its bitstream is {bitstream}: \
             the Pocket would show a black screen. Do not install it.",
            accepts.join(", ")
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

    let verdict = match compare_versions(&package.version, &old.version)
        .then_with(|| package.date_release.cmp(&old.date_release))
    {
        std::cmp::Ordering::Greater => UpdateVerdict::Update,
        std::cmp::Ordering::Less => UpdateVerdict::Older,
        std::cmp::Ordering::Equal => UpdateVerdict::SameDateDifferentBuild,
    };
    let line = match verdict {
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
    finish(verdict, reasons, installed, package, pair)
}

fn same_hash(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a == b,
        // A file the package does not carry cannot make the build differ.
        (_, None) => true,
        (None, Some(_)) => false,
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
    let hash = |path: String| read(&path).map(|bytes| sha256_hex(&bytes));
    let rom = read(&format!("Assets/{platform}/common/tau.rom"));
    Ok(Some(BuildIdentity {
        core_id: core_id.to_string(),
        shortname: text("shortname"),
        version: text("version"),
        date_release: text("date_release"),
        bitstream_sha256: hash(format!("Cores/{core_id}/bitstream.rbf_r")),
        rom_sha256: rom.as_ref().map(|bytes| sha256_hex(bytes)),
        cold_sha256: hash(format!("Assets/{platform}/common/tau-cold.bin")),
        rom_accepts: rom.as_deref().and_then(rom_pair_marker),
        rom_needs: rom.as_deref().and_then(rom_needs_marker),
        platform,
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
    match pair_status(&identity) {
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
    }

    // 4. Declared data slots and their files.
    let common = card_root.join("Assets").join(&platform).join("common");
    let slot_report = data_slot_problems(&core_dir, &common);
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
    push_library_check(&mut push, &core_dir, &common, &platform);

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

fn data_slot_problems(core_dir: &Path, common: &Path) -> Result<SlotReport, String> {
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
            if required && !common.join(filename).is_file() && !core_dir.join(filename).is_file() {
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
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn pkg(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/packages")
            .join(name)
    }
    fn alpha3() -> PathBuf {
        pkg("alfatreze.TAU_0.6.0_2026-10-04.zip")
    }
    fn alpha4() -> PathBuf {
        pkg("alfatreze.TAU_0.6.0_2026-10-07.zip")
    }
    fn diagnostic() -> PathBuf {
        pkg("alfatreze.TAU_DIAGNOSTIC_0.6.0_2026-10-07.zip")
    }
    fn old_release() -> PathBuf {
        pkg("alfatreze.TAU_0.4.0_2026-09-22.zip")
    }

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tau-update-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                + crate::test_uniq()
        ));
        fs::create_dir_all(root.join("Cores")).unwrap();
        fs::create_dir_all(root.join("Assets")).unwrap();
        root
    }

    fn install(zip: &Path, card: &Path) {
        let plan = package::plan_install(zip, card).unwrap();
        package::execute_install(zip, card, &plan, &plan.id).unwrap();
    }

    fn verdict(zip: &Path, card: &Path) -> UpdateAssessment {
        assess_update(zip, card).unwrap().cores.remove(0)
    }

    #[test]
    fn real_roms_carry_the_pairing_marker() {
        let reader = ZipReader::open(&alpha4()).unwrap();
        let rom = reader.read("Assets/tau/common/tau.rom").unwrap();
        assert_eq!(rom_pair_marker(&rom), Some(vec!["4D50331A".to_string()]));
        // TAUFWNEED may be absent from this ROM; when present it must parse.
        if let Some(needs) = rom_needs_marker(&rom) {
            assert!(
                needs
                    .iter()
                    .all(|f| f.bytes().all(|b| b.is_ascii_uppercase() || b == b'_'))
            );
        }
        let old = ZipReader::open(&old_release()).unwrap();
        let old_rom = old.read("Assets/tau/common/tau.rom").unwrap();
        assert_eq!(
            rom_pair_marker(&old_rom),
            None,
            "the 0.4.0 ROM predates the marker"
        );
    }

    #[test]
    fn marker_parser_rejects_malformed_lists() {
        assert_eq!(
            rom_pair_marker(b"xxTAUFWPAIR:4D50331A,4D50331B;yy"),
            Some(vec!["4D50331A".into(), "4D50331B".into()])
        );
        assert_eq!(
            rom_pair_marker(b"TAUFWPAIR:4d50331a;"),
            None,
            "lower case is not what the firmware writes"
        );
        assert_eq!(rom_pair_marker(b"TAUFWPAIR:4D5033;"), None, "short");
        assert_eq!(
            rom_pair_marker(b"TAUFWPAIR:4D50331A"),
            None,
            "no terminator"
        );
        assert_eq!(rom_needs_marker(b"TAUFWNEED:;"), Some(vec![]));
        assert_eq!(
            rom_needs_marker(b"TAUFWNEED:HALCYON,LPC;"),
            Some(vec!["HALCYON".into(), "LPC".into()])
        );
        assert_eq!(rom_needs_marker(b"TAUFWNEED:halcyon;"), None);
    }

    #[test]
    fn empty_card_is_a_new_install() {
        let card = scratch("new");
        let report = assess_update(&alpha4(), &card).unwrap();
        assert_eq!(report.cores.len(), 1);
        assert_eq!(report.cores[0].verdict, UpdateVerdict::NewInstall);
        assert_eq!(
            report.cores[0].pair,
            PairStatus::Verified {
                core_version: "4D50331A".into()
            }
        );
        assert!(report.files_replaced.is_empty());
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn same_zip_again_is_the_same_build() {
        let card = scratch("same");
        install(&alpha4(), &card);
        assert_eq!(verdict(&alpha4(), &card).verdict, UpdateVerdict::SameBuild);
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn newer_same_day_and_older_packages_are_told_apart() {
        // alpha3 -> alpha4 changes the ROM and the date only (the bitstream is the same).
        let card = scratch("order");
        install(&alpha3(), &card);
        let up = verdict(&alpha4(), &card);
        assert_eq!(up.verdict, UpdateVerdict::Update);
        assert!(
            up.reasons[0].contains("2026-10-07") && up.reasons[0].contains("2026-10-04"),
            "{:?}",
            up.reasons
        );
        let down = {
            install(&alpha4(), &card);
            verdict(&alpha3(), &card)
        };
        assert_eq!(down.verdict, UpdateVerdict::Older);

        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn different_bytes_on_the_same_date_is_ambiguous() {
        let card = scratch("ambiguous");
        install(&alpha4(), &card);
        // Same version and date in core.json, a different firmware file.
        let rom = card.join("Assets/tau/common/tau.rom");
        let mut bytes = fs::read(&rom).unwrap();
        *bytes.last_mut().unwrap() ^= 0xFF;
        fs::write(&rom, bytes).unwrap();
        let assessment = verdict(&alpha4(), &card);
        assert_eq!(assessment.verdict, UpdateVerdict::SameDateDifferentBuild);
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn the_diagnostic_build_is_not_an_update_of_the_normal_core() {
        let card = scratch("diag");
        install(&alpha4(), &card);
        // A different core id: its own new install, and the normal core is untouched.
        let assessment = verdict(&diagnostic(), &card);
        assert_eq!(assessment.verdict, UpdateVerdict::NewInstall);
        assert_eq!(assessment.package.core_id, "alfatreze.TAU_DIAGNOSTIC");
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_core_with_the_wrong_name_in_the_same_folder_is_a_mismatch() {
        let card = scratch("name");
        install(&alpha4(), &card);
        let core_json = card.join("Cores/alfatreze.TAU/core.json");
        let text = fs::read_to_string(&core_json)
            .unwrap()
            .replace("\"shortname\": \"TAU\"", "\"shortname\": \"OTHER\"");
        fs::write(&core_json, text).unwrap();
        assert_eq!(verdict(&alpha4(), &card).verdict, UpdateVerdict::Mismatch);
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn identity_without_a_marker_is_reported_not_refused() {
        let card = scratch("old");
        let report = assess_update(&old_release(), &card).unwrap();
        let core = &report.cores[0];
        assert_eq!(core.verdict, UpdateVerdict::NewInstall);
        assert_eq!(core.pair, PairStatus::NoMarker);
        assert!(
            core.reasons.iter().any(|r| r.contains("no pairing marker")),
            "{:?}",
            core.reasons
        );
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_pair_the_pocket_would_refuse_is_a_mismatch() {
        let mut package = identity_from("alfatreze.TAU", |p| {
            ZipReader::open(&alpha4()).unwrap().read(p)
        })
        .unwrap()
        .unwrap();
        package.rom_accepts = Some(vec!["4D503317".into()]); // what alpha.1 shipped with
        let assessment = assess(None, package);
        assert_eq!(assessment.verdict, UpdateVerdict::Mismatch);
        assert!(assessment.reasons[0].contains("black screen"));
    }

    #[test]
    fn an_update_names_what_it_replaces_and_what_it_keeps() {
        let card = scratch("keeps");
        install(&alpha3(), &card);
        let common = card.join("Assets/tau/common");
        fs::write(common.join("tau-library.tdb"), b"library").unwrap();
        fs::write(common.join("tau-assets.bin"), b"themes").unwrap();
        fs::write(common.join("song.mp3"), b"music").unwrap();
        let report = assess_update(&alpha4(), &card).unwrap();
        assert_eq!(report.cores[0].verdict, UpdateVerdict::Update);
        assert!(
            report
                .files_replaced
                .contains(&"Assets/tau/common/tau.rom".to_string())
        );
        assert!(
            report
                .files_replaced
                .contains(&"Cores/alfatreze.TAU/core.json".to_string())
        );
        assert!(
            !report
                .files_replaced
                .iter()
                .any(|p| p.contains("bitstream")),
            "the bitstream is identical"
        );
        assert_eq!(
            report.user_files_kept,
            vec![
                "Assets/tau/common/tau-library.tdb",
                "Assets/tau/common/tau-assets.bin"
            ]
        );
        // Carrying the update out leaves the user's files byte-for-byte alone.
        install(&alpha4(), &card);
        assert_eq!(
            fs::read(common.join("tau-library.tdb")).unwrap(),
            b"library"
        );
        assert_eq!(fs::read(common.join("tau-assets.bin")).unwrap(), b"themes");
        assert_eq!(fs::read(common.join("song.mp3")).unwrap(), b"music");
        assert_eq!(verdict(&alpha4(), &card).verdict, UpdateVerdict::SameBuild);
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_zip_without_a_core_is_not_a_package() {
        let dir = scratch("notpkg");
        let zip_path = dir.join("x.zip");
        {
            use std::io::Write;
            let mut writer = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
            writer
                .start_file("readme.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"hi").unwrap();
            writer.finish().unwrap();
        }
        assert!(assess_update(&zip_path, &dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn version_order_is_numeric() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_versions("0.10.0", "0.9.0"), Greater);
        assert_eq!(compare_versions("0.6.0", "0.6.0"), Equal);
        assert_eq!(compare_versions("0.5.0", "0.6.0"), Less);
        assert_eq!(compare_versions("0.6.0-alpha.4", "0.6.0"), Equal);
    }

    // ---- post-install check ----

    fn find<'a>(report: &'a PostInstallReport, name: &str) -> &'a CheckItem {
        report
            .items
            .iter()
            .find(|i| i.name == name)
            .unwrap_or_else(|| panic!("no item {name}"))
    }

    #[test]
    fn a_fresh_install_passes_with_only_the_expected_library_warning() {
        let card = scratch("check-fresh");
        install(&alpha4(), &card);
        let report = post_install_check(&card, "alfatreze.TAU", Some(&alpha4())).unwrap();
        for name in [
            "core.json",
            "files match package",
            "firmware pairing",
            "data slots",
            "catalog caches",
            "stray files",
        ] {
            assert_eq!(
                find(&report, name).status,
                CheckStatus::Pass,
                "{name}: {}",
                find(&report, name).detail
            );
        }
        let library = find(&report, "library index");
        assert_eq!(library.status, CheckStatus::Warn);
        assert!(library.detail.contains("Library file not found"));
        assert_eq!(report.verdict, CheckStatus::Warn);
        assert!(report.to_text().contains("[WARN] library index"));
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_changed_file_stale_caches_and_stubs_are_all_caught() {
        let card = scratch("check-bad");
        install(&alpha4(), &card);
        fs::write(card.join("Assets/tau/common/tau-cold.bin"), b"half written").unwrap();
        fs::create_dir_all(card.join("System")).unwrap();
        fs::write(card.join("System/cores_cache.bin"), b"old").unwrap();
        fs::write(card.join("Cores/alfatreze.TAU/._core.json"), b"stub").unwrap();
        let report = post_install_check(&card, "alfatreze.TAU", Some(&alpha4())).unwrap();
        assert_eq!(report.verdict, CheckStatus::Fail);
        assert!(
            find(&report, "files match package")
                .detail
                .contains("tau-cold.bin")
        );
        assert_eq!(find(&report, "catalog caches").status, CheckStatus::Warn);
        assert!(find(&report, "stray files").detail.contains("._core.json"));
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_missing_required_firmware_file_fails_the_slot_check() {
        let card = scratch("check-slot");
        install(&alpha4(), &card);
        fs::remove_file(card.join("Assets/tau/common/tau.rom")).unwrap();
        let report = post_install_check(&card, "alfatreze.TAU", None).unwrap();
        let slots = find(&report, "data slots");
        assert_eq!(slots.status, CheckStatus::Fail);
        assert!(slots.detail.contains("tau.rom"), "{}", slots.detail);
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_library_rooted_at_another_core_fails() {
        let card = scratch("check-root");
        install(&alpha4(), &card);
        let common = card.join("Assets/tau/common");
        let entries = crate::synth(5, 2, 2);
        let index =
            crate::build_index(&entries, &[], "/Assets/tau_dev_67/common/", &mut Vec::new())
                .unwrap();
        fs::write(common.join("tau-library.tdb"), &index).unwrap();
        let report = post_install_check(&card, "alfatreze.TAU", None).unwrap();
        let library = find(&report, "library index");
        assert_eq!(library.status, CheckStatus::Fail, "{}", library.detail);
        assert!(library.detail.contains("/Assets/tau_dev_67/common/"));
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_library_built_for_this_core_passes() {
        let card = scratch("check-lib");
        install(&alpha4(), &card);
        let common = card.join("Assets/tau/common");
        let entries = crate::synth(20, 4, 3);
        let index = crate::build_index(
            &entries,
            &[],
            &crate::root_prefix(&common).unwrap(),
            &mut Vec::new(),
        )
        .unwrap();
        fs::write(common.join("tau-library.tdb"), &index).unwrap();
        let report = post_install_check(&card, "alfatreze.TAU", None).unwrap();
        let library = find(&report, "library index");
        // The synthetic tracks have no files on this card: rooted correctly, but missing media.
        assert_eq!(library.status, CheckStatus::Warn, "{}", library.detail);
        assert!(
            library.detail.contains("rooted at this core") && library.detail.contains("missing")
        );
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_core_json_over_the_limits_fails() {
        let card = scratch("check-limits");
        install(&alpha4(), &card);
        let path = card.join("Cores/alfatreze.TAU/core.json");
        let long = "x".repeat(70);
        let text = fs::read_to_string(&path)
            .unwrap()
            .replace("TAU Music Player", &long);
        fs::write(&path, text).unwrap();
        let report = post_install_check(&card, "alfatreze.TAU", None).unwrap();
        let item = find(&report, "core.json");
        assert_eq!(item.status, CheckStatus::Fail);
        assert!(
            item.detail.contains("description is 70 characters"),
            "{}",
            item.detail
        );
        fs::remove_dir_all(card).unwrap();
    }

    #[test]
    fn a_missing_core_is_one_failed_item_not_an_error() {
        let card = scratch("check-none");
        let report = post_install_check(&card, "alfatreze.TAU", None).unwrap();
        assert_eq!(report.verdict, CheckStatus::Fail);
        assert_eq!(report.items.len(), 1);
        fs::remove_dir_all(card).unwrap();
    }
}
