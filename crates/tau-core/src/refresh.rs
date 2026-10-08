//! Refresh library (roadmap 1d): bring a core's `tau-library.tdb` back in line with the music actually in its media
//! folder, after files were copied by hand, renamed, or moved between cores.
//!
//! The Pocket trusts the index and opens files by the paths written in it, in ASCII only (the firmware cannot open
//! anything else), within 200 bytes. A hand-copied library breaks that in ways that show up only on the device: tracks
//! that will not open, a library that will not build at all. This module finds every such file, says why in plain
//! words, fixes what can be fixed safely on the card's own copy, and rebuilds the index rooted at *this* core.
//!
//! What it does and does not touch (SAFETY_RULES 4/6/7): only files inside the core's media folder
//! (`Assets/<platform>/common`); music files are never changed, only renamed to their ASCII form (folders and files),
//! and playlists (`.m3u`) that point at renamed files are rewritten to match. Everything it replaces is backed up
//! outside the card first, and [`rollback_refresh`] undoes a run from the backup alone.
//!
//! Not covered (the engine does not read it): a file's sample rate or bit depth, so a file the Pocket will refuse to
//! play for that reason is still listed.

use crate::{
    ErrorCode, MAX_ALBUMS, MAX_ARTISTS, MAX_TRACKS, TauError, WarningCode, ascii_name, ascii_text,
    build_index, parse, root_prefix, scan_dir, string_at, sync, verify,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

const MAX_PATH: usize = 200;
const INDEX: &str = "tau-library.tdb";
const JOURNAL: &str = "refresh-journal.json";
/// Audio formats people copy over that the Pocket cannot play.
const OTHER_AUDIO: [&str; 15] = [
    "m4a", "wav", "ogg", "opus", "aac", "aif", "aiff", "wma", "ape", "wv", "mpc", "mp2", "mp4",
    "webm", "mka",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Reason {
    /// An audio format the Pocket cannot play (not MP3 or FLAC): never in the library.
    UnsupportedFormat,
    /// A name with non-ASCII characters: the Pocket cannot open it. Fixed by renaming to the ASCII form.
    NonAsciiName,
    /// Folding the name to ASCII would collide with a file or folder already there: left alone, not in the library.
    NameCollision,
    /// The path is over the 200-byte limit even with the core's prefix: not in the library until shortened.
    PathTooLong,
    /// The tags could not be read: the track is listed by its file name instead.
    TagsUnreadable,
    /// ID3v2.2 tags: the Pocket shows file names instead of tags for these.
    OldTagVersion,
    /// Beyond the index's capacity (tracks, albums or artists): the library cannot be built.
    OverCapacity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Finding {
    pub rel: String,
    pub reason: Reason,
    /// Plain-language explanation for the user.
    pub message: String,
    /// The path this will be renamed to, when the refresh fixes it.
    pub fix: Option<String>,
    /// True when the file will not be in the library after the refresh.
    pub skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rename {
    /// Card-relative to the media folder; applied in order, so a folder is renamed before what is inside it.
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlaylistFix {
    /// The playlist's path after the renames.
    pub file: String,
    pub lines: usize,
}

/// What the library on the card looks like right now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndexState {
    pub present: bool,
    pub valid: bool,
    pub tracks: Option<u16>,
    /// The root path the index embeds (what the Pocket prepends to every track).
    pub root: Option<String>,
    /// The root equals this core's own (`/Assets/<platform>/common/`).
    pub root_matches: bool,
    /// Listed tracks whose file is not on the card.
    pub missing_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RefreshPlan {
    pub id: String,
    pub media_root: PathBuf,
    pub root_prefix: String,
    pub before: IndexState,
    /// MP3 and FLAC files found in the media folder.
    pub media_files: usize,
    /// Tracks the rebuilt library will list.
    pub would_index: usize,
    pub findings: Vec<Finding>,
    pub renames: Vec<Rename>,
    pub playlist_fixes: Vec<PlaylistFix>,
    /// `._` files from the operating system that will be removed.
    pub stubs_to_remove: usize,
    /// `Some(reason)` when the refresh must not run.
    pub refused: Option<String>,
    /// The library already matches the music: nothing would change.
    pub nothing_to_do: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RefreshReport {
    pub backup_dir: PathBuf,
    pub renamed: usize,
    pub playlists_rewritten: usize,
    pub stubs_removed: usize,
    pub before: IndexState,
    pub after: IndexState,
    pub index_path: PathBuf,
    pub nothing_to_do: bool,
}

fn refuse(message: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::RefreshRefused, message)
}

/// The library's health as a cheap read (no tag scan): is there an index, is it valid, is it rooted at this core, are
/// all its files there. Used for the badge; [`plan_refresh`] does the full comparison.
pub fn index_state(media_root: &Path) -> IndexState {
    let prefix = root_prefix(media_root).unwrap_or_default();
    let Ok(bytes) = fs::read(media_root.join(INDEX)) else {
        return IndexState::default();
    };
    let mut state = IndexState {
        present: true,
        ..IndexState::default()
    };
    let Ok(index) = parse(&bytes) else {
        return state;
    };
    state.valid = true;
    state.tracks = Some(index.counts.tracks);
    state.root = string_at(&index, index.root).ok().map(str::to_string);
    state.root_matches = state.root.as_deref() == Some(prefix.as_str());
    state.missing_files = if state.root_matches {
        verify(&bytes, Some(media_root))
            .map(|v| v.len())
            .unwrap_or(0)
    } else {
        index.counts.tracks as usize
    };
    state
}

/// All regular files under the media folder (relative, `/`-separated, with size), skipping nothing.
fn walk(root: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for e in read.flatten() {
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() {
                stack.push(e.path());
            } else if kind.is_file()
                && let Ok(rel) = e.path().strip_prefix(root)
            {
                out.push((
                    rel.to_string_lossy().replace('\\', "/"),
                    e.metadata().map(|m| m.len()).unwrap_or(0),
                ));
            }
        }
    }
    out.sort();
    out
}

/// Plans the renames that make non-ASCII names ASCII, and which files are blocked because the folded name would
/// collide. Names are compared case-insensitively (exFAT is). Returns the ordered renames, the final path of every
/// path that changes (files and folders), and the blocked originals with the reason.
type RenamePlan = (
    Vec<Rename>,
    BTreeMap<String, String>,
    BTreeMap<String, String>,
);

fn plan_renames(all_rels: &[String]) -> RenamePlan {
    // Every original component name per parent, lowercased, to detect collisions.
    let mut siblings: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for rel in all_rels {
        let parts: Vec<&str> = rel.split('/').collect();
        for i in 0..parts.len() {
            siblings
                .entry(parts[..i].join("/"))
                .or_default()
                .insert(parts[i].to_lowercase());
        }
    }
    let mut mapped: BTreeMap<String, String> = BTreeMap::new(); // original prefix -> final prefix
    let mut targets: BTreeMap<String, BTreeSet<String>> = BTreeMap::new(); // original parent -> lowercase targets used
    let mut blocked: BTreeMap<String, String> = BTreeMap::new(); // original prefix -> reason
    let mut renames: Vec<(usize, String, Rename)> = Vec::new();
    let mut prefixes: BTreeSet<(usize, String)> = BTreeSet::new();
    for rel in all_rels {
        let parts: Vec<&str> = rel.split('/').collect();
        for i in 1..=parts.len() {
            prefixes.insert((i, parts[..i].join("/")));
        }
    }
    for (depth, prefix) in prefixes {
        let parts: Vec<&str> = prefix.split('/').collect();
        let parent_orig = parts[..depth - 1].join("/");
        let name = parts[depth - 1];
        if blocked.contains_key(&parent_orig) {
            blocked.insert(prefix.clone(), blocked[&parent_orig].clone());
            continue;
        }
        let parent_final = if parent_orig.is_empty() {
            String::new()
        } else {
            mapped
                .get(&parent_orig)
                .cloned()
                .unwrap_or_else(|| parent_orig.clone())
        };
        let join = |p: &str, n: &str| {
            if p.is_empty() {
                n.to_string()
            } else {
                format!("{p}/{n}")
            }
        };
        if name.is_ascii() {
            mapped.insert(prefix.clone(), join(&parent_final, name));
            continue;
        }
        let folded = ascii_name(name);
        let lower = folded.to_lowercase();
        let taken = targets.entry(parent_orig.clone()).or_default();
        if folded.is_empty()
            || siblings
                .get(&parent_orig)
                .is_some_and(|s| s.contains(&lower))
            || taken.contains(&lower)
        {
            let why = if folded.is_empty() {
                format!("\"{name}\" has no ASCII form")
            } else {
                format!("\"{name}\" would become \"{folded}\", which is already there")
            };
            blocked.insert(prefix.clone(), why);
            continue;
        }
        taken.insert(lower);
        renames.push((
            depth,
            prefix.clone(),
            Rename {
                from: join(&parent_final, name),
                to: join(&parent_final, &folded),
            },
        ));
        mapped.insert(prefix, join(&parent_final, &folded));
    }
    renames.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    let order = renames.into_iter().map(|(_, _, r)| r).collect();
    let changed = mapped.into_iter().filter(|(k, v)| k != v).collect();
    (order, changed, blocked)
}

/// Resolves a playlist line to a path relative to the media root, the way the Pocket does.
fn resolve_line(base: &str, line: &str) -> String {
    match line.strip_prefix('/') {
        Some(rest) => rest.to_string(),
        None if base.is_empty() => line.to_string(),
        None => format!("{base}/{line}"),
    }
}

/// Rewrites a playlist's lines that point at renamed files. `orig_base` and `new_base` are the playlist's folder
/// before and after the renames; `final_of` maps an original path to its final one. Returns the new text and how many
/// lines changed.
fn rewrite_playlist(
    text: &str,
    orig_base: &str,
    new_base: &str,
    final_of: &dyn Fn(&str) -> Option<String>,
) -> (String, usize) {
    let mut changed = 0;
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let line = raw.trim_end_matches('\r');
        let trimmed = line.trim();
        let ending = &raw[line.len()..];
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out.push(raw.to_string());
            continue;
        }
        let orig = resolve_line(orig_base, trimmed);
        let Some(new) = final_of(&orig) else {
            // Not renamed itself, but the playlist may have moved: keep the relative form valid.
            if orig_base != new_base && !trimmed.starts_with('/') {
                let target = orig.clone();
                let rel = relative_to(&target, new_base);
                if rel != trimmed {
                    changed += 1;
                    out.push(format!("{rel}{ending}"));
                    continue;
                }
            }
            out.push(raw.to_string());
            continue;
        };
        let text_line = if trimmed.starts_with('/') {
            format!("/{new}")
        } else {
            relative_to(&new, new_base)
        };
        if text_line != trimmed {
            changed += 1;
        }
        out.push(format!("{text_line}{ending}"));
    }
    (out.join("\n"), changed)
}

fn relative_to(path: &str, base: &str) -> String {
    if base.is_empty() {
        path.to_string()
    } else {
        path.strip_prefix(&format!("{base}/"))
            .unwrap_or(path)
            .to_string()
    }
}

struct Analysis {
    plan: RefreshPlan,
    /// Playlists to rewrite: (original path, path after the renames, new text).
    playlists: Vec<(String, String, String)>,
}

fn analyse(media_root: &Path) -> Result<Analysis, TauError> {
    sync::validate_media_root(media_root)?;
    let prefix = root_prefix(media_root)?;
    let files = walk(media_root);
    let before = index_state(media_root);
    let scan = scan_dir(media_root, false)?;
    let stubs = files
        .iter()
        .filter(|(rel, size)| {
            rel.rsplit('/').next().is_some_and(|n| n.starts_with("._")) && *size <= 64 * 1024
        })
        .count();
    let media_files = scan.entries.len();
    let mut findings = Vec::new();

    for (rel, _) in &files {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        if name.starts_with("._") {
            continue;
        }
        let ext = name
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        if OTHER_AUDIO.contains(&ext.as_str()) {
            findings.push(Finding {
                rel: rel.clone(),
                reason: Reason::UnsupportedFormat,
                message: format!("The Pocket plays MP3 and FLAC only, not .{ext}; this file is not in the library."),
                fix: None,
                skipped: true,
            });
        }
    }

    // Renames: every path that holds a media file or a playlist.
    let mut all_rels: Vec<String> = scan.entries.iter().map(|e| e.rel.clone()).collect();
    let playlist_files: Vec<String> = files
        .iter()
        .filter(|(rel, _)| {
            rel.to_ascii_lowercase().ends_with(".m3u")
                && !rel.rsplit('/').next().unwrap_or("").starts_with("._")
        })
        .map(|(rel, _)| rel.clone())
        .collect();
    all_rels.extend(playlist_files.iter().cloned());
    all_rels.sort();
    all_rels.dedup();
    let (renames, changed, blocked) = plan_renames(&all_rels);
    let final_of = |orig: &str| -> Option<String> {
        // The final path of a file: its longest changed prefix applied.
        let parts: Vec<&str> = orig.split('/').collect();
        let mut result: Option<String> = None;
        for i in (1..=parts.len()).rev() {
            let prefix = parts[..i].join("/");
            if let Some(new) = changed.get(&prefix) {
                let rest = parts[i..].join("/");
                result = Some(if rest.is_empty() {
                    new.clone()
                } else {
                    format!("{new}/{rest}")
                });
                break;
            }
        }
        result
    };

    let mut excluded = BTreeSet::new();
    let mut keep = Vec::new();
    for e in &scan.entries {
        let blocked_by = {
            let parts: Vec<&str> = e.rel.split('/').collect();
            (1..=parts.len()).find_map(|i| blocked.get(&parts[..i].join("/")).cloned())
        };
        let final_rel = final_of(&e.rel).unwrap_or_else(|| e.rel.clone());
        if let Some(why) = blocked_by {
            findings.push(Finding {
                rel: e.rel.clone(),
                reason: Reason::NameCollision,
                message: format!("{why}; the file is left as it is and is not in the library until you rename it."),
                fix: None,
                skipped: true,
            });
            excluded.insert(e.rel.clone());
            continue;
        }
        if !e.rel.is_ascii() {
            findings.push(Finding {
                rel: e.rel.clone(),
                reason: Reason::NonAsciiName,
                message: "The Pocket cannot open names with accents or other non-ASCII characters; this is renamed to its plain form.".into(),
                fix: Some(final_rel.clone()),
                skipped: false,
            });
        }
        if prefix.len() + final_rel.len() > MAX_PATH {
            findings.push(Finding {
                rel: e.rel.clone(),
                reason: Reason::PathTooLong,
                message: format!(
                    "With the folder prefix this path is {} bytes; the Pocket allows {MAX_PATH}. Shorten the folder or file name; until then it is not in the library.",
                    prefix.len() + final_rel.len()
                ),
                fix: None,
                skipped: true,
            });
            excluded.insert(e.rel.clone());
            continue;
        }
        keep.push((e, final_rel));
    }
    for w in &scan.warnings {
        if w.code == WarningCode::TagsUnreadable
            && let Some(rel) = w.message.split(": tags unreadable").next()
            && !excluded.contains(rel)
        {
            findings.push(Finding {
                rel: rel.to_string(),
                reason: Reason::TagsUnreadable,
                message:
                    "The tags could not be read, so the Pocket lists this track by its file name."
                        .into(),
                fix: None,
                skipped: false,
            });
        }
    }
    for e in scan.entries.iter().filter(|e| !excluded.contains(&e.rel)) {
        if e.fmt == 1
            && fs::read(media_root.join(&e.rel))
                .ok()
                .is_some_and(|b| b.len() > 3 && b.starts_with(b"ID3") && b[3] == 2)
        {
            findings.push(Finding {
                rel: e.rel.clone(),
                reason: Reason::OldTagVersion,
                message: "ID3v2.2 tags are an old version the Pocket does not read, so it lists this track by its file name.".into(),
                fix: None,
                skipped: false,
            });
        }
    }

    // Plain-letter spelling of every renamed file -> its final path (see the playlist fallback below).
    let lenient: BTreeMap<String, String> = scan
        .entries
        .iter()
        .filter(|e| !e.rel.is_ascii())
        .filter_map(|e| final_of(&e.rel).map(|f| (ascii_text(&e.rel, usize::MAX), f)))
        .collect();
    // Playlists whose lines need rewriting.
    let mut playlists = Vec::new();
    let mut playlist_fixes = Vec::new();
    for orig in &playlist_files {
        let final_path = final_of(orig).unwrap_or_else(|| orig.clone());
        let (orig_base, new_base) = (
            orig.rsplit_once('/').map(|x| x.0).unwrap_or("").to_string(),
            final_path
                .rsplit_once('/')
                .map(|x| x.0)
                .unwrap_or("")
                .to_string(),
        );
        let Ok(bytes) = fs::read(media_root.join(orig)) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes).to_string();
        // A playlist written on another machine may spell an accented name differently from how the card stores it
        // (macOS stores "é" decomposed): fall back to matching on the plain-letter form.
        let (new_text, lines) = rewrite_playlist(&text, &orig_base, &new_base, &|p| {
            final_of(p).or_else(|| {
                (!p.is_ascii())
                    .then(|| lenient.get(&ascii_text(p, usize::MAX)).cloned())
                    .flatten()
            })
        });
        if lines > 0 {
            playlist_fixes.push(PlaylistFix {
                file: final_path.clone(),
                lines,
            });
            playlists.push((orig.clone(), final_path, new_text));
        }
    }

    // Would the index build? (Dry run in memory.)
    let mut refused = None;
    let would_index = keep.len();
    if would_index > MAX_TRACKS {
        findings.push(Finding {
            rel: String::new(),
            reason: Reason::OverCapacity,
            message: format!("{would_index} tracks is more than the Pocket's library holds ({MAX_TRACKS}); remove some albums."),
            fix: None,
            skipped: true,
        });
        refused = Some(format!(
            "The library would hold {would_index} tracks; the most the Pocket's index can list is {MAX_TRACKS}."
        ));
    } else if would_index == 0 && before.present {
        refused = Some(
            "There is no playable music in this folder, so the existing library is left as it is."
                .into(),
        );
    } else if would_index > 0 {
        let entries: Vec<_> = keep
            .iter()
            .map(|(e, final_rel)| {
                let mut e = (*e).clone();
                let (dir, file) = final_rel
                    .rsplit_once('/')
                    .map(|(a, b)| (a.to_string(), b.to_string()))
                    .unwrap_or((String::new(), final_rel.clone()));
                e.rel = final_rel.clone();
                e.dir = dir;
                e.file = file;
                e
            })
            .collect();
        if let Err(error) = build_index(&entries, &[], &prefix, &mut Vec::new()) {
            if matches!(error.code(), ErrorCode::IndexCapExceeded) {
                findings.push(Finding {
                    rel: String::new(),
                    reason: Reason::OverCapacity,
                    message: format!("The library is larger than the Pocket's index can list ({MAX_TRACKS} tracks, {MAX_ALBUMS} albums, {MAX_ARTISTS} artists)."),
                    fix: None,
                    skipped: true,
                });
            }
            refused = Some(error.message);
        }
    }

    findings
        .sort_by(|a, b| (a.rel.as_str(), a.reason as u8).cmp(&(b.rel.as_str(), b.reason as u8)));
    let nothing_to_do = refused.is_none()
        && renames.is_empty()
        && playlist_fixes.is_empty()
        && before.valid
        && before.root_matches
        && before.missing_files == 0
        && before.tracks == Some(would_index as u16)
        && !findings
            .iter()
            .any(|f| f.skipped && f.reason == Reason::OverCapacity);

    let mut hasher = Sha256::new();
    hasher.update(media_root.to_string_lossy().as_bytes());
    for (rel, size) in &files {
        hasher.update(rel.as_bytes());
        hasher.update(size.to_le_bytes());
    }
    if let Ok(bytes) = fs::read(media_root.join(INDEX)) {
        hasher.update(Sha256::digest(&bytes));
    }
    let id = format!("{:x}", hasher.finalize());
    let plan = RefreshPlan {
        id,
        media_root: media_root.to_path_buf(),
        root_prefix: prefix,
        before,
        media_files,
        would_index,
        findings,
        renames,
        playlist_fixes,
        stubs_to_remove: stubs,
        refused,
        nothing_to_do,
    };
    Ok(Analysis { plan, playlists })
}

/// Plans a refresh of the library in `media_root` (`Assets/<platform>/common`). Read-only.
pub fn plan_refresh(media_root: &Path) -> Result<RefreshPlan, TauError> {
    Ok(analyse(media_root)?.plan)
}

/// The `._` companion path the OS creates beside a media-folder-relative path.
fn companion(media_root: &Path, rel: &str) -> PathBuf {
    let path = media_root.join(rel);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!("._{name}"))
}

