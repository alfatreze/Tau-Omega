//! Conversion and ReplayGain: deciding per file what is made (before the plan is reviewed) and making it
//! (when the confirmed plan runs). Sources are only ever read.

use super::*;
use crate::convert::{
    Action, ConvertSettings, Kind, Mp3Quality, ReplayGainMode, convertible_extension,
};
use crate::details::{self, Details};
use std::collections::BTreeMap;
use tau_audio::decode::SourceInfo;
use tau_audio::loudness::{self, REFERENCE_LUFS};
use tau_audio::pipeline::{Pipeline, Target};

const TRACK_GAIN: &str = "REPLAYGAIN_TRACK_GAIN";
const TRACK_PEAK: &str = "REPLAYGAIN_TRACK_PEAK";
const ALBUM_GAIN: &str = "REPLAYGAIN_ALBUM_GAIN";
const ALBUM_PEAK: &str = "REPLAYGAIN_ALBUM_PEAK";

/// What the plan decided about one candidate file.
pub(super) struct Prepared {
    pub action: Action,
    pub reason: String,
    pub info: Option<SourceInfo>,
    pub replaygain: Vec<(String, String)>,
    /// The SHA-256 of the source when this stage already read it (the plan loop reuses it).
    pub sha256: Option<String>,
}

impl Prepared {
    fn copy() -> Self {
        Self {
            action: Action::Copy,
            reason: String::new(),
            info: None,
            replaygain: Vec::new(),
            sha256: None,
        }
    }
}

/// Is this candidate something the conversion or ReplayGain stage looks at?
pub(super) fn wanted_extension(path: &Path, settings: &ConvertSettings) -> bool {
    audio_file(path) || (settings.enabled && convertible_extension(path))
}

/// Measured loudness of one track as it will be written.
#[derive(Clone, Copy)]
struct Measure {
    lufs: f64,
    peak: f64,
    seconds: f64,
}

/// Loudness measured earlier, remembered between runs on the host (never on the card).
struct Cache {
    path: Option<PathBuf>,
    map: BTreeMap<String, Measure>,
    dirty: bool,
}

impl Cache {
    fn open(path: Option<&Path>) -> Self {
        let map = path
            .and_then(|p| fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|v| {
                v.as_object().map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| {
                            let a = v.as_array()?;
                            Some((
                                k.clone(),
                                Measure {
                                    lufs: a.first()?.as_f64()?,
                                    peak: a.get(1)?.as_f64()?,
                                    seconds: a.get(2)?.as_f64()?,
                                },
                            ))
                        })
                        .collect()
                })
            })
            .unwrap_or_default();
        Self {
            path: path.map(Path::to_path_buf),
            map,
            dirty: false,
        }
    }

    fn get(&self, key: &str) -> Option<Measure> {
        self.map.get(key).copied()
    }

    fn put(&mut self, key: String, m: Measure) {
        self.map.insert(key, m);
        self.dirty = true;
    }

    fn save(&self) {
        let (Some(path), true) = (&self.path, self.dirty) else {
            return;
        };
        let value: serde_json::Map<String, serde_json::Value> = self
            .map
            .iter()
            .map(|(k, m)| (k.clone(), serde_json::json!([m.lufs, m.peak, m.seconds])))
            .collect();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let temp = path.with_extension("tmp");
        if fs::write(&temp, serde_json::Value::Object(value).to_string()).is_ok() {
            let _ = fs::rename(&temp, path);
        }
    }
}

fn audio_err(e: tau_audio::AudioError) -> TauError {
    TauError::e(ErrorCode::UnsupportedTags, e.to_string())
}

/// Duration-weighted energy mean of the tracks' integrated loudness: deterministic, and computable from
/// remembered per-track values.
fn album_lufs(tracks: &[Measure]) -> Option<f64> {
    let total: f64 = tracks.iter().map(|m| m.seconds).sum();
    if total <= 0.0 {
        return None;
    }
    let energy: f64 = tracks
        .iter()
        .filter(|m| m.lufs.is_finite())
        .map(|m| m.seconds * 10f64.powf(m.lufs / 10.0))
        .sum();
    (energy > 0.0).then(|| 10.0 * (energy / total).log10())
}

