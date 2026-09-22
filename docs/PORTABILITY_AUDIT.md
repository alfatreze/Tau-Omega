# Portability audit — 2026-09-22

Audit of the engine against the stated goal: **`tau-core` must be reusable inside Pocket Sync**
without a rewrite. Findings are evidence-backed against the tree at the time of writing.

Scope note: an earlier candidate target (Feishin, an Electron/GPL streaming client with no local
files) was assessed and **dropped**. See `DECISIONS.md` D-009. Nothing below is written for it.

## Verdict

The engine is in better shape for embedding than its own `ARCHITECTURE.md` asks for. Every
significant gap is at the **boundary**, not in the core. Nothing found requires re-architecting.

## What holds up

| Property | Evidence |
|---|---|
| No runtime lock-in | `tau-core` depends on `crc32fast`, `serde_json`, `sha2`. No tokio, no async, no GUI, no Tauri. |
| No process ownership | No `println!`/`eprintln!`, no `process::Command`, no `process::exit`, no `std::env` in the engine. |
| No hidden state | No `static mut`, no `lazy_static`/`OnceLock`, no `thread_local`, no thread spawning. |
| Front-end independence is real, not claimed | Two front-ends already consume the engine (Tauri app + `tau` CLI). |
| The safety model is already portable | `plan()` returns a plain `SyncPlan` value; `execute()` takes it plus a token. This is a data contract, not a UI flow — better than the docs describe it. |
| The data contract is pinned | Byte-exact conformance against the Python reference, plus a corruption suite asserting firmware E-codes. |
| Index parsing is defensively ordered | `parse()` length-checks the header first, then validates magic/version/size, then **verifies the body CRC before any offset-driven traversal**, mirroring the firmware loader. |

Byte-level `try_into().unwrap()` calls in the tag and cover parsers were individually checked and
are all guarded by prior length checks — infallible by construction, not latent panics.

## Findings

### P0-1 — domain logic has leaked into both front-ends

The rule mapping a media root to its card path prefix (`/Assets/<platform>/common/`) is implemented
**twice**: once in `tau-cli` (`fn prefix`) and again inline in the Tauri adapter. The adapter
additionally decides `library_capable` presentation and the `"Index ready"` / `"No index"` status,
and reads/parses the index itself to count tracks.

Why it matters: this is a domain rule that determines paths written into the index. With two hosts
it is already duplicated; Pocket Sync would make three copies, and any divergence between them
corrupts real cards. It also contradicts decision D-001.

**Fix:** move root-prefix derivation and core/index status into `tau-core`. Front-ends call, never
re-derive.

### P0-2 — the engine speaks English prose as its API

`SyncPlan.warnings`, `SyncReport.warnings` and `Scan.warnings` are `Vec<String>` of human sentences.
`TauError` carries a numeric code, but **`TauError::code()` is called only by the conformance test** —
never by the UI, the CLI, or the Tauri adapter. All 34 boundary sites flatten errors with
`.to_string()`.

Why it matters: the tests prove the E-codes are correct and the product then throws them away. A
host cannot branch on failure, localise, or render its own styling without regexing English.

**Fix:** structured warnings (code + fields) and error propagation that preserves the code. English
rendering belongs in front-ends.

### P0-3 — no progress, no cancellation

There are no progress or cancellation hooks anywhere in the engine. `ARCHITECTURE.md` states jobs
"are cancellable, emit progress events (files, bytes, current path)" — this was never built.

Why it matters: scanning and hashing a multi-thousand-track library is one long blocking call a host
cannot report on or interrupt. Any host UI freezes. This is the hardest blocker for embedding.

**Fix:** a callback/observer parameter on the long-running entry points (scan, plan, execute).
Trait-object or `FnMut` — no runtime dependency, works for every host shape.

### P1-1 — core types are not serialisable

**Done (2026-09-22).** No `serde` derives on any public type. The Tauri adapter therefore
hand-wrote **13 `*View` DTO structs** to get data to its UI, and every future host repeated that
work.

**Fix:** `serde` behind an optional cargo feature. Zero cost when disabled; deletes the DTO layer
when enabled.

Every public type in `tau-core` now derives `Serialize`/`Deserialize` behind the `serde` cargo
feature (off by default; verified the default `cargo build -p tau-core` does not compile `serde` at
all). `ErrorCode` keeps its established plain-`u16` wire shape via a hand-written impl rather than
the derive's PascalCase default, and the fieldless enums (`WarningCode`, `CopyState`,
`DifferenceState`, `IndexStatus`, `Stage`) use `#[serde(rename_all = "snake_case")]` to match the
wire strings hand-written boundaries already used (see `docs/DEPENDENCIES.md`, checked by
`crates/tau-core/tests/serde_feature.rs`). With the feature enabled in `src-tauri`, the DTO layer
shrank from 13 structs to 7: `ApiError`, `WarningView`, `SettingView` and `DifferenceView` are
gone entirely (commands now return `TauError`, `Warning`, `PersistedSetting` and
`MediaDifference`/`MediaComparison` — via `#[serde(flatten)]` for the latter — directly).
`CoreView`, `SyncPlanView`, `PlaylistView`, `MediaScanView`, `LibrarySummaryView`,
`ComparisonView` and `DuplicateView` remain: each does real presentation or aggregation work (an
English status sentence, or counts that are not fields stored on the engine type), not routing
around a missing `Serialize` impl.

