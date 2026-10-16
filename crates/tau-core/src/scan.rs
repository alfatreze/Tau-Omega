//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Entry {
    pub rel: String,
    pub dir: String,
    pub file: String,
    pub tags: BTreeMap<String, String>,
    pub secs: u16,
    pub fmt: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Playlist {
    pub name: String,
    pub rel_ids: Vec<usize>,
    /// The `.m3u` file this playlist was read from, relative to the media
    /// root -- lets a front-end target rename/reorder/import operations at a
    /// specific file without re-deriving the scan's own naming rules.
    pub file: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Scan {
    pub entries: Vec<Entry>,
    pub playlists: Vec<Playlist>,
    pub warnings: Vec<Warning>,
    /// Files whose tags came from the verification ledger instead of being read (0 without a ledger).
    pub reused: u64,
    /// Files whose tags were read from the file itself.
    pub read: u64,
}

/// Returns printable device ASCII. Latin characters needed by common music tags
/// are folded explicitly; unsupported Unicode is dropped just as the reference does.
pub fn ascii_text(input: &str, limit: usize) -> String {
    let mut out = String::new();
    for c in input.chars() {
        let folded = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä'
            | 'Å' | 'Ā' | 'Ă' | 'Ą' => "a",
            'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' | 'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => "c",
            'ď' | 'đ' | 'Ð' | 'Ď' | 'Đ' => "d",
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' | 'È' | 'É' | 'Ê' | 'Ë' | 'Ē'
            | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "e",
            'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī'
            | 'Ĭ' | 'Į' => "i",
            'ñ' | 'ń' | 'ņ' | 'ň' | 'Ñ' | 'Ń' | 'Ņ' | 'Ň' => "n",
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö'
            | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => "o",
            'ß' => "ss",
            'ŕ' | 'ŗ' | 'ř' | 'Ŕ' | 'Ŗ' | 'Ř' => "r",
            'ś' | 'ŝ' | 'ş' | 'š' | 'Ś' | 'Ŝ' | 'Ş' | 'Š' => "s",
            'ţ' | 'ť' | 'ŧ' | 'Ţ' | 'Ť' | 'Ŧ' => "t",
            'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'Ù' | 'Ú' | 'Û' | 'Ü'
            | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => "u",
            'ý' | 'ÿ' | 'ŷ' | 'Ý' | 'Ÿ' | 'Ŷ' => "y",
            'ź' | 'ż' | 'ž' | 'Ź' | 'Ż' | 'Ž' => "z",
            x if x.is_ascii() && !x.is_ascii_control() => {
                out.push(x);
                continue;
            }
            x if x.is_whitespace() => " ",
            _ => "",
        };
        out.push_str(folded);
    }
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(limit)
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn ascii_name(input: &str) -> String {
    let mut name = ascii_text(input, usize::MAX);
    for bad in ['<', '>', ':', '"', '|', '?', '*', '\\'] {
        name = name.replace(bad, "_");
    }
    name.trim().to_string()
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) enum Token {
    Number(u64),
    Text(String),
}
impl Ord for Token {
    fn cmp(&self, o: &Self) -> Ordering {
        match (self, o) {
            (Self::Number(a), Self::Number(b)) => a.cmp(b),
            (Self::Text(a), Self::Text(b)) => a.cmp(b),
            (Self::Number(_), Self::Text(_)) => Ordering::Less,
            (Self::Text(_), Self::Number(_)) => Ordering::Greater,
        }
    }
}
impl PartialOrd for Token {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
pub(super) fn tokens(input: &str) -> Vec<Token> {
    let mut parts = Vec::new();
    let mut buf = String::new();
    let mut digits: Option<bool> = None;
    for c in input.to_lowercase().chars() {
        let d = c.is_ascii_digit();
        if digits.is_some_and(|old| old != d) {
            parts.push(if digits == Some(true) {
                Token::Number(buf.parse().unwrap_or(0))
            } else {
                Token::Text(buf.clone())
            });
            buf.clear();
        }
        digits = Some(d);
        buf.push(c);
    }
    if !buf.is_empty() {
        parts.push(if digits == Some(true) {
            Token::Number(buf.parse().unwrap_or(0))
        } else {
            Token::Text(buf)
        });
    }
    parts
}
pub(super) fn sort_key(input: &str) -> (u8, Vec<Token>) {
    let mut t = input.trim().to_lowercase();
    for article in ["the ", "a "] {
        if t.starts_with(article) && t.len() > article.len() {
            t = t[article.len()..].to_string();
            break;
        }
    }
    let class = t
        .as_bytes()
        .first()
        .filter(|b| b.is_ascii_lowercase())
        .map(|b| b - b'a' + 1)
        .unwrap_or(0);
    (class, tokens(&t))
}
pub(super) fn natural(input: &str) -> Vec<Token> {
    tokens(input)
}

/// Scans a destination media root. Source paths remain read-only; this operation only reads tags.
pub fn scan_dir(common: &Path, playlists: bool) -> Result<Scan, TauError> {
    scan_dir_with_progress(common, playlists, &mut None)
}

/// Same as [`scan_dir`], reporting per-file progress through an optional
/// observer that can also cancel the scan (see [`ProgressObserver`]).
pub fn scan_dir_with_progress(
    common: &Path,
    playlists: bool,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<Scan, TauError> {
    scan_dir_options(
        common,
        playlists,
        ScanKind {
            convertible: false,
            details: true,
            silence: false,
        },
        progress,
    )
}

/// A scan that also measures the silence at the start and end of each track not measured before, when Skip silence is on. Only the
/// scans that write the skip tables ask for it: a plain read of a library must never pay for a decode of every track.
pub fn scan_dir_measuring(
    common: &Path,
    playlists: bool,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<Scan, TauError> {
    scan_dir_options(
        common,
        playlists,
        ScanKind {
            convertible: false,
            details: true,
            silence: crate::trim::active().enabled,
        },
        progress,
    )
}

/// A scan for listing a library: titles, artists, albums and lengths only. It skips the technical details the card's index needs, so a
/// first read of a big library, or of one on a network share, opens each file once and reads as little of it as it can.
pub fn scan_dir_listing(
    common: &Path,
    playlists: bool,
    convertible: bool,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<Scan, TauError> {
    scan_dir_options(
        common,
        playlists,
        ScanKind {
            convertible,
            details: false,
            silence: false,
        },
        progress,
    )
}

/// What one scan does beyond reading the tags.
#[derive(Clone, Copy)]
struct ScanKind {
    /// Also list the audio files the Pocket cannot play but can be converted (a source library only).
    convertible: bool,
    /// Read the technical details of each track (the index needs them; a plain listing does not).
    details: bool,
    /// Measure the silence at the start and end of each track not measured before.
    silence: bool,
}

/// How many files are read at once. A network share is waiting on the network, not on the processor, so more
/// readers than cores still pay off; measuring silence is processor work and gains from every core.
fn scan_threads() -> usize {
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    (cores * 2).clamp(4, 16)
}

/// Runs `f` over `items` on several threads and returns the results in the order of the items.
fn par_map<T: Sync, R: Send>(items: &[T], threads: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let (tx, rx) = std::sync::mpsc::channel::<(usize, R)>();
    let mut out: Vec<Option<R>> = (0..items.len()).map(|_| None).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads.min(items.len()).max(1) {
            let (tx, next, f) = (tx.clone(), &next, &f);
            scope.spawn(move || {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= items.len() || tx.send((i, f(&items[i]))).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);
        for (i, r) in rx {
            out[i] = Some(r);
        }
    });
    out.into_iter().flatten().collect()
}

type TagRead = Result<(BTreeMap<String, String>, u16, u8), String>;

/// Reads one track from the file: tags, duration and format, plus the details and the silence when asked.
fn read_track(path: &Path, convert: bool, kind: ScanKind) -> TagRead {
    if convert {
        return crate::workbench::read_convertible(path);
    }
    let mut result = read_tags(path).map_err(|e| e.to_string())?;
    // The track's technical details travel in the tag map (hex, plus the encoder text) so the ledger cache keeps
    // them with the rest.
    if kind.details
        && let Ok(det) = crate::details::extract(path)
    {
        result.0.insert("_det".into(), det.to_hex());
        result.0.insert("_denc".into(), det.encoder);
    }
    // Leading and trailing silence at every level, only while trimming is switched on. A file that cannot be
    // measured is marked so it is not tried again on every scan.
    if kind.silence {
        let measured = tau_audio::silence::measure_levels(path, &mut |_| true)
            .ok()
            .and_then(|l| crate::trim::encode_levels(&l));
        match measured {
            Some(text) => result.0.insert("_sil".into(), text),
            None => result.0.insert("_silx".into(), "1".into()),
        };
    }
    Ok(result)
}

fn scan_dir_options(
    common: &Path,
    playlists: bool,
    kind: ScanKind,
    progress: &mut Option<&mut dyn ProgressObserver>,
) -> Result<Scan, TauError> {
    let mut files = Vec::new();
    collect_files(common, common, &mut files)?;
    files.sort();
    let mut output = Scan::default();
    // First pass: which files are media (no file access), then what each looks like (one `stat` each, several at a
    // time: on a network share these are round trips). The ledger needs the whole set before it can say whether
    // this card's modified times mean anything.
    struct Media {
        path: PathBuf,
        rel: String,
        is_flac: bool,
        /// A file the Pocket cannot play but this tool can convert: read through the decoder, listed with format 0.
        convert: bool,
        print: Option<ledger::Fingerprint>,
    }
    let mut media: Vec<Media> = Vec::new();
    for path in files {
        let extension = path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let convert = kind.convertible
            && !matches!(extension.as_str(), "mp3" | "flac")
            && crate::convert::convertible_extension(&path);
        if !(matches!(extension.as_str(), "mp3" | "flac") || convert)
            || path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("._"))
        {
            continue;
        }
        let rel = path
            .strip_prefix(common)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        media.push(Media {
            path,
            rel,
            is_flac: extension == "flac",
            convert,
            print: None,
        });
    }
    let total = media.len() as u64;
    let threads = scan_threads();
    let prints = par_map(&media, threads, |m| {
        fs::metadata(&m.path)
            .ok()
            .and_then(|md| ledger::fingerprint(&md))
    });
    for (m, p) in media.iter_mut().zip(prints) {
        m.print = p;
    }
    let mut cache = ledger::Session::open(common);
    if let Some(session) = cache.as_mut() {
        let prints: Vec<_> = media.iter().filter_map(|m| m.print).collect();
        session.assess(&prints);
    }
    // What the ledger already knows. A record from before details existed has no `_det` tag, and one from before
    // silence was measured has no `_sil`: those are read again, only when this scan needs them.
    let cached: Vec<Option<ledger::TagRecord>> = media
        .iter()
        .map(|m| match (cache.as_mut(), m.print.as_ref()) {
            (Some(session), Some(print)) => session.tags(&m.rel, print),
            _ => None,
        })
        .map(|hit| {
            hit.filter(|h| {
                (!kind.details || h.tags.contains_key("_det"))
                    && (!kind.silence
                        || h.tags.contains_key("_sil")
                        || h.tags.contains_key("_silx"))
            })
        })
        .collect();
    let misses: Vec<usize> = (0..media.len()).filter(|&i| cached[i].is_none()).collect();
    // Read what is not known, several files at a time. Results come back to this thread as they finish, so the
    // progress observer (which is not shared between threads) is only ever called here, and a cancel stops the workers.
    let mut read: Vec<Option<TagRead>> = (0..media.len()).map(|_| None).collect();
    if !misses.is_empty() {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let next = std::sync::atomic::AtomicUsize::new(0);
        let (tx, rx) = std::sync::mpsc::channel::<(usize, TagRead)>();
        let mut stopped: Option<TauError> = None;
        let mut done = total - misses.len() as u64;
        std::thread::scope(|scope| {
            for _ in 0..threads.min(misses.len()).max(1) {
                let (tx, next, cancel, media, misses) =
                    (tx.clone(), &next, &cancel, &media, &misses);
                scope.spawn(move || {
                    loop {
                        let n = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if n >= misses.len() || cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        let i = misses[n];
                        if tx
                            .send((i, read_track(&media[i].path, media[i].convert, kind)))
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            }
            drop(tx);
            for (i, result) in rx {
                read[i] = Some(result);
                done += 1;
                if stopped.is_none()
                    && let Err(e) = tick(
                        progress,
                        Progress {
                            stage: if kind.silence {
                                Stage::Silence
                            } else {
                                Stage::Scanning
                            },
                            done,
                            total,
                            path: Some(media[i].rel.clone()),
                        },
                    )
                {
                    stopped = Some(e);
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
        });
        if let Some(e) = stopped {
            return Err(e);
        }
    } else {
        tick(
            progress,
            Progress {
                stage: Stage::Scanning,
                done: total,
                total,
                path: None,
            },
        )?;
    }
    // Put it together in file order. The cover-file flag of the details describes the folder as it is now, not as
    // it was when the tags were cached, so it is looked at once per folder.
    let mut sidecar: BTreeMap<PathBuf, bool> = BTreeMap::new();
    for (i, m) in media.into_iter().enumerate() {
        let Media {
            path,
            rel,
            is_flac,
            convert,
            print,
        } = m;
        let (tags, secs, fmt) = if let Some(hit) = cached[i].clone() {
            output.reused += 1;
            (hit.tags, hit.secs, hit.fmt)
        } else {
            output.read += 1;
            match read[i].take() {
                Some(Ok(result)) => {
                    if let (Some(session), Some(print)) = (cache.as_mut(), print) {
                        session.put_tags(
                            &rel,
                            print,
                            ledger::TagRecord {
                                tags: result.0.clone(),
                                secs: result.1,
                                fmt: result.2,
                            },
                        );
                    }
                    result
                }
                other => {
                    let why = match other {
                        Some(Err(e)) => e,
                        _ => "not read".into(),
                    };
                    output.warnings.push(Warning::new(
                        WarningCode::TagsUnreadable,
                        format!("{rel}: tags unreadable ({why})"),
                    ));
                    (
                        BTreeMap::new(),
                        0,
                        if convert {
                            0
                        } else if is_flac {
                            2
                        } else {
                            1
                        },
                    )
                }
            }
        };
        let mut tags = tags;
        if let Some(mut det) = tags
            .get("_det")
            .and_then(|hex| crate::details::Details::from_hex(hex, ""))
        {
            let has = path.parent().is_some_and(|p| {
                *sidecar
                    .entry(p.to_path_buf())
                    .or_insert_with(|| p.join(crate::details::SIDECAR).is_file())
            });
            det.flags = (det.flags & !crate::details::F_SIDECAR)
                | if has { crate::details::F_SIDECAR } else { 0 };
            tags.insert("_det".into(), det.to_hex());
        }
        let (dir, file) = rel
            .rsplit_once('/')
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .unwrap_or((String::new(), rel.clone()));
        output.entries.push(Entry {
            rel,
            dir,
            file,
            tags,
            secs,
            fmt,
        });
    }
    if let Some(session) = cache {
        session.finish_scan();
    }
    if playlists {
        output.playlists = scan_playlists(common, &output.entries, &mut output.warnings)?;
    }
    Ok(output)
}

pub(super) fn collect_files(root: &Path, at: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(at)? {
        let entry = entry?;
        let child = entry.path();
        // The directory listing already says what each entry is, so nothing is asked of the file system again
        // (on a network share that is a round trip per entry). Only a link needs following.
        let (is_dir, is_file) = match entry.file_type() {
            Ok(t) if !t.is_symlink() => (t.is_dir(), t.is_file()),
            _ => (child.is_dir(), child.is_file()),
        };
        if is_dir {
            collect_files(root, &child, out)?;
        } else if is_file
            && !child
                .strip_prefix(root)
                .unwrap_or(&child)
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with("._"))
        {
            out.push(child);
        }
    }
    Ok(())
}

pub(super) fn scan_playlists(
    common: &Path,
    entries: &[Entry],
    warnings: &mut Vec<Warning>,
) -> Result<Vec<Playlist>, TauError> {
    let mut found = Vec::new();
    collect_files(common, common, &mut found)?;
    found.sort();
    let by_rel: BTreeMap<_, _> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.rel.as_str(), i))
        .collect();
    let mut used = BTreeSet::new();
    let mut output = Vec::new();
    for file in found.into_iter().filter(|f| {
        f.extension().is_some_and(|x| x.eq_ignore_ascii_case("m3u"))
            && !f
                .file_name()
                .is_some_and(|x| x.to_string_lossy().starts_with("._"))
    }) {
        let base = file
            .parent()
            .unwrap()
            .strip_prefix(common)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let mut ids = Vec::new();
        let mut dropped = 0;
        for line in String::from_utf8_lossy(&fs::read(&file)?)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let rel = if let Some(rest) = line.strip_prefix('/') {
                rest
            } else if base.is_empty() {
                line
            } else {
                &format!("{base}/{line}")
            };
            if let Some(id) = by_rel.get(rel) {
                ids.push(*id);
            } else {
                dropped += 1;
            }
        }
        if dropped > 0 {
            warnings.push(Warning::new(
                WarningCode::PlaylistEntriesDropped,
                format!(
                    "{}: {dropped} line(s) not in the library, dropped",
                    file.file_name().unwrap().to_string_lossy()
                ),
            ));
        }
        let mut own: Vec<_> = entries
            .iter()
            .filter(|e| e.dir == base)
            .map(|e| e.rel.as_str())
            .collect();
        own.sort_by_key(|r| natural(r.rsplit('/').next().unwrap_or(r)));
        if !base.is_empty()
            && ids
                .iter()
                .map(|id| entries[*id].rel.as_str())
                .eq(own.iter().copied())
        {
            continue;
        }
        if ids.is_empty() {
            continue;
        }
        if ids.len() > MAX_TRACKS {
            warnings.push(Warning::new(
                WarningCode::PlaylistTruncated,
                format!(
                    "{}: {} entries, truncated to {MAX_TRACKS}",
                    file.file_name().unwrap().to_string_lossy(),
                    ids.len()
                ),
            ));
            ids.truncate(MAX_TRACKS);
        }
        let stem = file.file_stem().unwrap_or_default().to_string_lossy();
        let mut name = ascii_text(&stem, 31);
        if name.is_empty() || name.eq_ignore_ascii_case("playlist") {
            name = if base.is_empty() {
                "Playlist".into()
            } else {
                ascii_text(
                    file.parent()
                        .unwrap()
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .as_ref(),
                    31,
                )
            };
        }
        let basename = if name.is_empty() {
            "Playlist".into()
        } else {
            name.clone()
        };
        let mut n = 2;
        while used.contains(&name.to_lowercase()) {
            name = format!("{} {n}", basename.chars().take(28).collect::<String>());
            n += 1;
        }
        used.insert(name.to_lowercase());
        let rel_file = file
            .strip_prefix(common)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        output.push(Playlist {
            name,
            rel_ids: ids,
            file: rel_file,
        });
    }
    Ok(output)
}

pub(crate) fn read_tags(path: &Path) -> Result<(BTreeMap<String, String>, u16, u8), TauError> {
    let mut f = fs::File::open(path)?;
    let size = f.metadata()?.len();
    if path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("flac"))
    {
        let (t, s) = read_flac(&mut f)?;
        return Ok((t, s.min(u16::MAX as u64) as u16, 2));
    }
    let (mut tags, start) = read_id3v2(&mut f)?;
    for (k, v) in read_id3v1(&mut f, size)? {
        tags.entry(k).or_insert(v);
    }
    let secs = mp3_seconds(&mut f, start, size)?
        .unwrap_or(0)
        .min(u16::MAX as u64) as u16;
    Ok((tags, secs, 1))
}
