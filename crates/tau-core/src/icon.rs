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
            .chunks_exact(2)
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
}