fn card_root(media_root: &Path) -> PathBuf {
    media_root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap_or(media_root)
        .to_path_buf()
}

/// Carries out a reviewed plan. Backs up what it replaces to `<backup_root>/refresh-<id>/` (outside the card), then
/// renames, rewrites playlists, removes `._` files, rebuilds and verifies the index. Any failure rolls back.
pub fn execute_refresh(
    media_root: &Path,
    plan: &RefreshPlan,
    confirmation: &str,
    backup_root: &Path,
) -> Result<RefreshReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    if let Some(reason) = &plan.refused {
        return Err(refuse(reason.clone()));
    }
    if backup_root.as_os_str().is_empty()
        || sync::backup_is_inside(backup_root, &card_root(media_root))
    {
        return Err(TauError::e(
            ErrorCode::UnsafeBackupLocation,
            "backup folder must be outside the card",
        ));
    }
    sync::recover_index(media_root)?;
    let analysis = analyse(media_root)?;
    if analysis.plan.id != plan.id {
        return Err(TauError::e(
            ErrorCode::SourceChangedSincePlan,
            "the media folder changed since the plan was reviewed; plan again",
        ));
    }
    let backup_dir = backup_root.join(format!("refresh-{}", plan.id));
    if plan.nothing_to_do {
        return Ok(RefreshReport {
            backup_dir,
            renamed: 0,
            playlists_rewritten: 0,
            stubs_removed: 0,
            before: plan.before.clone(),
            after: plan.before.clone(),
            index_path: media_root.join(INDEX),
            nothing_to_do: true,
        });
    }
    if backup_dir.join(JOURNAL).exists() {
        return Err(refuse(
            "this plan was already run and its backup is still there; plan again",
        ));
    }

    // Backup: the current index and every playlist that will be rewritten.
    let mut saved = Vec::new();
    let mut to_save: Vec<String> = analysis
        .playlists
        .iter()
        .map(|(orig, _, _)| orig.clone())
        .collect();
    if media_root.join(INDEX).is_file() {
        to_save.push(INDEX.to_string());
    }
    for rel in &to_save {
        let bytes = fs::read(media_root.join(rel))?;
        let dest = backup_dir.join("files").join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        sync::write_durable(&dest, &bytes)?;
        let hash = sync::sha256_bytes(&bytes);
        if sync::sha256_bytes(&fs::read(&dest)?) != hash {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                format!("the backup of {rel} did not read back the same, so nothing was changed"),
            ));
        }
        saved.push(json!({"backup": rel, "sha256": hash}));
    }
    write_journal(
        &backup_dir,
        &json!({
            "version": 1, "state": "started", "plan_id": plan.id, "media_root": media_root.to_string_lossy(),
            "renames": plan.renames.iter().map(|r| json!({"from": r.from, "to": r.to})).collect::<Vec<_>>(),
            "files": saved,
        }),
    )?;

    let run = || -> Result<(usize, usize, usize), TauError> {
        for r in &plan.renames {
            let (from, to) = (media_root.join(&r.from), media_root.join(&r.to));
            if to.exists() {
                return Err(TauError::e(
                    ErrorCode::NameCollision,
                    format!("{} already exists", r.to),
                ));
            }
            fs::rename(&from, &to).map_err(|e| {
                TauError::e(ErrorCode::Io, format!("could not rename {}: {e}", r.from))
            })?;
        }
        let mut rewritten = 0;
        for (_, new_rel, text) in &analysis.playlists {
            let target = media_root.join(new_rel);
            let temp = target.with_extension("m3u.tau-tmp");
            sync::write_durable(&temp, text.as_bytes())?;
            fs::rename(&temp, &target)?;
            rewritten += 1;
        }
        let stubs = sync::sweep_appledouble(media_root);
        // Rebuild: scan, drop what stays out, build, verify, swap.
        let scan = scan_dir(media_root, true)?;
        let entries: Vec<_> = scan
            .entries
            .iter()
            .filter(|e| e.rel.is_ascii() && prefix_fits(&plan.root_prefix, &e.rel))
            .cloned()
            .collect();
        let position: BTreeMap<&str, usize> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.rel.as_str(), i))
            .collect();
        let playlists: Vec<_> = scan
            .playlists
            .iter()
            .map(|p| {
                let mut p = p.clone();
                p.rel_ids = p
                    .rel_ids
                    .iter()
                    .filter_map(|&i| position.get(scan.entries.get(i)?.rel.as_str()).copied())
                    .collect();
                p
            })
            .collect();
        let index = build_index(&entries, &playlists, &plan.root_prefix, &mut Vec::new())?;
        parse(&index)?;
        let temp = media_root.join(".tau-library-refresh.tmp");
        sync::write_durable(&temp, &index)?;
        if sync::read_back_bytes(&temp)? != index {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                "the new library did not read back the same",
            ));
        }
        sync::swap_in_index(&temp, &media_root.join(INDEX))?;
        let stubs = stubs + sync::sweep_appledouble(media_root);
        let after = index_state(media_root);
        if !after.valid || !after.root_matches || after.missing_files != 0 {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                "the rebuilt library did not verify against the files on the card",
            ));
        }
        Ok((plan.renames.len(), rewritten, stubs))
    };
    let (renamed, playlists_rewritten, stubs_removed) = match run() {
        Ok(done) => done,
        Err(error) => {
            return Err(match rollback_refresh(media_root, &backup_dir) {
                Ok(()) => TauError::e(
                    error.code(),
                    format!("{} Everything was put back as it was.", error.message),
                ),
                Err(rb) => TauError::e(
                    error.code(),
                    format!(
                        "{} Putting it back also failed ({}); the backup is in {}.",
                        error.message,
                        rb.message,
                        backup_dir.display()
                    ),
                ),
            });
        }
    };
    let after = index_state(media_root);
    let mut journal = read_journal(&backup_dir)?;
    journal["state"] = json!("done");
    write_journal(&backup_dir, &journal)?;
    Ok(RefreshReport {
        backup_dir,
        renamed,
        playlists_rewritten,
        stubs_removed,
        before: plan.before.clone(),
        after,
        index_path: media_root.join(INDEX),
        nothing_to_do: false,
    })
}

