//! FLAC writing (flacenc, pure Rust) from a [`Pipeline`], with Vorbis comments and an optional picture.

use crate::AudioError;
use crate::pipeline::{Pipeline, Quantizer};
use flacenc::component::{BitRepr, MetadataBlockData};
use flacenc::error::Verify;
use flacenc::source::{Fill, Source};
use std::io::Write;
use std::path::Path;

/// A picture to put in the file: MIME type and bytes.
pub struct Picture<'a> {
    pub mime: &'a str,
    pub data: &'a [u8],
}

struct PullSource<'a> {
    pipeline: &'a mut Pipeline,
    quant: Quantizer,
    channels: usize,
    bits: usize,
    rate: usize,
    scratch: Vec<f64>,
    ints: Vec<i32>,
    error: Option<AudioError>,
    frames: u64,
    progress: &'a mut dyn FnMut(u64) -> bool,
    cancelled: bool,
}

impl Source for PullSource<'_> {
    fn channels(&self) -> usize {
        self.channels
    }
    fn bits_per_sample(&self) -> usize {
        self.bits
    }
    fn sample_rate(&self) -> usize {
        self.rate
    }
    fn read_samples<F: Fill>(
        &mut self,
        block_size: usize,
        dest: &mut F,
    ) -> Result<usize, flacenc::error::SourceError> {
        self.scratch.clear();
        let n = match self.pipeline.read(block_size, &mut self.scratch) {
            Ok(n) => n,
            Err(e) => {
                self.error = Some(e);
                0
            }
        };
        if n == 0 {
            return Ok(0);
        }
        self.frames += n as u64;
        if !(self.progress)(self.frames) {
            self.cancelled = true;
            return Ok(0);
        }
        self.ints.clear();
        for s in &self.scratch {
            self.ints.push(self.quant.to_i32(*s));
        }
        dest.fill_interleaved(&self.ints)?;
        Ok(n)
    }
}

fn vorbis_block(vendor: &str, comments: &[(String, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend((vendor.len() as u32).to_le_bytes());
    out.extend(vendor.as_bytes());
    out.extend((comments.len() as u32).to_le_bytes());
    for (k, v) in comments {
        let line = format!("{k}={v}");
        out.extend((line.len() as u32).to_le_bytes());
        out.extend(line.as_bytes());
    }
    out
}

fn picture_block(p: &Picture<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(3u32.to_be_bytes()); // front cover
    out.extend((p.mime.len() as u32).to_be_bytes());
    out.extend(p.mime.as_bytes());
    out.extend(0u32.to_be_bytes()); // description
    out.extend([0u8; 16]); // width, height, depth, colours: unknown
    out.extend((p.data.len() as u32).to_be_bytes());
    out.extend(p.data);
    out
}

/// Encodes the whole pipeline to `path` at `bits` (16 or 24). `dither` applies TPDF dither (use it only
/// when the depth is being reduced). `progress` gets the frames written so far and returns `false` to
/// cancel. The file is written to `path` directly; the caller names a temporary path.
pub fn encode(
    pipeline: &mut Pipeline,
    bits: u32,
    dither: bool,
    comments: &[(String, String)],
    picture: Option<Picture<'_>>,
    path: &Path,
    progress: &mut dyn FnMut(u64) -> bool,
) -> Result<u64, AudioError> {
    let channels = pipeline.target.channels as usize;
    let rate = pipeline.target.rate as usize;
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|e| AudioError::Process(format!("encoder settings: {e:?}")))?;
    let mut source = PullSource {
        pipeline,
        quant: Quantizer::new(bits, dither),
        channels,
        bits: bits as usize,
        rate,
        scratch: Vec::new(),
        ints: Vec::new(),
        error: None,
        frames: 0,
        progress,
        cancelled: false,
    };
    let mut stream = flacenc::encode_with_fixed_block_size(&config, &mut source, config.block_size)
        .map_err(|e| AudioError::Process(format!("FLAC encoding failed: {e:?}")))?;
    if let Some(e) = source.error.take() {
        return Err(e);
    }
    if source.cancelled {
        return Err(AudioError::Process("cancelled".into()));
    }
    let frames = source.frames;
    // flacenc counts the short last frame in the minimum block size; the format (and strict readers) mean
    // the minimum excluding the last block, which is the configured size unless there is only one block.
    let block = config.block_size.min(frames.max(1) as usize);
    stream
        .stream_info_mut()
        .set_block_sizes(block, config.block_size.max(block))
        .map_err(|e| AudioError::Process(format!("{e:?}")))?;
    let vendor = "tau-omega";
    stream.add_metadata_block(
        MetadataBlockData::new_unknown(4, &vorbis_block(vendor, comments))
            .map_err(|e| AudioError::Process(format!("{e:?}")))?,
    );
    if let Some(p) = picture {
        stream.add_metadata_block(
            MetadataBlockData::new_unknown(6, &picture_block(&p))
                .map_err(|e| AudioError::Process(format!("{e:?}")))?,
        );
    }
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| AudioError::Process(format!("{e:?}")))?;
    let mut file = std::fs::File::create(path)?;
    file.write_all(sink.as_slice())?;
    file.sync_all()?;
    Ok(frames)
}
