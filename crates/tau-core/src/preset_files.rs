//! Meter presets as files on the card (Tau Alpha `docs/features/METER_PRESETS_EXTERNAL.md`, firmware reader
//! `fw2/cores/mpreset_core.h`, reference writer and reader `tools/player2/mpreset.py`): under `<media root>/tau-presets/<meter
//! manifest id>/` one `index.tpi` (the presets of the meter in list order: id, hidden flag, name) and one `p<id>.tpr` per
//! preset (a sparse list of `(parameter key hash, value)`). Omega is the only author: the device never writes them.
//!
//! ```text
//! index.tpi  "TPRI" | version u8 = 1 | reserved u8 = 0 | meter id u16 | crc32 u32 (after the first 12 bytes) | count u16 |
//!            count x { id u8 | flags u8 (bit 0 hidden) | name[16] NUL padded }
//! p<id>.tpr  "TPRS" | version u8 = 1 | id u8 | meter id u16 | crc32 u32 (after the first 12 bytes) | nkeys u8 | name_len u8 |
//!            name | nkeys x { key u16 | value u16 }                                       (little endian)
//! ```
//! A parameter a preset omits takes the meter's default, an unknown key is ignored and a value is clamped, so a preset
//! survives a meter that gained or lost parameters. DEFAULT (every parameter at its default) is built into every meter: it is
//! not a file and never appears in an index. Ids are 0..=126 (the device remembers `id + 1` in 7 bits), at most 127 presets
//! a meter, names at most 15 characters, and the writer refuses two parameters whose key hashes collide.
//!
//! Checked byte for byte against the reference in `tests/preset_files.rs` and on a real card's factory set.

use crate::{
    ErrorCode, TauError,
    meter_presets::{ParamType, RegMeter, RegParam},
};
use std::collections::{BTreeMap, BTreeSet};

pub const DIR: &str = "tau-presets";
pub const INDEX_NAME: &str = "index.tpi";
pub const MAX_ID: u8 = 126;
pub const MAX_PRESETS: usize = 127;
pub const NAME_MAX: usize = 15;
/// The firmware reads a preset file into a buffer of `14 + 15 + 4 * 64` bytes.
pub const MAX_KEYS: usize = 64;
const IDX_MAGIC: &[u8; 4] = b"TPRI";
const PRE_MAGIC: &[u8; 4] = b"TPRS";
const VERSION: u8 = 1;
const NAME_FIELD: usize = 16;
const ENTRY: usize = 2 + NAME_FIELD;
const IDX_MAX_BYTES: usize = 14 + MAX_PRESETS * ENTRY;

fn bad(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidPathReference, msg)
}

/// FNV-1a over the key's bytes, the 32 bits folded to 16 (`mp_key_hash`).
pub fn key_hash(key: &str) -> u16 {
    let mut h: u32 = 2_166_136_261;
    for b in key.bytes() {
        h = (h ^ u32::from(b)).wrapping_mul(16_777_619);
    }
    ((h ^ (h >> 16)) & 0xFFFF) as u16
}

/// The keys the firmware shows as colours (raw RGB565 values): the whole 16-bit range is legal for them.
fn is_colour(p: &RegParam) -> bool {
    p.kind == ParamType::U16
        && (p.key.starts_with("custom_")
            || matches!(p.key.as_str(), "color_green" | "color_yellow" | "color_red"))
}

/// One parameter as a preset sees it: the key, its hash, the legal range and the default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamSpec {
    pub key: String,
    pub hash: u16,
    pub min: i64,
    pub max: i64,
    pub default: i64,
}

/// The parameters of a meter in the registry's order. Refuses two keys with one hash (the device could not tell them apart).
pub fn param_specs(meter: &RegMeter) -> Result<Vec<ParamSpec>, TauError> {
    let mut seen: BTreeMap<u16, &str> = BTreeMap::new();
    let mut out = Vec::new();
    for p in &meter.params {
        let hash = key_hash(&p.key);
        if let Some(other) = seen.insert(hash, &p.key) {
            return Err(bad(format!(
                "{}: the parameter keys {other:?} and {:?} have the same hash {hash:04x}",
                meter.key, p.key
            )));
        }
        let (min, max) = match p.kind {
            ParamType::Bool => (0, 1),
            ParamType::Enum => (0, p.values.len() as i64 - 1),
            ParamType::U16 if is_colour(p) => (0, 65_535),
            _ => (p.min, p.max),
        };
        let default = if p.kind == ParamType::Bool {
            i64::from(p.default != 0)
        } else {
            p.default
        };
        out.push(ParamSpec {
            key: p.key.clone(),
            hash,
            min,
            max,
            default,
        });
    }
    Ok(out)
}

