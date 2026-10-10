//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

/// Stable, front-end-branchable error codes.
///
/// `11..=17` mirror the firmware index loader's own E-codes exactly (see
/// `tau-alpha/docs/MEDIA_LIBRARY_0.4_SPEC.md`), so a front-end that already
/// knows the firmware's vocabulary reuses it here for index problems. Codes
/// from `30` up are engine/domain errors with no firmware equivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ErrorCode {
    IndexHeader = 11,
    IndexSize = 12,
    IndexBodyCrc = 13,
    IndexCapExceeded = 14,
    IndexSectionRange = 15,
    IndexRecordRange = 17,
    InvalidMediaRoot = 30,
    SourceMissing = 31,
    SamePath = 32,
    NameCollision = 33,
    NoSources = 34,
    ConfirmationMismatch = 35,
    UnsafeBackupLocation = 36,
    InvalidJournalLocation = 37,
    VerificationFailed = 38,
    SourceChangedSincePlan = 39,
    UnsupportedCover = 40,
    InvalidPathReference = 41,
    Io = 42,
    Json = 43,
    Cancelled = 44,
    NotFound = 45,
    NotACheckSummary = 46,
    InvalidTaudRecord = 47,
    NoQrCodeFound = 48,
    InvalidTim1Container = 49,
    UnsupportedTags = 50,
    InsufficientSpace = 51,
    InvalidTheme = 52,
    InvalidAssetsFile = 53,
    /// A Tau release manifest (`tau-compat.json`) that is malformed or of a schema this build does not know.
    InvalidReleaseManifest = 54,
    /// An install plan that must not run (a refused pair, a downgrade not chosen, a full card) or a rollback that cannot be trusted.
    InstallRefused = 55,
    /// A library refresh that must not run (nothing can be indexed, or the library exceeds what the Pocket holds).
    RefreshRefused = 56,
}

impl ErrorCode {
    /// The stable numeric identifier a front-end can branch on without
    /// parsing English text.
    pub const fn as_u16(self) -> u16 {
        self as u16
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "E{}", self.as_u16())
    }
}

impl TryFrom<u16> for ErrorCode {
    type Error = ();
    fn try_from(value: u16) -> Result<Self, ()> {
        Ok(match value {
            11 => Self::IndexHeader,
            12 => Self::IndexSize,
            13 => Self::IndexBodyCrc,
            14 => Self::IndexCapExceeded,
            15 => Self::IndexSectionRange,
            17 => Self::IndexRecordRange,
            30 => Self::InvalidMediaRoot,
            31 => Self::SourceMissing,
            32 => Self::SamePath,
            33 => Self::NameCollision,
            34 => Self::NoSources,
            35 => Self::ConfirmationMismatch,
            36 => Self::UnsafeBackupLocation,
            37 => Self::InvalidJournalLocation,
            38 => Self::VerificationFailed,
            39 => Self::SourceChangedSincePlan,
            40 => Self::UnsupportedCover,
            41 => Self::InvalidPathReference,
            42 => Self::Io,
            43 => Self::Json,
            44 => Self::Cancelled,
            45 => Self::NotFound,
            46 => Self::NotACheckSummary,
            47 => Self::InvalidTaudRecord,
            48 => Self::NoQrCodeFound,
            49 => Self::InvalidTim1Container,
            50 => Self::UnsupportedTags,
            51 => Self::InsufficientSpace,
            52 => Self::InvalidTheme,
            53 => Self::InvalidAssetsFile,
            54 => Self::InvalidReleaseManifest,
            55 => Self::InstallRefused,
            56 => Self::RefreshRefused,
            _ => return Err(()),
        })
    }
}

// `ErrorCode` is serialised as its stable `u16` (matching `.as_u16()`, the
// convention every hand-written boundary already used before this feature
// existed) rather than the derived default of a PascalCase variant name, so
// enabling `serde` does not change the wire shape a host already depends on.
#[cfg(feature = "serde")]
impl serde::Serialize for ErrorCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u16(self.as_u16())
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ErrorCode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = u16::deserialize(deserializer)?;
        ErrorCode::try_from(value)
            .map_err(|()| serde::de::Error::custom(format!("unknown error code {value}")))
    }
}

