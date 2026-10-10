//! T2's intentionally narrow write boundary: make a pure plan first, then
//! execute that exact plan after an explicit token confirmation.

use crate::{
    ErrorCode, Progress, ProgressObserver, Stage, TauError, Warning, WarningCode, ascii_name,
    build_index, cover, image, ledger, parse, scan_dir_with_progress, tick, verify,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CopyItem {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub cover: Option<CoverItem>,
    pub state: CopyState,
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoverItem {
    pub source: PathBuf,
    pub sha256: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum CopyState {
    New,
    Update,
    Same,
}
/// A folder-level `tau-art/cover_128.pal256.timg` sidecar this plan would
/// write. One entry per album folder (not per track), since the cover is
/// shared by every track in it — matching `tools/sync_media.py
/// --art-variants`'s own one-file-per-album convention.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArtSidecarItem {
    pub source_folder: PathBuf,
    pub destination: PathBuf,
    pub cover_source: PathBuf,
    pub cover_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeleteItem {
    pub destination: PathBuf,
    pub relative: PathBuf,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SyncPlan {
    pub id: String,
    pub destination: PathBuf,
    pub root_prefix: String,
    pub items: Vec<CopyItem>,
    pub deletions: Vec<DeleteItem>,
    pub embed_covers: bool,
    pub art_sidecars: Vec<ArtSidecarItem>,
    pub warnings: Vec<Warning>,
    pub bytes_to_write: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SyncReport {
    pub plan_id: String,
    pub copied: usize,
    pub unchanged: usize,
    pub bytes_written: u64,
    pub deleted: usize,
    pub art_sidecars_written: usize,
    pub index_path: PathBuf,
    pub index_sha256: String,
    pub warnings: Vec<Warning>,
}

/// Options for [`plan`]. `Default` gives the plain behaviour: add sources
/// under the destination, no mirror deletion, no cover embedding.
///
/// Replaces the four-deep `plan` -> `plan_with_options` -> `plan_with_features`
/// -> `plan_with_layout` telescoping-constructor chain (P1-3): one public
/// entry point, one options struct, so a new capability adds a field here
/// instead of another wrapper function.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlanOptions {
    /// Delete destination files that no longer correspond to a source.
    pub mirror: bool,
    /// Embed a discovered folder cover into destination MP3/FLAC copies. A
    /// cover hash is part of the plan token, preventing an unreviewed
    /// replacement.
    pub embed_covers: bool,
    /// Write a `tau-art/cover_128.pal256.timg` sidecar next to each album
    /// folder that has a discovered cover, encoded per tau-alpha's decided
    /// default (`IMAGE_FORMATS.md` D-I01/D-I02). Forward-prep: no firmware
    /// reader exists yet, so this has no effect on the Pocket itself today
    /// (`docs/FIRMWARE_SYNC.md`'s "Watched interfaces"); it does feed this
    /// app's own decode-and-preview path.
    pub art_sidecar_pal256: bool,
}

mod plan;
pub use plan::*;
mod execute;
pub use execute::*;
mod sources;
pub use sources::*;
mod recovery;
pub use recovery::*;
mod verified_copy;
pub(crate) use verified_copy::*;

#[cfg(test)]
mod tests;
