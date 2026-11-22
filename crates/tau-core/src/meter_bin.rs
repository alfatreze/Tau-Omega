//! The meter bin, version 2 (`Assets/tau/common/tau-meters.bin`, a `TPKB` file): reading and checking one. **Read side only**: nothing here
//! composes a bin or touches a card.
//!
//! Written from Tau Alpha's specification (`docs/features/meters/EXTERNAL_METERS_SPEC.md` sections 2, 3 and 7) and checked against their golden
//! vectors (`testdata/meter_bin/`, `tests/meter_bin.rs`). The checks run in the same order as their reference reader (`tools/meter_bin.py`)
//! and the firmware's (`fw2/cores/meterbin_core.h`), so a damaged bin gets the same refusal code here as there.
//!
//! Three levels, as in the firmware:
//! - [`open_toc`]: what the Pocket's reader does at boot ([`boot_check`] adds the firmware's rule that a fallback row must exist): the 40-byte header, then the table of contents and the previews (bytes `0..packs_off`), the
//!   `toc_crc`, and every row. No pack is read.
//! - [`check_pack`]: one pack's own header against its row (the Pocket does this when the meter is selected).
//! - [`parse`]: the full check Omega runs after a write: `open_toc`, then the whole-file CRC, then every pack's header against its row.
//!   A version 1 bundle (`tau-packs.bin` as old cores shipped it) is mapped to the same rows, to show an old card; the firmware reads v2 only.

use crate::{ErrorCode, TauError};

/// The v2 header, a table-of-contents row, the most meters in one bin, and the largest bin the firmware accepts (4.25 MiB).
pub const HEADER_LEN: usize = 40;
pub const ROW_LEN: usize = 256;
pub const MAX_METERS: usize = 64;
pub const MAX_BIN: usize = 4_456_448;
/// Where a pack runs: the 64 KiB code window in the Pocket's memory. Every pack in a v2 bin is linked for it.
pub const WINDOW: u32 = 0x2497_0000;
pub const WINDOW_SIZE: u32 = 0x1_0000;
/// The pack ABI this Omega understands (major 1, minor 2), as the firmware it was written against.
pub const SUPPORTED_ABI: u16 = 0x0102;

const MAX_PREVIEW: usize = 1024;
const PALETTE: usize = 64;
const PIXELS: u32 = 56 * 32;
const MAX_PARAMS: u16 = 48;
const MAX_NAME: usize = 23;
const PACK_HEADER: usize = 48;
const V1_HASH_BASE: u32 = 0x5631_0000;

pub const FLAG_VISIBLE: u16 = 1;
/// The built-in fallback meter (Winamp Bars): it cannot be hidden or removed.
pub const FLAG_FALLBACK: u16 = 2;
pub const FLAG_BINARY_ONLY: u16 = 4;
/// Set by this reader on a row mapped from a v1 bundle (its id is made up from the pack's number).
pub const FLAG_V1: u16 = 8;

/// Why a bin is refused: one code (the first check that fails), the same numbers as Tau Alpha's reader and the Pocket's `MTOC ERR n` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Refusal {
    Size = 1,
    Magic = 2,
    Version = 3,
    Crc = 4,
    Count = 5,
    Layout = 6,
    Id = 7,
    Hash = 8,
    Dup = 9,
    Name = 10,
    Pack = 11,
    Window = 12,
    Preview = 13,
    Reserved = 14,
    Origin = 15,
    Params = 16,
    Read = 17,
    Desc = 18,
    FileCrc = 19,
    /// Not a reader check: the firmware's own rule at boot. A bin with no built-in fallback row is refused whole and the core keeps its own packs.
    NoFallback = 20,
}

