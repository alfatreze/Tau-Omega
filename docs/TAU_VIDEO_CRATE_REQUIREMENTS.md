# Requirements: the `tau-video` Rust crate (TNV3 / TVC / TAC encoder, decoder, container tools)

Version 0.1, 2026-10-30. Written by Tau Omega as the consumer; addressed to whoever builds the crate (the Tau V encoder work).
Context and reasoning: `docs/VIDEO_SYNC_PLAN.md`, decision D-020. The formats themselves are **not** defined here: the normative specs stay
in Tau V (`docs/TNV3_CONTAINER_V0.md`, `TVC_BITSTREAM_V0.md`, `TAC_BITSTREAM_V0.md`, `TST_SUBTITLES_V0.md`). Where this document and a Tau V spec
disagree, the Tau V spec wins and this document is wrong. "MUST", "SHOULD", "MAY" are used in the usual sense.

## 1. Purpose and consumers

One pure-Rust crate that turns decoded source frames and audio into a valid `.tnv3`, checks and inspects a `.tnv3`, and decodes a `.tnv3` back to
pictures and PCM for **a simple player** (preview in Omega's Video section, and a reference player for tests). Consumers:

1. **Tau Omega** (Tauri desktop app, Svelte UI, CLI): encode jobs that run for hours with progress, cancel and resume; verify files before and after
   a card write; play a film or a still frame in the app.
2. **Tau V** itself: the crate is also the test oracle and the host-side tool (replaces the Python/C reference chain for production use, while the C
   references stay as the cross-check).
3. Other developers: a small, documented library API with a thin CLI.

Non-goals: decoding source formats (H.264, etc.; the caller supplies raw frames, see 4.1), running on the Pocket, GPU use, writing to SD cards,
anything UI.

## 2. Constraints from Tau Omega (hard)

| # | Requirement |
|---|---|
| C1 | **Portable and UI-free.** No Tauri, no Svelte, no platform APIs, no `std::process` use for core functionality. Builds on macOS, Windows and Linux with a stable Rust toolchain, no build scripts that need a C compiler or network. |
| C2 | **No `unsafe`** in the crate (`#![forbid(unsafe_code)]`). If a hot loop truly needs it, it is isolated in an opt-in feature that Omega does not enable, with the safe path kept as the default and tested equal. |
| C3 | **Few dependencies.** Default features: none beyond `std`, plus at most one small, well-known crate each for CRC32 and error derive if you do not want to hand-write them. Anything else (threads pool, SIMD, serde) is an optional Cargo feature, off by default. A new dependency needs the owner's yes before it lands (CLAUDE.md). |
| C4 | **No ffmpeg, no network, no filesystem assumptions in the library.** I/O goes through traits (4.4). The CLI binary may touch files. |
| C5 | **Never panics on any input**, however malformed (decoder, checker, reader). Every size, count and offset from a file is checked before use; allocation is bounded by documented limits (section 7). Fuzzable. |
| C6 | **Deterministic**: same input and settings give byte-identical output on every platform and every thread count. No floating point in anything that affects the bitstream or the decoder output, unless the spec itself defines it (state where it does). |
| C7 | **Licence**: MIT OR Apache-2.0 (Omega's licence) or, if Tau V insists on MIT only, MIT; the owner confirms. No code copied from FFmpeg or any LGPL/GPL source (Tau V's older MPEG-4 tables are LGPL and must not enter this crate). |
| C8 | **MSRV** stated in `Cargo.toml` and checked in CI; Omega's current toolchain is the baseline. |

## 3. Crate shape

```
tau-video/
  src/lib.rs            // pub API below, #![forbid(unsafe_code)]
  src/container/        // TNV3: header, chunk, index, subtitle area, reader, writer, checker
  src/tvc/              // TVC: bitstream, encoder, decoder, levels
  src/tac/              // TAC: encoder, decoder (MDCT)
  src/tst/              // subtitles: parse area, optional render
  src/error.rs
  tests/                // vectors, round trips, mutants, malformed corpus
  testdata/             // pinned Tau V vectors (path + SHA-256 + provenance README), never edited
  tau-video-cli/ (or src/bin/tau-video.rs, feature "cli")
```

Optional Cargo features: `serde` (derive on report and settings types), `parallel` (use `std::thread` scoped threads, no extra crate; a `rayon` feature
only with the owner's yes), `cli`. Default = library only.

## 4. Functional requirements

### 4.1 Encoder

**Input (the caller decodes the source):**
- Video as a stream of **planar YUV 4:2:0, 8 bit** frames (`Frame { width, height, y, u, v }`, with stride), frame rate as a rational `fps_num/fps_den`,
  colour assumed BT.709 limited range unless the spec says otherwise (state what the container/TVC expects and document the RGB565 conversion).
- Audio as interleaved or planar **48 kHz 16-bit (or f32) PCM**, 1 or 2 channels (resampling and downmix are the caller's job; the crate MAY offer them behind a feature).
- Optional subtitle tracks (already parsed events with style) if TST import is in scope for the first release; otherwise reserved API.

**Settings (all with documented defaults, serialisable):** quantiser (`qp`, default 30), level (`auto` or 1..3), target resolution and padding/crop rules, GOP
length (default 96), B-picture count, segment length in frames (default 480), audio bitrate (96..128 kbit/s), effort/preset (at least `fast` and `best`),
CRC on chunks (default on), `Threads(n)`.

**Behaviour:**
- E1. Produce a file that satisfies the container spec section by section: header CRC, display order, presentation sample formula, **audio interleaving bounds (4.1 of the spec)**, chunk byte-rate window (4.2), index, level limits. The crate's own checker MUST accept everything the encoder writes (tested in CI on every vector and a fuzzed settings matrix).
- E2. **Level selection**: smallest level that fits, error with a clear reason otherwise (resolution or macroblock rate too high). Enforce per-picture and per-second byte limits during encoding, re-encoding with the previous segment's tail as `encode_film.py` does; report how many pictures were re-encoded.
- E3. **Segmented and resumable.** The encoder is exposed as steps the caller can persist: `plan(source_info, settings) -> Plan` (segments, level, estimated size), `encode_segment(plan, index, frames, carry_in) -> SegmentOutput` (opaque but **serialisable bytes** plus the carry needed by the next segment), and `finish(plan, segments, audio) -> .tnv3`. The caller (Omega) stores segment outputs in its work folder; a job killed at any point resumes from the last finished segment and gives the **same final file** as an uninterrupted run (C6).
- E4. **Cancellation and progress**: a `Progress` callback (frames done, segment, bytes so far, elapsed) and a cancel flag checked at least every picture. Cancelling leaves no partial output files and no leaked threads.
- E5. **Streaming memory**: bounded by one segment of frames plus the window (the C `tvc_enc` holds the whole input; that is not acceptable for a 2-hour film). State the peak memory formula in the docs.
- E6. **Estimate**: `estimate(source_info, settings) -> { bytes_low, bytes_high, encode_seconds_per_source_second }` from a short measured sample (e.g. encode N frames) for Omega's plan screen.
- E7. **Speed**: state the throughput measured on the reference machine in `PERFORMANCE.md`; target at least the C encoder's rate per thread, and linear-ish scaling with `Threads(n)` across segments.
- E8. Audio (TAC) encoder: same determinism rule; bitrate setting; exact `sample_count` and `tac_frames` as the header requires.
- E9. Encoder decisions are not normative. Acceptance is validity (checker + independent decoder) and a quality floor measured by PSNR on the standard vectors, **not** byte equality with `tvc_enc`, except for the fixed bitstream vectors that Tau V pins.

### 4.2 Container reader, checker, info (the first deliverable)

- R1. `Tnv3Reader::open(impl Read + Seek)`: parses the header and index only (no full read), exposes `Info` (size, fps, level, audio, frame count, duration, subtitle tracks, flags, version, header CRC ok).
- R2. `check(&mut reader, CheckOptions) -> Report`: implements **every rule in the container spec** (layout, CRCs, display order, audio locking, byte-rate window, index consistency, subtitle area, reserved bits zero, version). One finding per violated rule with a stable code (`E-TNV3-xxx`) and byte offset. `Ok` only if the file is fully valid. A `quick` mode (header, index, sampled chunks) for large files on slow cards.
- R3. Must reject the 15 must-reject vectors and accept the 9 valid ones, and the independent Python demuxer's verdict is the second oracle.
- R4. Refuse unknown versions and unknown flag bits explicitly; never guess.
- R5. `Tnv3Writer` (the muxer) available on its own for tests and tools; same rules as E1.

### 4.3 Decoder (for the simple player)

- D1. `Decoder::new(reader)`: decodes TVC pictures in decoding order and **outputs frames in display order** with the presentation sample `S(k)` from the spec (exact integer arithmetic) so the caller can schedule.
- D2. Output formats: planar YUV 4:2:0 (reference) and **RGBA8 and RGB565** (RGB565 must equal what the Pocket's display path will produce; document the exact conversion and test it).
- D3. **Audio**: TAC decode to interleaved i16 PCM at 48 kHz with `sample_count` exact, locked to video by the spec's rule (the audio output position is the master clock).
- D4. **Seeking**: `seek(time or frame)` lands on the nearest preceding key picture from the index and decodes forward to the requested picture; cost is bounded (GOP length). `seek` never reads outside the file.
- D5. **Player helper**: `Player` wraps decoder + clock: `advance(audio_position_samples) -> FrameReady`, drop/repeat rules documented (late pictures are dropped, never the audio). Pure logic, no audio device, no windowing; Omega feeds it from the UI thread's audio clock.
- D6. **Bit-exactness**: decoded pictures MUST equal Tau V's `tvc_dec` and the independent Python decoder on every vector; TAC output MUST equal the reference `tac` decoder. The hardware decoder is bit-exact to the reference, so this decoder is also the model for what a Pocket will show.
- D7. **Speed**: software real time for L2 (800x464) at 24 fps on a current laptop single thread is the target; L3 (800x720) real time with `Threads(2..4)`. State measured numbers. A thumbnail path (`decode_key_pictures_only`) for poster frames and scrubbing.
- D8. Level violations and corrupt pictures return an error for that picture with the stream position; `Decoder` can continue at the next key picture (`resync()`).
- D9. Subtitles: `SubtitleTrack::events_at(sample) -> Vec<Rendered>` returning the bitmaps and placement from the area (size variants, palette, forced/SDH flags) per spec section 6; blending is the caller's job. MAY be deferred to the second release.

### 4.4 I/O and API conventions

- Library I/O uses `Read`/`Write`/`Seek` and in-memory slices (`&[u8]`), so Omega can use files, a card path or memory. No global state, no thread-locals.
- All public types are `Send`; `Decoder` and `Encoder` are not required to be `Sync`.
- Errors: one `Error` enum, `std::error::Error`, `#[non_exhaustive]`, no `unwrap`/`expect`/indexing panics reachable from input. Messages are plain English sentences Omega can show, plus the stable code.
- Public items documented; `cargo doc` has no warnings; examples in `examples/` (probe a file, decode frame 100 to PNG-ready RGBA, encode 2 s of generated frames).
- Semver from 0.1; every format version the crate understands is a public constant (`TNV3_VERSION`, `TVC_DRAFT`, `TAC_DRAFT`, `TST_DRAFT`).

### 4.5 CLI (`tau-video`, feature `cli`)

`tau-video info FILE [--json]`, `check FILE [--json] [--quick]`, `encode SRC.y4m [--wav AUDIO.wav] OUT.tnv3 [settings]` (reads Y4M and WAV on stdin or file, so any ffmpeg
can feed it), `decode FILE --frames A..B --format rgba|yuv --out DIR`, `extract-audio FILE OUT.wav`, `seek-test`. `--json` output is stable and documented. Exit codes: 0 ok, 1 findings, 2 usage, 3 I/O.

## 5. Interface Omega will call (sketch, not final)

```rust
pub struct SourceInfo { pub width: u32, pub height: u32, pub fps: Rational, pub frames: Option<u64>, pub audio: Option<AudioInfo> }
pub struct Settings { /* section 4.1, Default, Serialize/Deserialize */ }
pub fn plan(src: &SourceInfo, s: &Settings) -> Result<Plan, Error>;
pub fn estimate(src: &SourceInfo, s: &Settings, sample: &mut dyn FrameSource) -> Result<Estimate, Error>;
pub struct Encoder { /* holds Plan */ }
impl Encoder {
    pub fn encode_segment(&self, index: u32, frames: &mut dyn FrameSource, carry: Option<&Carry>, ctl: &Control) -> Result<Segment, Error>;
    pub fn encode_audio(&self, pcm: &mut dyn PcmSource, ctl: &Control) -> Result<AudioTrack, Error>;
    pub fn finish(&self, segments: &[Segment], audio: Option<&AudioTrack>, out: &mut dyn Write) -> Result<Summary, Error>;
}
pub struct Control<'a> { pub cancel: &'a AtomicBool, pub progress: &'a (dyn Fn(Progress) + Sync) }
pub fn check(r: &mut (impl Read + Seek), o: &CheckOptions) -> Result<Report, Error>;
pub struct Decoder<R> { /* open, info, seek, next_frame, next_audio */ }
pub struct Player { /* advance(audio_pos) */ }
```
Segments and `Carry` are `to_bytes`/`from_bytes` with a version byte so a work folder survives an Omega update; stale ones are rejected, not misread.

## 6. Test and acceptance requirements

| # | Requirement |
|---|---|
| T1 | **Pinned vectors**: Tau V's TNV3, TVC and TAC vectors copied once into `testdata/` with `README` (source path, commit, SHA-256 of every file); a test fails if a hash changes. Valid vectors decode bit-exact; negative vectors are rejected with the expected code. |
| T2 | **Cross-check against the C and Python references**: a test (ignored by default, runs when Tau V's built tools are present) encodes generated and real clips and has `tvc_dec`, `tac` and `tnv3 check` accept the output and decode it identically. |
| T3 | **Mutation checks**: for the checker, each rule has at least one mutated file that the checker must catch (Tau V already has 10 checker mutants; port them and add one per new rule). |
| T4 | **Round trip property tests**: random small frames and PCM, random settings within bounds: encode, check, decode, compare PSNR floor and exact audio length. |
| T5 | **Malformed corpus and fuzzing**: truncated at every byte of a small file, every header field extreme, random byte flips; no panics, no hangs (time limit), bounded memory. A `cargo fuzz` target for `check` and `Decoder` is provided (not run in Omega CI). |
| T6 | **Resume test**: kill an encode after segment N, resume, compare the final file byte for byte with an uninterrupted run. Threads 1, 2, 8 give identical files. |
| T7 | **Real clip**: a short (10-30 s) CC-licensed clip with audio at L2, committed as a recipe (script + hash), not as a file, that runs locally. |
| T8 | **Platforms**: CI on macOS, Linux and Windows (path separators, line endings, `Read`/`Seek` on files). |
| T9 | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo doc` clean; `cargo deny`/`cargo tree` shows the dependency list in the PR. |

## 7. Limits to document and enforce

Max picture size from the level table (L3 800x720); header dimensions up to 65535 are parsed but pictures above the level limit are refused before any allocation; max chunk length, max
`frame_count` (state the number; Omega expects films up to 4 hours at 30 fps), max subtitle area size, max audio frames per chunk. Allocation never exceeds a fixed multiple of the largest legal picture plus the
reference frames for the level (state the number per level). A file that declares more than the limits is `Error::LimitExceeded`, never an OOM.

## 8. Deliverables, in priority order

1. **Container reader, checker, info + CLI `info`/`check`** with T1, T3, T5 for those (this alone lets Omega verify files before and after a card write).
2. **Decoder (TVC + TAC) + `Player`**, T1, T2, D6, D7 (simple player and the poster/thumbnail path).
3. **Encoder segment API, resume, progress/cancel, estimate** (E1..E9, T4, T6).
4. Subtitles (TST) read and render; encoder import.
5. Performance work and the `parallel` feature.

Each step is its own commit with tests green; the crate's `CHANGELOG.md` lists the spec drafts it targets. Omega will pin a tagged release by hash (D-019 style) and never edit the pinned copy.

## 9. What Omega still needs answered by Tau V (does not block the crate)

Card folder and naming convention for films, any poster/index file the player reads, freeze order of the drafts, and whether TNV3 `.tnv3` is the final extension (see `docs/VIDEO_SYNC_PLAN.md` section 6).
