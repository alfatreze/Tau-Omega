//! Coverage-guided fuzzing for `tau_core::parse` (P2-2): raw, unstructured
//! bytes, exactly what an untrusted `tau-library.tdb` on a card could be.
//! libFuzzer will find its own way past the header/body CRC gate given
//! enough time and the corpus below as a seed; `../../crates/tau-core/tests/
//! fuzz_lite.rs` covers the same "CRC-valid but corrupted" class
//! deterministically and needs no extra tooling to run today.
//!
//! Run with `cargo +nightly fuzz run parse_index` from `fuzz/`. Seed the
//! corpus first from the real fixtures so it starts past the CRC gate:
//! `mkdir -p corpus/parse_index && cp ../testdata/*.tdb ../testdata/corruption/*.tdb corpus/parse_index/`

#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // parse() must never panic on any byte sequence; Ok or Err are both fine.
    let _ = tau_core::parse(data);
});
