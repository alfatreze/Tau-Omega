//! A pre-flight check of a core: every documented limit and rule that makes a core fail to load, vanish from the Pocket's menu or quietly drop
//! settings, found before anything is written to a card (`docs/DEV_LOOP_SPEC.md` section 3). The same engine checks a package zip, a folder (packed
//! first) and a core that is already on a card, so the manual-add preview and a developer's own builds are judged by one set of rules.
//!
//! Each finding carries the rule it comes from and how to fix it; severities are `Error` (will not load, will not show, or will lose data),
//! `Warning` (probably wrong) and `Note` (worth knowing). The rules come from Analogue's developer documentation and from failures seen in practice
//! (a 264-character description that hid a core from the menu, a seventeenth interact entry that was silently dropped), not from any one core's code.

use crate::{TauError, cardlayout, core_pack, icon};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    /// One plain sentence.
    pub message: String,
    /// Where the rule comes from.
    pub rule: String,
    pub fix: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LintReport {
    pub core_id: String,
    pub findings: Vec<Finding>,
    pub errors: usize,
    pub warnings: usize,
    pub notes: usize,
}

impl LintReport {
    fn add(
        &mut self,
        id: &str,
        severity: Severity,
        message: impl Into<String>,
        rule: &str,
        fix: &str,
    ) {
        self.findings.push(Finding {
            id: id.into(),
            severity,
            message: message.into(),
            rule: rule.into(),
            fix: fix.into(),
        });
    }

    fn finish(mut self) -> Self {
        self.findings.sort_by_key(|f| (f.severity, f.id.clone()));
        self.errors = self
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count();
        self.warnings = self
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Warning)
            .count();
        self.notes = self
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Note)
            .count();
        self
    }

    pub fn has(&self, id: &str) -> bool {
        self.findings.iter().any(|f| f.id == id)
    }
}

/// What the rules read: file names and bytes, wherever the core lives.
trait Files {
    /// Every file path, relative to the package or card root, forward slashes.
    fn names(&self) -> &[String];
    fn read(&mut self, name: &str, cap: u64) -> Option<Vec<u8>>;
    fn size(&mut self, name: &str) -> Option<u64>;
}

struct ZipFiles {
    archive: zip::ZipArchive<fs::File>,
    names: Vec<String>,
}

impl ZipFiles {
    fn open(path: &Path) -> Result<Self, TauError> {
        let mut archive = zip::ZipArchive::new(fs::File::open(path)?).map_err(|e| {
            TauError::e(
                crate::ErrorCode::InstallRefused,
                format!("This is not a readable zip: {e}"),
            )
        })?;
        let mut names = Vec::new();
        for i in 0..archive.len() {
            if let Ok(e) = archive.by_index(i)
                && !e.is_dir()
            {
                names.push(e.name().to_string());
            }
        }
        Ok(Self { archive, names })
    }
}

impl Files for ZipFiles {
    fn names(&self) -> &[String] {
        &self.names
    }

    fn read(&mut self, name: &str, cap: u64) -> Option<Vec<u8>> {
        let mut e = self.archive.by_name(name).ok()?;
        let mut v = Vec::new();
        e.by_ref().take(cap).read_to_end(&mut v).ok()?;
        Some(v)
    }

    fn size(&mut self, name: &str) -> Option<u64> {
        self.archive.by_name(name).ok().map(|e| e.size())
    }
}

/// A core already on a card: `Cores/<id>`, its platform files and its own asset folders (never the media).
struct CardFiles {
    root: PathBuf,
    names: Vec<String>,
}

