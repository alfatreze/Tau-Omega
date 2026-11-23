//! Player 2's persisted-settings layout (version 2) and the migrations into it: a Rust statement of Tau Alpha's reference
//! `tools/persist_layout.py` (itself proven equal to the firmware's C code), checked here against its golden vectors and against real
//! files captured on a Pocket (`testdata/persist_layout/`, with a README naming the source).
//!
//! A core keeps its settings in `Settings/<core id>/Interact/_core/interact_persist.json`: a list of variables, each one 32-bit
//! register-file word ([`ID_TO_WORD`]). Three layouts exist, told apart by word 0 bits 30..27: **legacy** (version 0: v0.6.1 and every
//! TAU DEV build up to 181, word 0 is the volume 0..100), **layout 1** (v0.7.0 and v0.7.1: the selected meter by table index, one preset
//! position and one preset file per meter slot) and **layout 2** (v0.7.5: the selected meter by the first 31 bits of its id hash, one
//! preset for it, tunings keyed by the meter's id and `params_rev`). Omega only *reads* and *describes*: it never converts a file (the
//! firmware migrates a legacy or layout-1 file itself at its first boot) and it never copies words between cores of different layouts.
//! [`migrate`] is here to prove the mirror against the reference, and to tell a person what the player will do with their file.
//!
//! Hardware status (Tau Alpha): the legacy → layout 1 migration of one real file was confirmed on a Pocket on 2026-10-26; layout 2 and the
//! migrations into it are host-proven and were run on the TAU DEV 230 to v0.7.5 builds (Tau Alpha `docs/features/PERSISTENCE_LAYOUT.md`).

use crate::diag::PersistedSetting;

pub const WORDS: usize = 32;
/// Bit 30 of words 1 and 2 marks a word already converted from the legacy layout; no field reaches it, so every reader masks it.
pub const NEW_MARK: u32 = 1 << 30;
/// Bit 29 of word 6 marks "converted to layout 2".
pub const MARK6: u32 = 1 << 29;
/// The words that carry a converted mark, and the bit.
pub const MARKS: [(usize, u32); 3] = [(1, 30), (2, 30), (6, 29)];
pub const MARKED_WORDS: [usize; 3] = [1, 2, 6];
/// The order both migrations write the words in (word 0, the version, last): word 6 (the selected preset and, as scratch, the old meter
/// index) before word 5 (the id hash), so a power loss between them is resumed from the index kept in word 6.
pub const WRITE_ORDER: [usize; 15] = [1, 2, 6, 5, 15, 16, 17, 18, 21, 8, 9, 10, 11, 19, 20];
pub const LAYOUT_VERSION: u32 = 2;
/// Word 6 bits 16..11 keep the meter index a migration came from (scratch for a resumed migration; the player ignores it).
pub const SCRATCH_SHIFT: u32 = 11;
pub const SCRATCH_WIDTH: u32 = 6;
const HASH31: u32 = 0x7FFF_FFFF;
/// Persist id -> register-file word.
pub const ID_TO_WORD: [(u64, usize); 19] = [
    (10, 0),
    (11, 1),
    (12, 2),
    (15, 5),
    (16, 6),
    (20, 8),
    (21, 9),
    (22, 10),
    (23, 11),
    (24, 12),
    (25, 13),
    (26, 14),
    (27, 16),
    (28, 17),
    (29, 18),
    (30, 15),
    (31, 19),
    (32, 20),
    (33, 21),
];

/// The register-file word a persist id lives in (ids this layout does not know give `None`).
pub fn word_of(id: u64) -> Option<usize> {
    ID_TO_WORD.iter().find(|(i, _)| *i == id).map(|(_, w)| *w)
}

/// The seven shipped meters, in table order: the first 32 bits of each id (`id_hash`) and its `params_rev`. Tau Alpha's
/// `fw2/cores/shipped_meter_ids.h`; checked against their golden vectors and against `testdata/meter_bin/v2_seven.bin`.
pub const SHIPPED_METERS: [(u32, u16); 7] = [
    (2_111_700_579, 1),
    (339_397_504, 1),
    (1_953_614_881, 1),
    (1_477_607_476, 1),
    (227_279_827, 1),
    (400_951_158, 1),
    (209_902_064, 1),
];
/// What a person calls the shipped meters, in the same order.
pub const SHIPPED_NAMES: [&str; 7] = [
    "Winamp Bars",
    "Winamp Scope",
    "Chladni",
    "Layered Wave",
    "VU Master",
    "Waterfall",
    "Sound Map",
];

/// The persisted selection (31 bits of `id_hash`) of a layout 0 or 1 meter index; 0 (none: the player falls back to Winamp Bars) for an
/// index that is not a shipped meter.
pub fn hash31_of_index(i: usize) -> u32 {
    SHIPPED_METERS.get(i).map_or(0, |m| m.0 & HASH31)
}

/// The 20-bit key of a tuning record: the low 16 bits of `id_hash` and the low 4 bits of `params_rev`.
pub fn tune_key(id_hash: u32, params_rev: u16) -> u32 {
    (id_hash & 0xFFFF) | (u32::from(params_rev) & 15) << 16
}

/// The tuning key of a shipped meter's table index; `None` for an index that is not a shipped meter.
pub fn key_of_index(i: usize) -> Option<u32> {
    SHIPPED_METERS.get(i).map(|m| tune_key(m.0, m.1))
}

/// The table index of the shipped meter whose persisted selection (31 bits) is `hash31`.
pub fn shipped_index_of_hash31(hash31: u32) -> Option<usize> {
    SHIPPED_METERS
        .iter()
        .position(|m| m.0 & HASH31 == hash31 && hash31 != 0)
}

/// One field of layout 2: name, word, shift, width in bits, legal range and default (the firmware's `sf_table`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub name: &'static str,
    pub word: usize,
    pub shift: u32,
    pub width: u32,
    pub min: u32,
    pub max: u32,
    pub default: u32,
}

const fn f(
    name: &'static str,
    word: usize,
    shift: u32,
    width: u32,
    min: u32,
    max: u32,
    default: u32,
) -> Field {
    Field {
        name,
        word,
        shift,
        width,
        min,
        max,
        default,
    }
}

