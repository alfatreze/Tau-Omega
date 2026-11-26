//! Safe card writes for the user meter bin (`Assets/<platform>/common/tau-meters.bin`) and bundled factory presets
//! (`Assets/<platform>/common/tau-presets/<8-hex>/`): plan -> review -> confirm -> verified backup outside the card -> atomic write + `.prev`
//! -> read-back verify -> rollback on error.
//!
//! Rules enforced (Tau Alpha 018-035, `EXTERNAL_METERS_SPEC.md`, `SAFETY_RULES.md`):
//! - Capability guard (A027/3): refuses to write `tau-meters.bin` unless at least one Tau core on the card declares a data slot for it
//!   (Tau v0.7.5+; older cores ignore the file).
//! - Built-in Winamp Bars fallback row (`FLAG_FALLBACK`) can never be hidden or removed.
//! - Refuses any `id_hash` collision with a different `id`.
//! - Flags "same id, different source" when a package replaces an installed meter whose recorded author differs, requiring explicit confirmation.
//! - Keeps `tau-meters.bin.prev` on the card when replacing an existing user bin, plus a SHA-256-verified backup outside the card.
//! - Seeds factory presets only when `tau-presets/<8-hex>/index.tpi` does not already exist (unless `overwrite_presets` is explicitly set).
//! - Never touches `Saves/<platform>/<core>/tau-meter-settings.bin`.

use crate::{
    ErrorCode, TauError,
    core_update::is_tau_core,
    meter_bin::{self, Availability, ComposeEntry, FLAG_FALLBACK, FLAG_VISIBLE},
    meter_package::{self, MeterPackage, PackageSummary},
    preset_files, sync,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const USER_FILE: &str = "tau-meters.bin";
const PREV_FILE: &str = "tau-meters.bin.prev";
const BUNDLE_FILE: &str = "tau-packs.bin";

/// One desired row in an edit-list action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RowEdit {
    pub id: String,
    pub visible: bool,
}

/// What the user wants to do to the card's user meter bin.
#[derive(Debug, Clone)]
pub enum MeterAction {
    /// Reorder, hide/show, or remove non-fallback meters from the current list.
    EditList { rows: Vec<RowEdit> },
    /// Install a new `.tmeter` package or update an already-installed meter with the same id.
    InstallPackage {
        package_bytes: Vec<u8>,
        /// Required when `plan.same_id_different_source` is true.
        allow_source_change: bool,
        /// When false (default), existing `tau-presets/<8-hex>/index.tpi` is preserved.
        overwrite_presets: bool,
    },
    /// Remove `tau-meters.bin` (after verified backup) so the core falls back to its own `tau-packs.bin`.
    ResetToShipped,
}

/// One row of the planned `tau-meters.bin`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlannedRow {
    pub id: String,
    pub id_hash_hex: String,
    pub name: String,
    pub version: String,
    pub visible: bool,
    pub fallback: bool,
    pub binary_only: bool,
    pub tuning_lost: bool,
    pub unavailable: Option<String>,
}

/// What writing the planned change will do. Read-only; nothing has been written yet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeterBinPlan {
    /// Confirmation token bound to the card's current state and the exact planned output.
    pub id: String,
    pub rel_path: String,
    /// `"edit"`, `"install"`, `"update"`, or `"reset"`.
    pub action_kind: String,
    pub resulting_bytes: u64,
    pub max_bytes: u64,
    pub rows: Vec<PlannedRow>,
    pub package: Option<PackageSummary>,
    /// True when the package has the same id as an installed meter whose recorded author differs.
    pub same_id_different_source: bool,
    pub previous_author: Option<String>,
    pub preset_dir_rel: Option<String>,
    pub preset_files_to_write: Vec<String>,
    pub preset_skipped_existing: bool,
    pub backup_files: Vec<String>,
    pub warnings: Vec<String>,
}

