//! Decoding Tau's pixel-grid report codes (TPG1/TPG2): the default view of every Check, sweep
//! and Info report since the Tau Alpha firmware of 2026-10-06. A port of the decoder in
//! `tau-alpha/tools/tpg.py` (format notes: `tau-alpha/docs/features/BARCODE_STUDY.md`), read-only;
//! the record it returns is the same `TAUD1` record the QR code carries (see [`crate::taud`]).
//!
//! The Pocket's screenshot is a lossless 400x360 PNG of exact RGB565 values. Mode L stores the
//! record in those values (16 bits per pixel); mode R stores 6 bits per 4x4 cell, two per
//! channel. Only an exact 400x360 screenshot is read here: a resized or recompressed copy (which
//! `tpg.py` can sometimes resample) reports "no grid found" rather than a guess.
//! There is no error correction; the payload CRC32 makes damage fail loudly.

use crate::{ErrorCode, TauError};
use std::{fs::File, io::BufReader, path::Path};

const W: usize = 400;
const H: usize = 360;
const CELL: usize = 4;
const HDR: usize = 16;
const LADDER_L: [usize; 12] = [64, 80, 96, 112, 128, 160, 192, 224, 256, 288, 320, 360];
const LADDER_R: [usize; 11] = [20, 24, 28, 32, 40, 48, 56, 64, 72, 80, 90];
const MODE_L: u8 = 0;
const MODE_R: u8 = 1;

fn units(side: usize, mode: u8) -> usize {
    if mode == MODE_L {
        side * side * 2
    } else {
        side * side * 6 / 8
    }
}

fn capacity(mode: u8) -> usize {
    let last = if mode == MODE_L {
        LADDER_L[LADDER_L.len() - 1]
    } else {
        LADDER_R[LADDER_R.len() - 1]
    };
    units(last, mode) - HDR
}

/// `Some((length, crc))` when the 16 header bytes carry this magic and mode.
fn parse_header(
    head: &[u8],
    mode: u8,
    magic: &[u8; 4],
    cap: usize,
) -> Result<Option<(usize, u32)>, TauError> {
    if head.len() < HDR || &head[..4] != magic || head[4] != mode {
        return Ok(None);
    }
    let n = u32::from_be_bytes([head[8], head[9], head[10], head[11]]) as usize;
    let crc = u32::from_be_bytes([head[12], head[13], head[14], head[15]]);
    if n > cap {
        return Err(damaged("the pixel grid header length is impossible"));
    }
    Ok(Some((n, crc)))
}

fn damaged(what: &str) -> TauError {
    TauError::e(
        ErrorCode::Io,
        format!("{what} (the screenshot was altered or cropped, or too much damage)"),
    )
}

fn check(raw: &[u8], n: usize, crc: u32, what: &str) -> Result<Vec<u8>, TauError> {
    if raw.len() != n || crc32fast::hash(raw) != crc {
        return Err(damaged(&format!("{what}: CRC mismatch")));
    }
    Ok(raw.to_vec())
}

fn rgb565(px: &[u8]) -> u16 {
    ((px[0] as u16 >> 3) << 11) | ((px[1] as u16 >> 2) << 5) | (px[2] as u16 >> 3)
}

fn lossless_stream(rgb: &[u8], x0: usize, y0: usize, side: usize, bytes: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes + 1);
    'rows: for y in 0..side {
        for x in 0..side {
            if out.len() >= bytes {
                break 'rows;
            }
            let v = rgb565(&rgb[((y0 + y) * W + x0 + x) * 3..][..3]);
            out.push((v >> 8) as u8);
            out.push(v as u8);
        }
    }
    out
}

/// The levels (0..3 per channel) of the first `cells` cells of a robust block, row-major, as
/// the bit stream they carry: the mean of the 2x2 centre of each 4x4 cell, rounded to the
/// nearest of the four levels (0, 85, 170, 255 after expansion).
fn robust_stream(rgb: &[u8], x0: usize, y0: usize, side: usize, cells: usize) -> Vec<u8> {
    let mut bits: Vec<u8> = Vec::with_capacity(cells * 6);
    for i in 0..cells.min(side * side) {
        let (cx, cy) = (i % side, i / side);
        let mut level = [0u8; 3];
        for (c, slot) in level.iter_mut().enumerate() {
            let mut sum = 0u32;
            for dy in 1..3 {
                for dx in 1..3 {
                    sum += rgb[((y0 + cy * CELL + dy) * W + x0 + cx * CELL + dx) * 3 + c] as u32;
                }
            }
            // mean = sum / 4; level = round(mean / 85), clipped to 0..=3
            *slot = (((sum as f32 / 4.0) / 85.0).round() as i32).clamp(0, 3) as u8;
        }
        for l in level {
            bits.push(l >> 1);
            bits.push(l & 1);
        }
    }
    bits.as_chunks::<8>()
        .0
        .iter()
        .map(|b| b.iter().fold(0u8, |a, &bit| (a << 1) | bit))
        .collect()
}