/// Layout 1's per-slot meter fields (read only, to describe a v0.7.0 / v0.7.1 file): the selected preset POSITION of meter slot m
/// (4 bits: word 5 for slots 0..5, word 6 for slot 6) and the preset FILE of the slot (7 bits: 0 none, else id + 1).
const MPRE_NAMES: [&str; 7] = [
    "MPRE0", "MPRE1", "MPRE2", "MPRE3", "MPRE4", "MPRE5", "MPRE6",
];
const MPID_NAMES: [&str; 7] = [
    "MPID0", "MPID1", "MPID2", "MPID3", "MPID4", "MPID5", "MPID6",
];
const V1_METER: (usize, u32, u32, u32, u32) = (5, 0, 6, 0, 62);
const MPID_SLOTS: [(usize, u32); 7] = [
    (6, 4),
    (6, 11),
    (6, 18),
    (15, 0),
    (15, 7),
    (15, 14),
    (15, 21),
];
fn v1_mpre(m: usize) -> (usize, u32, u32, u32, u32) {
    if m < 6 {
        (5, 6 + 4 * m as u32, 4, 0, 15)
    } else {
        (6, 0, 4, 0, 15)
    }
}
fn v1_mpid(m: usize) -> (usize, u32, u32, u32, u32) {
    (MPID_SLOTS[m].0, MPID_SLOTS[m].1, 7, 0, 127)
}

/// Layout 2's fields in the firmware's order.
pub fn fields() -> Vec<Field> {
    let mut t = vec![
        f("VOLUME", 2, 0, 7, 0, 100, 94),
        f("REPEAT", 2, 7, 2, 0, 2, 0),
        f("RGMODE", 2, 9, 2, 0, 2, 0),
        f("HALCYON", 2, 11, 5, 0, 30, 0),
        f("COLOUR", 1, 0, 5, 0, 18, 1),
        f("THEME", 1, 5, 6, 0, 31, 0),
        f("POLARITY", 1, 11, 1, 0, 1, 0),
        f("METER_HASH", 5, 0, 31, 0, HASH31, 0),
        f("FONT", 1, 12, 3, 0, 7, 0),
        f("LAYOUT", 0, 27, 4, 0, 15, LAYOUT_VERSION),
        f("TUNE_NEW", 0, 26, 1, 0, 1, 0),
        f("METER_PRE_POS", 6, 0, 4, 0, 15, 0),
        f("METER_PRE_ID", 6, 4, 7, 0, 127, 0),
    ];
    // The two tuning slots of four words each (slot A ids 20-23, slot B ids 27-29 and 33).
    for (k, w) in [8usize, 9, 10, 11, 16, 17, 18, 21].into_iter().enumerate() {
        t.push(f(
            [
                "TUNE0", "TUNE1", "TUNE2", "TUNE3", "TUNE4", "TUNE5", "TUNE6", "TUNE7",
            ][k],
            w,
            0,
            31,
            0,
            0x7FFF_FFFF,
            0,
        ));
    }
    t.push(f("LAST0", 19, 0, 31, 0, 0x7FFF_FFFF, 0));
    t.push(f("LAST1", 20, 0, 31, 0, 0x7FFF_FFFF, 0));
    t
}

/// The legacy layout (v0.6.1 .. TAU DEV 181): name, word, shift, width (32 = a whole word), range, default.
const LEGACY: [(&str, usize, u32, u32, u32, u32, u32); 18] = [
    ("VOLUME", 0, 0, 32, 0, 100, 94),
    ("REPEAT", 2, 0, 32, 0, 2, 0),
    ("RGMODE", 17, 4, 2, 0, 2, 0),
    ("HALCYON", 6, 0, 32, 0, 30, 0),
    ("COLOUR", 1, 0, 32, 0, 18, 1),
    ("THEME", 16, 0, 32, 0, 31, 0),
    ("POLARITY", 17, 0, 1, 0, 1, 0),
    ("METER", 5, 0, 32, 0, 14, 0),
    ("MPRE0", 18, 0, 4, 0, 7, 0),
    ("MPRE1", 18, 4, 4, 0, 7, 0),
    ("MPRE2", 18, 8, 4, 0, 7, 0),
    ("MPRE3", 18, 12, 4, 0, 15, 0),
    ("MPRE4", 18, 16, 4, 0, 7, 0),
    ("FONT", 18, 20, 3, 0, 7, 0),
    ("MPRE5", 18, 23, 4, 0, 7, 0),
    ("MPRE6", 15, 0, 4, 0, 15, 0),
    ("LAST0", 19, 0, 31, 0, 0x7FFF_FFFF, 0),
    ("LAST1", 20, 0, 31, 0, 0x7FFF_FFFF, 0),
];

fn mask(width: u32) -> u32 {
    if width >= 32 {
        u32::MAX
    } else {
        (1u32 << width) - 1
    }
}

pub type Words = [u32; WORDS];

