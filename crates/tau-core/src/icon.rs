//! Decoding the two monochrome bitmap formats the Analogue Platform
//! Framework ships alongside a core: `Cores/<id>/icon.bin` (a small 36x36
//! icon) and `Platforms/_images/<platform>.bin` (a 521x165 banner -- the
//! actual "artwork" for a platform, shared by every core on it, as opposed
//! to the icon's small-list-row role). Both are 16 bits per pixel
//! (brightness in the first byte of each pair, `0xFF` = fully on), stored
//! rotated 90 degrees counter-clockwise -- `tau-alpha`'s own
//! `analogue-pocket-dev` skill documents this shape from Analogue's
//! SD-packaging notes (`references/sd-packaging-assets.md`), though only for
//! the icon; the banner's format was confirmed here to be the same family
//! empirically, the same way the icon's own ambiguities were resolved:
//! decoded the real shipped `alfatreze.TAU` icon and its platform banner
//! several ways (no rotation, CW, CCW, 180 degrees; stored-dimensions
//! swapped for the non-square banner) and compared each against a real
//! reference -- the icon against `tau-alpha/assets/branding/author-icon.png`
//! (the same emblem drawn by hand), the banner by simply looking at the
//! result (a real "TAUα" wordmark on graph paper appeared only for the
//! 90-degree-clockwise, swapped-dimensions case). Both landed on the same
//! answer: brightness is the first byte of each pixel pair (the source
//! doc's "upper byte" language was about big-endian byte order, not a
//! machine word's low/high split -- confirmed for the icon by checking the
//! raw byte range, which only ever spanned 0-255 as little-endian 16-bit
//! words), and 90 degrees clockwise undoes the stored rotation.

use crate::{ErrorCode, TauError};

const ICON_SIZE: usize = 36;
const ICON_BYTES: usize = ICON_SIZE * ICON_SIZE * 2;

const BANNER_WIDTH: usize = 521;
const BANNER_HEIGHT: usize = 165;
const BANNER_BYTES: usize = BANNER_WIDTH * BANNER_HEIGHT * 2;

/// Decodes `icon.bin` bytes into a PNG (grayscale + alpha: opaque white
/// where the icon is "on", transparent elsewhere, so it composites over any
/// background colour a caller places it on). Returns an error for anything
/// that isn't exactly the expected 36x36x16bpp size -- a core with a
/// present-but-malformed icon should say so, not render something wrong.
pub fn decode_icon_bin(bytes: &[u8]) -> Result<Vec<u8>, TauError> {
    if bytes.len() != ICON_BYTES {
        return Err(TauError::e(
            ErrorCode::Io,
            format!(
                "icon.bin: expected {ICON_BYTES} bytes (36x36, 16 bits per pixel), got {}",
                bytes.len()
            ),
        ));
    }
    decode_monochrome_bitmap(bytes, ICON_SIZE, ICON_SIZE, "icon.bin")
}

/// Decodes a platform banner (`Platforms/_images/<platform>.bin`) into the
/// same kind of PNG `decode_icon_bin` produces, at its own 521x165 size.
/// This is the real per-platform "artwork" -- shared by every core that
/// declares the platform, not a second copy of the small icon.
pub fn decode_platform_image(bytes: &[u8]) -> Result<Vec<u8>, TauError> {
    if bytes.len() != BANNER_BYTES {
        return Err(TauError::e(
            ErrorCode::Io,
            format!(
                "platform image: expected {BANNER_BYTES} bytes ({BANNER_WIDTH}x{BANNER_HEIGHT}, 16 bits per pixel), got {}",
                bytes.len()
            ),
        ));
    }
    decode_monochrome_bitmap(bytes, BANNER_WIDTH, BANNER_HEIGHT, "platform image")
}