/// A name the files can carry: 1 to 15 printable ASCII characters (the factory set uses `[` and `]`).
pub fn name_ok(name: &str) -> bool {
    (1..=NAME_MAX).contains(&name.len()) && name.bytes().all(|b| (0x20..0x7F).contains(&b))
}

fn wrap(magic: &[u8; 4], ident: u8, meter_id: u16, body: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + body.len());
    v.extend_from_slice(magic);
    v.push(VERSION);
    v.push(ident);
    v.extend_from_slice(&meter_id.to_le_bytes());
    v.extend_from_slice(&crc32fast::hash(body).to_le_bytes());
    v.extend_from_slice(body);
    v
}

/// An entry of an index: the id is what the device remembers, never the position.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndexEntry {
    pub id: u8,
    pub name: String,
    pub hidden: bool,
}

pub fn build_index(meter_id: u16, entries: &[IndexEntry]) -> Result<Vec<u8>, TauError> {
    if entries.len() > MAX_PRESETS {
        return Err(bad(
            "more than 127 presets (ids are 0..126: the choice is remembered in 7 bits)",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut body = (entries.len() as u16).to_le_bytes().to_vec();
    for e in entries {
        if e.id > MAX_ID || !ids.insert(e.id) {
            return Err(bad("preset ids must be distinct and 0..126"));
        }
        if !name_ok(&e.name) {
            return Err(bad(format!(
                "a name of 1 to 15 printable ASCII characters: {:?}",
                e.name
            )));
        }
        body.push(e.id);
        body.push(u8::from(e.hidden));
        let mut field = [0u8; NAME_FIELD];
        field[..e.name.len()].copy_from_slice(e.name.as_bytes());
        body.extend_from_slice(&field);
    }
    Ok(wrap(IDX_MAGIC, 0, meter_id, &body))
}

/// Checks the common header and returns `(ident byte, meter id, body)`; the same refusals as `mp_index_check`/`mp_preset_check`.
fn split<'a>(data: &'a [u8], magic: &[u8; 4], what: &str) -> Result<(u8, u16, &'a [u8]), TauError> {
    if data.len() < 14 || &data[..4] != magic {
        return Err(bad(format!("not a {what} file")));
    }
    if data[4] != VERSION {
        return Err(bad(format!("{what}: version {} is not 1", data[4])));
    }
    let crc = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    if crc32fast::hash(&data[12..]) != crc {
        return Err(bad(format!("{what}: the checksum does not match")));
    }
    Ok((data[5], u16::from_le_bytes([data[6], data[7]]), &data[12..]))
}

pub fn parse_index(data: &[u8]) -> Result<(u16, Vec<IndexEntry>), TauError> {
    if data.len() > IDX_MAX_BYTES {
        return Err(bad("index.tpi is larger than 127 presets allow"));
    }
    let (reserved, meter_id, body) = split(data, IDX_MAGIC, "index.tpi")?;
    if reserved != 0 {
        return Err(bad("index.tpi: the reserved byte is not 0"));
    }
    let n = usize::from(u16::from_le_bytes([body[0], body[1]]));
    if n > MAX_PRESETS || body.len() != 2 + n * ENTRY {
        return Err(bad("index.tpi: the length does not match the count"));
    }
    let mut out = Vec::with_capacity(n);
    let (entries, _) = body[2..].as_chunks::<ENTRY>();
    for chunk in entries {
        if chunk[0] > MAX_ID {
            return Err(bad("index.tpi: a preset id is over 126"));
        }
        let raw = &chunk[2..];
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        // The firmware keeps at most 15 characters and a NUL: a name filling all 16 bytes is cut at 15.
        let name = String::from_utf8_lossy(&raw[..end.min(NAME_MAX)]).into_owned();
        out.push(IndexEntry {
            id: chunk[0],
            name,
            hidden: chunk[1] & 1 != 0,
        });
    }
    Ok((meter_id, out))
}

/// One preset file: sparse `(key hash, value)` pairs in the order written.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PresetFile {
    pub meter_id: u16,
    pub id: u8,
    pub name: String,
    pub pairs: Vec<(u16, u16)>,
}

/// Builds `p<id>.tpr` from `values` keyed by parameter key (sparse), refusing a key the meter does not have, a value outside
/// its range, a name over 15 characters, an id over 126 and more keys than the firmware's buffer holds.
pub fn build_preset(
    meter_id: u16,
    id: u8,
    name: &str,
    values: &[(String, i64)],
    specs: &[ParamSpec],
) -> Result<Vec<u8>, TauError> {
    if id > MAX_ID {
        return Err(bad("a preset id is 0..126"));
    }
    if name.len() > NAME_MAX || !name.bytes().all(|b| (0x20..0x7F).contains(&b)) {
        return Err(bad(format!(
            "a name of at most 15 printable ASCII characters: {name:?}"
        )));
    }
    if values.len() > MAX_KEYS {
        return Err(bad("a preset file holds at most 64 values"));
    }
    let mut body = vec![values.len() as u8, name.len() as u8];
    body.extend_from_slice(name.as_bytes());
    // Pairs are written in the registry's parameter order, so the same preset is always the same bytes (the factory set
    // is written that way too) and "only what changed" can be decided by comparing bytes.
    let mut ordered: Vec<(usize, u16, u16)> = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for (key, value) in values {
        let (at, spec) = specs
            .iter()
            .enumerate()
            .find(|(_, s)| &s.key == key)
            .ok_or_else(|| bad(format!("{key:?} is not a parameter of this meter")))?;
        if !seen.insert(key) {
            return Err(bad(format!("{key:?} appears twice")));
        }
        if !(spec.min..=spec.max).contains(value) {
            return Err(bad(format!(
                "{key} = {value} is outside {}..{}",
                spec.min, spec.max
            )));
        }
        ordered.push((at, spec.hash, *value as u16));
    }
    ordered.sort_by_key(|o| o.0);
    for (_, hash, value) in ordered {
        body.extend_from_slice(&hash.to_le_bytes());
        body.extend_from_slice(&value.to_le_bytes());
    }
    Ok(wrap(PRE_MAGIC, id, meter_id, &body))
}

pub fn parse_preset(data: &[u8]) -> Result<PresetFile, TauError> {
    if data.len() > 14 + NAME_MAX + 4 * MAX_KEYS {
        return Err(bad("a preset file is larger than the firmware reads"));
    }
    let (id, meter_id, body) = split(data, PRE_MAGIC, "preset")?;
    if id > MAX_ID {
        return Err(bad("a preset id is over 126"));
    }
    let (nkeys, name_len) = (usize::from(body[0]), usize::from(body[1]));
    if name_len > NAME_MAX || body.len() != 2 + name_len + 4 * nkeys {
        return Err(bad("preset: the length does not match the counts"));
    }
    let name = String::from_utf8_lossy(&body[2..2 + name_len]).into_owned();
    let pairs = body[2 + name_len..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| {
            (
                u16::from_le_bytes([c[0], c[1]]),
                u16::from_le_bytes([c[2], c[3]]),
            )
        })
        .collect();
    Ok(PresetFile {
        meter_id,
        id,
        name,
        pairs,
    })
}

/// What the firmware computes from a preset: every parameter at its default, then each key the meter has, clamped. Values in
/// the registry's order; keys the meter does not have are ignored (and counted by the caller if it cares).
pub fn apply(preset: &PresetFile, specs: &[ParamSpec]) -> Vec<i64> {
    let mut vals: Vec<i64> = specs.iter().map(|s| s.default).collect();
    for &(hash, value) in &preset.pairs {
        if let Some(i) = specs.iter().position(|s| s.hash == hash) {
            vals[i] = i64::from(value).clamp(specs[i].min, specs[i].max);
        }
    }
    vals
}

/// A preset as Omega edits it: sparse values by parameter key.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetPreset {
    pub id: u8,
    pub name: String,
    pub hidden: bool,
    pub values: BTreeMap<String, i64>,
}

