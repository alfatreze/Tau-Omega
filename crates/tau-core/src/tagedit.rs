//! Editing tags and covers on the copies that live on a card (never on the
//! user's source files).
//!
//! Edits follow plan -> confirm -> execute like every other write. Each file
//! is rewritten to a temporary name, read back to prove the new tags took,
//! and only then renamed over the original; the index is rebuilt last. Every
//! edit is also recorded in a small manifest on the card so a later sync,
//! which re-copies the pristine source over the edited file, can re-apply it
//! (`reapply_for`).

use crate::{
    ErrorCode, Progress, ProgressObserver, Stage, TauError, cover, read_tags, sync, tick,
    workbench,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MANIFEST_DIR: &str = "tau-omega";
const MANIFEST_FILE: &str = "tau-omega/edits.json";

/// Tag fields to set. `None` leaves a field alone; `Some("")` clears it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FieldEdits {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<String>,
}

impl FieldEdits {
    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.artist.is_none()
            && self.album.is_none()
            && self.album_artist.is_none()
            && self.year.is_none()
    }
    fn merged(&self, newer: &FieldEdits) -> FieldEdits {
        FieldEdits {
            title: newer.title.clone().or_else(|| self.title.clone()),
            artist: newer.artist.clone().or_else(|| self.artist.clone()),
            album: newer.album.clone().or_else(|| self.album.clone()),
            album_artist: newer.album_artist.clone().or_else(|| self.album_artist.clone()),
            year: newer.year.clone().or_else(|| self.year.clone()),
        }
    }
    /// `(id3 frame, vorbis key, value)` for every field being set.
    fn pairs(&self) -> Vec<(&'static str, &'static str, &str)> {
        let mut out = Vec::new();
        for (frame, key, value) in [
            ("TIT2", "TITLE", &self.title),
            ("TPE1", "ARTIST", &self.artist),
            ("TALB", "ALBUM", &self.album),
            ("TPE2", "ALBUMARTIST", &self.album_artist),
            ("TDRC", "DATE", &self.year),
        ] {
            if let Some(v) = value {
                out.push((frame, key, v.as_str()));
            }
        }
        out
    }
}

/// One requested edit: the album folder (and optionally a single track in it).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EditRequest {
    pub album_id: String,
    /// When set, only this file (media-relative) is edited, e.g. a title.
    pub track: Option<String>,
    pub fields: FieldEdits,
    /// A host image to use as the cover (baseline JPEG, validated).
    pub cover: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EditItem {
    pub relative: PathBuf,
    pub destination: PathBuf,
    pub sha256_before: String,
    pub fields: FieldEdits,
    pub cover: Option<sync::CoverItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EditPlan {
    pub id: String,
    pub destination: PathBuf,
    pub requests: Vec<EditRequest>,
    pub items: Vec<EditItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EditReport {
    pub plan_id: String,
    pub files_changed: usize,
    pub index_path: PathBuf,
}

/// Plans edits on the card media root `common`. Read-only.
pub fn plan_edit(common: &Path, requests: &[EditRequest]) -> Result<EditPlan, TauError> {
    sync::validate_media_root(common)?;
    if requests.is_empty() {
        return Err(TauError::e(ErrorCode::NoSources, "nothing to edit"));
    }
    let destination = common.canonicalize()?;
    let mut items = Vec::new();
    for request in requests {
        if request.fields.is_empty() && request.cover.is_none() {
            return Err(TauError::e(ErrorCode::NoSources, "an edit changes nothing"));
        }
        let dir = workbench::checked_dir(&destination, &request.album_id)?;
        let mut files: Vec<PathBuf> = workbench::direct_files(&dir)?
            .into_iter()
            .filter(|f| sync::audio_file(f))
            .collect();
        if let Some(track) = &request.track {
            files.retain(|f| {
                f.strip_prefix(&destination)
                    .is_ok_and(|r| r.to_string_lossy().replace('\\', "/") == *track)
            });
            if files.is_empty() {
                return Err(TauError::e(
                    ErrorCode::NotFound,
                    format!("track not found in this album: {track}"),
                ));
            }
        }
        let cover_item = match &request.cover {
            Some(image) => {
                cover::validate_jpeg_cover(image)?;
                Some(sync::CoverItem {
                    sha256: sync::sha256_file(image)?,
                    source: image.clone(),
                })
            }
            None => None,
        };
        for file in files {
            items.push(EditItem {
                relative: file.strip_prefix(&destination).unwrap().to_path_buf(),
                sha256_before: sync::sha256_file(&file)?,
                destination: file,
                fields: request.fields.clone(),
                cover: cover_item.clone(),
            });
        }
    }
    if items.is_empty() {
        return Err(TauError::e(ErrorCode::NotFound, "no audio files to edit"));
    }
    let mut hasher = Sha256::new();
    hasher.update(b"edit-v1");
    for item in &items {
        hasher.update(item.relative.to_string_lossy().as_bytes());
        hasher.update(item.sha256_before.as_bytes());
        hasher.update(format!("{:?}", item.fields).as_bytes());
        if let Some(c) = &item.cover {
            hasher.update(c.sha256.as_bytes());
        }
    }
    Ok(EditPlan {
        id: format!("{:x}", hasher.finalize()),
        destination,
        requests: requests.to_vec(),
        items,
    })
}

/// Applies a reviewed edit plan and records it in the card's edit manifest.
pub fn execute_edit(
    plan: &EditPlan,
    confirmation: &str,
    root_prefix: &str,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<EditReport, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "confirmation token does not match the current plan",
        ));
    }
    sync::validate_media_root(&plan.destination)?;
    let total = plan.items.len() as u64;
    for (done, item) in plan.items.iter().enumerate() {
        tick(
            progress,
            Progress {
                stage: Stage::Editing,
                done: done as u64,
                total,
                path: Some(item.relative.to_string_lossy().into_owned()),
            },
        )?;
        if sync::sha256_file(&item.destination).ok().as_deref() != Some(item.sha256_before.as_str())
        {
            return Err(TauError::e(
                ErrorCode::SourceChangedSincePlan,
                format!("changed since the plan was reviewed: {}", item.relative.display()),
            ));
        }
        let cover_path = match &item.cover {
            Some(c) => {
                if sync::sha256_file(&c.source).ok().as_deref() != Some(c.sha256.as_str()) {
                    return Err(TauError::e(
                        ErrorCode::SourceChangedSincePlan,
                        "cover image changed since the plan was reviewed",
                    ));
                }
                Some(c.source.as_path())
            }
            None => None,
        };
        apply_to_file(&item.destination, &item.fields, cover_path)?;
    }
    record_manifest(&plan.destination, &plan.requests)?;
    let mut warnings = Vec::new();
    sync::rebuild_index(&plan.destination, root_prefix, &plan.id, &mut warnings, progress)?;
    Ok(EditReport {
        plan_id: plan.id.clone(),
        files_changed: plan.items.len(),
        index_path: plan.destination.join("tau-library.tdb"),
    })
}

