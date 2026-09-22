//! Portable playlist import/export helpers.

use crate::{Entry, ErrorCode, Playlist, TauError};
use std::{fs, path::Path};

/// Exports a playlist as a conventional UTF-8 `.m3u` using media-relative paths.
pub fn export_m3u(
    path: impl AsRef<Path>,
    playlist: &Playlist,
    entries: &[Entry],
) -> Result<(), TauError> {
    let mut output = String::new();
    for &id in &playlist.rel_ids {
        let entry = entries.get(id).ok_or_else(|| TauError {
            code: ErrorCode::IndexRecordRange,
            message: "playlist item".into(),
        })?;
        output.push_str(&entry.rel);
        output.push('\n');
    }
    fs::write(path, output).map_err(TauError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::BTreeMap,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn exports_relative_lines_in_playlist_order() {
        let entries = vec![
            Entry {
                rel: "Artist/B/02.mp3".into(),
                dir: "Artist/B".into(),
                file: "02.mp3".into(),
                tags: BTreeMap::new(),
                secs: 1,
                fmt: 1,
            },
            Entry {
                rel: "Artist/A/01.flac".into(),
                dir: "Artist/A".into(),
                file: "01.flac".into(),
                tags: BTreeMap::new(),
                secs: 1,
                fmt: 2,
            },
        ];
        let playlist = Playlist {
            name: "Mix".into(),
            rel_ids: vec![1, 0],
        };
        let path = std::env::temp_dir().join(format!(
            "tau-playlist-{}.m3u",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        export_m3u(&path, &playlist, &entries).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "Artist/A/01.flac\nArtist/B/02.mp3\n"
        );
        fs::remove_file(path).unwrap();
    }
}
