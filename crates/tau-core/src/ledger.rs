//! The verification ledger (design and red-team: `docs/PERFORMANCE_AUDIT.md`).
//!
//! A **host-side cache, never written to the card**, of what was learned about the files on one card, so
//! that reloading the library, rebuilding the index and re-planning a sync stop re-reading file contents
//! over a slow link. Per media file it can hold:
//!
//! * the tags, duration and format read from it (a scan cache), and
//! * **provenance**: "this card file was produced from source `S` with cover `C` by embed version `V`
//!   and its verified content hash is `H`", which is what lets a re-sync of an album that is already on the
//!   card say "same" instead of rewriting it (an embedded copy never equals its source, so before this every
//!   re-sync rewrote every file).
//!
//! # Rules that keep it safe
//!
//! * An entry is used only if the file's **size and modified time both still match**. Anything else is read
//!   again. One `stat`, no content read.
//! * **Racy timestamps** (the git "racily clean" problem): an entry recorded within 2.5 s of the file's own
//!   modified time cannot be told apart from a later edit in the same tick, so it is not trusted; the file
//!   is read again and the entry refreshed. Provenance written by this tool right after its own verified
//!   write is exempt: its trust comes from the verification, not from timing.
//! * **Untrusted modified times:** if most of a large set of files share one timestamp (or sit at the 1980
//!   epoch), the file system's times say nothing, so nothing is reused for that scan.
//! * **The ledger may influence display and whether to skip a copy. It must never influence deletion,
//!   overwrite or verification.** Those always use fresh reads and hashes. A wrong ledger can therefore
//!   cause a missed update, never data loss. A small canary re-hashes a few "unchanged" files on every plan
//!   to catch drift (see `sync`).
//! * The file carries a version and a CRC; a missing, corrupt or wrong-version file is an empty ledger. It
//!   is written to a temporary name and renamed. Deleting it is always safe.
//! * The host decides *where* it lives and *which card it belongs to* (`register_locator`), because card
//!   identity (a volume UUID) is host knowledge. With no locator, or when the locator cannot identify the
//!   volume, there is no ledger and everything behaves exactly as before.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Map, Value, json};

/// Bump whenever the bytes produced by cover embedding change, so old provenance stops matching.
pub const EMBED_VERSION: u32 = 1;
const MAGIC: &[u8; 8] = b"TAULEDG1";
const FORMAT: u64 = 1;
/// FAT keeps 2 s, exFAT 10 ms; plus a margin. See "Racy timestamps" above.
const RACY_MS: i64 = 2_500;
/// Provenance entries older than this are re-verified by the canary even when nothing looks changed.
pub const CANARY_MAX_AGE_MS: i64 = 30 * 24 * 3_600 * 1_000;

/// Host hook: the ledger file for the card whose media root is given, or `None` for "no ledger".
pub type LedgerLocator = fn(&Path) -> Option<PathBuf>;
static LOCATOR: OnceLock<LedgerLocator> = OnceLock::new();

/// Registers the host's locator (first registration wins).
pub fn register_locator(locator: LedgerLocator) {
    let _ = LOCATOR.set(locator);
}

/// Whether a host has registered a locator at all.
pub fn is_enabled() -> bool {
    LOCATOR.get().is_some()
}

/// Forgets everything learned about the card holding `root` (deleting the file is always safe).
pub fn clear(root: &Path) -> bool {
    LOCATOR
        .get()
        .and_then(|locate| locate(root))
        .is_some_and(|path| fs::remove_file(path).is_ok())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// What a file looked like when something was learned about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint {
    pub size: u64,
    pub mtime_ms: i64,
}

/// A fingerprint from file metadata; `None` when the platform cannot say.
pub fn fingerprint(meta: &fs::Metadata) -> Option<Fingerprint> {
    let modified = meta.modified().ok()?;
    let mtime_ms = match modified.duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_millis()).ok()?,
        Err(e) => -i64::try_from(e.duration().as_millis()).ok()?,
    };
    Some(Fingerprint { size: meta.len(), mtime_ms })
}