/// Re-applies every recorded edit that covers one of `rels` (media-relative
/// files that a sync has just re-copied from the unedited source). Returns how
/// many files were rewritten. Part of an already-confirmed sync, so it takes
/// no token of its own.
pub fn reapply_for(common: &Path, rels: &[String]) -> Result<usize, TauError> {
    let manifest = read_manifest(common)?;
    let mut count = 0;
    for rel in rels {
        let path = common.join(rel);
        if !path.is_file() || !sync::audio_file(&path) {
            continue;
        }
        let dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
        let mut fields = FieldEdits::default();
        let mut cover_file: Option<PathBuf> = None;
        for album in manifest["albums"].as_array().into_iter().flatten() {
            if album["dir"].as_str() == Some(dir) {
                fields = fields.merged(&fields_from(album));
                if let Some(c) = album["cover"].as_str() {
                    cover_file = Some(common.join(c));
                }
            }
        }
        for track in manifest["tracks"].as_array().into_iter().flatten() {
            if track["rel"].as_str() == Some(rel.as_str()) {
                fields = fields.merged(&fields_from(track));
            }
        }
        if fields.is_empty() && cover_file.is_none() {
            continue;
        }
        let cover_file = cover_file.filter(|c| c.is_file());
        apply_to_file(&path, &fields, cover_file.as_deref())?;
        count += 1;
    }
    Ok(count)
}

fn fields_from(v: &Value) -> FieldEdits {
    let get = |k: &str| v[k].as_str().map(str::to_string);
    FieldEdits {
        title: get("title"),
        artist: get("artist"),
        album: get("album"),
        album_artist: get("album_artist"),
        year: get("year"),
    }
}
fn fields_json(f: &FieldEdits) -> Vec<(&'static str, Value)> {
    [
        ("title", &f.title),
        ("artist", &f.artist),
        ("album", &f.album),
        ("album_artist", &f.album_artist),
        ("year", &f.year),
    ]
    .into_iter()
    .filter_map(|(k, v)| v.as_ref().map(|v| (k, json!(v))))
    .collect()
}

