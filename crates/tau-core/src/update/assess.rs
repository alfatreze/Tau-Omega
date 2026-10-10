//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

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
pub(super) fn package_notes(package: &BuildIdentity, docs: &[CompatDoc]) -> Vec<String> {
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

pub(super) fn same_hash(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a == b,
        // A file the package does not carry cannot make the build differ.
        (_, None) => true,
        (None, Some(_)) => false,
    }
}

/// SemVer order of two `core.json` versions when both are release-shaped (`X.Y.Z[-suffix]`), else the numeric
/// dotted-prefix comparison.
pub(super) fn full_version_order(a: &str, b: &str) -> std::cmp::Ordering {
    let (ta, tb) = (format!("v{a}"), format!("v{b}"));
    if compat::is_release_tag(&ta) && compat::is_release_tag(&tb) {
        compat::compare_tags(&ta, &tb)
    } else {
        compare_versions(a, b)
    }
}

/// Numeric dotted-prefix comparison; a pre-release suffix is ignored (the
/// packages carry none), anything unparsable compares as equal.
pub(super) fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
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
