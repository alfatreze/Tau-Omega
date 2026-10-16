//! Where a core's files live on a card. Tau's Preview and Dev cores build on
//! their own platform (`tau_preview`, `tau_dev`) but read the library, covers
//! and music from `Assets/tau/common` through data-slot parameter bits
//! [25:24] (the index into `platform_ids`). Every part of Omega that needs the
//! media folder asks here, so there is one rule.

use serde_json::Value;
use std::{fs, path::Path};

/// Data-slot parameter bit 1: the file lives in `Assets/<platform>/<core>/`.
pub const SLOT_CORE_SPECIFIC: u64 = 0x2;

pub fn slot_parameters(slot: &Value) -> u64 {
    match slot.get("parameters") {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(Value::String(s)) => {
            let t = s.trim();
            t.strip_prefix("0x")
                .or_else(|| t.strip_prefix("0X"))
                .map_or_else(|| t.parse().ok(), |h| u64::from_str_radix(h, 16).ok())
                .unwrap_or(0)
        }
        _ => 0,
    }
}

/// Index into `platform_ids` a slot reads from (parameter bits [25:24]).
pub fn slot_platform_index(slot: &Value) -> usize {
    ((slot_parameters(slot) >> 24) & 3) as usize
}

fn slots_of(data_json: &Value) -> Vec<&Value> {
    data_json
        .pointer("/data/data_slots")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect()
}

/// The audio formats a core's data slots accept (`mp3`, `flac`), lower case, in that order. Empty for a core that reads neither.
pub fn audio_formats(data_json: Option<&Value>) -> Vec<String> {
    let all: Vec<String> = data_json
        .map(|j| {
            slots_of(j)
                .into_iter()
                .filter_map(|s| s.get("extensions").and_then(Value::as_array))
                .flatten()
                .filter_map(|e| {
                    e.as_str()
                        .map(|e| e.trim_start_matches('.').to_ascii_lowercase())
                })
                .collect()
        })
        .unwrap_or_default();
    ["mp3", "flac"]
        .iter()
        .filter(|f| all.iter().any(|e| e == *f))
        .map(|f| (*f).to_string())
        .collect()
}

/// How a core that plays music only from a playlist reads it, worked out from the core's own data slots (a slot that accepts `.m3u`, another that
/// accepts mp3 or flac, and no library slot).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlaylistSupport {
    /// The playlist file the core reads, relative to the media root. The slot's own file name when it names one, else `playlist.m3u`.
    pub file: String,
    /// The slot names this file, so the core only ever opens exactly this one.
    pub fixed_name: bool,
    /// The most tracks the core plays. Not part of the core's data slots: it comes from what the core's author documents (see [`KNOWN_LIMITS`]).
    pub max_tracks: Option<u32>,
    /// The largest playlist file the core reads, in bytes; past it the core drops the rest (about 48 characters per line at 256 tracks).
    pub max_bytes: Option<u32>,
    /// A remembered playlist name longer than this is forgotten at the next launch unless the playlist is listed in `list_file`.
    pub name_limit: Option<u8>,
    /// The playlist that lists other playlists by name, when the core has one.
    pub list_file: Option<String>,
    pub formats: Vec<String>,
}

/// What a core's author documents for its playlists (core id lower-case): tracks per playlist, `.m3u` size in bytes, longest remembered playlist name and
/// the playlist-of-playlists file. Not in a core's own files, so listed here in one place. HarpMudd.Mp3Player (its README, Limits): 256 tracks, 12 KB, 12 characters.
pub const KNOWN_LIMITS: [(&str, u32, u32, u8, &str); 1] =
    [("harpmudd.mp3player", 256, 12 * 1024, 12, "playlists.m3u")];

