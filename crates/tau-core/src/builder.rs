//! The Builder (`docs/DEV_LOOP_SPEC.md`, section 4): a raw Quartus bitstream plus a template core becomes an installable package, without Python. It never
//! writes to a card; the zip goes to a folder the developer chooses (usually the build folder the Builds page reads).
//!
//! What it does, in the order it does it: refuses a file that is not a raw `.rbf` (a `.sof`, an already converted `.rbf_r`, an empty or blank file, or one
//! whose hash a previous build record already lists as converted: converting twice is the "Bridge not responding" mistake); reads the template core's own
//! files (an installed core, a package zip, or a package or core folder); sets the new shortname, version, release date and (optionally) description in
//! `core.json`; reverses the bits of every byte of the bitstream once (the Pocket's `.rbf_r`); optionally replaces `icon.bin` from a 36x36 PNG and adds
//! extra files (a firmware ROM, data) under `Cores/<id>/`, `Assets/` or `Platforms/`; runs the core checker on the finished package and refuses to write it
//! while that finds an Error; and writes a build record (every hash, the inputs) beside the zip.

use crate::{
    ErrorCode, TauError,
    core_lint::{self, LintReport},
    core_pack,
    core_package::Limits,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Largest bitstream accepted (a Cyclone V E A4 raw file is about 4 MB).
pub const MAX_RBF: u64 = 64 << 20;
/// Largest single extra or template file.
pub const MAX_FILE: u64 = 64 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", rename_all = "snake_case"))]
pub enum TemplateSource {
    /// A core installed on a card (a disabled core too).
    Card { card: String, id: String },
    /// A package zip.
    Zip { path: String },
    /// A package folder or a core folder.
    Folder { path: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtraFile {
    pub src: String,
    /// Where it goes in the package, relative to the package root: `Cores/<id>/…`, `Assets/…` or `Platforms/…`. `{id}` stands for the new core id.
    pub dest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BuildSpec {
    pub rbf: String,
    pub template: TemplateSource,
    pub shortname: String,
    pub version: String,
    /// `YYYY-MM-DD`.
    pub date_release: String,
    pub description: Option<String>,
    pub icon_png: Option<String>,
    pub extra: Vec<ExtraFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlanFile {
    pub rel: String,
    pub bytes: u64,
    /// `bitstream`, `template`, `generated` (core.json), `icon` or `extra`.
    pub origin: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BuildPlan {
    pub core_id: String,
    pub files: Vec<PlanFile>,
    pub raw_sha256: String,
    pub converted_sha256: String,
    pub bitstream_bytes: u64,
    pub lint: LintReport,
    /// Why the package would not be written (the first core-check error); `None` when it is fine.
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BuildResult {
    pub zip: String,
    pub record: String,
    pub zip_bytes: u64,
    pub zip_sha256: String,
    pub plan: BuildPlan,
}

fn refuse(m: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InstallRefused, m)
}

fn hex(b: &[u8]) -> String {
    use std::fmt::Write as _;
    b.iter().fold(String::new(), |mut s, x| {
        let _ = write!(s, "{x:02x}");
        s
    })
}

pub fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// The Pocket's `.rbf_r`: every byte with its bits in reverse order (`0b0000_0001` becomes `0b1000_0000`). Applying it twice gives the original, which is
/// why converting an already converted file silently produces a bitstream the Pocket cannot load.
pub fn reverse_bits(raw: &[u8]) -> Vec<u8> {
    raw.iter().map(|b| b.reverse_bits()).collect()
}

fn read_capped(p: &Path, cap: u64, what: &str) -> Result<Vec<u8>, TauError> {
    let meta = fs::metadata(p).map_err(|e| refuse(format!("{what} {}: {e}", p.display())))?;
    if !meta.is_file() {
        return Err(refuse(format!("{what} {} is not a file.", p.display())));
    }
    if meta.len() > cap {
        return Err(refuse(format!(
            "{what} {} is {} MB, over the {} MB limit.",
            p.display(),
            meta.len() >> 20,
            cap >> 20
        )));
    }
    let mut v = Vec::new();
    fs::File::open(p)?.read_to_end(&mut v)?;
    Ok(v)
}

/// Checks that `path` is a raw Quartus bitstream and returns its bytes. `known_converted` are SHA-256s of bitstreams earlier builds already converted.
pub fn read_raw_bitstream(path: &Path, known_converted: &[String]) -> Result<Vec<u8>, TauError> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "rbf" => {}
        "rbf_r" => {
            return Err(refuse(
                "This is already a converted bitstream (.rbf_r). Choose the raw .rbf from Quartus: converting twice makes a bitstream the Pocket cannot load.",
            ));
        }
        "sof" | "pof" => {
            return Err(refuse(
                "A .sof is the JTAG programming file. Choose the raw binary file Quartus writes to output_files (.rbf): turn on Raw Binary File in Programming Files.",
            ));
        }
        _ => {
            return Err(refuse(
                "Choose the raw .rbf file Quartus writes to output_files.",
            ));
        }
    }
    let bytes = read_capped(path, MAX_RBF, "The bitstream")?;
    if bytes.is_empty() {
        return Err(refuse("The bitstream is empty."));
    }
    if bytes.iter().all(|b| *b == 0) || bytes.iter().all(|b| *b == 0xFF) {
        return Err(refuse(
            "The bitstream is blank (all zeros or all ones): the build did not produce a real one.",
        ));
    }
    if known_converted.iter().any(|h| *h == sha256(&bytes)) {
        return Err(refuse(
            "This file is a bitstream an earlier build already converted (its hash is in a build record). Choose the raw .rbf.",
        ));
    }
    Ok(bytes)
}

type Files = Vec<(String, Vec<u8>)>;

struct Template {
    /// Relative to `Cores/<id>/`.
    core: Files,
    /// Relative to the package root (`Platforms/…`).
    platform: Files,
}

fn is_bitstream(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.ends_with(".rbf_r") || l.ends_with(".rbf")
}

fn from_dir(
    core_dir: &Path,
    platforms_dir: Option<&Path>,
    platform_ids: &[String],
) -> Result<Template, TauError> {
    let mut core = Vec::new();
    for (rel, p) in crate::core_files::walk(core_dir) {
        if !is_bitstream(&rel) {
            core.push((rel, read_capped(&p, MAX_FILE, "A template file")?));
        }
    }
    let mut platform = Vec::new();
    if let Some(pd) = platforms_dir {
        for id in platform_ids {
            for rel in [format!("{id}.json"), format!("_images/{id}.bin")] {
                let p = pd.join(&rel);
                if p.is_file() {
                    platform.push((
                        format!("Platforms/{rel}"),
                        read_capped(&p, MAX_FILE, "A platform file")?,
                    ));
                }
            }
        }
    }
    Ok(Template { core, platform })
}

fn from_zip(zip_path: &Path) -> Result<Template, TauError> {
    let id = core_pack::zip_core_id(zip_path)?;
    let mut a =
        zip::ZipArchive::new(fs::File::open(zip_path)?).map_err(|e| refuse(e.to_string()))?;
    let prefix = format!("Cores/{id}/");
    let (mut core, mut platform) = (Vec::new(), Vec::new());
    for i in 0..a.len() {
        let mut e = a.by_index(i).map_err(|e| refuse(e.to_string()))?;
        if !e.is_file() || e.size() > MAX_FILE {
            continue;
        }
        let name = e.name().to_string();
        let mut buf = Vec::new();
        if let Some(rel) = name.strip_prefix(&prefix) {
            if !is_bitstream(rel) && !rel.is_empty() {
                e.read_to_end(&mut buf).map_err(|e| refuse(e.to_string()))?;
                core.push((rel.to_string(), buf));
            }
        } else if name.starts_with("Platforms/") {
            e.read_to_end(&mut buf).map_err(|e| refuse(e.to_string()))?;
            platform.push((name, buf));
        }
    }
    Ok(Template { core, platform })
}

fn read_template(src: &TemplateSource, tmp: &Path) -> Result<Template, TauError> {
    match src {
        TemplateSource::Card { card, id } => {
            let card = Path::new(card);
            let dir = crate::corehub::core_dir(card, id)?;
            let ids = crate::cardlayout::core_platforms(card, id).0;
            // core_platforms reads Cores/<id>; a disabled core's platforms come from its own core.json instead
            let ids = if ids.is_empty() {
                fs::read(dir.join("core.json"))
                    .ok()
                    .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                    .map(|j| crate::cardlayout::platform_ids(&j))
                    .unwrap_or_default()
            } else {
                ids
            };
            from_dir(&dir, Some(&card.join("Platforms")), &ids)
        }
        TemplateSource::Zip { path } => from_zip(Path::new(path)),
        TemplateSource::Folder { path } => {
            fs::create_dir_all(tmp)?;
            let packed = core_pack::pack_folder(Path::new(path), tmp, Limits::default())?;
            let t = from_zip(&packed.zip);
            let _ = fs::remove_file(&packed.zip);
            t
        }
    }
}

/// `dest` must stay inside the package: under `Cores/<id>/`, `Assets/` or `Platforms/`, no `..`, and never user data.
fn check_dest(dest: &str, id: &str) -> Result<String, TauError> {
    let d = dest.replace("{id}", id).replace('\\', "/");
    let bad = d.is_empty()
        || d.starts_with('/')
        || d.split('/')
            .any(|p| p.is_empty() || p == ".." || p == "." || p.starts_with('.'))
        || d.contains('\0');
    if bad {
        return Err(refuse(format!(
            "{dest:?} is not a place inside the package."
        )));
    }
    let ok = d.starts_with(&format!("Cores/{id}/"))
        || d.starts_with("Assets/")
        || d.starts_with("Platforms/");
    if !ok {
        return Err(refuse(format!(
            "{dest:?}: extra files go under Cores/{id}/, Assets/ or Platforms/ (never Saves, Settings or Memories)."
        )));
    }
    Ok(d)
}

struct Assembled {
    core_id: String,
    files: BTreeMap<String, (Vec<u8>, &'static str)>,
    raw_sha256: String,
    converted_sha256: String,
    bitstream_bytes: u64,
}

fn assemble(
    spec: &BuildSpec,
    known_converted: &[String],
    tmp: &Path,
) -> Result<Assembled, TauError> {
    let raw = read_raw_bitstream(Path::new(&spec.rbf), known_converted)?;
    let template = read_template(&spec.template, tmp)?;
    let core_json = template
        .core
        .iter()
        .find(|(n, _)| n == "core.json")
        .ok_or_else(|| refuse("The template has no core.json."))?;
    let mut json: serde_json::Value = serde_json::from_slice(&core_json.1)
        .map_err(|e| refuse(format!("The template's core.json is not valid JSON: {e}")))?;
    let name = spec.shortname.trim();
    if name.is_empty() {
        return Err(refuse("Give the build a shortname."));
    }
    let meta = json
        .pointer_mut("/core/metadata")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| refuse("The template's core.json has no core.metadata."))?;
    let author = meta
        .get("author")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if author.is_empty() {
        return Err(refuse(
            "The template's core.json has no author, so the core id cannot be made.",
        ));
    }
    meta.insert("shortname".into(), name.into());
    meta.insert("version".into(), spec.version.trim().into());
    meta.insert("date_release".into(), spec.date_release.trim().into());
    if let Some(d) = &spec.description {
        meta.insert("description".into(), d.trim().into());
    }
    let id = format!("{author}.{name}");
    let bitstream_name = json
        .pointer("/core/cores/0/filename")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("bitstream.rbf_r")
        .to_string();
    if !bitstream_name.to_ascii_lowercase().ends_with(".rbf_r")
        || bitstream_name.contains(['/', '\\'])
    {
        return Err(refuse(format!(
            "core.json names the bitstream {bitstream_name:?}; it must be a plain .rbf_r file name."
        )));
    }
    if json.pointer("/core/cores/0").is_none() {
        json["core"]["cores"] =
            serde_json::json!([{ "id": 0, "name": id, "filename": bitstream_name }]);
    }
    let pretty = serde_json::to_vec_pretty(&json).map_err(|e| refuse(e.to_string()))?;

    let mut files: BTreeMap<String, (Vec<u8>, &'static str)> = BTreeMap::new();
    for (rel, bytes) in template.core {
        let origin = if rel == "core.json" {
            "generated"
        } else {
            "template"
        };
        files.insert(
            format!("Cores/{id}/{rel}"),
            (
                if rel == "core.json" {
                    pretty.clone()
                } else {
                    bytes
                },
                origin,
            ),
        );
    }
    for (rel, bytes) in template.platform {
        files.insert(rel, (bytes, "template"));
    }
    let converted = reverse_bits(&raw);
    let (raw_sha256, converted_sha256) = (sha256(&raw), sha256(&converted));
    let bitstream_bytes = converted.len() as u64;
    files.insert(
        format!("Cores/{id}/{bitstream_name}"),
        (converted, "bitstream"),
    );
    if let Some(png) = &spec.icon_png {
        let bin = crate::icon::encode_icon_bin(&read_capped(Path::new(png), 4 << 20, "The icon")?)?;
        files.insert(format!("Cores/{id}/icon.bin"), (bin, "icon"));
    }
    for e in &spec.extra {
        let dest = check_dest(&e.dest, &id)?;
        if files
            .get(&dest)
            .is_some_and(|(_, o)| *o == "bitstream" || *o == "generated")
        {
            return Err(refuse(format!(
                "{dest} is made by the Builder and cannot be replaced by an extra file."
            )));
        }
        files.insert(
            dest,
            (
                read_capped(Path::new(&e.src), MAX_FILE, "An extra file")?,
                "extra",
            ),
        );
    }
    Ok(Assembled {
        core_id: id,
        files,
        raw_sha256,
        converted_sha256,
        bitstream_bytes,
    })
}

fn write_zip(path: &Path, a: &Assembled) -> Result<(), TauError> {
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};
    let mut w = ZipWriter::new(fs::File::create(path)?);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (rel, (bytes, _)) in &a.files {
        w.start_file(rel.as_str(), opts)
            .map_err(|e| refuse(e.to_string()))?;
        w.write_all(bytes)?;
    }
    w.finish().map_err(|e| refuse(e.to_string()))?;
    Ok(())
}

fn plan_of(a: &Assembled, lint: LintReport) -> BuildPlan {
    let blocked = lint
        .findings
        .iter()
        .find(|f| f.severity == core_lint::Severity::Error)
        .map(|f| format!("The core check found an error: {} {}", f.message, f.fix));
    BuildPlan {
        core_id: a.core_id.clone(),
        files: a
            .files
            .iter()
            .map(|(rel, (b, o))| PlanFile {
                rel: rel.clone(),
                bytes: b.len() as u64,
                origin: (*o).into(),
                sha256: sha256(b),
            })
            .collect(),
        raw_sha256: a.raw_sha256.clone(),
        converted_sha256: a.converted_sha256.clone(),
        bitstream_bytes: a.bitstream_bytes,
        lint,
        blocked,
    }
}

fn scratch(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-builder-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ))
}

/// Shows what would be built and what the core checker says, writing nothing but a temporary zip that is removed again.
pub fn plan(spec: &BuildSpec, known_converted: &[String]) -> Result<BuildPlan, TauError> {
    let tmp = scratch("plan");
    fs::create_dir_all(&tmp)?;
    let result = (|| {
        let a = assemble(spec, known_converted, &tmp)?;
        let z = tmp.join("check.zip");
        write_zip(&z, &a)?;
        Ok(plan_of(&a, core_lint::lint_zip(&z)?))
    })();
    let _ = fs::remove_dir_all(&tmp);
    result
}

/// Builds the package into `out_dir`: `<core id>.zip` and `<core id>.zip.build.json` (the record). Refuses while the core check finds an Error, and never
/// replaces a file that is already there.
pub fn build(
    spec: &BuildSpec,
    known_converted: &[String],
    out_dir: &Path,
    tool: &str,
    now: u64,
) -> Result<BuildResult, TauError> {
    let tmp = scratch("build");
    fs::create_dir_all(&tmp)?;
    let result = (|| {
        let a = assemble(spec, known_converted, &tmp)?;
        let safe: String = a
            .core_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        fs::create_dir_all(out_dir)?;
        let zip_path = out_dir.join(format!("{safe}.zip"));
        let record_path = out_dir.join(format!("{safe}.zip.build.json"));
        if zip_path.exists() || record_path.exists() {
            return Err(refuse(format!(
                "{} already exists in that folder. Change the shortname or choose another folder: a build is never overwritten.",
                zip_path.file_name().unwrap_or_default().to_string_lossy()
            )));
        }
        let staged = tmp.join("package.zip");
        write_zip(&staged, &a)?;
        let plan = plan_of(&a, core_lint::lint_zip(&staged)?);
        if let Some(b) = &plan.blocked {
            return Err(refuse(b.clone()));
        }
        fs::copy(&staged, &zip_path)?;
        let bytes = fs::read(&zip_path)?;
        let zip_sha256 = sha256(&bytes);
        let record = serde_json::json!({
            "tool": tool, "built_at": now, "core_id": plan.core_id, "zip": zip_path.file_name().map(|n| n.to_string_lossy()), "zip_sha256": zip_sha256,
            "bitstream": { "input": Path::new(&spec.rbf).file_name().map(|n| n.to_string_lossy()), "raw_sha256": plan.raw_sha256, "converted_sha256": plan.converted_sha256, "bytes": plan.bitstream_bytes },
            "identity": { "shortname": spec.shortname.trim(), "version": spec.version.trim(), "date_release": spec.date_release.trim() },
            "template": spec.template_label(),
            "files": plan.files.iter().map(|f| serde_json::json!({ "path": f.rel, "bytes": f.bytes, "sha256": f.sha256, "origin": f.origin })).collect::<Vec<_>>(),
            "core_check": { "errors": plan.lint.errors, "warnings": plan.lint.warnings, "notes": plan.lint.notes },
        });
        fs::write(
            &record_path,
            serde_json::to_vec_pretty(&record).map_err(|e| refuse(e.to_string()))?,
        )?;
        Ok(BuildResult {
            zip: zip_path.to_string_lossy().into_owned(),
            record: record_path.to_string_lossy().into_owned(),
            zip_bytes: bytes.len() as u64,
            zip_sha256,
            plan,
        })
    })();
    let _ = fs::remove_dir_all(&tmp);
    result
}

impl BuildSpec {
    fn template_label(&self) -> String {
        match &self.template {
            TemplateSource::Card { id, .. } => format!("installed core {id}"),
            TemplateSource::Zip { path } | TemplateSource::Folder { path } => {
                path.rsplit('/').next().unwrap_or(path).to_string()
            }
        }
    }
}

/// SHA-256s of every converted bitstream the build records in `dir` list, so a later build can refuse a file that was already converted.
pub fn known_converted(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".build.json"))
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(&fs::read(e.path()).ok()?).ok())
        .filter_map(|v| {
            v.pointer("/bitstream/converted_sha256")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corehub::testcard::{card, write};

    fn tmp(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("omega-builder-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// The fixture core has only the files its own tests needed; a real template holds all seven JSON files, so add the rest.
    fn complete(c: &Path) {
        let d = c.join("Cores/Spiritualized.GBA");
        for (n, body) in [
            ("audio.json", r#"{"audio":{"magic":"APF_VER_1"}}"#),
            (
                "video.json",
                r#"{"video":{"magic":"APF_VER_1","scaler_modes":[]}}"#,
            ),
            (
                "input.json",
                r#"{"input":{"magic":"APF_VER_1","controllers":[]}}"#,
            ),
            (
                "variants.json",
                r#"{"variants":{"magic":"APF_VER_1","variant_list":[]}}"#,
            ),
        ] {
            write(&d.join(n), body.as_bytes());
        }
    }

    fn spec(rbf: &Path, c: &Path) -> BuildSpec {
        BuildSpec {
            rbf: rbf.to_string_lossy().into(),
            template: TemplateSource::Card {
                card: c.to_string_lossy().into(),
                id: "Spiritualized.GBA".into(),
            },
            shortname: "GBA DEV 2".into(),
            version: "1.3.0-alpha.1".into(),
            date_release: "2026-10-16".into(),
            description: Some("A test build".into()),
            icon_png: None,
            extra: Vec::new(),
        }
    }

    #[test]
    fn bit_reversal_is_the_documented_conversion_and_applying_it_twice_undoes_it() {
        assert_eq!(
            reverse_bits(&[0b0000_0001, 0b1100_0000, 0x00, 0xFF, 0b1010_0101]),
            [0b1000_0000, 0b0000_0011, 0x00, 0xFF, 0b1010_0101]
        );
        let data: Vec<u8> = (0..=255).collect();
        assert_eq!(reverse_bits(&reverse_bits(&data)), data);
    }

    #[test]
    fn only_a_real_raw_rbf_is_accepted_and_each_refusal_says_why() {
        let d = tmp("refuse");
        let put = |n: &str, b: &[u8]| {
            let p = d.join(n);
            fs::write(&p, b).unwrap();
            p
        };
        assert!(
            read_raw_bitstream(&put("a.rbf_r", b"12345678"), &[])
                .unwrap_err()
                .message
                .contains("already a converted")
        );
        assert!(
            read_raw_bitstream(&put("a.sof", b"12345678"), &[])
                .unwrap_err()
                .message
                .contains("JTAG")
        );
        assert!(
            read_raw_bitstream(&put("a.bin", b"12345678"), &[])
                .unwrap_err()
                .message
                .contains("raw .rbf")
        );
        assert!(
            read_raw_bitstream(&put("empty.rbf", b""), &[])
                .unwrap_err()
                .message
                .contains("empty")
        );
        assert!(
            read_raw_bitstream(&put("zero.rbf", &[0u8; 64]), &[])
                .unwrap_err()
                .message
                .contains("blank")
        );
        let good = put("good.rbf", &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(read_raw_bitstream(&good, &[]).unwrap().len(), 8);
        let seen = sha256(&[1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(
            read_raw_bitstream(&good, &[seen])
                .unwrap_err()
                .message
                .contains("already converted")
        );
    }

    #[test]
    fn a_build_from_an_installed_core_gets_the_new_identity_and_a_converted_bitstream() {
        let c = card("builder1");
        complete(&c);
        let d = tmp("build1");
        let raw: Vec<u8> = (1..=200u8).collect();
        fs::write(d.join("in.rbf"), &raw).unwrap();
        let out = d.join("out");
        let r = build(&spec(&d.join("in.rbf"), &c), &[], &out, "test", 7).unwrap();
        assert_eq!(r.plan.core_id, "Spiritualized.GBA DEV 2");
        assert!(r.zip.ends_with("Spiritualized.GBA_DEV_2.zip"));
        let mut a = zip::ZipArchive::new(fs::File::open(&r.zip).unwrap()).unwrap();
        let mut read = |n: &str| {
            let mut v = Vec::new();
            a.by_name(n).unwrap().read_to_end(&mut v).unwrap();
            v
        };
        assert_eq!(
            read("Cores/Spiritualized.GBA DEV 2/bitstream.rbf_r"),
            reverse_bits(&raw)
        );
        let j: serde_json::Value =
            serde_json::from_slice(&read("Cores/Spiritualized.GBA DEV 2/core.json")).unwrap();
        assert_eq!(j.pointer("/core/metadata/shortname").unwrap(), "GBA DEV 2");
        assert_eq!(
            j.pointer("/core/metadata/version").unwrap(),
            "1.3.0-alpha.1"
        );
        assert_eq!(
            j.pointer("/core/metadata/description").unwrap(),
            "A test build"
        );
        assert!(
            read("Cores/Spiritualized.GBA DEV 2/data.json").len() > 10,
            "the template's other files come along"
        );
        // the record lists the hashes and a later build refuses the converted file
        let rec: serde_json::Value = serde_json::from_slice(&fs::read(&r.record).unwrap()).unwrap();
        assert_eq!(
            rec.pointer("/bitstream/raw_sha256")
                .unwrap()
                .as_str()
                .unwrap(),
            sha256(&raw)
        );
        assert_eq!(
            rec.pointer("/bitstream/converted_sha256")
                .unwrap()
                .as_str()
                .unwrap(),
            sha256(&reverse_bits(&raw))
        );
        let known = known_converted(&out);
        assert_eq!(known, vec![sha256(&reverse_bits(&raw))]);
        // the template card was not touched
        assert!(
            c.join("Cores/Spiritualized.GBA/core.json").is_file()
                && !c.join("Cores/Spiritualized.GBA DEV 2").exists()
        );
    }

    #[test]
    fn a_build_is_never_overwritten_and_extra_files_stay_inside_the_package() {
        let c = card("builder2");
        complete(&c);
        let d = tmp("build2");
        fs::write(d.join("in.rbf"), (1..=50u8).collect::<Vec<_>>()).unwrap();
        fs::write(d.join("boot.rom"), b"rom").unwrap();
        let out = d.join("out");
        let mut s = spec(&d.join("in.rbf"), &c);
        s.extra = vec![ExtraFile {
            src: d.join("boot.rom").to_string_lossy().into(),
            dest: "Assets/gba/{id}/boot.rom".into(),
        }];
        let r = build(&s, &[], &out, "test", 1).unwrap();
        assert!(
            r.plan
                .files
                .iter()
                .any(|f| f.rel == "Assets/gba/Spiritualized.GBA DEV 2/boot.rom"
                    && f.origin == "extra")
        );
        assert!(
            build(&s, &[], &out, "test", 2)
                .unwrap_err()
                .message
                .contains("never overwritten")
        );
        for bad in [
            "Saves/gba/x.sav",
            "../escape",
            "/abs",
            "Cores/other.Core/x",
            "Assets/../../x",
            "Assets/.hidden",
        ] {
            let mut t = spec(&d.join("in.rbf"), &c);
            t.shortname = "GBA DEV 9".into();
            t.extra = vec![ExtraFile {
                src: d.join("boot.rom").to_string_lossy().into(),
                dest: bad.into(),
            }];
            assert!(plan(&t, &[]).is_err(), "{bad} must be refused");
        }
        let mut t = spec(&d.join("in.rbf"), &c);
        t.extra = vec![ExtraFile {
            src: d.join("boot.rom").to_string_lossy().into(),
            dest: "Cores/{id}/core.json".into(),
        }];
        assert!(
            plan(&t, &[])
                .unwrap_err()
                .message
                .contains("made by the Builder")
        );
    }

    #[test]
    fn a_package_zip_can_be_the_template_and_a_missing_shortname_is_refused() {
        let c = card("builder3");
        complete(&c);
        let d = tmp("build3");
        fs::write(d.join("in.rbf"), (1..=50u8).collect::<Vec<_>>()).unwrap();
        let first = build(&spec(&d.join("in.rbf"), &c), &[], &d.join("out"), "t", 1).unwrap();
        let mut s = spec(&d.join("in.rbf"), &c);
        s.template = TemplateSource::Zip {
            path: first.zip.clone(),
        };
        s.shortname = "GBA DEV 3".into();
        let second = build(&s, &[], &d.join("out"), "t", 2).unwrap();
        assert_eq!(second.plan.core_id, "Spiritualized.GBA DEV 3");
        s.shortname = "  ".into();
        assert!(plan(&s, &[]).unwrap_err().message.contains("shortname"));
        write(&d.join("x"), b"x");
    }
}
