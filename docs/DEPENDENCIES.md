# Dependencies

`tau-core` is deliberately kept to a handful of small, permissive crates. It has to be embeddable in
another application (see `DECISIONS.md` D-009), and **every dependency here becomes a constraint on
every future host**. Tag parsing, cover handling and the index codec are hand-written rather than
pulled in, which is also what keeps the index byte-exact against the Python reference.

Adding one needs a reason that survives that test. `ARCHITECTURE.md` originally surveyed ~20 crates
(tokio, rusqlite, lofty, image, tracing, walkdir …); none were adopted and the lean result is the
better outcome — do not "restore" that list to match the survey.

## `tau-core` (the portable engine)

| Dependency | Purpose | Licence / review |
|---|---|---|
| `crc32fast` | Index CRC32 header and body fields | MIT OR Apache-2.0; small, widely used. |
| `sha2` | SHA-256 for sync verification, duplicate grouping, plan tokens and backup checks | MIT OR Apache-2.0; RustCrypto, widely used. |
| `serde_json` | Read-only `core.json` / `data.json` / `interact_persist.json` parsing | MIT OR Apache-2.0; small, widely used. |
| `fs4` (feature `sync` only, default features off) | Cross-platform free/total disk space for `storage::check_capacity` (`STATUS_HANDOFF.md` item 5) | MIT OR Apache-2.0. The one exception to "no dependency pulls in a subprocess or a heavy transitive graph": std has no cross-platform statvfs/GetDiskFreeSpaceEx equivalent, and getting free space any other way means either hand-rolling per-OS FFI (unix/windows raw syscalls, the same surface `fs4` already wraps) or shelling out to `df`/`dir` (which this crate's own audited "no `process::Command`" property rules out). Pulls in `rustix`+`bitflags`+`errno`+`linux-raw-sys` on Unix and `windows-sys`+`windows-link` on Windows — 7 packages, a real jump from this crate's usual "3 dependencies" — accepted deliberately (owner decision, 2026-09-23) rather than silently. `--no-default-features --features sync` keeps out its `tokio`/`async-std`/`smol` async backends and `fs-err` wrappers entirely. |
| `zip` (feature `deflate-flate2-zlib-rs` only, default features off) | Reads real Analogue core release package zips for `package::{inspect, plan_install, execute_install}` (`STATUS_HANDOFF.md` item 6) | MIT OR Apache-2.0. Real release zips (verified against `tau-alpha/release/alfatreze.TAU_0.4.0_2026-09-22.zip`, copied into `testdata/packages/`) are deflate-compressed, so this needs actual decompression, not just zip-structure parsing. Pulls in 7 packages (`flate2`, `zlib-rs`, `indexmap`, `hashbrown`, `equivalent`, `typed-path`, plus `zip` itself; reuses this crate's existing `crc32fast`) -- accepted deliberately (owner decision, 2026-09-23), same class of trade-off as `fs4`. The plain `deflate` feature was rejected: it also pulls in `zopfli` (a compression-only encoder, `bumpalo`+`log`+`simd-adler32`) that this read-only use never needs, since Tau Omega never creates or re-zips a package. No encryption, legacy-zip, bzip2, lzma, zstd, or timestamp features enabled. |
| `serde` (optional, feature `serde`) | `Serialize`/`Deserialize` on every public data type (`PORTABILITY_AUDIT.md` P1-1, done 2026-09-22) | MIT OR Apache-2.0; already an implied dependency of `serde_json`, which every consumer of this crate already builds. Not in the default feature set: verified the default `cargo build -p tau-core` does not compile `serde`/`serde_derive`/`syn` at all, vs. `cargo build -p tau-core --features serde`, which does. |
| `base64` | Decoding the `TAUD1:` QR/text report payload (`taud::from_text`) | MIT OR Apache-2.0. Zero extra transitive dependencies. |
| `png` | Decoding a screenshot PNG to raw pixels for QR detection (`taud::read_qr_text`); also *encoding* a decoded `Cores/<id>/icon.bin` to PNG (`icon::decode_icon_bin`) — the same dependency, no new one added for the second use | MIT OR Apache-2.0. Pulls in ~6 packages (`bitflags`, `fdeflate`, `flate2`, `miniz_oxide`, `simd-adler32`, `adler2`; reuses this crate's existing `crc32fast`) — accepted deliberately (owner decision, 2026-09-23, confirmed via `AskUserQuestion` after measuring the real transitive cost in a scratch crate), same class of trade-off as `fs4`/`zip`. |
| `rqrr` (default features off) | Locating and reading a QR code's grid from greyscale pixels (`taud::read_qr_text`) | MIT. `default-features = false` drops its optional `image`-crate integration entirely (confirmed: `cargo add rqrr --dry-run --no-default-features` shows no `image`/`img` features), so this crate works directly off the raw greyscale buffer `png` already decoded rather than pulling in a second, heavier image-handling crate on top. Pulls in ~10 packages, including a `syn`/`quote`/`proc-macro2` chain from `g2p` (a finite-field arithmetic proc-macro) — common in the Rust ecosystem already, but real compile-time cost, accepted the same way as `png` above. |
| `zune-jpeg` | Decoding a JPEG cover for `image::encode_cover_pal256` (the source image, not the `TIM1` output) | MIT OR Apache-2.0. Measured in a scratch crate (`cargo add zune-jpeg --dry-run` then `cargo tree`) before adding: exactly one transitive dependency, `zune-core`. Pure Rust, no C/FFI — the deliberate alternative to the much heavier general-purpose `image` crate this project's own `ARCHITECTURE.md` survey already rejected. Reused `png` (already a dependency, via `taud`/`icon`) for the PNG-cover decode path rather than adding a second PNG decoder. |
| `resize` (default features off), `rgb` (default features off) | Proportional Lanczos3 resize of a decoded cover to the `TIM1` container's target size (`image::encode_pal256_bytes`), matching `tau_image.py`'s own Lanczos scaling (D-I02) | MIT (both). `resize`'s default features pull in `rayon` (a thread pool) for parallel resizing of a single small (≤ a few hundred px) cover, which is unnecessary work for a one-shot encode; `default-features = false` drops it, leaving only `rgb` (a plain pixel-type crate, zero further transitive dependencies — confirmed via `cargo tree`). `rgb` is added directly (not just relied on transitively) because naming its `RGB8` type in this crate's own code requires it as a direct dependency, not merely present in the lockfile. |

`ErrorCode` and the fieldless enums (`WarningCode`, `CopyState`, `DifferenceState`, `IndexStatus`,
`Stage`) do not use the derive's default wire shape: `ErrorCode` has a hand-written
`Serialize`/`Deserialize` pair so it stays the plain `u16` every hand-written boundary (CLI exit
codes, the Tauri adapter's former `ApiError`) already used, and the fieldless enums use
`#[serde(rename_all = "snake_case")]` so they match `WarningCode::as_str()` and the hand-written wire
strings (`"only_left"`, `"new"`, …) that predate this feature, rather than the derive's own PascalCase
default. Checked by `crates/tau-core/tests/serde_feature.rs` (only compiled with `--features serde`).

## `fuzz/` (not part of the portable engine, not built by default)

A standalone `cargo-fuzz` workspace (its own `[workspace]`, so the main build's dependency graph is
untouched — verified `cargo metadata` from the repo root lists only `tau-core`/`tau-cli`/
`tau-testkit`) targeting `tau_core::parse` (`PORTABILITY_AUDIT.md` P2-2). `libfuzzer-sys` is its
only dependency, scoped entirely to `fuzz/`. Needs the separate `cargo-fuzz` tool and, at the time
of writing, a nightly toolchain for coverage instrumentation — neither is assumed to be available
in every environment this repository is built in, so it was written and reviewed but not run here.
The always-runnable complement, `crates/tau-core/tests/fuzz_lite.rs`, is part of the normal
dependency-free `cargo test` run and found no panics across 9,000 CRC-valid-but-corrupted trials
against three real fixtures.

## Front-ends (not part of the portable engine)

| Dependency | Purpose | Licence / review |
|---|---|---|
| `tauri` 2, `tauri-plugin-dialog` | Desktop shell and native file pickers | MIT OR Apache-2.0. Ships in the macOS bundle today. |
| `base64` (src-tauri only) | Encoding a screenshot's bytes as a `data:` URL for `read_image_data_url`, so the Settings screenshot gallery can show the real image inline | MIT OR Apache-2.0. Already resolved in the workspace lockfile via `tau-core`'s own `base64` dependency (`taud`), so this adds zero new transitive crates -- a presentation-only pass-through kept in the Tauri adapter rather than `tau-core`, since it has no domain logic (which file is valid is already `list_screenshots`'s decision). Deliberately not using Tauri's asset protocol (`convertFileSrc`), which would need broadening the webview's filesystem access via a capability/scope change for arbitrary card paths; this keeps that surface at zero. |
| Svelte, Vite, TypeScript, `@tauri-apps/api` | UI build and Tauri bridge | MIT. |

`tau-cli` has no dependency beyond `tau-core`, which is the check that the engine really is
front-end independent — if the CLI ever needs something the engine can't give it, that is a signal.

## Licence boundary

Tau Omega is **MIT OR Apache-2.0**. Integration targets are copyleft (Pocket Sync is AGPL-3.0), so
code may flow from here to there but **never the reverse** — see `DECISIONS.md` D-010.