/// The preset set of one meter, in list order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PresetSet {
    pub meter_id: u16,
    pub presets: Vec<SetPreset>,
}

/// The factory set of a meter from its `meters/<key>/presets.json` (`{"presets": [{"name", "values": {key: value}}]}`): ids
/// are the positions 0.., names upper-cased and cut to 15 characters, and only keys the meter has are kept, exactly as the
/// reference `factory_files` does. The ids are the ones the device remembers, so a factory preset keeps its id for ever.
pub fn factory_set(meter: &RegMeter, presets_json: &[u8]) -> Result<PresetSet, TauError> {
    let v: serde_json::Value = serde_json::from_slice(presets_json)
        .map_err(|e| bad(format!("{}: presets.json is not JSON: {e}", meter.key)))?;
    let list = v
        .get("presets")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| bad(format!("{}: presets.json has no `presets` list", meter.key)))?;
    let mut presets = Vec::new();
    for (i, pr) in list.iter().enumerate() {
        let id = u8::try_from(i)
            .ok()
            .filter(|n| *n <= MAX_ID)
            .ok_or_else(|| bad(format!("{}: more than 127 factory presets", meter.key)))?;
        let name: String = pr
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| bad(format!("{}: a factory preset has no name", meter.key)))?
            .to_uppercase()
            .chars()
            .take(NAME_MAX)
            .collect();
        let mut values = BTreeMap::new();
        for p in &meter.params {
            if let Some(n) = pr
                .get("values")
                .and_then(|m| m.get(&p.key))
                .and_then(|n| n.as_i64().or_else(|| n.as_bool().map(i64::from)))
            {
                values.insert(p.key.clone(), n);
            }
        }
        presets.push(SetPreset {
            id,
            name,
            hidden: false,
            values,
        });
    }
    Ok(PresetSet {
        meter_id: u16::from(meter.id),
        presets,
    })
}