fn existing_flags(path: &Path) -> u8 {
    if audio_file(path) {
        details::extract(path).map_or(0, |d: Details| d.flags)
    } else {
        0
    }
}

/// The decisions for every candidate, including the ReplayGain values to write. With conversion and
/// ReplayGain both off this is a no-op (every file is copied as it is).
pub(super) fn prepare(
    candidates: &[(PathBuf, PathBuf)],
    settings: &ConvertSettings,
    warnings: &mut Vec<Warning>,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<Vec<Prepared>, TauError> {
    let active = settings.enabled || settings.replaygain != ReplayGainMode::Off;
    let mut out: Vec<Prepared> = candidates.iter().map(|_| Prepared::copy()).collect();
    if !active {
        return Ok(out);
    }
    let lame = if settings.enabled {
        settings.find_lame()
    } else {
        None
    };
    let mut lame_missing = false;
    for (i, (source, _)) in candidates.iter().enumerate() {
        if !wanted_extension(source, settings) {
            continue;
        }
        let d = if settings.enabled || audio_file(source) {
            crate::convert::decide(source, settings, lame.is_some())
        } else {
            continue;
        };
        if matches!(&d.action, Action::Skip(r) if r.contains("needs LAME")) {
            lame_missing = true;
        }
        out[i].action = d.action;
        out[i].reason = d.reason;
        out[i].info = d.info;
    }
    if lame_missing {
        warnings.push(Warning::new(
            WarningCode::WillNotPlay,
            "Some files need LAME to be converted to MP3. Install LAME from Settings, then plan again.",
        ));
    }
    if settings.replaygain == ReplayGainMode::Off {
        return Ok(out);
    }

    // ReplayGain: per destination folder (an album).
    let mut folders: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    for (i, (_, relative)) in candidates.iter().enumerate() {
        if matches!(out[i].action, Action::Skip(_)) || !wanted_extension(&candidates[i].0, settings)
        {
            continue;
        }
        folders
            .entry(relative.parent().map(Path::to_path_buf).unwrap_or_default())
            .or_default()
            .push(i);
    }
    let mut cache = Cache::open(settings.analysis_cache.as_deref());
    let total: u64 = folders.values().map(|v| v.len() as u64).sum();
    let mut done = 0u64;
    for members in folders.values() {
        // Which tags each track already has (a converted file starts with none).
        let flags: Vec<u8> = members
            .iter()
            .map(|&i| {
                if matches!(out[i].action, Action::Convert(_)) {
                    0
                } else {
                    existing_flags(&candidates[i].0)
                }
            })
            .collect();
        let want = |f: u8| match settings.replaygain {
            ReplayGainMode::Rebuild => true,
            _ => {
                f & (details::F_RG_TRACK_GAIN
                    | details::F_RG_TRACK_PEAK
                    | details::F_RG_ALBUM_GAIN
                    | details::F_RG_ALBUM_PEAK)
                    != (details::F_RG_TRACK_GAIN
                        | details::F_RG_TRACK_PEAK
                        | details::F_RG_ALBUM_GAIN
                        | details::F_RG_ALBUM_PEAK)
            }
        };
        if !flags.iter().any(|f| want(*f)) {
            done += members.len() as u64;
            continue;
        }
        let mut measures = Vec::new();
        let mut ok = true;
        for &i in members {
            let (source, relative) = &candidates[i];
            tick(
                progress,
                Progress {
                    stage: Stage::Analysing,
                    done,
                    total,
                    path: Some(relative.to_string_lossy().into_owned()),
                },
            )?;
            done += 1;
            let sha = sha256_file(source)?;
            out[i].sha256 = Some(sha.clone());
            let (target_spec, target) = match (&out[i].action, &out[i].info) {
                (Action::Convert(kind), _) => (
                    kind.spec(),
                    Some(Target {
                        rate: kind.rate(),
                        channels: kind.channels(),
                    }),
                ),
                (_, Some(info)) => (
                    "native".to_string(),
                    Some(Target::native(&tau_audio::decode::SourceInfo {
                        ..info.clone()
                    })),
                ),
                _ => ("native".to_string(), None),
            };
            let key = format!("{sha}|{target_spec}");
            let m = match cache.get(&key) {
                Some(m) => Some(m),
                None => measure_file(source, target)
                    .ok()
                    .inspect(|m| cache.put(key, *m)),
            };
            match m {
                Some(m) => measures.push(m),
                None => {
                    ok = false;
                    warnings.push(Warning::new(
                        WarningCode::TagsUnreadable,
                        format!("{}: loudness could not be measured, so no ReplayGain was added to this album", relative.display()),
                    ));
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        let album = album_lufs(&measures);
        let album_peak = measures.iter().fold(0.0f64, |a, m| a.max(m.peak));
        for (n, &i) in members.iter().enumerate() {
            let f = flags[n];
            let all = settings.replaygain == ReplayGainMode::Rebuild;
            let mut tags = Vec::new();
            if all || f & details::F_RG_TRACK_GAIN == 0 {
                tags.push((
                    TRACK_GAIN.to_string(),
                    loudness::format_gain(loudness::gain_db(measures[n].lufs)),
                ));
            }
            if all || f & details::F_RG_TRACK_PEAK == 0 {
                tags.push((
                    TRACK_PEAK.to_string(),
                    loudness::format_peak(measures[n].peak),
                ));
            }
            if let Some(album) = album {
                if all || f & details::F_RG_ALBUM_GAIN == 0 {
                    tags.push((
                        ALBUM_GAIN.to_string(),
                        loudness::format_gain(loudness::gain_db(album)),
                    ));
                }
                if all || f & details::F_RG_ALBUM_PEAK == 0 {
                    tags.push((ALBUM_PEAK.to_string(), loudness::format_peak(album_peak)));
                }
            }
            out[i].replaygain = tags;
        }
    }
    cache.save();
    let _ = REFERENCE_LUFS;
    Ok(out)
}

fn measure_file(path: &Path, target: Option<Target>) -> Result<Measure, tau_audio::AudioError> {
    let info = tau_audio::decode::probe(path)?;
    let target = target.unwrap_or_else(|| Target::native(&info));
    let mut pipeline = Pipeline::open(path, target)?;
    let m = loudness::measure(&mut pipeline, &mut |_| true)?;
    let seconds = pipeline.frames_read as f64 / info.rate.max(1) as f64;
    Ok(Measure {
        lufs: m.track_lufs,
        peak: m.peak,
        seconds,
    })
}

/// The source's cover for an embedded picture: the first picture the file carries.
fn source_meta(path: &Path) -> tau_audio::meta::Meta {
    tau_audio::decode::Decoder::open(path)
        .map(|mut d| d.meta())
        .unwrap_or_default()
}

fn flac_comments(meta: &tau_audio::meta::Meta, rg: &[(String, String)]) -> Vec<(String, String)> {
    let mut c = Vec::new();
    let mut add = |k: &str, v: Option<String>| {
        if let Some(v) = v.filter(|v| !v.is_empty()) {
            c.push((k.to_string(), v));
        }
    };
    add("TITLE", meta.title.clone());
    add("ARTIST", meta.artist.clone());
    add("ALBUM", meta.album.clone());
    add("ALBUMARTIST", meta.album_artist.clone());
    add("TRACKNUMBER", meta.track.map(|n| n.to_string()));
    add("DISCNUMBER", meta.disc.map(|n| n.to_string()));
    add("DATE", meta.year.clone());
    add("GENRE", meta.genre.clone());
    c.extend(rg.iter().cloned());
    c
}

/// Makes the converted file at `temp`, checks it, and returns its tags-included SHA-256 later computed by the
/// caller. `frames_done` is called with the frames written so far; it returns `false` to cancel.
pub(super) fn convert_to(
    item: &CopyItem,
    transform: &Transform,
    kind: &Kind,
    settings_lame: Option<&Path>,
    temp: &Path,
    frames_done: &mut dyn FnMut(u64) -> bool,
) -> Result<(), TauError> {
    let meta = source_meta(&item.source);
    let target = Target {
        rate: kind.rate(),
        channels: kind.channels(),
    };
    let mut pipeline = Pipeline::open(&item.source, target).map_err(audio_err)?;
    let encoded = match kind {
        Kind::Flac { bits, dither, .. } => {
            let picture = meta
                .picture
                .as_ref()
                .map(|(mime, data)| tau_audio::flac_out::Picture { mime, data });
            tau_audio::flac_out::encode(
                &mut pipeline,
                *bits,
                *dither,
                &flac_comments(&meta, &transform.replaygain),
                picture,
                temp,
                frames_done,
            )
            .map_err(audio_err)?
        }
        Kind::Mp3 { quality, .. } => {
            let lame = settings_lame
                .ok_or_else(|| TauError::e(ErrorCode::UnsupportedTags, "LAME is not installed"))?;
            let tags = tau_audio::lame::Tags {
                title: meta.title.clone(),
                artist: meta.artist.clone(),
                album: meta.album.clone(),
                year: meta.year.clone(),
                track: meta.track.map(|n| n.to_string()),
                genre: meta.genre.clone(),
            };
            // lame refuses large pictures; the Pocket reads the cover file, so a big one is simply not embedded
            let cover = meta
                .picture
                .as_ref()
                .map(|(_, data)| data.as_slice())
                .filter(|d| d.len() <= 120_000);
            let q: tau_audio::lame::Quality = match quality {
                Mp3Quality::Vbr(v) => tau_audio::lame::Quality::Vbr(*v),
                Mp3Quality::Cbr(k) => tau_audio::lame::Quality::Cbr(*k),
            };
            let encoded =
                tau_audio::lame::encode(lame, &mut pipeline, q, &tags, cover, temp, frames_done)
                    .map_err(audio_err)?;
            if !transform.replaygain.is_empty() {
                let bytes = fs::read(temp)?;
                let tagged = crate::tagedit::set_custom_tags(&bytes, false, &transform.replaygain)?;
                write_durable(temp, &tagged)?;
            }
            encoded
        }
    };
    // Check what was written: it opens, has the rate and channels asked for, and the right length.
    let back = tau_audio::decode::probe(temp).map_err(audio_err)?;
    if back.rate != kind.rate() || back.channels != kind.channels() {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            format!(
                "the converted file does not have the requested format: {}",
                item.source.display()
            ),
        ));
    }
    if let Some(expected) = transform.frames
        && let Kind::Flac { .. } = kind
        && back.frames.is_some_and(|f| f != encoded.frames)
    {
        return Err(TauError::e(
            ErrorCode::VerificationFailed,
            format!(
                "the converted file has the wrong length ({expected} frames expected): {}",
                item.source.display()
            ),
        ));
    }
    Ok(())
}

/// Bytes of an ordinary copy with ReplayGain tags added (and the album cover embedded when planned).
pub(super) fn tagged_bytes(item: &CopyItem, transform: &Transform) -> Result<Vec<u8>, TauError> {
    let base = match &item.cover {
        Some(cover) => match item
            .source
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("mp3") => cover::embed_mp3_bytes(&item.source, &cover.source)?,
            Some("flac") => cover::embed_flac_bytes(&item.source, &cover.source)?,
            _ => unreachable!("only audio files receive cover plans"),
        },
        None => fs::read(&item.source)?,
    };
    let is_flac = item
        .source
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("flac"));
    crate::tagedit::set_custom_tags(&base, is_flac, &transform.replaygain)
}