/// Tags, duration and format read from a media file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRecord {
    pub tags: BTreeMap<String, String>,
    pub secs: u16,
    pub fmt: u8,
}

/// How a card file came to be, and what it hashed to when it was verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub source_sha: String,
    /// The cover that was embedded; empty when none was.
    pub cover_sha: String,
    pub embed_version: u32,
    /// SHA-256 of the card file itself, from the verified write.
    pub output_sha: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    fp: Fingerprint,
    /// When this entry was recorded (ms). See "Racy timestamps".
    recorded_ms: i64,
    tags: Option<TagRecord>,
    provenance: Option<Provenance>,
    /// When the content was last proven (hashed) rather than assumed.
    verified_ms: i64,
}

impl Record {
    fn trustworthy(&self, fp: &Fingerprint) -> bool {
        self.fp == *fp && self.recorded_ms - self.fp.mtime_ms > RACY_MS
    }
}

/// Counts for one pass, so a UI can say "checked 4,647 files, read 12".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub reused: u64,
    pub read: u64,
}

/// One working session on one card's ledger: open, ask and record, then `finish`.
pub struct Session {
    path: PathBuf,
    files: BTreeMap<String, Record>,
    seen: BTreeSet<String>,
    dirty: bool,
    /// False when the file system's modified times cannot be relied on for this card right now.
    trust: bool,
    pub stats: Stats,
}

impl Session {
    /// Opens the ledger for the card holding `root`, or `None` when there is none to use.
    pub fn open(root: &Path) -> Option<Session> {
        let path = LOCATOR.get().and_then(|locate| locate(root))?;
        let files = load(&path).unwrap_or_default();
        Some(Session { path, files, seen: BTreeSet::new(), dirty: false, trust: true, stats: Stats::default() })
    }

    /// Looks at every file's fingerprint once before any lookup and decides whether this card's modified
    /// times mean anything. A card whose files mostly share one timestamp (or sit at the 1980 epoch) gets
    /// no reuse this time.
    pub fn assess(&mut self, prints: &[Fingerprint]) {
        self.trust = mtimes_are_meaningful(prints);
    }

    /// Cached tags for `rel` if its fingerprint still matches and the entry is not racy. Marks it seen.
    pub fn tags(&mut self, rel: &str, fp: &Fingerprint) -> Option<TagRecord> {
        self.seen.insert(rel.to_string());
        if !self.trust {
            return None;
        }
        let hit = self.files.get(rel).filter(|r| r.trustworthy(fp)).and_then(|r| r.tags.clone());
        if hit.is_some() {
            self.stats.reused += 1;
        }
        hit
    }

    /// Records freshly read tags. Provenance for the same fingerprint is kept; a changed file starts over.
    pub fn put_tags(&mut self, rel: &str, fp: Fingerprint, tags: TagRecord) {
        self.stats.read += 1;
        self.seen.insert(rel.to_string());
        if !self.trust {
            return;
        }
        let now = now_ms();
        match self.files.get_mut(rel) {
            Some(record) if record.fp == fp => {
                record.tags = Some(tags);
                record.recorded_ms = now;
            }
            _ => {
                self.files.insert(rel.to_string(), Record { fp, recorded_ms: now, tags: Some(tags), provenance: None, verified_ms: 0 });
            }
        }
        self.dirty = true;
    }

    /// The provenance recorded for `rel`, if the file is still exactly as it was left.
    pub fn provenance(&self, rel: &str, fp: &Fingerprint) -> Option<(Provenance, i64)> {
        if !self.trust {
            return None;
        }
        let record = self.files.get(rel).filter(|r| r.trustworthy(fp))?;
        let prov = record.provenance.clone().filter(|p| p.embed_version == EMBED_VERSION)?;
        Some((prov, record.verified_ms))
    }

