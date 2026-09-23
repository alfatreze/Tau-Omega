# Tau Omega — status handoff

Updated 2026-09-23. All implementation work is contained in `Tau Omega/`.

## Read these first

1. This file — state, known issues, what to do next.
2. `DECISIONS.md` — D-001..D-013, including the integration target, the licence boundary, and the
   release artifact versioning/layout rule.
3. `PORTABILITY_AUDIT.md` — every P0/P1/P2 item, all done as of 2026-09-22.
4. `FIRMWARE_SYNC.md` — what we assume about tau-alpha, last verified 2026-09-22 against v0.4.0.

This folder is a Git repository, pushed to **github.com/alfatreze/Tau-Omega** (public), branch
`main`, with branch protection (PRs required, admin can bypass) and a GitHub Actions CI workflow
(`cargo fmt`/`clippy`/`test` — default and `serde`-feature builds — across macOS/Windows/Linux, plus
`npm run check`). **Standing instruction (session, 2026-09-23): push plain commits straight to
`main` going forward, no PRs, no waiting on CI** — that superseded the PR workflow CI was originally
set up for. Tagged releases: `v0.1.0`, `v0.2.0`, `v0.3.0`. Current version across
`Cargo.toml`/`src-tauri/Cargo.toml`/`src-tauri/tauri.conf.json`/`ui/package.json` is `0.3.0`. The
firmware project is the sibling `../tau-alpha`, which is read-only from here (D-010).

Release artifacts live under `releases/v{version}/` (`DECISIONS.md` D-013) — show a directory
preview and get approval before committing a new one.

## Current deliverable

**v0.3.0** (tagged, committed): `releases/v0.3.0/` — a macOS aarch64 `.app`, zipped
(`Tau Omega_0.3.0_aarch64.app.zip`), `SHA256SUMS.txt`, `RELEASE_NOTES.md`. No `.dmg` this release —
the bundler's `bundle_dmg.sh` hit a local Finder/AppleScript automation permission error on this
machine (macOS 26.6.2, "Can't set statusbar visible... (-10006)"), unrelated to the app; unresolved,
not investigated further this session. To rebuild: `cd ui && npm run build`, then from `src-tauri`,
`cargo tauri build --bundles app --config '{"build":{"beforeBuildCommand":""}}'` (the default
`beforeBuildCommand` — `npm run build` — fails because it runs from the repo root, which has no
`package.json`; build the frontend manually first instead). If `cargo build`/`cargo tauri build`
fails referencing a `.../Tau Browser/...` path that no longer exists, that's a stale build-script
cache from the pre-rename repo (`rm -rf src-tauri/target/release/build/tauri-* src-tauri/target/release/build/tau-omega-*`, or the debug-profile equivalent under `target/debug/build/`, then rebuild) —
seen and worked around twice now (clippy, and this release build), never fixed at the root.

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

### P1-1 — optional `serde` feature (2026-09-22)

Every public `tau-core` type now derives `Serialize`/`Deserialize` behind a `serde` cargo feature,
off by default (verified: the default `cargo build -p tau-core` does not compile `serde` at all).
`ErrorCode` keeps a hand-written impl so it stays the plain `u16` wire shape every boundary already
used (not the derive's PascalCase default); the fieldless enums (`WarningCode`, `CopyState`,
`DifferenceState`, `IndexStatus`, `Stage`) use `#[serde(rename_all = "snake_case")]` to match the
hand-written wire strings. Checked by `crates/tau-core/tests/serde_feature.rs` (compiled only with
the feature) and by `docs/DEPENDENCIES.md`.

`src-tauri` now enables the feature and its DTO layer shrank from 13 structs to 7: `ApiError`,
`WarningView`, `SettingView` and `DifferenceView` are gone — commands return `TauError`, `Warning`,
`PersistedSetting` and `MediaComparison`/`MediaDifference` (via `#[serde(flatten)]`) directly.
`CoreView`, `SyncPlanView`, `PlaylistView`, `MediaScanView`, `LibrarySummaryView`, `ComparisonView`
and `DuplicateView` stay: each does presentation (an English status sentence) or aggregation
(counts not stored on the engine type), not routing around a missing `Serialize` impl. The UI's
wire-visible shapes are unchanged except `execute_sync`/`execute_core_copy`/`execute_core_move`,
which now return the engine's own `SyncReport` (a superset of the old result view — `plan_id`,
`deleted` and `index_sha256` are new, additive fields); `ui/src/lib/types.ts` gained a `SyncReport`
type to match.

### P1-2 — no panics on caller input (2026-09-22)

