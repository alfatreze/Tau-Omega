//! Can an installed core work at all? The evidence that justifies offering to disable it (`docs/CORE_DISABLE_SPEC.md`): the core is **broken** (the
//! lint finds no bitstream, an unreadable `core.json`, or a missing required JSON file) or it is **unusable** (a file it requires is not on the card:
//! a fixed-name BIOS, or no file at all that a required game slot accepts). Everything else is healthy, including a core that needs no files, which no
//! check can recognise as special: that is why the verdict is only ever a suggestion, and why the user can always enable a core again by hand or mark
//! it "keep enabled". Read-only.

use crate::{
    TauError,
    core_files::{self, walk},
    core_lint::{self, Severity},
    corehub,
};
use std::path::Path;

/// The lint rules that make a core fail to start or load at all (everything else is advice).
const FATAL_LINT: [&str; 4] = ["L01", "L02", "L09", "L11"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Verdict {
    Healthy,
    /// The core's own files are unusable (no bitstream, invalid core.json, a required JSON file missing).
    Broken,
    /// The core is complete but a file it requires is missing, so it cannot start or has nothing to play.
    Unusable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Reason {
    /// `lint:<id>`, `missing-file` or `nothing-to-load`.
    pub code: String,
    pub message: String,
    /// What would make it work.
    pub fix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreHealth {
    pub id: String,
    pub shortname: String,
    pub verdict: Verdict,
    pub reasons: Vec<Reason>,
    /// No slot of the core requires a file: nothing here can be missing, so it is never flagged for that. (It may still need nothing, or have a quirk.)
    pub needs_no_files: bool,
}

fn files_with_ext(dir: &Path, exts: &[String]) -> usize {
    walk(dir)
        .iter()
        .filter(|(rel, _)| {
            let e = rel
                .rsplit_once('.')
                .map_or("", |(_, e)| e)
                .to_ascii_lowercase();
            exts.contains(&e)
        })
        .count()
}

/// The health of one installed core.
pub fn check_core(card: &Path, id: &str) -> Result<CoreHealth, TauError> {
    let detail = corehub::core_detail(card, id)?;
    let mut reasons = Vec::new();

    let lint = core_lint::lint_card_core(card, id)?;
    for f in lint
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error && FATAL_LINT.contains(&f.id.as_str()))
    {
        reasons.push(Reason {
            code: format!("lint:{}", f.id),
            message: f.message.clone(),
            fix: f.fix.clone(),
        });
    }
    let broken = !reasons.is_empty();

    for m in core_files::missing(card, &detail) {
        reasons.push(Reason {
            code: "missing-file".into(),
            message: format!(
                "It needs {} ({}), which is not on the card.",
                m.filename, m.slot
            ),
            fix: format!("Put {} in {}.", m.filename, m.folder_rel),
        });
    }
    // Slots fed another way do not need files of their own: a core with an instance-JSON slot (parameter bit 4) loads its other slots from the
    // instance file (Neo Geo's per-game ROM parts come from a zip the instance names), and a `deferload` slot is loaded by the core, not the Pocket.
    let data: Option<serde_json::Value> =
        std::fs::read(card.join("Cores").join(id).join("data.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
    let raw: Vec<serde_json::Value> = data
        .as_ref()
        .and_then(|d| d.pointer("/data/data_slots"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let params = |i: usize| raw.get(i).map_or(0, crate::cardlayout::slot_parameters);
    let has_instance = (0..raw.len()).any(|i| params(i) & 0x10 != 0);
    let mut required_slots = 0;
    for (i, s) in detail.slots.iter().enumerate().filter(|(_, s)| s.required) {
        required_slots += 1;
        let deferred = raw
            .get(i)
            .and_then(|r| r.get("deferload"))
            .and_then(serde_json::Value::as_bool)
            == Some(true);
        let fed_by_instance = has_instance && params(i) & 0x10 == 0;
        if !s.filename.is_empty() || s.extensions.is_empty() || deferred || fed_by_instance {
            continue;
        }
        let folder = if s.core_specific { id } else { "common" };
        let dir = card.join("Assets").join(&s.platform).join(folder);
        if files_with_ext(&dir, &s.extensions) == 0 {
            let exts = s
                .extensions
                .iter()
                .map(|e| format!(".{e}"))
                .collect::<Vec<_>>()
                .join(" ");
            reasons.push(Reason {
                code: "nothing-to-load".into(),
                message: format!(
                    "Nothing to load for \"{}\": no {exts} files in Assets/{}/{folder}.",
                    s.name, s.platform
                ),
                fix: format!("Add {exts} files to Assets/{}/{folder}.", s.platform),
            });
        }
    }
    let verdict = if broken {
        Verdict::Broken
    } else if reasons.is_empty() {
        Verdict::Healthy
    } else {
        Verdict::Unusable
    };
    Ok(CoreHealth {
        id: id.to_string(),
        shortname: detail.shortname,
        verdict,
        reasons,
        needs_no_files: required_slots == 0,
    })
}

/// Every core on a card.
pub fn check_card(card: &Path) -> Result<Vec<CoreHealth>, TauError> {
    crate::inspect_card(card)?
        .cores
        .iter()
        .map(|c| check_core(card, &c.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corehub::testcard::{card, core_json, write};

    #[test]
    fn a_core_missing_its_bios_is_unusable_and_the_reason_goes_when_it_is_added() {
        let c = card("health1");
        let h = check_core(&c, "Spiritualized.GBA").unwrap();
        // the fixture's GBA core requires gba_bios.bin, which the card does not have
        assert!(
            h.reasons
                .iter()
                .any(|r| r.code == "missing-file" && r.message.contains("gba_bios.bin")),
            "{:?}",
            h.reasons
        );
        write(&c.join("Assets/gba/common/gba_bios.bin"), &[0u8; 16384]);
        let h = check_core(&c, "Spiritualized.GBA").unwrap();
        assert!(h.reasons.iter().all(|r| r.code != "missing-file"));
    }

    #[test]
    fn nothing_to_load_is_reported_per_slot_and_a_core_needing_no_files_is_not_flagged() {
        let c = card("health2");
        write(
            &c.join("Cores/x.Empty/core.json"),
            core_json("x", "Empty", "1", &["xp"]).as_bytes(),
        );
        write(
            &c.join("Cores/x.Empty/data.json"),
            br#"{"data":{"data_slots":[{"name":"Game","id":1,"required":true,"parameters":0,"extensions":["bin","rom"]}]}}"#,
        );
        let h = check_core(&c, "x.Empty").unwrap();
        assert!(
            h.reasons
                .iter()
                .any(|r| r.code == "nothing-to-load" && r.message.contains(".bin .rom")),
            "{:?}",
            h.reasons
        );
        assert!(!h.needs_no_files);
        write(&c.join("Assets/xp/common/a.rom"), b"x");
        assert!(
            !check_core(&c, "x.Empty")
                .unwrap()
                .reasons
                .iter()
                .any(|r| r.code == "nothing-to-load")
        );
        // optional slots only: nothing can be missing, and the core says so
        write(
            &c.join("Cores/x.Free/core.json"),
            core_json("x", "Free", "1", &["xq"]).as_bytes(),
        );
        write(
            &c.join("Cores/x.Free/data.json"),
            br#"{"data":{"data_slots":[{"name":"Save","id":1,"required":false,"extensions":["sav"]}]}}"#,
        );
        let f = check_core(&c, "x.Free").unwrap();
        assert!(
            f.needs_no_files
                && !f
                    .reasons
                    .iter()
                    .any(|r| r.code == "nothing-to-load" || r.code == "missing-file")
        );
    }

    #[test]
    fn slots_fed_by_an_instance_file_or_deferred_are_not_judged_by_their_own_files() {
        let c = card("health4");
        write(
            &c.join("Cores/x.Inst/core.json"),
            core_json("x", "Inst", "1", &["xp"]).as_bytes(),
        );
        write(
            &c.join("Cores/x.Inst/data.json"),
            br#"{"data":{"data_slots":[
              {"name":"Setup","id":0,"required":true,"parameters":"0x13","extensions":["json"]},
              {"name":"PROG","id":4,"required":true,"parameters":0,"extensions":["prom"]},
              {"name":"Late","id":5,"required":true,"parameters":0,"extensions":["dat"],"deferload":true}]}}"#,
        );
        let h = check_core(&c, "x.Inst").unwrap();
        // only the instance slot itself counts: PROG comes from the instance file, Late is loaded by the core
        let codes: Vec<&str> = h
            .reasons
            .iter()
            .map(|r| r.message.as_str())
            .filter(|m| m.contains("Nothing to load"))
            .collect();
        assert_eq!(codes.len(), 1, "{codes:?}");
        assert!(codes[0].contains("Setup"));
        write(&c.join("Assets/xp/x.Inst/game.json"), b"{}");
        assert!(
            !check_core(&c, "x.Inst")
                .unwrap()
                .reasons
                .iter()
                .any(|r| r.message.contains("Nothing to load"))
        );
    }

    #[test]
    fn a_core_without_a_bitstream_is_broken_not_just_unusable() {
        let c = card("health3");
        write(
            &c.join("Cores/x.NoBits/core.json"),
            core_json("x", "NoBits", "1", &["xp"]).as_bytes(),
        );
        let h = check_core(&c, "x.NoBits").unwrap();
        assert_eq!(h.verdict, Verdict::Broken);
        assert!(h.reasons.iter().any(|r| r.code.starts_with("lint:")));
    }
}
