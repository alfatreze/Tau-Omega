//! Portable, UI-independent Tau Omega engine.
//!
//! This crate deliberately has no card-writing API in T0/T1. It can inspect a
//! card and build, parse and verify the Tau v1 index from a folder or fixture.

pub mod assets;
pub mod backup;
/// Test-only: a number no two calls in this process ever share, added to the
/// clock when building scratch folder names. The clock alone is not unique enough:
/// parallel tests that asked for the same folder name in the same clock tick shared
/// a directory and failed about 4 percent of full runs (8 of 200).
#[cfg(test)]
pub(crate) fn test_uniq() -> u128 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    u128::from(NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)) << 50
}

pub mod breakdown;
pub mod cardlayout;
pub mod changes;
pub mod compare;
pub mod compat;
pub mod cover;
pub mod diag;
pub mod diagnostics;
pub mod duplicates;
pub mod halcyon;
pub mod icon;
pub mod image;
pub mod install_exec;
pub mod install_plan;
pub mod journal;
pub mod ledger;
pub mod marker;
pub mod package;
pub mod playlist;
pub mod problems;
pub mod refresh;
pub mod release_check;
pub mod remove;
pub mod screenshots;
pub mod settings_migrate;
pub mod storage;
pub mod sync;
pub mod tagedit;
mod tags;
pub mod taud;
use tags::{mp3_seconds, read_flac, read_id3v1, read_id3v2};
pub mod tpg;
pub mod update;
pub mod workbench;

use crc32fast::hash as crc32;
use serde_json::Value;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self},
    path::{Path, PathBuf},
};

pub const ROOT_PREFIX: &str = "/Assets/tau/common/";
const MAGIC: u32 = 0x4249_4c54;
const HEADER: usize = 128;
const SECTIONS: [&str; 8] = [
    "artists",
    "albums",
    "tracks",
    "strings",
    "album_by_title",
    "track_by_title",
    "letters",
    "playlists",
];
/// The Pocket index's hard capacity limit (see the index format).
pub const MAX_TRACKS: usize = 16_384;
/// The Pocket index's hard capacity limit (see the index format).
pub const MAX_ALBUMS: usize = 2_048;
/// The Pocket index's hard capacity limit (see the index format).
pub const MAX_ARTISTS: usize = 1_024;
const MAX_PLAYLISTS: usize = 64;
const MAX_FILE: usize = 4 << 20;
const MAX_STRINGS: usize = 3 << 20;
const MAX_PATH: usize = 200;

mod error;
pub use error::*;
mod card;
pub use card::*;
mod scan;
pub use scan::*;
mod build;
pub use build::*;
mod index_read;
pub use index_read::*;