impl CardFiles {
    fn open(card: &Path, id: &str) -> Result<Self, TauError> {
        let core_dir = crate::corehub::core_dir(card, id)?;
        let mut names = Vec::new();
        collect(&core_dir, &format!("Cores/{id}"), &mut names);
        let platforms = fs::read(core_dir.join("core.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .map(|j| cardlayout::platform_ids(&j))
            .unwrap_or_default();
        for p in &platforms {
            for rel in [
                format!("Platforms/{p}.json"),
                format!("Platforms/_images/{p}.bin"),
            ] {
                if card.join(&rel).is_file() {
                    names.push(rel);
                }
            }
            let own = format!("Assets/{p}/{id}");
            collect(&card.join(&own), &own, &mut names);
        }
        Ok(Self {
            root: card.to_path_buf(),
            names,
        })
    }
}

fn collect(dir: &Path, rel: &str, out: &mut Vec<String>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for e in read.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let r = format!("{rel}/{name}");
        match e.file_type() {
            Ok(t) if t.is_dir() => collect(&e.path(), &r, out),
            Ok(t) if t.is_file() => out.push(r),
            _ => {}
        }
    }
}

impl Files for CardFiles {
    fn names(&self) -> &[String] {
        &self.names
    }

    fn read(&mut self, name: &str, cap: u64) -> Option<Vec<u8>> {
        let mut v = Vec::new();
        fs::File::open(self.root.join(name))
            .ok()?
            .take(cap)
            .read_to_end(&mut v)
            .ok()?;
        Some(v)
    }