### P1-2 — a public function panics on caller-supplied input

**Done (2026-09-22).** `build_index` is public and consumes `Entry` values whose fields are all
public. It then does `entries[i].tags["_tno"].parse::<u16>().unwrap()` — a map index plus an
unwrap on an internal invariant that only the engine's own scanner establishes.

Why it matters: a host feeding its own library in — precisely the integration being planned — panics
instead of getting an error.

**Fix:** validate and return `Err`; no public entry point may panic on caller data.

On inspection `_tno`/`_title` turned out to always be set by `build_index` itself just before
each read (so the described panic could not actually be triggered today), but the indexing style
(`tags["_tno"]` + `.unwrap()`) was still one refactor away from a real one, so it was replaced with
`tno_of`/`title_of` helpers that fall back to a safe default instead of indexing-and-unwrapping —
defensive by construction, not by an invariant a future change could quietly break. A second,
genuinely live panic was found and fixed in the same pass: `sync::plan`/`plan_with_features`'s
`sources: &[PathBuf]` is public caller input, and a source path with no derivable file name (`/`,
`.`, a bare drive letter) reached a bare `.file_name().unwrap()` in two places (the direct-file
case and `collect_source`'s use of the top-level root for `include_root` naming) — both now go
through a `required_file_name` helper returning `ErrorCode::InvalidPathReference` instead of
panicking. New tests: `build_index_does_not_panic_on_a_hand_built_entry_with_no_tags`
(`tests/conformance.rs`) and `required_file_name_does_not_panic_on_a_nameless_path`
(`sync.rs`). All byte-conformance tests still pass unchanged, confirming this was a pure
defensive refactor with no behaviour change on valid input.

### P1-3 — telescoping constructors

**Done (2026-09-22).** `plan` → `plan_with_options` → `plan_with_features` → `plan_with_layout`,
four deep at T2/T3, before any API is public. Each new capability adds another.

**Fix:** one entry point taking an options struct with `Default`, before the surface is frozen.

`plan_with_options` and `plan_with_features` are gone; the single public `plan(sources, common,
root_prefix, options: PlanOptions, progress)` takes a `#[derive(Default)]` `PlanOptions { mirror,
embed_covers }` instead. `plan_core_copy` stays a separate named function (it is a distinct
operation — whole-library layout, no wrapping source folder — not "plan with more options"), and
now calls the same private `plan_with_layout` impl with `PlanOptions::default()`. Every call site
(tau-cli, the Tauri adapter, this crate's own tests) was updated; behaviour is unchanged
(`--mirror`/`--embed-cover` verified end to end against a scratch copy of the real
`../tau-alpha/dist` card, producing distinct plan ids as before).

### P2-1 — plan token is thin and leaks a phase label

**Done (2026-09-22).** The plan id was a SHA-256 truncated to **32 bits**, formatted `T2-xxxxxxxx`.
The hash covers the right inputs (sources, hashes, cover hashes, deletions, flags, prefix), but
truncation is thin for a token that may be persisted or handed across a process boundary, and `T2-`
embedded an internal roadmap phase label into a durable identifier.

The id is now the full SHA-256 hex digest (64 hex characters), unprefixed — `format!("{:x}", ...)`
on the whole `finalize()` output instead of truncating to the first 4 bytes. Confirmation everywhere
(`execute*`, the CLI, the Tauri adapter) is a plain string comparison, so nothing needed updating
beyond the construction site itself and this crate's own tests. New test:
`plan_id_is_a_full_sha256_hex_digest_with_no_phase_prefix` (`sync.rs`).

### P2-2 — no fuzz target for the index parser

**Done (2026-09-22).** `parse()` is well-ordered and CRC-gated, and the corruption suite covers
realistic damage. A crafted-but-CRC-valid file driving offsets was not covered, and the
`u16_at`/`u32_at` helpers panic rather than return.

Manual re-derivation of every offset used by `walk`/`string_at`/`track_path` showed each one is
already bounds-checked by `parse()`'s own section-table validation before use (every section's
`(offset, length)` is checked against the buffer length and the 16-byte alignment/header-start
rule before any record inside it is read), so a panic was not expected — but that needed proof,
not just re-reading the code again. Two additions:
- `crates/tau-core/tests/fuzz_lite.rs`: a dependency-free, deterministic (xorshift64, no `rand`)
  test that takes real fixtures, corrupts a section's offset/length, the root string offset and
  all four record counts to boundary-heavy values (`0`, `u32::MAX`, the buffer length, one past
  it), **recomputes both CRCs** so the mutation reaches `parse()`'s offset-driven logic instead of
  being rejected at the CRC gate (the exact gap the audit named), and asserts `parse`/`track_path`/
  `verify` never panic. Runs as part of ordinary `cargo test`; found no panics across 9,000 trials
  (3 seed fixtures × 3,000 mutations each).
- `fuzz/`: a standalone `cargo-fuzz` scaffold (`fuzz_targets/parse_index.rs`) for real
  coverage-guided fuzzing with `cargo +nightly fuzz run parse_index`, for a maintainer with that
  tooling available. Its own `[workspace]` keeps it fully isolated from the main build (verified:
  `cargo metadata` from the repo root still lists only the three real crates). Not run in this
  session — no `cargo-fuzz` install or nightly toolchain available here; see `docs/DEPENDENCIES.md`.

### Addendum 2026-09-22 — fixture fidelity

A follow-up check against the firmware repo (`FIRMWARE_SYNC.md`) found and fixed a live bug that this
audit's method would not have caught: library capability detection never matched a real card, because
the test fixture invented a `data.json` shape instead of copying a real one. The engine's *portability*
was fine; its *fidelity to the platform* was not.

Worth recording next to the findings above, because the two failure modes look alike and are not:
the index path is byte-exact against a Python oracle and was flawless, while the card path was
verified against a fiction. **Any fixture standing in for something the firmware or APF produces must
be derived from a real artefact.**

## Documentation drift

| Doc claim | Reality |
|---|---|
| `ARCHITECTURE.md` module tree (`card/ cores/ scan/ tags/ names/ index/ art/ …`) | Flat files; `lib.rs` is a 1,276-line monolith holding card inspection, ASCII rules, scanning, tag parsing, index build/parse/verify. |
| `ARCHITECTURE.md` dependency list (tokio, rusqlite, lofty, image, tracing, walkdir, …) | Three dependencies. The lean result is **better** than the plan — the doc is what is wrong. |
| "Jobs are cancellable, emit progress events" | Not implemented (P0-3). |
| Five "Extension points" (core registry, versioned index writers, media processors, exporters, card adapters) | None exist as interfaces. These are the portability seams, so they are either work or they are fiction. |

`ARCHITECTURE.md` has been corrected for these. The drift mattered because the plan explicitly drives
a code-generating assistant from these documents.

## Remediation order

Do P0 before adding features; it is cheapest now and all three are prerequisites for any Pocket Sync
work.

1. **P0-1** — **Done (2026-09-22, commit `4eaa432`).** `tau_core::root_prefix` is the single
   implementation of the prefix rule; `Core::index_status` is computed by `inspect_card` itself.
   No path-prefix or index-status logic remains outside the engine (checked by grep and by
   running against the real `../tau-alpha/dist` v0.4.0 card).
2. **P0-2** — **Done (2026-09-22, commit `3046b28`).** `TauError { code: ErrorCode, message }`
   and `Warning { code: WarningCode, message }` replace every `Vec<String>`/English-only error.
   The CLI's process exit code is the error's numeric code (not only asserted in a test); Tauri
   returns a serialisable `ApiError`; the UI's `errorMessage()` renders `.message`. Verified: a
   bad destination path now exits `30` (`InvalidMediaRoot`).
3. **P0-3** — **Done (2026-09-22, commit `0c7fe6c`).** `ProgressObserver` (blanket-implemented
   for `FnMut(Progress) -> bool`, no runtime dependency) threads through
   `scan_dir_with_progress` and the whole `plan`/`execute` family; returning `false` cancels with
   `ErrorCode::Cancelled`. Tauri wires a `job_id` + `"tau://progress"` window event + a
   `cancel_job` command; the sync screen shows live progress and a Cancel button.
4. **P1-1** — **Done (2026-09-22).** Optional `serde` feature; the DTO layer shrank from 13 to 7
   structs (the rest do real presentation/aggregation, not serialisation workaround).
5. **P1-2** — **Done (2026-09-22).** `build_index`'s tag lookups and `sync::plan`'s source-path
   handling no longer index-and-unwrap; both fall back or return `Err` instead of panicking.
6. **P1-3** — **Done (2026-09-22).** `plan_with_options`/`plan_with_features` collapsed into one
   `plan(..., PlanOptions, ...)` entry point.
7. **P2** — **Done (2026-09-22).** Plan id is the full SHA-256 hex digest, unprefixed; a
   dependency-free fuzz-lite regression test runs in `cargo test`, and a `cargo-fuzz` scaffold
   exists under `fuzz/` for coverage-guided fuzzing when that tooling is available.

## Explicitly not doing

- No plugin ABI, no C ABI, no WASM. Pocket Sync is Rust; a crate dependency is the boundary.
  Revisit only if a non-Rust host is ever adopted.
- No source abstraction for remote/streaming libraries. It was designed for the dropped Feishin
  target; with Pocket Sync the source is a local filesystem on both sides.
