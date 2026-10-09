//! Carrying a core's saved settings to the core that replaces it (for example `alfatreze.TAU_DIAGNOSTIC` to
//! `alfatreze.TAU Diagnostics`, whose new id would otherwise start with factory settings).
//!
//! The Pocket keeps a core's persisted settings in `Settings/<core id>/Interact/_core/interact_persist.json`. The file
//! is a list of numbered words whose meaning belongs to the build that wrote it, so it is copied only when the release
//! manifest proves that no persisted id changed meaning between the old build and the new one
//! ([`compat::persist_changed_since`]). Anything else (unknown old build, a changed id, no manifest) is refused with the
//! reason. The old core's file is never touched, an existing file of the new core is never overwritten, and the copy is
//! read back and verified. Plan -> confirm token -> execute, with a rollback that removes only what the run created.

use crate::{ErrorCode, TauError, compat};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

const PERSIST: &str = "Interact/_core/interact_persist.json";

fn settings_rel(core_id: &str) -> String {
    format!("Settings/{core_id}/{PERSIST}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MigrationPlan {
    /// Confirmation token: covers the card, both cores, both paths and the source bytes.
    pub id: String,
    pub card_root: PathBuf,
    pub from_core: String,
    pub to_core: String,
    pub source: String,
    pub dest: String,
    /// Whether the copy may be made. When false, `reasons` says why and nothing can be executed.
    pub allowed: bool,
    pub reasons: Vec<String>,
    /// Persisted ids whose meaning changed (named from the manifest's registry when known).
    pub changed_ids: Vec<(u64, String)>,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MigrationReport {
    pub dest: String,
    pub bytes: u64,
    /// Folders the run created (rollback removes them again if empty).
    pub created_dirs: Vec<String>,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Judges whether `from_core`'s settings may move to `to_core`. Reads only.
pub fn plan(
    card_root: &Path,
    from_core: &str,
    to_core: &str,
    docs: &[compat::CompatDoc],
) -> Result<MigrationPlan, TauError> {
    let (source, dest) = (settings_rel(from_core), settings_rel(to_core));
    let source_bytes = fs::read(card_root.join(&source)).ok();
    let mut reasons = Vec::new();
    let mut changed_ids = Vec::new();
    let mut allowed = true;
    let mut refuse = |why: String, allowed: &mut bool| {
        *allowed = false;
        reasons.push(why);
    };
    if source_bytes.is_none() {
        refuse(
            format!("{from_core} has no saved settings to carry over."),
            &mut allowed,
        );
    }
    if card_root.join(&dest).exists() {
        refuse(
            format!("{to_core} already has saved settings; they are never overwritten."),
            &mut allowed,
        );
    }
    // The old build, identified by hash from the manifests of the releases it came from.
    let from_release = compat::identify_installed(docs, card_root, from_core)
        .into_iter()
        .max_by(|a, b| compat::compare_tags(a, b));
    // The new release: the newest whose package for `to_core` says it replaces `from_core`.
    let to_doc = docs
        .iter()
        .filter(|d| {
            d.packages
                .iter()
                .any(|p| p.core_id == to_core && p.replaces.iter().any(|r| r == from_core))
        })
        .max_by(|a, b| compat::compare_tags(&a.release, &b.release));
    match (&from_release, to_doc) {
        (None, _) => refuse(
            format!(
                "The build of {from_core} on the card is not one Omega knows from a release manifest, so it cannot tell whether its settings still mean the same."
            ),
            &mut allowed,
        ),
        (_, None) => refuse(
            format!("No known release says {to_core} replaces {from_core}."),
            &mut allowed,
        ),
        (Some(from_release), Some(doc)) => match compat::persist_changed_since(doc, from_release) {
            None => refuse(
                format!(
                    "{} does not say which settings changed since {from_release}.",
                    doc.release
                ),
                &mut allowed,
            ),
            Some(ids) if !ids.is_empty() => {
                changed_ids = ids
                    .iter()
                    .map(|id| {
                        (
                            *id,
                            doc.persist_registry
                                .get(id)
                                .map_or_else(|| format!("word {id}"), |m| m.name.clone()),
                        )
                    })
                    .collect();
                refuse(
                    format!(
                        "Settings changed meaning between {from_release} and {} ({}), so the old file would be misread.",
                        doc.release,
                        changed_ids
                            .iter()
                            .map(|(_, n)| n.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    &mut allowed,
                );
            }
            Some(_) => reasons.push(format!(
                "No saved setting changed meaning between {from_release} and {}.",
                doc.release
            )),
        },
    }
    let bytes = source_bytes.as_ref().map_or(0, |b| b.len() as u64);
    let mut hasher = Sha256::new();
    for part in [
        card_root.to_string_lossy().as_ref(),
        from_core,
        to_core,
        &source,
        &dest,
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hasher.update(source_bytes.as_deref().unwrap_or_default());
    Ok(MigrationPlan {
        id: format!("{:x}", hasher.finalize()),
        card_root: card_root.to_path_buf(),
        from_core: from_core.to_string(),
        to_core: to_core.to_string(),
        source,
        dest,
        allowed,
        reasons,
        changed_ids,
        bytes,
    })
}

/// Makes the copy of a reviewed plan. Refuses a plan that is not allowed, a wrong token, a source that changed since
/// the plan, and an existing destination.
pub fn execute(plan: &MigrationPlan, confirmation: &str) -> Result<MigrationReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the plan",
        ));
    }
    if !plan.allowed {
        return Err(TauError::e(
            ErrorCode::InstallRefused,
            plan.reasons.join(" "),
        ));
    }
    let source = fs::read(plan.card_root.join(&plan.source))?;
    let dest = plan.card_root.join(&plan.dest);
    // The token covers the source bytes, so a changed source gives a different plan.
    let fresh = self::plan(&plan.card_root, &plan.from_core, &plan.to_core, &[])?;
    if fresh.id != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "the settings file changed since the plan",
        ));
    }
    if dest.exists() {
        return Err(TauError::e(
            ErrorCode::InstallRefused,
            format!("{} already has saved settings.", plan.to_core),
        ));
    }
    let mut created_dirs = Vec::new();
    let mut parts: Vec<&str> = plan.dest.split('/').collect();
    parts.pop();
    for i in 1..=parts.len() {
        let rel = parts[..i].join("/");
        let abs = plan.card_root.join(&rel);
        if !abs.exists() {
            fs::create_dir(&abs)?;
            created_dirs.push(rel);
        }
    }
    let write = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&dest)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(&source)?;
            f.sync_all()
        });
    let verified = write.is_ok() && fs::read(&dest).is_ok_and(|b| sha(&b) == sha(&source));
    if !verified {
        let _ = fs::remove_file(&dest);
        remove_dirs(&plan.card_root, &created_dirs);
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            "the copy could not be verified and was removed",
        ));
    }
    crate::sync::sweep_appledouble(&plan.card_root.join("Settings").join(&plan.to_core));
    Ok(MigrationReport {
        dest: plan.dest.clone(),
        bytes: plan.bytes,
        created_dirs,
    })
}

