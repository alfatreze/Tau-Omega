//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

pub(super) fn mirror_deletions(
    destination: &Path,
    copies: &[CopyItem],
) -> Result<Vec<DeleteItem>, TauError> {
    let keep = copies
        .iter()
        .map(|item| {
            item.destination
                .strip_prefix(destination)
                .unwrap()
                .to_path_buf()
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut all = Vec::new();
    collect_files(destination, destination, &mut all)?;
    all.sort();
    all.into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(destination).ok()?.to_path_buf();
            let name = relative.file_name()?.to_string_lossy();
            if keep.contains(&relative)
                || name == "tau-library.tdb"
                || name.starts_with(".tau-library-")
                || !supported(&path)
            {
                return None;
            }
            Some((path, relative))
        })
        .map(|(destination, relative)| {
            Ok(DeleteItem {
                bytes: fs::metadata(&destination)?.len(),
                sha256: sha256_file(&destination)?,
                destination,
                relative,
            })
        })
        .collect()
}
pub(crate) fn rebuild_index(
    media_root: &Path,
    root_prefix: &str,
    plan_id: &str,
    warnings: &mut Vec<Warning>,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<(), TauError> {
    let scan = scan_dir_with_progress(media_root, true, progress)?;
    warnings.extend(scan.warnings);
    let index = build_index(&scan.entries, &scan.playlists, root_prefix, warnings)?;
    parse(&index)?;
    let temp = media_root.join(format!(".tau-library-source-{plan_id}.tmp"));
    write_durable(&temp, &index)?;
    let reparse = read_back_bytes(&temp)?;
    if reparse != index {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            "source index write verification failed",
        ));
    }
    parse(&reparse)?;
    swap_in_index(&temp, &media_root.join("tau-library.tdb"))?;
    Ok(())
}
pub(super) fn collect_files(
    root: &Path,
    at: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<(), TauError> {
    for child in fs::read_dir(at)? {
        let path = child?.path();
        if path.is_dir() {
            collect_files(root, &path, out)?;
        } else if path.is_file() && path.strip_prefix(root).is_ok() {
            out.push(path);
        }
    }
    Ok(())
}
/// Backs a card file up to the host in **one pass over the card**: the bytes are
/// hashed while they are copied, the hash must equal the one the user reviewed
/// (otherwise the file changed since the plan and nothing is kept), and the
/// finished backup is read back from the host disk before it is renamed into
/// place. Does not delete anything. The old per-file route read each card file
/// four times, which over the Pocket's USB mode is the difference between
/// minutes and an hour for a large album.
pub(crate) fn backup_copy_streaming(
    item: &DeleteItem,
    backup_root: &Path,
    plan_id: &str,
) -> Result<(), TauError> {
    let backup = backup_root.join(plan_id).join(&item.relative);
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = backup.with_extension(format!("tau-omega-{}.tmp", std::process::id()));
    let result = (|| -> Result<(), TauError> {
        let mut source = fs::File::open(&item.destination)?;
        let mut target = fs::File::create(&temp)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 1 << 20];
        loop {
            let n = source.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            target.write_all(&buffer[..n])?;
        }
        target.sync_all()?;
        if format!("{:x}", hasher.finalize()) != item.sha256 {
            return Err(TauError::e(
                ErrorCode::SourceChangedSincePlan,
                format!(
                    "changed since the plan was reviewed: {}",
                    item.relative.display()
                ),
            ));
        }
        verify_written(&temp, &item.sha256, &item.destination)
    })();
    let result = result.and_then(|()| fs::rename(&temp, &backup).map_err(TauError::from));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub(crate) fn backup_then_delete(
    item: &DeleteItem,
    backup_root: &Path,
    plan_id: &str,
) -> Result<(), TauError> {
    let backup = backup_root.join(plan_id).join(&item.relative);
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent)?;
    }
    let copy = CopyItem {
        source: item.destination.clone(),
        destination: backup.clone(),
        bytes: item.bytes,
        sha256: item.sha256.clone(),
        cover: None,
        state: CopyState::New,
    };
    copy_verified(&copy)?;
    if sha256_file(&backup)? != item.sha256 {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            format!("backup verification failed: {}", item.relative.display()),
        ));
    }
    fs::remove_file(&item.destination)?;
    Ok(())
}

