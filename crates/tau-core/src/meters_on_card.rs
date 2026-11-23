//! What meters are on a card, read only: for each Tau core, which list the Pocket will use (the user's meter file, the core's own bundle, or only
//! the built-in Winamp Bars) and the meters in it, in the player's order. Built on [`crate::meter_bin`]; nothing here writes.
//!
//! The rules (Tau Alpha 027, A027/3): the Pocket prefers the user file `Assets/<platform>/common/tau-meters.bin` when it is sound (a bin with no
//! fallback row is refused whole), else the core's own `Assets/<platform>/<core folder>/tau-packs.bin`, else only Winamp Bars. **A core reads the
//! user file only if its `data.json` declares a data slot for `tau-meters.bin`** (v0.7.5 and later; v0.7.1 and earlier ignore the file, so they are
//! shown as not using it and Omega must not offer a meter install for them).

use crate::{
    core_update::is_tau_core,
    meter_bin::{self, Availability, MeterBin, MeterRow},
};
use serde_json::Value;
use std::{fs, path::Path};

const USER_FILE: &str = "tau-meters.bin";
const BUNDLE: &str = "tau-packs.bin";

/// One meter as a person sees it in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RowView {
    pub name: String,
    /// `1.2.0`; empty for a meter of an old v1 bundle (it has no version).
    pub version: String,
    /// The built-in Winamp Bars: it cannot be hidden or removed.
    pub fallback: bool,
    /// Hidden meters are in the file but not in the player's list.
    pub visible: bool,
    /// Why the Pocket greys the meter out (needs a newer Tau, or another pack ABI); `None` when it can run.
    pub unavailable: Option<String>,
    /// Another meter shares its short id, so this one cannot remember tuned settings (it still runs).
    pub tuning_lost: bool,
    pub binary_only: bool,
}

/// A meter file or bundle on the card.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BinView {
    /// Relative to the card root, forward slashes.
    pub rel_path: String,
    pub bytes: u64,
    /// 2, or 1 for an old bundle.
    pub format: u16,
    pub rows: Vec<RowView>,
    /// In plain words, when the Pocket will not use the file: why.
    pub problem: Option<String>,
}

