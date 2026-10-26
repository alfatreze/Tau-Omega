//! Player 2's persisted-settings layout, and the migration from the legacy layout: a Rust statement of Tau Alpha's
//! reference `tools/persist_layout.py` (itself proven equal to the firmware's C code), checked here against its golden
//! vectors and against real files captured on a Pocket (`testdata/persist_layout/`, with a README naming the source).
//!
//! A core keeps its settings in `Settings/<core id>/Interact/_core/interact_persist.json`: a list of variables, each
//! one 32-bit register-file word (`ID_TO_WORD`). Two layouts exist: **legacy** (v0.6.1 and every TAU DEV build up to 181:
//! word 0 is the volume 0..100) and **new** (v0.7.0: word 0 carries the layout version in bits 30..27 and every other
//! word is regrouped by area). Omega only *reads* and *describes*; it never converts a file (the firmware migrates a
//! legacy file itself at the first boot) and it never copies words between cores of different layouts.
//!
//! Hardware status of the migration (Tau Alpha, 2026-10-26): one real legacy file (TAU DEV 181) migrated on TAU DEV 185 and
//! equals this module's [`migrate`] word for word; other legacy files and a power loss mid-migration are host-proven only.

use crate::diag::PersistedSetting;

pub const WORDS: usize = 32;
/// Bit 30 of words 1, 2, 5, 6 and 15 marks a word already converted; no field reaches it, so every reader masks it.
pub const NEW_MARK: u32 = 1 << 30;
pub const MARKED_WORDS: [usize; 5] = [1, 2, 5, 6, 15];
/// The order a migration writes the words in (word 0, the version, last): a legacy word is overwritten only after every
/// new word built from it.
pub const WRITE_ORDER: [usize; 15] = [1, 2, 5, 6, 15, 16, 17, 18, 21, 8, 9, 10, 11, 19, 20];
pub const LAYOUT_VERSION: u32 = 1;
/// Persist id -> register-file word.
pub const ID_TO_WORD: [(u64, usize); 18] = [
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
];
/// Ids that are not in [`ID_TO_WORD`]: 33 is the fourth word of tuning slot B (word 21).
const ID_33_WORD: usize = 21;

fn word_of(id: u64) -> Option<usize> {
    if id == 33 {
        return Some(ID_33_WORD);
    }
    ID_TO_WORD.iter().find(|(i, _)| *i == id).map(|(_, w)| *w)
}

/// One field of the new layout: name, word, shift, width in bits, legal range and default (the firmware's `sf_table`).
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

const MPRE_NAMES: [&str; 20] = [
    "MPRE0", "MPRE1", "MPRE2", "MPRE3", "MPRE4", "MPRE5", "MPRE6", "MPRE7", "MPRE8", "MPRE9",
    "MPRE10", "MPRE11", "MPRE12", "MPRE13", "MPRE14", "MPRE15", "MPRE16", "MPRE17", "MPRE18",
    "MPRE19",
];

/// The new layout's fields in the firmware's order.
pub fn fields() -> Vec<Field> {
    let mut t = vec![
        f("VOLUME", 2, 0, 7, 0, 100, 94),
        f("REPEAT", 2, 7, 2, 0, 2, 0),
        f("RGMODE", 2, 9, 2, 0, 2, 0),
        f("HALCYON", 2, 11, 5, 0, 30, 0),
        f("COLOUR", 1, 0, 5, 0, 18, 1),
        f("THEME", 1, 5, 6, 0, 31, 0),
        f("POLARITY", 1, 11, 1, 0, 1, 0),
        f("METER", 5, 0, 6, 0, 62, 0),
        f("FONT", 1, 12, 3, 0, 7, 0),
        f("LAYOUT", 0, 27, 4, 0, 15, LAYOUT_VERSION),
        f("TUNE_NEW", 0, 26, 1, 0, 1, 0),
    ];
    for (m, name) in MPRE_NAMES.iter().enumerate() {
        // The selected preset of meter m, 4 bits: word 5 (meters 0..5), word 6 (6..12), word 15 (13..19).
        let (w, s) = if m < 6 {
            (5, 6 + 4 * m as u32)
        } else if m < 13 {
            (6, 4 * (m as u32 - 6))
        } else {
            (15, 4 * (m as u32 - 13))
        };
        t.push(f(name, w, s, 4, 0, 15, 0));
    }
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
    /// v0.6.1 and every TAU DEV build up to 181.
    Legacy,
    /// v0.7.0: the layout version in word 0 bits 30..27.
    New,
}

/// Layout version 0 (bits 30..27 of word 0) is the legacy layout (a legacy volume 0..100, or an empty word).
pub fn layout(words: &Words) -> Layout {
    if (words[0] >> 27) & 15 == 0 {
        Layout::Legacy
    } else {
        Layout::New
    }
}

/// The new layout: name -> value (a field out of its range reads as its default, like the firmware).
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

/// Values as a legacy register file holds them; fields the legacy layout lacks take their default (the old single tuning
/// is not carried over).
pub fn legacy_unpack(words: &Words) -> Vec<(&'static str, u32)> {
    let mut out: Vec<(&'static str, u32)> = fields()
        .into_iter()
        .map(|fl| (fl.name, fl.default))
        .collect();
    for (name, word, shift, width, lo, hi, df) in LEGACY {
        let v = (words[word] >> shift) & mask(width);
        let v = if (lo..=hi).contains(&v) { v } else { df };
        if let Some(slot) = out.iter_mut().find(|(n, _)| *n == name) {
            slot.1 = v;
        }
    }
    out
}

/// Merges values into a copy of `words` keeping every other bit; sets the converted mark on words 1, 2, 5, 6, 15.
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
    for k in MARKED_WORDS {
        w[k] |= NEW_MARK;
    }
    w
}

