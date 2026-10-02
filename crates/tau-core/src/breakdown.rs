//! Where the space on a card goes: each Tau platform's media, versus everything
//! else. Read-only: it walks names and sizes and never opens a file's contents,
//! so on a slow link it costs directory reads only.
//!
//! Sizes are **on disk** (each file rounded up to the volume's allocation unit),
//! because that is what `total - available` counts; summing logical bytes would
//! make the "other data" figure absorb all the cluster waste.

use crate::{Progress, ProgressObserver, Stage, TauError, tick};
use std::path::Path;

/// What the caller already knows about a core (from `inspect_card`), so this does
/// not re-read every core's index just to learn its platform.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreRef {
    pub id: String,
    pub shortname: String,
    pub platform: String,
    pub library_capable: bool,
}

/// The Tau family: a core that serves the library index, or whose id starts with
/// `alfatreze.TAU`. Cores that are neither count as "other data".
pub fn is_tau_core(core: &CoreRef) -> bool {
    core.library_capable || core.id.to_ascii_lowercase().starts_with("alfatreze.tau")
}

/// One platform folder's share of the card. Cores that share a platform (for
/// example `TAU` and `TAU_DIAGNOSTIC`) share its media, so they are one segment.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MediaSegment {
    pub platform: String,
    pub core_ids: Vec<String>,
    pub shortnames: Vec<String>,
    pub bytes_on_disk: u64,
    pub files: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CardBreakdown {
    pub total_bytes: u64,
    pub available_bytes: u64,
    /// The volume's allocation unit used for rounding (bytes).
    pub unit: u64,
    /// One entry per Tau platform folder, ordered by platform name.
    pub segments: Vec<MediaSegment>,
    pub tau_bytes: u64,
    /// Used space that is not Tau media: other cores, saves, screenshots, system files.
    pub other_bytes: u64,
}

/// Size on disk and file count of everything under `dir`. Symlinks are never
/// followed (a link out of the card must not count the rest of the disk), and
/// anything unreadable is skipped rather than failing the whole measurement.
fn walk(
    dir: &Path,
    unit: u64,
    progress: &mut Option<&mut dyn ProgressObserver>,
    seen: &mut u64,
) -> Result<(u64, u64), TauError> {
    let mut bytes = 0u64;
    let mut files = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(folder) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file()
                && let Ok(meta) = entry.metadata()
            {
                bytes = bytes.saturating_add(crate::sync::round_up_to(meta.len(), unit));
                files += 1;
                *seen += 1;
                if (*seen).is_multiple_of(512) {
                    tick(
                        progress,
                        Progress {
                            stage: Stage::Scanning,
                            done: *seen,
                            total: 0,
                            path: Some(folder.to_string_lossy().into_owned()),
                        },
                    )?;
                }
            }
        }
    }
    Ok((bytes, files))
}

