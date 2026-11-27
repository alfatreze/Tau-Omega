//! Device-agnostic `SystemDriver` trait and normalized per-core view (`CoreSummary`, `CoreProfile`,
//! `CoreCapabilities`, `RomDestination`, `ArtworkTarget`) shared by Analogue Pocket, Game Bub,
//! and future targets (MiSTer, DSpico, etc.).

use crate::{ErrorCode, TauError, core_files, corehub, device::DeviceKind, gamebub, platforms};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum RomTransferMode {
    FixedPlatformDir,
    UserChosenDir,
    CoreSpecificDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ArtworkMode {
    PocketPlatformBin,
    AppOverrideOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreCapabilities {
    pub can_toggle_visibility: bool,
    pub has_managed_saves: bool,
    pub has_screenshots: bool,
    pub has_settings_files: bool,
    pub has_declared_settings: bool,
    pub can_remove: bool,
    pub rom_transfer_mode: RomTransferMode,
    pub artwork_mode: ArtworkMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RomDestination {
    pub base_rel: String,
    pub label: String,
    pub user_chooses_folder: bool,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArtworkTarget {
    pub mode: ArtworkMode,
    pub banner_rel: Option<String>,
    pub icon_rel: Option<String>,
    pub override_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreFact {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreTarget {
    pub label: String,
    pub filename: String,
    pub present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreSummary {
    pub device: DeviceKind,
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub platform_ids: Vec<String>,
    pub missing: usize,
    pub disabled: bool,
    pub capabilities: CoreCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreProfile {
    pub device: DeviceKind,
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub version: String,
    pub release_date: String,
    pub url: String,
    pub license: String,
    pub platform_ids: Vec<String>,
    pub capabilities: CoreCapabilities,
    pub facts: Vec<CoreFact>,
    pub targets: Vec<CoreTarget>,
    pub rom_destination: Option<RomDestination>,
    pub artwork_target: ArtworkTarget,
    pub missing_items: Vec<String>,
}

pub trait SystemDriver: Sync {
    fn device(&self) -> DeviceKind;
    fn capabilities(&self) -> CoreCapabilities;
    fn list_cores(&self, card: &Path) -> Result<Vec<CoreSummary>, TauError>;
    fn inspect_core(&self, card: &Path, core_id: &str) -> Result<CoreProfile, TauError>;
    fn resolve_rom_destination(
        &self,
        card: &Path,
        core_id: &str,
        platform_id: &str,
    ) -> Option<RomDestination>;
    fn resolve_artwork_target(&self, core_id: &str, platform_id: Option<&str>) -> ArtworkTarget;
}

pub struct AnaloguePocketDriver;
pub struct GameBubDriver;

static POCKET_DRIVER: AnaloguePocketDriver = AnaloguePocketDriver;
static GAMEBUB_DRIVER: GameBubDriver = GameBubDriver;

fn canonicalize_platform_id(id: &str) -> String {
    platforms::find(id)
        .map(|p| p.id.to_string())
        .unwrap_or_else(|| id.to_ascii_lowercase())
}

fn pocket_rom_slot(detail: &corehub::CoreDetail) -> Option<RomDestination> {
    let slot = detail
        .slots
        .iter()
        .find(|s| s.filename.is_empty() && !s.nonvolatile && !s.extensions.is_empty())?;
    let plat = if !slot.platform.is_empty() {
        slot.platform.clone()
    } else {
        detail
            .platforms
            .first()
            .map(|p| p.id.clone())
            .unwrap_or_default()
    };
    if plat.is_empty() || plat == corehub::NO_PLATFORM {
        return None;
    }
    let base_rel = if slot.core_specific {
        format!("Assets/{plat}/{}", detail.id)
    } else {
        format!("Assets/{plat}/common")
    };
    Some(RomDestination {
        base_rel: base_rel.clone(),
        label: if slot.core_specific {
            format!("{} core folder ({base_rel})", detail.shortname)
        } else {
            format!("Shared {plat} folder ({base_rel})")
        },
        user_chooses_folder: false,
        extensions: slot.extensions.clone(),
    })
}

impl SystemDriver for AnaloguePocketDriver {
    fn device(&self) -> DeviceKind {
        DeviceKind::AnaloguePocket
    }

    fn capabilities(&self) -> CoreCapabilities {
        CoreCapabilities {
            can_toggle_visibility: true,
            has_managed_saves: true,
            has_screenshots: true,
            has_settings_files: true,
            has_declared_settings: false,
            can_remove: true,
            rom_transfer_mode: RomTransferMode::FixedPlatformDir,
            artwork_mode: ArtworkMode::PocketPlatformBin,
        }
    }

    fn list_cores(&self, card: &Path) -> Result<Vec<CoreSummary>, TauError> {
        let caps = self.capabilities();
        let tree = corehub::platform_tree(card)?;
        let mut out = Vec::new();
        for node in tree {
            for c in node.cores {
                let platform_ids = if c.platforms.is_empty() {
                    if !c.platform.is_empty() && c.platform != corehub::NO_PLATFORM {
                        vec![canonicalize_platform_id(&c.platform)]
                    } else {
                        Vec::new()
                    }
                } else {
                    c.platforms
                        .iter()
                        .filter(|p| !p.is_empty() && p.as_str() != corehub::NO_PLATFORM)
                        .map(|p| canonicalize_platform_id(p))
                        .collect()
                };
                out.push(CoreSummary {
                    device: DeviceKind::AnaloguePocket,
                    id: c.id,
                    name: c.shortname,
                    author: c.author,
                    version: c.version,
                    platform_ids,
                    missing: c.missing,
                    disabled: c.disabled,
                    capabilities: caps.clone(),
                });
            }
        }
        Ok(out)
    }

    fn inspect_core(&self, card: &Path, core_id: &str) -> Result<CoreProfile, TauError> {
        let caps = self.capabilities();
        let detail = corehub::core_detail(card, core_id)?;
        let missing = core_files::missing(card, &detail);
        let platform_ids: Vec<String> = detail
            .platforms
            .iter()
            .filter(|p| !p.id.is_empty() && p.id != corehub::NO_PLATFORM)
            .map(|p| canonicalize_platform_id(&p.id))
            .collect();
        let raw_pid = detail
            .platforms
            .first()
            .map(|p| p.id.as_str())
            .filter(|p| *p != corehub::NO_PLATFORM);

        let opt_bool = |v: Option<bool>| match v {
            Some(true) => "Yes".to_string(),
            Some(false) => "No".to_string(),
            None => "Not stated".to_string(),
        };
        let facts = vec![
            CoreFact {
                label: "Pocket firmware".into(),
                value: if detail.firmware_required.is_empty() {
                    "Not stated".into()
                } else {
                    detail.firmware_required.clone()
                },
            },
            CoreFact {
                label: "Dock".into(),
                value: opt_bool(detail.dock_supported),
            },
            CoreFact {
                label: "Link port".into(),
                value: opt_bool(detail.link_port),
            },
            CoreFact {
                label: "Sleep".into(),
                value: opt_bool(detail.sleep_supported),
            },
        ];
        let rom_destination = pocket_rom_slot(&detail);
        let artwork_target = self.resolve_artwork_target(&detail.id, raw_pid);
        let missing_items = missing
            .into_iter()
            .map(|m| format!("{} ({})", m.filename, m.folder_rel))
            .collect();

        Ok(CoreProfile {
            device: DeviceKind::AnaloguePocket,
            id: detail.id.clone(),
            name: if detail.shortname.is_empty() {
                detail.id.clone()
            } else {
                detail.shortname
            },
            author: detail.author,
            description: detail.description,
            version: detail.version,
            release_date: detail.date_release,
            url: detail.url,
            license: String::new(),
            platform_ids,
            capabilities: caps,
            facts,
            targets: Vec::new(),
            rom_destination,
            artwork_target,
            missing_items,
        })
    }

    fn resolve_rom_destination(
        &self,
        card: &Path,
        core_id: &str,
        platform_id: &str,
    ) -> Option<RomDestination> {
        if let Ok(detail) = corehub::core_detail(card, core_id)
            && let Some(dest) = pocket_rom_slot(&detail)
        {
            return Some(dest);
        }
        let pid = canonicalize_platform_id(platform_id);
        let spec = platforms::find(&pid)?;
        Some(RomDestination {
            base_rel: format!("Assets/{}/common", spec.id),
            label: format!("Shared {} folder (Assets/{}/common)", spec.name, spec.id),
            user_chooses_folder: false,
            extensions: spec.extensions.iter().map(|s| (*s).to_string()).collect(),
        })
    }

    fn resolve_artwork_target(&self, core_id: &str, platform_id: Option<&str>) -> ArtworkTarget {
        let pid = platform_id.filter(|s| !s.is_empty() && *s != corehub::NO_PLATFORM);
        ArtworkTarget {
            mode: ArtworkMode::PocketPlatformBin,
            banner_rel: pid.map(|p| format!("Platforms/_images/{p}.bin")),
            icon_rel: Some(format!("Cores/{core_id}/icon.bin")),
            override_key: pid
                .map(|p| format!("pocket:platform:{p}"))
                .unwrap_or_else(|| format!("pocket:core:{core_id}")),
        }
    }
}

fn gamebub_rom_extensions(entry: &gamebub::GameBubInstalledCore) -> Vec<String> {
    let mut exts = Vec::new();
    for f in &entry.files {
        if f.user_selected {
            for ext in &f.extensions {
                let clean = ext.trim().trim_start_matches('.').to_ascii_lowercase();
                if !clean.is_empty() && !exts.contains(&clean) {
                    exts.push(clean);
                }
            }
        }
    }
    exts
}

fn gamebub_platform_ids(entry: &gamebub::GameBubInstalledCore) -> Vec<String> {
    let exts = gamebub_rom_extensions(entry);
    platforms::infer_from_core(&entry.core.id, &entry.core.name, &exts)
        .map(|p| vec![p.id.to_string()])
        .unwrap_or_default()
}

impl SystemDriver for GameBubDriver {
    fn device(&self) -> DeviceKind {
        DeviceKind::GameBub
    }

    fn capabilities(&self) -> CoreCapabilities {
        CoreCapabilities {
            can_toggle_visibility: false,
            has_managed_saves: false,
            has_screenshots: false,
            has_settings_files: false,
            has_declared_settings: true,
            can_remove: true,
            rom_transfer_mode: RomTransferMode::UserChosenDir,
            artwork_mode: ArtworkMode::AppOverrideOnly,
        }
    }

    fn list_cores(&self, card: &Path) -> Result<Vec<CoreSummary>, TauError> {
        let caps = self.capabilities();
        let view = gamebub::inspect_card(card);
        Ok(view
            .cores
            .into_iter()
            .map(|c| {
                let platform_ids = gamebub_platform_ids(&c);
                let missing = c.missing_required_files.len() + c.missing_bitstreams.len();
                CoreSummary {
                    device: DeviceKind::GameBub,
                    id: c.core.id.clone(),
                    name: if c.core.name.is_empty() {
                        c.core.id
                    } else {
                        c.core.name
                    },
                    author: c.core.author,
                    version: c.core.version,
                    platform_ids,
                    missing,
                    disabled: false,
                    capabilities: caps.clone(),
                }
            })
            .collect())
    }

    fn inspect_core(&self, card: &Path, core_id: &str) -> Result<CoreProfile, TauError> {
        let caps = self.capabilities();
        let view = gamebub::inspect_card(card);
        let c = view
            .cores
            .into_iter()
            .find(|x| x.core.id.eq_ignore_ascii_case(core_id))
            .ok_or_else(|| {
                TauError::e(
                    ErrorCode::NotFound,
                    format!("Game Bub core '{core_id}' was not found on this card."),
                )
            })?;

        let platform_ids = gamebub_platform_ids(&c);
        let first_pid = platform_ids.first().map(String::as_str);
        let exts = gamebub_rom_extensions(&c);

        let mut facts = vec![
            CoreFact {
                label: "Min firmware".into(),
                value: if c.core.min_firmware.is_empty() {
                    "Not stated".into()
                } else {
                    c.core.min_firmware.clone()
                },
            },
            CoreFact {
                label: "Folder".into(),
                value: format!("/cores/{}", c.folder),
            },
        ];
        if !c.core.hardware.cartridge_enable.is_empty() || c.core.hardware.cartridge_selectable {
            let mode = if c.core.hardware.cartridge_enable.is_empty() {
                "Supported".to_string()
            } else {
                c.core.hardware.cartridge_enable.clone()
            };
            facts.push(CoreFact {
                label: "Cartridge slot".into(),
                value: if c.core.hardware.cartridge_selectable {
                    format!("{mode} · selectable")
                } else {
                    mode
                },
            });
        }

        let targets = c
            .core
            .bitstreams
            .iter()
            .map(|b| CoreTarget {
                label: b.target.clone(),
                filename: b.filename.clone(),
                present: !c.missing_bitstreams.contains(&b.filename),
            })
            .collect();

        let rom_destination = {
            let folder_hint = first_pid.unwrap_or("roms");
            let use_exts = if exts.is_empty() {
                first_pid
                    .and_then(platforms::find)
                    .map(|p| p.extensions.iter().map(|s| (*s).to_string()).collect())
                    .unwrap_or_default()
            } else {
                exts
            };
            Some(RomDestination {
                base_rel: format!("roms/{folder_hint}"),
                label: format!("Choose SD folder (suggested /roms/{folder_hint})"),
                user_chooses_folder: true,
                extensions: use_exts,
            })
        };

        let mut missing_items: Vec<String> = c
            .missing_bitstreams
            .iter()
            .map(|b| format!("Bitstream {b} (/cores/{})", c.folder))
            .collect();
        missing_items.extend(
            c.missing_required_files
                .iter()
                .map(|f| format!("{f} (/cores/{})", c.folder)),
        );

        let artwork_target = self.resolve_artwork_target(&c.core.id, first_pid);
        Ok(CoreProfile {
            device: DeviceKind::GameBub,
            id: c.core.id.clone(),
            name: if c.core.name.is_empty() {
                c.core.id
            } else {
                c.core.name
            },
            author: c.core.author,
            description: c.core.description,
            version: c.core.version,
            release_date: c.core.release_date,
            url: c.core.url,
            license: c.core.license,
            platform_ids,
            capabilities: caps,
            facts,
            targets,
            rom_destination,
            artwork_target,
            missing_items,
        })
    }

    fn resolve_rom_destination(
        &self,
        card: &Path,
        core_id: &str,
        platform_id: &str,
    ) -> Option<RomDestination> {
        if let Ok(p) = self.inspect_core(card, core_id)
            && p.rom_destination.is_some()
        {
            return p.rom_destination;
        }
        let pid = canonicalize_platform_id(platform_id);
        let exts = platforms::find(&pid)
            .map(|s| s.extensions.iter().map(|e| (*e).to_string()).collect())
            .unwrap_or_default();
        Some(RomDestination {
            base_rel: format!("roms/{pid}"),
            label: format!("Choose SD folder (suggested /roms/{pid})"),
            user_chooses_folder: true,
            extensions: exts,
        })
    }

    fn resolve_artwork_target(&self, core_id: &str, _platform_id: Option<&str>) -> ArtworkTarget {
        ArtworkTarget {
            mode: ArtworkMode::AppOverrideOnly,
            banner_rel: None,
            icon_rel: None,
            override_key: format!("game_bub:core:{core_id}"),
        }
    }
}

/// Returns the static driver for a supported handheld system.
pub fn driver_for(device: DeviceKind) -> &'static dyn SystemDriver {
    match device {
        DeviceKind::AnaloguePocket => &POCKET_DRIVER,
        DeviceKind::GameBub => &GAMEBUB_DRIVER,
    }
}

/// Lists normalized core summaries for `device` on `card`.
pub fn list_cores(card: &Path, device: DeviceKind) -> Result<Vec<CoreSummary>, TauError> {
    driver_for(device).list_cores(card)
}

/// Returns the normalized core profile for `core_id` on `card`.
pub fn inspect_core(
    card: &Path,
    device: DeviceKind,
    core_id: &str,
) -> Result<CoreProfile, TauError> {
    driver_for(device).inspect_core(card, core_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn normalizes_pocket_and_gamebub_cores() {
        let root = std::env::temp_dir().join(format!("tau-sys-{}", crate::test_uniq()));
        let pocket = root.join("pocket");
        fs::create_dir_all(pocket.join("Cores/Spiritualized.GB")).unwrap();
        fs::create_dir_all(pocket.join("Platforms")).unwrap();
        fs::write(
            pocket.join("Cores/Spiritualized.GB/core.json"),
            r#"{"core":{"magic":"APF_VER_1","metadata":{"platform_ids":["gb"],"shortname":"GB","author":"Spiritualized","version":"1.3.0","date_release":"2026-01-01"},"framework":{"target_product":"Analogue Pocket","version_required":"1.1"}}}"#,
        )
        .unwrap();
        fs::write(
            pocket.join("Cores/Spiritualized.GB/data.json"),
            r#"{"data":{"magic":"APF_VER_1","data_slots":[{"name":"Cartridge","id":0,"required":true,"extensions":["gb"]}]}}"#,
        )
        .unwrap();

        let p_list = list_cores(&pocket, DeviceKind::AnaloguePocket).unwrap();
        assert_eq!(p_list.len(), 1);
        assert_eq!(p_list[0].platform_ids, ["gb"]);
        assert!(p_list[0].capabilities.can_toggle_visibility);
        let p_prof = inspect_core(&pocket, DeviceKind::AnaloguePocket, "Spiritualized.GB").unwrap();
        assert_eq!(
            p_prof.artwork_target.banner_rel.as_deref(),
            Some("Platforms/_images/gb.bin")
        );
        assert_eq!(p_prof.rom_destination.unwrap().base_rel, "Assets/gb/common");

        let gb = root.join("gamebub");
        fs::create_dir_all(gb.join("cores/Retro-Bub.Lynx")).unwrap();
        fs::write(
            gb.join("cores/Retro-Bub.Lynx/core.json"),
            r#"{"min_firmware":"1.0.3","metadata":{"id":"Retro-Bub.Lynx","name":"Atari Lynx","author":"Florin","version":"0.3.3"},"bitstreams":[{"target":"gamebub_rev4","filename":"lynx.bin"}]}"#,
        )
        .unwrap();
        fs::write(
            gb.join("cores/Retro-Bub.Lynx/files.json"),
            r#"{"files":[{"id":0,"label":"ROM","user_selected":true,"extensions":["lnx"]}]}"#,
        )
        .unwrap();

        let g_list = list_cores(&gb, DeviceKind::GameBub).unwrap();
        assert_eq!(g_list.len(), 1);
        assert_eq!(g_list[0].platform_ids, ["lynx"]);
        assert!(!g_list[0].capabilities.can_toggle_visibility);
        let g_prof = inspect_core(&gb, DeviceKind::GameBub, "Retro-Bub.Lynx").unwrap();
        assert_eq!(g_prof.artwork_target.mode, ArtworkMode::AppOverrideOnly);
        assert!(g_prof.rom_destination.unwrap().user_chooses_folder);

        let _ = fs::remove_dir_all(&root);
    }
}
