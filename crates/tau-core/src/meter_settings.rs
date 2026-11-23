//! The per-meter settings file, `tau-meter-settings.bin` (format `TMSF` version 1): reading it, finding it on a card, and the one thing Omega may do
//! to it, which is to delete a copy of the wrong size.
//!
//! The Pocket creates and writes this file itself, at Quit and at a power-off (Tau Alpha messages 028 to 032, decision D-M27). It lives at
//! `Saves/<platform>/<core folder>/tau-meter-settings.bin`, exactly 8,192 bytes: two generations of 4,096 bytes, each a 20-byte header (`TMSF`,
//! version, record count, `seq`, `used`, CRC-32) and `used` bytes of records. The reader is written from Tau Alpha's reference
//! (`tools/meter_settings.py`, spec `docs/features/meters/METER_SAVE_FILE_SPEC.md` section 4) and checked against their 18 golden vectors
//! (`testdata/meter_settings/`, `tests/meter_settings.rs`).
//!
//! **What Omega does not do, by Tau Alpha's rules (028 to 032):** it never creates the file (the Pocket makes it), never edits, overwrites or
//! rewrites one, never writes it while a core could be running, and keeps it across updates. There is therefore no writer in this module.
//! **The one exception is a file of the wrong size**: the Pocket does not skip such a file, it stops at start with "Error in framework file id
//! [11] size bad" and the core does not run (Tau Alpha 032, measured). [`plan_delete`] and [`execute_delete`] remove exactly that file, after a
//! reviewed plan and a verified backup outside the card, and refuse any file of the right size.

use crate::{ErrorCode, TauError, core_update::is_tau_core, sync::sha256_file};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

pub const FILE_NAME: &str = "tau-meter-settings.bin";
pub const VERSION: u16 = 1;
/// The size the Pocket accepts: any other size stops the core at start.
pub const FILE_SIZE: usize = 8192;
const BANK: usize = 4096;
const HEADER: usize = 20;
const MAX_USED: usize = BANK - HEADER;
const MAX_ENTRIES: usize = 64;

/// What a reader makes of a file's bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Status {
    /// A valid generation exists.
    Ok,
    /// No valid generation (zero, all 0xFF or damaged): every meter starts from its defaults.
    None,
    /// A generation with a version above 1: the whole file is ignored and nothing is written (a newer Tau made it).
    Locked,
    /// Not 8,192 bytes: the Pocket refuses the file with an error screen.
    Size,
}

/// One meter's saved settings: its id hash, the `params_rev` they were saved for (a record of another revision is not used), the preset and the
/// changed parameters as (index, value).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Record {
    pub id_hash: u32,
    pub params_rev: u16,
    pub preset: u8,
    pub entries: Vec<(u8, u16)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Parsed {
    pub status: Status,
    /// The bank (0 or 1) whose generation is used, `None` when there is none or the file is locked.
    pub active: Option<usize>,
    pub seq: u32,
    /// The active generation's records, oldest first (empty unless `status` is `Ok`).
    pub records: Vec<Record>,
    /// Each bank's `seq` when it holds a valid generation.
    pub generations: [Option<u32>; 2],
}

enum Generation {
    Valid(u32, Vec<Record>),
    Newer,
    Bad,
}

