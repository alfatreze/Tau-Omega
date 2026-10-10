//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

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

pub(super) fn finish_report(core_id: &str, items: Vec<CheckItem>) -> PostInstallReport {
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

pub(super) fn belongs_to_core(path: &str, core_id: &str, platform: &str) -> bool {
    path.starts_with(&format!("Cores/{core_id}/"))
        || path.starts_with(&format!("Assets/{platform}/"))
        || path == format!("Platforms/{platform}.json")
        || path == format!("Platforms/_images/{platform}.bin")
}

pub(super) fn core_json_problems(core_id: &str, json: &Value) -> Vec<String> {
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

pub(super) struct SlotReport {
    pub(super) slots: usize,
    pub(super) problems: Vec<String>,
}

use crate::cardlayout::{SLOT_CORE_SPECIFIC, slot_parameters};

pub(super) fn data_slot_problems(
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

pub(super) fn push_library_check(
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

pub(super) fn collect_junk(dir: &Path, card_root: &Path, out: &mut Vec<String>) {
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