impl Refusal {
    pub fn code(self) -> u8 {
        self as u8
    }
    /// Tau Alpha's short name for the code (`SIZE`, `MAGIC`, ...).
    pub fn name(self) -> &'static str {
        match self {
            Refusal::Size => "SIZE",
            Refusal::Magic => "MAGIC",
            Refusal::Version => "VERSION",
            Refusal::Crc => "CRC",
            Refusal::Count => "COUNT",
            Refusal::Layout => "LAYOUT",
            Refusal::Id => "ID",
            Refusal::Hash => "HASH",
            Refusal::Dup => "DUP",
            Refusal::Name => "NAME",
            Refusal::Pack => "PACK",
            Refusal::Window => "WINDOW",
            Refusal::Preview => "PREVIEW",
            Refusal::Reserved => "RESERVED",
            Refusal::Origin => "ORIGIN",
            Refusal::Params => "PARAMS",
            Refusal::Read => "READ",
            Refusal::Desc => "DESC",
            Refusal::FileCrc => "FILECRC",
            Refusal::NoFallback => "NOFALLBACK",
        }
    }
    /// The reason in plain words, for a screen.
    pub fn plain(self) -> &'static str {
        match self {
            Refusal::Size => "The meter file is the wrong size, or cut short.",
            Refusal::Magic => "This is not a meter file.",
            Refusal::Version => "This meter file is a version this Omega does not read.",
            Refusal::Crc => "The list of meters in the file is damaged.",
            Refusal::Count => "The file holds more meters than a Pocket can list (64).",
            Refusal::Layout => "The meter file is laid out in a way this Omega does not recognise.",
            Refusal::Id => "A meter in the file has an invalid id.",
            Refusal::Hash => "A meter's short id does not match its id.",
            Refusal::Dup => "Two meters in the file share the same short id.",
            Refusal::Name => "A meter in the file has a name the Pocket cannot show.",
            Refusal::Pack => "A meter's code is damaged or does not match its entry.",
            Refusal::Window => "A meter is too big for the Pocket's meter memory.",
            Refusal::Preview => "A meter's preview picture is damaged.",
            Refusal::Reserved => "The file uses a part of the format that is not defined yet.",
            Refusal::Origin => "A meter was built for a different place in memory.",
            Refusal::Params => "A meter claims more settings than a Pocket can hold (48).",
            Refusal::Read => "The meter file could not be read.",
            Refusal::Desc => "A meter's description block is damaged.",
            Refusal::FileCrc => {
                "The meter file was changed after it was written (its checksum is wrong)."
            }
            Refusal::NoFallback => {
                "The meter file has no built-in Winamp Bars entry, so the Pocket ignores the whole file."
            }
        }
    }
}

/// A refused bin: the code, the row it was about (when it was about one) and a short technical detail.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BinError {
    pub refusal: Refusal,
    pub row: Option<usize>,
    pub detail: String,
}

impl BinError {
    fn new(refusal: Refusal) -> Self {
        BinError {
            refusal,
            row: None,
            detail: String::new(),
        }
    }
    fn at(refusal: Refusal, row: usize) -> Self {
        BinError {
            refusal,
            row: Some(row),
            detail: String::new(),
        }
    }
    fn why(mut self, detail: &str) -> Self {
        self.detail = detail.to_string();
        self
    }
    /// The reason in plain words, naming the meter's position when there is one (1-based, as a person counts).
    pub fn plain(&self) -> String {
        match self.row {
            Some(r) => format!(
                "{} (meter {}.)",
                self.refusal.plain().trim_end_matches('.'),
                r + 1
            ),
            None => self.refusal.plain().to_string(),
        }
    }
}

impl std::fmt::Display for BinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.refusal.name())?;
        if let Some(r) = self.row {
            write!(f, " (row {r})")?;
        }
        if !self.detail.is_empty() {
            write!(f, ": {}", self.detail)?;
        }
        Ok(())
    }
}

impl std::error::Error for BinError {}

impl From<BinError> for TauError {
    fn from(e: BinError) -> Self {
        TauError::e(ErrorCode::InvalidPathReference, e.plain())
    }
}