fn read_manifest(common: &Path) -> Result<Value, TauError> {
    let path = common.join(MANIFEST_FILE);
    if !path.is_file() {
        return Ok(json!({"version": 1, "albums": [], "tracks": []}));
    }
    serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| TauError::e(ErrorCode::Json, format!("edit manifest unreadable: {e}")))
}

fn record_manifest(common: &Path, requests: &[EditRequest]) -> Result<(), TauError> {
    let mut manifest = read_manifest(common)?;
    for request in requests {
        let (list, key_name, key) = match &request.track {
            Some(track) => ("tracks", "rel", track.clone()),
            None => ("albums", "dir", request.album_id.clone()),
        };
        let entries = manifest[list].as_array_mut().ok_or_else(|| {
            TauError::e(ErrorCode::Json, "edit manifest is malformed")
        })?;
        let index = entries
            .iter()
            .position(|e| e[key_name].as_str() == Some(key.as_str()));
        let entry = match index {
            Some(i) => &mut entries[i],
            None => {
                entries.push(json!({ key_name: key }));
                entries.last_mut().unwrap()
            }
        };
        for (k, v) in fields_json(&request.fields) {
            entry[k] = v;
        }
        if let (Some(image), None) = (&request.cover, &request.track) {
            let digest = sync::sha256_file(image)?;
            let relative = format!("{MANIFEST_DIR}/covers/{digest}.jpg");
            let target = common.join(&relative);
            fs::create_dir_all(target.parent().unwrap())?;
            fs::copy(image, &target)?;
            entry["cover"] = json!(relative);
        }
    }
    let path = common.join(MANIFEST_FILE);
    fs::create_dir_all(path.parent().unwrap())?;
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| TauError::e(ErrorCode::Json, e.to_string()))?;
    sync::write_durable(&path, &bytes)?;
    Ok(())
}

/// A same-folder scratch name that keeps the real extension (so tag readers
/// pick the right parser) and starts with `._` (so a scan ignores it even if a
/// crash leaves it behind).
fn temp_path(path: &Path, tag: &str) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!("._tau-{tag}-{name}"))
}

/// Rewrites `path` with the new tags (and cover) via a temporary file, checks
/// the result reads back correctly, then renames it into place.
fn apply_to_file(path: &Path, fields: &FieldEdits, cover: Option<&Path>) -> Result<(), TauError> {
    let is_flac = path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("flac"));
    let original = fs::read(path)?;
    let rewritten = if fields.is_empty() {
        original.clone()
    } else if is_flac {
        flac_rewrite(&original, fields)?
    } else {
        id3_rewrite(&original, fields)?
    };
    let temp = temp_path(path, "edit");
    sync::write_durable(&temp, &rewritten)?;
    let result = (|| {
        let mut current = temp.clone();
        let cover_temp = temp_path(path, "cover");
        if let Some(image) = cover {
            if is_flac {
                cover::embed_flac_copy(&current, image, &cover_temp)?;
            } else {
                cover::embed_mp3_copy(&current, image, &cover_temp)?;
            }
            current = cover_temp.clone();
        }
        verify_written(&current, fields, cover.is_some())?;
        fs::rename(&current, path)?;
        let _ = fs::remove_file(&cover_temp);
        Ok(())
    })();
    let _ = fs::remove_file(&temp);
    result
}

fn verify_written(path: &Path, fields: &FieldEdits, expect_cover: bool) -> Result<(), TauError> {
    sync::evict_cache(path);
    let (tags, _, _) = read_tags(path)?;
    for (frame, _, value) in fields.pairs() {
        let got = if frame == "TDRC" {
            tags.get("TDRC")
                .or_else(|| tags.get("TYER"))
                .map(|y| y.chars().take(4).collect::<String>())
        } else {
            tags.get(frame).cloned()
        };
        let want = if frame == "TDRC" {
            value.chars().take(4).collect::<String>()
        } else {
            value.trim().to_string()
        };
        let ok = if want.is_empty() {
            got.as_deref().is_none_or(str::is_empty)
        } else {
            got.as_deref() == Some(want.as_str())
        };
        if !ok {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                format!("edited {frame} did not read back correctly"),
            ));
        }
    }
    if expect_cover && !cover::has_embedded_cover(path)? {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            "cover did not read back after editing",
        ));
    }
    Ok(())
}

// ---- ID3v2 -------------------------------------------------------------

fn syncsafe(b: &[u8]) -> usize {
    ((b[0] as usize) << 21) | ((b[1] as usize) << 14) | ((b[2] as usize) << 7) | b[3] as usize
}
fn to_syncsafe(n: usize) -> [u8; 4] {
    [
        ((n >> 21) & 0x7f) as u8,
        ((n >> 14) & 0x7f) as u8,
        ((n >> 7) & 0x7f) as u8,
        (n & 0x7f) as u8,
    ]
}