/// The factory set of a meter from the registry's own `presets` list (the same values as `presets.json`, which is where the
/// registry gets them): ids are the positions, names upper-cased and cut to 15, only the meter's keys kept.
pub fn factory_from_registry(meter: &RegMeter) -> Result<PresetSet, TauError> {
    let mut presets = Vec::new();
    for (i, f) in meter.factory.iter().enumerate() {
        let id = u8::try_from(i)
            .ok()
            .filter(|n| *n <= MAX_ID)
            .ok_or_else(|| bad(format!("{}: more than 127 factory presets", meter.key)))?;
        presets.push(SetPreset {
            id,
            name: f.name.to_uppercase().chars().take(NAME_MAX).collect(),
            hidden: false,
            values: f
                .values
                .iter()
                .filter(|(k, _)| meter.params.iter().any(|p| &p.key == *k))
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
        });
    }
    Ok(PresetSet {
        meter_id: u16::from(meter.id),
        presets,
    })
}

/// File name of a preset (`p12.tpr`).
pub fn preset_file_name(id: u8) -> String {
    format!("p{id}.tpr")
}

impl PresetSet {
    /// The files of the set, by path relative to `tau-presets/` (`<meter id>/index.tpi`, `<meter id>/p<id>.tpr`). Every refusal of
    /// the builders applies, and the files are read back through the parsers before they are returned.
    pub fn to_files(&self, specs: &[ParamSpec]) -> Result<BTreeMap<String, Vec<u8>>, TauError> {
        let mut files = BTreeMap::new();
        let entries: Vec<IndexEntry> = self
            .presets
            .iter()
            .map(|p| IndexEntry {
                id: p.id,
                name: p.name.clone(),
                hidden: p.hidden,
            })
            .collect();
        let index = build_index(self.meter_id, &entries)?;
        for p in &self.presets {
            let values: Vec<(String, i64)> =
                p.values.iter().map(|(k, v)| (k.clone(), *v)).collect();
            let bytes = build_preset(self.meter_id, p.id, &p.name, &values, specs)?;
            let back = parse_preset(&bytes)?;
            if back.id != p.id || back.name != p.name || back.pairs.len() != values.len() {
                return Err(bad("a preset did not read back as written"));
            }
            files.insert(
                format!("{}/{}", self.meter_id, preset_file_name(p.id)),
                bytes,
            );
        }
        if parse_index(&index)?.1 != entries {
            return Err(bad("the index did not read back as written"));
        }
        files.insert(format!("{}/{INDEX_NAME}", self.meter_id), index);
        Ok(files)
    }

