//! Album-level library operations behind the library workbench screen:
//! list a media root as albums and tracks, plan copying a *selection* of
//! albums, and plan/execute removing albums from a card.
//!
//! Everything here follows the engine's existing plan -> confirm -> execute
//! shape and its safety rules: sources are never modified, removals are
//! backed up first when a backup folder is given, verified before deletion,
//! and the index is rebuilt (and verified) last.

use crate::{
    ErrorCode, Progress, ProgressObserver, Stage, TauError, Warning, ascii_name, cover, playlist,
    sync, tick,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

mod listing;
pub use listing::*;
mod edits;
pub use edits::*;
/// The device-ASCII display of a name, exposed so a front-end can preview
/// what a rename will look like on the Pocket.
pub fn device_name(input: &str) -> String {
    ascii_name(input)
}

#[cfg(test)]
mod tests;
