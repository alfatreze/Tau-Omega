//! The host half of the verification ledger (`tau_core::ledger`, design in `docs/PERFORMANCE_AUDIT.md`):
//! *which card is this* and *where does its ledger live*.
//!
//! The ledger is a cache in the app's own cache folder, never on the card. It is keyed by the volume's UUID
//! plus its capacity plus the media folder (`Assets/<platform>/common`), never by a mount path: two cards
//! mounted at the same name must not share one. A volume with no UUID we can read, a folder that is not a
//! card media root (a local music library, say), or a system we cannot identify volumes on gets **no ledger**
//! and everything behaves exactly as before.

use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
};

static CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();
/// (mount path, device id) -> volume UUID, so the OS tool runs once per mount, not once per scan. The device
/// id changes when a card is unmounted and mounted again, so a swapped card never reuses an old answer.
static VOLUMES: Mutex<Option<HashMap<(PathBuf, u64), Option<String>>>> = Mutex::new(None);

/// Registers the ledger with the engine; `cache_dir` is the app's cache folder.
pub fn init(cache_dir: PathBuf) {
    let _ = CACHE_DIR.set(cache_dir.join("ledger"));
    tau_core::ledger::register_locator(locate);
}

/// `Assets/tau/common` -> `tau_common`: the part of a media root that names it, or `None` when the folder is
/// not under an `Assets` folder (so it is not a card media root).
pub fn media_root_tail(root: &Path) -> Option<String> {
    let parts: Vec<String> = root.components().filter_map(|c| match c { Component::Normal(n) => Some(n.to_string_lossy().into_owned()), _ => None }).collect();
    let at = parts.iter().rposition(|p| p == "Assets")?;
    let tail = &parts[at + 1..];
    (!tail.is_empty()).then(|| sanitize(&tail.join("_")))
}

fn sanitize(text: &str) -> String {
    text.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

/// The `Volume UUID` line of `diskutil info` output.
#[allow(dead_code)] // only called on macOS; unit-tested everywhere
pub fn parse_volume_uuid(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let value = line.trim().strip_prefix("Volume UUID:")?.trim();
        (value.len() >= 8 && value.chars().all(|c| c.is_ascii_hexdigit() || c == '-')).then(|| value.to_string())
    })
}

#[cfg(target_os = "macos")]
fn volume_uuid(root: &Path) -> Option<String> {
    let out = std::process::Command::new("diskutil").arg("info").arg(root).output().ok()?;
    out.status.success().then(|| parse_volume_uuid(&String::from_utf8_lossy(&out.stdout))).flatten()
}

/// Linux: the volume's UUID is the `/dev/disk/by-uuid` link that points at its device.
#[cfg(target_os = "linux")]
fn volume_uuid(root: &Path) -> Option<String> {
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    let device = crate::device::linux_device_for(&mounts, root)?;
    let device = std::fs::canonicalize(&device).ok()?;
    std::fs::read_dir("/dev/disk/by-uuid").ok()?.flatten().find_map(|e| {
        (std::fs::canonicalize(e.path()).ok()? == device).then(|| e.file_name().to_string_lossy().into_owned())
    })
}

/// Other systems: no way to identify a volume here yet, so no ledger.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn volume_uuid(_root: &Path) -> Option<String> {
    None
}

#[cfg(unix)]
fn device_id(root: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(root).map_or(0, |m| m.dev())
}
#[cfg(not(unix))]
fn device_id(_root: &Path) -> u64 {
    0
}

/// The mount point holding `path`: the highest folder still on the same device. The OS tools that identify a
/// volume (`diskutil info`) want the mount point, not a folder inside it.
fn mount_point(path: &Path) -> PathBuf {
    let device = device_id(path);
    let mut top = path;
    while let Some(parent) = top.parent() {
        if parent.as_os_str().is_empty() || device_id(parent) != device {
            break;
        }
        top = parent;
    }
    top.to_path_buf()
}

fn volume_key(root: &Path) -> Option<String> {
    let mount = mount_point(root);
    let id = (mount.clone(), device_id(&mount));
    let cached = VOLUMES.lock().ok()?.get_or_insert_with(HashMap::new).get(&id).cloned();
    let uuid = match cached {
        Some(known) => known,
        None => {
            let found = volume_uuid(&mount);
            if let Ok(mut map) = VOLUMES.lock() {
                map.get_or_insert_with(HashMap::new).insert(id, found.clone());
            }
            found
        }
    }?;
    let total = tau_core::storage::volume_space(root).ok()?.total_bytes;
    Some(format!("{}-{total}", sanitize(&uuid)))
}