/// Recognises a core that plays music only from a playlist: it has a data slot for `.m3u`, a slot that accepts mp3 or flac, and no library slot
/// (`library_capable`). Returns `None` for anything else.
pub fn playlist_support(
    data_json: Option<&Value>,
    library_capable: bool,
    core_id: &str,
) -> Option<PlaylistSupport> {
    let data = data_json?;
    if library_capable {
        return None;
    }
    let formats = audio_formats(Some(data));
    if formats.is_empty() {
        return None;
    }
    let slots = slots_of(data);
    let name_of = |s: &Value| {
        s.get("filename")
            .and_then(Value::as_str)
            .filter(|f| f.to_ascii_lowercase().ends_with(".m3u"))
            .map(str::to_string)
    };
    let takes_m3u = |s: &Value| {
        s.get("extensions")
            .and_then(Value::as_array)
            .is_some_and(|e| {
                e.iter()
                    .filter_map(Value::as_str)
                    .any(|x| x.trim_start_matches('.').eq_ignore_ascii_case("m3u"))
            })
    };
    let slot = slots
        .iter()
        .find(|s| name_of(s).is_some() || takes_m3u(s))?;
    let named = name_of(slot);
    let known = KNOWN_LIMITS
        .iter()
        .find(|k| k.0.eq_ignore_ascii_case(core_id));
    Some(PlaylistSupport {
        fixed_name: named.is_some(),
        file: named.unwrap_or_else(|| "playlist.m3u".to_string()),
        max_tracks: known.map(|k| k.1),
        max_bytes: known.map(|k| k.2),
        name_limit: known.map(|k| k.3),
        list_file: known.map(|k| k.4.to_string()),
        formats,
    })
}

