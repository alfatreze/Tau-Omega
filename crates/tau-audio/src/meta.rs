//! The descriptive tags and front cover of an audio file, in the one shape a converted copy needs.

use crate::decode::Decoder;
use symphonia::core::meta::StandardTag;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Meta {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track: Option<u64>,
    pub disc: Option<u64>,
    pub year: Option<String>,
    pub genre: Option<String>,
    /// MIME type and bytes of the first picture (the front cover when marked).
    pub picture: Option<(String, Vec<u8>)>,
}

impl Decoder {
    /// Reads the tags and cover the file carries (what the demuxer and its ID3/Vorbis/MP4 readers found).
    pub fn meta(&mut self) -> Meta {
        let mut out = Meta::default();
        let Some(rev) = self.format_metadata().current().cloned() else {
            return out;
        };
        for tag in &rev.media.tags {
            let Some(std) = &tag.std else { continue };
            match std {
                StandardTag::TrackTitle(v) => out.title.get_or_insert_with(|| v.to_string()),
                StandardTag::Artist(v) => out.artist.get_or_insert_with(|| v.to_string()),
                StandardTag::Album(v) => out.album.get_or_insert_with(|| v.to_string()),
                StandardTag::AlbumArtist(v) => {
                    out.album_artist.get_or_insert_with(|| v.to_string())
                }
                StandardTag::Genre(v) => out.genre.get_or_insert_with(|| v.to_string()),
                StandardTag::RecordingDate(v) | StandardTag::ReleaseDate(v) => {
                    out.year.get_or_insert_with(|| v.chars().take(4).collect())
                }
                StandardTag::TrackNumber(n) => {
                    out.track.get_or_insert(*n);
                    continue;
                }
                StandardTag::DiscNumber(n) => {
                    out.disc.get_or_insert(*n);
                    continue;
                }
                _ => continue,
            };
        }
        if let Some(v) = rev.media.visuals.first() {
            let mime = v.media_type.clone().unwrap_or_else(|| "image/jpeg".into());
            out.picture = Some((mime, v.data.to_vec()));
        }
        out
    }
}