    fn size(&mut self, name: &str) -> Option<u64> {
        fs::metadata(self.root.join(name)).ok().map(|m| m.len())
    }
}

const JSON_CAP: u64 = 1 << 20;
const REQUIRED_JSON: [&str; 7] = [
    "audio.json",
    "core.json",
    "data.json",
    "input.json",
    "interact.json",
    "variants.json",
    "video.json",
];
/// The Pocket shows at most this many entries of `interact.json`; the rest are dropped.
pub const INTERACT_CAP: usize = 16;

const R_CORE: &str = "Analogue developer docs: core.json (field limits)";
const R_FILES: &str = "Analogue developer docs: the core's required JSON files";
const R_INTERACT: &str =
    "Analogue developer docs: interact.json (the menu holds at most 16 entries)";
const R_DATA: &str = "Analogue developer docs: data.json (data slots)";
const R_CARD: &str = "The card's file system (names it cannot hold)";

fn json_of(f: &mut dyn Files, name: &str) -> Option<Value> {
    serde_json::from_slice(&f.read(name, JSON_CAP)?).ok()
}

fn text(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn semver_shaped(v: &str) -> bool {
    let core = v.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn date_ok(d: &str) -> bool {
    let p: Vec<&str> = d.split('-').collect();
    p.len() == 3
        && p[0].len() == 4
        && matches!((p[0].parse::<u32>(), p[1].parse::<u32>(), p[2].parse::<u32>()), (Ok(y), Ok(m), Ok(dd)) if (1990..=2100).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&dd))
}

fn bad_name(part: &str) -> bool {
    part.chars()
        .any(|c| c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\'))
        || part.ends_with('.')
        || part.ends_with(' ')
}

fn lint_files(f: &mut dyn Files, id: &str) -> LintReport {
    let mut r = LintReport {
        core_id: id.to_string(),
        ..LintReport::default()
    };
    let base = format!("Cores/{id}");
    let names: Vec<String> = f.names().to_vec();
    let has = |n: &str| names.iter().any(|x| x == n);

    // ---- names and junk
    for n in &names {
        if n.split('/').any(bad_name) {
            r.add(
                "L16",
                Severity::Error,
                format!("{n} has a character or ending the card's file system cannot hold."),
                R_CARD,
                "Rename it (no < > : \" | ? * \\, and no trailing dot or space).",
            );
            break;
        }
    }
    let junk = names
        .iter()
        .filter(|n| {
            n.split('/').any(|p| {
                p.starts_with("._") || p == ".DS_Store" || p == "__MACOSX" || p == "Thumbs.db"
            })
        })
        .count();
    if junk > 0 {
        r.add("L14", Severity::Warning, format!("{junk} junk file(s) from macOS or Windows (like ._ files or .DS_Store) are in the package."), "Packaging", "Remove them before zipping (zip with `-x '*.DS_Store' '._*'`).");
    }

    // ---- the seven JSON files
    for j in REQUIRED_JSON {
        if !has(&format!("{base}/{j}")) {
            r.add(
                "L09",
                Severity::Error,
                format!("{j} is missing from Cores/{id}."),
                R_FILES,
                "Add it; the core's folder must hold all seven JSON files.",
            );
        }
    }

    // ---- core.json
    let core = json_of(f, &format!("{base}/core.json"));
    let Some(core) = core else {
        if has(&format!("{base}/core.json")) {
            r.add("L01", Severity::Error, "core.json is not valid JSON.", R_CORE, "Fix the syntax (a stray comma or quote); the core disappears from the Pocket's menu while it is invalid.");
        }
        return r;
    };
    let meta = core
        .pointer("/core/metadata")
        .cloned()
        .unwrap_or(Value::Null);
    if meta.is_null() {
        r.add(
            "L02",
            Severity::Error,
            "core.json has no core.metadata section.",
            R_CORE,
            "Add metadata with shortname, author, version and platform_ids.",
        );
        return r;
    }
    let (short, author, version, desc, url, date) = (
        text(&meta, "shortname"),
        text(&meta, "author"),
        text(&meta, "version"),
        text(&meta, "description"),
        text(&meta, "url"),
        text(&meta, "date_release"),
    );
    for (k, v) in [
        ("shortname", &short),
        ("author", &author),
        ("version", &version),
    ] {
        if v.is_empty() {
            r.add(
                "L02",
                Severity::Error,
                format!("core.json has no {k}."),
                R_CORE,
                "Fill the field in.",
            );
        }
    }
    if format!("{author}.{short}") != id {
        r.add(
            "L03",
            Severity::Error,
            format!("core.json says {author}.{short} but the folder is {id}."),
            R_CORE,
            "Make the folder name author.shortname exactly as core.json says.",
        );
    }
    // Documented limits. Analogue's docs state them, but the Pocket does not refuse a core that exceeds them (many published cores do), so these are
    // warnings: the text may be cut or ignored, and a stricter firmware could refuse it.
    let limit = |r: &mut LintReport, field: &str, v: &str, max: usize| {
        if v.chars().count() > max {
            r.add("L04", Severity::Warning, format!("{field} is {} characters; the documented limit is {max}.", v.chars().count()), R_CORE, &format!("Shorten it to {max} characters or fewer so no firmware truncates or ignores it."));
        }
    };
    limit(&mut r, "shortname", &short, 31);
    limit(&mut r, "author", &author, 31);
    limit(&mut r, "version", &version, 31);
    limit(&mut r, "description", &desc, 63);
    limit(&mut r, "url", &url, 63);
    if !version.is_empty() && !semver_shaped(&version) {
        r.add(
            "L05",
            Severity::Note,
            format!(
                "The version \"{version}\" is not SemVer-shaped (x.y.z, optional -pre-release)."
            ),
            R_CORE,
            "SemVer lets tools order versions; pre-release suffixes like -alpha.1 are fine.",
        );
    }
    if !date.is_empty() && !date_ok(&date) {
        r.add(
            "L06",
            Severity::Warning,
            format!("date_release \"{date}\" is not a YYYY-MM-DD date."),
            R_CORE,
            "Write it as 2026-10-15.",
        );
    }
    let platforms = cardlayout::platform_ids(&core);
    if platforms.is_empty() {
        r.add(
            "L02",
            Severity::Error,
            "core.json lists no platform_ids.",
            R_CORE,
            "Name at least one platform id.",
        );
    }
    for p in &platforms {
        if !has(&format!("Platforms/{p}.json")) {
            r.add("L07", Severity::Note, format!("There is no Platforms/{p}.json here; the core shows under {p} only if the card already has that platform file."), R_CORE, "Ship the platform file with the core.");
        }
    }
    if core
        .pointer("/core/framework/version_required")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
    {
        r.add(
            "L08",
            Severity::Warning,
            "core.json does not state the Pocket firmware it needs (framework.version_required).",
            R_CORE,
            "Set the minimum firmware version.",
        );
    }

    // ---- icon and bitstream
    match f.read(&format!("{base}/icon.bin"), 1 << 16) {
        None => r.add(
            "L10",
            Severity::Warning,
            "There is no icon.bin: the core has no picture in the Pocket's menu.",
            R_FILES,
            "Add a 36x36 icon.bin.",
        ),
        Some(b) if icon::decode_icon_bin(&b).is_err() => r.add(
            "L10",
            Severity::Warning,
            format!(
                "icon.bin is {} bytes; it must be a 36x36 picture (2592 bytes).",
                b.len()
            ),
            R_FILES,
            "Re-export the icon at 36x36.",
        ),
        Some(_) => {}
    }
    // The bitstream is the file core.json names (`cores[].filename`: `.rbf_r` for most cores, `.rev` for some); without a name, any `.rbf_r`.
    let listed: Vec<String> = core
        .pointer("/core/cores")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|c| text(c, "filename"))
        .filter(|n| !n.is_empty())
        .collect();
    let bits: Vec<String> = if listed.is_empty() {
        names
            .iter()
            .filter(|n| {
                n.starts_with(&format!("{base}/")) && n.to_ascii_lowercase().ends_with(".rbf_r")
            })
            .cloned()
            .collect()
    } else {
        listed.iter().map(|n| format!("{base}/{n}")).collect()
    };
    if bits.is_empty() {
        r.add(
            "L11",
            Severity::Error,
            "core.json names no bitstream and the folder has no .rbf_r.",
            R_FILES,
            "List the bitstream in core.json (cores[].filename) and ship it.",
        );
    }
    for b in &bits {
        // the card's file system ignores case, so `Bitstream.rbf_r` in core.json matches `bitstream.rbf_r` on disk
        let found = names.iter().find(|n| n.eq_ignore_ascii_case(b)).cloned();
        let Some(b) = found.as_deref() else {
            r.add(
                "L11",
                Severity::Error,
                format!("The bitstream {b} that core.json names is not in the package."),
                R_FILES,
                "Ship the file core.json names, or fix the name.",
            );
            continue;
        };
        match f.read(b, 65_536) {
            Some(bytes) if bytes.is_empty() || f.size(b) == Some(0) => r.add(
                "L11",
                Severity::Error,
                format!("{b} is empty."),
                R_FILES,
                "Rebuild and copy the real bitstream.",
            ),
            Some(bytes) if bytes.iter().all(|x| *x == 0) => r.add(
                "L11",
                Severity::Error,
                format!("{b} starts with only zero bytes: it is not a bitstream."),
                R_FILES,
                "Rebuild; check the conversion step.",
            ),
            _ => {}
        }
    }

    // ---- data.json
    if let Some(data) = json_of(f, &format!("{base}/data.json")) {
        let slots = data
            .pointer("/data/data_slots")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut seen = BTreeSet::new();
        for s in &slots {
            let sid = s.get("id").map(|v| v.to_string()).unwrap_or_default();
            if !seen.insert(sid.clone()) {
                r.add(
                    "L12",
                    Severity::Error,
                    format!("Two data slots share the id {sid}."),
                    R_DATA,
                    "Give every slot its own id.",
                );
            }
        }
        let infos = crate::corehub::slots(Some(&data), &platforms);
        for s in &infos {
            if s.filename.is_empty() {
                continue;
            }
            let folder = if s.core_specific { id } else { "common" };
            let path = format!("Assets/{}/{folder}/{}", s.platform, s.filename);
            let alt = format!("{base}/{}", s.filename);
            if let Some(size) = f.size(&path).or_else(|| f.size(&alt)) {
                if let Some(want) = s.size_exact
                    && size != want
                {
                    r.add(
                        "L12",
                        Severity::Warning,
                        format!(
                            "{} is {size} bytes but its slot \"{}\" expects exactly {want}.",
                            s.filename, s.name
                        ),
                        R_DATA,
                        "Ship the right file or fix size_exact.",
                    );
                }
            } else if s.required {
                r.add("L12", Severity::Note, format!("The required file {} (slot \"{}\") is not included: the user must supply it in Assets/{}/{folder}.", s.filename, s.name, s.platform), R_DATA, "Fine for a BIOS you cannot redistribute; say so in info.txt.");
            }
        }
        let reload = slots
            .iter()
            .filter(|s| cardlayout::slot_parameters(s) & 1 != 0)
            .count();
        if reload > 6 {
            r.add("L17", Severity::Note, format!("{reload} data slots are marked user-reloadable; each adds a row to the core's settings screen."), R_DATA, "Clear the reload bit on slots users should not swap.");
        }
    }

    // ---- interact.json
    if let Some(inter) = json_of(f, &format!("{base}/interact.json")) {
        let vars = inter
            .pointer("/interact/variables")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if vars.len() > INTERACT_CAP {
            r.add("L13", Severity::Error, format!("interact.json has {} entries; the Pocket shows only {INTERACT_CAP}. The rest are dropped, and so is their saved state.", vars.len()), R_INTERACT, &format!("Reduce to {INTERACT_CAP} entries, or move some into a submenu."));
        }
        let mut ids = BTreeSet::new();
        for v in &vars {
            if let Some(i) = v.get("id")
                && !ids.insert(i.to_string())
            {
                r.add(
                    "L13",
                    Severity::Warning,
                    format!(
                        "Two interact entries share the id {i}: their saved values may collide."
                    ),
                    R_INTERACT,
                    "Give every entry its own id unless the sharing is deliberate.",
                );
                break;
            }
        }
    }
    r
}

/// Lints a package zip.
pub fn lint_zip(zip: &Path) -> Result<LintReport, TauError> {
    let id = core_pack::zip_core_id(zip)?;
    let mut f = ZipFiles::open(zip)?;
    Ok(lint_files(&mut f, &id).finish())
}

/// Lints a package folder or a bare core folder (packed to a temporary zip first, in `tmp`).
pub fn lint_folder(dir: &Path, tmp: &Path) -> Result<LintReport, TauError> {
    let packed = core_pack::pack_folder(dir, tmp, crate::core_package::Limits::default())?;
    lint_zip(&packed.zip)
}

/// Lints a core that is already installed on a card.
pub fn lint_card_core(card: &Path, id: &str) -> Result<LintReport, TauError> {
    let mut f = CardFiles::open(card, id)?;
    Ok(lint_files(&mut f, id).finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn core_json(desc: &str, version: &str, short: &str) -> String {
        format!(
            r#"{{"core":{{"magic":"APF_VER_1","metadata":{{"platform_ids":["nes"],"shortname":"{short}","author":"agg23","description":"{desc}","url":"https://example.org","version":"{version}","date_release":"2026-10-15"}},"framework":{{"version_required":"1.1"}}}}}}"#
        )
    }

    fn vars(n: usize) -> String {
        let v: Vec<String> = (0..n)
            .map(|i| format!(r#"{{"name":"S{i}","id":{i}}}"#))
            .collect();
        format!(r#"{{"interact":{{"variables":[{}]}}}}"#, v.join(","))
    }

    fn pack(entries: Vec<(String, Vec<u8>)>) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "tau-lint-{}-{}",
            std::process::id(),
            crate::test_uniq()
        ));
        fs::create_dir_all(&d).unwrap();
        let p = d.join("c.zip");
        let mut w = ZipWriter::new(fs::File::create(&p).unwrap());
        for (n, b) in entries {
            w.start_file(n, SimpleFileOptions::default()).unwrap();
            w.write_all(&b).unwrap();
        }
        w.finish().unwrap();
        p
    }

    /// A healthy core with every file; tests change one thing.
    fn healthy() -> Vec<(String, Vec<u8>)> {
        let c = "Cores/agg23.NES/";
        let mut v = vec![
            (format!("{c}core.json"), core_json("NES for the Pocket", "1.0.1", "NES").into_bytes()),
            (format!("{c}data.json"), br#"{"data":{"data_slots":[{"name":"Cartridge","id":"0x100","required":true,"parameters":129,"extensions":["nes"]}]}}"#.to_vec()),
            (format!("{c}interact.json"), vars(3).into_bytes()),
            (format!("{c}icon.bin"), vec![0x10; 2592]),
            (format!("{c}bitstream.rbf_r"), vec![7; 4096]),
            ("Platforms/nes.json".to_string(), b"{}".to_vec()),
        ];
        for j in ["audio.json", "input.json", "variants.json", "video.json"] {
            v.push((format!("{c}{j}"), b"{}".to_vec()));
        }
        v
    }

    fn lint(
        mut e: Vec<(String, Vec<u8>)>,
        change: impl FnOnce(&mut Vec<(String, Vec<u8>)>),
    ) -> LintReport {
        change(&mut e);
        lint_zip(&pack(e)).unwrap()
    }

    fn set(e: &mut Vec<(String, Vec<u8>)>, name: &str, bytes: Vec<u8>) {
        e.retain(|(n, _)| n != name);
        e.push((name.to_string(), bytes));
    }

    #[test]
    fn a_healthy_core_has_no_errors_or_warnings() {
        let r = lint(healthy(), |_| {});
        assert_eq!((r.errors, r.warnings), (0, 0), "{:?}", r.findings);
    }

    #[test]
    fn an_over_long_description_is_a_warning_with_the_documented_limit() {
        let long = "x".repeat(264);
        let r = lint(healthy(), |e| {
            set(
                e,
                "Cores/agg23.NES/core.json",
                core_json(&long, "1.0.1", "NES").into_bytes(),
            )
        });
        let f = r.findings.iter().find(|f| f.id == "L04").unwrap();
        assert!(
            f.severity == Severity::Warning
                && f.message.contains("264")
                && f.message.contains("63"),
            "{f:?}"
        );
        // exactly 63 is fine
        let ok = lint(healthy(), |e| {
            set(
                e,
                "Cores/agg23.NES/core.json",
                core_json(&"y".repeat(63), "1.0.1", "NES").into_bytes(),
            )
        });
        assert!(!ok.has("L04"));
    }

    #[test]
    fn the_seventeenth_interact_entry_and_duplicate_ids_are_errors() {
        let r = lint(healthy(), |e| {
            set(e, "Cores/agg23.NES/interact.json", vars(17).into_bytes())
        });
        assert!(r.findings.iter().any(|f| f.id == "L13"
            && f.message.contains("17 entries")
            && f.severity == Severity::Error));
        let sixteen = lint(healthy(), |e| {
            set(e, "Cores/agg23.NES/interact.json", vars(16).into_bytes())
        });
        assert!(!sixteen.has("L13"));
        let dup = lint(healthy(), |e| {
            set(
                e,
                "Cores/agg23.NES/interact.json",
                br#"{"interact":{"variables":[{"name":"a","id":1},{"name":"b","id":1}]}}"#.to_vec(),
            )
        });
        assert!(
            dup.findings
                .iter()
                .any(|f| f.id == "L13" && f.message.contains("share the id"))
        );
    }

    #[test]
    fn missing_files_a_dead_bitstream_and_a_wrong_folder_name_are_caught() {
        let missing = lint(healthy(), |e| e.retain(|(n, _)| !n.ends_with("video.json")));
        assert!(
            missing
                .findings
                .iter()
                .any(|f| f.id == "L09" && f.message.contains("video.json"))
        );
        let nobits = lint(healthy(), |e| e.retain(|(n, _)| !n.ends_with(".rbf_r")));
        assert!(
            nobits
                .findings
                .iter()
                .any(|f| f.id == "L11" && f.message.contains("no bitstream"))
        );
        // a bitstream named by core.json under another name or another case is found
        let named = |file: &str, on_disk: &str| {
            lint(healthy(), |e| {
                e.retain(|(n, _)| !n.ends_with(".rbf_r"));
                e.push((format!("Cores/agg23.NES/{on_disk}"), vec![9; 64]));
                let j = core_json("d", "1.0.1", "NES").replace(
                    r#""framework""#,
                    &format!(
                        r#""cores":[{{"name":"default","id":0,"filename":"{file}"}}],"framework""#
                    ),
                );
                set(e, "Cores/agg23.NES/core.json", j.into_bytes());
            })
        };
        assert!(!named("nes.rev", "nes.rev").has("L11"));
        assert!(!named("Bitstream.rbf_r", "bitstream.rbf_r").has("L11"));
        assert!(
            named("nes.rev", "other.rev")
                .findings
                .iter()
                .any(|f| f.id == "L11" && f.message.contains("not in the package"))
        );
        let zeros = lint(healthy(), |e| {
            set(e, "Cores/agg23.NES/bitstream.rbf_r", vec![0; 4096])
        });
        assert!(
            zeros
                .findings
                .iter()
                .any(|f| f.id == "L11" && f.message.contains("zero"))
        );
        let wrong = lint(healthy(), |e| {
            set(
                e,
                "Cores/agg23.NES/core.json",
                core_json("d", "1.0.1", "SNES").into_bytes(),
            )
        });
        assert!(wrong.findings.iter().any(|f| f.id == "L03"));
        let broken = lint(healthy(), |e| {
            set(e, "Cores/agg23.NES/core.json", b"{ \"core\": ".to_vec())
        });
        assert!(
            broken
                .findings
                .iter()
                .any(|f| f.id == "L01" && f.severity == Severity::Error)
        );
    }

    #[test]
    fn softer_findings_have_the_right_severity() {
        let r = lint(healthy(), |e| {
            set(
                e,
                "Cores/agg23.NES/core.json",
                core_json("d", "v1.0", "NES")
                    .replace("2026-10-15", "15/10/2026")
                    .into_bytes(),
            );
            set(e, "Cores/agg23.NES/icon.bin", vec![0; 10]);
            e.push(("Cores/agg23.NES/._core.json".into(), vec![1]));
            e.push(("Assets/nes/common/bad:name.nes".into(), vec![1]));
        });
        let sev = |id: &str| r.findings.iter().find(|f| f.id == id).map(|f| f.severity);
        assert_eq!(sev("L05"), Some(Severity::Note));
        assert_eq!(sev("L06"), Some(Severity::Warning));
        assert_eq!(sev("L10"), Some(Severity::Warning));
        assert_eq!(sev("L14"), Some(Severity::Warning));
        assert_eq!(sev("L16"), Some(Severity::Error));
        // errors sort first
        assert_eq!(r.findings[0].severity, Severity::Error);
    }

    #[test]
    fn data_slot_problems_are_reported_without_blocking_a_bios_you_supply() {
        let data = br#"{"data":{"data_slots":[
          {"name":"A","id":1,"required":true,"parameters":1,"extensions":["nes"]},
          {"name":"B","id":1,"required":false,"parameters":1,"extensions":["bin"]},
          {"name":"BIOS","id":3,"required":true,"filename":"fds.bios","parameters":1,"extensions":["bios"],"size_exact":8192},
          {"name":"Pal","id":4,"required":false,"filename":"c.pal","parameters":2,"extensions":["pal"],"size_exact":192}]}}"#.to_vec();
        let r = lint(healthy(), |e| {
            set(e, "Cores/agg23.NES/data.json", data);
            e.push(("Assets/nes/agg23.NES/c.pal".into(), vec![0; 100]));
        });
        assert!(r.findings.iter().any(|f| f.id == "L12"
            && f.severity == Severity::Error
            && f.message.contains("share the id")));
        assert!(r.findings.iter().any(|f| f.id == "L12"
            && f.severity == Severity::Note
            && f.message.contains("fds.bios")));
        assert!(r.findings.iter().any(|f| f.id == "L12"
            && f.severity == Severity::Warning
            && f.message.contains("expects exactly 192")));
    }

    #[test]
    fn a_core_on_a_card_is_checked_the_same_way() {
        let card = std::env::temp_dir().join(format!(
            "tau-lintcard-{}-{}",
            std::process::id(),
            crate::test_uniq()
        ));
        for (n, b) in healthy() {
            let p = card.join(&n);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, b).unwrap();
        }
        assert_eq!(lint_card_core(&card, "agg23.NES").unwrap().errors, 0);
        fs::write(
            card.join("Cores/agg23.NES/core.json"),
            core_json(&"z".repeat(100), "1.0.1", "NES"),
        )
        .unwrap();
        assert!(lint_card_core(&card, "agg23.NES").unwrap().has("L04"));
        assert!(lint_card_core(&card, "nobody.Here").is_err());
    }

    #[test]
    fn the_real_tau_package_has_no_errors() {
        let zip = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/packages/alfatreze.TAU_0.4.0_2026-09-22.zip");
        let r = lint_zip(&zip).unwrap();
        assert_eq!(r.errors, 0, "{:?}", r.findings);
    }
}