fn text_frame(major: u8, id: &str, value: &str) -> Vec<u8> {
    let mut data = Vec::new();
    if major == 4 {
        data.push(3);
        data.extend_from_slice(value.as_bytes());
    } else if value.chars().all(|c| (c as u32) < 256) {
        data.push(0);
        data.extend(value.chars().map(|c| c as u8));
    } else {
        data.extend_from_slice(&[1, 0xff, 0xfe]);
        for unit in value.encode_utf16() {
            data.extend_from_slice(&unit.to_le_bytes());
        }
    }
    let mut frame = id.as_bytes().to_vec();
    if major == 4 {
        frame.extend_from_slice(&to_syncsafe(data.len()));
    } else {
        frame.extend_from_slice(&(data.len() as u32).to_be_bytes());
    }
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(&data);
    frame
}

/// Returns `data` with the text frames for `fields` replaced. Every other
/// frame and the audio bytes are kept verbatim. A v2.3 tag is upgraded to
/// v2.4 (UTF-8 text) only when a new value needs characters ISO-8859-1 cannot
/// hold, and only if no frame carries flags that differ between versions.
fn id3_rewrite(data: &[u8], fields: &FieldEdits) -> Result<Vec<u8>, TauError> {
    let unsupported = |why: &str| TauError::e(ErrorCode::UnsupportedTags, why.to_string());
    type Frame = ([u8; 4], [u8; 2], Vec<u8>);
    let (mut major, kept, audio_start): (u8, Vec<Frame>, usize) = if data.starts_with(b"ID3") {
        if data.len() < 10 || !matches!(data[3], 3 | 4) {
            return Err(unsupported("unsupported ID3 version for tag editing"));
        }
        if data[5] & 0xd0 != 0 {
            return Err(unsupported(
                "ID3 unsynchronisation, extended header or footer is not safe to rewrite",
            ));
        }
        let major = data[3];
        let end = 10 + syncsafe(&data[6..10]);
        if end > data.len() {
            return Err(unsupported("malformed ID3 tag"));
        }
        let replaced: Vec<&str> = fields
            .pairs()
            .iter()
            .flat_map(|(frame, _, _)| {
                if *frame == "TDRC" { vec!["TDRC", "TYER"] } else { vec![*frame] }
            })
            .collect();
        let mut kept = Vec::new();
        let mut position = 10;
        while position + 10 <= end {
            let header = &data[position..position + 10];
            if header[0] == 0 {
                break;
            }
            let length = if major == 4 {
                syncsafe(&header[4..8])
            } else {
                u32::from_be_bytes(header[4..8].try_into().unwrap()) as usize
            };
            if length == 0 || position + 10 + length > end {
                return Err(unsupported("malformed ID3 frame"));
            }
            let id: [u8; 4] = header[..4].try_into().unwrap();
            if !replaced.contains(&String::from_utf8_lossy(&id).as_ref()) {
                kept.push((
                    id,
                    [header[8], header[9]],
                    data[position + 10..position + 10 + length].to_vec(),
                ));
            }
            position += 10 + length;
        }
        (major, kept, end)
    } else {
        (4, Vec::new(), 0)
    };
    let needs_unicode = fields
        .pairs()
        .iter()
        .any(|(_, _, v)| v.chars().any(|c| (c as u32) > 255));
    if major == 3 && needs_unicode {
        if kept.iter().any(|(_, flags, _)| *flags != [0, 0]) {
            return Err(unsupported(
                "this ID3v2.3 tag has flagged frames and cannot be upgraded for non-Latin text",
            ));
        }
        major = 4;
    }
    let mut frames = Vec::new();
    for (id, flags, body) in &kept {
        frames.extend_from_slice(id);
        if major == 4 {
            frames.extend_from_slice(&to_syncsafe(body.len()));
        } else {
            frames.extend_from_slice(&(body.len() as u32).to_be_bytes());
        }
        frames.extend_from_slice(flags);
        frames.extend_from_slice(body);
    }
    for (frame, _, value) in fields.pairs() {
        let value = value.trim();
        if value.is_empty() {
            continue; // clearing a field just drops the frame
        }
        let id = if frame == "TDRC" && major == 3 { "TYER" } else { frame };
        let value = if frame == "TDRC" { &value[..value.len().min(4)] } else { value };
        frames.extend_from_slice(&text_frame(major, id, value));
    }
    let mut out = Vec::with_capacity(frames.len() + data.len());
    out.extend_from_slice(b"ID3");
    out.extend_from_slice(&[major, 0, 0]);
    out.extend_from_slice(&to_syncsafe(frames.len()));
    out.extend_from_slice(&frames);
    out.extend_from_slice(&data[audio_start..]);
    Ok(out)
}

