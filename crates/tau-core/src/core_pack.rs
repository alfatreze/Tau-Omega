//! Turning what the user gave into a package zip that `core_package::vet_zip` can check, and checking a zip that holds an unknown core.
//!
//! * A **package folder** (has `Cores/`) or a **bare core folder** (`core.json` at its root, wrapped as `Cores/<author.shortname>/`) is packed into a
//!   zip in the app's cache; one level of wrapper folder (what unzipping a download leaves) is looked through. A source repository is refused with the
//!   reason. The user's folder is only read.
//! * [`vet_any`] takes a zip whose core id is not known in advance (no catalog): the id is the single folder under `Cores/`, and a package that carries
//!   user data (`Settings/`, `Saves/`, `Memories/`) or several cores is refused with its own reason before the shared vetting runs.
//!
//! One code path for vetting and installing: everything after this module sees a zip.

use crate::{
    ErrorCode, TauError,
    core_package::{Limits, Vetted, vet_zip},
};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

fn refuse(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InstallRefused, msg)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Layout {
    /// The card-root layout: `Cores/<id>/...`, optional `Assets/`, `Platforms/`.
    Package,
    /// One core's own folder: `core.json` at the top.
    BareCore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Packed {
    pub zip: PathBuf,
    pub layout: Layout,
    pub core_id: String,
    pub files: usize,
    pub bytes: u64,
    /// The inner folder that was used when the chosen folder only wrapped it.
    pub descended_into: Option<String>,
}

/// Names that macOS and Windows scatter into folders; never part of a core.
fn junk(name: &str) -> bool {
    name == ".DS_Store" || name == "Thumbs.db" || name == "__MACOSX" || name.starts_with("._")
}

fn entries(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| (e.file_name().to_string_lossy().to_string(), e.path()))
        .filter(|(n, _)| !junk(n))
        .collect();
    v.sort();
    v
}