/// The platform ids a core declares, in order (empty when core.json is unreadable).
pub fn platform_ids(core_json: &Value) -> Vec<String> {
    core_json
        .pointer("/core/metadata/platform_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

/// The platform whose `common/` holds the core's library: the platform picked
/// by the `tau-library.tdb` slot, else the first declared platform.
pub fn media_platform(platforms: &[String], data_json: Option<&Value>) -> String {
    let index = data_json
        .and_then(|json| {
            slots_of(json)
                .into_iter()
                .find(|s| s.get("filename").and_then(Value::as_str) == Some("tau-library.tdb"))
        })
        .map(slot_platform_index)
        .unwrap_or(0);
    platforms
        .get(index)
        .or_else(|| platforms.first())
        .cloned()
        .unwrap_or_default()
}

/// Reads a core's declared platforms and media platform from the card.
pub fn core_platforms(card_root: &Path, core_id: &str) -> (Vec<String>, String) {
    let dir = card_root.join("Cores").join(core_id);
    let read = |name: &str| {
        fs::read(dir.join(name))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
    };
    let platforms = read("core.json")
        .map(|j| platform_ids(&j))
        .unwrap_or_default();
    let media = media_platform(&platforms, read("data.json").as_ref());
    (platforms, media)
}

/// Another core (not `except`) that lists `platform` in any position.
pub fn platform_used_by_other(card_root: &Path, platform: &str, except: &str) -> bool {
    other_cores(card_root, except)
        .into_iter()
        .any(|(_, platforms, _)| platforms.iter().any(|p| p == platform))
}

/// True when another core still reads `filename` from the platform's `common/`
/// (a slot naming it without the core-specific bit, on that platform).
pub fn read_from_common_by_other(
    card_root: &Path,
    platform: &str,
    filename: &str,
    except: &str,
) -> bool {
    let cores_dir = card_root.join("Cores");
    let Ok(rd) = fs::read_dir(&cores_dir) else {
        return false;
    };
    for entry in rd.flatten() {
        let id = entry.file_name().to_string_lossy().to_string();
        if id == except || !entry.path().is_dir() {
            continue;
        }
        let read = |name: &str| {
            fs::read(entry.path().join(name))
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        };
        let platforms = read("core.json")
            .map(|j| platform_ids(&j))
            .unwrap_or_default();
        let Some(data) = read("data.json") else {
            continue;
        };
        for slot in slots_of(&data) {
            if slot.get("filename").and_then(Value::as_str) != Some(filename)
                || slot_parameters(slot) & SLOT_CORE_SPECIFIC != 0
            {
                continue;
            }
            let p = platforms
                .get(slot_platform_index(slot))
                .or_else(|| platforms.first());
            if p.map(String::as_str) == Some(platform) {
                return true;
            }
        }
    }
    false
}

fn other_cores(card_root: &Path, except: &str) -> Vec<(String, Vec<String>, String)> {
    let Ok(rd) = fs::read_dir(card_root.join("Cores")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let id = entry.file_name().to_string_lossy().to_string();
        if id == except || !entry.path().is_dir() {
            continue;
        }
        let (platforms, media) = core_platforms(card_root, &id);
        out.push((id, platforms, media));
    }
    out
}

/// Whether a manifest-declared obsolete path may ever be deleted for `core_id`.
/// Only the core's own areas qualify: its `Cores/<id>/` folder, its
/// `Assets/<platform>/<id>/` folders, platform files of a platform no other
/// core uses, and a build-bound file moved out of `common/` (decided by the
/// caller with [`read_from_common_by_other`]). User data is never allowed.
pub fn obsolete_path_allowed(
    card_root: &Path,
    core_id: &str,
    platforms: &[String],
    owned: &[String],
    path: &str,
) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == "..")
        || parts.len() < 2
    {
        return false;
    }
    match parts[0] {
        "Cores" => parts[1] == core_id,
        "Assets" => {
            if parts.len() == 4 && parts[2] == "common" && platforms.iter().any(|p| p == parts[1]) {
                // A build-bound file moved out of common/: only when the same
                // package now owns it in this core's own folder and no other
                // core still reads it from common/.
                let moved = format!("Assets/{}/{core_id}/{}", parts[1], parts[3]);
                return owned.contains(&moved)
                    && !read_from_common_by_other(card_root, parts[1], parts[3], core_id);
            }
            parts.len() >= 4 && platforms.iter().any(|p| p == parts[1]) && parts[2] == core_id
        }
        "Platforms" => {
            let name = parts[1..].join("/");
            let stem = name.split('.').next().unwrap_or("");
            let in_image_dir = parts.get(1) == Some(&"_images");
            let stem = if in_image_dir {
                parts
                    .get(2)
                    .map(|n| n.split('.').next().unwrap_or(""))
                    .unwrap_or("")
            } else {
                stem
            };
            platforms.iter().any(|p| p == stem) && !platform_used_by_other(card_root, stem, core_id)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn audio_formats_come_from_the_slot_extensions() {
        let j: serde_json::Value = serde_json::from_str(r#"{"data":{"data_slots":[{"name":"Music","extensions":["MP3",".flac","wav"]},{"name":"x","extensions":["gba"]}]}}"#).unwrap();
        assert_eq!(super::audio_formats(Some(&j)), ["mp3", "flac"]);
        let only: serde_json::Value =
            serde_json::from_str(r#"{"data":{"data_slots":[{"extensions":["flac"]}]}}"#).unwrap();
        assert_eq!(super::audio_formats(Some(&only)), ["flac"]);
        assert!(super::audio_formats(None).is_empty());
    }
    use super::*;
    use serde_json::json;

    fn core(card: &Path, id: &str, platforms: &[&str], slots: Value) {
        let dir = card.join("Cores").join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("core.json"),
            json!({"core":{"metadata":{"platform_ids":platforms}}}).to_string(),
        )
        .unwrap();
        fs::write(
            dir.join("data.json"),
            json!({"data":{"data_slots":slots}}).to_string(),
        )
        .unwrap();
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("tau-cardlayout-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn media_platform_follows_the_library_slot() {
        let card = tmp("media");
        core(
            &card,
            "alfatreze.TAU Preview",
            &["tau_preview", "tau"],
            json!([
                {"id":1,"filename":"tau.rom","parameters":"0x2"},
                {"id":5,"filename":"tau-library.tdb","parameters":"0x1000000"}
            ]),
        );
        let (platforms, media) = core_platforms(&card, "alfatreze.TAU Preview");
        assert_eq!(platforms, ["tau_preview", "tau"]);
        assert_eq!(media, "tau");
        core(
            &card,
            "alfatreze.TAU",
            &["tau"],
            json!([{"id":5,"filename":"tau-library.tdb","parameters":"0x0"}]),
        );
        assert_eq!(core_platforms(&card, "alfatreze.TAU").1, "tau");
        let _ = fs::remove_dir_all(&card);
    }

    #[test]
    fn platform_use_counts_any_position() {
        let card = tmp("use");
        core(&card, "a.One", &["tau"], json!([]));
        core(&card, "a.Two", &["tau_dev", "tau"], json!([]));
        assert!(platform_used_by_other(&card, "tau", "a.One"));
        assert!(!platform_used_by_other(&card, "tau_dev", "a.Two"));
        assert!(platform_used_by_other(&card, "tau_dev", "a.One"));
        let _ = fs::remove_dir_all(&card);
    }

    #[test]
    fn common_read_by_other_needs_a_non_core_specific_slot() {
        let card = tmp("common");
        core(
            &card,
            "a.New",
            &["tau"],
            json!([{"id":1,"filename":"tau.rom","parameters":"0x2"}]),
        );
        core(
            &card,
            "a.Old",
            &["tau"],
            json!([{"id":1,"filename":"tau.rom","parameters":"0x0"}]),
        );
        assert!(read_from_common_by_other(&card, "tau", "tau.rom", "a.New"));
        assert!(!read_from_common_by_other(&card, "tau", "tau.rom", "a.Old"));
        let _ = fs::remove_dir_all(&card);
    }

    #[test]
    fn obsolete_filter_stays_inside_the_core() {
        let card = tmp("obs");
        core(&card, "a.One", &["tau"], json!([]));
        let p = ["tau".to_string()];
        for ok in [
            "Cores/a.One/old.json",
            "Assets/tau/a.One/tau.rom",
            "Platforms/tau.json",
            "Platforms/_images/tau.bin",
        ] {
            assert!(obsolete_path_allowed(&card, "a.One", &p, &[], ok), "{ok}");
        }
        for bad in [
            "Cores/other/core.json",
            "Assets/tau/common/song.mp3",
            "Assets/tau/common/tau-library.tdb",
            "Saves/tau/a.sav",
            "Settings/a.One/x",
            "Memories/Screenshots/a.png",
            "System/cores_cache.bin",
            "Cores/../Saves/x",
            "/etc/passwd",
            "Assets/other/a.One/x",
        ] {
            assert!(
                !obsolete_path_allowed(&card, "a.One", &p, &[], bad),
                "{bad}"
            );
        }
        let owned = ["Assets/tau/a.One/tau.rom".to_string()];
        core(
            &card,
            "a.One",
            &["tau"],
            json!([{"id":1,"filename":"tau.rom","parameters":"0x2"}]),
        );
        assert!(obsolete_path_allowed(
            &card,
            "a.One",
            &p,
            &owned,
            "Assets/tau/common/tau.rom"
        ));
        assert!(
            !obsolete_path_allowed(&card, "a.One", &p, &[], "Assets/tau/common/tau.rom"),
            "not owned by the package"
        );
        assert!(!obsolete_path_allowed(
            &card,
            "a.One",
            &p,
            &owned,
            "Assets/tau/common/song.mp3"
        ));
        core(
            &card,
            "a.Old",
            &["tau"],
            json!([{"id":1,"filename":"tau.rom","parameters":"0x0"}]),
        );
        assert!(
            !obsolete_path_allowed(&card, "a.One", &p, &owned, "Assets/tau/common/tau.rom"),
            "another core reads it"
        );
        core(&card, "a.Two", &["tau", "x"], json!([]));
        assert!(!obsolete_path_allowed(
            &card,
            "a.One",
            &p,
            &[],
            "Platforms/tau.json"
        ));
        let _ = fs::remove_dir_all(&card);
    }

    #[test]
    fn a_core_that_plays_only_from_a_playlist_is_recognised_from_its_own_slots() {
        let harpmudd = json!({"data":{"data_slots":[
            {"name":"Firmware","id":1,"required":true,"parameters":"0x108","filename":"mp3player.rom"},
            {"name":"MP3","id":2,"deferload":true,"parameters":"0x1","extensions":["mp3","flac"]},
            {"name":"Playlist","id":3,"deferload":true,"parameters":"0x1","filename":"playlist.m3u","extensions":["m3u"]},
            {"name":"Lyrics","id":4,"extensions":["lrc"]}]}});
        let s = playlist_support(Some(&harpmudd), false, "HarpMudd.Mp3Player").unwrap();
        assert_eq!(
            (
                s.file.as_str(),
                s.fixed_name,
                s.max_tracks,
                s.formats.as_slice()
            ),
            (
                "playlist.m3u",
                true,
                Some(256),
                ["mp3".to_string(), "flac".to_string()].as_slice()
            )
        );
        // another core with the same shape has no published limit
        assert_eq!(
            playlist_support(Some(&harpmudd), false, "someone.Other")
                .unwrap()
                .max_tracks,
            None
        );
        // a library core, a player with no playlist slot, a playlist with nothing to play, and no data at all are not playlist players
        assert!(playlist_support(Some(&harpmudd), true, "x.Y").is_none());
        let no_pl = json!({"data":{"data_slots":[{"name":"MP3","id":2,"extensions":["mp3"]}]}});
        assert!(playlist_support(Some(&no_pl), false, "x.Y").is_none());
        let no_audio =
            json!({"data":{"data_slots":[{"name":"Playlist","id":3,"extensions":["m3u"]}]}});
        assert!(playlist_support(Some(&no_audio), false, "x.Y").is_none());
        assert!(playlist_support(None, false, "x.Y").is_none());
        // a playlist slot that names no file leaves the name to the user's choice, with a default
        let open = json!({"data":{"data_slots":[{"name":"Audio","id":2,"extensions":["flac"]},{"name":"List","id":3,"extensions":[".M3U"]}]}});
        let o = playlist_support(Some(&open), false, "x.Y").unwrap();
        assert_eq!((o.file.as_str(), o.fixed_name), ("playlist.m3u", false));
    }
}