    /// Records that this tool just wrote and **verified** `rel` (its hash was compared after a read-back
    /// that bypasses the cache), and `fp` is the file as it stands after the final rename. Trust comes from
    /// that verification, so the entry is not subject to the racy-timestamp rule.
    pub fn record_verified(&mut self, rel: &str, fp: Fingerprint, provenance: Provenance) {
        let now = now_ms();
        self.files.insert(
            rel.to_string(),
            Record {
                fp,
                recorded_ms: now.max(fp.mtime_ms + RACY_MS + 1),
                tags: None,
                provenance: Some(provenance),
                verified_ms: now,
            },
        );
        self.seen.insert(rel.to_string());
        self.dirty = true;
    }

    /// Drops the provenance of `rel`: the canary found the file is not what the ledger said.
    pub fn invalidate(&mut self, rel: &str) {
        if let Some(record) = self.files.get_mut(rel) {
            record.provenance = None;
            record.verified_ms = 0;
            self.dirty = true;
        }
    }

    /// Marks a file's provenance as freshly re-proven.
    pub fn touch_verified(&mut self, rel: &str) {
        if let Some(record) = self.files.get_mut(rel) {
            record.verified_ms = now_ms();
            self.dirty = true;
        }
    }

    /// Ends a **complete** scan: entries for files that no longer exist are dropped, and the ledger is
    /// saved if anything changed. Failure to save is ignored: it is only a cache.
    pub fn finish_scan(mut self) {
        let before = self.files.len();
        let seen = std::mem::take(&mut self.seen);
        self.files.retain(|rel, _| seen.contains(rel));
        if self.files.len() != before {
            self.dirty = true;
        }
        self.save();
    }

    /// Ends a session that did not cover the whole card (a plan or an execute): saves, drops nothing.
    pub fn finish(self) {
        self.save();
    }

    fn save(&self) {
        if self.dirty {
            let _ = store(&self.path, &self.files);
        }
    }

    /// How many files the ledger knows about (for tests and diagnostics).
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Whether modified times carry information: not when 90 percent or more of at least 50 files share one
/// timestamp, or when most sit at or before 1980-01-02 (the exFAT epoch used for "unknown").
pub fn mtimes_are_meaningful(prints: &[Fingerprint]) -> bool {
    if prints.len() < 50 {
        return true;
    }
    let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
    let mut epoch = 0usize;
    for p in prints {
        *counts.entry(p.mtime_ms).or_default() += 1;
        if p.mtime_ms <= 315_619_200_000 {
            epoch += 1;
        }
    }
    let most = counts.values().copied().max().unwrap_or(0);
    most * 10 < prints.len() * 9 && epoch * 2 < prints.len()
}

fn load(path: &Path) -> Option<BTreeMap<String, Record>> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() < 12 || &bytes[..8] != MAGIC {
        return None;
    }
    let crc = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let body = &bytes[12..];
    if crc32fast::hash(body) != crc {
        return None;
    }
    let root: Value = serde_json::from_slice(body).ok()?;
    if root.get("v")?.as_u64()? != FORMAT {
        return None;
    }
    let mut out = BTreeMap::new();
    for (rel, v) in root.get("files")?.as_object()? {
        let fp = Fingerprint { size: v.get("s")?.as_u64()?, mtime_ms: v.get("m")?.as_i64()? };
        let tags = v.get("t").and_then(Value::as_object).map(|t| TagRecord {
            tags: t.iter().filter_map(|(k, x)| Some((k.clone(), x.as_str()?.to_string()))).collect(),
            secs: v.get("d").and_then(Value::as_u64).unwrap_or(0).min(u64::from(u16::MAX)) as u16,
            fmt: v.get("f").and_then(Value::as_u64).unwrap_or(0).min(255) as u8,
        });
        let provenance = v.get("p").and_then(|p| {
            Some(Provenance {
                source_sha: p.get("ss")?.as_str()?.to_string(),
                cover_sha: p.get("cs")?.as_str()?.to_string(),
                embed_version: u32::try_from(p.get("ev")?.as_u64()?).ok()?,
                output_sha: p.get("os")?.as_str()?.to_string(),
            })
        });
        out.insert(
            rel.clone(),
            Record {
                fp,
                recorded_ms: v.get("r").and_then(Value::as_i64).unwrap_or(0),
                tags,
                provenance,
                verified_ms: v.get("vm").and_then(Value::as_i64).unwrap_or(0),
            },
        );
    }
    Some(out)
}