// ---- FLAC --------------------------------------------------------------

/// Returns `data` with the Vorbis comments for `fields` replaced. Other
/// metadata blocks (except padding) and the audio frames are kept verbatim.
fn flac_rewrite(data: &[u8], fields: &FieldEdits) -> Result<Vec<u8>, TauError> {
    let bad = || TauError::e(ErrorCode::UnsupportedTags, "malformed FLAC metadata");
    if !data.starts_with(b"fLaC") {
        return Err(TauError::e(ErrorCode::UnsupportedTags, "not a FLAC file"));
    }
    let mut position = 4;
    let mut blocks: Vec<(u8, Vec<u8>)> = Vec::new();
    loop {
        let header = *data.get(position).ok_or_else(bad)?;
        let length = ((*data.get(position + 1).ok_or_else(bad)? as usize) << 16)
            | ((*data.get(position + 2).ok_or_else(bad)? as usize) << 8)
            | *data.get(position + 3).ok_or_else(bad)? as usize;
        position += 4;
        let body = data.get(position..position + length).ok_or_else(bad)?;
        position += length;
        blocks.push((header & 0x7f, body.to_vec()));
        if header & 0x80 != 0 {
            break;
        }
    }
    let replaced: Vec<&str> = fields.pairs().iter().map(|(_, key, _)| *key).collect();
    let mut vendor = b"tau-omega".to_vec();
    let mut comments: Vec<Vec<u8>> = Vec::new();
    if let Some((_, body)) = blocks.iter().find(|(kind, _)| *kind == 4) {
        let vlen = u32::from_le_bytes(body.get(0..4).ok_or_else(bad)?.try_into().unwrap()) as usize;
        vendor = body.get(4..4 + vlen).ok_or_else(bad)?.to_vec();
        let mut p = 4 + vlen;
        let n = u32::from_le_bytes(body.get(p..p + 4).ok_or_else(bad)?.try_into().unwrap());
        p += 4;
        for _ in 0..n {
            let len = u32::from_le_bytes(body.get(p..p + 4).ok_or_else(bad)?.try_into().unwrap()) as usize;
            p += 4;
            let comment = body.get(p..p + len).ok_or_else(bad)?;
            p += len;
            let key = String::from_utf8_lossy(comment)
                .split_once('=')
                .map(|(k, _)| k.to_ascii_uppercase())
                .unwrap_or_default();
            if !replaced.contains(&key.as_str()) {
                comments.push(comment.to_vec());
            }
        }
    }
    for (_, key, value) in fields.pairs() {
        let value = value.trim();
        if !value.is_empty() {
            let value = if key == "DATE" { &value[..value.len().min(4)] } else { value };
            comments.push(format!("{key}={value}").into_bytes());
        }
    }
    let mut vorbis = Vec::new();
    vorbis.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    vorbis.extend_from_slice(&vendor);
    vorbis.extend_from_slice(&(comments.len() as u32).to_le_bytes());
    for comment in &comments {
        vorbis.extend_from_slice(&(comment.len() as u32).to_le_bytes());
        vorbis.extend_from_slice(comment);
    }
    let mut out_blocks: Vec<(u8, Vec<u8>)> = Vec::new();
    let mut wrote_vorbis = false;
    for (kind, body) in blocks {
        match kind {
            1 => {} // drop padding; sizes changed anyway
            4 => {
                if !wrote_vorbis {
                    out_blocks.push((4, vorbis.clone()));
                    wrote_vorbis = true;
                }
            }
            _ => out_blocks.push((kind, body)),
        }
    }
    if !wrote_vorbis {
        let at = out_blocks.len().min(1); // right after STREAMINFO
        out_blocks.insert(at, (4, vorbis));
    }
    let mut out = b"fLaC".to_vec();
    let last = out_blocks.len() - 1;
    for (i, (kind, body)) in out_blocks.iter().enumerate() {
        if body.len() >= 1 << 24 {
            return Err(TauError::e(ErrorCode::UnsupportedTags, "FLAC metadata block too large"));
        }
        out.push(kind | if i == last { 0x80 } else { 0 });
        out.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
        out.extend_from_slice(body);
    }
    out.extend_from_slice(&data[position..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "tau-te-{name}-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() + crate::test_uniq()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// A minimal MP3-ish file: an ID3v2 tag with the given frames, then audio.
    fn mp3(major: u8, frames: &[(&str, &str)], audio: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        for (id, value) in frames {
            body.extend_from_slice(&text_frame(major, id, value));
        }
        let mut out = b"ID3".to_vec();
        out.extend_from_slice(&[major, 0, 0]);
        out.extend_from_slice(&to_syncsafe(body.len()));
        out.extend_from_slice(&body);
        out.extend_from_slice(audio);
        out
    }
    /// A minimal FLAC: STREAMINFO (44.1 kHz, 1 s), a vorbis block, padding.
    fn flac(comments: &[&str], audio: &[u8]) -> Vec<u8> {
        let mut streaminfo = vec![0u8; 34];
        streaminfo[10] = 0x0a;
        streaminfo[11] = 0xc4;
        streaminfo[12] = 0x42;
        streaminfo[17] = 0x44; // 44100 samples
        let mut vorbis = Vec::new();
        vorbis.extend_from_slice(&5u32.to_le_bytes());
        vorbis.extend_from_slice(b"vendo");
        vorbis.extend_from_slice(&(comments.len() as u32).to_le_bytes());
        for c in comments {
            vorbis.extend_from_slice(&(c.len() as u32).to_le_bytes());
            vorbis.extend_from_slice(c.as_bytes());
        }
        let mut out = b"fLaC".to_vec();
        for (i, (kind, body)) in [(0u8, streaminfo), (4, vorbis), (1, vec![0; 16])].iter().enumerate() {
            out.push(kind | if i == 2 { 0x80 } else { 0 });
            out.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
            out.extend_from_slice(body);
        }
        out.extend_from_slice(audio);
        out
    }
    fn tags(path: &Path) -> std::collections::BTreeMap<String, String> {
        read_tags(path).unwrap().0
    }
    fn edits(album: &str) -> FieldEdits {
        FieldEdits { album: Some(album.into()), ..Default::default() }
    }

    #[test]
    fn mp3_edit_changes_tags_and_keeps_other_frames_and_audio() {
        let dir = tmp("mp3");
        let file = dir.join("a.mp3");
        let audio = vec![0xff, 0xfb, 0x90, 0x00, 1, 2, 3, 4, 5];
        fs::write(&file, mp3(3, &[("TIT2", "Song"), ("TALB", "Old"), ("TRCK", "3")], &audio)).unwrap();
        apply_to_file(&file, &FieldEdits { album: Some("New Album".into()), artist: Some("Bjork".into()), year: Some("1997".into()), ..Default::default() }, None).unwrap();
        let t = tags(&file);
        assert_eq!(t["TALB"], "New Album");
        assert_eq!(t["TPE1"], "Bjork");
        assert_eq!(t["TIT2"], "Song");
        assert_eq!(t["TRCK"], "3");
        assert_eq!(t["TYER"], "1997");
        assert!(fs::read(&file).unwrap().ends_with(&audio));
    }

    #[test]
    fn mp3_edit_handles_non_latin_text_and_v24_and_untagged_files() {
        let dir = tmp("mp3b");
        let v24 = dir.join("v24.mp3");
        fs::write(&v24, mp3(4, &[("TIT2", "Song")], &[0xff, 0xfb, 0x90, 0])).unwrap();
        apply_to_file(&v24, &edits("Sigur Rós – Ágætis"), None).unwrap();
        assert_eq!(tags(&v24)["TALB"], "Sigur Rós – Ágætis");
        let v23 = dir.join("v23.mp3");
        fs::write(&v23, mp3(3, &[("TIT2", "Song")], &[0xff, 0xfb, 0x90, 0])).unwrap();
        apply_to_file(&v23, &edits("東京 Album"), None).unwrap();
        assert_eq!(tags(&v23)["TALB"], "東京 Album");
        let bare = dir.join("bare.mp3");
        fs::write(&bare, [0xff, 0xfb, 0x90, 0, 9, 9]).unwrap();
        apply_to_file(&bare, &edits("Fresh"), None).unwrap();
        assert_eq!(tags(&bare)["TALB"], "Fresh");
    }

    #[test]
    fn clearing_a_field_removes_it() {
        let dir = tmp("clear");
        let file = dir.join("a.mp3");
        fs::write(&file, mp3(3, &[("TALB", "Old"), ("TIT2", "S")], &[0xff, 0xfb, 0x90, 0])).unwrap();
        apply_to_file(&file, &edits(""), None).unwrap();
        assert!(!tags(&file).contains_key("TALB"));
        assert_eq!(tags(&file)["TIT2"], "S");
    }

    #[test]
    fn unsafe_id3_is_refused_and_the_file_is_left_alone() {
        let dir = tmp("unsafe");
        let file = dir.join("a.mp3");
        let mut data = mp3(3, &[("TIT2", "S")], &[0xff, 0xfb, 0x90, 0]);
        data[5] = 0x80; // unsynchronisation flag
        fs::write(&file, &data).unwrap();
        let err = apply_to_file(&file, &edits("X"), None).unwrap_err();
        assert_eq!(err.code(), ErrorCode::UnsupportedTags);
        assert_eq!(fs::read(&file).unwrap(), data);
        assert!(!temp_path(&file, "edit").exists());
    }

    #[test]
    fn flac_edit_replaces_comments_and_keeps_audio() {
        let dir = tmp("flac");
        let file = dir.join("a.flac");
        let audio = vec![0xff, 0xf8, 1, 2, 3];
        fs::write(&file, flac(&["TITLE=Song", "ALBUM=Old", "TRACKNUMBER=2"], &audio)).unwrap();
        apply_to_file(&file, &FieldEdits { album: Some("New".into()), album_artist: Some("VA".into()), year: Some("2001-05".into()), ..Default::default() }, None).unwrap();
        let t = tags(&file);
        assert_eq!(t["TALB"], "New");
        assert_eq!(t["TPE2"], "VA");
        assert_eq!(t["TIT2"], "Song");
        assert_eq!(t["TRCK"], "2");
        assert_eq!(t["TYER"], "2001");
        assert!(fs::read(&file).unwrap().ends_with(&audio));
    }

    #[test]
    fn edit_plan_needs_the_token_and_refuses_changed_files() {
        let root = tmp("plan");
        let common = root.join("Assets/tau/common");
        fs::create_dir_all(common.join("Artist/Album")).unwrap();
        let file = common.join("Artist/Album/01.mp3");
        fs::write(&file, mp3(3, &[("TALB", "Old")], &[0xff, 0xfb, 0x90, 0])).unwrap();
        let req = EditRequest { album_id: "Artist/Album".into(), fields: edits("New"), ..Default::default() };
        let plan = plan_edit(&common, &[req]).unwrap();
        assert_eq!(
            execute_edit(&plan, "nope", "/Assets/tau/common/", &mut None).unwrap_err().code(),
            ErrorCode::ConfirmationMismatch
        );
        fs::write(&file, mp3(3, &[("TALB", "Someone else")], &[0xff, 0xfb, 0x90, 0])).unwrap();
        assert_eq!(
            execute_edit(&plan, &plan.id, "/Assets/tau/common/", &mut None).unwrap_err().code(),
            ErrorCode::SourceChangedSincePlan
        );
    }

    #[test]
    fn edits_are_recorded_and_reapplied_after_a_resync_overwrites_them() {
        let root = tmp("reapply");
        let common = root.join("Assets/tau/common");
        let album = common.join("Artist/Album");
        fs::create_dir_all(&album).unwrap();
        let pristine = mp3(3, &[("TALB", "Original"), ("TIT2", "One")], &[0xff, 0xfb, 0x90, 0]);
        fs::write(album.join("01.mp3"), &pristine).unwrap();
        let req = EditRequest { album_id: "Artist/Album".into(), fields: edits("Renamed"), ..Default::default() };
        let plan = plan_edit(&common, &[req]).unwrap();
        execute_edit(&plan, &plan.id, "/Assets/tau/common/", &mut None).unwrap();
        assert_eq!(tags(&album.join("01.mp3"))["TALB"], "Renamed");
        // a later sync copies the pristine source over the edited file
        fs::write(album.join("01.mp3"), &pristine).unwrap();
        assert_eq!(tags(&album.join("01.mp3"))["TALB"], "Original");
        let n = reapply_for(&common, &["Artist/Album/01.mp3".to_string()]).unwrap();
        assert_eq!(n, 1);
        assert_eq!(tags(&album.join("01.mp3"))["TALB"], "Renamed");
        assert_eq!(tags(&album.join("01.mp3"))["TIT2"], "One");
        // unrelated albums are untouched
        fs::create_dir_all(common.join("Other")).unwrap();
        fs::write(common.join("Other/01.mp3"), &pristine).unwrap();
        assert_eq!(reapply_for(&common, &["Other/01.mp3".to_string()]).unwrap(), 0);
    }

    #[test]
    fn edit_rebuilds_a_verifying_index() {
        let root = tmp("index");
        let common = root.join("Assets/tau/common");
        fs::create_dir_all(common.join("A/B")).unwrap();
        fs::write(common.join("A/B/01.mp3"), mp3(3, &[("TIT2", "T"), ("TALB", "Old"), ("TPE1", "A")], &[0xff, 0xfb, 0x90, 0])).unwrap();
        let req = EditRequest { album_id: "A/B".into(), fields: edits("New"), ..Default::default() };
        let plan = plan_edit(&common, &[req]).unwrap();
        let report = execute_edit(&plan, &plan.id, "/Assets/tau/common/", &mut None).unwrap();
        let index = fs::read(report.index_path).unwrap();
        assert!(crate::verify(&index, Some(&common)).unwrap().is_empty());
    }

    const JPEG: [u8; 23] = [
        0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, 0, 1, 0, 1, 3, 1, 17, 0, 2, 17, 0, 3, 17, 0, 0xff, 0xd9,
    ];

    #[test]
    fn cover_change_embeds_in_mp3_and_flac_and_survives_a_resync() {
        let root = tmp("cover");
        let common = root.join("Assets/tau/common");
        let album = common.join("A/B");
        fs::create_dir_all(&album).unwrap();
        let mp3_pristine = mp3(3, &[("TIT2", "One")], &[0xff, 0xfb, 0x90, 0]);
        let flac_pristine = flac(&["TITLE=Two"], &[0xff, 0xf8, 1, 2]);
        fs::write(album.join("01.mp3"), &mp3_pristine).unwrap();
        fs::write(album.join("02.flac"), &flac_pristine).unwrap();
        let image = root.join("new-cover.jpg");
        fs::write(&image, JPEG).unwrap();
        let req = EditRequest { album_id: "A/B".into(), cover: Some(image), ..Default::default() };
        let plan = plan_edit(&common, &[req]).unwrap();
        assert_eq!(plan.items.len(), 2);
        execute_edit(&plan, &plan.id, "/Assets/tau/common/", &mut None).unwrap();
        assert!(cover::has_embedded_cover(&album.join("01.mp3")).unwrap());
        assert!(cover::has_embedded_cover(&album.join("02.flac")).unwrap());
        assert_eq!(tags(&album.join("01.mp3"))["TIT2"], "One");
        // no scratch files left behind
        assert!(fs::read_dir(&album).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with("._")));
        // a re-sync overwrites the copies with the source; the recorded cover comes back
        fs::write(album.join("01.mp3"), &mp3_pristine).unwrap();
        fs::write(album.join("02.flac"), &flac_pristine).unwrap();
        assert!(!cover::has_embedded_cover(&album.join("01.mp3")).unwrap());
        let n = reapply_for(&common, &["A/B/01.mp3".to_string(), "A/B/02.flac".to_string()]).unwrap();
        assert_eq!(n, 2);
        assert!(cover::has_embedded_cover(&album.join("01.mp3")).unwrap());
        assert!(cover::has_embedded_cover(&album.join("02.flac")).unwrap());
    }

    #[test]
    fn a_progressive_cover_is_refused_at_plan_time() {
        let root = tmp("badcover");
        let common = root.join("Assets/tau/common");
        fs::create_dir_all(common.join("A/B")).unwrap();
        fs::write(common.join("A/B/01.mp3"), mp3(3, &[("TIT2", "One")], &[0xff, 0xfb, 0x90, 0])).unwrap();
        let image = root.join("prog.jpg");
        fs::write(&image, [0xff, 0xd8, 0xff, 0xc2, 0, 2, 0xff, 0xd9]).unwrap();
        let req = EditRequest { album_id: "A/B".into(), cover: Some(image), ..Default::default() };
        assert!(plan_edit(&common, &[req]).is_err());
    }

    #[test]
    fn a_single_track_title_edit_leaves_the_rest_of_the_album_alone() {
        let root = tmp("title");
        let common = root.join("Assets/tau/common");
        let album = common.join("A/B");
        fs::create_dir_all(&album).unwrap();
        for n in ["01.mp3", "02.mp3"] {
            fs::write(album.join(n), mp3(3, &[("TIT2", "Old")], &[0xff, 0xfb, 0x90, 0])).unwrap();
        }
        let req = EditRequest {
            album_id: "A/B".into(),
            track: Some("A/B/02.mp3".into()),
            fields: FieldEdits { title: Some("Better".into()), ..Default::default() },
            ..Default::default()
        };
        let plan = plan_edit(&common, &[req]).unwrap();
        assert_eq!(plan.items.len(), 1);
        execute_edit(&plan, &plan.id, "/Assets/tau/common/", &mut None).unwrap();
        assert_eq!(tags(&album.join("02.mp3"))["TIT2"], "Better");
        assert_eq!(tags(&album.join("01.mp3"))["TIT2"], "Old");
    }
}
