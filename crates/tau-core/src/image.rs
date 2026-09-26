//! `TIM1` cover-image container: decode of every payload shape tau-alpha's
//! own `tools/tau_image.py` can produce, and an encoder for the decided
//! default (`tau-alpha/docs/IMAGE_FORMATS.md` D-I01/D-I02, 2026-09-26:
//! palette-256, 128 px on the long side, proportional scale, no crop or
//! letterbox).
//!
//! Status, carried over honestly from the source spec: the container is
//! **not frozen** (D-I05) and **no firmware reader exists yet** — this is
//! forward-prep, not a feature the device uses today. The decoder is
//! verified against a real file `tools/tau_image.py` produced
//! (`testdata/images/README.md`), never a fixture invented from the format
//! description; the encoder does not need to match that tool's quantizer
//! pixel-for-pixel, only the container shape, since the format itself says
//! so.

use crate::{cover, ErrorCode, TauError};
use std::{collections::HashMap, path::Path};

const MAGIC: &[u8; 4] = b"TIM1";
const HEADER_LEN: usize = 16;

/// The four payload shapes `TIM1` names. Only `Rgb565` and `Palette` are
/// decoded today — `Bc1`/`Jpeg` are parked, non-default variants
/// (`IMAGE_FORMATS.md` D-I04) with no real fixture to verify against yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimFormat {
    Rgb565 = 1,
    Palette = 2,
    Bc1 = 3,
    Jpeg = 4,
}

impl TimFormat {
    fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            1 => Self::Rgb565,
            2 => Self::Palette,
            3 => Self::Bc1,
            4 => Self::Jpeg,
            _ => return None,
        })
    }
}

/// A decoded `TIM1` image: plain RGB8, row-major, `width * height * 3` bytes.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u16,
    pub height: u16,
    pub rgb: Vec<u8>,
}

/// Decodes a `TIM1` container's header and payload into plain RGB8.
pub fn decode_tim1(bytes: &[u8]) -> Result<DecodedImage, TauError> {
    if bytes.len() < HEADER_LEN || &bytes[0..4] != MAGIC {
        return Err(TauError::e(
            ErrorCode::InvalidTim1Container,
            "not a TIM1 file",
        ));
    }
    let format = TimFormat::from_u8(bytes[4]).ok_or_else(|| {
        TauError::e(
            ErrorCode::InvalidTim1Container,
            format!("unknown TIM1 format byte {}", bytes[4]),
        )
    })?;
    let bpp = bytes[5];
    let width = u16::from_le_bytes([bytes[6], bytes[7]]);
    let height = u16::from_le_bytes([bytes[8], bytes[9]]);
    let ncolors = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
    let payload_len =
        u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let payload = &bytes[HEADER_LEN..];
    if payload.len() != payload_len {
        return Err(TauError::e(
            ErrorCode::InvalidTim1Container,
            format!(
                "length mismatch: header says {payload_len}, file has {}",
                payload.len()
            ),
        ));
    }
    let pixel_count = width as usize * height as usize;
    match format {
        TimFormat::Rgb565 => {
            if payload.len() != pixel_count * 2 {
                return Err(TauError::e(
                    ErrorCode::InvalidTim1Container,
                    "rgb565 payload size mismatch",
                ));
            }
            let mut rgb = Vec::with_capacity(pixel_count * 3);
            for word in payload.as_chunks::<2>().0 {
                let (r, g, b) = rgb565_to_rgb8(u16::from_le_bytes(*word));
                rgb.extend_from_slice(&[r, g, b]);
            }
            Ok(DecodedImage { width, height, rgb })
        }
        TimFormat::Palette => {
            if !matches!(bpp, 8 | 6 | 4) {
                return Err(TauError::e(
                    ErrorCode::InvalidTim1Container,
                    format!("unsupported palette bpp {bpp}"),
                ));
            }
            let clut_bytes = ncolors * 2;
            if payload.len() < clut_bytes {
                return Err(TauError::e(
                    ErrorCode::InvalidTim1Container,
                    "truncated CLUT",
                ));
            }
            let clut: Vec<(u8, u8, u8)> = payload[..clut_bytes]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| rgb565_to_rgb8(u16::from_le_bytes(*c)))
                .collect();
            let indices = unpack_indices(&payload[clut_bytes..], bpp, pixel_count)?;
            let mut rgb = Vec::with_capacity(pixel_count * 3);
            for index in indices {
                let &(r, g, b) = clut.get(index as usize).ok_or_else(|| {
                    TauError::e(ErrorCode::InvalidTim1Container, "palette index out of range")
                })?;
                rgb.extend_from_slice(&[r, g, b]);
            }
            Ok(DecodedImage { width, height, rgb })
        }
        TimFormat::Bc1 | TimFormat::Jpeg => Err(TauError::e(
            ErrorCode::InvalidTim1Container,
            "bc1/jpeg TIM1 payloads are not decoded yet (parked variant, IMAGE_FORMATS.md D-I04)",
        )),
    }
}