/// One meter: a row of the table of contents, as the firmware reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeterRow {
    /// The first 32 bits of the id (its first 8 hex digits read as a number): the key of the saved choice, the tunings and the preset folder.
    pub id_hash: u32,
    /// 32 lower-case hex digits (empty on a row mapped from a v1 bundle).
    pub id: String,
    pub name: String,
    pub version: (u16, u16, u16),
    /// The pack ABI the meter needs, `major << 8 | minor`.
    pub abi: u16,
    /// The oldest TAU that can run it, `major << 8 | minor` (0 = any).
    pub tau_min: u16,
    pub flags: u16,
    pub params_rev: u16,
    pub nparams: u16,
    pub cmds: u16,
    pub max_cycles: u32,
    pub pack_off: u32,
    pub pack_size: u32,
    pub pack_crc: u32,
    pub pstate: u32,
    pub bss: u32,
    pub preview_off: u32,
    pub preview_len: u32,
}

impl MeterRow {
    pub fn visible(&self) -> bool {
        self.flags & FLAG_VISIBLE != 0
    }
    /// The built-in Winamp Bars row.
    pub fn fallback(&self) -> bool {
        self.flags & FLAG_FALLBACK != 0
    }
    pub fn binary_only(&self) -> bool {
        self.flags & FLAG_BINARY_ONLY != 0
    }
    /// A row mapped from a v1 bundle (no real id, no version).
    pub fn from_v1(&self) -> bool {
        self.flags & FLAG_V1 != 0
    }
    /// The preview block (palette plus run-length picture) inside `bin`, or `None` when the range is not inside it.
    pub fn preview<'a>(&self, bin: &'a [u8]) -> Option<&'a [u8]> {
        let (from, len) = (self.preview_off as usize, self.preview_len as usize);
        bin.get(from..from.checked_add(len)?)
    }
}

/// What a bin holds: the rows in display order (the table of contents is the order).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeterBin {
    /// 2, or 1 for a bundle mapped for inspection.
    pub version: u16,
    pub bytes: usize,
    pub rows: Vec<MeterRow>,
}

impl MeterBin {
    /// The built-in fallback row (Winamp Bars, flag bit 1), if the bin has one. The Pocket refuses a bin without one (refusal 20,
    /// [`boot_check`]); the reader's own checks do not, as Tau Alpha's vectors say, so a composer must always include it.
    pub fn fallback(&self) -> Option<&MeterRow> {
        self.rows.iter().find(|r| r.fallback())
    }
}

/// Whether a meter can run on a given firmware: the list greys a row that cannot, and the bin is not refused for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Availability {
    Available,
    /// Needs another ABI major, or a minor above the firmware's.
    NeedsOtherAbi,
    /// Needs a newer TAU than the one on the card.
    NeedsNewerTau,
}

pub fn availability(row: &MeterRow, firmware_abi: u16, tau: u16) -> Availability {
    if row.abi >> 8 != firmware_abi >> 8 || row.abi & 255 > firmware_abi & 255 {
        Availability::NeedsOtherAbi
    } else if row.tau_min > tau {
        Availability::NeedsNewerTau
    } else {
        Availability::Available
    }
}

/// One flag per row: true when another row has the same low 16 bits of `id_hash`. A tuning is keyed by those bits, so the Pocket starts such
/// a meter from its defaults and never saves its tuning (the meter still runs); the composer tells the user.
pub fn tuning_collisions(rows: &[MeterRow]) -> Vec<bool> {
    rows.iter()
        .map(|r| {
            rows.iter()
                .filter(|o| o.id_hash & 0xFFFF == r.id_hash & 0xFFFF)
                .count()
                > 1
        })
        .collect()
}

fn u16at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u32at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
fn crc(b: &[u8]) -> u32 {
    crc32fast::hash(b)
}