/// Shared decode for both formats. `width`/`height` describe the final,
/// upright image; the raw buffer holds it rotated 90 degrees CCW, so for a
/// non-square bitmap the buffer's own row length is `height`, not `width`
/// (rotating swaps the dimensions) -- confirmed against the real 521x165
/// platform banner, which is stored as 165 rows of 521 pixels each, not the
/// other way round.
fn decode_monochrome_bitmap(
    bytes: &[u8],
    width: usize,
    height: usize,
    label: &str,
) -> Result<Vec<u8>, TauError> {
    // upright[row][col] = stored[width-1-col][row], where `stored` has
    // `width` rows of `height` pixels each.
    let mut alpha = vec![0u8; width * height];
    for row in 0..height {
        for col in 0..width {
            let stored_row = width - 1 - col;
            let stored_col = row;
            alpha[row * width + col] = bytes[(stored_row * height + stored_col) * 2];
        }
    }

    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, width as u32, height as u32);
        encoder.set_color(png::ColorType::GrayscaleAlpha);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| TauError::e(ErrorCode::Io, format!("{label}: {error}")))?;
        let mut pixels = Vec::with_capacity(width * height * 2);
        for value in &alpha {
            pixels.push(255);
            pixels.push(*value);
        }
        writer
            .write_image_data(&pixels)
            .map_err(|error| TauError::e(ErrorCode::Io, format!("{label}: {error}")))?;
    }
    Ok(png_bytes)
}

/// How a picture of another shape is fitted into the fixed size of an icon or a banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Fit {
    /// The whole picture, centred, with empty (off) space where the shapes differ.
    #[default]
    Contain,
    /// The picture fills the whole area; what does not fit is cropped from the centre.
    Cover,
    /// The picture is stretched to the area, ignoring its shape.
    Stretch,
}

/// A PNG or JPEG as brightness values (0 = off, 255 = on): luminance times opacity, so a white emblem on a transparent background is "on" where it is
/// drawn and a JPEG (which has no transparency) is its grey picture.
fn load_brightness(bytes: &[u8]) -> Result<(usize, usize, Vec<u8>), TauError> {
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let (w, h, rgb) = crate::image::decode_source_image(bytes)?;
        let lum = rgb
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| {
                ((u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000)
                    as u8
            })
            .collect();
        return Ok((w as usize, h as usize, lum));
    }
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|e| TauError::e(ErrorCode::Io, format!("PNG: {e}")))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| TauError::e(ErrorCode::Io, "PNG: too large"))?;
    let mut buf = vec![0u8; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| TauError::e(ErrorCode::Io, format!("PNG: {e}")))?;
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => {
            return Err(TauError::e(
                ErrorCode::Io,
                "PNG: an indexed PNG was not expanded",
            ));
        }
    };
    let (w, h) = (info.width as usize, info.height as usize);
    let mut out = Vec::with_capacity(w * h);
    for p in buf[..info.buffer_size()].chunks_exact(channels) {
        let (lum, alpha) = match channels {
            1 => (u32::from(p[0]), 255),
            2 => (u32::from(p[0]), u32::from(p[1])),
            3 => (
                (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000,
                255,
            ),
            _ => (
                (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000,
                u32::from(p[3]),
            ),
        };
        out.push(((lum * alpha + 127) / 255) as u8);
    }
    Ok((w, h, out))
}

fn resize_gray(
    src: &[u8],
    sw: usize,
    sh: usize,
    dw: usize,
    dh: usize,
) -> Result<Vec<u8>, TauError> {
    if (sw, sh) == (dw, dh) {
        return Ok(src.to_vec());
    }
    let mut dst = vec![0u8; dw * dh];
    resize::new(sw, sh, dw, dh, resize::Pixel::Gray8, resize::Type::Lanczos3)
        .and_then(|mut r| {
            r.resize(
                rgb::FromSlice::as_gray(src),
                rgb::FromSlice::as_gray_mut(&mut dst[..]),
            )
        })
        .map_err(|e| TauError::e(ErrorCode::Io, format!("resize: {e}")))?;
    Ok(dst)
}

/// Brightness values of `bytes` fitted into `dw` x `dh` (row by row, top first).
fn fit_brightness(bytes: &[u8], dw: usize, dh: usize, fit: Fit) -> Result<Vec<u8>, TauError> {
    let (sw, sh, src) = load_brightness(bytes)?;
    if sw == 0 || sh == 0 {
        return Err(TauError::e(ErrorCode::Io, "The picture is empty."));
    }
    match fit {
        Fit::Stretch => resize_gray(&src, sw, sh, dw, dh),
        Fit::Contain | Fit::Cover => {
            // scale so the picture fits inside (Contain) or covers (Cover) the area, keeping its shape
            let (fw, fh) = (dw as f64 / sw as f64, dh as f64 / sh as f64);
            let k = if fit == Fit::Contain {
                fw.min(fh)
            } else {
                fw.max(fh)
            };
            let (tw, th) = (
                ((sw as f64 * k).round() as usize).max(1),
                ((sh as f64 * k).round() as usize).max(1),
            );
            let scaled = resize_gray(&src, sw, sh, tw, th)?;
            let mut out = vec![0u8; dw * dh];
            for y in 0..dh {
                for x in 0..dw {
                    let sx = x as i64 + (tw as i64 - dw as i64).div_euclid(2);
                    let sy = y as i64 + (th as i64 - dh as i64).div_euclid(2);
                    if sx >= 0 && sy >= 0 && (sx as usize) < tw && (sy as usize) < th {
                        out[y * dw + x] = scaled[sy as usize * tw + sx as usize];
                    }
                }
            }
            Ok(out)
        }
    }
}

/// Packs upright brightness values into the Pocket's stored form: 16 bits per pixel (brightness first), rotated 90 degrees counter-clockwise. The inverse
/// of the decoder.
fn pack_stored(upright: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 2];
    for row in 0..height {
        for col in 0..width {
            out[((width - 1 - col) * height + row) * 2] = upright[row * width + col];
        }
    }
    out
}

