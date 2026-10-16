//! T2's intentionally narrow write boundary: make a pure plan first, then
//! execute that exact plan after an explicit token confirmation.

use crate::{
    ErrorCode, Progress, ProgressObserver, Stage, TauError, Warning, WarningCode, ascii_name,
    build_index, convert, cover, image, ledger, parse, scan_dir_measuring, tick, verify,
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
    /// Set when the copy is not byte for byte: a conversion, or ReplayGain tags added to the copy.
    #[cfg_attr(feature = "serde", serde(default))]
    pub transform: Option<Transform>,
}
/// What is done to a file on its way to the card besides copying it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transform {
    /// `Some` for a conversion; `None` when only ReplayGain tags are added to the copy.
    pub kind: Option<convert::Kind>,
    /// Why (shown in the review).
    pub reason: String,
    /// ReplayGain tags written into the copy (name, value).
    pub replaygain: Vec<(String, String)>,
    /// Frames the converted file will have, when the source says.
    pub frames: Option<u64>,
    /// The source's rate, for the progress display and the output check.
    pub source_rate: u32,
}
impl Transform {
    /// A stable text identifying exactly what will be made, for the plan token and the ledger.
    pub fn spec(&self) -> String {
        let mut text = self
            .kind
            .as_ref()
            .map_or_else(|| "tags".to_string(), convert::Kind::spec);
        for (k, v) in &self.replaygain {
            text.push_str(&format!("|{k}={v}"));
        }
        text
    }
}
/// A source file left out of the plan, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SkippedItem {
    pub source: PathBuf,
    pub reason: String,
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
    /// True when the picture is the cover embedded in `cover_source` (an audio file), not an image file.
    #[cfg_attr(feature = "serde", serde(default))]
    pub from_embedded: bool,
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
    /// Files left out (formats the Pocket cannot play that were not converted).
    #[cfg_attr(feature = "serde", serde(default))]
    pub skipped: Vec<SkippedItem>,
    /// The `lame` found when planning (used to make MP3s).
    #[cfg_attr(feature = "serde", serde(default))]
    pub lame: Option<PathBuf>,
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
    /// Files written as conversions, and files that got ReplayGain tags in their copy.
    #[cfg_attr(feature = "serde", serde(default))]
    pub converted: usize,
    #[cfg_attr(feature = "serde", serde(default))]
    pub replaygain_tagged: usize,
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
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
    /// Conversion of files the Pocket cannot play, and ReplayGain. The default is everything off.
    pub convert: convert::ConvertSettings,
}

mod plan;
pub use plan::*;
mod covers;
pub use covers::*;
mod execute;
pub use execute::*;
mod sources;
pub use sources::*;
mod recovery;
pub use recovery::*;
mod transform;
use transform::*;
mod verified_copy;
pub(crate) use verified_copy::*;

#[cfg(test)]
mod tests;
