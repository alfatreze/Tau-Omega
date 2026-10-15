//! What a core package says about itself, before anything is planned or written (`docs/MARKETPLACE_SPEC.md` section 7.2): identity, requirements, the
//! picture, where it sits in the Library's grouping, the files it needs (and which of them the user must supply), the settings it offers and the
//! author's own text. Built only from the package zip: no card, no network. Everything here is read, never trusted: sizes are capped and text is
//! shown as plain text.

use crate::{
    ErrorCode, TauError, cardlayout,
    core_package::{Limits, Vetted},
    corehub, file_purpose, icon, taxonomy,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

const TEXT_CAP: u64 = 8_000;
const JSON_CAP: u64 = 1 << 20;
/// The Pocket shows at most this many settings; entries past it are dropped by the Pocket (documented limit).
pub const INTERACT_CAP: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PreviewPlatform {
    pub id: String,
    pub name: String,
    /// The Library's grouping (`taxonomy::resolve`): category and manufacturer.
    pub category: String,
    pub manufacturer: String,
    /// The package ships this platform's own file.
    pub in_package: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NeedFile {
    pub slot: String,
    /// A fixed file name, else empty (any file with an accepted extension).
    pub filename: String,
    pub extensions: Vec<String>,
    pub required: bool,
    pub size_exact: Option<u64>,
    pub size_maximum: Option<u64>,
    pub core_specific: bool,
    /// Where on the card it goes, relative to the card root.
    pub folder_rel: String,
    /// The package already carries this file.
    pub in_package: bool,
    /// A required file with a fixed name that is not in the package: the user supplies it (a BIOS, a key file).
    pub you_supply: bool,
    /// The plain-language sentence, the same wording as the Files and Settings tabs.
    pub purpose: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CorePreview {
    pub core_id: String,
    pub shortname: String,
    pub author: String,
    pub description: String,
    pub url: String,
    pub version: String,
    pub date_release: String,
    pub firmware_required: String,
    pub dock_supported: Option<bool>,
    pub link_port: Option<bool>,
    pub sleep_supported: Option<bool>,
    pub platforms: Vec<PreviewPlatform>,
    /// The core's icon as a PNG (decoded from `icon.bin`), when the package has a valid one.
    pub icon_png: Option<Vec<u8>>,
    /// The first declared platform's banner as a PNG, when the package ships it.
    pub banner_png: Option<Vec<u8>>,
    pub needs: Vec<NeedFile>,
    /// Names of the settings in `interact.json`, in order.
    pub settings: Vec<String>,
    pub info_text: String,
    pub has_bitstream: bool,
    pub sha256: String,
    pub zip_bytes: u64,
    pub files: usize,
    pub uncompressed_bytes: u64,
    pub asset_files: usize,
    /// Things worth knowing that do not stop anything.
    pub warnings: Vec<String>,
}

fn read_named(archive: &mut zip::ZipArchive<fs::File>, name: &str, cap: u64) -> Option<Vec<u8>> {
    let mut e = archive.by_name(name).ok()?;
    if e.size() > cap {
        return None;
    }
    let mut v = Vec::new();
    e.read_to_end(&mut v).ok()?;
    Some(v)
}

fn sha256_of(path: &Path) -> Result<String, TauError> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

fn text(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Describes a vetted package zip (`vetted` came from `core_pack::vet_any` or `core_package::vet_zip`).
pub fn preview(zip_path: &Path, vetted: &Vetted) -> Result<CorePreview, TauError> {
    let id = &vetted.core_id;
    let mut archive = zip::ZipArchive::new(fs::File::open(zip_path)?)
        .map_err(|e| TauError::e(ErrorCode::InstallRefused, e.to_string()))?;
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.name().to_string()))
        .collect();
    let has = |n: &str| names.iter().any(|x| x == n);
    let json_of = |archive: &mut zip::ZipArchive<fs::File>, n: &str| -> Option<Value> {
        serde_json::from_slice(&read_named(archive, n, JSON_CAP)?).ok()
    };
    let core = json_of(&mut archive, &format!("Cores/{id}/core.json")).unwrap_or(Value::Null);
    let meta = core
        .pointer("/core/metadata")
        .cloned()
        .unwrap_or(Value::Null);
    let framework = core
        .pointer("/core/framework")
        .cloned()
        .unwrap_or(Value::Null);
    let platform_ids = cardlayout::platform_ids(&core);

    let platforms: Vec<PreviewPlatform> = platform_ids
        .iter()
        .map(|p| {
            let own = json_of(&mut archive, &format!("Platforms/{p}.json"));
            let field = |k: &str| {
                own.as_ref()
                    .and_then(|j| j.pointer(&format!("/platform/{k}")))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let (category, manufacturer) =
                taxonomy::resolve(p, &field("category"), &field("manufacturer"));
            PreviewPlatform {
                id: p.clone(),
                name: if field("name").is_empty() {
                    p.clone()
                } else {
                    field("name")
                },
                category,
                manufacturer,
                in_package: own.is_some(),
            }
        })
        .collect();

    let icon_png = read_named(&mut archive, &format!("Cores/{id}/icon.bin"), 1 << 16)
        .and_then(|b| icon::decode_icon_bin(&b).ok());
    let banner_png = platform_ids.first().and_then(|p| {
        read_named(&mut archive, &format!("Platforms/_images/{p}.bin"), 1 << 20)
            .and_then(|b| icon::decode_platform_image(&b).ok())
    });

    let data = json_of(&mut archive, &format!("Cores/{id}/data.json"));
    let slots = corehub::slots(data.as_ref(), &platform_ids);
    let needs: Vec<NeedFile> = slots
        .iter()
        .map(|s| {
            let folder = if s.core_specific {
                id.as_str()
            } else {
                "common"
            };
            let folder_rel = format!("Assets/{}/{}", s.platform, folder);
            let in_package = !s.filename.is_empty()
                && (has(&format!("{folder_rel}/{}", s.filename))
                    || has(&format!("Cores/{id}/{}", s.filename)));
            NeedFile {
                slot: s.name.clone(),
                filename: s.filename.clone(),
                extensions: s.extensions.clone(),
                required: s.required,
                size_exact: s.size_exact,
                size_maximum: s.size_maximum,
                core_specific: s.core_specific,
                folder_rel,
                in_package,
                you_supply: s.required && !s.filename.is_empty() && !in_package,
                purpose: if s.filename.is_empty() {
                    file_purpose::describe(
                        crate::core_files::FolderRole::AssetsShared,
                        &format!("x.{}", s.extensions.first().cloned().unwrap_or_default()),
                        std::slice::from_ref(s),
                    )
                    .summary
                } else {
                    file_purpose::describe(
                        if s.core_specific {
                            crate::core_files::FolderRole::AssetsCore
                        } else {
                            crate::core_files::FolderRole::AssetsShared
                        },
                        &s.filename,
                        std::slice::from_ref(s),
                    )
                    .summary
                },
            }
        })
        .collect();

    let settings: Vec<String> = json_of(&mut archive, &format!("Cores/{id}/interact.json"))
        .and_then(|j| j.pointer("/interact/variables").cloned())
        .and_then(|v| v.as_array().cloned())
        .map(|a| {
            a.iter()
                .map(|x| text(x, "name"))
                .filter(|n| !n.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let info_text = read_named(&mut archive, &format!("Cores/{id}/info.txt"), TEXT_CAP)
        .map(|b| String::from_utf8_lossy(&b).trim().to_string())
        .unwrap_or_default();
    let has_bitstream = names.iter().any(|n| {
        n.starts_with(&format!("Cores/{id}/")) && n.to_ascii_lowercase().ends_with(".rbf_r")
    });

    let mut warnings = Vec::new();
    if !has_bitstream {
        warnings.push(
            "The package has no bitstream (.rbf_r). The core cannot start without one.".into(),
        );
    }
    if icon_png.is_none() {
        warnings.push(
            "No valid icon.bin (36x36): the core will have no picture in the Pocket's menu.".into(),
        );
    }
    if settings.len() > INTERACT_CAP {
        warnings.push(format!(
            "interact.json lists {} settings; the Pocket shows only the first {INTERACT_CAP} and drops the rest, including from saved settings.",
            settings.len()
        ));
    }
    if text(&meta, "description").chars().count() > 63 {
        warnings.push(
            "The description is over 63 characters: the Pocket may not list the core.".into(),
        );
    }
    for p in &platforms {
        if !p.in_package {
            warnings.push(format!(
                "The package has no Platforms/{0}.json. The core shows under {0} only if the card already has that platform file.",
                p.id
            ));
        }
    }

    Ok(CorePreview {
        core_id: id.clone(),
        shortname: text(&meta, "shortname"),
        author: text(&meta, "author"),
        description: text(&meta, "description"),
        url: text(&meta, "url"),
        version: vetted.version.clone(),
        date_release: vetted.date_release.clone(),
        firmware_required: vetted.firmware_required.clone(),
        dock_supported: vetted.dock_supported,
        link_port: framework
            .pointer("/hardware/link_port")
            .and_then(Value::as_bool),
        sleep_supported: framework.get("sleep_supported").and_then(Value::as_bool),
        platforms,
        icon_png,
        banner_png,
        needs,
        settings,
        info_text,
        has_bitstream,
        sha256: sha256_of(zip_path)?,
        zip_bytes: fs::metadata(zip_path)?.len(),
        files: vetted.files,
        uncompressed_bytes: vetted.uncompressed_bytes,
        asset_files: vetted.asset_files,
        warnings,
    })
}

/// Vets and describes a package zip whose core is not known in advance.
pub fn preview_zip(zip_path: &Path, limits: Limits) -> Result<CorePreview, TauError> {
    let vetted = crate::core_pack::vet_any(zip_path, limits)?;
    preview(zip_path, &vetted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{ZipWriter, write::SimpleFileOptions};

    const CORE_JSON: &str = r#"{"core":{"magic":"APF_VER_1","metadata":{"platform_ids":["nes"],"shortname":"NES","author":"agg23","description":"NES for the Pocket","url":"https://example.org/nes","version":"1.0.1","date_release":"2023-07-13"},"framework":{"version_required":"1.1","sleep_supported":true,"dock":{"supported":true},"hardware":{"link_port":false}}}}"#;
    const DATA_JSON: &str = r#"{"data":{"data_slots":[
      {"name":"Cartridge","id":"0x100","required":true,"parameters":129,"extensions":["nes","fds"],"size_maximum":100663296},
      {"name":"BIOS","id":"0x104","required":true,"filename":"fds.bios","parameters":"0x1","extensions":["bios"],"size_exact":8192},
      {"name":"VS Pal","id":"0x105","required":false,"filename":"C2C03.PAL","parameters":"0x2","extensions":["pal"],"size_exact":192}]}}"#;
    const INTERACT: &str = r#"{"interact":{"magic":"APF_VER_1","variables":[{"name":"Region","id":1},{"name":"Overscan","id":2}]}}"#;

    fn make(entries: &[(&str, Vec<u8>)]) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "tau-prev-{}-{}",
            std::process::id(),
            crate::test_uniq()
        ));
        fs::create_dir_all(&d).unwrap();
        let p = d.join("c.zip");
        let mut w = ZipWriter::new(fs::File::create(&p).unwrap());
        for (n, b) in entries {
            w.start_file(*n, SimpleFileOptions::default()).unwrap();
            w.write_all(b).unwrap();
        }
        w.finish().unwrap();
        p
    }

    fn core(extra: &[(&str, Vec<u8>)]) -> std::path::PathBuf {
        let mut v: Vec<(&str, Vec<u8>)> = vec![
            ("Cores/agg23.NES/core.json", CORE_JSON.as_bytes().to_vec()),
            ("Cores/agg23.NES/data.json", DATA_JSON.as_bytes().to_vec()),
            (
                "Cores/agg23.NES/interact.json",
                INTERACT.as_bytes().to_vec(),
            ),
            (
                "Cores/agg23.NES/info.txt",
                b"A cycle-accurate NES.\n".to_vec(),
            ),
            ("Cores/agg23.NES/bitstream.rbf_r", vec![7; 32]),
            ("Assets/nes/agg23.NES/C2C03.PAL", vec![0; 192]),
        ];
        v.extend(extra.iter().map(|(n, b)| (*n, b.clone())));
        make(&v)
    }

    #[test]
    fn identity_grouping_and_needs_come_from_the_package() {
        let p = preview_zip(&core(&[]), Limits::default()).unwrap();
        assert_eq!(
            (
                p.core_id.as_str(),
                p.author.as_str(),
                p.version.as_str(),
                p.date_release.as_str()
            ),
            ("agg23.NES", "agg23", "1.0.1", "2023-07-13")
        );
        assert_eq!(
            (
                p.firmware_required.as_str(),
                p.dock_supported,
                p.link_port,
                p.sleep_supported
            ),
            ("1.1", Some(true), Some(false), Some(true))
        );
        // the Library's grouping, not the package's own words
        assert_eq!(
            (
                p.platforms[0].category.as_str(),
                p.platforms[0].manufacturer.as_str()
            ),
            ("Console", "Nintendo")
        );
        assert_eq!(p.settings, ["Region", "Overscan"]);
        assert_eq!(p.info_text, "A cycle-accurate NES.");
        assert!(p.has_bitstream && p.warnings.iter().all(|w| !w.contains("bitstream")));
        let by = |n: &str| p.needs.iter().find(|x| x.slot == n).unwrap();
        // a required fixed-name file that is not in the package is the user's to supply
        assert!(by("BIOS").you_supply && !by("BIOS").in_package);
        assert_eq!(by("BIOS").folder_rel, "Assets/nes/common");
        // a file the package carries, in the core's own folder
        assert!(by("VS Pal").in_package && !by("VS Pal").you_supply);
        assert_eq!(by("VS Pal").folder_rel, "Assets/nes/agg23.NES");
        assert!(by("VS Pal").purpose.contains("Vs. System"));
        // the game slot has no fixed name: nothing to supply
        assert!(!by("Cartridge").you_supply && by("Cartridge").filename.is_empty());
        assert_eq!(p.sha256.len(), 64);
    }

    #[test]
    fn a_sparse_package_warns_instead_of_failing() {
        let z = make(&[
            ("Cores/a.B/core.json", br#"{"core":{"metadata":{"platform_ids":["zzz"],"shortname":"B","author":"a","version":"1","description":"0123456789012345678901234567890123456789012345678901234567890123456789"}}}"#.to_vec()),
        ]);
        let p = preview_zip(&z, Limits::default()).unwrap();
        let w = p.warnings.join(" | ");
        assert!(
            w.contains("no bitstream")
                && w.contains("icon.bin")
                && w.contains("over 63")
                && w.contains("Platforms/zzz.json"),
            "{w}"
        );
        assert!(
            p.icon_png.is_none()
                && p.banner_png.is_none()
                && p.needs.is_empty()
                && p.settings.is_empty()
        );
        // unknown to the Library and the package: grouped as Other
        assert_eq!(p.platforms[0].category, "Other");
    }

    #[test]
    fn more_than_sixteen_settings_are_called_out() {
        let z = make(&[
            ("Cores/agg23.NES/core.json", CORE_JSON.as_bytes().to_vec()),
            (
                "Cores/agg23.NES/interact.json",
                format!(
                    r#"{{"interact":{{"variables":[{}]}}}}"#,
                    (0..18)
                        .map(|i| format!(r#"{{"name":"S{i}","id":{i}}}"#))
                        .collect::<Vec<_>>()
                        .join(",")
                )
                .into_bytes(),
            ),
        ]);
        let p = preview_zip(&z, Limits::default()).unwrap();
        assert_eq!(p.settings.len(), 18);
        assert!(p.warnings.iter().any(|w| w.contains("first 16")));
    }

    #[test]
    fn the_real_tau_package_describes_itself() {
        let zip = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/packages/alfatreze.TAU_0.4.0_2026-09-22.zip");
        let p = preview_zip(&zip, Limits::default()).unwrap();
        assert_eq!(p.core_id, "alfatreze.TAU");
        assert!(p.has_bitstream && !p.platforms.is_empty());
        assert!(
            p.icon_png
                .as_ref()
                .is_some_and(|b| b.starts_with(&[0x89, b'P', b'N', b'G']))
        );
        assert_eq!(p.platforms[0].category, "Media Players");
    }
}