/// Decodes the record from an exact 400x360 RGB image. `Ok(None)`: no pixel grid in it.
/// `Err`: a grid header was found but the payload fails its CRC.
pub fn decode_rgb(width: usize, height: usize, rgb: &[u8]) -> Result<Option<Vec<u8>>, TauError> {
    if width != W || height != H || rgb.len() < W * H * 3 {
        return Ok(None);
    }
    // Mode L, TPG2: the header sits in the first pixels of the centred block of each ladder step.
    for &s in &LADDER_L {
        let (x0, y0) = ((W - s) / 2, (H - s) / 2);
        let head = lossless_stream(rgb, x0, y0, s.min(HDR / 2), HDR);
        if let Some((n, crc)) = parse_header(&head, MODE_L, b"TPG2", capacity(MODE_L))? {
            let all = lossless_stream(rgb, x0, y0, s, HDR + n);
            return check(&all[HDR..(HDR + n).min(all.len())], n, crc, "TPG2 mode L").map(Some);
        }
    }
    // Legacy TPG1 mode L: from pixel (0, 0) over the whole screen.
    let flat: Vec<u8> = (0..W * H)
        .flat_map(|i| {
            let v = rgb565(&rgb[i * 3..][..3]);
            [(v >> 8) as u8, v as u8]
        })
        .collect();
    if let Some((n, crc)) = parse_header(&flat, MODE_L, b"TPG1", W * H * 2 - HDR)? {
        return check(&flat[HDR..HDR + n], n, crc, "TPG1 mode L").map(Some);
    }
    // Mode R, TPG2: probe the 22 header cells of each ladder step.
    for &s in &LADDER_R {
        let px = s * CELL;
        let (x0, y0) = ((W - px) / 2, (H - px) / 2);
        let head = robust_stream(rgb, x0, y0, s, (HDR * 8).div_ceil(6));
        if let Some((n, crc)) = parse_header(&head, MODE_R, b"TPG2", capacity(MODE_R))? {
            let all = robust_stream(rgb, x0, y0, s, s * s);
            return check(&all[HDR..(HDR + n).min(all.len())], n, crc, "TPG2 mode R").map(Some);
        }
    }
    // Legacy TPG1 mode R: a 100 x 90 cell grid from (0, 0).
    let (gw, gh) = (W / CELL, H / CELL);
    let mut bits = Vec::new();
    for i in 0..gw * gh {
        let (cx, cy) = (i % gw, i / gw);
        for c in 0..3 {
            let mut sum = 0u32;
            for dy in 1..3 {
                for dx in 1..3 {
                    sum += rgb[((cy * CELL + dy) * W + cx * CELL + dx) * 3 + c] as u32;
                }
            }
            let l = (((sum as f32 / 4.0) / 85.0).round() as i32).clamp(0, 3) as u8;
            bits.push(l >> 1);
            bits.push(l & 1);
        }
    }
    let f: Vec<u8> = bits
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| b.iter().fold(0u8, |a, &bit| (a << 1) | bit))
        .collect();
    if let Some((n, crc)) = parse_header(&f, MODE_R, b"TPG1", gw * gh * 6 / 8 - HDR)? {
        return check(&f[HDR..HDR + n], n, crc, "TPG1 mode R").map(Some);
    }
    Ok(None)
}