    /// Reads a meter's folder back: `files` maps a file name (`index.tpi`, `p3.tpr`) to its bytes. A listed preset whose file is
    /// missing or invalid is an error (the device would drop the choice and fall back to DEFAULT).
    pub fn from_files(
        meter_id: u16,
        files: &BTreeMap<String, Vec<u8>>,
        specs: &[ParamSpec],
    ) -> Result<Self, TauError> {
        let index = files
            .get(INDEX_NAME)
            .ok_or_else(|| bad("the meter has no index.tpi"))?;
        let (idx_meter, entries) = parse_index(index)?;
        if idx_meter != meter_id {
            return Err(bad(format!(
                "index.tpi is for meter {idx_meter}, not {meter_id}"
            )));
        }
        let mut presets = Vec::new();
        for e in entries {
            let name = preset_file_name(e.id);
            let bytes = files
                .get(&name)
                .ok_or_else(|| bad(format!("{name} is listed in index.tpi but missing")))?;
            let file = parse_preset(bytes)?;
            if file.meter_id != meter_id || file.id != e.id {
                return Err(bad(format!(
                    "{name} is not preset {} of meter {meter_id}",
                    e.id
                )));
            }
            // Keep every pair the meter knows, as keys; pairs of a key the meter lost are dropped (the device ignores them).
            let mut values = BTreeMap::new();
            for &(hash, value) in &file.pairs {
                if let Some(s) = specs.iter().find(|s| s.hash == hash) {
                    values.insert(s.key.clone(), i64::from(value));
                }
            }
            presets.push(SetPreset {
                id: e.id,
                name: e.name,
                hidden: e.hidden,
                values,
            });
        }
        Ok(Self { meter_id, presets })
    }

    /// The folder of this set as the card holds it: `index.tpi` and `p<id>.tpr`, by file name.
    pub fn folder(&self, specs: &[ParamSpec]) -> Result<BTreeMap<String, Vec<u8>>, TauError> {
        let prefix = format!("{}/", self.meter_id);
        Ok(self
            .to_files(specs)?
            .into_iter()
            .map(|(k, v)| (k.strip_prefix(&prefix).unwrap_or(&k).to_string(), v))
            .collect())
    }

