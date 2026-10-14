//! Vetting a third-party core zip BEFORE anything is decompressed or planned: only the zip's central directory
//! (names, sizes, flags) and its `core.json` are read. A zip that fails is refused with one plain reason.
//!
//! What a core package may contain: `Cores/<id>/...`, `Assets/<platform>/...`, `Platforms/...` and nothing else at
//! the top; one core folder, named exactly as the catalog says and matching `core.json`'s `author.shortname`; no
//! absolute or parent-relative paths, no symlinks, no names the card's file system cannot hold, bounded size.

use crate::{ErrorCode, TauError};
use serde_json::Value;
use std::{collections::BTreeSet, fs, io::Read, path::Path};

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// The zip file itself.
    pub max_zip_bytes: u64,
    /// Sum of the uncompressed sizes it declares.
    pub max_total_bytes: u64,
    pub max_entries: usize,
    /// A big entry that shrinks by more than this factor is refused as a zip bomb.
    pub max_ratio: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_zip_bytes: 200 << 20,
            max_total_bytes: 600 << 20,
            max_entries: 5000,
            max_ratio: 200,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vetted {
    pub core_id: String,
    pub version: String,
    pub date_release: String,
    pub platform_ids: Vec<String>,
    /// Minimum Pocket firmware (`framework.version_required`), empty if the core does not say.
    pub firmware_required: String,
    pub dock_supported: Option<bool>,
    pub files: usize,
    pub uncompressed_bytes: u64,
    /// Entries under `Assets/` (media folders the core brings).
    pub asset_files: usize,
}

fn refuse(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InstallRefused, msg)
}

/// A path inside the zip as it would land under the card root; `None` when it must not be written.
fn safe_path(name: &str) -> Option<String> {
    if name.is_empty() || name.contains(['\0', '\\', ':']) || name.starts_with('/') {
        return None;
    }
    let trimmed = name.trim_end_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    let bad = parts.iter().any(|p| {
        p.is_empty()
            || *p == "."
            || *p == ".."
            || p.len() > 120
            || p.ends_with(' ')
            || p.ends_with('.')
    });
    (!bad).then(|| trimmed.to_string())
}

