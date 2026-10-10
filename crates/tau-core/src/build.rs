//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

pub(super) fn num(value: Option<&String>, hi: u16) -> u16 {
    value
        .and_then(|v| v.trim().split('/').next()?.parse::<u16>().ok())
        .map(|v| v.min(hi))
        .unwrap_or(0)
}
pub(super) fn year(tags: &BTreeMap<String, String>) -> u16 {
    tags.get("TDRC")
        .or_else(|| tags.get("TYER"))
        .and_then(|v| {
            v.as_bytes()
                .windows(4)
                .find_map(|w| std::str::from_utf8(w).ok()?.parse().ok())
        })
        .unwrap_or(0)
}

/// Reads the `_tno` (track/disc ordering) tag `build_index` itself just wrote
/// on every entry, defaulting to `0` instead of indexing-and-unwrapping. The
/// normal path always sets a valid value here first, but `Entry::tags` is a
/// public field: a host that constructs its own `Entry` values directly
/// (P1-2) must get an ordering fallback, not a panic, if it happens to pass
/// one in with that tag already missing or non-numeric.
pub(super) fn tno_of(entry: &Entry) -> u16 {
    entry
        .tags
        .get("_tno")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Reads the `_title` tag the same way, for the same reason: falls back to a
/// generic label instead of panicking on a hand-built `Entry`.
pub(super) fn title_of(entry: &Entry) -> &str {
    entry
        .tags
        .get("_title")
        .map(String::as_str)
        .unwrap_or("Track")
}
pub(super) fn push_u16(out: &mut Vec<u8>, n: u16) {
    out.extend_from_slice(&n.to_le_bytes());
}
pub(super) fn push_u32(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_le_bytes());
}
pub(super) fn put_u16(out: &mut [u8], at: usize, n: u16) {
    out[at..at + 2].copy_from_slice(&n.to_le_bytes());
}
pub(super) fn put_u32(out: &mut [u8], at: usize, n: u32) {
    out[at..at + 4].copy_from_slice(&n.to_le_bytes());
}
pub(super) fn align(out: &mut Vec<u8>) {
    out.resize((out.len() + 15) & !15, 0);
}

#[derive(Default)]
pub(super) struct Pool {
    buffer: Vec<u8>,
    offsets: BTreeMap<String, u32>,
}
impl Pool {
    fn new() -> Self {
        let mut out = Self::default();
        out.buffer.push(0);
        out.offsets.insert(String::new(), 0);
        out
    }
    fn add(&mut self, text: &str) -> u32 {
        if let Some(offset) = self.offsets.get(text) {
            return *offset;
        }
        let offset = self.buffer.len() as u32;
        self.buffer.extend_from_slice(text.as_bytes());
        self.buffer.push(0);
        self.offsets.insert(text.into(), offset);
        offset
    }
}

#[derive(Clone)]
pub(super) struct Album {
    dir: String,
    ids: Vec<usize>,
    artist: String,
    title: String,
    year: u16,
}