/// PNG file to (width, height, packed RGB). Alpha is dropped; greyscale is replicated.
pub fn read_png_rgb(path: &Path) -> Result<(usize, usize, Vec<u8>), TauError> {
    let mut decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|e| TauError::e(ErrorCode::Io, format!("invalid PNG: {e}")))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| TauError::e(ErrorCode::Io, "PNG frame too large to decode"))?;
    let mut buf = vec![0u8; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| TauError::e(ErrorCode::Io, format!("invalid PNG frame: {e}")))?;
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => {
            return Err(TauError::e(ErrorCode::Io, "indexed PNG was not expanded"));
        }
    };
    let mut rgb = Vec::with_capacity(info.width as usize * info.height as usize * 3);
    for p in buf[..info.buffer_size()].chunks_exact(channels) {
        if channels <= 2 {
            rgb.extend_from_slice(&[p[0]; 3])
        } else {
            rgb.extend_from_slice(&p[..3])
        }
    }
    Ok((info.width as usize, info.height as usize, rgb))
}

/// Reads the pixel-grid record from a PNG screenshot; `Ok(None)` when it has no grid.
pub fn read_record(path: impl AsRef<Path>) -> Result<Option<Vec<u8>>, TauError> {
    let (w, h, rgb) = read_png_rgb(path.as_ref())?;
    decode_rgb(w, h, &rgb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taud;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/screenshots")
            .join(name)
    }

    fn qr_record(name: &str) -> Vec<u8> {
        taud::from_text(&taud::read_qr_text(fixture(name)).unwrap()).unwrap()
    }

    // Each real capture's grid record must equal the QR record of the same report, byte for byte
    // (tau-alpha checked the same equality with decode_tau_suite.py --grid and --qr, 2026-10-04).
    #[test]
    fn real_grids_equal_their_qr_records() {
        let cases = [
            ("20261004_190911.png", "20261004_190900.png"), // TPG1 mode L
            ("20261004_190919.png", "20261004_190900.png"), // TPG1 mode R
            ("20261004_194758.png", "20261004_194810.png"), // TPG2 R, Info export
            ("20261004_194804.png", "20261004_194810.png"), // TPG2 L, Info export
            ("20261004_194856.png", "20261004_194907.png"), // TPG2 R, Check
            ("20261004_194901.png", "20261004_194907.png"), // TPG2 L, Check
        ];
        for (grid, qr) in cases {
            let record = read_record(fixture(grid))
                .unwrap()
                .unwrap_or_else(|| panic!("{grid}: no grid"));
            assert_eq!(record, qr_record(qr), "{grid}");
            taud::parse_record(&record).unwrap();
        }
    }

    #[test]
    fn a_qr_or_ordinary_screenshot_has_no_grid() {
        assert!(
            read_record(fixture("20261004_194810.png"))
                .unwrap()
                .is_none()
        );
        assert!(
            read_record(fixture("20260921_231822.png"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn one_call_reads_either_view() {
        let a = taud::read_screenshot_report(fixture("20261004_194856.png")).unwrap();
        let b = taud::read_screenshot_report(fixture("20261004_194907.png")).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn a_damaged_grid_fails_its_crc() {
        let (w, h, mut rgb) = read_png_rgb(&fixture("20261004_194804.png")).unwrap();
        // flip a payload pixel in the first row of the 64 px lossless block
        let i = (((h - 64) / 2) * w + (w - 64) / 2 + 10) * 3;
        rgb[i] ^= 0xF8;
        assert!(decode_rgb(w, h, &rgb).is_err());
    }

    // The only real capture with the final tag numbers (26 Info rows, 27 now playing): the Info
    // export of 2026-10-08, from tau-alpha's probe-2 folder. Expected values cross-checked with
    // decode_tau_suite.py --grid --json.
    #[test]
    fn decodes_info_rows_and_now_playing_from_a_real_grid() {
        let r = taud::read_screenshot_report(fixture("20261008_223548.png")).unwrap();
        assert_eq!(r.profile, "none");
        assert!(r.unknown.is_empty());
        assert_eq!(r.entries.info_rows.len(), 37);
        let first = &r.entries.info_rows[0];
        assert_eq!(
            (first.row, first.label.as_str(), first.value.as_str()),
            (0, "FIRMWARE", "0.6.0")
        );
        let last = r.entries.info_rows.last().unwrap();
        assert_eq!(
            (last.row, last.label.as_str(), last.value.as_str()),
            (36, "GAP LATENCY", "-")
        );
        let np = r.entries.now_playing.as_ref().unwrap();
        assert_eq!(
            (
                np.state.as_str(),
                np.queue_pos,
                np.queue_len,
                np.title.as_str()
            ),
            ("nothing loaded", 0, 0, "")
        );
    }
}