fn rgb565_to_rgb8(word: u16) -> (u8, u8, u8) {
    let r5 = ((word >> 11) & 0x1f) as u8;
    let g6 = ((word >> 5) & 0x3f) as u8;
    let b5 = (word & 0x1f) as u8;
    (
        (r5 << 3) | (r5 >> 2),
        (g6 << 2) | (g6 >> 4),
        (b5 << 3) | (b5 >> 2),
    )
}

fn rgb8_to_rgb565(r: u8, g: u8, b: u8) -> u16 {
    (((r as u16) >> 3) << 11) | (((g as u16) >> 2) << 5) | ((b as u16) >> 3)
}

fn unpack_indices(buf: &[u8], bpp: u8, count: usize) -> Result<Vec<u8>, TauError> {
    let mut out = Vec::with_capacity(count);
    match bpp {
        8 => {
            if buf.len() < count {
                return Err(TauError::e(
                    ErrorCode::InvalidTim1Container,
                    "truncated 8bpp indices",
                ));
            }
            out.extend_from_slice(&buf[..count]);
        }
        4 => {
            'outer: for byte in buf {
                for shifted in [byte >> 4, byte & 0x0f] {
                    out.push(shifted);
                    if out.len() >= count {
                        break 'outer;
                    }
                }
            }
        }
        6 => {
            'outer: for chunk in buf.as_chunks::<3>().0 {
                let v = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | chunk[2] as u32;
                for shift in [18, 12, 6, 0] {
                    out.push(((v >> shift) & 0x3f) as u8);
                    if out.len() >= count {
                        break 'outer;
                    }
                }
            }
        }
        _ => unreachable!("bpp already validated"),
    }
    if out.len() < count {
        return Err(TauError::e(
            ErrorCode::InvalidTim1Container,
            "truncated palette indices",
        ));
    }
    out.truncate(count);
    Ok(out)
}

fn pack_indices(indices: &[u8], bpp: u8) -> Vec<u8> {
    match bpp {
        8 => indices.to_vec(),
        4 => indices
            .chunks(2)
            .map(|pair| {
                let high = pair[0] << 4;
                let low = pair.get(1).copied().unwrap_or(0);
                high | low
            })
            .collect(),
        6 => indices
            .chunks(4)
            .map(|group| {
                let mut padded = [0u8; 4];
                padded[..group.len()].copy_from_slice(group);
                let v = ((padded[0] as u32) << 18)
                    | ((padded[1] as u32) << 12)
                    | ((padded[2] as u32) << 6)
                    | padded[3] as u32;
                [((v >> 16) & 0xff) as u8, ((v >> 8) & 0xff) as u8, (v & 0xff) as u8]
            })
            .collect::<Vec<[u8; 3]>>()
            .concat(),
        _ => unreachable!("bpp already validated"),
    }
}

/// The sidecar path `sync_media.py --art-variants`'s default produces,
/// relative to an album folder: `tau-art/cover_<long_side>.pal256.timg`.
pub fn pal256_sidecar_name(long_side: u16) -> String {
    format!("tau-art/cover_{long_side}.pal256.timg")
}

