//! P2-2: a lightweight, dependency-free, deterministic complement to
//! `fuzz/fuzz_targets/parse_index.rs` (which needs the separate `cargo-fuzz`
//! toolchain and is not run as part of `cargo test`). This one runs with no
//! extra tooling and targets the audit's own gap exactly: "a crafted-but-
//! CRC-valid file driving offsets is not covered". A plain random byte flip
//! almost always fails `parse()`'s header/body CRC check immediately, before
//! ever reaching the offset-driven section/record walking the audit was
//! worried about -- so every mutation here recomputes both CRCs afterwards,
//! specifically to get past that gate and exercise the code the corruption
//! fixtures in `testdata/corruption/` don't (those are all CRC-invalid).

use std::{panic, path::PathBuf};
use tau_core::{parse, track_path, verify};

fn testdata(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(name)
}

/// A tiny deterministic xorshift64 PRNG. Not `rand`: this only needs
/// reproducible variety across trials, not cryptographic randomness, and
/// `tau-core`'s dependency list is kept deliberately minimal even for tests
/// (see `docs/DEPENDENCIES.md`).
struct Rng(u64);
impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

fn put_u32(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Restores both CRCs so a header/section mutation still passes `parse()`'s
/// CRC gate and reaches its offset-driven validation and traversal.
fn recompute_crcs(data: &mut [u8]) {
    let body = crc32fast::hash(&data[128..]);
    put_u32(data, 24, body);
    let header = crc32fast::hash(&data[..124]);
    put_u32(data, 124, header);
}

/// Boundary-heavy values for an offset/length/count field: not just noise,
/// but the specific values most likely to trip an off-by-one or an
/// unchecked arithmetic step (zero, max, exactly the buffer length, one past
/// it, and small values near the 128-byte header boundary).
fn interesting_u32(rng: &mut Rng, len: usize) -> u32 {
    match rng.below(6) {
        0 => 0,
        1 => u32::MAX,
        2 => len as u32,
        3 => (len as u32).wrapping_add(1),
        4 => rng.next_u32() % 16,
        _ => rng.next_u32(),
    }
}

fn fuzz_one(seed_name: &str, trials: u64) {
    let original = std::fs::read(testdata(seed_name)).unwrap();
    assert!(
        parse(&original).is_ok(),
        "seed fixture must itself be a valid index: {seed_name}"
    );
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ seed_name.len() as u64);
    for trial in 0..trials {
        let mut data = original.clone();
        // Corrupt one of the 8 section (offset, length) pairs, the root
        // string offset, and all four record counts -- exactly the fields
        // that feed every offset-driven read in `walk`/`string_at`/`track_path`.
        let len = data.len();
        let section = rng.below(8);
        let a = interesting_u32(&mut rng, len);
        let b = interesting_u32(&mut rng, len);
        let c = interesting_u32(&mut rng, len);
        put_u32(&mut data, 48 + section * 8, a);
        put_u32(&mut data, 52 + section * 8, b);
        put_u32(&mut data, 40, c);
        for count_offset in [32, 34, 36, 38] {
            let value = (rng.next_u32() & 0xffff) as u16;
            data[count_offset..count_offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        recompute_crcs(&mut data);
        let outcome = panic::catch_unwind(|| {
            let parsed = parse(&data);
            if let Ok(index) = &parsed {
                for id in [0u16, 1, index.counts.tracks.saturating_sub(1), u16::MAX] {
                    let _ = track_path(index, id);
                }
                let _ = verify(&data, None);
            }
        });
        assert!(
            outcome.is_ok(),
            "parse() (or a function it feeds) panicked on trial {trial} of a \
             CRC-valid-but-corrupted {seed_name}"
        );
    }
}

#[test]
fn parse_does_not_panic_on_crc_valid_corrupted_fixtures() {
    for seed in ["fixture.tdb", "synth-400.tdb", "synth-400-playlists.tdb"] {
        fuzz_one(seed, 3000);
    }
}