/// The engine's locator: the ledger file for the card holding this media root, or `None` for "no ledger".
fn locate(root: &Path) -> Option<PathBuf> {
    let dir = CACHE_DIR.get()?;
    Some(dir.join(format!("{}-{}.tauledger", volume_key(root)?, media_root_tail(root)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_media_folder_names_the_ledger_and_other_folders_get_none() {
        assert_eq!(media_root_tail(Path::new("/Volumes/Pock/Assets/tau/common")).as_deref(), Some("tau_common"));
        assert_eq!(media_root_tail(Path::new("/Volumes/Pock/Assets/tau_dev_63/common/")).as_deref(), Some("tau_dev_63_common"));
        assert_eq!(media_root_tail(Path::new("/Users/me/Documents/Music")), None, "a local library is not a card media root");
        assert_eq!(media_root_tail(Path::new("/Volumes/Pock/Assets")), None);
    }

    #[test]
    fn the_volume_uuid_is_read_from_diskutil_output_and_nothing_else_counts() {
        let real = "   Device Identifier:         disk4s1\n   Volume Name:               Pock\n   Volume UUID:               4A5B6C7D-1234-5678-9ABC-DEF012345678\n   Disk / Partition UUID:     11111111-2222-3333-4444-555555555555\n";
        assert_eq!(parse_volume_uuid(real).as_deref(), Some("4A5B6C7D-1234-5678-9ABC-DEF012345678"), "the volume's UUID, not the partition's");
        assert_eq!(parse_volume_uuid("   Volume UUID:               Not applicable\n"), None);
        assert_eq!(parse_volume_uuid("nonsense"), None);
    }
}

/// `#[ignore]`d: mounts a throwaway FAT disk image to prove a real mounted volume gets a ledger path, and that
/// a local folder does not. Run with `cargo test --features tau-core/serde -- --ignored real_volume`.
#[cfg(all(test, target_os = "macos"))]
mod real_volume_check {
    use super::*;
    use std::{fs, process::Command};

    #[test]
    #[ignore]
    fn a_mounted_volume_gets_a_ledger_path_and_a_local_folder_does_not() {
        init(std::env::temp_dir().join(format!("tau-ledger-host-{}", std::process::id())));
        let dir = std::env::temp_dir().join(format!("tau-ledger-dmg-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let image = dir.join("l.dmg");
        let ok = |c: &mut Command| c.output().map(|o| o.status.success()).unwrap_or(false);
        assert!(ok(Command::new("hdiutil").args(["create", "-size", "16m", "-fs", "MS-DOS", "-volname", "TAULEDGER", "-type", "UDIF"]).arg(&image)));
        let attach = Command::new("hdiutil").args(["attach", "-nobrowse"]).arg(&image).output().unwrap();
        let text = String::from_utf8_lossy(&attach.stdout).into_owned();
        let device = text.lines().find(|l| l.contains("/Volumes/TAULEDGER")).and_then(|l| l.split_whitespace().next()).unwrap().to_string();
        let result = std::panic::catch_unwind(|| {
            let root = Path::new("/Volumes/TAULEDGER/Assets/tau/common");
            fs::create_dir_all(root).unwrap();
            let path = locate(root).expect("a real volume with a UUID gets a ledger");
            assert!(path.to_string_lossy().ends_with("-tau_common.tauledger"), "{path:?}");
            assert_eq!(locate(root), Some(path), "the same card gives the same answer every time");
            assert!(locate(Path::new("/Volumes/TAULEDGER")).is_none(), "the volume root is not a media root");
            assert!(locate(&std::env::temp_dir()).is_none(), "a local folder gets no ledger");
        });
        let _ = Command::new("hdiutil").args(["detach", &device, "-force"]).output();
        let _ = fs::remove_dir_all(&dir);
        result.unwrap();
    }
}

/// `#[ignore]`d, read-only: scans every Tau media folder on the mounted Pocket twice and checks that the
/// second pass reads no file contents and returns the same answer. The ledger is written to a temporary cache
/// folder, never to the card. Run with `cargo test --features tau-core/serde -- --ignored --nocapture real_pocket`.
#[cfg(all(test, target_os = "macos"))]
mod real_pocket_check {
    use super::*;
    use std::time::Instant;

    #[test]
    #[ignore]
    fn the_second_scan_of_the_real_pocket_reads_nothing() {
        let card = Path::new("/Volumes/Pock");
        if !card.is_dir() {
            eprintln!("no Pocket mounted: nothing to check");
            return;
        }
        let cache = std::env::temp_dir().join(format!("tau-ledger-pocket-{}", std::process::id()));
        init(cache.clone());
        let mut checked = 0;
        for platform in std::fs::read_dir(card.join("Assets")).unwrap().flatten() {
            let root = platform.path().join("common");
            if !root.join("tau-library.tdb").is_file() {
                continue;
            }
            let path = locate(&root).expect("the Pocket's volume gets a ledger");
            let t = Instant::now();
            let cold = tau_core::scan_dir(&root, false).unwrap();
            let cold_time = t.elapsed();
            let t = Instant::now();
            let warm = tau_core::scan_dir(&root, false).unwrap();
            let warm_time = t.elapsed();
            eprintln!(
                "{:<18} {:>4} tracks   cold: read {:>4} in {:>8.1?}   warm: read {:>3}, reused {:>4} in {:>8.1?}   ledger {} bytes",
                platform.file_name().to_string_lossy(), cold.entries.len(), cold.read, cold_time, warm.read, warm.reused, warm_time,
                std::fs::metadata(&path).map_or(0, |m| m.len())
            );
            assert_eq!(cold.entries, warm.entries, "the cached answer must equal the read one");
            assert_eq!(warm.read, 0, "nothing needed reading the second time");
            checked += 1;
        }
        let _ = std::fs::remove_dir_all(cache);
        assert!(checked > 0, "no Tau media folder with a library index was found on the Pocket");
    }
}
