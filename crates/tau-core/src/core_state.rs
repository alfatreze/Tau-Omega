//! Disabling a core so the Pocket does not list it, and enabling it again (`docs/CORE_DISABLE_SPEC.md`). A disabled core is **moved, not copied or
//! changed**: `Cores/<id>` becomes `Disabled Cores/<id>` on the same card (one rename: instant, atomic, nothing rewritten). Its saves, settings, presets,
//! assets and platform files stay where they are, so enabling it again restores exactly what was there. The Pocket's menu caches are cleared after
//! either change so the menu is rebuilt. The folder name is a plain, visible one at the card root, outside `Cores/` (the Pocket scans `Cores/` only).
//!
//! Plan, confirm, execute like every other write: the plan lists the moves, the token covers them, and nothing moves with a wrong token.

use crate::{ErrorCode, TauError, corehub, update::CATALOG_CACHES};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

/// The holding folder at the card root.
pub const DISABLED_DIR: &str = "Disabled Cores";
const INDEX: &str = "omega-disabled.json";

fn refuse(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InstallRefused, msg)
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DisabledCore {
    pub id: String,
    pub shortname: String,
    pub version: String,
    /// Unix seconds, 0 when unknown (a core moved by hand).
    pub disabled_at: u64,
    /// Why Omega disabled it (plain sentences); empty when the user chose to.
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatePlan {
    pub token: String,
    pub enable: bool,
    /// Core ids, in the order they move.
    pub ids: Vec<String>,
    pub from: Vec<String>,
    pub to: Vec<String>,
    pub blocked: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StateReport {
    pub moved: usize,
    pub caches_cleared: usize,
}

fn id_ok(id: &str) -> bool {
    !id.is_empty() && !id.contains(['/', '\\', '\0']) && !id.contains("..") && !id.starts_with('.')
}

#[cfg(feature = "serde")]
fn read_index(card: &Path) -> Vec<DisabledCore> {
    fs::read(card.join(DISABLED_DIR).join(INDEX))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

#[cfg(feature = "serde")]
fn write_index(card: &Path, list: &[DisabledCore]) -> Result<(), TauError> {
    let dir = card.join(DISABLED_DIR);
    fs::create_dir_all(&dir)?;
    let tmp = dir.join(format!("{INDEX}.part"));
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(list).map_err(|e| TauError::e(ErrorCode::Json, e.to_string()))?,
    )?;
    fs::rename(&tmp, dir.join(INDEX))?;
    Ok(())
}

/// The cores in the holding folder (any folder with a `core.json`, including ones moved there by hand), with what Omega recorded about them.
#[cfg(feature = "serde")]
pub fn list_disabled(card: &Path) -> Vec<DisabledCore> {
    let index = read_index(card);
    let mut out: Vec<DisabledCore> = fs::read_dir(card.join(DISABLED_DIR))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("core.json").is_file())
        .map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            let meta = fs::read(e.path().join("core.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                .and_then(|j| j.pointer("/core/metadata").cloned())
                .unwrap_or(serde_json::Value::Null);
            let text = |k: &str| {
                meta.get(k)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let rec = index.iter().find(|d| d.id == id);
            DisabledCore {
                shortname: text("shortname"),
                version: text("version"),
                disabled_at: rec.map_or(0, |d| d.disabled_at),
                reasons: rec.map(|d| d.reasons.clone()).unwrap_or_default(),
                id,
            }
        })
        .collect();
    out.sort_by_key(|d| d.id.to_lowercase());
    out
}

fn token(card: &Path, enable: bool, ids: &[String]) -> String {
    let mut h = Sha256::new();
    h.update(card.to_string_lossy().as_bytes());
    h.update([u8::from(enable)]);
    for id in ids {
        h.update([0]);
        h.update(id.as_bytes());
    }
    format!("{:x}", h.finalize())[..32].to_string()
}

/// Plans moving `ids` out of (disable) or back into (enable) `Cores/`. Reads the card only.
pub fn plan(card: &Path, ids: &[String], enable: bool) -> Result<StatePlan, TauError> {
    let mut p = StatePlan {
        token: String::new(),
        enable,
        ids: Vec::new(),
        from: Vec::new(),
        to: Vec::new(),
        blocked: None,
        notes: Vec::new(),
    };
    for id in ids {
        if !id_ok(id) {
            return Err(refuse(format!("{id:?} is not a core id.")));
        }
        let (live, held) = (format!("Cores/{id}"), format!("{DISABLED_DIR}/{id}"));
        let (from, to) = if enable { (held, live) } else { (live, held) };
        if !card.join(&from).join("core.json").is_file() {
            return Err(refuse(format!(
                "{id} is not {} on this card.",
                if enable { "disabled" } else { "installed" }
            )));
        }
        if card.join(&to).exists() {
            return Err(refuse(format!(
                "{to} already exists, so {id} cannot be {}. Remove or rename that folder first.",
                if enable { "enabled" } else { "disabled" }
            )));
        }
        p.ids.push(id.clone());
        p.from.push(from);
        p.to.push(to);
    }
    if p.ids.is_empty() {
        p.blocked = Some("No core is selected.".into());
    }
    if !enable && p.ids.iter().any(|i| crate::core_update::is_tau_core(i)) {
        p.notes.push("A Tau core is included: its music library and settings stay in place, and it returns exactly as it was when you enable it.".into());
    }
    if !enable
        && corehub::platform_tree(card)
            .is_ok_and(|t| t.iter().map(|n| n.cores.len()).sum::<usize>() <= p.ids.len())
    {
        p.notes
            .push("This leaves no core installed on the card.".into());
    }
    p.token = token(card, enable, &p.ids);
    Ok(p)
}

/// Moves the planned cores. The plan is rebuilt and must carry the same token; every move is checked afterwards (the core is at its new place and
/// gone from the old one) and a failed move is moved back. `reasons` records why Omega disabled each core (for the Disabled list); it is advice only.
#[cfg(feature = "serde")]
pub fn execute(
    card: &Path,
    ids: &[String],
    enable: bool,
    confirmation: &str,
    reasons: &[(String, Vec<String>)],
    now: u64,
) -> Result<StateReport, TauError> {
    let p = plan(card, ids, enable)?;
    if confirmation != p.token {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "The card changed since this was shown. Review it again.",
        ));
    }
    if let Some(b) = &p.blocked {
        return Err(refuse(b.clone()));
    }
    fs::create_dir_all(card.join(DISABLED_DIR))?;
    let mut done: Vec<(String, String)> = Vec::new();
    for (from, to) in p.from.iter().zip(&p.to) {
        let step = fs::rename(card.join(from), card.join(to))
            .map_err(TauError::from)
            .and_then(|()| {
                if card.join(to).join("core.json").is_file() && !card.join(from).exists() {
                    Ok(())
                } else {
                    Err(refuse(format!("{from} did not move cleanly.")))
                }
            });
        if let Err(e) = step {
            for (f, t) in done.iter().rev() {
                let _ = fs::rename(card.join(t), card.join(f));
            }
            return Err(e);
        }
        done.push((from.clone(), to.clone()));
    }
    let mut index = read_index(card);
    for id in &p.ids {
        index.retain(|d| &d.id != id);
        if !enable {
            index.push(DisabledCore {
                id: id.clone(),
                shortname: String::new(),
                version: String::new(),
                disabled_at: now,
                reasons: reasons
                    .iter()
                    .find(|(i, _)| i == id)
                    .map(|(_, r)| r.clone())
                    .unwrap_or_default(),
            });
        }
    }
    write_index(card, &index)?;
    let mut cleared = 0;
    for rel in CATALOG_CACHES {
        if fs::remove_file(card.join(rel)).is_ok() {
            cleared += 1;
        }
    }
    Ok(StateReport {
        moved: p.ids.len(),
        caches_cleared: cleared,
    })
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::corehub::testcard::{card, write};

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn a_core_moves_out_of_cores_and_back_with_everything_else_untouched() {
        let c = card("state1");
        write(&c.join("System/corelist_cache.bin"), b"stale");
        let before_saves = fs::read(c.join("Saves/gba/common/Boktai 2 (USA).sav")).unwrap();
        let p = plan(&c, &ids(&["other.GBAlt"]), false).unwrap();
        assert_eq!(
            (p.from[0].as_str(), p.to[0].as_str()),
            ("Cores/other.GBAlt", "Disabled Cores/other.GBAlt")
        );
        assert!(execute(&c, &ids(&["other.GBAlt"]), false, "wrong", &[], 1).is_err());
        assert!(c.join("Cores/other.GBAlt").exists());
        let r = execute(
            &c,
            &ids(&["other.GBAlt"]),
            false,
            &p.token,
            &[("other.GBAlt".into(), vec!["No game files".into()])],
            5,
        )
        .unwrap();
        assert_eq!((r.moved, r.caches_cleared), (1, 1));
        assert!(
            !c.join("Cores/other.GBAlt").exists()
                && c.join("Disabled Cores/other.GBAlt/core.json").is_file()
        );
        // the card scan no longer sees it; the holding folder does, with the reason
        assert!(
            !crate::inspect_card(&c)
                .unwrap()
                .cores
                .iter()
                .any(|x| x.id == "other.GBAlt")
        );
        let d = list_disabled(&c);
        assert_eq!(
            (
                d[0].id.as_str(),
                d[0].version.as_str(),
                d[0].disabled_at,
                d[0].reasons.clone()
            ),
            ("other.GBAlt", "0.3", 5, vec!["No game files".to_string()])
        );
        // enable it again: the same files are back and the record is gone
        let p = plan(&c, &ids(&["other.GBAlt"]), true).unwrap();
        execute(&c, &ids(&["other.GBAlt"]), true, &p.token, &[], 6).unwrap();
        assert!(c.join("Cores/other.GBAlt/core.json").is_file() && list_disabled(&c).is_empty());
        assert_eq!(
            fs::read(c.join("Saves/gba/common/Boktai 2 (USA).sav")).unwrap(),
            before_saves
        );
        assert!(c.join("Assets/gba/other.GBAlt/Only Mine.gba").is_file());
    }

    #[test]
    fn refusals_leave_the_card_alone() {
        let c = card("state2");
        assert!(plan(&c, &ids(&["nobody.Here"]), false).is_err());
        assert!(plan(&c, &ids(&["../Cores/other.GBAlt"]), false).is_err());
        assert!(plan(&c, &ids(&["other.GBAlt"]), true).is_err()); // not disabled
        // a leftover folder in the way stops the move
        write(&c.join("Disabled Cores/other.GBAlt/x"), b"x");
        assert!(plan(&c, &ids(&["other.GBAlt"]), false).is_err());
        assert!(c.join("Cores/other.GBAlt/core.json").is_file());
        assert!(plan(&c, &[], false).unwrap().blocked.is_some());
    }

    #[test]
    fn a_core_moved_by_hand_is_still_listed_and_a_conflict_blocks_enabling() {
        let c = card("state3");
        fs::create_dir_all(c.join("Disabled Cores")).unwrap();
        fs::rename(
            c.join("Cores/solo.Thing"),
            c.join("Disabled Cores/solo.Thing"),
        )
        .unwrap();
        let d = list_disabled(&c);
        assert_eq!((d[0].id.as_str(), d[0].disabled_at), ("solo.Thing", 0));
        write(&c.join("Cores/solo.Thing/core.json"), b"{}");
        assert!(
            plan(&c, &ids(&["solo.Thing"]), true)
                .unwrap_err()
                .message
                .contains("already exists")
        );
    }
}
