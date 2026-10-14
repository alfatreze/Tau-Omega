//! Decoding any file the bundled demuxers and codecs know (FLAC, MP3, WAV, AIFF, ALAC, AAC-LC, Vorbis,
//! PCM, ADPCM) into interleaved `f32` frames. 24-bit audio is exact in `f32`.

use crate::AudioError;
use std::path::Path;
use symphonia::core::audio::Channels;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// What a file is, read without decoding the audio.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceInfo {
    /// A short lowercase codec name (`flac`, `mp3`, `aac`, `alac`, `vorbis`, `pcm`, `adpcm`, ...).
    pub codec: String,
    pub lossless: bool,
    pub rate: u32,
    pub channels: u16,
    /// Bits per sample of the source, when it has one (lossless and PCM sources).
    pub bits: Option<u32>,
    pub frames: Option<u64>,
}

pub struct Decoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    pub info: SourceInfo,
    /// The speaker position of each interleaved channel, when the file says (see [`crate::mixdown`]).
    pub positions: Option<u64>,
}

fn codec_name(id: symphonia::core::codecs::audio::AudioCodecId) -> (&'static str, bool) {
    use symphonia::core::codecs::audio::well_known::*;
    match id {
        CODEC_ID_FLAC => ("flac", true),
        CODEC_ID_ALAC => ("alac", true),
        CODEC_ID_MP3 => ("mp3", false),
        CODEC_ID_AAC => ("aac", false),
        CODEC_ID_VORBIS => ("vorbis", false),
        x if (CODEC_ID_PCM_S32LE..=CODEC_ID_PCM_F64BE_PLANAR).contains(&x) => ("pcm", true),
        x if (CODEC_ID_ADPCM_G722..=CODEC_ID_ADPCM_IMA_QT).contains(&x) => ("adpcm", false),
        _ => ("other", false),
    }
}

impl Decoder {
    pub fn open(path: &Path) -> Result<Self, AudioError> {
        let file = std::fs::File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let format = symphonia::default::get_probe()
            .probe(
                &hint,
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| AudioError::Decode(format!("not an audio file this tool reads ({e})")))?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| AudioError::Decode("no audio track".into()))?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or_else(|| AudioError::Decode("the audio parameters are missing".into()))?;
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|e| AudioError::Decode(format!("no decoder for this codec ({e})")))?;
        let rate = params
            .sample_rate
            .ok_or_else(|| AudioError::Decode("unknown sample rate".into()))?;
        let channels = params
            .channels
            .as_ref()
            .map(|c| c.count() as u16)
            .filter(|c| *c > 0)
            .ok_or_else(|| AudioError::Decode("unknown channel layout".into()))?;
        let positions = match params.channels.as_ref() {
            Some(Channels::Positioned(p)) => Some(p.bits()),
            _ => None,
        };
        let (codec, lossless) = codec_name(params.codec);
        let info = SourceInfo {
            codec: codec.to_string(),
            lossless,
            rate,
            channels,
            bits: params.bits_per_sample,
            frames: track.num_frames,
        };
        let track_id = track.id;
        Ok(Self {
            format,
            decoder,
            track_id,
            info,
            positions,
        })
    }

    pub(crate) fn format_metadata(&mut self) -> symphonia::core::meta::Metadata<'_> {
        self.format.metadata()
    }

    /// Appends the next packet's frames (interleaved, the file's channel order) to `out`. `Ok(false)` at the
    /// end of the stream. Undecodable packets are skipped, as a player does.
    pub fn next(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => return Ok(false),
                Err(Error::ResetRequired) => return Ok(false),
                Err(e) => return Err(AudioError::Decode(e.to_string())),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buf) => {
                    let n = buf.samples_interleaved();
                    let at = out.len();
                    out.resize(at + n, 0.0);
                    buf.copy_to_slice_interleaved(&mut out[at..]);
                    return Ok(true);
                }
                Err(Error::DecodeError(_)) | Err(Error::IoError(_)) => continue,
                Err(e) => return Err(AudioError::Decode(e.to_string())),
            }
        }
    }
}

/// Reads what a file is without decoding it. `Err` means this tool cannot read it.
pub fn probe(path: &Path) -> Result<SourceInfo, AudioError> {
    Ok(Decoder::open(path)?.info)
}
