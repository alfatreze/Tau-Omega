//! Cover files for albums already on a card: finds albums that have none (the Pocket needs one per album,
//! made from a folder image or the art embedded in the tracks) and writes them. Reviewed like every other
//! card change: plan first, then execute with the plan's id.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoverFilePlan {
    pub id: String,
    /// Albums (folders with tracks) in the media root.
    pub albums: usize,
    /// Albums that already have a cover file.
    pub have_cover_file: usize,
    pub items: Vec<ArtSidecarItem>,
    /// Folders that have no cover file and no picture to make one from.
    pub no_picture: Vec<String>,
}

/// Which folders hold tracks, with their tracks, in a stable order.
fn album_folders(
    media_root: &Path,
) -> Result<std::collections::BTreeMap<PathBuf, Vec<PathBuf>>, TauError> {
    let mut files = Vec::new();
    collect_files(media_root, media_root, &mut files)?;
    files.sort();
    let mut folders: std::collections::BTreeMap<PathBuf, Vec<PathBuf>> = Default::default();
    for file in files {
        let hidden = file
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("._"));
        if audio_file(&file)
            && !hidden
            && let Some(parent) = file.parent()
        {
            folders.entry(parent.to_path_buf()).or_default().push(file);
        }
    }
    Ok(folders)
}

/// How many albums have no cover file. Reads folder listings only.
pub fn count_missing_cover_files(media_root: &Path) -> usize {
    album_folders(media_root)
        .map(|folders| {
            folders
                .keys()
                .filter(|f| !f.join(image::pal256_sidecar_name(128)).is_file())
                .count()
        })
        .unwrap_or(0)
}

pub fn plan_cover_files(media_root: &Path) -> Result<CoverFilePlan, TauError> {
    let folders = album_folders(media_root)?;
    let mut plan = CoverFilePlan {
        id: String::new(),
        albums: folders.len(),
        have_cover_file: 0,
        items: Vec::new(),
        no_picture: Vec::new(),
    };
    for (folder, tracks) in &folders {
        let destination = folder.join(image::pal256_sidecar_name(128));
        if destination.is_file() {
            plan.have_cover_file += 1;
            continue;
        }
        match art_source(folder, tracks)? {
            Some((cover_source, cover_sha256, from_embedded)) => plan.items.push(ArtSidecarItem {
                source_folder: folder.clone(),
                destination,
                cover_source,
                cover_sha256,
                from_embedded,
            }),
            None => plan.no_picture.push(
                folder
                    .strip_prefix(media_root)
                    .unwrap_or(folder)
                    .to_string_lossy()
                    .replace('\\', "/"),
            ),
        }
    }
    let mut hasher = Sha256::new();
    for item in &plan.items {
        hasher.update(item.destination.to_string_lossy().as_bytes());
        hasher.update(item.cover_sha256.as_bytes());
    }
    plan.id = format!("{:x}", hasher.finalize());
    Ok(plan)
}

/// Writes a reviewed plan's cover files, then rebuilds the library index (when there is one) so the
/// player's track details see them. Returns how many files were written.
pub fn execute_cover_files(
    media_root: &Path,
    plan: &CoverFilePlan,
    confirmation: &str,
) -> Result<usize, TauError> {
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "the confirmation does not match the reviewed plan",
        ));
    }
    let fresh = plan_cover_files(media_root)?;
    if fresh.id != plan.id {
        return Err(TauError::e(
            ErrorCode::SourceChangedSincePlan,
            "the media changed since the plan was reviewed",
        ));
    }
    for item in &plan.items {
        write_art_sidecar(item)?;
    }
    if let Some(root) = crate::refresh::index_state(media_root)
        .root
        .filter(|_| media_root.join("tau-library.tdb").is_file())
    {
        rebuild_index(media_root, &root, &plan.id[..8], &mut Vec::new(), &mut None)?;
    }
    Ok(plan.items.len())
}
