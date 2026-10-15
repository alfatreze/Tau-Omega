//! "Installed, but not in the Pocket's menu": the checks that explain it (`docs/DEV_LOOP_SPEC.md`, section 3, the symptom helper). Each finding says what
//! was seen, why it hides a core, and the fix. Reads the card only; clearing the menu caches is a write and stays an explicit action elsewhere.

use crate::{
    core_lint::{Finding, Severity},
    update::CATALOG_CACHES,
};
use std::{fs, path::Path, time::SystemTime};

const RULE: &str = "Analogue docs: the Pocket lists only valid cores in Cores/<author>.<shortname>, from its menu cache";

fn finding(
    id: &str,
    severity: Severity,
    message: impl Into<String>,
    fix: impl Into<String>,
) -> Finding {
    Finding {
        id: id.into(),
        severity,
        message: message.into(),
        rule: RULE.into(),
        fix: fix.into(),
    }
}

fn newest(dir: &Path, depth: u8) -> Option<SystemTime> {
    if depth > 6 {
        return None;
    }
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            if m.is_dir() {
                newest(&e.path(), depth + 1)
            } else {
                m.modified().ok()
            }
        })
        .max()
}

/// Why `id` might not be in the menu, most likely first. An empty list means none of the usual causes was found.
pub fn symptoms(card: &Path, id: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let live = card.join("Cores").join(id);
    if !live.join("core.json").is_file() {
        if card
            .join(crate::core_state::DISABLED_DIR)
            .join(id)
            .join("core.json")
            .is_file()
        {
            out.push(finding("M1", Severity::Error, "This core is disabled: it was moved out of Cores to Disabled Cores, which the Pocket does not read.", "Enable it from the Disabled section on the Cores page."));
        } else {
            out.push(finding("M2", Severity::Error, format!("There is no Cores/{id}/core.json on this card."), "Install the core, or check the folder is directly inside Cores and named author.shortname."));
        }
        return out;
    }
    let meta = fs::read(live.join("core.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.pointer("/core/metadata").cloned());
    let Some(meta) = meta else {
        out.push(finding(
            "M3",
            Severity::Error,
            "core.json is not valid JSON or has no core.metadata.",
            "Fix the file; the Pocket drops the core from its menu when core.json cannot be read.",
        ));
        return out;
    };
    let text = |k: &str| {
        meta.get(k)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let expect = format!("{}.{}", text("author"), text("shortname"));
    if !expect.eq_ignore_ascii_case(id) {
        out.push(finding(
            "M4",
            Severity::Error,
            format!("The folder is named {id}, but core.json says {expect}."),
            format!("Rename the folder to {expect}, or fix author and shortname in core.json."),
        ));
    }
    for (field, max) in [
        ("description", 63usize),
        ("shortname", 31),
        ("author", 31),
        ("url", 63),
        ("version", 31),
    ] {
        let n = text(field).chars().count();
        if n > max {
            out.push(finding("M5", Severity::Warning, format!("{field} is {n} characters; the documented limit is {max}. An over-long field has been seen to make a core vanish."), format!("Shorten {field} to {max} characters or fewer.")));
        }
    }
    if let Ok(l) = crate::core_lint::lint_card_core(card, id) {
        for f in l.findings.iter().filter(|f| f.severity == Severity::Error) {
            out.push(Finding {
                id: format!("M6:{}", f.id),
                ..f.clone()
            });
        }
    }
    if let Some(core_time) = newest(&live, 0) {
        let stale: Vec<&str> = CATALOG_CACHES
            .iter()
            .copied()
            .filter(|c| {
                fs::metadata(card.join(c))
                    .and_then(|m| m.modified())
                    .is_ok_and(|t| t < core_time)
            })
            .collect();
        if !stale.is_empty() {
            out.push(finding("M7", Severity::Warning, format!("The Pocket's menu cache ({}) is older than this core's files, so the menu may not have been rebuilt since it was installed.", stale.join(", ")), "Clear the five menu cache files in System (an Omega install does this for you); the Pocket rebuilds its menu on the next start."));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corehub::testcard::{card, core_json, write};

    fn ids(v: &[Finding]) -> Vec<String> {
        v.iter().map(|f| f.id.clone()).collect()
    }

    #[test]
    fn a_missing_or_disabled_core_is_explained_first() {
        let c = card("sym1");
        assert_eq!(ids(&symptoms(&c, "no.Such")), ["M2"]);
        write(
            &c.join("Disabled Cores/x.Held/core.json"),
            core_json("x", "Held", "1", &["xp"]).as_bytes(),
        );
        assert_eq!(ids(&symptoms(&c, "x.Held")), ["M1"]);
    }

    #[test]
    fn a_broken_core_json_a_wrong_folder_name_and_a_long_field_are_each_named() {
        let c = card("sym2");
        write(&c.join("Cores/x.Bad/core.json"), b"{ not json");
        assert_eq!(ids(&symptoms(&c, "x.Bad")), ["M3"]);
        write(
            &c.join("Cores/other.Name/core.json"),
            core_json("x", "Real", "1", &["xp"]).as_bytes(),
        );
        assert!(ids(&symptoms(&c, "other.Name")).contains(&"M4".to_string()));
        let long = core_json("x", "Real", "1", &["xp"]).replace(
            "\"version\"",
            &format!("\"description\":\"{}\",\"version\"", "d".repeat(70)),
        );
        write(&c.join("Cores/x.Real/core.json"), long.as_bytes());
        let found = symptoms(&c, "x.Real");
        assert!(
            found
                .iter()
                .any(|f| f.id == "M5" && f.message.contains("description is 70 characters")),
            "{found:?}"
        );
    }

    #[test]
    fn a_menu_cache_older_than_the_core_is_reported_and_a_fresh_one_is_not() {
        let c = card("sym3");
        write(
            &c.join("Cores/x.Fine/core.json"),
            core_json("x", "Fine", "1", &["xp"]).as_bytes(),
        );
        write(&c.join("System/corelist_cache.bin"), b"old");
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        let f = fs::File::options()
            .write(true)
            .open(c.join("System/corelist_cache.bin"))
            .unwrap();
        f.set_modified(old).unwrap();
        assert!(ids(&symptoms(&c, "x.Fine")).contains(&"M7".to_string()));
        let now = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        fs::File::options()
            .write(true)
            .open(c.join("System/corelist_cache.bin"))
            .unwrap()
            .set_modified(now)
            .unwrap();
        assert!(!ids(&symptoms(&c, "x.Fine")).contains(&"M7".to_string()));
    }
}