    /// The smallest id not used by a preset of this set and not in `retired` (ids never come back for a different preset: the
    /// device remembers `id + 1`, and a saved tuning is anchored to it).
    pub fn next_free_id(&self, retired: &BTreeSet<u8>) -> Option<u8> {
        let used: BTreeSet<u8> = self.presets.iter().map(|p| p.id).collect();
        (0..=MAX_ID).find(|i| !used.contains(i) && !retired.contains(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param(key: &str, kind: ParamType, min: i64, max: i64, default: i64) -> RegParam {
        let values = if kind == ParamType::Enum {
            vec!["A".into(), "B".into(), "C".into()]
        } else {
            Vec::new()
        };
        RegParam {
            key: key.into(),
            kind,
            min,
            max,
            values,
            default,
        }
    }
    fn meter() -> RegMeter {
        RegMeter {
            key: "demo".into(),
            name: "DEMO".into(),
            factory: Vec::new(),
            id: 16,
            schema: 1,
            list_position: 0,
            params: vec![
                param("view", ParamType::Enum, 0, 0, 1),
                param("layers", ParamType::U8, 1, 6, 3),
                param("on", ParamType::Bool, 0, 1, 1),
                param("custom_outer", ParamType::U16, 0, 100, 0),
            ],
        }
    }

    #[test]
    fn the_key_hash_is_fnv1a_folded_to_16_bits() {
        // Values computed with tools/player2/mpreset.py key_hash.
        assert_eq!(key_hash(""), 0x9DC5 ^ 0x811C);
        assert_ne!(key_hash("view"), key_hash("layers"));
        assert_eq!(key_hash("view"), key_hash("view"));
    }

    #[test]
    fn ranges_follow_the_reference_rules() {
        let s = param_specs(&meter()).unwrap();
        assert_eq!((s[0].min, s[0].max), (0, 2)); // enum: 0..len-1
        assert_eq!((s[1].min, s[1].max), (1, 6));
        assert_eq!((s[2].min, s[2].max, s[2].default), (0, 1, 1)); // bool
        assert_eq!((s[3].min, s[3].max), (0, 65_535)); // a colour: the whole word, whatever the manifest says
    }

    #[test]
    fn a_hash_collision_is_refused() {
        // Find two keys that collide (the 16-bit space makes one quickly) and put them in one meter.
        let mut seen = BTreeMap::new();
        let mut pair = None;
        for i in 0..200_000u32 {
            let k = format!("k{i}");
            if let Some(o) = seen.insert(key_hash(&k), k.clone()) {
                pair = Some((o, k));
                break;
            }
        }
        let (a, b) = pair.unwrap();
        let mut m = meter();
        m.params = vec![
            param(&a, ParamType::U8, 0, 1, 0),
            param(&b, ParamType::U8, 0, 1, 0),
        ];
        assert!(param_specs(&m).unwrap_err().message.contains("same hash"));
    }

    fn set() -> PresetSet {
        let v = |pairs: &[(&str, i64)]| pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        PresetSet {
            meter_id: 16,
            presets: vec![
                SetPreset {
                    id: 4,
                    name: "AURORA".into(),
                    hidden: false,
                    values: v(&[("view", 2), ("layers", 5)]),
                },
                SetPreset {
                    id: 0,
                    name: "MINE [EX]".into(),
                    hidden: true,
                    values: v(&[("custom_outer", 60_000)]),
                },
            ],
        }
    }

    #[test]
    fn a_set_writes_and_reads_back_and_applies_like_the_firmware() {
        let specs = param_specs(&meter()).unwrap();
        let files = set().to_files(&specs).unwrap();
        assert_eq!(
            files.keys().cloned().collect::<Vec<_>>(),
            ["16/index.tpi", "16/p0.tpr", "16/p4.tpr"]
        );
        let flat: BTreeMap<String, Vec<u8>> = files
            .iter()
            .map(|(k, v)| (k.rsplit('/').next().unwrap().to_string(), v.clone()))
            .collect();
        assert_eq!(PresetSet::from_files(16, &flat, &specs).unwrap(), set());
        let aurora = parse_preset(&flat["p4.tpr"]).unwrap();
        // view 2, layers 5, `on` omitted -> its default 1, colour omitted -> 0.
        assert_eq!(apply(&aurora, &specs), vec![2, 5, 1, 0]);
    }

    #[test]
    fn the_writer_refuses_what_the_firmware_would_repair() {
        let specs = param_specs(&meter()).unwrap();
        let one = |k: &str, v: i64| vec![(k.to_string(), v)];
        assert!(build_preset(16, 0, "X", &one("nope", 1), &specs).is_err());
        assert!(build_preset(16, 0, "X", &one("layers", 7), &specs).is_err());
        assert!(build_preset(16, 0, "X", &one("layers", 0), &specs).is_err());
        assert!(build_preset(16, 127, "X", &[], &specs).is_err());
        assert!(build_preset(16, 0, "SIXTEEN CHARS 16", &[], &specs).is_err());
        assert!(build_preset(16, 0, "caf\u{e9}", &[], &specs).is_err());
        let dup = vec![("layers".to_string(), 2), ("layers".to_string(), 3)];
        assert!(build_preset(16, 0, "X", &dup, &specs).is_err());
        let e = |id, name: &str| IndexEntry {
            id,
            name: name.into(),
            hidden: false,
        };
        assert!(build_index(16, &[e(0, "A"), e(0, "B")]).is_err());
        assert!(build_index(16, &[e(127, "A")]).is_err());
        assert!(build_index(16, &[e(1, "")]).is_err());
        assert!(build_index(16, &[e(1, "SIXTEEN CHARS 16")]).is_err());
        let many: Vec<IndexEntry> = (0..128u16).map(|i| e((i % 127) as u8, "A")).collect();
        assert!(build_index(16, &many).is_err());
    }

    #[test]
    fn readers_refuse_damage_the_way_the_firmware_does() {
        let specs = param_specs(&meter()).unwrap();
        let files = set().to_files(&specs).unwrap();
        let idx = &files["16/index.tpi"];
        let pre = &files["16/p4.tpr"];
        // A flipped payload bit, a truncation, a wrong magic and a wrong version are all refused.
        let mut x = idx.clone();
        *x.last_mut().unwrap() ^= 1;
        assert!(parse_index(&x).unwrap_err().message.contains("checksum"));
        assert!(parse_index(&idx[..idx.len() - 1]).is_err());
        assert!(parse_index(pre).is_err());
        let mut x = pre.clone();
        x[4] = 2;
        assert!(parse_preset(&x).unwrap_err().message.contains("version"));
        assert!(parse_preset(&pre[..13]).is_err());
        // The reserved byte is outside the CRC, so only the explicit check catches it.
        let mut x = idx.clone();
        x[5] = 1;
        assert!(parse_index(&x).unwrap_err().message.contains("reserved"));
        // An id over 126 with a valid CRC.
        let mut body = 1u16.to_le_bytes().to_vec();
        body.extend_from_slice(&[127, 0]);
        body.extend_from_slice(&[b'A'; NAME_FIELD]);
        assert!(parse_index(&wrap(IDX_MAGIC, 0, 16, &body)).is_err());
    }

    #[test]
    fn a_listed_preset_that_is_missing_or_for_another_meter_is_an_error() {
        let specs = param_specs(&meter()).unwrap();
        let files = set().to_files(&specs).unwrap();
        let mut flat: BTreeMap<String, Vec<u8>> = files
            .iter()
            .map(|(k, v)| (k.rsplit('/').next().unwrap().to_string(), v.clone()))
            .collect();
        assert!(PresetSet::from_files(17, &flat, &specs).is_err());
        flat.remove("p4.tpr");
        assert!(
            PresetSet::from_files(16, &flat, &specs)
                .unwrap_err()
                .message
                .contains("missing")
        );
    }

    #[test]
    fn ids_are_never_reused_for_another_preset() {
        let s = set();
        let retired: BTreeSet<u8> = [1, 2].into_iter().collect();
        assert_eq!(s.next_free_id(&BTreeSet::new()), Some(1));
        assert_eq!(s.next_free_id(&retired), Some(3));
    }

    #[test]
    fn a_value_outside_the_range_is_clamped_when_applied_and_unknown_keys_are_ignored() {
        let specs = param_specs(&meter()).unwrap();
        // Written by hand (the writer refuses it): layers = 99 (max 6), an unknown key hash, view = 1.
        let unknown = (0..=u16::MAX)
            .find(|h| specs.iter().all(|s| s.hash != *h))
            .unwrap();
        let pre = PresetFile {
            meter_id: 16,
            id: 0,
            name: "X".into(),
            pairs: vec![(specs[1].hash, 99), (unknown, 5), (specs[0].hash, 1)],
        };
        assert_eq!(apply(&pre, &specs), vec![1, 6, 1, 0]);
    }

    #[test]
    fn the_factory_import_upper_cases_and_cuts_names_and_keeps_only_the_meters_keys() {
        let json = br#"{"presets":[{"name":"quiet room that is long","values":{"layers":4,"gone":9,"on":false}}]}"#;
        let set = factory_set(&meter(), json).unwrap();
        assert_eq!(set.presets[0].name, "QUIET ROOM THAT");
        assert_eq!(set.presets[0].name.len(), 15);
        let set = factory_set(
            &meter(),
            br#"{"presets":[{"name":"abcdefghijklmnopq","values":{}}]}"#,
        )
        .unwrap();
        assert_eq!(set.presets[0].name, "ABCDEFGHIJKLMNO");
        let set = factory_set(&meter(), json).unwrap();
        assert_eq!(set.presets[0].values.get("layers"), Some(&4));
        assert_eq!(set.presets[0].values.get("on"), Some(&0));
        assert!(!set.presets[0].values.contains_key("gone"));
    }
}