/// The register-file words of a settings file; ids this layout does not know (and non-integer values) are ignored.
pub fn words_from_settings(settings: &[PersistedSetting]) -> Words {
    let mut w = [0u32; WORDS];
    for s in settings {
        if let (Some(word), Some(v)) = (word_of(s.id), s.value.as_u64()) {
            w[word] = (v & 0xFFFF_FFFF) as u32;
        }
    }
    w
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Layout {
    /// v0.6.1 and every TAU DEV build up to 181: word 0 is the volume.
    Legacy,
    /// v0.7.0 and v0.7.1: the selected meter by table index.
    V1,
    /// v0.7.5: the selected meter by id hash.
    V2,
    /// A version this Omega does not know (newer than 2): nothing but the version is read.
    Newer,
}

/// The layout version of a register file: word 0 bits 30..27.
pub fn version(words: &Words) -> u32 {
    (words[0] >> 27) & 15
}

/// Layout version 0 is the legacy layout (a legacy volume 0..100, or an empty word).
pub fn layout(words: &Words) -> Layout {
    match version(words) {
        0 => Layout::Legacy,
        1 => Layout::V1,
        2 => Layout::V2,
        _ => Layout::Newer,
    }
}

/// Layout 2: name -> value (a field out of its range reads as its default, like the firmware).
pub fn unpack(words: &Words) -> Vec<(&'static str, u32)> {
    fields()
        .into_iter()
        .map(|fl| {
            let v = (words[fl.word] >> fl.shift) & mask(fl.width);
            (
                fl.name,
                if (fl.min..=fl.max).contains(&v) {
                    v
                } else {
                    fl.default
                },
            )
        })
        .collect()
}

fn legacy_value(words: &Words, name: &str) -> u32 {
    let (_, word, shift, width, lo, hi, df) = LEGACY
        .iter()
        .copied()
        .find(|e| e.0 == name)
        .expect("a legacy field name");
    let v = (words[word] >> shift) & mask(width);
    if (lo..=hi).contains(&v) { v } else { df }
}

/// Values as a legacy register file holds them, as layout 2 fields: the selected meter index becomes `METER_HASH` (0 = none for an index
/// that is not a shipped meter) and its preset position `METER_PRE_POS`; fields the legacy layout lacks take their default (the old
/// single tuning is not carried over).
pub fn legacy_unpack(words: &Words) -> Vec<(&'static str, u32)> {
    let idx = legacy_value(words, "METER") as usize;
    fields()
        .into_iter()
        .map(|fl| {
            let v = match fl.name {
                "VOLUME" | "REPEAT" | "RGMODE" | "HALCYON" | "COLOUR" | "THEME" | "POLARITY"
                | "FONT" | "LAST0" | "LAST1" => legacy_value(words, fl.name),
                "METER_HASH" => hash31_of_index(idx),
                "METER_PRE_POS" => {
                    if idx < 7 {
                        legacy_value(words, MPRE_NAMES[idx])
                    } else {
                        0
                    }
                }
                _ => fl.default,
            };
            (fl.name, v)
        })
        .collect()
}

/// Merges values into a copy of `words` keeping every other bit (values clamped to the field's range, a missing one taking its default);
/// sets the converted marks (words 1 and 2 bit 30, word 6 bit 29).
pub fn pack(words: &Words, values: &[(&str, u32)]) -> Words {
    let mut w = *words;
    for fl in fields() {
        let v = values
            .iter()
            .find(|(n, _)| *n == fl.name)
            .map_or(fl.default, |(_, v)| *v);
        let v = v.clamp(fl.min, fl.max);
        let m = mask(fl.width) << fl.shift;
        w[fl.word] = (w[fl.word] & !m) | ((v << fl.shift) & m);
    }
    for (k, b) in MARKS {
        w[k] |= 1 << b;
    }
    w
}

fn get(words: &Words, spec: (usize, u32, u32, u32, u32)) -> u32 {
    let (word, shift, width, lo, hi) = spec;
    let v = (words[word] >> shift) & mask(width);
    if (lo..=hi).contains(&v) { v } else { 0 }
}

/// What layout 1's words select: (meter table index, preset position, preset file id + 1). An index that is not a shipped meter selects
/// no preset.
pub fn v1_selection(words: &Words) -> (usize, u32, u32) {
    let idx = get(words, V1_METER) as usize;
    if idx >= SHIPPED_METERS.len() {
        return (idx, 0, 0);
    }
    (idx, get(words, v1_mpre(idx)), get(words, v1_mpid(idx)))
}

// ---- the tuning record: layout 1 (check 8 | meter 5 | preset 7 | count 4 | entries from bit 24, seed 0x5A) and layout 2
// (check 8 | key 20 | preset 7 | count 4 | entries from bit 39, seed 0x3C), four words of 31 bits ----
pub const TUNE_SLOT_WORDS: [[usize; 4]; 2] = [[8, 9, 10, 11], [16, 17, 18, 21]];
const TUNE_BITS: usize = 124;
const TUNE_HEAD_V2: usize = 39;
const TUNE_SEED_V1: u32 = 0x5A;
const TUNE_SEED_V2: u32 = 0x3C;

/// The largest value of each parameter of a meter (by table index), which the layout-1 tuning needs to be read; the player reads the real
/// ones from its meters, Omega only has the golden vectors' fixture.
pub type MaxOf<'a> = &'a dyn Fn(usize) -> Option<Vec<u32>>;

fn bits_of(w4: &[u32; 4]) -> [u8; 128] {
    let mut b = [0u8; 128];
    for (k, w) in w4.iter().enumerate() {
        for i in 0..31 {
            b[31 * k + i] = ((w >> i) & 1) as u8;
        }
    }
    b
}
fn get_bits(b: &[u8; 128], pos: usize, n: usize) -> (u32, usize) {
    (
        (0..n).fold(0u32, |v, i| v | u32::from(b[pos + i]) << i),
        pos + n,
    )
}
fn put_bits(b: &mut [u8; 128], pos: usize, v: u32, n: usize) -> usize {
    for i in 0..n {
        b[pos + i] = ((v >> i) & 1) as u8;
    }
    pos + n
}
fn tune_nbits(mx: u32) -> usize {
    let mut n = 1;
    while n < 16 && (mx >> n) != 0 {
        n += 1;
    }
    n
}
fn check_byte(b: &[u8; 128], seed: u32) -> u32 {
    let mut c = seed;
    for k in 1..16 {
        let by = (0..8).fold(0u32, |v, j| v | u32::from(b[8 * k + j]) << j);
        c = ((c << 1) | (c >> 7)) & 255;
        c = (c + by) & 255;
    }
    c
}
fn words_of_bits(b: &[u8; 128]) -> [u32; 4] {
    let mut w = [0u32; 4];
    for (k, word) in w.iter_mut().enumerate() {
        *word = (0..31).fold(0u32, |v, j| v | u32::from(b[31 * k + j]) << j);
    }
    w
}

/// A tuning record of layout 2: the check byte matches and a count is stored (the entries are validated against the meter only when the
/// tuning is applied).
pub fn tune_v2_valid(w4: &[u32; 4]) -> bool {
    let b = bits_of(w4);
    let (chk, _) = get_bits(&b, 0, 8);
    let (n, _) = get_bits(&b, 35, 4);
    n != 0 && chk == check_byte(&b, TUNE_SEED_V2)
}

/// A layout-1 tuning record: the meter index, the preset and the (parameter index, value) pairs.
type TuneV1 = (usize, u32, Vec<(usize, u32)>);

/// Parses a layout-1 record, or `None` (zeros, a bad check, an unknown meter, an index past its table, a value past its maximum).
fn tune_v1_parse(w4: &[u32; 4], max_of: Option<MaxOf>) -> Option<TuneV1> {
    let b = bits_of(w4);
    let (chk, pos) = get_bits(&b, 0, 8);
    let (meter, pos) = get_bits(&b, pos, 5);
    let (preset, pos) = get_bits(&b, pos, 7);
    let (n, mut pos) = get_bits(&b, pos, 4);
    if n == 0 || chk != check_byte(&b, TUNE_SEED_V1) {
        return None;
    }
    let mx = max_of?(meter as usize)?;
    let mut out = Vec::new();
    for _ in 0..n {
        let (i, p) = get_bits(&b, pos, 6);
        pos = p;
        let i = i as usize;
        if i >= mx.len() {
            return None;
        }
        let nb = tune_nbits(mx[i]);
        if pos + nb > TUNE_BITS {
            return None;
        }
        let (v, p) = get_bits(&b, pos, nb);
        pos = p;
        if v > mx[i] {
            return None;
        }
        out.push((i, v));
    }
    Some((meter as usize, preset, out))
}

/// The four words of a layout-2 record for `entries` in index order (those that do not fit are skipped, at most 15 are stored); zeros when
/// none is stored.
fn tune_v2_pack(key: u32, preset: u32, entries: &[(usize, u32)], mx: &[u32]) -> [u32; 4] {
    let mut bits = [0u8; 128];
    let (mut pos, mut n) = (TUNE_HEAD_V2, 0u32);
    for &(i, v) in entries {
        if n >= 15 {
            break;
        }
        let nb = tune_nbits(mx[i]);
        if pos + 6 + nb > TUNE_BITS {
            continue;
        }
        pos = put_bits(&mut bits, pos, i as u32, 6);
        pos = put_bits(&mut bits, pos, v, nb);
        n += 1;
    }
    if n == 0 {
        return [0; 4];
    }
    put_bits(&mut bits, 8, key & 0xF_FFFF, 20);
    put_bits(&mut bits, 28, preset & 127, 7);
    put_bits(&mut bits, 35, n, 4);
    let chk = check_byte(&bits, TUNE_SEED_V2);
    put_bits(&mut bits, 0, chk, 8);
    words_of_bits(&bits)
}

/// A slot of layout 1 (or one a migration already converted) as layout 2: a converted record is kept; a layout-1 record of a shipped meter
/// is re-keyed (its entries and preset kept, those that no longer fit dropped); anything else (no record, a meter that is not shipped, no
/// `max_of`) becomes empty.
fn tune_migrate_slot(w4: &[u32; 4], max_of: Option<MaxOf>) -> [u32; 4] {
    if tune_v2_valid(w4) {
        return *w4;
    }
    let Some((meter, preset, entries)) = tune_v1_parse(w4, max_of) else {
        return [0; 4];
    };
    match (key_of_index(meter), max_of.and_then(|m| m(meter))) {
        (Some(key), Some(mx)) => tune_v2_pack(key, preset, &entries, &mx),
        _ => [0; 4],
    }
}

/// The 32 words a layout 0 or layout 1 register file becomes (layout 2): what the player writes at its first boot of v0.7.5. `max_of` is
/// needed only to keep the tunings of layout 1 (without it they are dropped). A file that is already layout 2 or newer is returned as is.
pub fn migrate(words: &Words, max_of: Option<MaxOf>) -> Words {
    match version(words) {
        0 => {
            let vals = legacy_unpack(words);
            let mut out = pack(&[0; WORDS], &vals);
            let idx = legacy_value(words, "METER");
            out[6] |= idx.min(63) << SCRATCH_SHIFT;
            out
        }
        1 => {
            let mut out = *words;
            let (idx, pos, pid) = v1_selection(words);
            out[5] = hash31_of_index(idx);
            out[6] = pos | pid << 4 | (idx.min(63) as u32) << SCRATCH_SHIFT | MARK6;
            out[15] = 0;
            for slot in TUNE_SLOT_WORDS {
                let w4 = slot.map(|k| words[k]);
                for (k, v) in slot.iter().zip(tune_migrate_slot(&w4, max_of)) {
                    out[*k] = v;
                }
            }
            out[0] = (words[0] & !(15 << 27)) | LAYOUT_VERSION << 27;
            out
        }
        _ => *words,
    }
}

/// The same for a file a migration half wrote (the player writes [`WRITE_ORDER`], word 0 last, so the version is still 0 or 1): a word 6
/// carrying [`MARK6`] is kept and word 5 is rebuilt from the index it keeps; (layout 0) a word 1 or 2 with its mark is kept.
pub fn migrate_resume(words: &Words, max_of: Option<MaxOf>) -> Words {
    let mut out = migrate(words, max_of);
    if version(words) == 0 {
        for k in [1, 2] {
            if words[k] & NEW_MARK != 0 {
                out[k] = words[k];
            }
        }
    }
    if words[6] & MARK6 != 0 {
        out[6] = words[6];
        out[5] = hash31_of_index(((words[6] >> SCRATCH_SHIFT) & mask(SCRATCH_WIDTH)) as usize);
    }
    out
}

/// The tuning slots in which a resumed migration differs from the uninterrupted one: a power cut INSIDE a slot's four writes leaves a
/// record that mixes the two layouts, which no reader accepts, so that one tuning is lost. `Some([])` when the two agree, else the names
/// (`'A'`, `'B'`) of the slots that differ; `None` when they differ anywhere else (a bug).
pub fn resume_lost_slots(resumed: &Words, full: &Words) -> Option<Vec<char>> {
    let lost: Vec<char> = ['A', 'B']
        .into_iter()
        .zip(TUNE_SLOT_WORDS)
        .filter(|(_, slot)| slot.iter().any(|&k| resumed[k] != full[k]))
        .map(|(n, _)| n)
        .collect();
    let inside: Vec<usize> = TUNE_SLOT_WORDS.iter().flatten().copied().collect();
    (0..WORDS)
        .filter(|k| !inside.contains(k))
        .all(|k| resumed[k] == full[k])
        .then_some(lost)
}

/// One decoded value for people: what it is called, its number and the words to show.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DecodedValue {
    pub group: String,
    pub label: String,
    pub value: u32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayoutView {
    pub layout: Layout,
    pub values: Vec<DecodedValue>,
    /// For a legacy file: the words the player would write at its first boot of v0.7.5 (word index, value), non-zero only.
    pub migrated_words: Vec<(usize, u32)>,
}

/// Looks a preset's name up: (shipped meter slot, preset file id) -> the name in that meter's `index.tpi`, when the host can tell.
pub type PresetNames<'a> = &'a dyn Fn(usize, u8) -> Option<String>;

fn meter_text(hash31: u32) -> String {
    if hash31 == 0 {
        return "none chosen (the player starts on Winamp Bars)".into();
    }
    match shipped_index_of_hash31(hash31) {
        Some(i) => SHIPPED_NAMES[i].to_string(),
        None => format!("a meter you added (id starts {hash31:08x})"),
    }
}

fn describe_value(
    name: &str,
    v: u32,
    names: PresetNames,
    selected: Option<usize>,
) -> Option<DecodedValue> {
    let (group, label, text): (&'static str, String, String) = match name {
        "VOLUME" => ("Audio", "Volume".into(), format!("{v} of 100")),
        "REPEAT" => ("Audio", "Repeat".into(), v.to_string()),
        "RGMODE" => (
            "Audio",
            "ReplayGain".into(),
            ["off", "track", "album"]
                .get(v as usize)
                .unwrap_or(&"unknown")
                .to_string(),
        ),
        "HALCYON" => ("Audio", "Halcyon EQ preset".into(), v.to_string()),
        "COLOUR" => ("Appearance", "Colour index".into(), v.to_string()),
        "THEME" => ("Appearance", "Theme".into(), v.to_string()),
        "POLARITY" => (
            "Appearance",
            "Mode".into(),
            if v == 1 { "Light" } else { "Dark" }.into(),
        ),
        "FONT" => ("Appearance", "Font".into(), v.to_string()),
        // Layout 2: the meter by id hash and its one preset.
        "METER_HASH" => ("Meters", "Selected meter".into(), meter_text(v)),
        "METER_PRE_POS" => {
            if v == 0 {
                return None;
            }
            // The position in a meter's own list (what a meter without preset files uses).
            (
                "Meters",
                "Selected meter preset position".into(),
                v.to_string(),
            )
        }
        "METER_PRE_ID" => {
            // 0 = none, else the preset FILE's id + 1.
            if v == 0 {
                return None;
            }
            let id = (v - 1) as u8;
            let text = match selected.and_then(|slot| names(slot, id)) {
                Some(name) => format!("{name} (id {id})"),
                None => format!("preset id {id}"),
            };
            ("Meters", "Selected meter preset".into(), text)
        }
        // Layout 1: the meter by table index, a preset position and a preset file per meter slot.
        "METER" => (
            "Meters",
            "Selected meter".into(),
            match SHIPPED_NAMES.get(v as usize) {
                Some(n) => (*n).to_string(),
                None => format!("meter {v} (not one of the shipped meters)"),
            },
        ),
        "LAST0" | "LAST1" => (
            "Last played",
            if name == "LAST0" {
                "Last played, word 1"
            } else {
                "Last played, word 2"
            }
            .into(),
            format!("{v:#x}"),
        ),
        n if n.starts_with("MPRE") => {
            if v == 0 {
                return None;
            }
            (
                "Meters",
                format!("Meter slot {} preset position", &n[4..]),
                v.to_string(),
            )
        }
        n if n.starts_with("MPID") => {
            if v == 0 {
                return None;
            }
            let (slot, id) = (n[4..].parse::<usize>().unwrap_or(0), (v - 1) as u8);
            let text = match names(slot, id) {
                Some(name) => format!("{name} (id {id})"),
                None => format!("preset id {id}"),
            };
            ("Meters", format!("Meter slot {slot} preset"), text)
        }
        "LAYOUT" => ("Playback", "Layout version".into(), v.to_string()),
        // The tuning words and the "tuning saved last" bit are opaque records, not settings to read.
        _ => return None,
    };
    Some(DecodedValue {
        group: group.to_string(),
        label,
        value: v,
        text,
    })
}

/// Decodes a settings file by its own layout. Nothing is guessed: the layout version in word 0 decides.
pub fn describe(settings: &[PersistedSetting]) -> LayoutView {
    describe_with_names(settings, &|_, _| None)
}

/// As [`describe`], showing the chosen preset by its name when `names` can look it up (by shipped meter slot and preset file id).
pub fn describe_with_names(settings: &[PersistedSetting], names: PresetNames) -> LayoutView {
    let w = words_from_settings(settings);
    let lay = layout(&w);
    let mut selected = None;
    let raw: Vec<(String, u32)> = match lay {
        Layout::Legacy => legacy_unpack(&w)
            .into_iter()
            .filter(|(n, _)| *n != "LAYOUT")
            .map(|(n, v)| (n.to_string(), v))
            .collect(),
        Layout::V2 => {
            let vals = unpack(&w);
            selected = vals
                .iter()
                .find(|(n, _)| *n == "METER_HASH")
                .and_then(|(_, h)| shipped_index_of_hash31(*h));
            vals.into_iter().map(|(n, v)| (n.to_string(), v)).collect()
        }
        Layout::V1 => {
            let mut out: Vec<(String, u32)> = unpack(&w)
                .into_iter()
                .filter(|(n, _)| {
                    matches!(
                        *n,
                        "VOLUME"
                            | "REPEAT"
                            | "RGMODE"
                            | "HALCYON"
                            | "COLOUR"
                            | "THEME"
                            | "POLARITY"
                            | "FONT"
                            | "LAYOUT"
                            | "LAST0"
                            | "LAST1"
                    )
                })
                .map(|(n, v)| (n.to_string(), v))
                .collect();
            out.push(("METER".into(), get(&w, V1_METER)));
            for (m, name) in MPRE_NAMES.iter().enumerate() {
                out.push(((*name).into(), get(&w, v1_mpre(m))));
            }
            for (m, name) in MPID_NAMES.iter().enumerate() {
                out.push(((*name).into(), get(&w, v1_mpid(m))));
            }
            out
        }
        Layout::Newer => vec![("LAYOUT".to_string(), version(&w))],
    };
    let values = raw
        .into_iter()
        .filter_map(|(n, v)| describe_value(&n, v, names, selected))
        .collect();
    let migrated_words = if lay == Layout::Legacy {
        migrate(&w, None)
            .iter()
            .enumerate()
            .filter(|(_, v)| **v != 0)
            .map(|(i, v)| (i, *v))
            .collect()
    } else {
        Vec::new()
    };
    LayoutView {
        layout: lay,
        values,
        migrated_words,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/persist_layout")
            .join(name)
    }
    fn golden() -> Value {
        serde_json::from_slice(&std::fs::read(fixture("golden_vectors.json")).unwrap()).unwrap()
    }
    fn words_of(map: &Value) -> Words {
        let mut w = [0u32; WORDS];
        for (k, v) in map.as_object().unwrap() {
            w[k.parse::<usize>().unwrap()] = v.as_u64().unwrap() as u32;
        }
        w
    }
    fn settings(name: &str) -> Vec<PersistedSetting> {
        crate::diag::read_persisted_settings(fixture(name)).unwrap()
    }
    fn words(name: &str) -> Words {
        words_from_settings(&settings(name))
    }
    /// The golden vectors' fixture for the shipped meters' parameter tables (the player reads the real ones).
    fn max_table() -> Vec<Vec<u32>> {
        let g = golden();
        (0..7)
            .map(|i| {
                g["tune_max_fixture"][i.to_string()]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u32)
                    .collect()
            })
            .collect()
    }
    fn from_settings_of(words: &Words, base: &[PersistedSetting]) -> Vec<PersistedSetting> {
        let mut out = base.to_vec();
        for s in &mut out {
            if let Some(word) = word_of(s.id) {
                s.value = serde_json::json!(words[word]);
            }
        }
        out
    }
    fn slot_words(w: &Words) -> Vec<[u32; 4]> {
        TUNE_SLOT_WORDS.iter().map(|s| s.map(|k| w[k])).collect()
    }
    fn lost(v: &Value) -> Option<Vec<char>> {
        v.as_array().map(|a| {
            a.iter()
                .map(|x| x.as_str().unwrap().chars().next().unwrap())
                .collect()
        })
    }

    #[test]
    fn the_constants_and_the_field_table_are_the_references() {
        let g = golden();
        assert_eq!(
            u64::from(LAYOUT_VERSION),
            g["layout_version"].as_u64().unwrap()
        );
        let ref_fields = g["fields"].as_array().unwrap();
        let ours = fields();
        assert_eq!(ours.len(), ref_fields.len());
        for (a, b) in ours.iter().zip(ref_fields) {
            let n = |k: &str| b[k].as_u64().unwrap() as u32;
            assert_eq!(
                (
                    a.name,
                    a.word as u32,
                    a.shift,
                    a.width,
                    a.min,
                    a.max,
                    a.default
                ),
                (
                    b["name"].as_str().unwrap(),
                    n("word"),
                    n("shift"),
                    n("width"),
                    n("min"),
                    n("max"),
                    n("default")
                ),
                "{}",
                a.name
            );
        }
        let order: Vec<usize> = g["write_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(order, WRITE_ORDER);
        let marks: Vec<(usize, u32)> = g["marks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| {
                (
                    m[0].as_u64().unwrap() as usize,
                    m[1].as_u64().unwrap() as u32,
                )
            })
            .collect();
        assert_eq!(marks, MARKS);
        assert_eq!(MARKED_WORDS, [1, 2, 6]);
        let scratch = &g["scratch"];
        assert_eq!(
            (6, u64::from(SCRATCH_SHIFT), u64::from(SCRATCH_WIDTH)),
            (
                scratch["word"].as_u64().unwrap(),
                scratch["shift"].as_u64().unwrap(),
                scratch["width"].as_u64().unwrap()
            )
        );
        assert_eq!(g["id_to_word"].as_object().unwrap().len(), ID_TO_WORD.len());
        for (id, word) in g["id_to_word"].as_object().unwrap() {
            assert_eq!(
                word_of(id.parse().unwrap()),
                Some(word.as_u64().unwrap() as usize),
                "id {id}"
            );
        }
    }

    #[test]
    fn the_shipped_meter_table_is_tau_alphas_and_the_meter_bin_vectors() {
        let g = golden();
        let rows = g["shipped_meters"].as_array().unwrap();
        assert_eq!(rows.len(), SHIPPED_METERS.len());
        for (i, r) in rows.iter().enumerate() {
            assert_eq!(r["index"].as_u64().unwrap() as usize, i);
            assert_eq!(
                u64::from(SHIPPED_METERS[i].0),
                r["id_hash"].as_u64().unwrap(),
                "meter {i}"
            );
            assert_eq!(
                u64::from(SHIPPED_METERS[i].1),
                r["params_rev"].as_u64().unwrap(),
                "meter {i}"
            );
            assert_eq!(
                u64::from(hash31_of_index(i)),
                r["id_hash31"].as_u64().unwrap()
            );
            assert_eq!(
                key_of_index(i).map(u64::from),
                r["tune_key"].as_u64(),
                "meter {i}"
            );
        }
        assert_eq!((hash31_of_index(7), key_of_index(7)), (0, None));
        // The same seven meters, in the same order, as the meter bin vector (ids, revisions, and names but one).
        let bin = std::fs::read(fixture("../meter_bin/v2_seven.bin")).unwrap();
        let rows = crate::meter_bin::parse(&bin).unwrap().rows;
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row.id_hash, SHIPPED_METERS[i].0, "{}", row.name);
            assert_eq!(row.params_rev, SHIPPED_METERS[i].1, "{}", row.name);
            // The names are compared except the fifth: the vector was composed with an earlier build whose fifth meter was called
            // MASTER VU; the shipped v0.7.5 pack says VU MASTER (`tests/update_v075_cases.rs` checks the shipped file's names).
            if i != 4 {
                assert_eq!(
                    row.name.to_ascii_lowercase(),
                    SHIPPED_NAMES[i].to_ascii_lowercase()
                );
            }
        }
        assert_eq!(rows[4].name, "MASTER VU");
        assert_eq!(SHIPPED_NAMES[4], "VU Master");
    }

    #[test]
    fn every_legacy_vector_migrates_to_the_same_words_and_values() {
        let g = golden();
        let cases = g["cases"].as_array().unwrap();
        assert!(cases.len() >= 38);
        for c in cases {
            let legacy = words_of(&c["legacy_words"]);
            assert_eq!(layout(&legacy), Layout::Legacy, "{}", c["name"]);
            assert_eq!(
                migrate(&legacy, None),
                words_of(&c["migrated_words"]),
                "{}",
                c["name"]
            );
            let got = legacy_unpack(&legacy);
            assert_eq!(
                got.len(),
                c["values"].as_object().unwrap().len(),
                "{}",
                c["name"]
            );
            for (name, v) in got {
                assert_eq!(
                    u64::from(v),
                    c["values"][name].as_u64().unwrap(),
                    "{} {name}",
                    c["name"]
                );
            }
        }
    }

    #[test]
    fn an_interrupted_legacy_migration_resumes_as_the_reference_says() {
        for r in golden()["resume"].as_array().unwrap() {
            let (legacy, half) = (words_of(&r["legacy_words"]), words_of(&r["half_words"]));
            let resumed = migrate_resume(&half, None);
            assert_eq!(resumed, words_of(&r["resumed_words"]));
            let full = migrate(&legacy, None);
            assert_eq!(
                resumed == full,
                r["equals_full_migration"].as_bool().unwrap()
            );
            assert_eq!(
                resume_lost_slots(&resumed, &full),
                lost(&r["lost_tuning_slots"])
            );
        }
    }

    #[test]
    fn every_layout_1_vector_migrates_and_keeps_or_drops_its_tunings_as_the_reference_does() {
        let (g, table) = (golden(), max_table());
        let max_of = |i: usize| table.get(i).cloned();
        let cases = g["cases_v1"].as_array().unwrap();
        assert!(cases.len() >= 31);
        for c in cases {
            let w = words_of(&c["v1_words"]);
            assert_eq!(layout(&w), Layout::V1, "{}", c["name"]);
            let migrated = migrate(&w, Some(&max_of));
            assert_eq!(migrated, words_of(&c["migrated_words"]), "{}", c["name"]);
            let (idx, pos, pid) = v1_selection(&w);
            let sel = &c["selected"];
            assert_eq!(
                (
                    idx as u64,
                    u64::from(pos),
                    u64::from(pid),
                    u64::from(hash31_of_index(idx))
                ),
                (
                    sel["meter_index"].as_u64().unwrap(),
                    sel["preset_position"].as_u64().unwrap(),
                    sel["preset_id_plus_1"].as_u64().unwrap(),
                    sel["id_hash31"].as_u64().unwrap()
                ),
                "{}",
                c["name"]
            );
            let kept: Vec<bool> = slot_words(&migrated).iter().map(tune_v2_valid).collect();
            let want: Vec<bool> = c["tuning_slots_kept"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| b.as_bool().unwrap())
                .collect();
            assert_eq!(kept, want, "{}", c["name"]);
            assert_eq!(layout(&migrated), Layout::V2);
        }
        // Without the parameter tables no layout-1 tuning can be re-keyed: every slot is dropped, nothing else changes.
        let some_tuned = cases
            .iter()
            .find(|c| {
                c["tuning_slots_kept"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|b| b.as_bool().unwrap())
            })
            .unwrap();
        let w = words_of(&some_tuned["v1_words"]);
        let (with, without) = (migrate(&w, Some(&max_of)), migrate(&w, None));
        assert!(slot_words(&without).iter().all(|s| *s == [0; 4]));
        assert!(
            (0..WORDS)
                .filter(|k| !TUNE_SLOT_WORDS.iter().flatten().any(|t| t == k))
                .all(|k| with[k] == without[k])
        );
    }

    #[test]
    fn an_interrupted_layout_1_migration_resumes_as_the_reference_says() {
        let (g, table) = (golden(), max_table());
        let max_of = |i: usize| table.get(i).cloned();
        let all = g["resume_v1"].as_array().unwrap();
        assert!(all.len() >= 48);
        for r in all {
            let (v1, half) = (words_of(&r["v1_words"]), words_of(&r["half_words"]));
            let resumed = migrate_resume(&half, Some(&max_of));
            assert_eq!(resumed, words_of(&r["resumed_words"]));
            let full = migrate(&v1, Some(&max_of));
            assert_eq!(
                resumed == full,
                r["equals_full_migration"].as_bool().unwrap()
            );
            assert_eq!(
                resume_lost_slots(&resumed, &full),
                lost(&r["lost_tuning_slots"])
            );
        }
    }

    #[test]
    fn the_real_captures_migrate_to_the_staged_layout_2_files_and_resume_from_a_cut() {
        let table = max_table();
        let max_of = |i: usize| table.get(i).cloned();
        let expected = words("staged/expected_layout2_from_dev181_layout0_interact_persist.json");
        assert_eq!(
            expected,
            words("staged/expected_layout2_from_dev185_layout1_interact_persist.json"),
            "both inputs hold the same settings"
        );
        let (dev181, dev185) = (
            words("dev181_interact_persist.json"),
            words("dev185_after_migration_interact_persist.json"),
        );
        assert_eq!(
            (layout(&dev181), layout(&dev185), layout(&expected)),
            (Layout::Legacy, Layout::V1, Layout::V2)
        );
        assert_eq!(migrate(&dev181, None), expected);
        assert_eq!(migrate(&dev185, Some(&max_of)), expected);
        for (tag, input, with) in [
            ("dev181_layout0", &dev181, None),
            ("dev185_layout1", &dev185, Some(&max_of as MaxOf)),
        ] {
            for n in ["03", "04", "10"] {
                let cut = words(&format!(
                    "staged/interrupted_{tag}_after_{n}_writes_interact_persist.json"
                ));
                assert_eq!(
                    migrate_resume(&cut, with),
                    expected,
                    "{tag} cut after {n} writes"
                );
                assert_eq!(migrate(input, with), expected);
            }
        }
    }

    #[test]
    fn a_layout_2_file_shows_the_meter_by_name_and_its_preset() {
        let base = settings("staged/expected_layout2_from_dev181_layout0_interact_persist.json");
        let view = describe(&base);
        assert_eq!(view.layout, Layout::V2);
        assert!(view.migrated_words.is_empty());
        let get = |l: &str| {
            view.values
                .iter()
                .find(|v| v.label == l)
                .map(|v| v.text.clone())
        };
        assert_eq!(
            (
                get("Volume"),
                get("Colour index"),
                get("Selected meter"),
                get("Font")
            ),
            (
                Some("73 of 100".into()),
                Some("7".into()),
                Some("Sound Map".into()),
                Some("2".into())
            )
        );
        // A chosen preset file is shown by name when the resolver knows it (meter 2, file id 4 stored as 5).
        let mut w = words_from_settings(&base);
        w[5] = hash31_of_index(2);
        w[6] = (w[6] & !(0x7F << 4)) | 5 << 4;
        let s2 = from_settings_of(&w, &base);
        let named = describe_with_names(&s2, &|slot, id| {
            (slot == 2 && id == 4).then(|| "AURORA".to_string())
        });
        let row = |v: &LayoutView, l: &str| {
            v.values
                .iter()
                .find(|x| x.label == l)
                .map(|x| x.text.clone())
        };
        assert_eq!(
            (
                row(&named, "Selected meter"),
                row(&named, "Selected meter preset")
            ),
            (Some("Chladni".into()), Some("AURORA (id 4)".into()))
        );
        assert_eq!(
            row(&describe(&s2), "Selected meter preset"),
            Some("preset id 4".into())
        );
        // A meter that is not one of the seven (added by the user): no name is asked for, the id is shown.
        w[5] = 0x1234_5678;
        let other = describe_with_names(&from_settings_of(&w, &base), &|_, _| {
            panic!("a user meter has no shipped slot")
        });
        assert_eq!(
            row(&other, "Selected meter"),
            Some("a meter you added (id starts 12345678)".into())
        );
        assert_eq!(
            row(&other, "Selected meter preset"),
            Some("preset id 4".into())
        );
        w[5] = 0;
        assert!(
            row(&describe(&from_settings_of(&w, &base)), "Selected meter")
                .unwrap()
                .starts_with("none chosen")
        );
    }

    #[test]
    fn layout_1_files_still_read_by_meter_slot_and_a_legacy_file_says_what_will_happen() {
        for n in [
            "dev182_interact_persist.json",
            "dev184_interact_persist.json",
            "dev213_interact_persist.json",
        ] {
            let v = describe(&settings(n));
            assert_eq!(v.layout, Layout::V1, "{n}");
            assert!(v.migrated_words.is_empty());
            assert!(v.values.iter().any(|x| x.label == "Volume"));
        }
        let v1 = describe(&settings("dev185_after_migration_interact_persist.json"));
        let get = |l: &str| v1.values.iter().find(|v| v.label == l).map(|v| v.value);
        assert_eq!(
            (
                get("Volume"),
                get("Colour index"),
                get("Selected meter"),
                get("Font")
            ),
            (Some(73), Some(7), Some(6), Some(2))
        );
        let legacy = describe(&settings("dev181_interact_persist.json"));
        assert_eq!(legacy.layout, Layout::Legacy);
        assert!(!legacy.migrated_words.is_empty());
        assert!(
            legacy.migrated_words.iter().any(|(k, _)| *k == 0),
            "word 0 carries the new version"
        );
        let find = |l: &str| {
            legacy
                .values
                .iter()
                .find(|x| x.label == l)
                .map(|x| x.text.clone())
        };
        assert_eq!(
            (find("Volume"), find("Selected meter")),
            (Some("73 of 100".into()), Some("Sound Map".into()))
        );
    }

    #[test]
    fn the_converted_marks_never_reach_a_value_out_of_range_reads_as_default_and_a_newer_layout_is_not_guessed()
     {
        let mut w = migrate(
            &[
                73, 7, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0,
            ],
            None,
        );
        assert!(MARKS.iter().all(|(k, b)| w[*k] & (1 << b) != 0));
        let get = |w: &Words, n: &str| unpack(w).into_iter().find(|(m, _)| *m == n).unwrap().1;
        assert_eq!(
            (get(&w, "VOLUME"), get(&w, "METER_HASH")),
            (73, hash31_of_index(6))
        );
        w[2] = (w[2] & !0x7F) | 101; // 101 is above the legal 100
        assert_eq!(
            get(&w, "VOLUME"),
            94,
            "an out-of-range volume reads as the default"
        );
        // An empty word 0 is a legacy file (volume 0 is a valid legacy volume); versions 1, 2 and anything above are told apart.
        assert_eq!(layout(&[0; WORDS]), Layout::Legacy);
        let with_version = |v: u32| {
            let mut x = [0u32; WORDS];
            x[0] = v << 27;
            x
        };
        assert_eq!(
            [1, 2, 3, 15].map(|v| layout(&with_version(v))),
            [Layout::V1, Layout::V2, Layout::Newer, Layout::Newer]
        );
        // A layout this Omega does not know is not decoded; a layout 2 file is never converted.
        let base = settings("staged/expected_layout2_from_dev181_layout0_interact_persist.json");
        let mut newer = words_from_settings(&base);
        newer[0] = (newer[0] & !(15 << 27)) | 3 << 27;
        let view = describe(&from_settings_of(&newer, &base));
        assert_eq!((view.layout, view.values.len()), (Layout::Newer, 1));
        assert_eq!(
            migrate(&words_from_settings(&base), None),
            words_from_settings(&base)
        );
    }

    #[test]
    fn a_tuning_record_is_valid_only_with_its_check_byte_and_a_count() {
        assert!(!tune_v2_valid(&[0; 4]));
        let table = max_table();
        let key = key_of_index(2).unwrap();
        let rec = tune_v2_pack(key, 3, &[(0, 1), (2, table[2][2] / 2)], &table[2]);
        assert!(tune_v2_valid(&rec));
        let mut torn = rec;
        torn[1] ^= 1 << 4;
        assert!(
            !tune_v2_valid(&torn),
            "a bit flipped in the record is refused"
        );
        assert_eq!(tune_v2_pack(key, 3, &[], &table[2]), [0; 4]);
    }
}