fn core_json_id(path: &Path) -> Result<String, TauError> {
    let raw = fs::read(path)?;
    let json: Value = serde_json::from_slice(&raw)
        .map_err(|e| refuse(format!("core.json is not valid JSON: {e}")))?;
    let meta = json
        .pointer("/core/metadata")
        .ok_or_else(|| refuse("core.json has no core metadata"))?;
    let text = |k: &str| {
        meta.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let (author, short) = (text("author"), text("shortname"));
    if author.is_empty() || short.is_empty() {
        return Err(refuse("core.json has no author or no shortname"));
    }
    Ok(format!("{author}.{short}"))
}

fn classify_dir(dir: &Path) -> Result<(Layout, String), TauError> {
    let items = entries(dir);
    let has = |n: &str| items.iter().any(|(name, _)| name == n);
    if has("Cores") {
        let cores = dir.join("Cores");
        let mut ids = Vec::new();
        for (name, p) in entries(&cores) {
            if p.is_dir() && p.join("core.json").is_file() {
                ids.push(name);
            }
        }
        return match ids.len() {
            0 => Err(refuse(
                "The Cores folder holds no core (no core.json inside a core folder).",
            )),
            1 => Ok((Layout::Package, ids.remove(0))),
            n => Err(refuse(format!(
                "This folder holds {n} cores. Add them one at a time."
            ))),
        };
    }
    if dir.join("core.json").is_file() {
        return Ok((Layout::BareCore, core_json_id(&dir.join("core.json"))?));
    }
    if has(".git")
        || items.iter().any(|(n, _)| {
            let l = n.to_ascii_lowercase();
            l.ends_with(".qsf") || l.ends_with(".qpf") || l == "src" || l.ends_with(".sv")
        })
    {
        return Err(refuse(
            "This looks like a source repository, not a built core. Use the release download, or the folder that contains core.json.",
        ));
    }
    Err(refuse(
        "This folder is not a core: it has no Cores folder and no core.json.",
    ))
}

/// Looks through one wrapper folder (what unzipping a download usually leaves).
fn locate(dir: &Path) -> Result<(PathBuf, Layout, String, Option<String>), TauError> {
    match classify_dir(dir) {
        Ok((l, id)) => Ok((dir.to_path_buf(), l, id, None)),
        Err(first) => {
            let subdirs: Vec<(String, PathBuf)> = entries(dir)
                .into_iter()
                .filter(|(_, p)| p.is_dir())
                .collect();
            if subdirs.len() == 1 && entries(dir).len() == 1 {
                let (name, p) = &subdirs[0];
                if let Ok((l, id)) = classify_dir(p) {
                    return Ok((p.clone(), l, id, Some(name.clone())));
                }
            }
            Err(first)
        }
    }
}

struct Item {
    rel: String,
    path: PathBuf,
    size: u64,
}

fn walk(root: &Path, prefix: &str, out: &mut Vec<Item>) -> Result<(), TauError> {
    for (name, p) in entries(root) {
        let meta = fs::symlink_metadata(&p)?;
        if meta.file_type().is_symlink() {
            return Err(refuse(format!(
                "{name} is a symbolic link; links are not accepted."
            )));
        }
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if meta.is_dir() {
            walk(&p, &rel, out)?;
        } else if meta.is_file() {
            out.push(Item {
                rel,
                path: p,
                size: meta.len(),
            });
        }
    }
    Ok(())
}

/// Packs `dir` (a package folder or a bare core folder) into `out_dir/<id>.zip`.
pub fn pack_folder(dir: &Path, out_dir: &Path, limits: Limits) -> Result<Packed, TauError> {
    let (root, layout, core_id, descended_into) = locate(dir)?;
    let mut items = Vec::new();
    match layout {
        Layout::Package => {
            for (name, p) in entries(&root) {
                match name.as_str() {
                    "Cores" | "Assets" | "Platforms" => {
                        if p.is_dir() {
                            walk(&p, &name, &mut items)?;
                        }
                    }
                    "Settings" | "Saves" | "Memories" => {
                        return Err(refuse(format!(
                            "The folder has a {name} folder. A core package must not carry user data."
                        )));
                    }
                    other => {
                        return Err(refuse(format!(
                            "The folder has {other:?} beside Cores, Assets and Platforms. Remove it, or choose the folder that holds only those."
                        )));
                    }
                }
            }
        }
        Layout::BareCore => {
            let mut inner = Vec::new();
            walk(&root, "", &mut inner)?;
            for mut it in inner {
                it.rel = format!("Cores/{core_id}/{}", it.rel);
                items.push(it);
            }
        }
    }
    if items.is_empty() || items.len() > limits.max_entries {
        return Err(refuse(format!(
            "The folder has {} files (allowed 1 to {}).",
            items.len(),
            limits.max_entries
        )));
    }
    let total: u64 = items.iter().map(|i| i.size).sum();
    if total > limits.max_total_bytes {
        return Err(refuse(format!(
            "The folder is {} MB, over the {} MB limit.",
            total >> 20,
            limits.max_total_bytes >> 20
        )));
    }
    fs::create_dir_all(out_dir)?;
    let safe: String = core_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let zip_path = out_dir.join(format!("{safe}.zip"));
    let mut w = ZipWriter::new(fs::File::create(&zip_path)?);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut buf = Vec::new();
    for it in &items {
        w.start_file(it.rel.as_str(), opts)
            .map_err(|e| refuse(e.to_string()))?;
        buf.clear();
        fs::File::open(&it.path)?.read_to_end(&mut buf)?;
        w.write_all(&buf)?;
    }
    w.finish().map_err(|e| refuse(e.to_string()))?;
    Ok(Packed {
        zip: zip_path,
        layout,
        core_id,
        files: items.len(),
        bytes: total,
        descended_into,
    })
}

/// The single core folder a zip holds. Refuses a zip with user data, no core, or several cores, each with its own reason.
pub fn zip_core_id(zip_path: &Path) -> Result<String, TauError> {
    let mut archive = zip::ZipArchive::new(fs::File::open(zip_path)?)
        .map_err(|e| refuse(format!("This is not a readable zip: {e}")))?;
    let mut ids = BTreeSet::new();
    for i in 0..archive.len() {
        let name = archive
            .by_index(i)
            .map_err(|e| refuse(e.to_string()))?
            .name()
            .to_string();
        let mut parts = name.trim_end_matches('/').split('/');
        match parts.next() {
            Some("Settings" | "Saves" | "Memories") => {
                return Err(refuse(
                    "The zip carries user data (Settings, Saves or Memories). A core package must not.",
                ));
            }
            Some("Cores") => {
                if let Some(id) = parts.next().filter(|p| !p.is_empty()) {
                    ids.insert(id.to_string());
                }
            }
            _ => {}
        }
    }
    let mut ids: Vec<String> = ids.into_iter().collect();
    match ids.len() {
        0 => Err(refuse("The zip has no core in a Cores folder.")),
        1 => Ok(ids.remove(0)),
        n => Err(refuse(format!(
            "The zip holds {n} cores. Add them one at a time."
        ))),
    }
}

/// Vets a zip whose core is not known in advance (a manual add).
pub fn vet_any(zip_path: &Path, limits: Limits) -> Result<Vetted, TauError> {
    let id = zip_core_id(zip_path)?;
    vet_zip(zip_path, &id, limits)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORE_JSON: &str = r#"{"core":{"magic":"APF_VER_1","metadata":{"platform_ids":["ardu"],"shortname":"Arduboy","author":"agg23","version":"0.9.0","date_release":"2022-09-03"},"framework":{"version_required":"1.1","dock":{"supported":true}}}}"#;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "tau-pack-{name}-{}-{}",
            std::process::id(),
            crate::test_uniq()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(root: &Path, rel: &str, data: &[u8]) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }

    fn package(root: &Path) {
        put(root, "Cores/agg23.Arduboy/core.json", CORE_JSON.as_bytes());
        put(root, "Cores/agg23.Arduboy/arduboy.rev", &[1; 64]);
        put(root, "Platforms/ardu.json", b"{}");
        put(root, "Assets/ardu/common/.gitkeep", b"");
    }

    #[test]
    fn a_package_folder_becomes_a_zip_that_vets() {
        let (src, out) = (scratch("p"), scratch("o"));
        package(&src);
        put(&src, ".DS_Store", b"junk");
        put(&src, "Cores/agg23.Arduboy/._core.json", b"junk");
        let p = pack_folder(&src, &out, Limits::default()).unwrap();
        assert_eq!(
            (p.layout, p.core_id.as_str(), p.files),
            (Layout::Package, "agg23.Arduboy", 4)
        );
        let v = vet_any(&p.zip, Limits::default()).unwrap();
        assert_eq!(
            (v.core_id.as_str(), v.version.as_str()),
            ("agg23.Arduboy", "0.9.0")
        );
    }

    #[test]
    fn a_bare_core_folder_is_wrapped_and_one_wrapper_level_is_looked_through() {
        let (src, out) = (scratch("b"), scratch("o2"));
        put(&src, "core.json", CORE_JSON.as_bytes());
        put(&src, "arduboy.rev", &[1; 8]);
        put(&src, "icon.bin", &[0; 8]);
        let p = pack_folder(&src, &out, Limits::default()).unwrap();
        assert_eq!((p.layout, p.files), (Layout::BareCore, 3));
        assert_eq!(
            vet_any(&p.zip, Limits::default()).unwrap().core_id,
            "agg23.Arduboy"
        );
        // an unzipped download: one wrapper folder
        let wrap = scratch("w");
        put(&wrap, "agg23.Arduboy-0.9.0/core.json", CORE_JSON.as_bytes());
        let p = pack_folder(&wrap, &out, Limits::default()).unwrap();
        assert_eq!(p.descended_into.as_deref(), Some("agg23.Arduboy-0.9.0"));
    }

    #[test]
    fn folders_that_are_not_a_core_are_refused_with_their_reason() {
        let out = scratch("o3");
        let repo = scratch("r");
        put(&repo, ".git/HEAD", b"x");
        put(&repo, "src/fpga/core_top.v", b"x");
        let e = pack_folder(&repo, &out, Limits::default())
            .unwrap_err()
            .message;
        assert!(e.contains("source repository"), "{e}");
        let empty = scratch("e");
        assert!(
            pack_folder(&empty, &out, Limits::default())
                .unwrap_err()
                .message
                .contains("not a core")
        );
        let extra = scratch("x");
        package(&extra);
        put(&extra, "README.md", b"hi");
        assert!(
            pack_folder(&extra, &out, Limits::default())
                .unwrap_err()
                .message
                .contains("README.md")
        );
        let data = scratch("d");
        package(&data);
        put(&data, "Settings/agg23.Arduboy/x.json", b"{}");
        assert!(
            pack_folder(&data, &out, Limits::default())
                .unwrap_err()
                .message
                .contains("user data")
        );
        let two = scratch("t");
        package(&two);
        put(&two, "Cores/other.Core/core.json", CORE_JSON.as_bytes());
        assert!(
            pack_folder(&two, &out, Limits::default())
                .unwrap_err()
                .message
                .contains("2 cores")
        );
    }

    #[test]
    fn a_symlink_and_an_oversize_folder_are_refused() {
        let (src, out) = (scratch("s"), scratch("o4"));
        package(&src);
        std::os::unix::fs::symlink("/etc/hosts", src.join("Cores/agg23.Arduboy/link")).unwrap();
        assert!(
            pack_folder(&src, &out, Limits::default())
                .unwrap_err()
                .message
                .contains("symbolic link")
        );
        let big = scratch("big");
        package(&big);
        let tiny = Limits {
            max_total_bytes: 10,
            ..Limits::default()
        };
        assert!(
            pack_folder(&big, &out, tiny)
                .unwrap_err()
                .message
                .contains("limit")
        );
    }

    #[test]
    fn a_zip_with_user_data_or_several_cores_is_refused() {
        let out = scratch("o5");
        let w = |name: &str, files: &[(&str, &[u8])]| {
            let p = out.join(name);
            let mut z = ZipWriter::new(fs::File::create(&p).unwrap());
            for (n, d) in files {
                z.start_file(*n, SimpleFileOptions::default()).unwrap();
                z.write_all(d).unwrap();
            }
            z.finish().unwrap();
            p
        };
        let data = w(
            "d.zip",
            &[
                ("Cores/agg23.Arduboy/core.json", CORE_JSON.as_bytes()),
                ("Saves/x/a.sav", b"x"),
            ],
        );
        assert!(
            zip_core_id(&data)
                .unwrap_err()
                .message
                .contains("user data")
        );
        let two = w(
            "t.zip",
            &[
                ("Cores/a.B/core.json", b"{}"),
                ("Cores/c.D/core.json", b"{}"),
            ],
        );
        assert!(zip_core_id(&two).unwrap_err().message.contains("2 cores"));
        let none = w("n.zip", &[("Assets/x/y", b"x")]);
        assert!(zip_core_id(&none).unwrap_err().message.contains("no core"));
        let ok = w(
            "o.zip",
            &[("Cores/agg23.Arduboy/core.json", CORE_JSON.as_bytes())],
        );
        assert_eq!(
            vet_any(&ok, Limits::default()).unwrap().core_id,
            "agg23.Arduboy"
        );
    }
}