/// A PNG or JPEG as an `icon.bin` (the 36x36 author icon the Pocket draws beside the core), fitted by `fit`. White or bright is "on".
pub fn encode_icon_from_image(bytes: &[u8], fit: Fit) -> Result<Vec<u8>, TauError> {
    Ok(pack_stored(
        &fit_brightness(bytes, ICON_SIZE, ICON_SIZE, fit)?,
        ICON_SIZE,
        ICON_SIZE,
    ))
}

/// A PNG or JPEG as a platform banner (`Platforms/_images/<platform>.bin`, 521x165), fitted by `fit`.
pub fn encode_platform_image(bytes: &[u8], fit: Fit) -> Result<Vec<u8>, TauError> {
    Ok(pack_stored(
        &fit_brightness(bytes, BANNER_WIDTH, BANNER_HEIGHT, fit)?,
        BANNER_WIDTH,
        BANNER_HEIGHT,
    ))
}

/// Encodes a PNG as an `icon.bin` when it is exactly 36x36 (the strict form: no scaling). The inverse of [`decode_icon_bin`]; see
/// [`encode_icon_from_image`] for any size or a JPEG.
pub fn encode_icon_bin(png_bytes: &[u8]) -> Result<Vec<u8>, TauError> {
    let (w, h, _) = load_brightness(png_bytes)?;
    if (w, h) != (ICON_SIZE, ICON_SIZE) {
        return Err(TauError::e(
            ErrorCode::Io,
            format!(
                "The icon must be exactly {ICON_SIZE}x{ICON_SIZE} pixels; this one is {w}x{h}."
            ),
        ));
    }
    encode_icon_from_image(png_bytes, Fit::Stretch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::File,
        io::{Cursor, Read},
        path::Path,
    };

    /// A real file extracted directly from the real release zip already
    /// used by `crate::package`'s own tests -- no separate binary fixture
    /// needed.
    fn real_bytes(entry_name: &str) -> Vec<u8> {
        let zip_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/packages/alfatreze.TAU_0.4.0_2026-09-22.zip");
        let file = File::open(zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut entry = archive.by_name(entry_name).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        bytes
    }

    fn assert_valid_png(png_bytes: &[u8], width: u32, height: u32) {
        let mut decoder = png::Decoder::new(Cursor::new(png_bytes));
        decoder.set_transformations(png::Transformations::IDENTITY);
        let mut reader = decoder.read_info().unwrap();
        assert_eq!(reader.info().width, width);
        assert_eq!(reader.info().height, height);
        let mut buffer = vec![0u8; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buffer).unwrap();
        assert_eq!(info.color_type, png::ColorType::GrayscaleAlpha);
        // Real artwork: some pixels fully on, some fully off, not blank.
        let alpha_values: Vec<u8> = buffer[..info.buffer_size()]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| pair[1])
            .collect();
        assert!(alpha_values.iter().any(|&a| a > 200));
        assert!(alpha_values.iter().any(|&a| a < 50));
    }

    #[test]
    fn decodes_the_real_shipped_icon_to_a_valid_png() {
        let bytes = real_bytes("Cores/alfatreze.TAU/icon.bin");
        assert_eq!(bytes.len(), ICON_BYTES);
        let png_bytes = decode_icon_bin(&bytes).unwrap();
        assert_valid_png(&png_bytes, ICON_SIZE as u32, ICON_SIZE as u32);
    }

    #[test]
    fn decodes_the_real_shipped_platform_banner_to_a_valid_png() {
        let bytes = real_bytes("Platforms/_images/tau.bin");
        assert_eq!(bytes.len(), BANNER_BYTES);
        let png_bytes = decode_platform_image(&bytes).unwrap();
        assert_valid_png(&png_bytes, BANNER_WIDTH as u32, BANNER_HEIGHT as u32);
    }

    #[test]
    fn rejects_a_wrong_sized_icon_buffer_instead_of_misreading_it() {
        let error = decode_icon_bin(&[0u8; 100]).unwrap_err();
        assert_eq!(error.code(), ErrorCode::Io);
    }

    #[test]
    fn rejects_a_wrong_sized_banner_buffer_instead_of_misreading_it() {
        let error = decode_platform_image(&[0u8; 100]).unwrap_err();
        assert_eq!(error.code(), ErrorCode::Io);
    }

    #[test]
    fn an_icon_png_survives_encode_then_decode_and_a_wrong_size_is_refused() {
        let mut px = Vec::new();
        for i in 0..ICON_SIZE * ICON_SIZE {
            px.push(255);
            px.push(((i * 7) % 256) as u8);
        }
        let mut png_bytes = Vec::new();
        {
            let mut e = png::Encoder::new(&mut png_bytes, ICON_SIZE as u32, ICON_SIZE as u32);
            e.set_color(png::ColorType::GrayscaleAlpha);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&px).unwrap();
        }
        let bin = encode_icon_bin(&png_bytes).unwrap();
        assert_eq!(bin.len(), ICON_BYTES);
        assert_eq!(
            encode_icon_bin(&decode_icon_bin(&bin).unwrap()).unwrap(),
            bin,
            "encode is the inverse of decode"
        );
        let mut small = Vec::new();
        {
            let mut e = png::Encoder::new(&mut small, 8, 8);
            e.set_color(png::ColorType::Grayscale);
            e.set_depth(png::BitDepth::Eight);
            e.write_header()
                .unwrap()
                .write_image_data(&[0u8; 64])
                .unwrap();
        }
        assert!(
            encode_icon_bin(&small)
                .unwrap_err()
                .message
                .contains("36x36")
        );
    }

    fn gray_png(w: u32, h: u32, f: impl Fn(u32, u32) -> u8) -> Vec<u8> {
        let mut px = Vec::new();
        for y in 0..h {
            for x in 0..w {
                px.push(f(x, y));
            }
        }
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, w, h);
            e.set_color(png::ColorType::Grayscale);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&px).unwrap();
        }
        out
    }

    #[test]
    fn any_size_png_becomes_a_banner_and_an_icon_fitted_as_asked() {
        let src = gray_png(
            400,
            100,
            |x, _| if (100..300).contains(&x) { 255 } else { 0 },
        );
        let banner = encode_platform_image(&src, Fit::Contain).unwrap();
        assert_eq!(banner.len(), BANNER_BYTES);
        assert!(decode_platform_image(&banner).is_ok());
        // stored[(w-1-x)*h + y]: upright pixel (x, y)
        let up =
            |bin: &[u8], w: usize, h: usize, x: usize, y: usize| bin[((w - 1 - x) * h + y) * 2];
        assert_eq!(
            up(&banner, BANNER_WIDTH, BANNER_HEIGHT, 260, 82),
            255,
            "the bar's middle is on"
        );
        assert_eq!(
            up(&banner, BANNER_WIDTH, BANNER_HEIGHT, 2, 82),
            0,
            "the left edge is off"
        );
        let wide = gray_png(200, 100, |_, _| 255);
        let icon_c = encode_icon_from_image(&wide, Fit::Contain).unwrap();
        let icon_f = encode_icon_from_image(&wide, Fit::Cover).unwrap();
        assert_eq!(
            up(&icon_c, ICON_SIZE, ICON_SIZE, 18, 0),
            0,
            "padding above a contained picture"
        );
        assert_eq!(up(&icon_c, ICON_SIZE, ICON_SIZE, 18, 18), 255);
        assert_eq!(
            up(&icon_f, ICON_SIZE, ICON_SIZE, 18, 0),
            255,
            "a covered picture fills the area"
        );
        assert!(
            encode_icon_bin(&gray_png(8, 8, |_, _| 9))
                .unwrap_err()
                .message
                .contains("36x36")
        );
        assert!(encode_icon_from_image(b"not an image", Fit::Contain).is_err());
    }
}
