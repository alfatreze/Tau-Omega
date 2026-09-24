//! Decoding a core's `icon.bin` (`Cores/<id>/icon.bin`): a 36x36 monochrome
//! bitmap, 16 bits per pixel (brightness in the first byte of each pair,
//! `0xFF` = fully on), stored rotated 90 degrees counter-clockwise --
//! `tau-alpha`'s own `analogue-pocket-dev` skill documents this shape from
//! Analogue's SD-packaging notes. Verified here against the real shipped
//! `alfatreze.TAU` icon: decoded and rendered, then compared pixel-for-pixel
//! against `tau-alpha/assets/branding/author-icon.png`, the same mark drawn
//! by hand -- an exact silhouette match once rotated 90 degrees clockwise to
//! undo the stored rotation confirmed both the byte position and the
//! rotation direction, not just the width/height/stride arithmetic.

use crate::{ErrorCode, TauError};

const SIZE: usize = 36;
const EXPECTED_BYTES: usize = SIZE * SIZE * 2;

/// Decodes `icon.bin` bytes into a PNG (grayscale + alpha: opaque white
/// where the icon is "on", transparent elsewhere, so it composites over any
/// background colour a caller places it on). Returns an error for anything
/// that isn't exactly the expected 36x36x16bpp size -- a core with a
/// present-but-malformed icon should say so, not render something wrong.
pub fn decode_icon_bin(bytes: &[u8]) -> Result<Vec<u8>, TauError> {
    if bytes.len() != EXPECTED_BYTES {
        return Err(TauError::e(
            ErrorCode::Io,
            format!(
                "icon.bin: expected {EXPECTED_BYTES} bytes (36x36, 16 bits per pixel), got {}",
                bytes.len()
            ),
        ));
    }

    // Stored rotated 90 degrees CCW; undo that here (rotate 90 degrees CW):
    // upright[row][col] = stored[SIZE-1-col][row].
    let mut alpha = vec![0u8; SIZE * SIZE];
    for row in 0..SIZE {
        for col in 0..SIZE {
            let src_row = SIZE - 1 - col;
            let src_col = row;
            alpha[row * SIZE + col] = bytes[(src_row * SIZE + src_col) * 2];
        }
    }

    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, SIZE as u32, SIZE as u32);
        encoder.set_color(png::ColorType::GrayscaleAlpha);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| TauError::e(ErrorCode::Io, format!("icon.bin: {error}")))?;
        let mut pixels = Vec::with_capacity(SIZE * SIZE * 2);
        for value in &alpha {
            pixels.push(255);
            pixels.push(*value);
        }
        writer
            .write_image_data(&pixels)
            .map_err(|error| TauError::e(ErrorCode::Io, format!("icon.bin: {error}")))?;
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

    /// The real shipped `alfatreze.TAU` icon, extracted directly from the
    /// real release zip already used by `crate::package`'s own tests --
    /// no separate binary fixture needed.
    fn real_icon_bytes() -> Vec<u8> {
        let zip_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/packages/alfatreze.TAU_0.4.0_2026-09-22.zip");
        let file = File::open(zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut entry = archive.by_name("Cores/alfatreze.TAU/icon.bin").unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn decodes_the_real_shipped_icon_to_a_valid_png() {
        let bytes = real_icon_bytes();
        assert_eq!(bytes.len(), EXPECTED_BYTES);
        let png_bytes = decode_icon_bin(&bytes).unwrap();
        // A minimal, real round-trip check: re-decode the PNG we just wrote
        // and confirm it reports the exact dimensions and color type this
        // module promises, rather than only checking "no error".
        let mut decoder = png::Decoder::new(Cursor::new(&png_bytes));
        decoder.set_transformations(png::Transformations::IDENTITY);
        let mut reader = decoder.read_info().unwrap();
        assert_eq!(reader.info().width, SIZE as u32);
        assert_eq!(reader.info().height, SIZE as u32);
        let mut buffer = vec![0u8; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buffer).unwrap();
        assert_eq!(info.color_type, png::ColorType::GrayscaleAlpha);
        // Real icon: some pixels fully on, some fully off, not a blank image.
        let alpha_values: Vec<u8> = buffer[..info.buffer_size()]
            .chunks_exact(2)
            .map(|pair| pair[1])
            .collect();
        assert!(alpha_values.iter().any(|&a| a > 200));
        assert!(alpha_values.iter().any(|&a| a < 50));
    }

    #[test]
    fn rejects_a_wrong_sized_buffer_instead_of_misreading_it() {
        let error = decode_icon_bin(&[0u8; 100]).unwrap_err();
        assert_eq!(error.code(), ErrorCode::Io);
    }
}