`build_index`'s `entries[i].tags["_tno"].parse::<u16>().unwrap()` turned out to already be safe in
practice (`build_index` sets `_tno`/`_title` on every entry itself, just before reading them back),
but the indexing style was one refactor away from a real panic, so it's now `tno_of`/`title_of`
helpers that fall back to a safe default (`0`/`"Track"`) rather than index-and-unwrap — safe by
construction, not by an invariant that could quietly break. A second, genuinely live panic was
found in the same pass: `sync::plan`'s public `sources: &[PathBuf]` reached a bare
`.file_name().unwrap()` for a source path with no derivable file name (`/`, `.`, a bare drive
letter) in two places; both now go through a `required_file_name` helper returning
`ErrorCode::InvalidPathReference`. New tests:
`build_index_does_not_panic_on_a_hand_built_entry_with_no_tags` (`tests/conformance.rs`) and
`required_file_name_does_not_panic_on_a_nameless_path` (`sync.rs`). All byte-conformance tests
still pass unchanged — this was a pure defensive refactor with no behaviour change on valid input.

### P1-3 — collapsed telescoping constructors (2026-09-22)

`plan` -> `plan_with_options` -> `plan_with_features` -> `plan_with_layout` (four deep) is now one
public `plan(sources, common, root_prefix, options: PlanOptions, progress)`, where `PlanOptions {
mirror, embed_covers }` derives `Default`. `plan_with_options` and `plan_with_features` are gone.
`plan_core_copy` stays separate (a distinct whole-library-copy operation, not "plan with more
options") and now forwards `PlanOptions::default()` into the same private impl. Every call site
(tau-cli, the Tauri adapter, this crate's own tests) was updated to match; verified end to end
against a scratch copy of the real `../tau-alpha/dist` card that plain and `--mirror` plans still
produce distinct tokens as before.

### P2 — plan token and a parser fuzz target (2026-09-22)

**P2-1:** the plan id was a SHA-256 truncated to 32 bits, formatted `T2-xxxxxxxx` (thin for a token
that may be persisted or handed across a process boundary, and the prefix leaked an internal
roadmap phase label). It's now the full 64-character SHA-256 hex digest, unprefixed. Confirmation
everywhere is a plain string comparison, so nothing else needed to change. New test:
`plan_id_is_a_full_sha256_hex_digest_with_no_phase_prefix`.

**P2-2:** re-derived every offset `walk`/`string_at`/`track_path` use and confirmed each is
bounds-checked by `parse()`'s own section-table validation before use — but that needed proof, not
just re-reading the code. New `crates/tau-core/tests/fuzz_lite.rs`: a dependency-free, deterministic
test that corrupts real fixtures' section offsets/lengths, root string offset and record counts to
boundary-heavy values, **recomputes both CRCs** so the mutation reaches the offset-driven logic
instead of failing at the CRC gate (the audit's exact gap: "a crafted-but-CRC-valid file driving
offsets is not covered"), and asserts no panic. Runs in ordinary `cargo test`; found none across
9,000 trials. Also added a standalone `cargo-fuzz` scaffold under `fuzz/` (its own `[workspace]`,
zero effect on the main build) for real coverage-guided fuzzing — not run in this session (no
`cargo-fuzz`/nightly toolchain available here); see `docs/DEPENDENCIES.md`.

## Validation

- `cargo test` (workspace): 46 `tau-core` unit tests passing as of 2026-09-23 (grown from 20 across
  this session's features — Library/Problems/Job-history/Playlist/Storage/Backup/Package/diag —
  each with its own tests against real or synthetic fixtures), plus 6 index conformance, 4 card
  inspection, 1 fuzz-lite, 2 testkit; +6 more in the feature-gated `serde_feature.rs` (only compiled
  with `--features tau-core/serde`). Re-run `cargo test --workspace` for the current exact count
  rather than trusting this number as it ages.
- `npm run check`: zero Svelte errors on the last validation.
- `cargo clippy --all-targets`: clean apart from seven pre-existing `clone`-on-slice warnings in
  `sync.rs` test code (one more than after P0-3, added by the new plan-id test following the same
  pre-existing idiom) and one `#[allow(clippy::too_many_arguments)]` on
  `journal::execute_core_move_to_journal`, explained in a doc comment (mirrors
  `sync::execute_core_move`'s own pre-existing parameter count).
- `cargo-tauri build`: builds clean with `tau-core`'s `serde` feature enabled (`src-tauri/Cargo.toml`).
- `cargo metadata` from the repo root still lists only `tau-core`/`tau-cli`/`tau-testkit` —
  `fuzz/`'s own `[workspace]` keeps it fully isolated from the main build.

## Known issues and incomplete wiring

- Playlist export currently requires typing an output file path; a save-dialog picker is still pending.
- Some UI pages remain in `App.svelte`; extracted component work should continue before adding large new flows.
- Playlist create/import destination filenames are typed by hand rather than picked from a directory
  listing of existing `.m3u` files, same limitation as playlist export above.
- **Fixed 2026-09-22 (P0-1/P0-2/P0-3/P1-1/P1-2/P1-3/P2):** every item in `PORTABILITY_AUDIT.md` is
  now done — the three P0 boundary defects (duplicated root-prefix/index-status logic,
  English-only warnings and errors, no progress/cancellation), P1-1 (optional `serde` feature; DTO
  layer shrank from 13 to 7 structs), P1-2 (no panics on caller input), P1-3 (collapsed
  `plan_with_*` into one `plan(..., PlanOptions, ...)`), and P2 (full-width unprefixed plan token;
  a fuzz-lite regression test plus a `cargo-fuzz` scaffold for the index parser). See "Portability
  boundary", "P1-1", "P1-2", "P1-3" and "P2" above.
- **Fixed 2026-09-22:** library capability detection never matched a real card (it read `data.json`'s
  `data` key as an array; the real APF layout is `data.data_slots`), so every shipped Tau core showed
  as "legacy". The fixture had invented the shape, and nothing tested `inspect_card`. See
  `FIRMWARE_SYNC.md`. Two firmware-side questions remain open there: the art file's data slot is
  double-booked with the Phase G cold image, and its pixel format is unconfirmed — both due before
  thumbnails can be built.
- **Fixed 2026-09-23:** the Library screen is now user-accessible end to end — nav button, native
  folder picker, `scan_library` (renamed from `summarize_library`) returning real `TrackRow`s
  (title/artist/album from tags, filename fallback, duration, format), progress/cancel during the
  scan, a search box plus MP3/FLAC filter, and a hand-rolled virtualised track table (fixed row
  height, overscan window, `translateY`) verified against 12,000 synthetic rows in a real browser
  session. Incidental find while verifying it: a pre-existing Svelte CSS-scoping bug meant
  `App.svelte`'s shared rules (`.jobs-panel`, `.picker`, `.picker-row`, `.settings-card`,
  `.settings-values`, `.comparison-counts`) never applied to *any* child-component overlay screen
  (Jobs/Playlists/Problems/Settings, not just Library) — a component's `<style>` block only scopes
  to its own template. Fixed by moving those rules into the already-global `ui/src/styles.css`.
- **Fixed 2026-09-23:** Problems now covers five categories, not just duplicates — new
  `tau_core::problems::find_problems` (replacing the old bare `find_duplicates` command, renamed
  `find_problems`) also flags missing title/artist tags, ID3v2.2 tags (the cover embedder only
  handles v2.3/v2.4), path issues (non-ASCII names that would be renamed on sync, paths over the
  firmware's 200-character limit, and names that collide once folded to on-card ASCII — reusing
  `ascii_name`), and folders with neither a folder-level cover file nor any embedded APIC/PICTURE
  art (new `cover::has_embedded_cover`, read-only). `ProblemsView.svelte` groups results by category
  with counts; verified in a browser against synthetic data covering all five kinds. FAT32-specific
  checks are not covered — nothing in this engine writes to a card yet, so there is no FAT32 path to
  validate against.
- **Fixed 2026-09-23:** Job history now persists across restarts through a configured reports
  directory instead of one-file-at-a-time manual loading. `journal::execute_to_journal` and
  `execute_core_move_to_journal` now record a `kind` (`sync`/`mirror`/`core_copy`/`core_move`) in
  every journal, and new `journal::list_journals` lists every journal in a directory, newest first,
  skipping anything unreadable rather than failing the whole listing. The Tauri adapter persists the
  user's chosen directory as one small file in this app's own config directory (`get_reports_dir`/
  `set_reports_dir`, via `tauri::Manager::path().app_config_dir()`) and adds `list_journals`. In
  `App.svelte`, once a reports directory is set, `runSync`/`runCoreCopy`/`runCoreMove` write their
  journal there automatically (a generated `{dir}/{timestamp}-{kind}.json` path) instead of the
  manual manifest field, and the Jobs page auto-lists history from it with a "Details" button per
  entry opening the full journal JSON — replacing the old one-line summary. The manual single-file
  loader stays, for a journal outside the configured directory. Verified against synthetic journal
  data in a browser (list, kind labels, state, and the detail panel).
- **Fixed 2026-09-23:** Playlists now support create, rename, reorder, and import, not just read and
  export. New `tau_core::playlist::{PlaylistPlan, plan_write, plan_rename, plan_import, execute}`
  follow the same plan -> review -> confirm -> execute shape as `sync::plan`/`sync::execute`: a
  content-hash `id` gates `execute`, so a stale confirmation refuses. `plan_write` handles both
  create and reorder (writing an ordered track list to a file); `plan_rename` moves the `.m3u` file
  while normalising any bare (folder-relative) lines to root-rooted form first, so the playlist keeps
  resolving correctly from its new location; `plan_import` matches each line of an external `.m3u`
  against the media root's tracks (first by root-relative path, then by unique bare filename),
  listing anything unmatched in `PlaylistPlan::dropped` rather than silently keeping or dropping it
  unreported. `Playlist` gained a `file` field (which `.m3u` it was read from) so a front-end can
  target these without re-deriving the scan's own naming/folder-collapsing rules. New Tauri commands
  `plan_playlist_write`/`execute_playlist_write` (create+reorder), `plan_playlist_rename`/
  `execute_playlist_rename`, `plan_playlist_import`/`execute_playlist_import`; `scan_media`'s
  `MediaScanView` now resolves each playlist's tracks to relative-path strings (`PlaylistDetailView`)
  instead of a bare count, since the Playlists page needs them to reorder in place. `PlaylistsView.svelte`
  gained reorder (up/down per track), rename, create (paste-in track paths), and import (with a
  dropped-lines list) sections, each with its own plan/review/confirm flow. 7 new engine tests;
  verified end to end in a browser against synthetic scan/plan data (reorder swap+save, import with
  2 matched/2 dropped lines).
- **Fixed 2026-09-23:** Storage planning and a generic backup dry-run, both read-only (per
  `IMPLEMENTATION_PLAN.md`'s phase-2 "safe dry-run now; validation later" — an execute path for
  backup is deliberately not built; that's real-card-write territory, item 7). New
  `tau_core::storage::{VolumeSpace, CapacityCheck, check_capacity}` reports free/total space on a
  path's volume (walking up to the nearest existing ancestor for a destination that doesn't exist
  yet) and whether a plan's `bytes_to_write` fits after a 16 MiB safety margin. This needed the
  `fs4` crate (owner decision, since std has no cross-platform statvfs equivalent and this crate's
  audited "no `process::Command`" property rules out shelling out to `df`) — a real jump from
  `tau-core`'s usual 3 dependencies to 4 direct (+7 transitive via `rustix`/`windows-sys`), scoped
  to `--no-default-features --features sync` and written up in `DEPENDENCIES.md`. New
  `tau_core::backup::{BackupItem, BackupPlan, plan}`: unlike `compare::media_roots`, `source`/
  `destination` are not required to be `Assets/<platform>/common` media roots (a backup target is
  commonly just a folder on an external drive), and a destination that doesn't exist yet is treated
  as "nothing to compare against" rather than an error; reuses `compare::files_by_relative_path`
  (now `pub(crate)`) rather than re-implementing the walk-and-hash logic. New Tauri commands
  `check_storage_capacity`, `plan_backup`. New "Backup" nav page (`BackupView.svelte`): plan a
  folder-to-folder backup, see new/updated/unchanged/destination-only counts, bytes to write, the
  capacity check, and the full item list — framed explicitly as preview-only. The existing Sync and
  Compare-cores plan reviews also gained an inline capacity-check line (fits/doesn't fit + free
  space) next to their existing plan summaries. 7 new engine tests; verified in a browser against
  synthetic data (a plan that doesn't fit on Backup, one that fits on Sync).
- **Fixed 2026-09-23:** `read_persisted_settings` never actually read a real card. It looked for
  `variables` at the JSON root, but the real APF layout nests it under `interact_persist`
  (`{"interact_persist": {"magic": "...", "variables": [...]}}`) — found only because real
  hardware-captured `interact_persist.json` files were copied in as fixtures instead of trusting the
  existing hand-written one, the same class of bug `slots_have_library` had (`FIRMWARE_SYNC.md`'s own
  closing lesson, now proven twice). The Settings screen has silently shown nothing from a real card
  since it was written. Fixed by checking `/interact_persist/variables` first, falling back to a bare
  top-level `variables` for older hand-written fixtures/tools. Also added
  `tau_core::diag::{decode_check_summary, read_check_summary}` (see "Deferred" below) and wired a
  "Diagnostic Check summary" card into `SettingsView.svelte`, shown only when persist ids 20-23
  actually decode as one. Verified against the real fixtures in the browser.
- **Fixed 2026-09-23:** core package install/update against a real release zip. New
  `tau_core::package::{PackageManifest, PackagePlan, PackageReport, inspect, plan_install,
  execute_install}`, the same plan -> review -> confirm -> execute shape as `sync`/`playlist`:
  `execute_install` re-hashes every entry against the zip immediately before writing (catches a zip
  that changed on disk since the plan) and reads the written file back to verify it after. Needed the
  `zip` crate (owner decision) — real release zips are deflate-compressed, so structure-only parsing
  wasn't enough; scoped to `deflate-flate2-zlib-rs` only (no `zopfli`, which is compression-only and
  this is read-only), 7 new transitive packages, written up in `DEPENDENCIES.md`. Tested against the
  real `alfatreze.TAU_0.4.0_2026-09-22.zip` copied into `testdata/packages/` (15 real files): first
  install is all-new, a second plan against the now-installed card sees everything unchanged, and
  changing one file on disk makes the next plan correctly call it out as an update and only rewrite
  that one file. New Tauri commands `inspect_package`/`plan_package_install`/`execute_package_install`
  and a new "Packages" nav page (`PackageView.svelte`): choose a zip and a staging-card folder,
  inspect, plan, review the new/updated/unchanged counts and full item list, confirm. Verified in a
  browser against the real manifest shape. Removing an installed core is explicitly out of scope —
  `Cores/<id>` is safe to delete alone, but more than one core can share an `Assets/<platform>`
  folder, so a correct "remove" needs to check every other installed core's `core.json` first; a
  separate, smaller piece of work, not a corner to cut here.

## Deferred because validation/fixtures are required

- Real SD-card write/eject validation.
- Removing an installed core (`Cores/<id>` plus, only when safe, its `Assets/<platform>` folder) —
  see "Known issues and incomplete wiring" above for why this is separate from install/update.
- **Done 2026-09-23:** the persisted Check-report summary (persist ids 20-23) is decoded —
  `tau_core::diag::{decode_check_summary, read_check_summary}`, a byte-exact port of
  `tau-alpha/tools/decode_tau_suite.py`'s `unpack_words`, tested against three real hardware-captured
  `interact_persist.json` fixtures in `testdata/interact_persist/` (all passed, some failed, and the
  legacy-overload rejection case). See "Known issues and incomplete wiring" below. The full TAUD1
  QR/text record format (base64 + CRC32 TLV, `parse_record`) is not ported — no real QR screenshot
  fixture exists yet to test it against, and it needs a `base64` dependency decision first.
- Screenshot/log discovery and decoding: no real hardware screenshot fixtures found yet (only doc
  illustrations); needed both for QR-based Check decoding and for any screenshot-driven diagnostics.
- Real device-specific capability verification.

## Next recommended implementation order

**Boundary work is done — decision D-011.** Every P0/P1/P2 item in `PORTABILITY_AUDIT.md` is now
done (2026-09-22; see "Portability boundary", "P1-1", "P1-2", "P1-3" and "P2" above). `tau-core` is
ready for Pocket Sync to adopt as a crate dependency on the boundary-correctness front; nothing
below is blocked on it. Next:

1. ~~Finish Library navigation, picker, scanned rows, search, filters, and virtualisation.~~ **Done
   2026-09-23** — see "Known issues and incomplete wiring" above.
2. ~~Expand Problems checks from duplicates to format/tag/path/cover issues.~~ **Done 2026-09-23** —
   see "Known issues and incomplete wiring" above.
3. ~~Persist Job history through a configured reports directory and detail view.~~ **Done
   2026-09-23** — see "Known issues and incomplete wiring" above.
4. ~~Add playlist create/rename/reorder/import plan flows.~~ **Done 2026-09-23** — see "Known
   issues and incomplete wiring" above.
5. ~~Add storage planning and backup/package dry-run views.~~ **Storage planning and backup dry-run
   done 2026-09-23** — see "Known issues and incomplete wiring" above. Package dry-run stays out of
   scope until item 6's fixtures arrive (it needs the same zip-reading groundwork as real package
   install, so it isn't worth building twice).
6. Add fixture-based diagnostics, screenshots, logs, and core package workflows when their source
   fixtures are provided. **Diagnostics (Check summary) and core package install/update done
   2026-09-23** — see "Known issues and incomplete wiring" above. Still open: core remove, the full
   TAUD1 QR/text record format, and screenshot/log discovery (no real fixtures found yet for either).
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
