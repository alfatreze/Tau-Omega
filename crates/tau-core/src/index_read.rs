//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

#[derive(Debug, Clone)]
pub struct Index {
    pub data: Vec<u8>,
    pub flags: u32,
    pub build_id: u32,
    pub counts: Counts,
    pub sections: BTreeMap<String, (usize, usize)>,
    pub root: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Counts {
    pub artists: u16,
    pub albums: u16,
    pub tracks: u16,
    pub playlists: u16,
}
pub(super) fn u16_at(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes(data[off..off + 2].try_into().unwrap())
}
pub(super) fn u32_at(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(data[off..off + 4].try_into().unwrap())
}

/// Parses using the same validation order and E-codes as the firmware loader.
pub fn parse(data: impl AsRef<[u8]>) -> Result<Index, TauError> {
    let data = data.as_ref();
    if data.len() < HEADER {
        return Err(TauError::e(ErrorCode::IndexHeader, "shorter than header"));
    }
    if u32_at(data, 0) != MAGIC
        || u16_at(data, 6) > 1
        || u32_at(data, 8) != HEADER as u32
        || crc32(&data[..124]) != u32_at(data, 124)
    {
        return Err(TauError::e(ErrorCode::IndexHeader, "bad header"));
    }
    if u32_at(data, 20) as usize != data.len() {
        return Err(TauError::e(
            ErrorCode::IndexSize,
            "size differs from header",
        ));
    }
    if crc32(&data[HEADER..]) != u32_at(data, 24) {
        return Err(TauError::e(ErrorCode::IndexBodyCrc, "body CRC"));
    }
    let counts = Counts {
        artists: u16_at(data, 32),
        albums: u16_at(data, 34),
        tracks: u16_at(data, 36),
        playlists: u16_at(data, 38),
    };
    if counts.tracks as usize > MAX_TRACKS
        || counts.albums as usize > MAX_ALBUMS
        || counts.artists as usize > MAX_ARTISTS
        || counts.playlists as usize > MAX_PLAYLISTS
        || data.len() > MAX_FILE
    {
        return Err(TauError::e(ErrorCode::IndexCapExceeded, "count above cap"));
    }
    let mut sections = BTreeMap::new();
    for (i, name) in SECTIONS.iter().enumerate() {
        let (off, len) = (
            u32_at(data, 48 + i * 8) as usize,
            u32_at(data, 52 + i * 8) as usize,
        );
        if off % 16 != 0 || off < HEADER || off.checked_add(len).is_none_or(|end| end > data.len())
        {
            return Err(TauError::e(
                ErrorCode::IndexSectionRange,
                format!("section {name} out of range"),
            ));
        }
        sections.insert((*name).to_string(), (off, len));
    }
    for (name, want) in [
        ("artists", counts.artists as usize * 8),
        ("albums", counts.albums as usize * 20),
        ("tracks", counts.tracks as usize * 16),
        ("album_by_title", counts.albums as usize * 2),
        ("track_by_title", counts.tracks as usize * 2),
        ("letters", 162),
    ] {
        if sections[name].1 != want {
            return Err(TauError::e(
                ErrorCode::IndexSectionRange,
                format!("section {name} length"),
            ));
        }
    }
    if sections["strings"].1 > MAX_STRINGS {
        return Err(TauError::e(ErrorCode::IndexCapExceeded, "string cap"));
    }
    let ix = Index {
        data: data.to_vec(),
        flags: u32_at(data, 12),
        build_id: u32_at(data, 16),
        counts,
        sections,
        root: u32_at(data, 40),
    };
    walk(&ix)?;
    Ok(ix)
}
pub(super) fn string_at(ix: &Index, offset: u32) -> Result<&str, TauError> {
    let (off, len) = ix.sections["strings"];
    let start = off + offset as usize;
    if offset as usize >= len {
        return Err(TauError::e(ErrorCode::IndexRecordRange, "string offset"));
    }
    let bytes = &ix.data[start..off + len];
    let end = bytes.iter().position(|x| *x == 0).unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..end])
        .map_err(|_| TauError::e(ErrorCode::IndexRecordRange, "non-ASCII string"))
}
pub(super) fn walk(ix: &Index) -> Result<(), TauError> {
    let strings = ix.sections["strings"].1;
    if ix.root as usize >= strings {
        return Err(TauError::e(ErrorCode::IndexRecordRange, "root string"));
    }
    let (tracks, _) = ix.sections["tracks"];
    for i in 0..ix.counts.tracks as usize {
        if i >= 256 && i % 64 != 0 {
            continue;
        }
        let r = tracks + i * 16;
        if u32_at(&ix.data, r) as usize >= strings
            || u32_at(&ix.data, r + 4) as usize >= strings
            || u16_at(&ix.data, r + 12) >= ix.counts.albums
        {
            return Err(TauError::e(ErrorCode::IndexRecordRange, "track record"));
        }
    }
    let (albums, _) = ix.sections["albums"];
    for i in 0..ix.counts.albums as usize {
        let r = albums + i * 20;
        if u32_at(&ix.data, r) as usize >= strings
            || u32_at(&ix.data, r + 4) as usize >= strings
            || u16_at(&ix.data, r + 8) >= ix.counts.artists
            || usize::from(u16_at(&ix.data, r + 12)) + usize::from(u16_at(&ix.data, r + 14))
                > ix.counts.tracks as usize
        {
            return Err(TauError::e(ErrorCode::IndexRecordRange, "album record"));
        }
    }
    let (lists, len) = ix.sections["playlists"];
    if len < 8 * ix.counts.playlists as usize
        || !(len - 8 * ix.counts.playlists as usize).is_multiple_of(2)
    {
        return Err(TauError::e(ErrorCode::IndexSectionRange, "playlist range"));
    }
    let items = (len - 8 * ix.counts.playlists as usize) / 2;
    for i in 0..ix.counts.playlists as usize {
        let r = lists + i * 8;
        if u32_at(&ix.data, r) as usize >= strings
            || usize::from(u16_at(&ix.data, r + 4)) + usize::from(u16_at(&ix.data, r + 6)) > items
            || u16_at(&ix.data, r + 6) as usize > MAX_TRACKS
        {
            return Err(TauError::e(ErrorCode::IndexRecordRange, "playlist record"));
        }
    }
    for i in 0..items {
        if i >= 256 && i % 64 != 0 {
            continue;
        }
        if u16_at(&ix.data, lists + 8 * ix.counts.playlists as usize + i * 2) >= ix.counts.tracks {
            return Err(TauError::e(ErrorCode::IndexRecordRange, "playlist item"));
        }
    }
    Ok(())
}
pub fn track_path(ix: &Index, id: u16) -> Result<String, TauError> {
    if id >= ix.counts.tracks {
        return Err(TauError::e(ErrorCode::IndexRecordRange, "track id"));
    }
    let (t, _) = ix.sections["tracks"];
    let (a, _) = ix.sections["albums"];
    let r = t + id as usize * 16;
    let album = u16_at(&ix.data, r + 12) as usize;
    let ar = a + album * 20;
    let root = string_at(ix, ix.root)?;
    let dir = string_at(ix, u32_at(&ix.data, ar + 4))?;
    let file = string_at(ix, u32_at(&ix.data, r + 4))?;
    Ok(if dir.is_empty() {
        format!("{root}{file}")
    } else {
        format!("{root}{dir}/{file}")
    })
}
pub fn verify(data: impl AsRef<[u8]>, root: Option<&Path>) -> Result<Vec<String>, TauError> {
    let ix = parse(data)?;
    let mut issues = Vec::new();
    if let Some(root) = root {
        for id in 0..ix.counts.tracks {
            let path = track_path(&ix, id)?;
            let relative = path.strip_prefix(string_at(&ix, ix.root)?).unwrap_or(&path);
            if !root.join(relative).is_file() {
                issues.push(format!("file missing: {relative}"));
            }
        }
    }
    Ok(issues)
}

pub fn synth(tracks: usize, albums: usize, artists: usize) -> Vec<Entry> {
    (0..tracks)
        .map(|i| {
            let album = i * albums / tracks;
            let artist = album * artists / albums;
            let mut tags = BTreeMap::new();
            tags.insert("TIT2".into(), format!("Track {:05}", i + 1));
            tags.insert("TPE1".into(), format!("Artist {:04}", artist + 1));
            tags.insert("TALB".into(), format!("Album {:04}", album + 1));
            tags.insert("TRCK".into(), format!("{}", (i % 12) + 1));
            Entry {
                rel: format!(
                    "Artist {:04}/Album {:04}/{:02} Track {:05}.mp3",
                    artist + 1,
                    album + 1,
                    (i % 12) + 1,
                    i + 1
                ),
                dir: format!("Artist {:04}/Album {:04}", artist + 1, album + 1),
                file: format!("{:02} Track {:05}.mp3", (i % 12) + 1, i + 1),
                tags,
                secs: 180,
                fmt: 1,
            }
        })
        .collect()
}