/// Measures the Tau media on a card and how it compares with everything else.
pub fn card_breakdown(
    root: &Path,
    cores: &[CoreRef],
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<CardBreakdown, TauError> {
    let space = crate::storage::volume_space(root)?;
    let unit = fs4::allocation_granularity(root)
        .ok()
        .filter(|u| *u > 0)
        .unwrap_or(4096);
    let mut platforms: std::collections::BTreeMap<String, (Vec<String>, Vec<String>)> =
        std::collections::BTreeMap::new();
    for core in cores
        .iter()
        .filter(|c| is_tau_core(c) && !c.platform.is_empty())
    {
        let entry = platforms.entry(core.platform.clone()).or_default();
        entry.0.push(core.id.clone());
        entry.1.push(core.shortname.clone());
    }
    let mut segments = Vec::new();
    let mut seen = 0u64;
    let mut tau_bytes = 0u64;
    for (platform, (core_ids, shortnames)) in platforms {
        let dir = root.join("Assets").join(&platform);
        let (bytes_on_disk, files) = if dir.is_dir() {
            walk(&dir, unit, progress, &mut seen)?
        } else {
            (0, 0)
        };
        tau_bytes = tau_bytes.saturating_add(bytes_on_disk);
        segments.push(MediaSegment {
            platform,
            core_ids,
            shortnames,
            bytes_on_disk,
            files,
        });
    }
    let used = space.total_bytes.saturating_sub(space.available_bytes);
    Ok(CardBreakdown {
        total_bytes: space.total_bytes,
        available_bytes: space.available_bytes,
        unit,
        segments,
        tau_bytes,
        other_bytes: used.saturating_sub(tau_bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn core(id: &str, shortname: &str, platform: &str, library: bool) -> CoreRef {
        CoreRef {
            id: id.into(),
            shortname: shortname.into(),
            platform: platform.into(),
            library_capable: library,
        }
    }

    fn card(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tau-breakdown-{name}-{}",
            std::process::id() as u128 + crate::test_uniq()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn put(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![7u8; bytes]).unwrap();
    }

    #[test]
    fn the_tau_family_is_library_cores_and_the_alfatreze_tau_prefix() {
        assert!(is_tau_core(&core("alfatreze.TAU", "TAU", "tau", false)));
        assert!(is_tau_core(&core(
            "alfatreze.TAU_DEV_61",
            "TAU_DEV_61",
            "tau_dev_61",
            false
        )));
        assert!(is_tau_core(&core(
            "someone.player",
            "player",
            "mp3player",
            true
        )));
        assert!(!is_tau_core(&core("agg23.GB", "GB", "gb", false)));
    }

    #[test]
    fn media_is_counted_per_platform_in_whole_clusters_and_other_data_is_the_rest() {
        let root = card("segments");
        put(&root.join("Assets/tau/common/a/01.mp3"), 1000);
        put(&root.join("Assets/tau/common/a/02.mp3"), 5000);
        put(&root.join("Assets/tau/alfatreze.TAU/core.json"), 10);
        put(&root.join("Assets/tau_dev_61/common/x.flac"), 70_000);
        put(&root.join("Assets/gb/common/game.gb"), 999_999); // not a Tau core's media
        let cores = [
            core("alfatreze.TAU", "TAU", "tau", true),
            core("alfatreze.TAU_DIAGNOSTIC", "TAU_DIAGNOSTIC", "tau", true), // shares the platform
            core("alfatreze.TAU_DEV_61", "TAU_DEV_61", "tau_dev_61", true),
            core("agg23.GB", "GB", "gb", false),
        ];
        let b = card_breakdown(&root, &cores, &mut None).unwrap();
        let unit = b.unit;
        assert_eq!(
            b.segments.len(),
            2,
            "tau and tau_dev_61; the gb platform is not Tau media"
        );
        let tau = b.segments.iter().find(|s| s.platform == "tau").unwrap();
        assert_eq!(
            tau.core_ids,
            ["alfatreze.TAU", "alfatreze.TAU_DIAGNOSTIC"],
            "cores sharing a platform are one segment"
        );
        assert_eq!(tau.files, 3);
        let round = |n: u64| n.div_ceil(unit) * unit;
        assert_eq!(tau.bytes_on_disk, round(1000) + round(5000) + round(10));
        let dev = b
            .segments
            .iter()
            .find(|s| s.platform == "tau_dev_61")
            .unwrap();
        assert_eq!(dev.bytes_on_disk, round(70_000));
        assert_eq!(b.tau_bytes, tau.bytes_on_disk + dev.bytes_on_disk);
        let used = b.total_bytes - b.available_bytes;
        assert_eq!(b.other_bytes, used.saturating_sub(b.tau_bytes));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_missing_platform_folder_is_an_empty_segment_not_an_error() {
        let root = card("missing");
        let b = card_breakdown(
            &root,
            &[core("alfatreze.TAU", "TAU", "tau", true)],
            &mut None,
        )
        .unwrap();
        assert_eq!(b.segments.len(), 1);
        assert_eq!((b.segments[0].bytes_on_disk, b.segments[0].files), (0, 0));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        let root = card("symlink");
        let elsewhere = card("symlink-target");
        put(&elsewhere.join("huge.bin"), 4_000_000);
        put(&root.join("Assets/tau/common/01.mp3"), 1000);
        std::os::unix::fs::symlink(&elsewhere, root.join("Assets/tau/common/link")).unwrap();
        std::os::unix::fs::symlink(
            elsewhere.join("huge.bin"),
            root.join("Assets/tau/common/filelink"),
        )
        .unwrap();
        let b = card_breakdown(
            &root,
            &[core("alfatreze.TAU", "TAU", "tau", true)],
            &mut None,
        )
        .unwrap();
        assert_eq!(b.segments[0].files, 1);
        assert!(b.segments[0].bytes_on_disk < 4_000_000);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(elsewhere).unwrap();
    }

    #[test]
    fn a_cancel_request_stops_the_walk() {
        let root = card("cancel");
        for n in 0..600 {
            put(&root.join(format!("Assets/tau/common/{n}.mp3")), 10);
        }
        let mut stop = |_: crate::Progress| false;
        let mut observer: Option<&mut dyn ProgressObserver> = Some(&mut stop);
        let error = card_breakdown(
            &root,
            &[core("alfatreze.TAU", "TAU", "tau", true)],
            &mut observer,
        )
        .unwrap_err();
        assert_eq!(error.code(), crate::ErrorCode::Cancelled);
        fs::remove_dir_all(root).unwrap();
    }
}