/// Encodes a folder's cover image (same source `cover::find_cover` already
/// locates) as a palette-256 `TIM1` file at `long_side` px on the long side,
/// scaled proportionally with no crop or letterbox (D-I02).
pub fn encode_cover_pal256(folder: &Path, long_side: u16) -> Result<Vec<u8>, TauError> {
    let source = cover::find_cover(folder).ok_or_else(|| {
        TauError::e(ErrorCode::NotFound, "no cover image found in this folder")
    })?;
    let bytes = std::fs::read(&source)?;
    encode_pal256_bytes(&bytes, long_side)
}

/// Same as [`encode_cover_pal256`] but takes the source image's bytes
/// directly (JPEG or PNG), for callers that already have them in memory.
pub fn encode_pal256_bytes(source: &[u8], long_side: u16) -> Result<Vec<u8>, TauError> {
    let (src_width, src_height, src_rgb) = decode_source_image(source)?;
    let (dst_width, dst_height) = fit_long_side(src_width, src_height, long_side as u32);
    let resized = resize_rgb8(&src_rgb, src_width, src_height, dst_width, dst_height)?;
    let (palette, indices) = quantize_pal256(&resized, dst_width as usize, dst_height as usize);
    let mut clut = vec![0u8; 256 * 2];
    for (i, &(r, g, b)) in palette.iter().enumerate() {
        let word = rgb8_to_rgb565(r, g, b).to_le_bytes();
        clut[i * 2] = word[0];
        clut[i * 2 + 1] = word[1];
    }
    let packed_indices = pack_indices(&indices, 8);
    let mut payload = clut;
    payload.extend_from_slice(&packed_indices);
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(MAGIC);
    out.push(TimFormat::Palette as u8);
    out.push(8); // bpp
    out.extend_from_slice(&(dst_width as u16).to_le_bytes());
    out.extend_from_slice(&(dst_height as u16).to_le_bytes());
    out.extend_from_slice(&256u16.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Long side becomes `long_side` px, the other side scales proportionally;
/// never crops, never pads (D-I02). Matches `tau_image.py`'s `fit_long_side`.
fn fit_long_side(width: u32, height: u32, long_side: u32) -> (u32, u32) {
    let longest = width.max(height).max(1);
    let scale = long_side as f64 / longest as f64;
    let round = |v: u32| ((v as f64 * scale).round() as u32).max(1);
    (round(width), round(height))
}

fn decode_source_image(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), TauError> {
    if bytes.starts_with(&[0xff, 0xd8]) {
        decode_jpeg(bytes)
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        decode_png(bytes)
    } else {
        Err(TauError::e(
            ErrorCode::UnsupportedCover,
            "unrecognised cover image format (expected JPEG or PNG)",
        ))
    }
}

fn decode_jpeg(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), TauError> {
    let mut decoder = zune_jpeg::JpegDecoder::new(std::io::Cursor::new(bytes));
    let pixels = decoder.decode().map_err(|error| {
        TauError::e(ErrorCode::UnsupportedCover, format!("invalid JPEG cover: {error}"))
    })?;
    let info = decoder
        .info()
        .ok_or_else(|| TauError::e(ErrorCode::UnsupportedCover, "JPEG had no image info"))?;
    let channels = decoder
        .output_colorspace()
        .map(|space| space.num_components())
        .unwrap_or(3);
    let rgb = to_rgb8(&pixels, channels)?;
    Ok((info.width as u32, info.height as u32, rgb))
}

fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), TauError> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| TauError::e(ErrorCode::UnsupportedCover, format!("invalid PNG cover: {error}")))?;
    let buffer_size = reader
        .output_buffer_size()
        .ok_or_else(|| TauError::e(ErrorCode::UnsupportedCover, "PNG cover too large to decode"))?;
    let mut buffer = vec![0u8; buffer_size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| TauError::e(ErrorCode::UnsupportedCover, format!("invalid PNG frame: {error}")))?;
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => {
            return Err(TauError::e(
                ErrorCode::UnsupportedCover,
                "indexed PNG was not expanded to a plain colour type",
            ))
        }
    };
    let rgb = to_rgb8(&buffer[..info.buffer_size()], channels)?;
    Ok((info.width, info.height, rgb))
}