/// The 32 words a legacy register file becomes (what the firmware writes at its first boot).
pub fn migrate(words: &Words) -> Words {
    pack(&[0; WORDS], &legacy_unpack(words))
}

/// The same for a file a migration half wrote: a marked word is kept, the others are built from the legacy words still there.
pub fn migrate_resume(words: &Words) -> Words {
    let mut out = migrate(words);
    for k in MARKED_WORDS {
        if words[k] & NEW_MARK != 0 {
            out[k] = words[k];
        }
    }
    out
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
    /// For a legacy file: the words the firmware would write at its first boot (word index, value), non-zero only.
    pub migrated_words: Vec<(usize, u32)>,
}

fn describe_value(name: &'static str, v: u32) -> Option<DecodedValue> {
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
        "METER" => ("Meters", "Selected meter".into(), v.to_string()),
        "LAYOUT" => ("Playback", "Layout version".into(), v.to_string()),
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
            ("Meters", format!("Meter {} preset", &n[4..]), v.to_string())
        }
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
    let w = words_from_settings(settings);
    let lay = layout(&w);
    let raw = if lay == Layout::Legacy {
        legacy_unpack(&w)
    } else {
        unpack(&w)
    };
    let values = raw
        .into_iter()
        .filter_map(|(n, v)| describe_value(n, v))
        .collect();
    let migrated_words = if lay == Layout::Legacy {
        migrate(&w)
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

    #[test]
    fn the_field_table_is_the_reference_table() {
        let g = golden();
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
        let marked: Vec<usize> = g["marked_words"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(marked, MARKED_WORDS);
        for (id, word) in g["id_to_word"].as_object().unwrap() {
            assert_eq!(
                word_of(id.parse().unwrap()),
                Some(word.as_u64().unwrap() as usize),
                "id {id}"
            );
        }
    }

    #[test]
    fn every_golden_vector_migrates_to_the_same_words_and_values() {
        let g = golden();
        let cases = g["cases"].as_array().unwrap();
        assert!(cases.len() >= 37);
        for c in cases {
            let legacy = words_of(&c["legacy_words"]);
            assert_eq!(layout(&legacy), Layout::Legacy, "{}", c["name"]);
            assert_eq!(
                migrate(&legacy),
                words_of(&c["migrated_words"]),
                "{}",
                c["name"]
            );
            for (name, v) in legacy_unpack(&legacy) {
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
    fn an_interrupted_migration_resumes_to_the_full_one() {
        let g = golden();
        for r in g["resume"].as_array().unwrap() {
            let half = words_of(&r["half_words"]);
            assert_eq!(migrate_resume(&half), words_of(&r["resumed_words"]));
            assert_eq!(
                migrate_resume(&half) == migrate(&words_of(&r["legacy_words"])),
                r["equals_full_migration"].as_bool().unwrap()
            );
        }
    }

    #[test]
    fn the_real_legacy_file_migrates_to_exactly_what_a_pocket_wrote() {
        // TAU DEV 181's real legacy file, and the file TAU DEV 185 wrote after migrating it on a Pocket (2026-10-26).
        let before = words_from_settings(&settings("dev181_interact_persist.json"));
        let after = words_from_settings(&settings("dev185_after_migration_interact_persist.json"));
        assert_eq!(layout(&before), Layout::Legacy);
        assert_eq!(layout(&after), Layout::New);
        assert_eq!(migrate(&before), after);
        let view = describe(&settings("dev185_after_migration_interact_persist.json"));
        let get = |l: &str| view.values.iter().find(|v| v.label == l).map(|v| v.value);
        assert_eq!(
            (
                get("Volume"),
                get("Colour index"),
                get("Selected meter"),
                get("Font")
            ),
            (Some(73), Some(7), Some(6), Some(2))
        );
    }

    #[test]
    fn the_real_new_layout_files_decode_and_a_legacy_file_says_what_will_happen() {
        for n in [
            "dev182_interact_persist.json",
            "dev184_interact_persist.json",
        ] {
            let v = describe(&settings(n));
            assert_eq!(v.layout, Layout::New, "{n}");
            assert!(v.migrated_words.is_empty());
            assert!(v.values.iter().any(|x| x.label == "Volume"));
        }
        let legacy = describe(&settings("dev181_interact_persist.json"));
        assert_eq!(legacy.layout, Layout::Legacy);
        assert!(!legacy.migrated_words.is_empty());
        assert_eq!(
            legacy
                .values
                .iter()
                .find(|x| x.label == "Volume")
                .unwrap()
                .value,
            73
        );
    }

    #[test]
    fn the_converted_mark_never_reaches_a_value_and_out_of_range_reads_as_default() {
        let mut w = migrate(&[
            73, 7, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ]);
        assert!(MARKED_WORDS.iter().all(|k| w[*k] & NEW_MARK != 0));
        let vol = |w: &Words| {
            unpack(w)
                .into_iter()
                .find(|(n, _)| *n == "VOLUME")
                .unwrap()
                .1
        };
        assert_eq!(vol(&w), 73);
        w[2] = (w[2] & !0x7F) | 101; // 101 is above the legal 100
        assert_eq!(vol(&w), 94, "an out-of-range volume reads as the default");
        // An empty word 0 is a legacy file (volume 0 is a valid legacy volume).
        assert_eq!(layout(&[0; WORDS]), Layout::Legacy);
    }
}