fn prefix_fits(prefix: &str, rel: &str) -> bool {
    prefix.len() + rel.len() <= MAX_PATH
}

fn write_journal(dir: &Path, value: &Value) -> Result<(), TauError> {
    fs::create_dir_all(dir)?;
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| TauError::e(ErrorCode::Json, e.to_string()))?;
    sync::write_durable(&dir.join(JOURNAL), &bytes)
}

fn read_journal(dir: &Path) -> Result<Value, TauError> {
    let bytes = fs::read(dir.join(JOURNAL))
        .map_err(|_| refuse("there is no refresh journal in that backup folder"))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| refuse(format!("the refresh journal is damaged: {e}")))
}

/// Undoes a refresh from its backup folder alone: puts back the old index and playlists (hash-checked) and renames
/// everything back. Safe to run twice.
pub fn rollback_refresh(media_root: &Path, backup_dir: &Path) -> Result<(), TauError> {
    let journal = read_journal(backup_dir)?;
    if journal.get("version").and_then(Value::as_u64) != Some(1)
        || journal.get("media_root").and_then(Value::as_str) != Some(&media_root.to_string_lossy())
    {
        return Err(refuse(
            "this backup was made for a different folder or by a version Omega does not know",
        ));
    }
    // Renames back, deepest first (the reverse of the order they were made in).
    let renames: Vec<(String, String)> = journal
        .get("renames")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| {
            Some((
                r.get("from")?.as_str()?.to_string(),
                r.get("to")?.as_str()?.to_string(),
            ))
        })
        .collect();
    for (from, to) in renames.iter().rev() {
        if [from, to]
            .iter()
            .any(|p| p.split('/').any(|c| c == ".." || c.is_empty()) || p.starts_with('/'))
        {
            return Err(refuse("the refresh journal names an unsafe path"));
        }
        let (now, back) = (media_root.join(to), media_root.join(from));
        if now.exists() && !back.exists() {
            // The OS makes a `._` companion for what it renames; one that was not there before is ours to remove.
            let had = companion(media_root, from).exists();
            fs::rename(&now, &back)?;
            if !had {
                let _ = fs::remove_file(companion(media_root, from));
            }
        }
    }
    for entry in journal
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(backup), Some(hash)) = (
            entry.get("backup").and_then(Value::as_str),
            entry.get("sha256").and_then(Value::as_str),
        ) else {
            return Err(refuse("the refresh journal has a malformed entry"));
        };
        if backup.split('/').any(|c| c == ".." || c.is_empty()) || backup.starts_with('/') {
            return Err(refuse("the refresh journal names an unsafe path"));
        }
        let saved = fs::read(backup_dir.join("files").join(backup))
            .map_err(|_| refuse(format!("the backup of {backup} is missing")))?;
        if sync::sha256_bytes(&saved) != hash {
            return Err(refuse(format!(
                "the backup of {backup} no longer matches its recorded hash; it was not restored"
            )));
        }
        let dest = media_root.join(backup);
        let had = companion(media_root, backup).exists();
        sync::write_durable(&dest, &saved)?;
        if sync::sha256_bytes(&fs::read(&dest)?) != hash {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                format!("{backup} did not read back the same after it was restored"),
            ));
        }
        if !had {
            let _ = fs::remove_file(companion(media_root, backup));
        }
    }
    // The index the refresh wrote is gone if there was none before.
    let had_index = journal
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|f| f.get("backup").and_then(Value::as_str) == Some(INDEX));
    if !had_index {
        let _ = fs::remove_file(media_root.join(INDEX));
    }
    let mut journal = journal;
    journal["state"] = json!("rolled_back");
    write_journal(backup_dir, &journal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn card(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "tau-refresh-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                + crate::test_uniq()
        ));
        let common = root.join("card/Assets/tau/common");
        fs::create_dir_all(&common).unwrap();
        (root, common)
    }
    fn put(common: &Path, rel: &str) {
        let p = common.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, format!("audio:{rel}")).unwrap();
    }
    fn backups(root: &Path) -> PathBuf {
        root.join("backups")
    }
    fn run(common: &Path, root: &Path) -> RefreshReport {
        let plan = plan_refresh(common).unwrap();
        execute_refresh(common, &plan, &plan.id, &backups(root)).unwrap()
    }
    fn tree(common: &Path) -> Vec<(String, String)> {
        walk(common)
            .into_iter()
            .map(|(p, _)| {
                let h = sync::sha256_bytes(&fs::read(common.join(&p)).unwrap());
                (p, h)
            })
            .collect()
    }

    #[test]
    fn a_clean_hand_copied_library_gets_an_index_and_then_has_nothing_to_do() {
        let (root, common) = card("clean");
        for f in [
            "Miles/Kind of Blue/01 So What.mp3",
            "Miles/Kind of Blue/02 Blue in Green.mp3",
            "Coltrane/Blue Train/01 Blue Train.flac",
        ] {
            put(&common, f);
        }
        let plan = plan_refresh(&common).unwrap();
        assert!(
            !plan.before.present
                && plan.would_index == 3
                && !plan.nothing_to_do
                && plan.refused.is_none()
        );
        let report = execute_refresh(&common, &plan, &plan.id, &backups(&root)).unwrap();
        assert_eq!(report.after.tracks, Some(3));
        assert!(report.after.valid && report.after.root_matches && report.after.missing_files == 0);
        let again = plan_refresh(&common).unwrap();
        assert!(again.nothing_to_do, "{:?}", again.findings);
        let report = execute_refresh(&common, &again, &again.id, &backups(&root)).unwrap();
        assert!(report.nothing_to_do);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn accented_folders_and_files_are_renamed_to_ascii_and_indexed() {
        let (root, common) = card("ascii");
        put(&common, "Beyoncé/Lemonade/01 Formation.mp3");
        put(&common, "Beyoncé/Lemonade/02 Sorry.mp3");
        put(&common, "Plain/Album/01 Naïve.flac");
        let plan = plan_refresh(&common).unwrap();
        let froms: Vec<_> = plan
            .renames
            .iter()
            .map(|r| (r.from.as_str(), r.to.as_str()))
            .collect();
        assert_eq!(
            froms[0],
            ("Beyoncé", "Beyonce"),
            "a folder is renamed before what is inside it: {froms:?}"
        );
        assert!(froms.contains(&("Plain/Album/01 Naïve.flac", "Plain/Album/01 Naive.flac")));
        let ascii = plan
            .findings
            .iter()
            .filter(|f| f.reason == Reason::NonAsciiName && !f.skipped)
            .count();
        assert_eq!(ascii, 3);
        assert_eq!(plan.would_index, 3);
        let before = tree(&common);
        let report = execute_refresh(&common, &plan, &plan.id, &backups(&root)).unwrap();
        assert_eq!(report.renamed, 2);
        assert!(
            common.join("Beyonce/Lemonade/01 Formation.mp3").is_file()
                && !common.join("Beyoncé").exists()
        );
        let index = parse(fs::read(common.join(INDEX)).unwrap()).unwrap();
        assert_eq!(index.counts.tracks, 3);
        assert!(plan_refresh(&common).unwrap().nothing_to_do);
        // Undo: names and the (absent) index come back exactly.
        rollback_refresh(&common, &report.backup_dir).unwrap();
        assert_eq!(tree(&common), before);
        assert!(!common.join(INDEX).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_name_that_would_collide_is_left_alone_and_kept_out_of_the_library() {
        let (root, common) = card("collide");
        put(&common, "Cafe/Album/01 One.mp3");
        put(&common, "Café/Album/01 One.mp3");
        put(&common, "Other/01 Fine.mp3");
        let plan = plan_refresh(&common).unwrap();
        assert!(plan.renames.is_empty(), "{:?}", plan.renames);
        let skipped: Vec<_> = plan
            .findings
            .iter()
            .filter(|f| f.reason == Reason::NameCollision)
            .map(|f| f.rel.as_str())
            .collect();
        assert_eq!(skipped, ["Café/Album/01 One.mp3"]);
        assert_eq!(plan.would_index, 2);
        let before = tree(&common);
        let report = execute_refresh(&common, &plan, &plan.id, &backups(&root)).unwrap();
        assert_eq!(report.after.tracks, Some(2));
        let untouched: Vec<_> = before.iter().filter(|(p, _)| p != INDEX).collect();
        assert!(
            untouched
                .iter()
                .all(|(p, h)| tree(&common).contains(&(p.clone(), h.clone()))),
            "no music file changed"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_path_over_the_limit_and_unplayable_formats_are_listed_and_skipped() {
        let (root, common) = card("long");
        put(&common, "Fine/01 Ok.mp3");
        let long = format!("{}/01 Song.mp3", "L".repeat(190));
        put(&common, &long);
        put(&common, "Fine/02 Voice memo.m4a");
        let plan = plan_refresh(&common).unwrap();
        let reasons: Vec<_> = plan
            .findings
            .iter()
            .map(|f| (f.reason, f.skipped))
            .collect();
        assert!(
            reasons.contains(&(Reason::PathTooLong, true))
                && reasons.contains(&(Reason::UnsupportedFormat, true)),
            "{reasons:?}"
        );
        assert_eq!(plan.would_index, 1);
        let report = execute_refresh(&common, &plan, &plan.id, &backups(&root)).unwrap();
        assert_eq!(report.after.tracks, Some(1));
        assert!(
            common.join(&long).is_file(),
            "the long file is left where it is"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_index_built_for_another_core_is_rebuilt_for_this_one() {
        let (root, common) = card("root");
        put(&common, "A/B/01 One.mp3");
        let scan = scan_dir(&common, true).unwrap();
        let wrong = build_index(
            &scan.entries,
            &scan.playlists,
            "/Assets/tau_dev_42/common/",
            &mut Vec::new(),
        )
        .unwrap();
        fs::write(common.join(INDEX), wrong).unwrap();
        let before = index_state(&common);
        assert!(before.valid && !before.root_matches && before.missing_files == 1);
        let plan = plan_refresh(&common).unwrap();
        assert!(!plan.nothing_to_do);
        let report = execute_refresh(&common, &plan, &plan.id, &backups(&root)).unwrap();
        assert!(report.after.root_matches && report.after.missing_files == 0);
        // Rollback puts the old (wrong) index back byte for byte.
        let wrong_hash = sync::sha256_bytes(
            &fs::read(
                backups(&root)
                    .join(format!("refresh-{}", plan.id))
                    .join("files")
                    .join(INDEX),
            )
            .unwrap(),
        );
        rollback_refresh(&common, &report.backup_dir).unwrap();
        assert_eq!(
            sync::sha256_bytes(&fs::read(common.join(INDEX)).unwrap()),
            wrong_hash
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn playlists_that_point_at_renamed_files_are_rewritten() {
        let (root, common) = card("playlist");
        put(&common, "Beyoncé/Lemonade/01 Formation.mp3");
        put(&common, "Beyoncé/Lemonade/02 Sorry.mp3");
        fs::write(
            common.join("All.m3u"),
            "#EXTM3U\n/Beyoncé/Lemonade/01 Formation.mp3\nBeyoncé/Lemonade/02 Sorry.mp3\n",
        )
        .unwrap();
        fs::write(
            common.join("Beyoncé/Lemonade/Mix.m3u"),
            "01 Formation.mp3\n",
        )
        .unwrap();
        let plan = plan_refresh(&common).unwrap();
        assert_eq!(
            plan.playlist_fixes.len(),
            1,
            "{:?}: the one inside the renamed folder keeps valid relative lines",
            plan.playlist_fixes
        );
        let report = execute_refresh(&common, &plan, &plan.id, &backups(&root)).unwrap();
        assert_eq!(report.playlists_rewritten, 1);
        assert_eq!(
            fs::read_to_string(common.join("All.m3u")).unwrap(),
            "#EXTM3U\n/Beyonce/Lemonade/01 Formation.mp3\nBeyonce/Lemonade/02 Sorry.mp3\n"
        );
        let scan = scan_dir(&common, true).unwrap();
        assert!(
            scan.playlists.iter().any(|p| p.rel_ids.len() == 2),
            "the playlist still resolves after the renames"
        );
        let all = fs::read_to_string(common.join("All.m3u")).unwrap();
        rollback_refresh(&common, &report.backup_dir).unwrap();
        assert_ne!(fs::read_to_string(common.join("All.m3u")).unwrap(), all);
        assert!(
            fs::read_to_string(common.join("All.m3u"))
                .unwrap()
                .contains("Beyoncé")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_playlist_spelled_differently_from_how_the_card_stores_the_name_is_still_followed() {
        let (root, common) = card("nfd");
        // The card stores "é" decomposed (as macOS does on exFAT); the playlist, written elsewhere, uses the composed form.
        put(&common, "Beyonce\u{301}/Lemonade/01 Formation.mp3");
        fs::write(
            common.join("All.m3u"),
            "/Beyonc\u{e9}/Lemonade/01 Formation.mp3\n",
        )
        .unwrap();
        let plan = plan_refresh(&common).unwrap();
        assert_eq!(plan.renames.len(), 1);
        assert_eq!(plan.playlist_fixes.len(), 1, "{:?}", plan.playlist_fixes);
        execute_refresh(&common, &plan, &plan.id, &backups(&root)).unwrap();
        assert_eq!(
            fs::read_to_string(common.join("All.m3u")).unwrap(),
            "/Beyonce/Lemonade/01 Formation.mp3\n"
        );
        assert!(
            scan_dir(&common, true)
                .unwrap()
                .playlists
                .iter()
                .any(|p| p.rel_ids.len() == 1)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn os_stubs_are_removed_and_never_counted_as_music() {
        let (root, common) = card("stubs");
        put(&common, "A/B/01 One.mp3");
        fs::write(common.join("A/B/._01 One.mp3"), b"stub").unwrap();
        fs::write(common.join("._A"), b"stub").unwrap();
        let plan = plan_refresh(&common).unwrap();
        assert_eq!((plan.stubs_to_remove, plan.would_index), (2, 1));
        let report = execute_refresh(&common, &plan, &plan.id, &backups(&root)).unwrap();
        assert!(report.stubs_removed >= 2);
        assert!(
            walk(&common)
                .iter()
                .all(|(p, _)| !p.contains("/._") && !p.starts_with("._"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_folder_with_no_playable_music_never_replaces_a_working_library() {
        let (root, common) = card("empty");
        put(&common, "A/B/01 One.mp3");
        run(&common, &root);
        fs::remove_file(common.join("A/B/01 One.mp3")).unwrap();
        let plan = plan_refresh(&common).unwrap();
        assert!(
            plan.refused
                .as_ref()
                .is_some_and(|r| r.contains("no playable music")),
            "{:?}",
            plan.refused
        );
        assert_eq!(
            execute_refresh(&common, &plan, &plan.id, &backups(&root))
                .unwrap_err()
                .code(),
            ErrorCode::RefreshRefused
        );
        assert!(common.join(INDEX).is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn guards_run_before_anything_is_touched() {
        let (root, common) = card("guards");
        put(&common, "Beyoncé/A/01 One.mp3");
        let before = tree(&common);
        let plan = plan_refresh(&common).unwrap();
        assert_eq!(
            execute_refresh(&common, &plan, "nope", &backups(&root))
                .unwrap_err()
                .code(),
            ErrorCode::ConfirmationMismatch
        );
        let inside = common.join("backups");
        assert_eq!(
            execute_refresh(&common, &plan, &plan.id, &inside)
                .unwrap_err()
                .code(),
            ErrorCode::UnsafeBackupLocation
        );
        put(&common, "New/B/01 Two.mp3");
        assert_eq!(
            execute_refresh(&common, &plan, &plan.id, &backups(&root))
                .unwrap_err()
                .code(),
            ErrorCode::SourceChangedSincePlan
        );
        assert!(tree(&common).len() == before.len() + 1 && !common.join(INDEX).exists());
        assert!(common.join("Beyoncé").exists(), "nothing was renamed");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_failure_after_the_renames_puts_everything_back() {
        let (root, common) = card("failure");
        put(&common, "Beyoncé/A/01 One.mp3");
        let before = tree(&common);
        let plan = plan_refresh(&common).unwrap();
        // The temp file the new index is written to is now a folder: the build fails after the renames.
        fs::create_dir(common.join(".tau-library-refresh.tmp")).unwrap();
        let before_with_dir = tree(&common);
        let error = execute_refresh(&common, &plan, &plan.id, &backups(&root));
        // (The extra folder changes the plan's file list only if it holds files; it holds none.)
        let error = error.unwrap_err();
        assert!(error.message.contains("put back"), "{}", error.message);
        assert!(common.join("Beyoncé/A/01 One.mp3").is_file() && !common.join("Beyonce").exists());
        assert_eq!(tree(&common), before_with_dir);
        let _ = before;
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_damaged_backup_is_never_restored() {
        let (root, common) = card("damaged");
        put(&common, "A/B/01 One.mp3");
        let report = run(&common, &root);
        put(&common, "A/B/02 Two.mp3");
        let second = plan_refresh(&common).unwrap();
        let report2 = execute_refresh(&common, &second, &second.id, &backups(&root)).unwrap();
        fs::write(report2.backup_dir.join("files").join(INDEX), b"corrupt").unwrap();
        let err = rollback_refresh(&common, &report2.backup_dir).unwrap_err();
        assert!(err.message.contains("no longer matches"), "{}", err.message);
        let _ = report;
        fs::remove_dir_all(root).unwrap();
    }

    /// The real-card run of Refresh library. Gated; read-only unless the mode is `run` or `rollback`.
    /// `TAU_REAL_REFRESH_MEDIA` (a core's `Assets/<platform>/common`), `TAU_REAL_REFRESH_BACKUP` (outside the card),
    /// `TAU_REAL_REFRESH_MODE` = `plan` | `run` | `rollback` (needs `TAU_REAL_REFRESH_JOURNAL`, the `refresh-<id>` folder).
    #[test]
    #[ignore]
    fn real_card_refresh_run() {
        let (Ok(media), Ok(backup), Ok(mode)) = (
            std::env::var("TAU_REAL_REFRESH_MEDIA"),
            std::env::var("TAU_REAL_REFRESH_BACKUP"),
            std::env::var("TAU_REAL_REFRESH_MODE"),
        ) else {
            println!("skipped: set TAU_REAL_REFRESH_MEDIA, _BACKUP and _MODE");
            return;
        };
        let (media, backup) = (PathBuf::from(media), PathBuf::from(backup));
        let show = |p: &RefreshPlan| {
            println!(
                "plan id {}\nmedia files {}, would index {}, refused {:?}, nothing_to_do {}",
                p.id, p.media_files, p.would_index, p.refused, p.nothing_to_do
            );
            println!("before: {:?}", p.before);
            for f in &p.findings {
                println!(
                    "  {:?} skipped={} {} -> {:?}",
                    f.reason, f.skipped, f.rel, f.fix
                );
            }
            println!(
                "renames: {:?}\nplaylist fixes: {:?}\nstubs: {}",
                p.renames, p.playlist_fixes, p.stubs_to_remove
            );
        };
        match mode.as_str() {
            "plan" => {
                let before = tree(&media);
                let plan = plan_refresh(&media).unwrap();
                show(&plan);
                assert_eq!(tree(&media), before, "planning must not change the folder");
                println!("PLAN ONLY: the folder is byte-identical to before");
            }
            "run" => {
                let plan = plan_refresh(&media).unwrap();
                show(&plan);
                let r = execute_refresh(&media, &plan, &plan.id, &backup).unwrap();
                println!(
                    "backup {}\nrenamed {} playlists {} stubs {}\nafter: {:?}",
                    r.backup_dir.display(),
                    r.renamed,
                    r.playlists_rewritten,
                    r.stubs_removed,
                    r.after
                );
                assert!(r.after.valid && r.after.root_matches && r.after.missing_files == 0);
            }
            "rollback" => {
                let journal = PathBuf::from(
                    std::env::var("TAU_REAL_REFRESH_JOURNAL").expect("TAU_REAL_REFRESH_JOURNAL"),
                );
                rollback_refresh(&media, &journal).unwrap();
                println!("ROLLED BACK");
            }
            other => panic!("unknown mode {other}"),
        }
    }
}
