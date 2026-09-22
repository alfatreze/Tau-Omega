# Tau Omega — status handoff

Updated 2026-09-22. All implementation work is contained in `Tau Omega/`.

## Read these first

1. This file — state, known issues, what to do next.
2. `DECISIONS.md` — D-001..D-012, including the integration target and the licence boundary.
3. `PORTABILITY_AUDIT.md` — the ranked P0/P1 work that comes before new features.
4. `FIRMWARE_SYNC.md` — what we assume about tau-alpha, last verified 2026-09-22 against v0.4.0.

This folder is a Git repository (initialised 2026-09-22, branch `main`). It has **no remote**, so the
history is local-only — worth fixing before it matters. The firmware project is the sibling
`../tau-alpha`, which is read-only from here (D-010).

## Current deliverable

The macOS app bundle is produced at:

`src-tauri/target/release/bundle/macos/Tau Omega.app`

The bundle uses the `assets/appicon.png` icon and embedded Space Grotesk font.

## Implemented

### Portable Rust engine

- Byte-conformant Tau index reader, writer, verifier, corruption checks, and golden fixtures.
- Read-only card/core inspection and library capability detection from `data.json`.
- Plan-first sync, verification, destination-index-last publishing, host-side journals, and cover embedding for destination MP3/FLAC copies.
- Core comparison, safe copy, and guarded move with external backup plus second confirmation.
- Persisted-settings reader and history-word decoding foundation.
- Playlist scanning/import rules already used by index building; ordinary relative-path `.m3u` export.
- SHA-256 read-only duplicate grouping.
- Read-only host-side journal reader.

### Desktop UI

- Cards, Sync, Compare Cores, Playlists, Problems, Recent Jobs, and Settings screens.
- Native picker support for staging/media folders, reports, journals, settings files, backup folders, and playlist export paths.
- Playlist scanning, warning display, selected-playlist `.m3u` export.
- Read-only duplicate Problems surface with empty, loading, results, and error states.
- Session job history plus manual host-journal loading.
- Settings display with friendly known Tau labels and decoded library-history words.

### Architecture improvements

- `tau-core::diag` owns persisted-settings parsing; Tauri only adapts it.
- `tau-core::playlist` owns `.m3u` export.
- `tau-core::duplicates` owns duplicate detection.
- Shared UI contracts are in `ui/src/lib/types.ts`.
- Tauri invocation is isolated in `ui/src/lib/backend.ts`.
- Typed feature commands are in `ui/src/lib/tau-api.ts`.
- Settings, Jobs, Playlists, Problems, and Library views are extracted Svelte components.

### Portability boundary (P0-1..P0-3, 2026-09-22 — see `PORTABILITY_AUDIT.md`)

All three P0 items are done, each its own commit:

- **P0-1** (`root_prefix` / index status): `tau_core::root_prefix(&Path)` is the single
  implementation of the `/Assets/<platform>/common/` rule; the CLI's `prefix()` and the Tauri
  adapter's `root_prefix()` are gone, both call the engine now. `Core::index_status:
  IndexStatus` (`NoIndex`/`Ready { tracks }`/`NeedsRepair`) is computed by `inspect_card` itself;
  the Tauri adapter no longer re-derives the media-root path or re-reads/re-parses the index —
  it only maps the enum to a display string.
- **P0-2** (structured warnings/errors): `TauError` is `{ code: ErrorCode, message: String }`.
  `ErrorCode` 11-17 mirror the firmware index loader's own E-codes; 30+ are engine/domain codes
  (`InvalidMediaRoot`, `ConfirmationMismatch`, `SourceChangedSincePlan`, `Cancelled`, ...). Every
  `Vec<String>` warnings field (`Card`, `Scan`, `SyncPlan`, `SyncReport`) is now `Vec<Warning>`
  (`{ code: WarningCode, message: String }`). The CLI's process exit code is the error's numeric
  code (clamped to a byte) for engine failures, `2` for usage mistakes — verified end to end
  against a scratch copy of the real `../tau-alpha/dist` card: a bad destination path exits `30`
  (`InvalidMediaRoot`). Tauri commands return a serialisable `ApiError { code, message }` instead
  of a flattened string; the UI's `errorMessage()` picks `.message` for display.
- **P0-3** (progress/cancellation): `ProgressObserver` (blanket-implemented for
  `FnMut(Progress) -> bool`, no runtime dependency) threads through `scan_dir_with_progress` and
  the whole `plan`/`execute` family; returning `false` cancels with `ErrorCode::Cancelled`,
  checked between units of work. Tauri wires this to a `job_id` + `"tau://progress"` window
  event + a `cancel_job` command; the sync screen shows a live stage/done/total line and a
  Cancel button.

Each commit builds and passes its own tests standalone (verified by reconstructing the three as
separate, individually buildable layers rather than one combined diff): `3046b28` (P0-2),
`4eaa432` (P0-1), `0c7fe6c` (P0-3).

## Validation

- `cargo test` (workspace): 28 tests passing — 18 `tau-core` unit (17 plus the new
  `cancelling_partway_through_a_plan_stops_hashing`), 5 index conformance, 4 card inspection
  (`tests/card.rs`), 2 testkit.
- `npm run check`: zero Svelte errors on the last validation.
- `cargo clippy --all-targets`: clean apart from six pre-existing `clone`-on-slice warnings in
  `sync.rs` test code (one more than before P0-3, added by the new cancellation test following
  the same pre-existing idiom) and one `#[allow(clippy::too_many_arguments)]` on
  `journal::execute_core_move_to_journal`, explained in a doc comment (mirrors
  `sync::execute_core_move`'s own pre-existing parameter count; P1-3 will collapse the whole
  family into an options struct in one pass, not piecemeal per wrapper).
