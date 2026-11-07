//! The `METR` section of `tau-assets.bin`: per-meter preset sets (Tau Alpha `docs/THEME_FILE_FORMAT.md`, reference writer and reader
//! `tools/tau_assets.py`). A file can replace a meter's presets (1 to 8, names of at most 15 characters of `A-Z 0-9 space _ -`) and pick
//! its boot preset; it cannot add a meter or change a parameter's range: the registry (`tools/meters_schema.json`, passed in as
//! [`MeterRegistry`]) and the firmware's compiled tables are the authority. This module only builds and reads the section bytes; the
//! container that carries it is `assets` (which already keeps unknown sections byte for byte).
//!
//! Wire format (little endian), checked byte for byte against the Python reference in `tests/meter_presets.rs`:
//! `"TMTR" | version u16 = 1 | 2 pad | crc32(everything after the 12-byte header) u32 | count u16 |` then per meter
//! `len u16 | id u8 | schema u8 | flags u8 | order u8 | npre u8 | nparams u8 | npre x { name[16] | values } | default_preset u8`
//! (one byte per `u8`, `bool` and `enum` parameter, two per `u16`, in the registry's parameter order).

use crate::{ErrorCode, TauError};
use serde_json::Value;
use std::collections::BTreeMap;

pub const MAX_PRESETS: usize = 8;
const NAME_LEN: usize = 16;
const VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ParamType {
    U8,
    U16,
    Bool,
    Enum,
}

/// One parameter of a meter, as the registry lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RegParam {
    pub key: String,
    pub kind: ParamType,
    pub min: i64,
    pub max: i64,
    /// The names of an enum's values (the legal range is `0..len`).
    pub values: Vec<String>,
    /// The value a meter runs on when no preset sets this parameter (the registry's `default`, else the minimum).
    pub default: i64,
}

impl RegParam {
    fn width(&self) -> usize {
        if self.kind == ParamType::U16 { 2 } else { 1 }
    }
    fn range(&self) -> (i64, i64) {
        match self.kind {
            ParamType::Bool => (0, 1),
            ParamType::Enum => (0, self.values.len() as i64 - 1),
            _ => (self.min, self.max),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RegMeter {
    pub key: String,
    /// The display name (`LAYERED WAVE`), the key when the registry has none.
    pub name: String,
    pub id: u8,
    pub schema: u8,
    pub list_position: u8,
    pub params: Vec<RegParam>,
    /// The factory presets the registry lists (`presets`: name and values by key, in list order; ids are the positions).
    pub factory: Vec<FactoryPreset>,
}

/// One factory preset of a meter as the registry lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FactoryPreset {
    pub name: String,
    pub values: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeterRegistry {
    pub meters: Vec<RegMeter>,
}

fn bad(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InvalidPathReference, msg)
}

impl MeterRegistry {
    /// Reads `tools/meters_schema.json`'s `meters` list (`key`, `id`, `schema`, `list_position`, `params[{key,type,min,max,values}]`).
    pub fn from_json(bytes: &[u8]) -> Result<Self, TauError> {
        let v: Value = serde_json::from_slice(bytes)
            .map_err(|e| bad(format!("the meter registry is not JSON: {e}")))?;
        // `meters_schema.json` has the list at its root; the lab drop's `lab-data.json` may carry that same object under a key.
        let v = if v.get("meters").is_some() {
            v
        } else {
            ["schema", "meters_schema", "registry"]
                .iter()
                .find_map(|k| v.get(*k).filter(|n| n.get("meters").is_some()).cloned())
                .unwrap_or(v)
        };
        let list = v
            .get("meters")
            .and_then(Value::as_array)
            .ok_or_else(|| bad("the meter registry has no `meters` list"))?;
        let mut meters = Vec::new();
        for m in list {
            let key = m
                .get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("a meter has no key"))?
                .to_string();
            let int = |v: &Value, k: &str| v.get(k).and_then(Value::as_i64);
            let id = int(m, "id")
                .and_then(|n| u8::try_from(n).ok())
                .ok_or_else(|| bad(format!("{key}: no usable id")))?;
            let mut params = Vec::new();
            for p in m
                .get("params")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let pkey = p
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad(format!("{key}: a parameter has no key")))?;
                let kind = match p.get("type").and_then(Value::as_str) {
                    Some("u8") => ParamType::U8,
                    Some("u16") => ParamType::U16,
                    Some("bool") => ParamType::Bool,
                    Some("enum") => ParamType::Enum,
                    other => {
                        return Err(bad(format!(
                            "{key}/{pkey}: unknown parameter type {other:?}"
                        )));
                    }
                };
                params.push(RegParam {
                    key: pkey.to_string(),
                    kind,
                    min: int(p, "min").unwrap_or(0),
                    max: int(p, "max").unwrap_or(0),
                    values: p
                        .get("values")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default(),
                    default: p
                        .get("default")
                        .and_then(|d| d.as_i64().or_else(|| d.as_bool().map(i64::from)))
                        .unwrap_or_else(|| int(p, "min").unwrap_or(0)),
                });
            }
            let name = m
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(&key)
                .to_string();
            let factory = m
                .get("presets")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|pr| FactoryPreset {
                    name: pr
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    values: pr
                        .get("values")
                        .and_then(Value::as_object)
                        .into_iter()
                        .flatten()
                        .filter_map(|(k, v)| {
                            v.as_i64()
                                .or_else(|| v.as_bool().map(i64::from))
                                .map(|n| (k.clone(), n))
                        })
                        .collect(),
                })
                .collect();
            meters.push(RegMeter {
                key,
                name,
                id,
                schema: int(m, "schema")
                    .and_then(|n| u8::try_from(n).ok())
                    .unwrap_or(1),
                list_position: int(m, "list_position")
                    .and_then(|n| u8::try_from(n).ok())
                    .unwrap_or(0),
                params,
                factory,
            });
        }
        Ok(Self { meters })
    }
    pub fn find(&self, key: &str) -> Option<&RegMeter> {
        self.meters.iter().find(|m| m.key == key)
    }
}