fn u16at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u32at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// One 4,096-byte generation: valid when the magic, version, `used <= 4076`, the records exactly filling `used` and the CRC (CRC-32 over header
/// bytes 0..15, then the records) all hold. A version above 1 is `Newer`.
fn generation(g: &[u8]) -> Generation {
    if &g[..4] != b"TMSF" {
        return Generation::Bad;
    }
    let (version, count, seq, used, reserved, crc) = (
        u16at(g, 4),
        usize::from(u16at(g, 6)),
        u32at(g, 8),
        usize::from(u16at(g, 12)),
        u16at(g, 14),
        u32at(g, 16),
    );
    if version > VERSION {
        return Generation::Newer;
    }
    if version != VERSION || reserved != 0 || used > MAX_USED {
        return Generation::Bad;
    }
    let body = &g[HEADER..HEADER + used];
    let mut h = crc32fast::Hasher::new();
    h.update(&g[..16]);
    h.update(body);
    if h.finalize() != crc {
        return Generation::Bad;
    }
    let (mut records, mut pos) = (Vec::new(), 0usize);
    while records.len() < count {
        if pos + 8 > used {
            return Generation::Bad;
        }
        let (id_hash, params_rev, preset, n) = (
            u32at(body, pos),
            u16at(body, pos + 4),
            body[pos + 6],
            usize::from(body[pos + 7]),
        );
        if n > MAX_ENTRIES || pos + 8 + 3 * n > used {
            return Generation::Bad;
        }
        let entries = (0..n)
            .map(|k| (body[pos + 8 + 3 * k], u16at(body, pos + 9 + 3 * k)))
            .collect();
        records.push(Record {
            id_hash,
            params_rev,
            preset,
            entries,
        });
        pos += 8 + 3 * n;
    }
    if pos != used {
        return Generation::Bad;
    }
    Generation::Valid(seq, records)
}

/// Reads a file's bytes. The active generation is the valid one with the larger `seq` in serial arithmetic (`a - b` read as a signed 32-bit
/// number is positive; a tie takes bank 0). A version above 1 in either generation ignores the whole file. Never panics.
pub fn parse(b: &[u8]) -> Parsed {
    let none = |status| Parsed {
        status,
        active: None,
        seq: 0,
        records: Vec::new(),
        generations: [None, None],
    };
    if b.len() != FILE_SIZE {
        return none(Status::Size);
    }
    let gens = [generation(&b[..BANK]), generation(&b[BANK..])];
    let locked = gens.iter().any(|g| matches!(g, Generation::Newer));
    let seqs = [0, 1].map(|i| match &gens[i] {
        Generation::Valid(s, _) => Some(*s),
        _ => None,
    });
    let active = match seqs {
        [Some(a), Some(c)] => Some(usize::from(c.wrapping_sub(a) as i32 > 0)),
        [Some(_), None] => Some(0),
        [None, Some(_)] => Some(1),
        [None, None] => None,
    };
    if locked {
        return Parsed {
            generations: seqs,
            ..none(Status::Locked)
        };
    }
    let Some(bank) = active else {
        return Parsed {
            generations: seqs,
            ..none(Status::None)
        };
    };
    let Generation::Valid(seq, records) = gens.into_iter().nth(bank).unwrap() else {
        unreachable!("the active bank is valid")
    };
    Parsed {
        status: Status::Ok,
        active: Some(bank),
        seq,
        records,
        generations: seqs,
    }
}

/// The record a meter would start from: its id hash and the `params_rev` it is running (a record saved for another revision is not returned).
pub fn find(parsed: &Parsed, id_hash: u32, params_rev: u16) -> Option<&Record> {
    parsed
        .records
        .iter()
        .find(|r| r.id_hash == id_hash && r.params_rev == params_rev)
}

// ---------------------------------------------------------------- on a card