/// Result of executing a confirmed [`MeterBinPlan`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeterBinReport {
    pub rel_path: String,
    pub bytes_written: u64,
    pub presets_written: usize,
    pub backup_dir: Option<PathBuf>,
    pub prev_rel_path: Option<String>,
}

struct CardContext {
    platform: String,
    tau_version: Option<u16>,
    user_bin_rel: String,
    user_bin_path: PathBuf,
    seed_bundle_path: Option<PathBuf>,
}

fn bad(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidPathReference, msg)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn json_file(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn declares_file(v: &Value, name: &str) -> bool {
    match v {
        Value::Object(m) => {
            m.get("filename").and_then(Value::as_str) == Some(name)
                || m.values().any(|x| declares_file(x, name))
        }
        Value::Array(a) => a.iter().any(|x| declares_file(x, name)),
        _ => false,
    }
}

fn version_pair(v: &str) -> Option<(u16, u16)> {
    let mut parts = v.split(['.', '-', '+']);
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// Finds the platform and capability of Tau cores on `card`. Refuses if no Tau core declares `tau-meters.bin`.
fn card_context(card: &Path) -> Result<CardContext, TauError> {
    let cores_dir = card.join("Cores");
    let mut core_names: Vec<String> = fs::read_dir(&cores_dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.') && is_tau_core(n))
                .collect()
        })
        .unwrap_or_default();
    core_names.sort();

    if core_names.is_empty() {
        return Err(bad("No Tau core was found on this card."));
    }

    let mut chosen: Option<(String, Option<u16>, PathBuf)> = None;
    for core in &core_names {
        let dir = cores_dir.join(core);
        let reads_user =
            json_file(&dir.join("data.json")).is_some_and(|d| declares_file(&d, USER_FILE));
        if !reads_user {
            continue;
        }
        let meta = json_file(&dir.join("core.json"));
        let meta = meta.as_ref().and_then(|j| j.pointer("/core/metadata"));
        let platform = meta
            .and_then(|m| m.pointer("/platform_ids/0"))
            .and_then(Value::as_str)
            .unwrap_or("tau")
            .to_string();
        let tau_ver = meta
            .and_then(|m| m.get("version"))
            .and_then(Value::as_str)
            .and_then(version_pair)
            .map(|(a, b)| a << 8 | b);
        let bundle = card
            .join("Assets")
            .join(&platform)
            .join(core)
            .join(BUNDLE_FILE);
        if chosen.is_none() || (bundle.is_file() && !chosen.as_ref().unwrap().2.is_file()) {
            chosen = Some((platform, tau_ver, bundle));
        }
    }

    let Some((platform, tau_version, bundle)) = chosen else {
        return Err(bad(
            "No Tau core on this card reads a user meter file yet (update Tau to 0.7.5 or newer first).",
        ));
    };

    let user_bin_rel = format!("Assets/{platform}/common/{USER_FILE}");
    let user_bin_path = card.join(&user_bin_rel);
    Ok(CardContext {
        platform,
        tau_version,
        user_bin_rel,
        user_bin_path,
        seed_bundle_path: bundle.is_file().then_some(bundle),
    })
}

fn load_base_entries(ctx: &CardContext) -> Result<Vec<ComposeEntry>, TauError> {
    if ctx.user_bin_path.is_file() {
        let bytes = sync::read_back_bytes(&ctx.user_bin_path)?;
        if let Ok(entries) = meter_bin::extract_entries(&bytes) {
            return Ok(entries);
        }
    }
    if let Some(bundle) = &ctx.seed_bundle_path {
        let bytes = sync::read_back_bytes(bundle)?;
        return meter_bin::extract_entries(&bytes).map_err(|e| {
            bad(format!(
                "Could not read the core's shipped meter bundle: {}",
                e.plain()
            ))
        });
    }
    Err(bad(
        "No valid v2 meter file or shipped meter bundle was found on this card.",
    ))
}

struct Prepared {
    ctx: CardContext,
    plan: MeterBinPlan,
    composed_bin: Option<Vec<u8>>,
    preset_writes: BTreeMap<String, Vec<u8>>,
    preset_dir_abs: Option<PathBuf>,
}

fn prepare(
    card: &Path,
    action: &MeterAction,
    known_authors: &BTreeMap<String, String>,
) -> Result<Prepared, TauError> {
    let ctx = card_context(card)?;
    let existing_user_bytes = if ctx.user_bin_path.is_file() {
        Some(sync::read_back_bytes(&ctx.user_bin_path)?)
    } else {
        None
    };
    let existing_hash = existing_user_bytes
        .as_deref()
        .map(sha256_hex)
        .unwrap_or_else(|| "none".into());

    let mut backup_files = Vec::new();
    if existing_user_bytes.is_some() {
        backup_files.push(ctx.user_bin_rel.clone());
    }

    if matches!(action, MeterAction::ResetToShipped) {
        if existing_user_bytes.is_none() {
            return Err(bad("There is no user meter file on this card to remove."));
        }
        let token = sha256_hex(
            format!(
                "meter-reset|{}|{}|{}",
                card.display(),
                ctx.user_bin_rel,
                existing_hash
            )
            .as_bytes(),
        );
        let plan = MeterBinPlan {
            id: token[..16].to_string(),
            rel_path: ctx.user_bin_rel.clone(),
            action_kind: "reset".into(),
            resulting_bytes: 0,
            max_bytes: meter_bin::MAX_BIN as u64,
            rows: Vec::new(),
            package: None,
            same_id_different_source: false,
            previous_author: None,
            preset_dir_rel: None,
            preset_files_to_write: Vec::new(),
            preset_skipped_existing: false,
            backup_files,
            warnings: Vec::new(),
        };
        return Ok(Prepared {
            ctx,
            plan,
            composed_bin: None,
            preset_writes: BTreeMap::new(),
            preset_dir_abs: None,
        });
    }

    let base = load_base_entries(&ctx)?;
    let mut warnings = Vec::new();
    let mut package_summary = None;
    let mut same_id_different_source = false;
    let mut previous_author = None;
    let mut preset_dir_rel = None;
    let mut preset_dir_abs = None;
    let mut preset_writes = BTreeMap::new();
    let mut preset_skipped_existing = false;
    let action_kind: String;

    let next_entries: Vec<ComposeEntry> = match action {
        MeterAction::EditList { rows } => {
            action_kind = "edit".into();
            let by_id: BTreeMap<&str, &ComposeEntry> =
                base.iter().map(|e| (e.id.as_str(), e)).collect();
            let mut seen = BTreeSet::new();
            let mut out = Vec::with_capacity(rows.len());
            for req in rows {
                if !seen.insert(req.id.as_str()) {
                    return Err(bad(format!(
                        "Meter '{}' appears twice in the list.",
                        req.id
                    )));
                }
                let Some(orig) = by_id.get(req.id.as_str()) else {
                    return Err(bad(format!(
                        "Meter '{}' is not currently in this card's meter list.",
                        req.id
                    )));
                };
                let mut e = (*orig).clone();
                if e.flags & FLAG_FALLBACK != 0 {
                    if !req.visible {
                        return Err(bad(
                            "Winamp Bars is the built-in fallback meter and cannot be hidden.",
                        ));
                    }
                    e.flags |= FLAG_VISIBLE;
                } else if req.visible {
                    e.flags |= FLAG_VISIBLE;
                } else {
                    e.flags &= !FLAG_VISIBLE;
                }
                out.push(e);
            }
            if !out.iter().any(|e| e.flags & FLAG_FALLBACK != 0) {
                return Err(bad(
                    "Winamp Bars is the built-in fallback meter and cannot be removed.",
                ));
            }
            out
        }
        MeterAction::InstallPackage {
            package_bytes,
            overwrite_presets,
            ..
        } => {
            let pkg: MeterPackage = meter_package::parse(package_bytes)?;
            let mut out = base;
            let new_hash = pkg.summary.id_hash;

            // Refuse id_hash collision with a different 128-bit id.
            if let Some(clash) = out
                .iter()
                .find(|e| meter_bin::id_hash_of_str(&e.id) == new_hash && e.id != pkg.summary.id)
            {
                return Err(bad(format!(
                    "Cannot install '{}': its short id ({}) collides with installed meter '{}'.",
                    pkg.summary.name, pkg.summary.id_hash_hex, clash.name
                )));
            }

            if let Some(existing) = out.iter_mut().find(|e| e.id == pkg.summary.id) {
                action_kind = "update".into();
                if let Some(prev) = known_authors.get(&pkg.summary.id)
                    && prev != &pkg.summary.author
                {
                    same_id_different_source = true;
                    previous_author = Some(prev.clone());
                    warnings.push(format!(
                        "Same id, different source: this meter was previously installed from '{}', but this package says '{}'. Confirm explicitly before replacing.",
                        prev, pkg.summary.author
                    ));
                }
                // Preserve visibility and fallback bit on update.
                let keep_visible = existing.flags & FLAG_VISIBLE;
                let keep_fallback = existing.flags & FLAG_FALLBACK;
                let mut updated = pkg.entry.clone();
                updated.flags = (updated.flags & !(FLAG_VISIBLE | FLAG_FALLBACK))
                    | keep_visible
                    | keep_fallback;
                *existing = updated;
            } else {
                action_kind = "install".into();
                out.push(pkg.entry.clone());
            }

            // Plan factory preset seeding into Assets/<platform>/common/tau-presets/<8-hex>/.
            if !pkg.presets.is_empty() {
                let rel_dir = format!(
                    "Assets/{}/common/{}/{}",
                    ctx.platform,
                    preset_files::DIR,
                    pkg.summary.id_hash_hex
                );
                let abs_dir = card.join(&rel_dir);
                let existing_index = abs_dir.join(preset_files::INDEX_NAME).is_file();
                preset_dir_rel = Some(rel_dir.clone());
                preset_dir_abs = Some(abs_dir.clone());
                if existing_index && !*overwrite_presets {
                    preset_skipped_existing = true;
                } else {
                    for (fname, data) in &pkg.presets {
                        let file_abs = abs_dir.join(fname);
                        if file_abs.is_file() {
                            backup_files.push(format!("{rel_dir}/{fname}"));
                        }
                        preset_writes.insert(fname.clone(), data.clone());
                    }
                }
            }

            package_summary = Some(pkg.summary);
            out
        }
        MeterAction::ResetToShipped => unreachable!(),
    };

    let composed = meter_bin::compose(&next_entries).map_err(|e| {
        bad(format!(
            "Could not compose tau-meters.bin ({}): {}",
            e.refusal.name(),
            e.plain()
        ))
    })?;
    let parsed = meter_bin::parse(&composed).map_err(|e| bad(e.plain()))?;
    let collisions = meter_bin::tuning_collisions(&parsed.rows);
    let tau = ctx.tau_version.unwrap_or(u16::MAX);

    let mut rows = Vec::with_capacity(parsed.rows.len());
    for (r, lost) in parsed.rows.iter().zip(collisions) {
        let unavailable = match meter_bin::availability(r, meter_bin::SUPPORTED_ABI, tau) {
            Availability::Available => None,
            Availability::NeedsOtherAbi => {
                Some("Made for a different version of the Pocket's meter system.".into())
            }
            Availability::NeedsNewerTau => {
                Some("Needs a newer Tau than the one on this card.".into())
            }
        };
        if lost {
            warnings.push(format!(
                "'{}' shares its low 16 id bits with another meter: it will still run, but the Pocket cannot remember its tuned settings.",
                r.name
            ));
        }
        if let Some(reason) = &unavailable {
            warnings.push(format!(
                "'{}' will be greyed out on this card: {reason}",
                r.name
            ));
        }
        rows.push(PlannedRow {
            id: r.id.clone(),
            id_hash_hex: format!("{:08x}", r.id_hash),
            name: r.name.clone(),
            version: format!("{}.{}.{}", r.version.0, r.version.1, r.version.2),
            visible: r.visible(),
            fallback: r.fallback(),
            binary_only: r.binary_only(),
            tuning_lost: lost,
            unavailable,
        });
    }

    let composed_hash = sha256_hex(&composed);
    let mut preset_files_to_write: Vec<String> = preset_writes.keys().cloned().collect();
    preset_files_to_write.sort();
    let token_input = format!(
        "meter-bin|{}|{}|{}|{}|{}|{}",
        card.display(),
        ctx.user_bin_rel,
        existing_hash,
        composed_hash,
        preset_dir_rel.as_deref().unwrap_or(""),
        preset_files_to_write.join(",")
    );
    let token = sha256_hex(token_input.as_bytes());

    let plan = MeterBinPlan {
        id: token[..16].to_string(),
        rel_path: ctx.user_bin_rel.clone(),
        action_kind,
        resulting_bytes: composed.len() as u64,
        max_bytes: meter_bin::MAX_BIN as u64,
        rows,
        package: package_summary,
        same_id_different_source,
        previous_author,
        preset_dir_rel,
        preset_files_to_write,
        preset_skipped_existing,
        backup_files,
        warnings,
    };

    Ok(Prepared {
        ctx,
        plan,
        composed_bin: Some(composed),
        preset_writes,
        preset_dir_abs,
    })
}

/// Plans an edit, `.tmeter` install, or reset of `tau-meters.bin`. Reads the card only; writes nothing.
pub fn plan(
    card: &Path,
    action: &MeterAction,
    known_authors: &BTreeMap<String, String>,
) -> Result<MeterBinPlan, TauError> {
    Ok(prepare(card, action, known_authors)?.plan)
}

fn write_and_verify(path: &Path, bytes: &[u8]) -> Result<(), TauError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let part = path.with_file_name(format!(
        "{}.part",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let expected = sha256_hex(bytes);
    {
        let mut f = fs::File::create(&part)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    let back = sync::read_back_bytes(&part)?;
    if sha256_hex(&back) != expected {
        let _ = fs::remove_file(&part);
        return Err(bad(format!(
            "Read-back verification failed for '{}'.",
            path.display()
        )));
    }
    fs::rename(&part, path)?;
    Ok(())
}

fn backup_one(card: &Path, rel: &str, backup_dir: &Path) -> Result<(), TauError> {
    let src = card.join(rel);
    if !src.is_file() {
        return Ok(());
    }
    let dst = backup_dir.join(rel);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let src_hash = sync::sha256_file(&src)?;
    fs::copy(&src, &dst)?;
    let dst_hash = sync::sha256_file(&dst)?;
    if src_hash != dst_hash {
        return Err(bad(format!(
            "Backup verification failed for '{rel}': nothing on the card was changed."
        )));
    }
    Ok(())
}

fn sweep_appledouble_in(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("._") && e.file_type().is_ok_and(|t| t.is_file()) {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}

/// Executes a reviewed [`MeterBinPlan`]. Verifies the confirmation token, backs up replaced files outside the card,
/// writes atomically (keeping `tau-meters.bin.prev`), reads back, and restores on failure.
pub fn execute(
    card: &Path,
    action: &MeterAction,
    known_authors: &BTreeMap<String, String>,
    confirmation: &str,
    backup_root: &Path,
) -> Result<MeterBinReport, TauError> {
    let prep = prepare(card, action, known_authors)?;
    if prep.plan.id != confirmation {
        return Err(bad(
            "Confirmation token does not match the current card state. Review the plan again.",
        ));
    }
    if prep.plan.same_id_different_source {
        let allowed = matches!(
            action,
            MeterAction::InstallPackage {
                allow_source_change: true,
                ..
            }
        );
        if !allowed {
            return Err(bad(
                "This package has the same id as an installed meter from a different source. Confirm the source change explicitly before installing.",
            ));
        }
    }

    // Verified backup outside the card before any write.
    let backup_dir = if !prep.plan.backup_files.is_empty() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let dir = backup_root.join(format!("meters-{stamp}-{}", prep.plan.id));
        fs::create_dir_all(&dir)?;
        for rel in &prep.plan.backup_files {
            backup_one(card, rel, &dir)?;
        }
        Some(dir)
    } else {
        None
    };

    let bin_path = &prep.ctx.user_bin_path;
    let common_dir = bin_path
        .parent()
        .ok_or_else(|| bad("Invalid user meter bin path."))?;
    let prev_path = common_dir.join(PREV_FILE);
    let prev_rel = format!("Assets/{}/common/{PREV_FILE}", prep.ctx.platform);

    // Handle reset (remove tau-meters.bin after verified backup).
    let Some(composed) = prep.composed_bin else {
        if bin_path.is_file() {
            fs::remove_file(bin_path)?;
        }
        sweep_appledouble_in(common_dir);
        return Ok(MeterBinReport {
            rel_path: prep.ctx.user_bin_rel,
            bytes_written: 0,
            presets_written: 0,
            backup_dir,
            prev_rel_path: None,
        });
    };

    // Atomic write of tau-meters.bin with .prev preservation and full read-back verification.
    fs::create_dir_all(common_dir)?;
    let part_path = common_dir.join(format!("{USER_FILE}.part"));
    let expected_hash = sha256_hex(&composed);
    {
        let mut f = fs::File::create(&part_path)?;
        f.write_all(&composed)?;
        f.sync_all()?;
    }
    let part_back = sync::read_back_bytes(&part_path)?;
    if sha256_hex(&part_back) != expected_hash
        || meter_bin::parse(&part_back).is_err()
        || meter_bin::boot_check(&part_back, part_back.len()).is_err()
    {
        let _ = fs::remove_file(&part_path);
        return Err(bad(
            "Verification of the newly written meter file failed; the previous file was not touched.",
        ));
    }

    let had_existing = bin_path.is_file();
    if had_existing {
        if prev_path.is_file() {
            let _ = fs::remove_file(&prev_path);
        }
        fs::rename(bin_path, &prev_path)?;
    }

    if let Err(e) = fs::rename(&part_path, bin_path) {
        if had_existing && prev_path.is_file() {
            let _ = fs::rename(&prev_path, bin_path);
        }
        let _ = fs::remove_file(&part_path);
        return Err(e.into());
    }

    // Final read-back check of live tau-meters.bin.
    let live_back = sync::read_back_bytes(bin_path)?;
    if sha256_hex(&live_back) != expected_hash {
        if had_existing && prev_path.is_file() {
            let _ = fs::remove_file(bin_path);
            let _ = fs::rename(&prev_path, bin_path);
        }
        return Err(bad(
            "Final read-back of tau-meters.bin failed; restored the previous file.",
        ));
    }
    sweep_appledouble_in(common_dir);

    // Write bundled factory presets (.tpr first, index.tpi last).
    let mut presets_written = 0usize;
    if let Some(pdir) = &prep.preset_dir_abs
        && !prep.preset_writes.is_empty()
    {
        fs::create_dir_all(pdir)?;
        for (fname, data) in &prep.preset_writes {
            if fname != preset_files::INDEX_NAME {
                write_and_verify(&pdir.join(fname), data)?;
                presets_written += 1;
            }
        }
        if let Some(index_data) = prep.preset_writes.get(preset_files::INDEX_NAME) {
            write_and_verify(&pdir.join(preset_files::INDEX_NAME), index_data)?;
            presets_written += 1;
        }
        sweep_appledouble_in(pdir);
    }

    Ok(MeterBinReport {
        rel_path: prep.ctx.user_bin_rel,
        bytes_written: composed.len() as u64,
        presets_written,
        backup_dir,
        prev_rel_path: had_existing.then_some(prev_rel),
    })
}
