//! The Builds page's engine (`docs/DEV_LOOP_SPEC.md`): what is in a developer's build folder, where each build sits in its series, how it compares with
//! what the card has, and which installed cores are older builds of the same series. Reads only; installing goes through the usual reviewed plan.

use crate::{
    TauError,
    build_series::{Placement, Scheme, Subject, classify, compare},
    core_lint,
    core_package::Limits,
    core_preview,
};
use std::{
    cmp::Ordering,
    fs,
    path::{Path, PathBuf},
};

/// Most entries listed from one folder.
pub const MAX_BUILDS: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BuildEntry {
    pub path: String,
    /// `"zip"` or `"folder"`.
    pub kind: String,
    /// The file or folder name.
    pub file: String,
    pub core_id: String,
    pub shortname: String,
    pub version: String,
    pub date_release: String,
    pub bytes: u64,
    /// Unix seconds, 0 when unknown.
    pub modified: u64,
    pub placement: Option<Placement>,
    /// Core-checker results (zips only; a folder is checked when it is installed).
    pub lint_errors: usize,
    pub lint_warnings: usize,
    /// Why this entry could not be read; the other fields are then empty.
    pub error: Option<String>,
}

fn modified(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

fn tree_bytes(dir: &Path, depth: u8) -> u64 {
    if depth > 8 {
        return 0;
    }
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => tree_bytes(&e.path(), depth + 1),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// The `core.json` of a package folder (`Cores/<id>/core.json`) or a bare core folder (`core.json`), when there is exactly one.
fn folder_core_json(dir: &Path) -> Option<PathBuf> {
    if dir.join("core.json").is_file() {
        return Some(dir.join("core.json"));
    }
    let found: Vec<PathBuf> = fs::read_dir(dir.join("Cores"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join("core.json"))
        .filter(|p| p.is_file())
        .collect();
    (found.len() == 1).then(|| found.into_iter().next().unwrap_or_default())
}

fn entry_base(path: &Path, kind: &str) -> BuildEntry {
    BuildEntry {
        path: path.to_string_lossy().into_owned(),
        kind: kind.into(),
        file: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        core_id: String::new(),
        shortname: String::new(),
        version: String::new(),
        date_release: String::new(),
        bytes: 0,
        modified: modified(path),
        placement: None,
        lint_errors: 0,
        lint_warnings: 0,
        error: None,
    }
}

fn place(e: &mut BuildEntry, schemes: &[Scheme]) {
    let stem = e
        .file
        .strip_suffix(".zip")
        .or_else(|| e.file.strip_suffix(".ZIP"))
        .unwrap_or(&e.file)
        .to_string();
    e.placement = classify(
        schemes,
        &Subject {
            core_id: &e.core_id,
            shortname: &e.shortname,
            version: &e.version,
            file: &stem,
        },
    );
}

fn read_zip(path: &Path, schemes: &[Scheme]) -> BuildEntry {
    let mut e = entry_base(path, "zip");
    match core_preview::preview_zip(path, Limits::default()) {
        Ok(p) => {
            e.core_id = p.core_id;
            e.shortname = p.shortname;
            e.version = p.version;
            e.date_release = p.date_release;
            e.bytes = p.zip_bytes;
            if let Ok(l) = core_lint::lint_zip(path) {
                e.lint_errors = l.errors;
                e.lint_warnings = l.warnings;
            }
            place(&mut e, schemes);
        }
        Err(err) => e.error = Some(err.message),
    }
    e
}

fn read_folder(path: &Path, json: &Path, schemes: &[Scheme]) -> BuildEntry {
    let mut e = entry_base(path, "folder");
    let meta = fs::read(json)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.pointer("/core/metadata").cloned());
    let Some(meta) = meta else {
        e.error = Some("core.json is missing its core.metadata.".into());
        return e;
    };
    let text = |k: &str| {
        meta.get(k)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    e.core_id = format!("{}.{}", text("author"), text("shortname"));
    e.shortname = text("shortname");
    e.version = text("version");
    e.date_release = text("date_release");
    e.bytes = tree_bytes(path, 0);
    place(&mut e, schemes);
    e
}

/// Lists the builds in `dir`: every `.zip` and every package or core folder one level down. Unreadable entries are listed with their reason instead of
/// failing the scan. Newest first within each series; builds with no series follow, newest file first.
pub fn scan(dir: &Path, schemes: &[Scheme]) -> Result<Vec<BuildEntry>, TauError> {
    let mut out = Vec::new();
    let mut names: Vec<PathBuf> = fs::read_dir(dir)?.flatten().map(|e| e.path()).collect();
    names.sort();
    for p in names {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        if out.len() >= MAX_BUILDS {
            break;
        }
        if p.is_file() && name.to_ascii_lowercase().ends_with(".zip") {
            out.push(read_zip(&p, schemes));
        } else if let (true, Some(json)) = (p.is_dir(), folder_core_json(&p)) {
            out.push(read_folder(&p, &json, schemes));
        }
    }
    out.sort_by(|a, b| match (&a.placement, &b.placement) {
        (Some(x), Some(y)) => x.series.cmp(&y.series).then_with(|| compare(y, x)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => b.modified.cmp(&a.modified),
    });
    Ok(out)
}

/// How a build compares with the same core on the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Relation {
    NotInstalled,
    Same,
    /// The build is newer than what the card has.
    Newer,
    /// The build is older than what the card has.
    Older,
    /// Installed, but the two cannot be ordered (no scheme matches one of them) and the versions differ.
    Different,
}

/// A core on the card, as far as builds care.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Installed {
    pub id: String,
    pub shortname: String,
    pub version: String,
}

fn placement_of(c: &Installed, schemes: &[Scheme]) -> Option<Placement> {
    classify(
        schemes,
        &Subject {
            core_id: &c.id,
            shortname: &c.shortname,
            version: &c.version,
            file: "",
        },
    )
}

/// The build against the card's copy of the same core id.
pub fn relation(build: &BuildEntry, card: &[Installed], schemes: &[Scheme]) -> Relation {
    let Some(have) = card
        .iter()
        .find(|c| c.id.eq_ignore_ascii_case(&build.core_id))
    else {
        return Relation::NotInstalled;
    };
    if have.version == build.version {
        return Relation::Same;
    }
    match (&build.placement, placement_of(have, schemes)) {
        (Some(b), Some(h)) if b.series == h.series => match compare(b, &h) {
            Ordering::Greater => Relation::Newer,
            Ordering::Less => Relation::Older,
            Ordering::Equal => Relation::Same,
        },
        _ => Relation::Different,
    }
}

/// Installed cores that belong to the build's series and are older than it: what "remove older builds" would offer. Never the build's own core id.
pub fn older_on_card(build: &BuildEntry, card: &[Installed], schemes: &[Scheme]) -> Vec<Installed> {
    let Some(b) = &build.placement else {
        return Vec::new();
    };
    let mut v: Vec<(Placement, Installed)> = card
        .iter()
        .filter(|c| !c.id.eq_ignore_ascii_case(&build.core_id))
        .filter_map(|c| placement_of(c, schemes).map(|p| (p, c.clone())))
        .filter(|(p, _)| p.series == b.series && compare(p, b) == Ordering::Less)
        .collect();
    v.sort_by(|x, y| compare(&x.0, &y.0));
    v.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_series::builtin;

    fn folder(root: &Path, name: &str, short: &str, version: &str) {
        let d = root
            .join(name)
            .join("Cores")
            .join(format!("alfatreze.{short}"));
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join("core.json"),
            format!(r#"{{"core":{{"metadata":{{"author":"alfatreze","shortname":"{short}","version":"{version}","date_release":"2026-10-01"}}}}}}"#),
        )
        .unwrap();
        fs::write(d.join("bitstream.rbf_r"), b"x").unwrap();
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("omega-builds-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn inst(id: &str, short: &str, ver: &str) -> Installed {
        Installed {
            id: id.into(),
            shortname: short.into(),
            version: ver.into(),
        }
    }

    #[test]
    fn a_build_folder_is_listed_newest_first_per_series_with_junk_and_bad_entries_handled() {
        let d = tmp("scan");
        folder(&d, "b9", "TAU DEV 9", "0.6.0");
        folder(&d, "b111", "TAU DEV 111", "0.6.0");
        folder(&d, "b20", "TAU DEV 20", "0.6.0");
        fs::create_dir_all(d.join(".hidden")).unwrap();
        fs::write(d.join("notes.txt"), b"x").unwrap();
        fs::write(d.join("broken.zip"), b"not a zip").unwrap();
        fs::create_dir_all(d.join("empty")).unwrap();
        let list = scan(&d, &builtin()).unwrap();
        let names: Vec<&str> = list.iter().map(|e| e.shortname.as_str()).collect();
        assert_eq!(
            &names[..3],
            ["TAU DEV 111", "TAU DEV 20", "TAU DEV 9"],
            "newest first, by number not by text"
        );
        let broken = list.iter().find(|e| e.file == "broken.zip").unwrap();
        assert!(broken.error.is_some() && broken.placement.is_none());
        assert_eq!(
            list.len(),
            4,
            "notes, the hidden folder and the empty folder are not builds"
        );
        assert_eq!(
            list.last().unwrap().file,
            "broken.zip",
            "unplaced entries come last"
        );
    }

    #[test]
    fn relation_to_the_card_and_older_builds_follow_the_series() {
        let d = tmp("rel");
        folder(&d, "b111", "TAU DEV 111", "0.6.0");
        let s = builtin();
        let build = scan(&d, &s).unwrap().remove(0);
        let card = vec![
            inst("alfatreze.TAU DEV 99", "TAU DEV 99", "0.6.0"),
            inst("alfatreze.TAU DEV 105", "TAU DEV 105", "0.6.0"),
            inst("alfatreze.TAU DEV 112", "TAU DEV 112", "0.6.0"),
            inst("alfatreze.TAU", "TAU", "0.6.0"),
            inst("someone.Other", "Other 3", "1.0.0"),
        ];
        assert_eq!(relation(&build, &card, &s), Relation::NotInstalled);
        let older: Vec<String> = older_on_card(&build, &card, &s)
            .into_iter()
            .map(|c| c.shortname)
            .collect();
        assert_eq!(
            older,
            ["TAU DEV 99", "TAU DEV 105"],
            "older first; 112 is newer and TAU is another series"
        );
        let mut same = card.clone();
        same.push(inst("alfatreze.TAU DEV 111", "TAU DEV 111", "0.6.0"));
        assert_eq!(relation(&build, &same, &s), Relation::Same);
    }

    #[test]
    fn the_same_core_id_compares_by_version_in_either_direction() {
        let mut b = entry_base(Path::new("x.zip"), "zip");
        b.core_id = "a.Core".into();
        b.shortname = "Core".into();
        b.version = "0.6.0-alpha.4".into();
        let s = builtin();
        place(&mut b, &s);
        assert_eq!(
            relation(&b, &[inst("a.Core", "Core", "0.6.0-alpha.3")], &s),
            Relation::Newer
        );
        assert_eq!(
            relation(&b, &[inst("a.Core", "Core", "0.6.0")], &s),
            Relation::Older
        );
        assert_eq!(
            relation(&b, &[inst("a.Core", "Core", "weird")], &s),
            Relation::Different
        );
        assert_eq!(
            relation(&b, &[inst("A.CORE", "Core", "0.6.0-alpha.4")], &s),
            Relation::Same
        );
    }
}