/// Encodes exactly the Tau v1 index layout. The input describes files already
/// laid out under the destination common directory; it does not copy or rename them.
pub fn build_index(
    entries: &[Entry],
    playlists: &[Playlist],
    root_prefix: &str,
    warnings: &mut Vec<Warning>,
) -> Result<Vec<u8>, TauError> {
    let mut entries = entries.to_vec();
    let mut grouped: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, e) in entries.iter_mut().enumerate() {
        let title = ascii_text(e.tags.get("TIT2").map(String::as_str).unwrap_or(""), 63);
        e.tags.insert(
            "_title".into(),
            if title.is_empty() {
                let stem = e.file.rsplit_once('.').map(|x| x.0).unwrap_or(&e.file);
                let fallback = ascii_text(
                    stem.trim_start_matches(|c: char| {
                        c.is_ascii_digit()
                            || c.is_ascii_whitespace()
                            || matches!(c, '.' | '_' | '-')
                    }),
                    63,
                );
                if fallback.is_empty() {
                    "Track".into()
                } else {
                    fallback
                }
            } else {
                title
            },
        );
        let tno = (num(e.tags.get("TPOS"), 63) << 10) | num(e.tags.get("TRCK"), 1023);
        e.tags.insert("_tno".into(), tno.to_string());
        if (root_prefix.len() + e.rel.len()) > MAX_PATH {
            return Err(TauError::e(
                ErrorCode::IndexCapExceeded,
                format!("path over {MAX_PATH} bytes: {}", e.rel),
            ));
        }
        if !(e.tags.contains_key("TIT2")
            && (e.tags.contains_key("TPE1") || e.tags.contains_key("TPE2")))
        {
            warnings.push(Warning::new(
                WarningCode::MissingTag,
                format!("{}: missing title/artist tag, using names", e.rel),
            ));
        }
        grouped.entry(e.dir.clone()).or_default().push(i);
    }
    let mut albums = Vec::new();
    for (dir, mut ids) in grouped {
        ids.sort_by_key(|&i| {
            let tno = tno_of(&entries[i]);
            (tno >> 10, tno & 1023, natural(&entries[i].file))
        });
        let first = &entries[ids[0]];
        let artist = ids
            .iter()
            .find_map(|&i| {
                let t = ascii_text(
                    entries[i]
                        .tags
                        .get("TPE2")
                        .or_else(|| entries[i].tags.get("TPE1"))
                        .map(String::as_str)
                        .unwrap_or(""),
                    63,
                );
                (!t.is_empty()).then_some(t)
            })
            .unwrap_or_else(|| "Unknown Artist".into());
        let title = ascii_text(first.tags.get("TALB").map(String::as_str).unwrap_or(""), 63);
        let title = if title.is_empty() {
            let fallback = ascii_text(dir.rsplit('/').next().unwrap_or(""), 63);
            if fallback.is_empty() {
                "Unknown Album".into()
            } else {
                fallback
            }
        } else {
            title
        };
        let album_year = ids
            .iter()
            .map(|&i| year(&entries[i].tags))
            .find(|v| *v != 0)
            .unwrap_or(0);
        albums.push(Album {
            dir,
            ids,
            artist,
            title,
            year: album_year,
        });
    }
    let mut artist_names: Vec<_> = albums.iter().map(|a| a.artist.clone()).collect();
    artist_names.sort_by_key(|n| (sort_key(n), n.clone()));
    artist_names.dedup();
    let artist_ids: BTreeMap<_, _> = artist_names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.clone(), i))
        .collect();
    albums.sort_by_key(|a| {
        (
            artist_ids[&a.artist],
            a.year,
            sort_key(&a.title),
            a.dir.clone(),
        )
    });
    if entries.len() > MAX_TRACKS
        || albums.len() > MAX_ALBUMS
        || artist_names.len() > MAX_ARTISTS
        || playlists.len() > MAX_PLAYLISTS
    {
        return Err(TauError::e(
            ErrorCode::IndexCapExceeded,
            "library exceeds a hard cap",
        ));
    }
    let mut pool = Pool::new();
    let root = pool.add(root_prefix);
    let (mut tracks, mut albums_b, mut first_album, mut track_new) =
        (Vec::new(), Vec::new(), BTreeMap::new(), BTreeMap::new());
    let mut album_titles = Vec::new();
    let mut track_titles = Vec::new();
    for (ai, a) in albums.iter().enumerate() {
        let artist = artist_ids[&a.artist];
        let item = first_album.entry(artist).or_insert((ai, 0usize));
        item.1 += 1;
        push_u32(&mut albums_b, pool.add(&a.title));
        push_u32(&mut albums_b, pool.add(&a.dir));
        push_u16(&mut albums_b, artist as u16);
        push_u16(&mut albums_b, a.year);
        push_u16(&mut albums_b, (tracks.len() / 16) as u16);
        push_u16(&mut albums_b, a.ids.len() as u16);
        push_u16(&mut albums_b, u16::MAX);
        push_u16(&mut albums_b, 0);
        album_titles.push(a.title.clone());
        for &i in &a.ids {
            let e = &entries[i];
            track_new.insert(i, tracks.len() / 16);
            let title = title_of(e).to_string();
            track_titles.push(title.clone());
            push_u32(&mut tracks, pool.add(&title));
            push_u32(&mut tracks, pool.add(&e.file));
            push_u16(&mut tracks, e.secs);
            push_u16(&mut tracks, tno_of(e));
            push_u16(&mut tracks, ai as u16);
            tracks.push(e.fmt);
            tracks.push(0);
        }
    }
    let mut artists_b = Vec::new();
    for (i, name) in artist_names.iter().enumerate() {
        let (first, count) = first_album[&i];
        push_u32(&mut artists_b, pool.add(name));
        push_u16(&mut artists_b, first as u16);
        push_u16(&mut artists_b, count as u16);
    }
    let mut albums_order: Vec<_> = (0..albums.len()).collect();
    albums_order.sort_by_key(|&i| (sort_key(&album_titles[i]), i));
    let mut tracks_order: Vec<_> = (0..entries.len()).collect();
    tracks_order.sort_by_key(|&i| (sort_key(&track_titles[i]), i));
    let make_letters = |names: Vec<String>| -> Vec<u16> {
        let classes: Vec<_> = names.iter().map(|n| sort_key(n).0).collect();
        (0..27)
            .map(|c| {
                classes
                    .iter()
                    .position(|&cls| cls >= c)
                    .unwrap_or(classes.len()) as u16
            })
            .collect()
    };
    let mut letter_values = make_letters(artist_names.clone());
    letter_values.extend(make_letters(
        albums_order
            .iter()
            .map(|&i| album_titles[i].clone())
            .collect(),
    ));
    letter_values.extend(make_letters(
        tracks_order
            .iter()
            .map(|&i| track_titles[i].clone())
            .collect(),
    ));
    let (mut playlists_b, mut items) = (Vec::new(), Vec::new());
    for playlist in playlists {
        push_u32(&mut playlists_b, pool.add(&playlist.name));
        push_u16(&mut playlists_b, (items.len() / 2) as u16);
        push_u16(&mut playlists_b, playlist.rel_ids.len() as u16);
        for old in &playlist.rel_ids {
            let new = track_new.get(old).ok_or_else(|| {
                TauError::e(
                    ErrorCode::IndexRecordRange,
                    "playlist references a missing track",
                )
            })?;
            push_u16(&mut items, *new as u16);
        }
    }
    let mut bodies = vec![
        artists_b,
        albums_b,
        tracks,
        pool.buffer.clone(),
        albums_order
            .iter()
            .flat_map(|i| (*i as u16).to_le_bytes())
            .collect(),
        tracks_order
            .iter()
            .flat_map(|i| (*i as u16).to_le_bytes())
            .collect(),
        letter_values.iter().flat_map(|i| i.to_le_bytes()).collect(),
        [playlists_b, items].concat(),
    ];
    let mut out = vec![0; HEADER];
    let mut sections = Vec::new();
    for body in bodies.drain(..) {
        align(&mut out);
        sections.push((out.len(), body.len()));
        out.extend(body);
    }
    align(&mut out);
    if out.len() > MAX_FILE || pool.buffer.len() > MAX_STRINGS {
        return Err(TauError::e(
            ErrorCode::IndexCapExceeded,
            "index exceeds a size cap",
        ));
    }
    let body_crc = crc32(&out[HEADER..]);
    let file_size = out.len() as u32;
    put_u32(&mut out, 0, MAGIC);
    put_u16(&mut out, 4, 1);
    put_u16(&mut out, 6, 1);
    put_u32(&mut out, 8, HEADER as u32);
    put_u32(&mut out, 12, if playlists.is_empty() { 0 } else { 2 });
    put_u32(&mut out, 16, body_crc);
    put_u32(&mut out, 20, file_size);
    put_u32(&mut out, 24, body_crc);
    put_u32(&mut out, 28, 0);
    put_u16(&mut out, 32, artist_names.len() as u16);
    put_u16(&mut out, 34, albums.len() as u16);
    put_u16(&mut out, 36, entries.len() as u16);
    put_u16(&mut out, 38, playlists.len() as u16);
    put_u32(&mut out, 40, root);
    put_u32(&mut out, 44, 0);
    for (i, (off, len)) in sections.iter().enumerate() {
        put_u32(&mut out, 48 + i * 8, *off as u32);
        put_u32(&mut out, 52 + i * 8, *len as u32);
    }
    let header_crc = crc32(&out[..124]);
    put_u32(&mut out, 124, header_crc);
    Ok(out)
}