/// One named preset: a value for every parameter of its meter.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Preset {
    pub name: String,
    pub values: BTreeMap<String, i64>,
}

/// The preset set of one meter and which preset it boots into.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeterPresets {
    pub key: String,
    pub presets: Vec<Preset>,
    pub default_preset: u8,
}

/// Whether `name` is a legal preset name: 1 to 15 of `A-Z 0-9 space _ -` (what the firmware keeps; anything else it would rewrite).
pub fn name_ok(name: &str) -> bool {
    (1..=15).contains(&name.len())
        && name.bytes().all(|b| {
            b.is_ascii_uppercase() || b.is_ascii_digit() || matches!(b, b' ' | b'_' | b'-')
        })
}

/// Builds the `METR` section. Validates exactly as Tau's writer does: the meter must be in the registry and have parameters, 1..=8
/// presets with unique legal names, every parameter given and inside its range (the writer refuses what the firmware would clamp).
pub fn pack_meters(reg: &MeterRegistry, sets: &[MeterPresets]) -> Result<Vec<u8>, TauError> {
    let mut body = Vec::new();
    body.extend_from_slice(&(sets.len() as u16).to_le_bytes());
    for e in sets {
        let m = reg
            .find(&e.key)
            .filter(|m| !m.params.is_empty())
            .ok_or_else(|| {
                bad(format!(
                    "meter {:?} is not in the registry or has no parameters",
                    e.key
                ))
            })?;
        if !(1..=MAX_PRESETS).contains(&e.presets.len()) {
            return Err(bad(format!("{}: 1..{MAX_PRESETS} presets", e.key)));
        }
        if usize::from(e.default_preset) >= e.presets.len() {
            return Err(bad(format!("{}: default_preset out of range", e.key)));
        }
        let mut seen = std::collections::BTreeSet::new();
        for p in &e.presets {
            if !name_ok(&p.name) || !seen.insert(p.name.as_str()) {
                return Err(bad(format!(
                    "{}: preset names must be unique, 1..15 of A-Z 0-9 space _ -",
                    e.key
                )));
            }
        }
        let mut ent = vec![
            m.id,
            m.schema,
            0,
            m.list_position,
            e.presets.len() as u8,
            m.params.len() as u8,
        ];
        for p in &e.presets {
            let given: std::collections::BTreeSet<&str> =
                p.values.keys().map(String::as_str).collect();
            let want: std::collections::BTreeSet<&str> =
                m.params.iter().map(|x| x.key.as_str()).collect();
            if given != want {
                return Err(bad(format!(
                    "{}/{}: must give exactly {:?}",
                    e.key,
                    p.name,
                    m.params.iter().map(|x| x.key.as_str()).collect::<Vec<_>>()
                )));
            }
            let mut name = [0u8; NAME_LEN];
            name[..p.name.len()].copy_from_slice(p.name.as_bytes());
            ent.extend_from_slice(&name);
            for x in &m.params {
                let v = p.values[&x.key];
                let (lo, hi) = x.range();
                if !(lo..=hi).contains(&v) {
                    return Err(bad(format!(
                        "{}/{}: {}={v} outside {lo}..{hi}",
                        e.key, p.name, x.key
                    )));
                }
                ent.extend_from_slice(&(v as u16).to_le_bytes()[..x.width()]);
            }
        }
        ent.push(e.default_preset);
        body.extend_from_slice(&(ent.len() as u16).to_le_bytes());
        body.extend_from_slice(&ent);
    }
    let mut out = b"TMTR".to_vec();
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&crc32fast::hash(&body).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// What a reader found for one meter entry.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParsedMeter {
    pub id: u8,
    pub schema: u8,
    pub flags: u8,
    pub order: u8,
    /// The meter the registry names for this id (`None` when the registry does not know it: the firmware skips it too).
    pub key: Option<String>,
    /// An entry the firmware would skip (unknown id, another parameter count, a newer schema).
    pub skipped: bool,
    pub presets: Vec<Preset>,
    pub default_preset: u8,
}

