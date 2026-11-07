//! Card names: how a source file or folder name is written to the card.
//!
//! The Pocket opens a path as code page 437, one byte per character (Tau Alpha
//! `docs/features/CARD_NAME_RULES.md`, hardware B-991..B-1000), so a name keeps
//! every character that exists there (accented Latin letters, ß, æ, ¢ ...) in
//! the precomposed (NFC) form the card stores, and any other run of characters
//! is transliterated to ASCII. Two originals that end up with the same name in
//! one folder get ` (2)`, ` (3)` in the sorted order of the originals, so the
//! result is the same on every run.
//!
//! This is a port of the reference `tools/sync_media.py` (`card_part`,
//! `NameMap`, `_translit`). The shared guarantee is the rules and the code page
//! 437 bytes; the transliteration table (`deunicode` here, `anyascii` there)
//! may differ, and the name sidecar keeps the original text.

use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

/// The 128 characters of code page 437 for bytes 0x80..=0xFF.
const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ',
    'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ',
    'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕',
    '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦',
    '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐',
    '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±',
    '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
];

/// The code page 437 byte for a character, if there is one.
pub fn cp437_byte(c: char) -> Option<u8> {
    let o = c as u32;
    if (0x20..0x7F).contains(&o) {
        return Some(o as u8);
    }
    CP437_HIGH
        .iter()
        .position(|&x| x == c)
        .map(|i| 0x80 + i as u8)
}

/// A name or path as the Pocket reads it: code page 437 bytes of the NFC form.
/// `None` when a character is outside code page 437 (the file cannot be opened).
pub fn path_bytes(s: &str) -> Option<Vec<u8>> {
    s.nfc().map(cp437_byte).collect()
}

/// True when the Pocket can open a file with this path.
pub fn reachable(s: &str) -> bool {
    path_bytes(s).is_some()
}

/// Decodes code page 437 bytes (the index stores path pieces this way).
pub fn from_cp437(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b < 0x80 {
                b as char
            } else {
                CP437_HIGH[(b - 0x80) as usize]
            }
        })
        .collect()
}

/// Kept as is: printable ASCII, or a code page 437 character above 0x9F.
fn keeps(c: char) -> bool {
    let o = c as u32;
    (0x20..0x7F).contains(&o) || (o > 0x9F && cp437_byte(c).is_some())
}

fn is_greek(c: char) -> bool {
    let o = c as u32;
    (0x370..=0x3FF).contains(&o) || (0x1F00..=0x1FFF).contains(&o)
}

/// A run of characters the Pocket cannot open, as plain ASCII. Emoji, arrows,
/// dingbats, variation selectors and the zero width joiner give nothing.
pub fn translit(run: &str) -> String {
    let kept: String = run
        .chars()
        .filter(|&c| {
            let o = c as u32;
            !(o >= 0x1F000
                || (0x2190..=0x2BFF).contains(&o)
                || (0xFE00..=0xFE0F).contains(&o)
                || o == 0x200D)
        })
        .collect();
    if kept.is_empty() {
        return String::new();
    }
    let ascii: String = deunicode::deunicode_with_tofu(&kept, "")
        .chars()
        .filter(|c| (' '..='~').contains(c))
        .collect();
    ascii.split_whitespace().collect::<Vec<_>>().join(" ")
}

const FORBIDDEN: [char; 8] = ['<', '>', ':', '"', '|', '?', '*', '\\'];
const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Room kept for the root (`/Assets/tau_psram_07/common/`), the whole path limit
/// and the longest folder name, as in the reference.
pub const ROOT_BUDGET: usize = 30;
pub const MAX_PATH: usize = 200;
pub const FOLDER_MAX: usize = 64;