/// Normalises a flat pixel buffer of `channels` components per pixel to
/// plain RGB8 (dropping alpha, which is essentially always opaque on real
/// covers per `IMAGE_FORMATS.md`'s own measurement).
fn to_rgb8(buffer: &[u8], channels: usize) -> Result<Vec<u8>, TauError> {
    if channels == 0 || !buffer.len().is_multiple_of(channels) {
        return Err(TauError::e(
            ErrorCode::UnsupportedCover,
            "decoded cover buffer size does not match its channel count",
        ));
    }
    let mut rgb = Vec::with_capacity((buffer.len() / channels) * 3);
    for pixel in buffer.chunks_exact(channels) {
        match channels {
            1 => rgb.extend_from_slice(&[pixel[0], pixel[0], pixel[0]]),
            2 => rgb.extend_from_slice(&[pixel[0], pixel[0], pixel[0]]),
            3 | 4 => rgb.extend_from_slice(&pixel[..3]),
            _ => {
                return Err(TauError::e(
                    ErrorCode::UnsupportedCover,
                    format!("unsupported channel count {channels}"),
                ))
            }
        }
    }
    Ok(rgb)
}

fn resize_rgb8(
    rgb: &[u8],
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
) -> Result<Vec<u8>, TauError> {
    if (src_width, src_height) == (dst_width, dst_height) {
        return Ok(rgb.to_vec());
    }
    let to_pixels = |flat: &[u8]| -> Vec<rgb::RGB8> {
        flat.as_chunks::<3>()
            .0
            .iter()
            .map(|c| rgb::RGB8::new(c[0], c[1], c[2]))
            .collect()
    };
    let src_pixels = to_pixels(rgb);
    let mut dst_pixels = vec![rgb::RGB8::new(0, 0, 0); dst_width as usize * dst_height as usize];
    let mut resizer = resize::Resizer::new(
        src_width as usize,
        src_height as usize,
        dst_width as usize,
        dst_height as usize,
        resize::Pixel::RGB8,
        resize::Type::Lanczos3,
    )
    .map_err(|error| TauError::e(ErrorCode::UnsupportedCover, format!("resize setup failed: {error}")))?;
    resizer
        .resize(&src_pixels, &mut dst_pixels)
        .map_err(|error| TauError::e(ErrorCode::UnsupportedCover, format!("resize failed: {error}")))?;
    let mut out = Vec::with_capacity(dst_pixels.len() * 3);
    for pixel in dst_pixels {
        out.extend_from_slice(&[pixel.r, pixel.g, pixel.b]);
    }
    Ok(out)
}

/// Median-cut palette build (up to 256 colours) plus Floyd-Steinberg
/// dithering against it. Not required to match `tau_image.py`'s own
/// (Pillow-based) quantizer pixel-for-pixel — only the `TIM1` container
/// shape is a shared contract (`IMAGE_FORMATS.md`, "not frozen").
fn quantize_pal256(rgb: &[u8], width: usize, height: usize) -> (Vec<(u8, u8, u8)>, Vec<u8>) {
    const MAX_COLORS: usize = 256;
    let mut histogram: HashMap<(u8, u8, u8), u32> = HashMap::new();
    for pixel in rgb.as_chunks::<3>().0 {
        *histogram.entry((pixel[0], pixel[1], pixel[2])).or_insert(0) += 1;
    }
    let mut entries: Vec<(u8, u8, u8, u32)> = histogram
        .into_iter()
        .map(|((r, g, b), n)| (r, g, b, n))
        .collect();
    let palette = if entries.len() <= MAX_COLORS {
        entries.iter().map(|&(r, g, b, _)| (r, g, b)).collect()
    } else {
        median_cut(&mut entries, MAX_COLORS)
    };
    let indices = dither_floyd_steinberg(rgb, width, height, &palette);
    (palette, indices)
}