fn remove_dirs(card_root: &Path, dirs: &[String]) {
    for rel in dirs.iter().rev() {
        let _ = fs::remove_dir(card_root.join(rel));
    }
}

/// Undoes [`execute`]: removes the copied file and the folders the run created (only if empty). The old core's
/// file is not involved.
pub fn rollback(card_root: &Path, report: &MigrationReport) -> Result<(), TauError> {
    let dest = card_root.join(&report.dest);
    if dest.exists() {
        fs::remove_file(&dest)?;
    }
    if let Some(to_core) = report.dest.split('/').nth(1) {
        crate::sync::sweep_appledouble(&card_root.join("Settings").join(to_core));
    }
    remove_dirs(card_root, &report.created_dirs);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "alfatreze.TAU_DIAGNOSTIC";
    const NEW: &str = "alfatreze.TAU Diagnostics";

    fn card(name: &str) -> PathBuf {
        let c = std::env::temp_dir().join(format!("tau-migrate-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&c);
        fs::create_dir_all(c.join(format!("Cores/{OLD}"))).unwrap();
        fs::write(c.join(format!("Cores/{OLD}/core.json")), b"old core").unwrap();
        fs::create_dir_all(c.join(format!("Settings/{OLD}/Interact/_core"))).unwrap();
        fs::write(
            c.join(format!("Settings/{OLD}/{PERSIST}")),
            b"{\"interact_persist\":{}}",
        )
        .unwrap();
        c
    }

    fn doc(
        release: &str,
        core_id: &str,
        replaces: &[&str],
        owned: Option<(&str, &str)>,
        registry: &str,
    ) -> compat::CompatDoc {
        let layout = owned
            .map(|(path, hash)| serde_json::json!([{"path": path, "role": "owned", "sha256": hash, "slot": null, "required": false}]))
            .unwrap_or_else(|| serde_json::json!([{"path": "Cores/x/core.json", "role": "owned", "sha256": "00", "slot": null, "required": false}]));
        let value = serde_json::json!({
            "schema": 2, "release": release, "date_release": "2026-11-01", "prerelease": false,
            "packages": [{"zip": "x.zip", "zip_sha256": "00", "core_id": core_id, "replaces": replaces,
                "bitstream_sha256": "00", "bitstream_core_version": "4D50331A", "rom_sha256": "00", "cold_sha256": "00",
                "rom_accepts": [], "rom_needs": [], "layout": layout}],
            "persist_registry": serde_json::from_str::<serde_json::Value>(registry).unwrap(),
            "requires_omega": {"min_omega": "0.4.0"}
        });
        compat::parse_compat(&serde_json::to_vec(&value).unwrap()).unwrap()
    }

    fn docs(registry_new: &str) -> Vec<compat::CompatDoc> {
        let hash = sha(b"old core");
        vec![
            doc(
                "v0.6.0",
                OLD,
                &[],
                Some((&format!("Cores/{OLD}/core.json"), &hash)),
                "{}",
            ),
            doc("v0.7.0", NEW, &[OLD], None, registry_new),
        ]
    }

    #[test]
    fn settings_are_copied_when_no_id_changed_meaning_and_can_be_rolled_back() {
        let c = card("ok");
        let docs = docs(r#"{"16": {"name": "EQ", "meaning": 1, "since": "v0.5.0"}}"#);
        let p = plan(&c, OLD, NEW, &docs).unwrap();
        assert!(p.allowed, "{:?}", p.reasons);
        assert!(execute(&p, "wrong").is_err());
        let report = execute(&p, &p.id).unwrap();
        assert_eq!(
            fs::read(c.join(&p.dest)).unwrap(),
            fs::read(c.join(&p.source)).unwrap()
        );
        assert!(
            fs::read_dir(c.join(format!("Settings/{OLD}"))).is_ok(),
            "the old file stays"
        );
        // A second plan sees the destination and refuses to overwrite.
        let again = plan(&c, OLD, NEW, &docs).unwrap();
        assert!(
            !again.allowed
                && again
                    .reasons
                    .iter()
                    .any(|r| r.contains("never overwritten"))
        );
        rollback(&c, &report).unwrap();
        assert!(!c.join(format!("Settings/{NEW}")).exists());
        assert!(c.join(&p.source).is_file());
        fs::remove_dir_all(c).unwrap();
    }

    #[test]
    fn a_changed_id_unknown_build_or_missing_manifest_refuses_with_a_reason() {
        let c = card("refuse");
        let changed =
            docs(r#"{"16": {"name": "Halcyon EQ preset", "meaning": 2, "since": "v0.7.0"}}"#);
        let p = plan(&c, OLD, NEW, &changed).unwrap();
        assert!(!p.allowed);
        assert_eq!(p.changed_ids, vec![(16, "Halcyon EQ preset".to_string())]);
        assert!(
            execute(&p, &p.id).is_err(),
            "a refused plan cannot be executed"
        );
        assert!(!c.join(format!("Settings/{NEW}")).exists());
        // The old build is not identified (its core.json differs from the release's).
        fs::write(c.join(format!("Cores/{OLD}/core.json")), b"hand edited").unwrap();
        let p = plan(&c, OLD, NEW, &docs("{}")).unwrap();
        assert!(!p.allowed && p.reasons.iter().any(|r| r.contains("not one Omega knows")));
        // No manifests at all.
        let p = plan(&c, OLD, NEW, &[]).unwrap();
        assert!(!p.allowed);
        fs::remove_dir_all(c).unwrap();
    }

    #[test]
    fn a_source_changed_after_the_plan_is_refused() {
        let c = card("stale");
        let docs = docs(r#"{"16": {"name": "EQ", "meaning": 1, "since": "v0.5.0"}}"#);
        let p = plan(&c, OLD, NEW, &docs).unwrap();
        assert!(p.allowed, "{:?}", p.reasons);
        fs::write(c.join(&p.source), b"changed since the plan").unwrap();
        assert!(execute(&p, &p.id).is_err());
        assert!(!c.join(&p.dest).exists());
        fs::remove_dir_all(c).unwrap();
    }
}