- `cargo-tauri build`: last successful app bundle includes Playlists, Problems, journal loading, and prior completed UI work.

## Known issues and incomplete wiring

- `LibraryView.svelte` exists and the `summarizeLibrary` backend/API exists, but Library navigation and its native picker were not completed. Do not claim the Library screen is user-accessible until this is wired and rebuilt.
- Recent Jobs can load a selected journal but does not yet discover journals automatically or persist a configurable report directory.
- Problems currently reports duplicate groups only; it does not yet include format, tag, cover, path, FAT32, or collision checks.
- Playlist export currently requires typing an output file path; a save-dialog picker is still pending.
- Jobs shown from a loaded journal are a concise summary, not a full journal-detail view.
- Some UI pages remain in `App.svelte`; extracted component work should continue before adding large new flows.
- **Fixed 2026-09-22 (P0-1/P0-2/P0-3):** the three boundary defects the portability audit found —
  duplicated root-prefix/index-status logic, English-only warnings and errors, and no
  progress/cancellation — are all done. See "Portability boundary" above. Remaining P1 items
  (optional `serde` feature, no panics on caller input, collapse `plan_with_*` into an options
  struct, widen the plan token, a parser fuzz target) are still open — see `PORTABILITY_AUDIT.md`.
- `build_index` is public, takes public `Entry` values, and panics on a missing `_tno` tag rather than
  returning an error — a host feeding its own data in crashes.
- **Fixed 2026-09-22:** library capability detection never matched a real card (it read `data.json`'s
  `data` key as an array; the real APF layout is `data.data_slots`), so every shipped Tau core showed
  as "legacy". The fixture had invented the shape, and nothing tested `inspect_card`. See
  `FIRMWARE_SYNC.md`. Two firmware-side questions remain open there: the art file's data slot is
  double-booked with the Phase G cold image, and its pixel format is unconfirmed — both due before
  thumbnails can be built.

## Deferred because validation/fixtures are required

- Real SD-card write/eject validation.
- Core install/update/remove against real package zips and staging-card fixtures.
- Firmware diagnostic-record decoding: docs point to an external Python decoder/firmware source, but its byte format was not copied into `Tau Omega`.
- Screenshot/log discovery and decoding: fixture layouts are not yet supplied.
- Real device-specific capability verification.

## Next recommended implementation order

**Boundary work comes first — decision D-011.** The three P0 items in `PORTABILITY_AUDIT.md` are
now done (2026-09-22; see "Portability boundary" above). Next is the P1 items (optional `serde`
feature; no panics on caller input; collapse the `plan_with_*` family into an options struct; widen
the plan token and drop the `T2-` prefix; a parser fuzz target). Then:

1. Finish Library navigation, picker, scanned rows, search, filters, and virtualisation.
2. Expand Problems checks from duplicates to format/tag/path/cover issues.
3. Persist Job history through a configured reports directory and detail view.
4. Add playlist create/rename/reorder/import plan flows.
5. Add storage planning and backup/package dry-run views.
6. Add fixture-based diagnostics, screenshots, logs, and core package workflows when their source fixtures are provided.
7. Validate write operations on a designated test card only after review.

## Safety and UX baseline

- Source media is never edited by sync, cover, playlist, or duplicate workflows.
- Writes use plan → review → confirmation → verification; destination index writes last.
- Move requires a verified external backup and separate deletion confirmation.
- Problems and diagnostics are read-only.
- New screens use text status in addition to colour, native controls, visible focus, keyboard operation, and status messages. Target: WCAG 2.2 AA; runtime testing is still required before any compliance claim.

## Model guidance

- Use GPT-5.6 Luna at low reasoning for contained UI, docs, simple tests, and small adapters.
- Use GPT-5.6 Terra at low reasoning for cross-cutting Rust/UI safety workflows, multi-file refactors, and feature integration.
- Escalate reasoning only for failed attempts, ambiguous firmware formats, or high-risk card-write logic.