/// One file or folder name as it goes on the card (rules 1 to 5 of CARD_NAME_RULES.md).
pub fn card_part(name: &str, is_file: bool) -> String {
    let chars: Vec<char> = name
        .nfc()
        .map(|c| {
            if (c as u32) < 0x20 || (0x7F..=0x9F).contains(&(c as u32)) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let mut keep: Vec<bool> = chars.iter().map(|&c| keeps(c)).collect();
    let mut k = 0;
    while k < chars.len() {
        // Greek: all of a run or none of it.
        if is_greek(chars[k]) {
            let mut e = k;
            while e < chars.len() && is_greek(chars[e]) {
                e += 1;
            }
            if !keep[k..e].iter().all(|&x| x) {
                keep[k..e].iter_mut().for_each(|x| *x = false);
            }
            k = e;
        } else {
            k += 1;
        }
    }
    // Pieces: a kept character, or a transliterated run (None = it gave nothing).
    let mut out: Vec<Option<String>> = Vec::new();
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut Vec<Option<String>>| {
        if !run.is_empty() {
            let t = translit(run);
            out.push(if t.is_empty() { None } else { Some(t) });
            run.clear();
        }
    };
    for (&c, &kp) in chars.iter().zip(&keep) {
        if kp {
            flush(&mut run, &mut out);
            out.push(Some(c.to_string()));
        } else {
            run.push(c);
        }
    }
    flush(&mut run, &mut out);
    // A dropped run leaves no double space: " <dropped> " collapses to one space.
    let mut s = String::new();
    let mut i = 0;
    while i < out.len() {
        match &out[i] {
            Some(t) => s.push_str(t),
            None => {
                let before_space = s.ends_with(' ');
                let after_space = matches!(out.get(i + 1), Some(Some(t)) if t.starts_with(' '));
                if before_space && after_space {
                    // keep only the one space already written, skip the next one
                    if let Some(Some(t)) = out.get(i + 1) {
                        s.push_str(&t[1..]);
                    }
                    i += 1;
                } else if before_space || after_space {
                    // the neighbouring space stays; nothing to add
                }
            }
        }
        i += 1;
    }
    for bad in FORBIDDEN {
        s = s.replace(bad, "_");
    }
    let (stem, ext) = split_ext(&s, is_file);
    let mut stem = stem.trim_matches(|c| c == ' ' || c == '.').to_string();
    if stem.is_empty() {
        stem = "track".into();
    }
    if RESERVED.contains(&stem.to_lowercase().as_str()) {
        stem.push('_');
    }
    format!("{stem}{ext}")
}

/// Splits `name.ext` where the extension is the last `.` piece of 1 to 5
/// characters without a space (files only). The extension keeps its dot.
fn split_ext(s: &str, is_file: bool) -> (&str, &str) {
    if is_file && let Some(dot) = s.rfind('.') {
        let ext = &s[dot + 1..];
        let n = ext.chars().count();
        if (1..=5).contains(&n) && !ext.contains(' ') {
            return (&s[..dot], &s[dot..]);
        }
    }
    (s, "")
}

/// Original relative path (every folder and the file, NFC) to its card path.
/// Two different originals that convert to the same name in one folder
/// (compared without case: the card is exFAT) get ` (2)`, ` (3)` in sorted
/// order of the originals.
#[derive(Debug, Default, Clone)]
pub struct NameMap {
    map: BTreeMap<Vec<String>, Vec<String>>,
    taken: BTreeMap<(Vec<String>, String), Vec<String>>,
}

impl NameMap {
    pub fn new<I, P>(rels: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[String]>,
    {
        let mut me = Self::default();
        me.map.insert(Vec::new(), Vec::new());
        let mut all: Vec<Vec<String>> = rels
            .into_iter()
            .map(|r| r.as_ref().iter().map(|x| x.nfc().collect()).collect())
            .collect();
        all.sort();
        all.dedup();
        for rel in &all {
            for i in 1..=rel.len() {
                me.one(&rel[..i], i == rel.len());
            }
        }
        me
    }

    fn one(&mut self, orig: &[String], last_is_file: bool) {
        if self.map.contains_key(orig) {
            return;
        }
        let parent = self.map[&orig[..orig.len() - 1]].clone();
        let base = card_part(&orig[orig.len() - 1], last_is_file);
        let (stem, ext) = split_ext(&base, last_is_file);
        let (mut stem, ext) = (stem.to_string(), ext.to_string());
        let room = if last_is_file {
            let parent_len = parent.join("/").chars().count();
            let used = parent_len + usize::from(!parent.is_empty()) + ext.chars().count() + 5;
            (MAX_PATH.saturating_sub(ROOT_BUDGET).saturating_sub(used)).max(8)
        } else {
            FOLDER_MAX.max(8)
        };
        let mut base = base;
        if stem.chars().count() > room {
            stem = stem
                .chars()
                .take(room)
                .collect::<String>()
                .trim_end_matches([' ', '.'])
                .to_string();
            base = format!("{stem}{ext}");
        }
        let mut n = 1;
        let mut cand = base;
        loop {
            let key = (parent.clone(), cand.to_lowercase());
            match self.taken.get(&key) {
                Some(owner) if owner.as_slice() != orig => {
                    n += 1;
                    cand = format!("{stem} ({n}){ext}");
                }
                _ => break,
            }
        }
        self.taken
            .insert((parent.clone(), cand.to_lowercase()), orig.to_vec());
        let mut path = parent;
        path.push(cand);
        self.map.insert(orig.to_vec(), path);
    }

    /// The card path pieces for an original relative path given to `new`.
    pub fn get(&self, rel: &[String]) -> Option<&Vec<String>> {
        let key: Vec<String> = rel.iter().map(|x| x.nfc().collect()).collect();
        self.map.get(&key)
    }
}

/// Display text for the index (Tau Alpha `tau_library.latin1_text`, FONT_SPEC 3.6):
/// Latin-1, one byte per character. Characters up to U+00FF stay, common
/// typographic ones are folded (curly quotes, dashes, the ellipsis), a letter
/// that decomposes to Latin-1 loses only its mark, any other run is
/// transliterated like a card name (emoji give nothing). Empty when nothing is
/// left, so the caller falls back to the file name. The limit counts characters.
pub fn latin1_text(s: &str, limit: usize) -> String {
    use unicode_normalization::char::is_combining_mark;
    fn typo(o: u32) -> Option<&'static str> {
        Some(match o {
            0x2018..=0x201B => "'",
            0x201C..=0x201E => "\"",
            0x2013 | 0x2014 | 0x2212 => "-",
            0x2026 => "...",
            0xA0 => " ",
            0x2022 => "*",
            0x2122 => "TM",
            0x20AC => "EUR",
            _ => return None,
        })
    }
    let drawable = |c: char| {
        let o = c as u32;
        (0x20..=0x7E).contains(&o) || ((0xA1..=0xFF).contains(&o) && o != 0xAD)
    };
    let mut out = String::new();
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if !run.is_empty() {
            out.push_str(&translit(run));
            run.clear();
        }
    };
    for c in s.nfc() {
        let o = c as u32;
        if let Some(t) = typo(o) {
            flush(&mut run, &mut out);
            out.push_str(t);
        } else if drawable(c) {
            flush(&mut run, &mut out);
            out.push(c);
        } else if is_combining_mark(c) || o == 0xAD || o < 0x20 || (0x7F..=0xA0).contains(&o) {
            flush(&mut run, &mut out);
            if matches!(o, 0x09 | 0x0A | 0x0D) {
                out.push(' ');
            }
        } else {
            let base: String = std::iter::once(c)
                .nfkd()
                .filter(|&x| !is_combining_mark(x))
                .collect();
            if !base.is_empty() && base.chars().all(drawable) {
                flush(&mut run, &mut out);
                out.push_str(&base);
            } else {
                run.push(c);
            }
        }
    }
    flush(&mut run, &mut out);
    let text: String = out
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(limit)
        .collect::<String>()
        .trim()
        .to_string();
    if text.replace('?', "").trim().is_empty() {
        String::new()
    } else {
        text
    }
}

#[cfg(test)]
mod tests;