/// What is on the card at a settings file's place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum FileState {
    /// Holds saved settings for this many meters.
    Saved(usize),
    /// Nothing usable inside (zero, all 0xFF or damaged): the meters start from their defaults. The Pocket accepts it.
    Defaults,
    /// Written by a newer Tau: left alone, nothing in it is used.
    Newer,
    /// Not 8,192 bytes: **the Pocket will refuse it and the core will not start.**
    WrongSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FoundFile {
    /// Relative to the card root, forward slashes: `Saves/<platform>/<core folder>/tau-meter-settings.bin`.
    pub rel_path: String,
    pub platform: String,
    pub core: String,
    pub bytes: u64,
    pub state: FileState,
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .map(|r| {
            r.filter_map(Result::ok)
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// The settings files of the card's Tau cores (`Saves/<platform>/<Tau core folder>/tau-meter-settings.bin`), in path order. Reads only the files of
/// exactly 8,192 bytes (the others are reported by their size). Writes nothing.
pub fn scan_card(card: &Path) -> Vec<FoundFile> {
    let mut out = Vec::new();
    for platform in names(&card.join("Saves")) {
        for core in names(&card.join("Saves").join(&platform)) {
            if !is_tau_core(&core) {
                continue;
            }
            let path = card
                .join("Saves")
                .join(&platform)
                .join(&core)
                .join(FILE_NAME);
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let state = if meta.len() != FILE_SIZE as u64 {
                FileState::WrongSize
            } else {
                match fs::read(&path).map(|b| parse(&b)) {
                    Ok(p) => match p.status {
                        Status::Ok => FileState::Saved(p.records.len()),
                        Status::Locked => FileState::Newer,
                        Status::None | Status::Size => FileState::Defaults,
                    },
                    Err(_) => continue,
                }
            };
            out.push(FoundFile {
                rel_path: format!("Saves/{platform}/{core}/{FILE_NAME}"),
                platform: platform.clone(),
                core,
                bytes: meta.len(),
                state,
            });
        }
    }
    out
}

/// What deleting one wrong-sized settings file would do.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeletePlan {
    /// Confirmation token: covers the card, the path, the size and the modification time.
    pub id: String,
    pub rel_path: String,
    pub core: String,
    pub bytes: u64,
    /// `Some(reason)` when the plan must not run.
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeleteReport {
    pub bytes: u64,
    /// Where the file was copied first (outside the card).
    pub backup_dir: PathBuf,
}

fn refuse(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InstallRefused, msg)
}

/// Checks `rel` is exactly `Saves/<platform>/<Tau core folder>/tau-meter-settings.bin` and returns the core folder.
fn core_of(rel: &str) -> Result<String, TauError> {
    let parts: Vec<&str> = rel.split('/').collect();
    let sound = |p: &str| {
        !p.is_empty() && p != "." && p != ".." && !p.contains(['\\', ':']) && !p.starts_with('.')
    };
    if parts.len() != 4
        || parts[0] != "Saves"
        || parts[3] != FILE_NAME
        || !parts[1..3].iter().all(|p| sound(p))
        || !is_tau_core(parts[2])
    {
        return Err(refuse(format!(
            "{rel} is not a Tau settings file, so it is not removed."
        )));
    }
    Ok(parts[2].to_string())
}

/// Plans deleting a settings file of the wrong size. Reads the card; writes nothing. A file of the right size is **blocked**: the Pocket accepts
/// it, it is the user's saved meters, and Omega never deletes or rewrites a working one.
pub fn plan_delete(card: &Path, rel: &str) -> Result<DeletePlan, TauError> {
    let core = core_of(rel)?;
    let path = card.join(rel);
    let meta = fs::symlink_metadata(&path)
        .map_err(|_| refuse(format!("{rel} is not on the card any more.")))?;
    if !meta.is_file() {
        return Err(refuse(format!("{rel} is not a file.")));
    }
    let mtime = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let mut h = Sha256::new();
    h.update(card.to_string_lossy().as_bytes());
    h.update([0]);
    h.update(rel.as_bytes());
    h.update(meta.len().to_le_bytes());
    h.update(mtime.to_le_bytes());
    let blocked = (meta.len() == FILE_SIZE as u64).then(|| {
        "This file is the right size, so the Pocket accepts it. It holds your saved meter settings, and Omega never deletes or changes a working one.".to_string()
    });
    Ok(DeletePlan {
        id: format!("{:x}", h.finalize()),
        rel_path: rel.to_string(),
        core,
        bytes: meta.len(),
        blocked,
    })
}

/// Deletes the reviewed file. The plan is rebuilt and must have the same id (a file that changed since it was shown stops the run); the file is
/// copied to `backup_root/<plan id>/<path>`, verified by SHA-256, and only then deleted. The `._` companion macOS leaves on exFAT goes with it.
pub fn execute_delete(
    card: &Path,
    rel: &str,
    confirmation: &str,
    backup_root: &Path,
) -> Result<DeleteReport, TauError> {
    let plan = plan_delete(card, rel)?;
    if confirmation != plan.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "The confirmation does not match the current plan: something changed since it was shown. Review it again.",
        ));
    }
    if let Some(why) = plan.blocked {
        return Err(refuse(why));
    }
    if backup_root.starts_with(card) {
        return Err(refuse("The backup folder must be outside the card."));
    }
    let (path, backup_dir) = (card.join(rel), backup_root.join(&plan.id[..16]));
    let backup = backup_dir.join(rel);
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent)?;
    }
    let sha = sha256_file(&path)?;
    fs::copy(&path, &backup)?;
    if sha256_file(&backup)? != sha {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            format!("The backup of {rel} did not verify; it was not removed."),
        ));
    }
    fs::remove_file(&path)?;
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        let _ = fs::remove_file(parent.join(format!("._{}", name.to_string_lossy())));
    }
    Ok(DeleteReport {
        bytes: plan.bytes,
        backup_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "tau-mset-{tag}-{}",
            u128::from(std::process::id()) + crate::test_uniq()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn put(card: &Path, rel: &str, bytes: &[u8]) {
        let p = card.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }
    const TAU: &str = "Saves/tau/alfatreze.TAU/tau-meter-settings.bin";

    #[test]
    fn a_zero_file_and_a_short_one_are_what_the_reference_says() {
        assert_eq!(parse(&[0; FILE_SIZE]).status, Status::None);
        assert_eq!(parse(&[0xFF; FILE_SIZE]).status, Status::None);
        for n in [0, 1, 8191, 8193, 20_000] {
            let p = parse(&vec![0; n]);
            assert_eq!(
                (p.status, p.active, p.records.len()),
                (Status::Size, None, 0),
                "{n} bytes"
            );
        }
    }

    #[test]
    fn sequence_numbers_compare_in_serial_arithmetic_and_a_tie_takes_bank_zero() {
        let newer = |a: u32, b: u32| usize::from(b.wrapping_sub(a) as i32 > 0);
        assert_eq!(newer(1, 2), 1);
        assert_eq!(newer(2, 1), 0);
        assert_eq!(newer(0xFFFF_FFFF, 0), 1, "0 is newer than 0xFFFFFFFF");
        assert_eq!(newer(0, 0xFFFF_FFFF), 0);
        assert_eq!(newer(0x7FFF_FFFF, 0x8000_0000), 1);
        assert_eq!(newer(0x8000_0000, 0x7FFF_FFFF), 0);
        assert_eq!(newer(5, 5), 0);
    }

    #[test]
    fn the_scan_finds_tau_cores_settings_by_state_and_ignores_everything_else() {
        let card = scratch("scan");
        put(&card, TAU, &[0; FILE_SIZE]);
        put(
            &card,
            "Saves/tau/alfatreze.TAU Diagnostics/tau-meter-settings.bin",
            &[0; 8191],
        );
        put(
            &card,
            "Saves/tau_dev/alfatreze.TAU DEV 230/tau-meter-settings.bin",
            &[0xFF; FILE_SIZE],
        );
        put(
            &card,
            "Saves/gba/other.Core/tau-meter-settings.bin",
            &[0; 10],
        ); // not a Tau core
        put(&card, "Saves/tau/alfatreze.TAU/.hidden", b"x");
        let found = scan_card(&card);
        let got: Vec<(&str, FileState)> = found
            .iter()
            .map(|f| (f.rel_path.as_str(), f.state))
            .collect();
        assert_eq!(
            got,
            [
                (
                    "Saves/tau/alfatreze.TAU/tau-meter-settings.bin",
                    FileState::Defaults
                ),
                (
                    "Saves/tau/alfatreze.TAU Diagnostics/tau-meter-settings.bin",
                    FileState::WrongSize
                ),
                (
                    "Saves/tau_dev/alfatreze.TAU DEV 230/tau-meter-settings.bin",
                    FileState::Defaults
                ),
            ]
        );
        assert_eq!(found[1].bytes, 8191);
        assert!(scan_card(&card.join("nothing")).is_empty());
        assert!(scan_card(&scratch("empty")).is_empty());
    }

    #[test]
    fn only_a_wrong_sized_tau_settings_file_can_be_planned_for_deletion() {
        let card = scratch("plan");
        put(&card, TAU, &[7; FILE_SIZE]);
        put(
            &card,
            "Saves/tau/alfatreze.TAU Diagnostics/tau-meter-settings.bin",
            &[7; 8191],
        );
        put(
            &card,
            "Saves/gba/other.Core/tau-meter-settings.bin",
            &[7; 5],
        );
        let good = plan_delete(&card, TAU).unwrap();
        assert!(
            good.blocked.as_deref().unwrap().contains("right size"),
            "a working file is never deleted"
        );
        let bad = plan_delete(
            &card,
            "Saves/tau/alfatreze.TAU Diagnostics/tau-meter-settings.bin",
        )
        .unwrap();
        assert_eq!(
            (bad.bytes, bad.core.as_str(), bad.blocked.is_none()),
            (8191, "alfatreze.TAU Diagnostics", true)
        );
        for rel in [
            "Saves/gba/other.Core/tau-meter-settings.bin",
            "Saves/tau/alfatreze.TAU/other.bin",
            "Saves/tau/../tau/alfatreze.TAU/tau-meter-settings.bin",
            "Saves/tau/alfatreze.TAU/tau-meter-settings.bin/x",
            "/abs",
            "Assets/tau/common/tau-meters.bin",
            "Saves/tau/alfatreze.TAU/tau-meter-settings.bin\\x",
            "",
        ] {
            assert!(plan_delete(&card, rel).is_err(), "{rel}");
        }
        assert!(
            plan_delete(
                &card,
                "Saves/tau/alfatreze.TAU Preview/tau-meter-settings.bin"
            )
            .is_err(),
            "not on the card"
        );
    }

    #[test]
    fn deleting_backs_up_verifies_and_removes_exactly_that_file() {
        let card = scratch("del");
        let backup = scratch("delbk");
        let rel = "Saves/tau/alfatreze.TAU/tau-meter-settings.bin";
        put(&card, rel, &[9; 5000]);
        put(
            &card,
            "Saves/tau/alfatreze.TAU/._tau-meter-settings.bin",
            b"apple",
        );
        put(&card, "Saves/tau/alfatreze.TAU/other.sav", b"keep me");
        let plan = plan_delete(&card, rel).unwrap();
        // a wrong confirmation, or a backup folder on the card, stops before anything happens
        assert_eq!(
            execute_delete(&card, rel, "nope", &backup)
                .unwrap_err()
                .code,
            ErrorCode::ConfirmationMismatch
        );
        assert!(execute_delete(&card, rel, &plan.id, &card.join("backups")).is_err());
        assert!(card.join(rel).exists());
        let rep = execute_delete(&card, rel, &plan.id, &backup).unwrap();
        assert_eq!(rep.bytes, 5000);
        assert!(!card.join(rel).exists());
        assert!(
            !card
                .join("Saves/tau/alfatreze.TAU/._tau-meter-settings.bin")
                .exists()
        );
        assert_eq!(fs::read(rep.backup_dir.join(rel)).unwrap(), vec![9u8; 5000]);
        assert_eq!(
            fs::read(card.join("Saves/tau/alfatreze.TAU/other.sav")).unwrap(),
            b"keep me",
            "the neighbours are untouched"
        );
    }

    #[test]
    fn a_file_that_changed_after_the_plan_was_shown_is_not_deleted_and_a_right_sized_one_is_never_deleted()
     {
        let card = scratch("changed");
        let backup = scratch("changedbk");
        let rel = "Saves/tau/alfatreze.TAU/tau-meter-settings.bin";
        put(&card, rel, &[1; 100]);
        let plan = plan_delete(&card, rel).unwrap();
        put(&card, rel, &[1; 200]); // the file changed
        assert_eq!(
            execute_delete(&card, rel, &plan.id, &backup)
                .unwrap_err()
                .code,
            ErrorCode::ConfirmationMismatch
        );
        assert!(card.join(rel).exists());
        // even with a matching token, a file of the right size is refused
        put(&card, rel, &[1; FILE_SIZE]);
        let blocked = plan_delete(&card, rel).unwrap();
        assert!(blocked.blocked.is_some());
        assert!(execute_delete(&card, rel, &blocked.id, &backup).is_err());
        assert_eq!(fs::read(card.join(rel)).unwrap().len(), FILE_SIZE);
    }
}