fn valid_id(raw: &[u8]) -> bool {
    raw.len() == 32 && raw.iter().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

/// A name field: 1 to 23 printable ASCII characters, a NUL after them, and only NULs after that.
fn valid_name(raw: &[u8]) -> bool {
    match raw.iter().position(|&c| c == 0) {
        None => false,
        Some(n) => {
            (1..=MAX_NAME).contains(&n)
                && raw[..n].iter().all(|c| (0x20..=0x7E).contains(c))
                && raw[n..].iter().all(|&c| c == 0)
        }
    }
}

/// A preview block: a 64-byte palette and a run-length stream that covers exactly 56 x 32 pixels, 65 to 1024 bytes in all.
fn preview_ok(blk: &[u8]) -> bool {
    blk.len() > PALETTE
        && blk.len() <= MAX_PREVIEW
        && blk[PALETTE..]
            .iter()
            .map(|&c| u32::from(c & 31) + 1)
            .sum::<u32>()
            == PIXELS
}

/// The pack fits the window with its saved state (`size` bytes of image, state at `ps_off` for `ps_size` bytes).
fn window_ok(size: u32, ps_off: u32, ps_size: u32) -> bool {
    size > 0
        && size <= WINDOW_SIZE
        && ps_off >= size
        && ps_off <= WINDOW_SIZE
        && ps_size <= WINDOW_SIZE - ps_off
}

fn id_hash_of(id: &[u8]) -> u32 {
    id[..8].iter().fold(0u32, |n, &c| {
        n * 16 + (char::from(c).to_digit(16).unwrap_or(0))
    })
}

struct Header {
    count: usize,
    pv_off: u32,
    packs_off: u32,
    total: u32,
}

fn toc_header(b: &[u8], file_len: usize) -> Result<Header, BinError> {
    if file_len < 8 || b.len() < 8 {
        return Err(BinError::new(Refusal::Size));
    }
    if &b[..4] != b"TPKB" {
        return Err(BinError::new(Refusal::Magic));
    }
    if u16at(b, 4) != 2 {
        return Err(BinError::new(Refusal::Version));
    }
    if !(HEADER_LEN + 4..=MAX_BIN).contains(&file_len) || b.len() < HEADER_LEN {
        return Err(BinError::new(Refusal::Size));
    }
    let count = usize::from(u16at(b, 6));
    if count > MAX_METERS {
        return Err(BinError::new(Refusal::Count));
    }
    let (toc_off, toc_len, pv_off, pv_len, packs_off, total, toc_crc, reserved) = (
        u32at(b, 8),
        u32at(b, 12),
        u32at(b, 16),
        u32at(b, 20),
        u32at(b, 24),
        u32at(b, 28),
        u32at(b, 32),
        u32at(b, 36),
    );
    if total as usize != file_len {
        return Err(
            BinError::new(Refusal::Size).why(&format!("total size {total}, file {file_len}"))
        );
    }
    let toc = (ROW_LEN * count) as u64;
    if u64::from(toc_off) != HEADER_LEN as u64
        || u64::from(toc_len) != toc
        || u64::from(pv_off) != HEADER_LEN as u64 + toc
        || u64::from(packs_off) != u64::from(pv_off) + u64::from(pv_len)
        || packs_off & 3 != 0
        || u64::from(packs_off) + 4 > u64::from(total)
    {
        return Err(BinError::new(Refusal::Layout));
    }
    if reserved != 0 {
        return Err(BinError::new(Refusal::Reserved));
    }
    if b.len() < packs_off as usize {
        return Err(
            BinError::new(Refusal::Size).why("the table of contents and previews are cut short")
        );
    }
    if crc(&b[HEADER_LEN..packs_off as usize]) != toc_crc {
        return Err(BinError::new(Refusal::Crc));
    }
    Ok(Header {
        count,
        pv_off,
        packs_off,
        total,
    })
}

fn row_v2(b: &[u8], i: usize, h: &Header) -> Result<MeterRow, BinError> {
    let o = HEADER_LEN + ROW_LEN * i;
    let r = &b[o..o + ROW_LEN];
    let id = &r[4..44];
    if !valid_id(&id[..32]) || id[32..].iter().any(|&c| c != 0) {
        return Err(BinError::at(Refusal::Id, i));
    }
    if u32at(r, 0) != id_hash_of(id) {
        return Err(BinError::at(Refusal::Hash, i));
    }
    if !valid_name(&r[44..68]) {
        return Err(BinError::at(Refusal::Name, i));
    }
    let flags = u16at(r, 78);
    if flags & !7 != 0 || u16at(r, 86) != 0 || r[120..].iter().any(|&c| c != 0) {
        return Err(BinError::at(Refusal::Reserved, i));
    }
    let nparams = u16at(r, 82);
    if nparams > MAX_PARAMS {
        return Err(BinError::at(Refusal::Params, i));
    }
    let (pack_off, pack_size, pack_crc, pstate, bss, prev_off, prev_len) = (
        u32at(r, 92),
        u32at(r, 96),
        u32at(r, 100),
        u32at(r, 104),
        u32at(r, 108),
        u32at(r, 112),
        u32at(r, 116),
    );
    let end = h.total - 4;
    if (pack_size as usize) < PACK_HEADER
        || pack_off & 3 != 0
        || pack_off < h.packs_off
        || pack_off > end
        || pack_size > end - pack_off
    {
        return Err(BinError::at(Refusal::Pack, i).why("range"));
    }
    let image = u64::from(pack_size) - PACK_HEADER as u64;
    if image > u64::from(WINDOW_SIZE)
        || u64::from(pstate) > u64::from(WINDOW_SIZE)
        || image + u64::from(pstate) > u64::from(WINDOW_SIZE)
    {
        return Err(BinError::at(Refusal::Window, i));
    }
    if prev_off < h.pv_off
        || prev_off > h.packs_off
        || prev_len > h.packs_off - prev_off
        || !preview_ok(&b[prev_off as usize..(prev_off + prev_len) as usize])
    {
        return Err(BinError::at(Refusal::Preview, i));
    }
    Ok(MeterRow {
        id_hash: u32at(r, 0),
        id: String::from_utf8_lossy(&id[..32]).into_owned(),
        name: String::from_utf8_lossy(
            &r[44..44 + r[44..68].iter().position(|&c| c == 0).unwrap_or(0)],
        )
        .into_owned(),
        version: (u16at(r, 68), u16at(r, 70), u16at(r, 72)),
        abi: u16at(r, 74),
        tau_min: u16at(r, 76),
        flags,
        params_rev: u16at(r, 80),
        nparams,
        cmds: u16at(r, 84),
        max_cycles: u32at(r, 88),
        pack_off,
        pack_size,
        pack_crc,
        pstate,
        bss,
        preview_off: prev_off,
        preview_len: prev_len,
    })
}

fn dups(rows: &[MeterRow]) -> Result<(), BinError> {
    for (i, r) in rows.iter().enumerate() {
        if rows[..i].iter().any(|o| o.id_hash == r.id_hash) {
            return Err(BinError::at(Refusal::Dup, i));
        }
    }
    Ok(())
}

/// What the Pocket does at boot: the header, then the table of contents and the previews, `toc_crc`, and every row. No pack is read, so `b`
/// may end at `packs_off`; `file_len` is the file's real size. Used by tools to answer "would the Pocket accept this at boot".
pub fn open_toc(b: &[u8], file_len: usize) -> Result<MeterBin, BinError> {
    let h = toc_header(b, file_len)?;
    let rows = (0..h.count)
        .map(|i| row_v2(b, i, &h))
        .collect::<Result<Vec<_>, _>>()?;
    dups(&rows)?;
    Ok(MeterBin {
        version: 2,
        bytes: file_len,
        rows,
    })
}

/// What the Pocket's firmware answers at boot for a user bin: the boot read ([`open_toc`]), then its own rule that a fallback row must exist
/// (refusal 20, [`Refusal::NoFallback`]). `Ok` means the Pocket will use the file. An empty bin has no fallback row, so it is refused too (the
/// core then keeps its own `tau-packs.bin`).
pub fn boot_check(b: &[u8], file_len: usize) -> Result<MeterBin, BinError> {
    let bin = open_toc(b, file_len)?;
    if bin.fallback().is_none() {
        return Err(BinError::new(Refusal::NoFallback));
    }
    Ok(bin)
}

/// One pack's own header against its row: magic, size, ABI, CRC, saved-state and bss sizes as the row says, the exact window fit and the
/// window origin. The Pocket does this when the meter is selected, never at boot.
pub fn check_pack(b: &[u8], row: &MeterRow, i: usize) -> Result<(), BinError> {
    let at = row.pack_off as usize;
    let Some(ph) = b.get(at..at + PACK_HEADER) else {
        return Err(BinError::at(Refusal::Pack, i).why("magic"));
    };
    if &ph[..4] != b"TMPK" {
        return Err(BinError::at(Refusal::Pack, i).why("magic"));
    }
    let (abi, size, origin, pack_crc, bss, ps_off, ps_size) = (
        u16at(ph, 4),
        u32at(ph, 8),
        u32at(ph, 12),
        u32at(ph, 20),
        u32at(ph, 36),
        u32at(ph, 40),
        u32at(ph, 44),
    );
    if u64::from(size) + PACK_HEADER as u64 != u64::from(row.pack_size)
        || abi != row.abi
        || pack_crc != row.pack_crc
        || ps_size != row.pstate
        || bss != row.bss
    {
        return Err(BinError::at(Refusal::Pack, i).why("TOC and pack header disagree"));
    }
    if !window_ok(size, ps_off, ps_size) {
        return Err(BinError::at(Refusal::Window, i));
    }
    if origin != WINDOW {
        return Err(BinError::at(Refusal::Origin, i));
    }
    Ok(())
}

/// The check the loader makes while it copies a pack into the window: the image bytes match the row's `pack_crc`.
pub fn pack_image_ok(b: &[u8], row: &MeterRow) -> bool {
    let from = row.pack_off as usize + PACK_HEADER;
    row.pack_size as usize >= PACK_HEADER
        && b.get(from..row.pack_off as usize + row.pack_size as usize)
            .is_some_and(|image| crc(image) == row.pack_crc)
}

/// `open_toc`, then the whole-file CRC, then every pack's header against its row: the full check (Omega after a write, tools).
fn check_file(b: &[u8]) -> Result<MeterBin, BinError> {
    let bin = open_toc(b, b.len())?;
    if crc(&b[HEADER_LEN..b.len() - 4]) != u32at(b, b.len() - 4) {
        return Err(BinError::new(Refusal::FileCrc));
    }
    for (i, row) in bin.rows.iter().enumerate() {
        check_pack(b, row, i)?;
    }
    Ok(bin)
}

struct Desc {
    nparams: u16,
    cmds: u16,
    max_cycles: u32,
    name: String,
    tot: usize,
    pal_o: usize,
    preview_len: usize,
}

/// The descriptor a pack's image ends with (only what a row needs). Used to map a v1 bundle, whose rows have no table of contents.
fn desc_info(image: &[u8]) -> Result<Desc, BinError> {
    let n = image.len();
    if n < 48 {
        return Err(BinError::new(Refusal::Desc).why("no room for a descriptor"));
    }
    let (tot, magic) = (u32at(image, n - 8) as usize, u32at(image, n - 4));
    if magic != 0x3143_5344 || tot < 48 || tot > n || tot > 4096 || tot & 3 != 0 {
        return Err(BinError::new(Refusal::Desc).why("trailer"));
    }
    let d = &image[n - tot..];
    if u32at(d, 0) != 0x4353_444D || u16at(d, 4) != 3 || usize::from(u16at(d, 6)) != tot {
        return Err(BinError::new(Refusal::Desc).why("header"));
    }
    let nparams = u16::from(d[8]);
    if nparams > MAX_PARAMS {
        return Err(BinError::new(Refusal::Params));
    }
    let (name_o, pal_o, rle_o, rle_n) = (
        usize::from(u16at(d, 20)),
        usize::from(u16at(d, 26)),
        usize::from(u16at(d, 28)),
        usize::from(u16at(d, 30)),
    );
    let end = tot - 8;
    if name_o >= end || pal_o < 40 || pal_o + PALETTE > end || rle_o + rle_n > end {
        return Err(BinError::new(Refusal::Desc).why("offsets"));
    }
    let Some(z) = d[name_o..end]
        .iter()
        .position(|&c| c == 0)
        .map(|p| p + name_o)
    else {
        return Err(BinError::new(Refusal::Name));
    };
    if !valid_name(&d[name_o..=z]) {
        return Err(BinError::new(Refusal::Name));
    }
    if rle_o != pal_o + PALETTE {
        return Err(BinError::new(Refusal::Preview).why("palette and stream not contiguous"));
    }
    Ok(Desc {
        nparams,
        cmds: u16at(d, 10),
        max_cycles: u32at(d, 16),
        name: String::from_utf8_lossy(&d[name_o..z]).into_owned(),
        tot,
        pal_o,
        preview_len: PALETTE + rle_n,
    })
}

/// A v1 bundle (`TPKB` version 1: an 8-byte header, packs back to back, no outer checksum), mapped to rows. For showing an old card only.
fn parse_v1(b: &[u8]) -> Result<MeterBin, BinError> {
    if b.len() > MAX_BIN {
        return Err(BinError::new(Refusal::Size));
    }
    let count = usize::from(u16at(b, 6));
    if count > MAX_METERS {
        return Err(BinError::new(Refusal::Count));
    }
    let (mut off, mut offs) = (8usize, Vec::new());
    for i in 0..count {
        if off + PACK_HEADER > b.len() {
            return Err(BinError::at(Refusal::Size, i));
        }
        if &b[off..off + 4] != b"TMPK" {
            return Err(BinError::at(Refusal::Pack, i));
        }
        let size = u32at(b, off + 8) as usize;
        if size == 0 || off + PACK_HEADER + size > b.len() {
            return Err(BinError::at(Refusal::Size, i));
        }
        offs.push(off);
        off += PACK_HEADER + size;
    }
    if off != b.len() {
        return Err(BinError::new(Refusal::Size)
            .why(&format!("{} bytes after the last pack", b.len() - off)));
    }
    let mut rows = Vec::with_capacity(count);
    for (i, &off) in offs.iter().enumerate() {
        let size = u32at(b, off + 8) as usize;
        let (abi, mid, crc_v, bss, ps_off, ps_size) = (
            u16at(b, off + 4),
            u16at(b, off + 6),
            u32at(b, off + 20),
            u32at(b, off + 36),
            u32at(b, off + 40),
            u32at(b, off + 44),
        );
        let image = &b[off + PACK_HEADER..off + PACK_HEADER + size];
        let di = desc_info(image).map_err(|mut e| {
            e.row = Some(i);
            e
        })?;
        if !window_ok(size as u32, ps_off, ps_size) {
            return Err(BinError::at(Refusal::Window, i));
        }
        let preview_off = off + PACK_HEADER + size - di.tot + di.pal_o;
        if !preview_ok(&b[preview_off..preview_off + di.preview_len]) {
            return Err(BinError::at(Refusal::Preview, i));
        }
        rows.push(MeterRow {
            id_hash: V1_HASH_BASE | u32::from(mid),
            id: String::new(),
            name: di.name,
            version: (0, 0, 0),
            abi,
            tau_min: 0,
            flags: FLAG_VISIBLE | FLAG_V1 | if mid == 0 { FLAG_FALLBACK } else { 0 },
            params_rev: 0,
            nparams: di.nparams,
            cmds: di.cmds,
            max_cycles: di.max_cycles,
            pack_off: off as u32,
            pack_size: (PACK_HEADER + size) as u32,
            pack_crc: crc_v,
            pstate: ps_size,
            bss,
            preview_off: preview_off as u32,
            preview_len: di.preview_len as u32,
        });
    }
    dups(&rows)?;
    Ok(MeterBin {
        version: 1,
        bytes: b.len(),
        rows,
    })
}

/// The full check of a v2 bin; a v1 bundle is mapped to rows (inspection only: the firmware reads v2 alone). Never panics, whatever the bytes.
pub fn parse(b: &[u8]) -> Result<MeterBin, BinError> {
    if b.len() < 8 {
        return Err(BinError::new(Refusal::Size));
    }
    if &b[..4] != b"TPKB" {
        return Err(BinError::new(Refusal::Magic));
    }
    match u16at(b, 4) {
        2 => check_file(b),
        1 => parse_v1(b),
        _ => Err(BinError::new(Refusal::Version)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_rule_one_to_twenty_three_printable_characters_then_nuls() {
        let field = |s: &[u8]| {
            let mut f = [0u8; 24];
            f[..s.len()].copy_from_slice(s);
            f
        };
        assert!(valid_name(&field(b"WINAMP BARS")));
        assert!(valid_name(&field(&[b'x'; 23])));
        assert!(!valid_name(&field(b"")));
        assert!(!valid_name(&[b'x'; 24]), "24 characters leave no NUL");
        assert!(!valid_name(&field(b"tab\there")));
        assert!(!valid_name(&field("caf\u{e9}".as_bytes())));
        let mut junk = field(b"ab");
        junk[10] = b'z';
        assert!(!valid_name(&junk), "something after the NUL");
    }

    #[test]
    fn ids_are_exactly_32_lower_case_hex_digits_and_the_short_id_is_the_first_eight() {
        assert!(valid_id(b"7dddfe63121e904843088c075cc28ce9"));
        assert!(!valid_id(b"7DDDFE63121e904843088c075cc28ce9"));
        assert!(!valid_id(b"7dddfe63121e904843088c075cc28ce"));
        assert_eq!(id_hash_of(b"7dddfe63121e904843088c075cc28ce9"), 0x7ddd_fe63);
    }

    #[test]
    fn availability_greys_a_meter_for_another_abi_or_a_newer_tau() {
        let row = |abi, tau_min| MeterRow {
            id_hash: 1,
            id: String::new(),
            name: String::new(),
            version: (0, 0, 0),
            abi,
            tau_min,
            flags: 1,
            params_rev: 0,
            nparams: 0,
            cmds: 0,
            max_cycles: 0,
            pack_off: 0,
            pack_size: 0,
            pack_crc: 0,
            pstate: 0,
            bss: 0,
            preview_off: 0,
            preview_len: 0,
        };
        assert_eq!(
            availability(&row(0x0102, 0), 0x0102, 8),
            Availability::Available
        );
        assert_eq!(
            availability(&row(0x0100, 0), 0x0102, 8),
            Availability::Available,
            "an older minor still runs"
        );
        assert_eq!(
            availability(&row(0x0103, 0), 0x0102, 8),
            Availability::NeedsOtherAbi
        );
        assert_eq!(
            availability(&row(0x0202, 0), 0x0102, 8),
            Availability::NeedsOtherAbi
        );
        assert_eq!(
            availability(&row(0x0102, 9), 0x0102, 8),
            Availability::NeedsNewerTau
        );
    }

    #[test]
    fn two_meters_that_share_their_low_sixteen_id_bits_lose_tuning_the_others_keep_it() {
        let mk = |h| MeterRow {
            id_hash: h,
            id: String::new(),
            name: String::new(),
            version: (0, 0, 0),
            abi: 0,
            tau_min: 0,
            flags: 1,
            params_rev: 0,
            nparams: 0,
            cmds: 0,
            max_cycles: 0,
            pack_off: 0,
            pack_size: 0,
            pack_crc: 0,
            pstate: 0,
            bss: 0,
            preview_off: 0,
            preview_len: 0,
        };
        assert_eq!(
            tuning_collisions(&[mk(0x0001_1234), mk(0x0002_1234), mk(0x0001_9999)]),
            [true, true, false]
        );
        assert_eq!(tuning_collisions(&[]), Vec::<bool>::new());
    }

    #[test]
    fn a_refusal_carries_tau_alphas_number_and_name_and_a_plain_sentence() {
        assert_eq!((Refusal::Crc.code(), Refusal::Crc.name()), (4, "CRC"));
        assert_eq!(
            (Refusal::FileCrc.code(), Refusal::FileCrc.name()),
            (19, "FILECRC")
        );
        assert_eq!(
            (Refusal::NoFallback.code(), Refusal::NoFallback.name()),
            (20, "NOFALLBACK")
        );
        let e = BinError::at(Refusal::Dup, 1);
        assert_eq!(e.to_string(), "DUP (row 1)");
        assert_eq!(
            e.plain(),
            "Two meters in the file share the same short id (meter 2.)"
        );
        assert!(BinError::new(Refusal::Size).plain().ends_with('.'));
    }

    #[test]
    fn short_or_empty_input_is_refused_never_a_panic() {
        for n in 0..60 {
            let b = vec![0u8; n];
            assert!(parse(&b).is_err() && open_toc(&b, n).is_err());
        }
        assert_eq!(parse(b"TPKB\x02\x00").unwrap_err().refusal, Refusal::Size);
    }
}