/// Resolves `path` for "is it inside that folder" comparisons even when it
/// does not exist yet: the nearest existing ancestor is canonicalised (so
/// symlinked temp/volume roots such as macOS `/var` -> `/private/var` compare
/// equal) and the not-yet-created remainder is appended unchanged.
pub(crate) fn resolve_for_compare(path: &Path) -> PathBuf {
    if let Ok(real) = path.canonicalize() {
        return real;
    }
    let mut tail = Vec::new();
    let mut cursor = path;
    while let Some(parent) = cursor.parent() {
        if let Some(name) = cursor.file_name() {
            tail.push(name.to_os_string());
        }
        if let Ok(real) = parent.canonicalize() {
            return tail.iter().rev().fold(real, |acc, part| acc.join(part));
        }
        cursor = parent;
    }
    path.to_path_buf()
}

/// Whether a backup folder lies inside `root` (the card media root), compared
/// after resolving symlinks on both sides.
pub fn backup_is_inside(backup: &Path, root: &Path) -> bool {
    resolve_for_compare(backup).starts_with(resolve_for_compare(root))
}

pub(crate) fn validate_media_root(common: &Path) -> Result<(), TauError> {
    let components: Vec<_> = common.components().collect();
    let has_assets = components
        .iter()
        .any(|c| matches!(c,Component::Normal(n) if *n == "Assets"));
    if !has_assets || common.file_name().is_none_or(|n| n != "common") {
        return Err(TauError::e(
            ErrorCode::InvalidMediaRoot,
            "destination must be an explicit Assets/<platform>/common media root",
        ));
    }
    if !common.is_dir() {
        return Err(TauError::e(
            ErrorCode::InvalidMediaRoot,
            "destination media root does not exist",
        ));
    }
    Ok(())
}
pub(super) fn collect_source(
    root: &Path,
    at: &Path,
    include_root: bool,
    out: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), TauError> {
    let mut children: Vec<_> = fs::read_dir(at)?.filter_map(Result::ok).collect();
    children.sort_by_key(|e| e.file_name());
    for child in children {
        let path = child.path();
        let name = child.file_name().to_string_lossy().to_string();
        if is_junk(&name) {
            continue;
        }
        if path.is_dir() {
            collect_source(root, &path, false, out)?;
        } else if path.is_file() && supported(&path) {
            let rel = path.strip_prefix(root).unwrap();
            let mut converted = PathBuf::new();
            if include_root {
                converted.push(ascii_name(&required_file_name(root)?));
            }
            for part in rel.components() {
                converted.push(ascii_file_name(part.as_os_str().to_string_lossy().as_ref()));
            }
            out.push((path, converted));
        }
    }
    Ok(())
}
pub(crate) fn supported(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp3" | "flac" | "m3u")
    )
}
pub(crate) fn audio_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp3" | "flac")
    )
}
pub(crate) fn is_junk(name: &str) -> bool {
    name.starts_with("._") || matches!(name, ".DS_Store" | "Thumbs.db")
}
/// A source's file name, used to derive an ASCII-safe destination name.
/// `Path::file_name()` returns `None` for a handful of paths (`/`, `.`,
/// `..`, a bare prefix like `C:\`) that can still pass `.exists()`; a plain
/// `.unwrap()` here would let a caller-supplied source path panic the whole
/// plan instead of failing it cleanly (P1-2).
pub(super) fn required_file_name(path: &Path) -> Result<String, TauError> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| {
            TauError::e(
                ErrorCode::InvalidPathReference,
                format!(
                    "source has no file name to derive a destination name from: {}",
                    path.display()
                ),
            )
        })
}
pub(crate) fn ascii_file_name(name: &str) -> String {
    let name = ascii_name(name);
    if name.is_empty() {
        "track".into()
    } else {
        name
    }
}
pub(crate) fn sha256_file(path: &Path) -> Result<String, TauError> {
    let mut file = fs::File::open(path)?;
    let mut h = Sha256::new();
    // On the heap: commands now run on thread-pool threads with small stacks.
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