/// Vets `zip_path` for the catalog core `expected_id`.
pub fn vet_zip(zip_path: &Path, expected_id: &str, limits: Limits) -> Result<Vetted, TauError> {
    let len = fs::metadata(zip_path)?.len();
    if len > limits.max_zip_bytes {
        return Err(refuse(format!(
            "the download is {} MB, over the {} MB limit",
            len >> 20,
            limits.max_zip_bytes >> 20
        )));
    }
    let mut archive = zip::ZipArchive::new(fs::File::open(zip_path)?)
        .map_err(|e| refuse(format!("this is not a readable zip: {e}")))?;
    if archive.is_empty() || archive.len() > limits.max_entries {
        return Err(refuse(format!(
            "the zip has {} entries (allowed 1 to {})",
            archive.len(),
            limits.max_entries
        )));
    }
    let mut total = 0u64;
    let (mut files, mut asset_files) = (0usize, 0usize);
    let mut core_json_index = None;
    let mut seen = BTreeSet::new();
    let want_prefix = format!("Cores/{expected_id}/");
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| refuse(e.to_string()))?;
        let name = entry.name().to_string();
        let path = safe_path(&name)
            .ok_or_else(|| refuse(format!("the zip contains an unsafe path: {name:?}")))?;
        if entry
            .unix_mode()
            .is_some_and(|m| m & 0o170_000 == 0o120_000)
        {
            return Err(refuse(format!("the zip contains a symbolic link: {path}")));
        }
        let top = path.split('/').next().unwrap_or_default();
        if !matches!(top, "Cores" | "Assets" | "Platforms") {
            return Err(refuse(format!(
                "the zip contains {path:?}, which is outside Cores, Assets and Platforms"
            )));
        }
        // Names that differ only by case would collide on the card's file system.
        if !seen.insert(path.to_ascii_lowercase()) {
            return Err(refuse(format!(
                "the zip has two entries that differ only by case: {path}"
            )));
        }
        if top == "Cores" {
            let folder = path.split('/').nth(1);
            if folder.is_some() && folder != Some(expected_id) {
                return Err(refuse(format!(
                    "the zip holds a core folder named {:?}, but the catalog entry is {expected_id:?}",
                    folder.unwrap_or_default()
                )));
            }
        }
        if entry.is_dir() {
            continue;
        }
        files += 1;
        if top == "Assets" {
            asset_files += 1;
        }
        total = total.saturating_add(entry.size());
        if entry.size() > (1 << 20)
            && entry.size() / entry.compressed_size().max(1) > limits.max_ratio
        {
            return Err(refuse(format!(
                "{path} expands by more than {}x: refusing a possible zip bomb",
                limits.max_ratio
            )));
        }
        if path == format!("{want_prefix}core.json") {
            core_json_index = Some(i);
        }
    }
    if total > limits.max_total_bytes {
        return Err(refuse(format!(
            "the zip unpacks to {} MB, over the {} MB limit",
            total >> 20,
            limits.max_total_bytes >> 20
        )));
    }
    let i =
        core_json_index.ok_or_else(|| refuse(format!("the zip has no {want_prefix}core.json")))?;
    let mut raw = Vec::new();
    let mut entry = archive.by_index(i).map_err(|e| refuse(e.to_string()))?;
    if entry.size() > (1 << 20) {
        return Err(refuse("core.json is larger than 1 MB"));
    }
    entry.read_to_end(&mut raw)?;
    let json: Value = serde_json::from_slice(&raw)
        .map_err(|e| refuse(format!("core.json is not valid JSON: {e}")))?;
    let core = json
        .get("core")
        .ok_or_else(|| refuse("core.json has no \"core\" section"))?;
    let meta = core
        .get("metadata")
        .ok_or_else(|| refuse("core.json has no metadata"))?;
    let s = |v: &Value, k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let id = format!("{}.{}", s(meta, "author"), s(meta, "shortname"));
    if !id.eq_ignore_ascii_case(expected_id) {
        return Err(refuse(format!(
            "core.json says {id:?}, but the catalog entry is {expected_id:?}"
        )));
    }
    let framework = core.get("framework").unwrap_or(&Value::Null);
    Ok(Vetted {
        core_id: expected_id.to_string(),
        version: s(meta, "version"),
        date_release: s(meta, "date_release"),
        platform_ids: meta
            .get("platform_ids")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str().map(str::to_string))
            .collect(),
        firmware_required: s(framework, "version_required"),
        dock_supported: framework
            .get("dock")
            .and_then(|d| d.get("supported"))
            .and_then(Value::as_bool),
        files,
        uncompressed_bytes: total,
        asset_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    const CORE_JSON: &str = r#"{"core":{"magic":"APF_VER_1","metadata":{"platform_ids":["ardu"],"shortname":"Arduboy","author":"agg23","version":"0.9.0","date_release":"2022-09-03"},"framework":{"version_required":"1.1","dock":{"supported":true}}}}"#;

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "tau-corepkg-{name}-{}-{}",
            std::process::id(),
            crate::test_uniq()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// Builds a zip from (name, bytes, unix mode) triples.
    fn make(dir: &Path, entries: &[(&str, &[u8], u32)]) -> std::path::PathBuf {
        let path = dir.join("c.zip");
        let mut w = ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, data, mode) in entries {
            let o = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(*mode);
            if *mode == 0o120_777 {
                w.add_symlink(*name, std::str::from_utf8(data).unwrap(), o)
                    .unwrap();
            } else if name.ends_with('/') {
                w.add_directory(*name, o).unwrap();
            } else {
                w.start_file(*name, o).unwrap();
                w.write_all(data).unwrap();
            }
        }
        w.finish().unwrap();
        path
    }

    fn good(extra: &[(&str, &[u8], u32)]) -> Vec<(&'static str, Vec<u8>, u32)> {
        let mut v: Vec<(&'static str, Vec<u8>, u32)> = vec![
            (
                "Cores/agg23.Arduboy/core.json",
                CORE_JSON.as_bytes().to_vec(),
                0o644,
            ),
            ("Cores/agg23.Arduboy/arduboy.rev", vec![1; 64], 0o644),
            ("Assets/ardu/common/.gitkeep", vec![], 0o644),
            ("Platforms/ardu.json", b"{}".to_vec(), 0o644),
        ];
        for (n, d, m) in extra {
            v.push((Box::leak(n.to_string().into_boxed_str()), d.to_vec(), *m));
        }
        v
    }

    fn run(entries: Vec<(&'static str, Vec<u8>, u32)>, limits: Limits) -> Result<Vetted, TauError> {
        let d = scratch("t");
        let refs: Vec<(&str, &[u8], u32)> = entries
            .iter()
            .map(|(n, b, m)| (*n, b.as_slice(), *m))
            .collect();
        let zip = make(&d, &refs);
        let r = vet_zip(&zip, "agg23.Arduboy", limits);
        fs::remove_dir_all(&d).ok();
        r
    }

    fn reason(r: Result<Vetted, TauError>) -> String {
        r.expect_err("must be refused").to_string()
    }

    #[test]
    fn a_normal_core_zip_passes_and_reports_facts() {
        let v = run(good(&[]), Limits::default()).unwrap();
        assert_eq!(v.core_id, "agg23.Arduboy");
        assert_eq!(v.version, "0.9.0");
        assert_eq!(v.firmware_required, "1.1");
        assert_eq!(v.dock_supported, Some(true));
        assert_eq!(v.platform_ids, ["ardu"]);
        assert_eq!((v.files, v.asset_files), (4, 1));
    }

    #[test]
    fn hostile_paths_are_refused() {
        for bad in [
            "../evil.bin",
            "Cores/../../x",
            "/etc/passwd",
            "Assets\\x",
            "C:/x",
            "Assets/a/../../b",
            "Assets/trail./x",
            "Assets//x",
        ] {
            assert!(
                run(good(&[(bad, b"x", 0o644)]), Limits::default()).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn stray_top_level_other_core_symlink_and_case_collision_are_refused() {
        assert!(
            reason(run(
                good(&[("run.sh", b"#!/bin/sh", 0o755)]),
                Limits::default()
            ))
            .contains("outside Cores")
        );
        assert!(
            reason(run(
                good(&[("Cores/other.Core/core.json", b"{}", 0o644)]),
                Limits::default()
            ))
            .contains("catalog entry")
        );
        assert!(
            reason(run(
                good(&[("Assets/ardu/common/link", b"/etc", 0o120_777)]),
                Limits::default()
            ))
            .contains("symbolic link")
        );
        assert!(
            reason(run(
                good(&[("assets/ardu/common/.GITKEEP", b"", 0o644)]),
                Limits::default()
            ))
            .contains("outside Cores")
        );
        assert!(
            reason(run(
                good(&[("Assets/ardu/common/.GitKeep", b"", 0o644)]),
                Limits::default()
            ))
            .contains("differ only by case")
        );
    }

    #[test]
    fn identity_must_match_catalog_and_core_json_must_exist() {
        let wrong = CORE_JSON.replace("Arduboy", "Other");
        let mut v = good(&[]);
        v[0].1 = wrong.into_bytes();
        assert!(reason(run(v, Limits::default())).contains("core.json says"));
        let mut v = good(&[]);
        v.remove(0);
        assert!(reason(run(v, Limits::default())).contains("no Cores/agg23.Arduboy/core.json"));
        let mut v = good(&[]);
        v[0].1 = b"not json".to_vec();
        assert!(reason(run(v, Limits::default())).contains("not valid JSON"));
    }

    #[test]
    fn size_entry_and_ratio_limits() {
        let small = Limits {
            max_total_bytes: 100,
            ..Limits::default()
        };
        assert!(
            reason(run(
                good(&[("Assets/ardu/common/big.bin", &[7u8; 4096], 0o644)]),
                small
            ))
            .contains("unpacks to")
        );
        let few = Limits {
            max_entries: 3,
            ..Limits::default()
        };
        assert!(reason(run(good(&[]), few)).contains("entries"));
        let tiny = Limits {
            max_zip_bytes: 10,
            ..Limits::default()
        };
        assert!(reason(run(good(&[]), tiny)).contains("over the"));
        // 2 MB of zeros deflates by far more than 200x
        let bomb = vec![0u8; 2 << 20];
        assert!(
            reason(run(
                good(&[("Assets/ardu/common/bomb.bin", &bomb, 0o644)]),
                Limits::default()
            ))
            .contains("zip bomb")
        );
    }

    #[test]
    fn not_a_zip_is_refused() {
        let d = scratch("nz");
        let p = d.join("x.zip");
        fs::write(&p, b"PK nope").unwrap();
        assert!(vet_zip(&p, "agg23.Arduboy", Limits::default()).is_err());
        fs::remove_dir_all(&d).ok();
    }
}