/// Reads a `METR` section (CRC and header first). Entries the firmware would skip are returned with `skipped`.
pub fn parse_meters(reg: &MeterRegistry, d: &[u8]) -> Result<Vec<ParsedMeter>, TauError> {
    if d.len() < 14 || &d[..4] != b"TMTR" {
        return Err(bad("bad METR magic"));
    }
    if u16::from_le_bytes([d[4], d[5]]) != VERSION {
        return Err(bad("bad METR version"));
    }
    if crc32fast::hash(&d[12..]) != u32::from_le_bytes([d[8], d[9], d[10], d[11]]) {
        return Err(bad("METR CRC mismatch"));
    }
    let count = usize::from(u16::from_le_bytes([d[12], d[13]]));
    let (mut pos, mut out) = (14usize, Vec::new());
    for _ in 0..count {
        let ln = d
            .get(pos..pos + 2)
            .map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])))
            .ok_or_else(|| bad("METR entry runs past the end"))?;
        let e = d
            .get(pos + 2..pos + 2 + ln)
            .ok_or_else(|| bad("METR entry runs past the end"))?;
        pos += 2 + ln;
        if e.len() < 6 {
            return Err(bad("METR entry too short"));
        }
        let (id, schema, flags, order, npre, nparams) =
            (e[0], e[1], e[2], e[3], usize::from(e[4]), usize::from(e[5]));
        let m = reg.meters.iter().find(|m| m.id == id);
        let mut item = ParsedMeter {
            id,
            schema,
            flags,
            order,
            key: m.map(|m| m.key.clone()),
            skipped: false,
            presets: Vec::new(),
            default_preset: 0,
        };
        let Some(m) = m.filter(|m| m.params.len() == nparams && schema <= m.schema) else {
            item.skipped = true;
            out.push(item);
            continue;
        };
        let mut q = 6;
        for _ in 0..npre {
            let name_bytes = e
                .get(q..q + NAME_LEN)
                .ok_or_else(|| bad("METR preset runs past the end"))?;
            let name =
                String::from_utf8_lossy(name_bytes.split(|b| *b == 0).next().unwrap_or_default())
                    .into_owned();
            q += NAME_LEN;
            let mut values = BTreeMap::new();
            for p in &m.params {
                let w = p.width();
                let raw = e
                    .get(q..q + w)
                    .ok_or_else(|| bad("METR value runs past the end"))?;
                values.insert(
                    p.key.clone(),
                    if w == 2 {
                        i64::from(u16::from_le_bytes([raw[0], raw[1]]))
                    } else {
                        i64::from(raw[0])
                    },
                );
                q += w;
            }
            item.presets.push(Preset { name, values });
        }
        item.default_preset = *e
            .get(q)
            .ok_or_else(|| bad("METR entry has no default preset"))?;
        out.push(item);
    }
    Ok(out)
}