fn store(path: &Path, files: &BTreeMap<String, Record>) -> std::io::Result<()> {
    let mut map = Map::new();
    for (rel, r) in files {
        let mut o = Map::new();
        o.insert("s".into(), json!(r.fp.size));
        o.insert("m".into(), json!(r.fp.mtime_ms));
        o.insert("r".into(), json!(r.recorded_ms));
        o.insert("vm".into(), json!(r.verified_ms));
        if let Some(t) = &r.tags {
            o.insert("t".into(), Value::Object(t.tags.iter().map(|(k, v)| (k.clone(), json!(v))).collect()));
            o.insert("d".into(), json!(t.secs));
            o.insert("f".into(), json!(t.fmt));
        }
        if let Some(p) = &r.provenance {
            o.insert("p".into(), json!({ "ss": p.source_sha, "cs": p.cover_sha, "ev": p.embed_version, "os": p.output_sha }));
        }
        map.insert(rel.clone(), Value::Object(o));
    }
    let body = serde_json::to_vec(&json!({ "v": FORMAT, "files": Value::Object(map) }))?;
    let mut bytes = Vec::with_capacity(12 + body.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&crc32fast::hash(&body).to_le_bytes());
    bytes.extend_from_slice(&body);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    fs::write(&temp, &bytes)?;
    fs::rename(&temp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(size: u64, mtime_ms: i64) -> Fingerprint {
        Fingerprint { size, mtime_ms }
    }
    fn tags(title: &str) -> TagRecord {
        TagRecord { tags: BTreeMap::from([("TIT2".to_string(), title.to_string())]), secs: 180, fmt: 1 }
    }
    fn session(name: &str) -> Session {
        let dir = std::env::temp_dir().join(format!("tau-ledger-unit-{name}-{}", std::process::id() as u128 + crate::test_uniq()));
        fs::create_dir_all(&dir).unwrap();
        Session { path: dir.join("l.bin"), files: BTreeMap::new(), seen: BTreeSet::new(), dirty: false, trust: true, stats: Stats::default() }
    }
    const OLD: i64 = 1_700_000_000_000;

    #[test]
    fn an_entry_is_reused_only_while_size_and_time_both_match() {
        let mut s = session("match");
        s.put_tags("a.mp3", fp(100, OLD), tags("A"));
        s.files.get_mut("a.mp3").unwrap().recorded_ms = OLD + 60_000; // recorded long after the file changed
        assert_eq!(s.tags("a.mp3", &fp(100, OLD)), Some(tags("A")));
        assert_eq!(s.tags("a.mp3", &fp(101, OLD)), None, "size differs");
        assert_eq!(s.tags("a.mp3", &fp(100, OLD + 10)), None, "time differs");
        assert_eq!(s.tags("missing.mp3", &fp(100, OLD)), None);
    }

    #[test]
    fn an_entry_recorded_within_a_timestamp_tick_of_the_file_is_not_trusted() {
        let mut s = session("racy");
        s.put_tags("a.mp3", fp(100, OLD), tags("A"));
        s.files.get_mut("a.mp3").unwrap().recorded_ms = OLD + 1_000; // within 2.5 s of the file's own time
        assert_eq!(s.tags("a.mp3", &fp(100, OLD)), None, "racy: a later edit in the same tick would look identical");
        s.files.get_mut("a.mp3").unwrap().recorded_ms = OLD + 2_501;
        assert!(s.tags("a.mp3", &fp(100, OLD)).is_some());
    }

    #[test]
    fn verified_provenance_is_trusted_straight_away_but_only_for_the_same_embed_version() {
        let mut s = session("prov");
        let prov = Provenance { source_sha: "s".into(), cover_sha: "c".into(), embed_version: EMBED_VERSION, output_sha: "o".into() };
        s.record_verified("a.mp3", fp(100, OLD), prov.clone());
        assert_eq!(s.provenance("a.mp3", &fp(100, OLD)).map(|p| p.0), Some(prov.clone()), "no waiting out the racy window after our own verified write");
        assert!(s.provenance("a.mp3", &fp(100, OLD + 1)).is_none(), "the file changed");
        s.files.get_mut("a.mp3").unwrap().provenance.as_mut().unwrap().embed_version = EMBED_VERSION + 1;
        assert!(s.provenance("a.mp3", &fp(100, OLD)).is_none(), "a different embed algorithm invalidates it");
        s.invalidate("a.mp3");
        assert!(s.files["a.mp3"].provenance.is_none());
    }

    #[test]
    fn a_card_whose_times_say_nothing_gets_no_reuse() {
        let same: Vec<_> = (0..100).map(|n| fp(n, OLD)).collect();
        assert!(!mtimes_are_meaningful(&same), "all one timestamp");
        let epoch: Vec<_> = (0..100).map(|n| fp(n, 315_532_800_000)).collect();
        assert!(!mtimes_are_meaningful(&epoch), "all at the 1980 epoch");
        let real: Vec<_> = (0..100).map(|n| fp(n, OLD + n as i64 * 1_234)).collect();
        assert!(mtimes_are_meaningful(&real));
        assert!(mtimes_are_meaningful(&same[..10]), "too few files to judge");
        let mut s = session("untrusted");
        s.assess(&same);
        s.put_tags("a.mp3", fp(1, OLD), tags("A"));
        assert!(s.is_empty(), "nothing is stored while the times are untrusted");
        assert_eq!(s.tags("a.mp3", &fp(1, OLD)), None);
    }

    #[test]
    fn the_file_round_trips_and_a_damaged_one_is_an_empty_ledger() {
        let mut s = session("file");
        s.put_tags("Artist/Album/01.mp3", fp(100, OLD), tags("Straße"));
        s.record_verified("Artist/Album/02.mp3", fp(200, OLD), Provenance { source_sha: "s".into(), cover_sha: String::new(), embed_version: EMBED_VERSION, output_sha: "o".into() });
        store(&s.path, &s.files).unwrap();
        let loaded = load(&s.path).unwrap();
        assert_eq!(loaded, s.files);
        // flip one byte in the body: the CRC no longer matches
        let mut bytes = fs::read(&s.path).unwrap();
        let last = bytes.len() - 3;
        bytes[last] ^= 0x55;
        fs::write(&s.path, &bytes).unwrap();
        assert!(load(&s.path).is_none());
        // wrong magic, truncated, wrong version
        fs::write(&s.path, b"nope").unwrap();
        assert!(load(&s.path).is_none());
        let body = serde_json::to_vec(&json!({ "v": 99, "files": {} })).unwrap();
        let mut future = MAGIC.to_vec();
        future.extend(crc32fast::hash(&body).to_le_bytes());
        future.extend(body);
        fs::write(&s.path, future).unwrap();
        assert!(load(&s.path).is_none(), "a version this build does not know is ignored, not guessed at");
    }

    #[test]
    fn finishing_a_complete_scan_forgets_files_that_are_gone() {
        let mut s = session("gc");
        s.put_tags("keep.mp3", fp(1, OLD), tags("K"));
        s.put_tags("gone.mp3", fp(2, OLD), tags("G"));
        s.seen.clear();
        s.seen.insert("keep.mp3".into());
        let path = s.path.clone();
        s.finish_scan();
        let loaded = load(&path).unwrap();
        assert!(loaded.contains_key("keep.mp3") && !loaded.contains_key("gone.mp3"));
    }
}