/// A structured engine error: `code` is stable and meant for a host to branch
/// on; `message` is an English sentence for display only.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TauError {
    pub code: ErrorCode,
    pub message: String,
}

impl TauError {
    pub fn code(&self) -> ErrorCode {
        self.code
    }
    pub(crate) fn e(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for TauError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for TauError {}
impl From<io::Error> for TauError {
    fn from(e: io::Error) -> Self {
        Self::e(ErrorCode::Io, e.to_string())
    }
}

/// A stable identifier for a non-fatal warning. Unlike `ErrorCode` these never
/// mirror a firmware code; they are entirely this engine's own vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum WarningCode {
    NotAPocketCard,
    MissingCoreJson,
    TagsUnreadable,
    MissingTag,
    PlaylistEntriesDropped,
    PlaylistTruncated,
    NoMediaFound,
    /// An album's cover could not be put inside its songs (not a baseline JPEG, or over the
    /// firmware's size limit). The songs are still copied, just without that embedded picture.
    CoverNotEmbedded,
    /// A file on the card is not what this tool wrote there, so it (and the files from earlier syncs that
    /// could not be re-checked) will be copied again.
    CardFileChanged,
}

impl WarningCode {
    /// A short, stable, machine-readable identifier (never renders English).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotAPocketCard => "not_a_pocket_card",
            Self::MissingCoreJson => "missing_core_json",
            Self::TagsUnreadable => "tags_unreadable",
            Self::MissingTag => "missing_tag",
            Self::PlaylistEntriesDropped => "playlist_entries_dropped",
            Self::PlaylistTruncated => "playlist_truncated",
            Self::NoMediaFound => "no_media_found",
            Self::CoverNotEmbedded => "cover_not_embedded",
            Self::CardFileChanged => "card_file_changed",
        }
    }
}

/// A structured warning: `code` is stable and meant for a host to branch on;
/// `message` is an English sentence for display only.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Warning {
    pub code: WarningCode,
    pub message: String,
}
impl Warning {
    pub(crate) fn new(code: WarningCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Which phase of a long-running call a `Progress` report belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Stage {
    Scanning,
    Hashing,
    Copying,
    BuildingIndex,
    Verifying,
    Deleting,
    Editing,
}

/// One progress report from a long-running scan/plan/execute call. `done` and
/// `total` share a unit within one stage (files, except bytes for `Copying`).
/// `total` is `0` when it is not known in advance.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Progress {
    pub stage: Stage,
    pub done: u64,
    pub total: u64,
    pub path: Option<String>,
}

/// A caller-supplied progress/cancellation observer for scan/plan/execute.
/// Returning `false` from `report` cancels the running call, which then
/// returns `Err` with `ErrorCode::Cancelled`. Any `FnMut(Progress) -> bool`
/// implements this automatically, so a plain closure is enough; there is no
/// runtime or executor dependency.
pub trait ProgressObserver {
    fn report(&mut self, progress: Progress) -> bool;
}
impl<F: FnMut(Progress) -> bool> ProgressObserver for F {
    fn report(&mut self, progress: Progress) -> bool {
        self(progress)
    }
}

/// Reports one unit of progress through an optional observer. Returns
/// `Err(Cancelled)` if the observer declines to continue; every long-running
/// loop in this crate calls this once per unit of work so a host can cancel
/// promptly rather than only at the next stage boundary.
pub(super) fn tick(
    observer: &mut Option<&mut dyn ProgressObserver>,
    progress: Progress,
) -> Result<(), TauError> {
    if let Some(observer) = observer
        && !observer.report(progress)
    {
        return Err(TauError::e(ErrorCode::Cancelled, "cancelled by caller"));
    }
    Ok(())
}