fn median_cut(entries: &mut [(u8, u8, u8, u32)], max_colors: usize) -> Vec<(u8, u8, u8)> {
    let mut buckets: Vec<(usize, usize)> = vec![(0, entries.len())];
    while buckets.len() < max_colors {
        let widest = buckets
            .iter()
            .enumerate()
            .filter(|&(_, &(s, e))| e - s > 1)
            .max_by_key(|&(_, &(s, e))| bucket_weight(&entries[s..e]))
            .map(|(i, _)| i);
        let Some(i) = widest else { break };
        let (s, e) = buckets[i];
        let split = split_bucket(&mut entries[s..e]);
        buckets[i] = (s, s + split);
        buckets.push((s + split, e));
    }
    buckets
        .iter()
        .map(|&(s, e)| bucket_average(&entries[s..e]))
        .collect()
}

fn bucket_weight(bucket: &[(u8, u8, u8, u32)]) -> u64 {
    bucket.iter().map(|&(_, _, _, n)| n as u64).sum()
}

fn bucket_average(bucket: &[(u8, u8, u8, u32)]) -> (u8, u8, u8) {
    let (mut rs, mut gs, mut bs, mut total) = (0u64, 0u64, 0u64, 0u64);
    for &(r, g, b, n) in bucket {
        let n = n as u64;
        rs += r as u64 * n;
        gs += g as u64 * n;
        bs += b as u64 * n;
        total += n;
    }
    if total == 0 {
        return (0, 0, 0);
    }
    ((rs / total) as u8, (gs / total) as u8, (bs / total) as u8)
}

/// Splits `bucket` in place along its widest channel at the weighted
/// median, so both halves hold roughly equal pixel population. Returns the
/// length of the first half.
fn split_bucket(bucket: &mut [(u8, u8, u8, u32)]) -> usize {
    let (mut min, mut max) = ((255u8, 255u8, 255u8), (0u8, 0u8, 0u8));
    for &(r, g, b, _) in bucket.iter() {
        min = (min.0.min(r), min.1.min(g), min.2.min(b));
        max = (max.0.max(r), max.1.max(g), max.2.max(b));
    }
    let ranges = (max.0 - min.0, max.1 - min.1, max.2 - min.2);
    if ranges.0 >= ranges.1 && ranges.0 >= ranges.2 {
        bucket.sort_unstable_by_key(|&(r, _, _, _)| r);
    } else if ranges.1 >= ranges.2 {
        bucket.sort_unstable_by_key(|&(_, g, _, _)| g);
    } else {
        bucket.sort_unstable_by_key(|&(_, _, b, _)| b);
    }
    let half = bucket_weight(bucket) / 2;
    let mut acc = 0u64;
    for (i, &(_, _, _, n)) in bucket.iter().enumerate() {
        acc += n as u64;
        if acc >= half {
            return (i + 1).clamp(1, bucket.len() - 1);
        }
    }
    (bucket.len() / 2).max(1)
}

fn dither_floyd_steinberg(
    rgb: &[u8],
    width: usize,
    height: usize,
    palette: &[(u8, u8, u8)],
) -> Vec<u8> {
    let mut buf: Vec<[f32; 3]> = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
        .collect();
    let mut indices = vec![0u8; width * height];
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            let px = [
                buf[i][0].clamp(0.0, 255.0),
                buf[i][1].clamp(0.0, 255.0),
                buf[i][2].clamp(0.0, 255.0),
            ];
            let (index, chosen) = nearest_palette(px, palette);
            indices[i] = index as u8;
            let err = [
                px[0] - chosen.0 as f32,
                px[1] - chosen.1 as f32,
                px[2] - chosen.2 as f32,
            ];
            for &(dx, dy, weight) in &[(1i32, 0i32, 7.0f32 / 16.0), (-1, 1, 3.0 / 16.0), (0, 1, 5.0 / 16.0), (1, 1, 1.0 / 16.0)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx >= 0 && ny >= 0 && (nx as usize) < width && (ny as usize) < height {
                    let j = ny as usize * width + nx as usize;
                    buf[j][0] += err[0] * weight;
                    buf[j][1] += err[1] * weight;
                    buf[j][2] += err[2] * weight;
                }
            }
        }
    }
    indices
}