/// Which list the Pocket will use for a core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Uses {
    /// The user's meter file (`tau-meters.bin`).
    UserFile,
    /// The core's own shipped bundle (`tau-packs.bin`).
    OwnBundle,
    /// Neither is usable: only the built-in Winamp Bars.
    BuiltInOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreMeters {
    /// The core's folder name, e.g. `alfatreze.TAU`.
    pub core: String,
    /// `0.7.5`, empty when unknown.
    pub version: String,
    /// The core declares a data slot for `tau-meters.bin`: only then does it read the user file.
    pub reads_user_file: bool,
    pub uses: Uses,
    /// The core's own bundle, when it has one.
    pub bundle: Option<BinView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OnCard {
    /// The user's meter file, when the card has one (shared by the Tau cores of a platform).
    pub user_file: Option<BinView>,
    pub cores: Vec<CoreMeters>,
}

fn dirs(path: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(path)
        .map(|r| {
            r.filter_map(Result::ok)
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn json(path: &Path) -> Option<Value> {
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

/// `(major, minor)` of a version text like `0.7.5` (a pre-release tail is ignored).
fn version_pair(v: &str) -> Option<(u16, u16)> {
    let mut parts = v.split(['.', '-', '+']);
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

fn rows_of(bin: &MeterBin, tau: Option<u16>) -> Vec<RowView> {
    let collisions = meter_bin::tuning_collisions(&bin.rows);
    bin.rows
        .iter()
        .zip(collisions)
        .map(|(r, lost): (&MeterRow, bool)| RowView {
            name: r.name.clone(),
            version: if r.from_v1() {
                String::new()
            } else {
                format!("{}.{}.{}", r.version.0, r.version.1, r.version.2)
            },
            fallback: r.fallback(),
            visible: r.visible(),
            unavailable: match meter_bin::availability(
                r,
                meter_bin::SUPPORTED_ABI,
                tau.unwrap_or(u16::MAX),
            ) {
                Availability::Available => None,
                Availability::NeedsOtherAbi => {
                    Some("It was made for a different version of the Pocket's meter system.".into())
                }
                Availability::NeedsNewerTau => {
                    Some("It needs a newer Tau than the one on the card.".into())
                }
            },
            tuning_lost: lost && !r.from_v1(),
            binary_only: r.binary_only(),
        })
        .collect()
}

/// Reads one file for display. `usable` says whether the Pocket would use it (a v2 file must pass the firmware's boot check, including the fallback
/// row; an old v1 bundle is read by the firmware that shipped it).
fn view(card: &Path, rel: &str, tau: Option<u16>) -> Option<(BinView, bool)> {
    let path = card.join(rel);
    let meta = fs::symlink_metadata(&path).ok().filter(|m| m.is_file())?;
    let mk = |format, rows, problem: Option<String>| BinView {
        rel_path: rel.to_string(),
        bytes: meta.len(),
        format,
        rows,
        problem,
    };
    if meta.len() as usize > meter_bin::MAX_BIN {
        return Some((
            mk(0, vec![], Some(meter_bin::Refusal::Size.plain().into())),
            false,
        ));
    }
    let Ok(bytes) = fs::read(&path) else {
        return Some((
            mk(0, vec![], Some(meter_bin::Refusal::Read.plain().into())),
            false,
        ));
    };
    Some(match meter_bin::parse(&bytes) {
        Err(e) => (mk(0, vec![], Some(e.plain())), false),
        Ok(bin) if bin.version == 1 => {
            let rows = rows_of(&bin, tau);
            (mk(1, rows, None), true)
        }
        Ok(bin) => {
            let problem = meter_bin::boot_check(&bytes, bytes.len())
                .err()
                .map(|e| e.plain());
            let usable = problem.is_none();
            (mk(2, rows_of(&bin, tau), problem), usable)
        }
    })
}

/// Reads the card: every Tau core, its own bundle, and the user's meter file of its platform. Writes nothing.
pub fn read(card: &Path) -> OnCard {
    let mut user_file = None;
    let mut user_usable = false;
    let mut cores = Vec::new();
    let mut user_seen: Vec<String> = Vec::new();
    for core in dirs(&card.join("Cores")) {
        if !is_tau_core(&core) {
            continue;
        }
        let meta = json(&card.join("Cores").join(&core).join("core.json"));
        let meta = meta.as_ref().and_then(|j| j.pointer("/core/metadata"));
        let version = meta
            .and_then(|m| m.get("version"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let platform = meta
            .and_then(|m| m.pointer("/platform_ids/0"))
            .and_then(Value::as_str)
            .unwrap_or("tau")
            .to_string();
        let tau = version_pair(&version).map(|(a, b)| a << 8 | b);
        let reads_user_file = json(&card.join("Cores").join(&core).join("data.json"))
            .is_some_and(|d| declares_file(&d, USER_FILE));
        if user_file.is_none() {
            // The user's file of the first core's platform (the Tau cores of a platform share it).
            let rel = format!("Assets/{platform}/common/{USER_FILE}");
            if !user_seen.contains(&rel) {
                user_seen.push(rel.clone());
                if let Some((v, usable)) = view(card, &rel, tau) {
                    user_usable = usable;
                    user_file = Some(v);
                }
            }
        }
        let own = view(card, &format!("Assets/{platform}/{core}/{BUNDLE}"), tau);
        let own_usable = own.as_ref().is_some_and(|(_, u)| *u);
        let uses = if reads_user_file && user_usable {
            Uses::UserFile
        } else if own_usable {
            Uses::OwnBundle
        } else {
            Uses::BuiltInOnly
        };
        cores.push(CoreMeters {
            core,
            version,
            reads_user_file,
            uses,
            bundle: own.map(|(v, _)| v),
        });
    }
    OnCard { user_file, cores }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "tau-onc-{tag}-{}",
            u128::from(std::process::id()) + crate::test_uniq()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn put(card: &Path, rel: &str, bytes: &[u8]) {
        let p = card.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }
    fn vector(dir: &str, name: &str) -> Vec<u8> {
        fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../testdata")
                .join(dir)
                .join(name),
        )
        .unwrap()
    }
    fn core(card: &Path, id: &str, version: &str, reads_user_file: bool) {
        put(
            card,
            &format!("Cores/{id}/core.json"),
            format!(
                r#"{{"core":{{"metadata":{{"version":"{version}","platform_ids":["tau"]}}}}}}"#
            )
            .as_bytes(),
        );
        let slots = if reads_user_file {
            r#"{"filename":"tau-meters.bin"},{"filename":"tau-library.tdb"}"#
        } else {
            r#"{"filename":"tau-library.tdb"}"#
        };
        put(
            card,
            &format!("Cores/{id}/data.json"),
            format!(r#"{{"data":{{"data_slots":[{slots}]}}}}"#).as_bytes(),
        );
    }

    #[test]
    fn a_core_that_reads_the_user_file_uses_it_when_it_is_sound() {
        let card = scratch("user");
        core(&card, "alfatreze.TAU", "0.7.5", true);
        put(
            &card,
            "Assets/tau/common/tau-meters.bin",
            &vector("meter_bin", "v2_reordered_hidden.bin"),
        );
        put(
            &card,
            "Assets/tau/alfatreze.TAU/tau-packs.bin",
            &vector("meter_bin", "v2_seven.bin"),
        );
        let on = read(&card);
        let c = &on.cores[0];
        assert_eq!(
            (
                c.core.as_str(),
                c.version.as_str(),
                c.reads_user_file,
                c.uses
            ),
            ("alfatreze.TAU", "0.7.5", true, Uses::UserFile)
        );
        let user = on.user_file.unwrap();
        assert_eq!(
            (
                user.rel_path.as_str(),
                user.format,
                user.problem.is_none(),
                user.rows.len()
            ),
            ("Assets/tau/common/tau-meters.bin", 2, true, 7)
        );
        assert_eq!(
            user.rows[0].name, "SOUND MAP",
            "the table of contents is the order"
        );
        assert_eq!(
            user.rows
                .iter()
                .filter(|r| !r.visible)
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>(),
            ["MASTER VU"]
        );
        assert!(
            user.rows
                .iter()
                .any(|r| r.fallback && r.name == "WINAMP BARS")
        );
        assert_eq!(
            c.bundle.as_ref().unwrap().rows.len(),
            7,
            "the core's own bundle is shown too"
        );
    }

    #[test]
    fn a_core_that_does_not_declare_the_slot_ignores_the_user_file_and_uses_its_own_bundle() {
        let card = scratch("old");
        core(&card, "alfatreze.TAU", "0.7.1", false);
        put(
            &card,
            "Assets/tau/common/tau-meters.bin",
            &vector("meter_bin", "v2_seven.bin"),
        );
        put(
            &card,
            "Assets/tau/alfatreze.TAU/tau-packs.bin",
            &vector("meter_bin", "v1_seven.bin"),
        );
        let on = read(&card);
        assert_eq!(
            (on.cores[0].reads_user_file, on.cores[0].uses),
            (false, Uses::OwnBundle)
        );
        let own = on.cores[0].bundle.as_ref().unwrap();
        assert_eq!(
            (
                own.format,
                own.rows.len(),
                own.rows[0].version.as_str(),
                own.problem.is_none()
            ),
            (1, 7, "", true),
            "a v1 bundle has no versions"
        );
        assert!(own.rows.iter().all(|r| !r.tuning_lost));
    }

    #[test]
    fn a_file_the_pocket_would_refuse_says_why_and_the_core_falls_back() {
        let card = scratch("refused");
        core(&card, "alfatreze.TAU", "0.7.5", true);
        put(
            &card,
            "Assets/tau/common/tau-meters.bin",
            &vector("meter_bin", "v2_no_fallback.bin"),
        );
        let on = read(&card);
        assert_eq!(on.cores[0].uses, Uses::BuiltInOnly);
        let user = on.user_file.unwrap();
        assert!(
            user.problem
                .as_deref()
                .unwrap()
                .contains("no built-in Winamp Bars"),
            "{:?}",
            user.problem
        );
        assert_eq!(
            user.rows.len(),
            7,
            "the meters are still listed so a person can see what is in the file"
        );
        // damaged files are refused with a reason and no rows
        put(
            &card,
            "Assets/tau/common/tau-meters.bin",
            &vector("meter_bin", "bad_toc_crc.bin"),
        );
        let user = read(&card).user_file.unwrap();
        assert_eq!(
            (user.rows.len(), user.problem.as_deref()),
            (0, Some("The list of meters in the file is damaged."))
        );
    }

    #[test]
    fn rows_that_cannot_run_or_share_an_id_are_marked_and_a_card_without_meter_files_uses_only_the_built_in()
     {
        let card = scratch("marks");
        core(&card, "alfatreze.TAU", "0.7.5", true);
        put(
            &card,
            "Assets/tau/common/tau-meters.bin",
            &vector("meter_bin", "v2_unavailable_rows.bin"),
        );
        let rows = read(&card).user_file.unwrap().rows;
        assert!(rows[0].unavailable.is_none());
        assert!(
            rows[1]
                .unavailable
                .as_deref()
                .unwrap()
                .contains("different version")
        );
        assert!(
            rows[2]
                .unavailable
                .as_deref()
                .unwrap()
                .contains("newer Tau")
        );
        put(
            &card,
            "Assets/tau/common/tau-meters.bin",
            &vector("meter_bin", "v2_tuning_collision.bin"),
        );
        let rows = read(&card).user_file.unwrap().rows;
        assert_eq!(
            rows.iter().map(|r| r.tuning_lost).collect::<Vec<_>>(),
            [true, true, false]
        );
        let bare = scratch("bare");
        core(&bare, "alfatreze.TAU", "0.7.5", true);
        let on = read(&bare);
        assert_eq!(
            (on.user_file, on.cores[0].uses, on.cores[0].bundle.is_none()),
            (None, Uses::BuiltInOnly, true)
        );
        assert!(read(&scratch("nothing")).cores.is_empty());
    }
}
