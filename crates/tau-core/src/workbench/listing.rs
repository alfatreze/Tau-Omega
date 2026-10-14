//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

/// One album folder: a directory that directly contains audio files.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlbumInfo {
    /// Folder path relative to the listed root, `/` separated. Empty when the
    /// audio sits directly in the root.
    pub id: String,
    /// Where this folder lands on a card: `id` with every component in the
    /// device's ASCII form. Equal to `id` for a listing of a card.
    pub dest_id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<String>,
    pub tracks: usize,
    pub bytes: u64,
    pub has_cover: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrackInfo {
    pub rel: String,
    pub album_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub secs: u16,
    pub bytes: u64,
    pub format: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlaylistInfo {
    pub name: String,
    pub file: String,
    pub tracks: usize,
}

/// The Pocket index's hard capacity limits, so a front-end can show how full
/// the library is and stop before an over-limit sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndexLimits {
    pub max_tracks: usize,
    pub max_albums: usize,
    pub max_artists: usize,
}
impl Default for IndexLimits {
    fn default() -> Self {
        Self {
            max_tracks: crate::MAX_TRACKS,
            max_albums: crate::MAX_ALBUMS,
            max_artists: crate::MAX_ARTISTS,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LibraryListing {
    pub limits: IndexLimits,
    pub albums: Vec<AlbumInfo>,
    pub tracks: Vec<TrackInfo>,
    pub playlists: Vec<PlaylistInfo>,
    pub warnings: Vec<Warning>,
    /// Files whose tags came from the verification ledger instead of being read from the file.
    pub files_reused: u64,
    /// Files whose tags were read from the file itself.
    pub files_read: u64,
}

/// Device ASCII form of a `/`-separated relative folder path.
pub fn dest_dir(id: &str) -> String {
    id.split('/')
        .filter(|part| !part.is_empty())
        .map(sync::ascii_file_name)
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn most_common(values: impl Iterator<Item = String>) -> Option<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for value in values.filter(|v| !v.trim().is_empty()) {
        *counts.entry(value).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(value, _)| value)
}

/// Lists a media root as albums (one per audio folder), tracks and playlists.
/// Read-only.
pub fn list_library(
    root: &Path,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<LibraryListing, TauError> {
    list_library_with(root, false, progress)
}

/// Audio files the Pocket cannot play but this tool can convert (WAV, AIFF, M4A, Ogg, ...), as listing
/// entries with their tags read from the file. Only used for a source library.
fn convertible_entries(root: &Path) -> Vec<crate::Entry> {
    let mut files = Vec::new();
    let _ = crate::scan::collect_files(root, root, &mut files);
    files.sort();
    let mut out = Vec::new();
    for path in files {
        if !crate::convert::convertible_extension(&path) {
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        let Ok(mut decoder) = tau_audio::decode::Decoder::open(&path) else {
            continue;
        };
        let meta = decoder.meta();
        let info = decoder.info.clone();
        let mut tags = BTreeMap::new();
        for (key, value) in [
            ("TIT2", meta.title),
            ("TPE1", meta.artist),
            ("TALB", meta.album),
            ("TPE2", meta.album_artist),
            ("TDRC", meta.year),
            ("TRCK", meta.track.map(|n| n.to_string())),
            ("TPOS", meta.disc.map(|n| n.to_string())),
        ] {
            if let Some(v) = value.filter(|v| !v.is_empty()) {
                tags.insert(key.to_string(), v);
            }
        }
        let (dir, file) = rel
            .rsplit_once('/')
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .unwrap_or((String::new(), rel.clone()));
        let secs = info
            .frames
            .map(|f| (f / info.rate.max(1) as u64).min(65_535) as u16)
            .unwrap_or(0);
        out.push(crate::Entry {
            rel,
            dir,
            file,
            tags,
            secs,
            fmt: 0,
        });
    }
    out
}

/// [`list_library`], optionally also listing files that can be converted for the Pocket.
pub fn list_library_with(
    root: &Path,
    include_convertible: bool,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<LibraryListing, TauError> {
    if !root.is_dir() {
        return Err(TauError::e(
            ErrorCode::NotFound,
            format!("not a folder: {}", root.display()),
        ));
    }
    let mut scan = scan_dir_with_progress(root, true, progress)?;
    if include_convertible {
        scan.entries.extend(convertible_entries(root));
    }
    let mut by_dir: BTreeMap<String, Vec<&crate::Entry>> = BTreeMap::new();
    for entry in &scan.entries {
        by_dir.entry(entry.dir.clone()).or_default().push(entry);
    }
    let root_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut listing = LibraryListing {
        warnings: scan.warnings.clone(),
        files_reused: scan.reused,
        files_read: scan.read,
        ..Default::default()
    };
    for (dir, entries) in by_dir {
        let tag = |e: &crate::Entry, key: &str| e.tags.get(key).cloned().unwrap_or_default();
        let folder_name = dir
            .rsplit('/')
            .next()
            .filter(|n| !n.is_empty())
            .unwrap_or(&root_name)
            .to_string();
        let title = most_common(entries.iter().map(|e| tag(e, "TALB"))).unwrap_or(folder_name);
        let artist = most_common(entries.iter().map(|e| tag(e, "TPE2")))
            .or_else(|| most_common(entries.iter().map(|e| tag(e, "TPE1"))))
            .unwrap_or_default();
        let year = most_common(
            entries
                .iter()
                .map(|e| format!("{}{}", tag(e, "TDRC"), tag(e, "TYER"))),
        )
        .map(|y| y.chars().take(4).collect::<String>())
        .filter(|y| y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()));
        let mut bytes = 0;
        for entry in &entries {
            let size = fs::metadata(root.join(&entry.rel))
                .map(|m| m.len())
                .unwrap_or(0);
            bytes += size;
            listing.tracks.push(TrackInfo {
                rel: entry.rel.clone(),
                album_id: dir.clone(),
                title: {
                    let t = tag(entry, "TIT2");
                    if t.is_empty() {
                        entry
                            .file
                            .rsplit_once('.')
                            .map_or(entry.file.clone(), |(stem, _)| stem.to_string())
                    } else {
                        t
                    }
                },
                artist: {
                    let a = tag(entry, "TPE1");
                    if a.is_empty() { tag(entry, "TPE2") } else { a }
                },
                album: tag(entry, "TALB"),
                secs: entry.secs,
                bytes: size,
                format: match entry.fmt {
                    2 => "FLAC".to_string(),
                    1 => "MP3".to_string(),
                    _ => entry
                        .file
                        .rsplit_once('.')
                        .map_or("AUDIO".to_string(), |(_, e)| e.to_uppercase()),
                },
            });
        }
        let has_cover = cover::find_cover(&root.join(&dir)).is_some();
        listing.albums.push(AlbumInfo {
            dest_id: dest_dir(&dir),
            id: dir,
            title,
            artist,
            year,
            tracks: entries.len(),
            bytes,
            has_cover,
        });
    }
    listing.albums.sort_by(|a, b| {
        (a.artist.to_lowercase(), a.title.to_lowercase())
            .cmp(&(b.artist.to_lowercase(), b.title.to_lowercase()))
    });
    listing.playlists = scan
        .playlists
        .iter()
        .map(|p| PlaylistInfo {
            name: p.name.clone(),
            file: p.file.clone(),
            tracks: p.rel_ids.len(),
        })
        .collect();
    Ok(listing)
}

/// Counts the audio files under a media root and the folders that hold them,
/// without reading any tags (cheap enough to run on every plan).
pub(crate) fn count_audio(
    root: &Path,
) -> Result<(usize, std::collections::BTreeSet<String>), TauError> {
    fn walk(
        root: &Path,
        at: &Path,
        tracks: &mut usize,
        dirs: &mut std::collections::BTreeSet<String>,
    ) -> Result<(), TauError> {
        for child in fs::read_dir(at)? {
            let path = child?.path();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.starts_with("._") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, tracks, dirs)?;
            } else if sync::audio_file(&path) {
                *tracks += 1;
                let dir = path
                    .parent()
                    .and_then(|p| p.strip_prefix(root).ok())
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                dirs.insert(dir);
            }
        }
        Ok(())
    }
    let mut tracks = 0;
    let mut dirs = std::collections::BTreeSet::new();
    walk(root, root, &mut tracks, &mut dirs)?;
    Ok((tracks, dirs))
}

/// One album's cover thumbnail: a small PNG, base64-encoded, or `None` when the
/// album has no readable picture (the UI then shows a placeholder).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Thumbnail {
    pub id: String,
    pub png_base64: Option<String>,
}

/// Cover thumbnails for the given album folders under `root`: the folder's
/// cover picture if it has one, otherwise the picture embedded in its first
/// track. Never fails the batch because one album has no or a broken picture.
/// Read-only.
pub fn album_thumbnails(
    root: &Path,
    ids: &[String],
    long_side: u16,
) -> Result<Vec<Thumbnail>, TauError> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let picture = checked_dir(root, id).ok().and_then(|dir| {
            let bytes = match cover::find_cover(&dir) {
                Some(file) => fs::read(file).ok(),
                None => direct_files(&dir, false)
                    .ok()?
                    .into_iter()
                    .filter(|f| sync::audio_file(f))
                    .find_map(|f| cover::extract_embedded_cover(&f)),
            }?;
            if bytes.len() > 16 << 20 {
                return None;
            }
            crate::image::thumbnail_png(&bytes, long_side).ok()
        });
        out.push(Thumbnail {
            id: id.clone(),
            png_base64: picture.map(|p| STANDARD.encode(p)),
        });
    }
    Ok(out)
}

/// Rejects an album id that could leave `root` (absolute, `..`, prefixes).
pub(crate) fn checked_dir(root: &Path, id: &str) -> Result<PathBuf, TauError> {
    let relative = Path::new(id);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            format!("album id must be a relative folder: {id}"),
        ));
    }
    let dir = root.join(relative);
    if !dir.is_dir() {
        return Err(TauError::e(
            ErrorCode::NotFound,
            format!("album folder not found: {id}"),
        ));
    }
    Ok(dir)
}

/// Audio (and album-local playlist) files directly in `dir`, sorted.
pub(crate) fn direct_files(dir: &Path, convertible: bool) -> Result<Vec<PathBuf>, TauError> {
    let mut files = Vec::new();
    for child in fs::read_dir(dir)? {
        let child = child?;
        let name = child.file_name().to_string_lossy().into_owned();
        let path = child.path();
        if path.is_file()
            && !sync::is_junk(&name)
            && (sync::supported(&path)
                || (convertible && crate::convert::convertible_extension(&path)))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
