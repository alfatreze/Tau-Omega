//! Read-only, expanded library-health checks for a scanned media root:
//! duplicate content, missing tags, path issues that a card sync would
//! silently rewrite or collide on, an ID3 version the cover embedder can't
//! handle, and folders with no cover art at all. Nothing here writes to disk.

use crate::{cover, duplicates, Entry, TauError};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ProblemKind {
    Duplicate,
    MissingTag,
    UnsupportedFormat,
    PathIssue,
    MissingCover,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Problem {
    pub kind: ProblemKind,
    pub files: Vec<String>,
    pub message: String,
}

/// Runs every read-only check against a scanned media root's entries.
pub fn find_problems(root: impl AsRef<Path>, entries: &[Entry]) -> Result<Vec<Problem>, TauError> {
    let root = root.as_ref();
    let mut problems = duplicate_problems(root, entries)?;
    problems.extend(tag_problems(entries));
    problems.extend(path_problems(entries));
    problems.extend(format_and_cover_problems(root, entries));
    Ok(problems)
}

fn duplicate_problems(root: &Path, entries: &[Entry]) -> Result<Vec<Problem>, TauError> {
    Ok(duplicates::find_duplicates(root, entries)?
        .into_iter()
        .map(|files| Problem {
            kind: ProblemKind::Duplicate,
            message: format!("{} files share identical content", files.len()),
            files,
        })
        .collect())
}

fn tag_problems(entries: &[Entry]) -> Vec<Problem> {
    entries
        .iter()
        .filter_map(|entry| {
            let mut missing = Vec::new();
            if !entry.tags.contains_key("TIT2") {
                missing.push("title");
            }
            if !entry.tags.contains_key("TPE1") && !entry.tags.contains_key("TPE2") {
                missing.push("artist");
            }
            if missing.is_empty() {
                return None;
            }
            Some(Problem {
                kind: ProblemKind::MissingTag,
                message: format!("missing {}", missing.join(" and ")),
                files: vec![entry.rel.clone()],
            })
        })
        .collect()
}

/// Non-ASCII names, over-length paths, and names that collide once folded to
/// the on-card ASCII a sync would actually write (see [`crate::ascii_name`]).
fn path_problems(entries: &[Entry]) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut folded: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for entry in entries {
        if !entry.rel.is_ascii() {
            problems.push(Problem {
                kind: ProblemKind::PathIssue,
                message: "non-ASCII filename will be renamed when synced to a card".into(),
                files: vec![entry.rel.clone()],
            });
        }
        if entry.rel.len() > crate::MAX_PATH {
            problems.push(Problem {
                kind: ProblemKind::PathIssue,
                message: format!(
                    "path exceeds the firmware's {}-character limit",
                    crate::MAX_PATH
                ),
                files: vec![entry.rel.clone()],
            });
        }
        folded
            .entry((entry.dir.clone(), crate::ascii_name(&entry.file)))
            .or_default()
            .push(entry.rel.clone());
    }
    for files in folded.into_values() {
        if files.len() > 1 {
            problems.push(Problem {
                kind: ProblemKind::PathIssue,
                message: "these filenames collide once folded to on-card ASCII".into(),
                files,
            });
        }
    }
    problems
}

/// ID3v2.2 tags (the cover embedder only handles v2.3/v2.4) and folders
/// with neither a folder-level cover file nor any embedded art.
fn format_and_cover_problems(root: &Path, entries: &[Entry]) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut folder_has_cover: BTreeMap<String, bool> = BTreeMap::new();
    let mut missing_cover: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in entries {
        let path = root.join(&entry.rel);
        if entry.fmt != 2
            && let Ok(bytes) = fs::read(&path)
            && bytes.len() > 3
            && bytes.starts_with(b"ID3")
            && bytes[3] == 2
        {
            problems.push(Problem {
                kind: ProblemKind::UnsupportedFormat,
                message: "ID3v2.2 tags are not supported for cover embedding".into(),
                files: vec![entry.rel.clone()],
            });
        }
        let has_folder_cover = *folder_has_cover
            .entry(entry.dir.clone())
            .or_insert_with(|| cover::find_cover(&root.join(&entry.dir)).is_some());
        if !has_folder_cover && !cover::has_embedded_cover(&path).unwrap_or(false) {
            missing_cover
                .entry(entry.dir.clone())
                .or_default()
                .push(entry.rel.clone());
        }
    }
    for (dir, files) in missing_cover {
        let label = if dir.is_empty() {
            "the root folder".to_string()
        } else {
            dir
        };
        problems.push(Problem {
            kind: ProblemKind::MissingCover,
            message: format!(
                "{label} has no folder cover and no embedded art on {} track(s)",
                files.len()
            ),
            files,
        });
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::BTreeMap,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn entry(rel: &str, tags: &[(&str, &str)], fmt: u8) -> Entry {
        let (dir, file) = rel
            .rsplit_once('/')
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .unwrap_or((String::new(), rel.to_string()));
        Entry {
            rel: rel.into(),
            dir,
            file,
            tags: tags
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<BTreeMap<_, _>>(),
            secs: 1,
            fmt,
        }
    }

    fn scratch_root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tau-problems-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn flags_missing_tags_and_path_collisions() {
        let root = scratch_root("tags");
        fs::write(root.join("café.mp3"), b"a").unwrap();
        fs::write(root.join("cafe.mp3"), b"b").unwrap();
        let entries = [
            entry("café.mp3", &[("TIT2", "Song"), ("TPE1", "Artist")], 1),
            entry("cafe.mp3", &[], 1),
        ];
        let problems = find_problems(&root, &entries).unwrap();
        assert!(problems
            .iter()
            .any(|p| p.kind == ProblemKind::MissingTag && p.files == ["cafe.mp3"]));
        assert!(problems
            .iter()
            .any(|p| p.kind == ProblemKind::PathIssue && p.files == ["café.mp3"]));
        assert!(problems.iter().any(|p| p.kind == ProblemKind::PathIssue
            && p.files.len() == 2
            && p.message.contains("collide")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn flags_folders_with_no_cover_and_id3v22() {
        let root = scratch_root("cover");
        // Minimal ID3v2.2 header: "ID3", version 2, flags 0, size 0.
        fs::write(root.join("no-cover.mp3"), b"ID3\x02\x00\x00\x00\x00\x00\x00").unwrap();
        let entries = [entry(
            "no-cover.mp3",
            &[("TIT2", "Song"), ("TPE1", "Artist")],
            1,
        )];
        let problems = find_problems(&root, &entries).unwrap();
        assert!(problems
            .iter()
            .any(|p| p.kind == ProblemKind::UnsupportedFormat));
        assert!(problems
            .iter()
            .any(|p| p.kind == ProblemKind::MissingCover && p.files == ["no-cover.mp3"]));
        fs::remove_dir_all(root).unwrap();
    }
}