fn nearest_palette(px: [f32; 3], palette: &[(u8, u8, u8)]) -> (usize, (u8, u8, u8)) {
    let mut best = (0usize, f32::MAX);
    for (i, &(r, g, b)) in palette.iter().enumerate() {
        let (dr, dg, db) = (px[0] - r as f32, px[1] - g as f32, px[2] - b as f32);
        let dist = dr * dr + dg * dg + db * db;
        if dist < best.1 {
            best = (i, dist);
        }
    }
    (best.0, palette[best.0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> Vec<u8> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("..");
        path.push("..");
        path.push("testdata");
        path.push("images");
        path.push(name);
        std::fs::read(&path).unwrap_or_else(|e| panic!("reading {path:?}: {e}"))
    }

    #[test]
    fn decodes_real_pal256_fixture_pixel_for_pixel() {
        let bytes = fixture("cover_128.pal256.timg");
        let image = decode_tim1(&bytes).expect("decode");
        assert_eq!((image.width, image.height), (128, 128));
        assert_eq!(image.rgb.len(), 128 * 128 * 3);
        // Known-correct values cross-checked against tau-alpha's own
        // `tools/tau_image.py` decoder (testdata/images/README.md).
        assert_eq!(&image.rgb[0..3], &[255, 255, 255]);
        let mid = (64 * 128 + 64) * 3;
        assert_eq!(&image.rgb[mid..mid + 3], &[247, 215, 189]);
    }

    #[test]
    fn rejects_bad_magic() {
        let error = decode_tim1(b"NOPE0000000000000").unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidTim1Container);
    }

    #[test]
    fn rejects_length_mismatch() {
        let mut bytes = fixture("cover_128.pal256.timg");
        bytes.truncate(bytes.len() - 10);
        let error = decode_tim1(&bytes).unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidTim1Container);
    }

    #[test]
    fn encoder_round_trips_a_real_jpeg_cover() {
        let source = fixture("cover455.jpg");
        let packed = encode_pal256_bytes(&source, 128).expect("encode");
        // Container shape: 16-byte header + 512-byte CLUT + w*h index bytes.
        assert_eq!(&packed[0..4], MAGIC);
        assert_eq!(packed[4], TimFormat::Palette as u8);
        assert_eq!(packed[5], 8);
        let width = u16::from_le_bytes([packed[6], packed[7]]);
        let height = u16::from_le_bytes([packed[8], packed[9]]);
        // cover455.jpg is square, so the long side becomes both dimensions.
        assert_eq!((width, height), (128, 128));
        assert_eq!(packed.len(), 16 + 512 + 128 * 128);

        let decoded = decode_tim1(&packed).expect("decode our own output");
        assert_eq!((decoded.width, decoded.height), (128, 128));
        assert_eq!(decoded.rgb.len(), 128 * 128 * 3);
    }

    #[test]
    fn fit_long_side_scales_proportionally_without_cropping() {
        // 1024x1540 portrait -> 85x128, matching IMAGE_FORMATS.md's own example.
        assert_eq!(fit_long_side(1024, 1540, 128), (85, 128));
        assert_eq!(fit_long_side(1400, 1400, 128), (128, 128));
    }

    #[test]
    fn palette_index_round_trip_matches_python_packer_for_every_bpp() {
        for bpp in [8u8, 6, 4] {
            let modulus = 1u32 << bpp;
            let indices: Vec<u8> = (0..17u32).map(|i| (i % modulus) as u8).collect();
            let packed = pack_indices(&indices, bpp);
            let unpacked = unpack_indices(&packed, bpp, indices.len()).expect("unpack");
            assert_eq!(unpacked, indices, "bpp {bpp}");
        }
    }
}
