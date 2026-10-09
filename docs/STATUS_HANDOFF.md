# Tau Omega — status handoff

## START HERE (2026-10-08): fresh-session handoff

**Read `ROADMAP.md` (repo root) next: it is the one ordered list of Omega work.** Then this block, then the dated sections below only for detail.

**Scope rule (owner, 2026-10-08): Tau Omega work only.** The Tau Alpha repository (`../tau-alpha`) is **read-only** from here: never edit, commit or reset there; read it for integration facts. A Tau Alpha
feature is planned in Omega **only once it is on Tau Alpha `main`** (not a branch, worktree or design doc): meter packs, the RAM diet and 720p output are therefore not planned yet. (On 2026-10-08 a roadmap was first
written into the tau-alpha repo by mistake; it was reverted with a hard reset to `origin/main`, nothing lost, and the roadmap rewritten here.)

**Repo state.** One worktree, `Tau Omega/` (branch `main`, pushed, clean). The old `Tau Omega-integration/` worktree and its branches were merged and removed on 2026-10-08. Older work is kept as tags on origin
(`archive/main-2026-10-02`, `archive/integration-workbench-base-2026-10-03`, `archive/local-main-2026-10-02`, `archive/ci-fix-and-add-ui-check`, `archive/omega-cards-meters-firmware`, `archive/claude-exciting-bell`,
`archive/claude-jolly-meitner`) and in a full bundle at `~/Downloads/tau-omega-repo-backup-2026-10-08.bundle`. Remaining remote branches (old, archived, deletion optional): `claude/exciting-bell-emz8il`,
`claude/jolly-meitner-jwdzwx`, `omega-cards-meters-firmware`. Newest test build: `releases/dev-builds/0.4.0-alpha.8/Tau Omega.app` (built from `main` at `94c9687`, opened by the owner for a first look; app bundles are
untracked, only `BUILD.txt` and `SHA256SUMS.txt` are committed). Use a single worktree per task from now on (`git worktree add`), never switch the branch of a shared one.

**Built and verified this stretch (all on `main`):** the verification ledger (scan cache, provenance, canary, "Forget" menu item); "Preparing sync" status the instant Start sync is pressed; Appearance editor and **theme file install to a card
(real write on `tau_dev_67`, loaded on a Pocket, `Info > THEME FILE` = `1 LOADED`)**; the AppleDouble `._*` sweep after sync, core move, workbench and theme install (real-card checked); Send diagnostics (read, summary, zip, no track names);
QR tags 16, 19-22 decoded from real captures plus the BLIT TEST profile and Cold frame test name; CI fmt/clippy green locally; roadmap, architecture and safety docs refreshed (`SAFETY_RULES` 14-19). 161 Rust tests, 174 browser checks, clippy and fmt clean.

**Next, in order (details in `ROADMAP.md`):** (1) **install and update paths** (owner's top item: first install onto a Pocket card that has other cores but no Tau; update from GitHub, opt-in; update from a zip; "Refresh library" for files copied by hand), with a shared
post-install check; (2) **read the pixel-grid report codes** (Tau Alpha main's default report view; Omega decodes QR only; real fixtures are in `testdata/screenshots/20261004_*.png`, format in tau-alpha `tools/tpg.py` and `docs/features/BARCODE_STUDY.md`);
(3) `tau-assets.bin` read-modify-write plus the Halcyon `PRST` writer and APO/AutoEQ importer; (4) hardening. Owner decisions still open: update check button-only or also at launch; compatibility table inside Omega or fetched; automatic library re-verify after an update;
the HTTPS client for the GitHub check (recommend `ureq`; measure and flag its cost before adding any dependency).

**Roadmap item 1 progress (2026-10-08).** Engine pieces built and tested on real release zips, no card written: `tau_core::update` (`assess_update`: new install / same build / update / same-date-different-build / older / mismatch, with reasons, the files an update replaces and the user files it keeps; `pair_status` reads `TAUFWPAIR`/`TAUFWNEED` from the ROM and checks the bitstream against `KNOWN_BITSTREAMS`, seeded from the alpha.3/alpha.4 release gate; `post_install_check`: seven checks, worst status wins, `to_text()` for the saved report). Findings: a package cannot reveal its bitstream's `CORE_VERSION`, so pairing is verified only for bitstreams in the table (others read "cannot verify", never a failure); the shipped core declares slot names over 15 characters and works, so that Analogue limit is not enforced. Not done yet: Tauri/CLI wiring, catalog-cache clearing and `._` sweep in the install itself, backup-and-rollback, GitHub check (needs the HTTPS client decision), Refresh library (1d).

**`tau-assets.bin` read-modify-write (2026-10-08, merged to `main`; archive tag `archive/taua-roundtrip`).** Roadmap item 3's first half: saving themes, by install or Export, no longer deletes the card's meter presets (`METR`) or Halcyon user EQ presets (`PRST`). `pack_assets_keeping`/`kept_sections` in `assets.rs`; `appearance_export` gained an optional `keep_from` (the opened file); a newer container version is refused; the 8-section and 64 KiB limits are enforced. 185 Rust tests with serde, strict clippy, fmt and svelte-check clean. Built from a Tau Alpha session at the owner's request (Tau Alpha review `RELEASE_SYSTEM_REVIEW_2026-10-08.md` H1); merged fast-forward on 2026-10-08 after review (one fix: the required-slot check keeps the old `Cores/<id>/` fallback).

**Release manifests and the new card layout (2026-10-08, merged to `main`; archive tag `archive/release-compat`).** Built from a Tau Alpha session at the owner's request (Tau Alpha review `RELEASE_SYSTEM_REVIEW_2026-10-08.md`), engine only, no UI wiring yet. **`tau_core::compat`:**
- `parse_compat` reads `tau-compat.json` schemas 1 and 2. It refuses any other schema (new error code 54, `InvalidReleaseManifest`) and ignores unknown keys.
- `identify_installed` names the installed release by hashing its owned files. It removes the guesswork behind `SameDateDifferentBuild` once releases carry the file.
- `persist_changed_since` implements the union rule over skipped releases from `persist_registry`.
- `check_card` follows the same rules as Tau's reference `tau_compat.py check-card`.
- `compare_tags` gives SemVer precedence (`dev < preview < rc < release`).

**Paths for Tau's layout change (H4).** `tau.rom`, `tau-cold.bin` and `tau-loading.bin` move to `Assets/<platform>/<core>/` with core-specific data slots. Omega accepts both places:
- `update.rs`: the build identity, and the required-slot check, which now honours parameter bit 1;
- `diagnostics.rs`: the file list.

**Tests:** 7 new; 191 Rust tests with serde, strict clippy and fmt clean. **Cross-implementation check:** a real Tau dev-package manifest parsed and checked clean in both implementations, and both reported the same single error after a one-byte ROM change (`TAU_COMPAT_CASE=<dir>` runs it).

**Gate:** H4 is on Tau's `release-system` branch, not on Tau `main` (D-015) and not yet confirmed on a Pocket. Merging this is safe either way (old layout still read), but the Tau layout itself waits for the card probe.

**Manifest wiring (2026-10-08).** `update::assess_update_with`, `assess_with`, `pair_status_with`, `post_install_check_with` take release manifests (`compat::CompatDoc`); the old functions are the same with none. A manifest names the installed and packaged builds by release tag (tags order two same-day builds exactly), supplies the bitstream `CORE_VERSION` (outranks `KNOWN_BITSTREAMS`, and vouches for a ROM without a marker), lists the persisted ids an update changes (`UpdateReport.persist_changed`), and a schema-2 manifest adds a `release layout` item (`compat::check_card`) to the post-install check. Tested with schema-2 manifests built from the real zips' own hashes; Tau's own manifest is not on Tau main yet. Still open: fetching `tau-compat.json` in the GitHub check (`ureq`), reading a dev package's manifest beside a local zip, UI wiring.

**GitHub update check (2026-10-08, roadmap 1b).** Owner decisions: the check runs automatically at start (after a one-time notice, with an off switch in Settings), the update is never automatic; the compatibility table comes from the release (`tau-compat.json`). `tau_core::release_check` is the pure logic (real fixture `testdata/github/releases.json`); `src-tauri/src/updates.rs` does the fetching with `ureq` (+15 crates, +2 MB, see `docs/DEPENDENCIES.md`); `UpdateBanner.svelte` shows the notice and the alert. Verified live against the real repository: release list read, zip downloaded, hash checked against `SHA256SUMS.txt`, zip inspected (`cargo test -- --ignored live_check` in `src-tauri`). Findings: the real releases carry two zips per core (`alfatreze.TAU_0.6.0-alpha.3_2026-10-04.zip` and the plain `alfatreze.TAU_0.6.0_2026-10-04.zip`) but `SHA256SUMS.txt` lists only the plain ones, so only listed zips are offered; no release has a `tau-compat.json` yet, so installed builds are identified by date until one does. Not run in the app UI (browser suite needs Playwright), svelte-check clean.

**Install plan (2026-10-08, roadmap 1a/1c, plan stage only, nothing executes).** `tau_core::install_plan::plan(zip, card, manifests, allow_downgrade)` returns one reviewable `InstallPlan` with a confirmation `id`: the update verdict and pairing result, the file plan, **what to back up first** (installed files that would be overwritten, manifest-obsolete files, the five catalog caches), what to remove (obsolete files, those caches, `._` stubs beside files the install writes), the user files kept, superseded numbered test cores (offered, never auto-removed), free space on the card, `nothing_to_do`, a `refused` reason (pair the Pocket would black-screen on, mismatching core, downgrade not chosen, card too full) and `cautions` (downgrade, same-day builds, pairing not checkable). Finding: `package::plan_install`'s `id` covers the zip and destination but **not the card's current contents**, so `still_current` compares the per-file states instead. Next, with approval at each card write: the executor (backup outside the card, verified, replace, remove, clear caches, sweep, post-install check, journal, rollback from the backup), tested on scratch folders first, then the owner's `CARDWRITE` card.

**Install executor (2026-10-08, roadmap 1a/1c; scratch folders only, no real card written).** `tau_core::install_exec::execute(zip, card, plan, confirmation, backup_root, manifests)`: checks the token, refusal, a backup outside the card and plan freshness; copies everything it will overwrite or remove to `<backup_root>/<plan id>/files/` and **reads each back and hash-checks it**; writes `install-journal.json` before the first card write; writes the package files; removes obsolete files, the five catalog caches and `._` stubs; runs the post-install check per core (`report.ok()`); **any failure after the backup rolls back by itself**. `rollback(card, backup_dir)` restores from the journal alone (re-verifies each backup hash first, refuses another card's backup or a damaged one, removes files and folders the install created, safe to run twice). New error code E55 `InstallRefused`. Tauri commands `plan_core_update`, `execute_core_update`, `rollback_core_update` (card write lock held; backups under the Settings backup folder's `installs/`, else the app data dir) are wired but have no UI and pass no manifests yet. Tests (9) cover update, rollback to byte-identical (except swept `._` junk), first install rolled back to an untouched card, a mid-way failure rolling itself back, every pre-write guard, a repeated plan, a damaged backup, same-build no-op, wrong-card journal. Still owed: a real-card run on `CARDWRITE` (needs the owner's approval), UI for plan review, manifests passed from the GitHub check, a note that a `.part`/power-loss mid-run leaves the journal in state `started` (rollback works from it).\n\n**Plan-review UI (2026-10-08).** `ui/src/lib/UpdatePlanView.svelte`, embedded in the Packages page under the inspected package: *Review update* shows the verdict per core with reasons and the pairing result, a downgrade checkbox (re-plans), refusals and cautions, the safety net in plain words (what is backed up and where, caches cleared, stubs removed, obsolete files, files kept, persisted settings that may reset, superseded test builds left alone, space), the per-file list, then *Confirm and install*; the result shows the post-install checks and an *Undo this update* with its own confirmation. The old plain-file plan stays, relabelled. Checked in a real browser against `dev-mock.ts` (mocks added): review, install, report and undo all render and work; svelte-check clean. Not exercised against the real backend from the UI, and the browser suite has no test for it yet. Install still passes no manifests.\n\n**Manifests passed into plan and install (2026-10-08).** `release_check::{manifest_matching_zip, sibling_manifest, save_cached, load_cached, manifests_for_install}`: a plan now knows (a) every `tau-compat.json` the GitHub check or a download has seen (kept in the app cache, `manifests/`, so planning works offline) and (b) a dev package's own `tau-compat.json` beside the zip, **used only if one of its packages is exactly that zip by SHA-256** (another zip's manifest never vouches for this one; the sibling wins over a cached manifest for the same release). `plan_core_update` and `execute_core_update` build the list the same way, so the confirmation token matches; a download also writes the manifest beside the zips. Effects: release tags in the verdict, manifest `CORE_VERSION` for pairing, obsolete files in the backup/removal, persisted-id changes, and the `release layout` item in the post-install check. 224 tests, clippy clean; not exercised with a real Tau manifest (none published on Tau main yet).\n\n**First real-card install run (2026-10-08, `CARDWRITE`, spare card, owner-approved step by step).** Plan: read-only, card proven byte-identical. Install of the real `alfatreze.TAU_0.6.0_2026-10-07.zip`: 15 files written and checked byte for byte, the 5 catalog caches backed up (outside the card, hash-verified) then cleared, post-install check passed except the expected "no library yet"; `alfatreze.CARDWRITE02` and `Test Album` untouched. **Three real bugs found only on the real card (macOS/fskit creates a `._` companion for every file and folder it writes) and fixed:** (1) the executor swept only stubs that existed at plan time, so the install's own writes left 19 (15 reported by the check): it now sweeps companions of everything it wrote, after writing; (2) rollback removed the 15 files but left their folders and companions: the journal now records `created_dirs`, rollback removes the companions, then only the folders the install made (an older journal falls back to the folders above created files); (3) rollback's *restore* writes created 5 new companions in `System/`: a companion that was not there before a restore is now removed after it. Rollback on the card: 5 files restored, 15 removed, folders and `._` for them gone; the 5 `System/._*` from bug 3 were still on the card when it ran (deleted with the owner's OK; the card then matched the before-snapshot on every file hash). **Re-run with the fixed code (same day, owner-approved): install swept all 19 companions (the check's `stray files` item passed), rollback restored 5 / removed 15 and the card then matched the before-snapshot on every file hash, `._` files included; journal state `rolled_back`. Not tested: booting the Pocket from the installed core, and an update (older to newer) on a real card.** Artefacts: `../tau-omega-install-run/` (card copy before, `before.sha256`, the backup and journal). Gated test: `install_exec::tests::real_card_install_run` (modes plan/install/rollback, env vars in its doc comment).\n\n**Refresh library (2026-10-08, roadmap 1d; scratch folders and a mocked browser only, no real card write).** `tau_core::refresh`: `index_state` (cheap health: present, valid, rooted at this core, listed files missing), `plan_refresh`, `execute_refresh`, `rollback_refresh`. The plan lists every file the library would skip or change, each with a plain reason: non-ASCII names (renamed to the ASCII form, folders before the files inside them), name collisions after folding (case-insensitive, as exFAT; left alone and kept out of the library), paths over 200 bytes with the core's prefix (kept out), formats the Pocket cannot play (.m4a, .wav, ...), unreadable tags and ID3v2.2 (listed by file name), capacity. Playlists (`.m3u`) that point at renamed files, absolute or relative, are rewritten to follow. Execution: token and backup-outside-card guards, plan freshness, backup of the old index and every playlist it rewrites (hash-checked, journal), renames, playlists, `._` sweep, rebuild with the excluded files filtered out, verify against the files on the card, **any failure rolls back by itself**; `rollback_refresh` undoes a run from the backup alone. A folder with no playable music never replaces a working library. New error code E56 `RefreshRefused`. Tauri commands `library_health`, `plan_library_refresh`, `execute_library_refresh`, `rollback_library_refresh`; UI `RefreshLibrary.svelte` in the core detail panel (health badge plus *Refresh library…* with findings list, safety text, confirm, report and undo), checked in a browser against mocks. 11 engine tests. **Not covered: a file's sample rate/bit depth (the engine does not read it), so a file the Pocket refuses for that reason is still indexed.** **Real-card run on `CARDWRITE` (owner-approved; a throwaway `Assets/refreshtest/common` with accented, colliding and unplayable names, two real MP3s from Test Album, a playlist; removed afterwards, card byte-identical to the original snapshot): plan read-only; refresh renamed 2 folders, rewrote the playlist, rebuilt a 4-track library rooted at the core, verified; rollback restored names, playlist and absent index byte for byte. Three real findings fixed: (1) macOS stores `é` decomposed on exFAT (`e` + U+0301): handled by the renames, and a playlist spelled in the composed form is matched through the plain-letter form; (2) rollback swept every `._` file, including ones that were there before: it now removes only the companions its own restores and renames create; (3) a folder carrying extended attributes shows a `._` file that cannot be deleted, so stubs no longer count against `nothing_to_do`.**\n\n**Spotlight marker for Pocket cards (2026-10-08, owner decision: whole card, ask once, then a setting).** `tau_core::marker` (`find_card_root`, `ensure`, `remove`, `is_present`): an empty `.metadata_never_index` at the card root (a Pocket card = a folder with both `Assets` and `Cores`; nothing is ever written to a plain folder; `remove` only deletes an empty file of that exact name). Setting `card_marker` in the prefs (`ask` default, `on`, `off`). Host: every card-write command now goes through `begin_card_write(app, path)` (takes the write lock and, when the setting is `on`, ensures the marker first, best effort) instead of `card_write_guard()`; commands `card_marker_status`, `set_card_marker` (saves the answer and, for `on`, adds the marker to the open card now), `remove_card_marker`. UI: `CardMarkerPrompt.svelte` asks once when a Pocket card is open and the setting is `ask` (plain words, what it is and that it can be turned off in Settings); Settings has *Keep Spotlight off Pocket cards* with Turn on/off and *Remove from this card*; *Reset notices* asks again. **Layout bug found and fixed while building it:** the root `main` is a two-column grid, so an element inserted before the pages (the update banner, which would have shown on first launch) pushed the page into the wrong cell; pages and the two bars now sit in one `.content` column. Not applied to `CARDWRITE` yet (nothing on the card was changed for this); the stuck Trash item on that card is still open.\n\n**Tau Alpha's new release system, evaluated (2026-10-08).** It is on Tau Alpha `main` now (`release-system` merged, `dc9d21d`). Omega checked against packages built by Tau's own tools for all five channel cores: the manifest, install, check, migration from the old layout and Tau's own `check-card` all agree, and the earlier gated tests now run and pass. **Seven findings were reproduced, three unsafe, none fixed yet; see `docs/RELEASE_SYSTEM_IMPACT_2026-10-08.md` and ROADMAP 1e.** Gated tests: `crates/tau-core/tests/new_layout.rs`.\n\n**How to run things.** `cargo test --workspace --features tau-core/serde`; `cargo clippy --workspace -- -D warnings`; `cargo fmt --all -- --check`; `cd ui && npm run check`; test build: `tools/dev-build.sh` (next number automatic).
Browser suite: needs Playwright and a Chromium: `npm i -g playwright` (or any install), `cd ui && npx vite --port 5199 --strictPort &`, then `CHROMIUM_PATH=<headless chromium> URL=http://localhost:5199 NODE_PATH=$(npm root -g) node scripts/workbench-test.cjs`
(expect 174 checks; the install the last session used lived in a temporary folder and is gone). Real-card tests are `#[ignore]`d and gated by environment variables, read-only unless stated: `TAU_REAL_CARD=/Volumes/Pock cargo test -p tau-core real_card -- --ignored --nocapture`
(plan/diagnostics read-only), `TAU_REAL_WRITE_MEDIA=... TAU_REAL_WRITE_BACKUP=...` (theme install write), `TAU_REAL_SYNC_MEDIA/LIB/BACKUP` (add-and-remove sync). **Never write to a card without the owner's approval; use `tau_dev_67` or a throwaway core, backups go outside the card.**

**Traps.** `cp -a`/Finder leave `._` stubs on exFAT (package installs do not sweep them yet: roadmap 1a); a package carries no alpha number (`core.json` is `0.6.0` for every alpha): compare by `date_release` and file hashes; `tau-assets.bin` is replaced whole on save (drops `METR`/`PRST`: roadmap 3);
Omega's QR reader misses grid reports (roadmap 2); `ci.yml`'s browser-suite step has never been seen to run on GitHub; the card may carry a second volume (`CARDWRITE`): not ours.


**Updated 2026-10-02: two parallel sessions were integrated — read "Library workbench, and the integration of two parallel sessions" first.** Sections dated before that describe the earlier session's work and may name files (`CardsView`, `SyncView`, `connection_kind`) that the integration did not carry over; each such section carries an integration note.

Updated 2026-09-26 (core removal, the full TAUD1 QR decoder, real hardware write validation,
screenshot discovery, a UX/UI review with two real bug fixes and a screenshot gallery, fixes for
`cargo tauri dev` and a missing event capability found running the real app for the first time,
known/mounted-card auto-open, a Cards-screen redesign — player-core cards, a platform-category
signal, "Set as player", a detail side panel, and a help panel replacing the old read-only text —
a sidebar active-card/-core switcher, real per-core icon.bin decoding, `TIM1` cover-image
decode/encode, cover-sidecar writing wired into Sync, a lazy cover preview in the Sync plan review,
playlist path pickers, extracting the last three inline pages, a Meters UI-structure scaffold against
a synthetic schema, and real USB connection detection (Pocket vs. plain USB storage) driving a
connection-aware card icon, added in sequence). All implementation work is contained in `Tau Omega/`.

## Read these first

1. This file — state, known issues, what to do next.
2. `DECISIONS.md` — D-001..D-013, including the integration target, the licence boundary, and the
   release artifact versioning/layout rule.
3. `PORTABILITY_AUDIT.md` — every P0/P1/P2 item, all done as of 2026-09-22.
4. `FIRMWARE_SYNC.md` — what we assume about tau-alpha, last verified 2026-09-26 against its working
   tree (post-B-294; no new tagged release since v0.4.0).
5. `LIBRARY_WORKBENCH_PLAN.md` (with `LIBRARY_UX_REVIEW.md` and `usability/SYNTHESIS.md`) — the Library
   workbench, the main screen of the app.

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

- `cargo test` (workspace): 75 `tau-core` unit tests passing as of 2026-09-26 (grown from 20 across
  this project's features — Library/Problems/Job-history/Playlist/Storage/Backup/Package/diag/
  Remove/taud/screenshots/image/sync-art-sidecar — each with its own tests against real or synthetic
  fixtures), plus 6 index conformance, 4 card
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

- **Open, 2026-09-29 (cross-project, from Tau-Alpha):** Tau Omega running (even just open, not
  actively syncing) appears to hold the Pocket's mounted SD card in a way that blocks other
  processes from touching it — observed directly in the sibling Tau-Alpha session: `ls /Volumes/`
  (and other basic filesystem calls) failed there with no card listed while Tau Omega was open, and
  the mount appeared immediately once the owner closed Tau Omega. Not yet root-caused on this side —
  candidates worth checking: a held file handle from a background watcher/poll (e.g. a
  `fs::read_dir`/tokio task left scanning the card path), a Tauri asset-protocol scope keeping the
  volume referenced, or a lock file Tau Omega writes and never releases while the window is open. A
  fix should let the OS unmount/eject the card cleanly whenever Tau Omega has no operation actually
  in flight against it, not just when the app is fully quit. Needs someone to reproduce with Tau
  Omega open + idle, then narrow with `lsof`/Activity Monitor's "Open Files" against the mounted
  volume path before touching any code.
- **Fixed 2026-09-26:** playlist export now has a native save-dialog picker (filtered to `.m3u`); the
  rename/create/import destination fields gained a `<datalist>` of the media root's own already-known
  `.m3u` filenames (from the existing scan result, no new backend command) so an existing file can be
  picked instead of retyped, while free typing for a new name still works. See "Playlist path pickers
  and page extraction" below.
- **Fixed 2026-09-26:** Cards, Sync and Compare Cores are now extracted components (`CardsView.svelte`,
  `SyncView.svelte`, `CompareView.svelte`), matching the `BackupView`/`PackageView`/etc. pattern —
  `App.svelte`'s own template shrank from ~110 to ~25 lines across these three pages. See "Playlist
  path pickers and page extraction" below. *Integration note 2026-10-02: this extraction was not
  carried onto the Library workbench base; Cards and Compare are inline in `App.svelte` there and the
  Sync page no longer exists.*
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
  browser against the real manifest shape.
- **Fixed 2026-09-23:** removing an installed core is now implemented. New
  `tau_core::remove::{plan_remove, execute_remove, RemovePlan, RemoveReport}`, the same plan ->
  review -> confirm -> execute shape as `package`/`playlist`: `plan_remove` takes an already-inspected
  `Card` (so it always sees every other installed core) and a core id, always includes `Cores/<id>`
  and that core's own `Assets/<platform>/<id>` subfolder, and only additionally includes the
  platform-wide shared files (`Assets/<platform>/common`, `Platforms/<platform>.json`,
  `Platforms/_images/<platform>.bin`) when no *other* installed core still declares that platform.
  When the whole `Assets/<platform>` folder would end up holding only this core's own subfolder plus
  `common`, the plan removes that one folder instead of leaving an empty directory behind. New
  `plan_remove_core`/`execute_remove_core` Tauri commands (re-inspecting the card each time, matching
  `execute_package_install`'s own re-plan-before-execute pattern) and a "Remove an installed core"
  section on the Packages page: list the card's installed cores, pick one, review exactly which paths
  would be deleted and why (shared or not), confirm. 5 new engine tests against the real
  `alfatreze.TAU_0.4.0_2026-09-22.zip` fixture, including one that installs it twice under two core
  ids sharing `Assets/tau` (the same two-Tau-cores-on-one-card shape tau-alpha's own audit trail
  records on real hardware) to verify the shared files survive removing one of them.
- **Fixed 2026-09-23:** the full TAUD1 QR report is now decoded. New `tau_core::taud` (byte-exact
  port of `tau-alpha/tools/decode_tau_suite.py`'s `parse_record`/`from_text`): `parse_text`/
  `parse_record` decode the `{tag u8, length u8, value}` TLV body (build identity, SDRAM/PSRAM cycle
  histograms, the `CT_AUD` playback window, Decode Profile Sweep entries, and the per-test PASS/FAIL/
  SKIPPED/N/A list with the CT_BLT busy-permille special case) and validate the CRC32 trailer;
  `read_qr_text`/`read_qr_report` decode a screenshot PNG (`png` crate) to greyscale, locate the QR
  grid (`rqrr`, `default-features = false` so it never pulls in a second image crate on top of `png`)
  and parse its text. Added three dependencies (`base64`, `png`, `rqrr`) after measuring the real
  transitive cost (~19 new crates, no C/FFI) in a scratch crate and confirming with the owner via
  `AskUserQuestion`, per `DECISIONS.md`'s `fs4`/`zip` precedent — see `DEPENDENCIES.md`. New
  `read_qr_report` Tauri command (returns `None`, not an error, when an image simply has no QR code)
  and a "Full Check report (QR screenshot)" card on the Settings page: choose a screenshot, decode,
  see every test plus build/SDRAM/PSRAM info — not just the 4-word persisted summary
  `CheckSummary` already showed. 7 new engine tests against the real screenshots in
  `testdata/screenshots/`: a FULL-profile pass (13 tests, all decoded fields cross-checked), a real
  SDRAM-speed FAILURE (verdict, value 506, matching B-060), a USER CHECK short run, a STANDARD run
  with stress tests, and the non-Check now-playing screenshot correctly reported as "no QR found"
  rather than a false decode.
- **Fixed 2026-09-24:** screenshot discovery is now built. New `tau_core::screenshots::{ScreenshotEntry,
  list_screenshots}`: lists every file under a card's `Memories/Screenshots/` (the Pocket's own
  screenshot save location, confirmed by inspecting a real mounted card — `tau-alpha/docs/
  AUDIT_TRAIL.md` B-133), newest first, parsing the Pocket's own `YYYYMMDD_HHMMSS.png` filenames into
  a plain timestamp while still listing anything that doesn't match that shape rather than dropping
  it, and skipping Finder's `._*` junk and non-PNG files. Returns an empty list, not an error, when
  the folder doesn't exist (a normal card that has never taken a screenshot). New `list_screenshots`
  Tauri command and a "Browse screenshots on a card" section on the Settings page, above the existing
  QR-decode card: choose the card root, list its screenshots with timestamp/filename/size, click one
  to decode it directly — no more hunting for a file path by hand. 3 new engine tests, two against the
  real files in `testdata/screenshots/` (newest-first ordering, timestamp parsing, junk-file
  skipping) and one for a file that doesn't match the naming convention.

## Deferred because validation/fixtures are required

- **Done 2026-09-23:** real SD-card write/eject validation — see "Real hardware write validation" above.
- **Done 2026-09-23:** removing an installed core — see "Known issues and incomplete wiring" above.
- **Done 2026-09-23:** the persisted Check-report summary (persist ids 20-23) is decoded —
  `tau_core::diag::{decode_check_summary, read_check_summary}`, a byte-exact port of
  `tau-alpha/tools/decode_tau_suite.py`'s `unpack_words`, tested against three real hardware-captured
  `interact_persist.json` fixtures in `testdata/interact_persist/` (all passed, some failed, and the
  legacy-overload rejection case). See "Known issues and incomplete wiring" below.
- **Done 2026-09-23:** the full TAUD1 QR report is decoded — `tau_core::taud`, see "Known issues and
  incomplete wiring" above.
- **Done 2026-09-24:** screenshot *discovery* — `tau_core::screenshots::list_screenshots`, see "Known
  issues and incomplete wiring" above.
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
6. ~~Add fixture-based diagnostics, screenshots, logs, and core package workflows when their source
   fixtures are provided.~~ **Done 2026-09-23/24** — see "Known issues and incomplete wiring" above.
   Diagnostics (Check summary and the full TAUD1 QR report), core package install/update, core
   removal, real QR/screenshot fixtures, and screenshot discovery are all in place. Log discovery
   (finding e.g. host-side journals or a firmware log format, if one exists beyond the journal this
   project already writes itself) is not scoped further — nothing has asked for it yet.
7. ~~Validate write operations on a designated test card only after review.~~ **Done 2026-09-23** —
   see "Real hardware write validation" below.

## Real hardware write validation (2026-09-23)

Ran all three write paths against the actual mounted Pocket card (`/Volumes/Pock`, the owner's real
card — every other Analogue core and their real Tau install were present throughout), staged
low-risk to high-risk, each stopped for review before the next:

1. **Sync** (`tau-cli plan`/`sync`): copied 3 real MP3s (from `tau-alpha/work/test-music/`) into a
   brand-new `Assets/tauomegasync/common`. Verified: 3 new/0 unchanged, index built, `tau-cli scan`
   read back 3 tracks.
2. **Package install** (`tau_core::package`): repackaged the real `testdata/packages/
   alfatreze.TAU_0.4.0_2026-09-22.zip` under an entirely separate platform/core namespace
   (`tauomegapkg`/`alfatreze.TAU_OMEGA_PKG` — zero overlap with the real `tau` platform or the
   card's real `alfatreze.TAU`/`TAU_DIAGNOSTIC` cores) via a throwaway scratch binary linking
   `tau-core` directly (no CLI subcommand exists for package/remove yet). `inspect`/`plan_install`
   confirmed all 15 entries `OnlyLeft` before installing; installed; re-`plan_install` showed all 15
   `Identical`.
3. **Core remove** (`tau_core::remove`): removed the just-installed `alfatreze.TAU_OMEGA_PKG`
   (solo-core, non-shared-platform case). Plan correctly listed only the 5 `tauomegapkg`-namespaced
   paths; execute reported 28 files / 2,315,518 bytes removed; verified gone.

**A real, harmless edge case found only by testing on an actual mounted volume:** Finder's
`._*` AppleDouble junk files under `Assets/tauomegapkg/` meant `plan_remove`'s whole-folder-collapse
optimization (removing all of `Assets/<platform>` in one path when it holds only the core's own
folder plus `common`) didn't trigger — the junk files aren't recognized entries, so it correctly fell
back to the two safe per-subfolder deletes instead, leaving those two dotfiles behind rather than
guessing. Not a bug: the fallback is exactly the "only remove what's recognized" safety property
working as designed; cleaned up by hand this time since it was scratch data anyway.

Before and after every stage, the real `alfatreze.TAU`/`alfatreze.TAU_DIAGNOSTIC` core files, the
shared `Assets/tau/common/tau.rom`, and `Platforms/tau.json`/`_images/tau.bin` were SHA-256-verified
unchanged. All scratch namespaces (`tauomegasync`, `tauomegapkg`) were removed after validation and
the card ejected cleanly (`diskutil eject`). See the sibling `tau-alpha` repo's `docs/AUDIT_TRAIL.md`
for the paired log entry.

## UX/UI review (2026-09-24)

Owner asked for a product-design review focused on simple flows, instant feedback, visible
progress, polished UI, and rich content over raw inputs/lists. No installed skill matched "product
design," so this was a direct review — done by actually running the app in a browser (real DOM/
accessibility-tree inspection, not just reading markup), which surfaced two real, verified defects
rather than only opinions:

1. **Every non-Cards/Sync/Compare page kept the Compare page mounted and hidden behind it.**
   `App.svelte`'s router rendered Cards/Sync/(trailing-`{:else}`)Compare inside `<main>`, then
   separately mounted Playlists/Backup/Packages/Problems/Library/Jobs/Settings as
   `position:fixed` overlays *after* `</main>` closed — the `{:else}` fired for every page that
   wasn't Cards/Sync, not just Compare, so Compare's form fields stayed mounted and focusable
   behind whichever page was actually showing. Confirmed via the accessibility tree: two full sets
   of interactive fields present at once.
2. **Content could render past the viewport's right edge on every fixed-overlay page.** Confirmed
   by measuring the DOM directly: Backup's own lede paragraph rendered 182px past a 1024px-wide
   window. The overlay's child `.page` never resolved its `width:100%` against the fixed
   container correctly.

**Fixed:** one exclusive `if`/`else if` chain for all ten page values, all as real children of
`<main>`'s grid content column instead of the overlay hack; `.jobs-panel` (and the "jobs-panel"
class on the six view components that had it) removed; `<aside>` is now `position:sticky` so the
sidebar stays in view as content scrolls, instead of relying on the overlay to cover it. Also fixed
a header-wrapping bug found in the same pass (long lede text pushed a header's button off the right
edge instead of wrapping onto a second line). Verified live: all ten pages now render exactly one
`.page` element each, zero hidden duplicate content, zero horizontal overflow at 1024px width.

**Built:** a real screenshot gallery replacing the plain filename-list-with-a-Decode-button. New
`read_image_data_url` Tauri command (base64-encodes one image file as a `data:` URL — deliberately
not Tauri's asset protocol, which would need broadening filesystem access via a capability/scope
change; see `DEPENDENCIES.md`). The Settings screen's "Screenshots & Check reports" card is now a
master-detail layout: a scrollable row list on the left (unchanged, cheap — no eager thumbnail
loading, since a card can have 100+ screenshots), and a detail panel on the right that shows the
actual selected image alongside its decoded TAUD1 report (or a plain "no QR code in this image"
message) the moment a row is clicked. The two previously separate cards ("Browse screenshots" and
"Full Check report") are now one flow; the manual path-entry field is kept as a collapsed "or
decode a screenshot from elsewhere…" fallback rather than removed. Verified end to end in a browser
against a mocked Tauri backend (`window.__TAURI_INTERNALS__.invoke`) covering the loading, decoded,
and no-QR-found states.

**Not done, deliberately scoped as future work, not started without further direction:** drag-and-drop
onto the Cards screen, a shared toast/progress system wiring the engine's existing `ProgressObserver`
plumbing into visible progress bars everywhere (today only Sync/Library scan use it), cover-art
thumbnails in the core/library lists, and sidebar grouping/icons. See the conversation this session
for the full write-up. ("Recent cards" itself is now done — see below.)

## Running the real app for the first time, and Cards-page fixes (2026-09-24)

Every fix up to this point had only been verified via `npm run dev` in a plain browser (Tauri IPC
mocked or absent). Running the actual `cargo tauri dev` desktop app for the first time surfaced two
more real defects, plus three UX gaps the owner found by using it:

- **`cargo tauri dev` didn't work at all.** `tauri.conf.json`'s `beforeDevCommand`/
  `beforeBuildCommand` ran `npm run dev`/`npm run build` from the repo root, but `package.json` lives
  in `ui/` — every attempt failed with `ENOENT`. Fixed with the object form (`{ "script": ...,
  "cwd": "../ui" }`) both hooks support.
- **The app had no `capabilities/` file at all**, so Tauri v2's default permission set blocked
  `event.listen` the moment the app launched — the `tau://progress` event Sync/Library scan progress
  relies on was silently broken in every real build anyone had made. Added
  `src-tauri/capabilities/default.json` granting `core:event:default` and `dialog:default`. This
  app's own commands (sync/package/remove/taud/screenshots/etc.) need no grant — only Tauri's own
  plugin/core APIs are capability-gated.
- **Choosing a card required a separate "Inspect" click.** Picking a folder now inspects it
  immediately; the manual path field + Inspect button still exist for retyping a path.
- **The core list showed every core on the card**, unfiltered — dozens of unrelated Analogue cores
  (Amiga, NES, GB, …) on a real card, not just Tau ones. Now defaults to Tau cores only (matched by
  id/platform containing "tau"), with a "Show all cores" checkbox for everything else, and an empty
  state explaining why nothing showed.
- **The "View" button on each core did nothing** — no click handler at all. It now jumps to the
  Library screen scoped to that core's actual media root (`Assets/<platform>/common`) and scans it
  immediately.

**Known/mounted-card auto-open**, per the owner's explicit ask ("always open known cards by default
as well as checking cards or mounted analogue pockets on load"): new `get_recent_cards`/
`record_recent_card`/`list_mounted_cards` Tauri commands. Recent cards persist the same way
`reports_dir` already does (one small text file in the app's config directory, most-recent-first,
capped at 8); every successful `inspect_card` call records itself. `list_mounted_cards` checks
`/Volumes/*` (macOS only, matching this app's current bundle target — returns empty on other OSes
rather than guessing at unverified mount conventions) for the same `Cores`+`Assets` shape
`inspect_card` itself checks, done as a cheap directory check rather than a full inspection. On
launch, the app now auto-opens a card without the user doing anything: a currently-mounted Pocket
takes priority over a merely remembered path (since that's almost certainly what the user wants to
see), falling back to the most recent card otherwise. A "Known cards" quick-open row appears on the
Cards page, badged "Mounted"/"Recent", so switching cards is a single click instead of retyping or
re-browsing a path. (This section's own wording and the manual path field it originally described
were superseded the same day — see the redesign below.)

## Cards-screen redesign: player cores, "Set as player", detail panel, help panel (2026-09-24)

Owner feedback after using the redesigned app for real, addressed as one pass:

- **The manual "type a path directly" panel is gone.** It only ever duplicated what the native
  folder picker already does (macOS's own dialog supports typing/pasting a path via Cmd+Shift+G),
  so nothing was actually lost. Replaced with a small "?" help button, fixed to the top-right corner
  across every page, opening a side panel with the same instructions the removed panel's prose gave
  plus a summary of the player-core/plan-review-confirm model below.
- **A real, general signal for "is this a media player core."** New `Core::platform_category`
  (`tau_core`): reads `Platforms/<platform>.json`'s own `platform.category` field (e.g. `"Media
  Players"` — the exact field the real shipped `tau.json` declares), the actual APF metadata for
  this, rather than string-matching "tau" in a core's id. Falls back to the old id/platform
  substring check only when the category is missing entirely (an older or malformed
  `Platforms/*.json`), so a card lacking that file doesn't regress. 2 new tests against the real
  shipped shape and the missing-file case, both through `inspect_card` end to end, not just the
  private helper. `CoreView` now also carries `shortname` (previously read by the engine but dropped
  before reaching the UI) for a core's human name, separate from its dev-prefixed id.
- **Two-tier core display.** Cores whose platform category is "Media Players" (or that the user has
  manually promoted) render as large cards — art placeholder, human name, developer name + a generic
  glyph, track count, a "Library ready"/"Legacy core" chip, and an (i) button opening a detail side
  panel with every field `Core` carries. Clicking the card itself opens that core's library directly.
  Every other core on the card is collapsed behind a "Show N other cores" checkbox, in the existing
  small list format, each row offering **Set as player** — the "even if it's just the media copying"
  exception for a media player Tau Omega hasn't specifically recognised (e.g. a real card's own
  `HarpMudd.Mp3Player`, correctly auto-detected as a player by its platform category in testing,
  demonstrating the mechanism already generalises beyond Tau). New `get_manual_players`/
  `set_manual_player` commands persist the override list the same one-file-in-config-dir way
  recent cards do.
- **A real reactivity bug found and fixed while verifying this live**: `isPlayerCore` originally
  closed over the `manualPlayers` array rather than taking it as a parameter, so Svelte's `$:`
  dependency tracking (which only sees variables referenced directly inside the reactive statement,
  not inside an arbitrary closure) never re-ran `playerCores`/`otherCores` when a manual override
  changed — "Set as player" silently did nothing until an unrelated re-render. Fixed by passing
  `manualPlayers` in explicitly; verified live (via a mocked Tauri backend in a browser) that
  clicking "Set as player" moves a core into the grid immediately, no reload needed.
- **Card-ejection detection**: the manual "Inspect" button is gone from the main flow (auto-inspect
  on choose already covered opening; the header keeps a small, subtle refresh icon next to "Player
  cores on this card" instead of a prominent button). A `window.addEventListener('focus', ...)`
  handler re-checks `list_mounted_cards` whenever the app regains focus — exactly the "possibly only
  a refresh, in case I eject the card" case, since switching back to the app *is* that refresh
  moment — and shows a small inline banner ("This card is no longer connected") with a Reconnect
  action when a currently-open card (one under `/Volumes/`) is no longer in that list. A plain
  staging folder outside `/Volumes/` never triggers this (nothing to "eject").
- **Layout: top-aligned, not centered.** `.page{margin:auto}` centers a grid item both horizontally
  *and* vertically in CSS Grid when the row is taller than its content — auto margins override
  `align-items`, which is why the earlier `align-items:start` fix wasn't enough on its own. Changed
  to `margin:0 auto` (horizontal centering only).
- **The device/card icon is a deliberate placeholder, not the real thing** — no Analogue Pocket icon
  or logo asset was available to embed, and reproducing Analogue's actual trademarked logo without a
  legitimate source isn't something to fabricate. Used for the sidebar widget and known-card tiles
  (both represent a *card*, not a specific core, so there's no per-card artwork to decode anyway).
  **Per-core artwork is real, decoded from the card itself** — see below.

## Sidebar active-card/-core switcher (2026-09-24)

Owner: "I always want to know which card and core I am working with... visible on the sidebar and
easily swappable... considering the Cards UI does the heavy lifting when needed." A compact widget
between the brand and the nav list, always visible on every page: the current card's volume name
and the current core's name (`activeCore`, a new piece of app-wide state), with the same device-icon
glyph the known-card tiles use. Clicking it opens a small dropdown, not a full page:

- **Switch card** — every known card (mounted or recent), same list the Cards page's own tiles show,
  with a small dot marking whichever one is actually mounted right now.
- **Switch core** — every player core on the *currently open* card; picking one sets `activeCore` and
  opens that core's library directly (reuses `openCoreLibrary`, the same action a Cards-page card
  click already performs).
- **"Manage cards & cores →"** at the bottom jumps to the full Cards page — the heavy-lifting surface
  (Set as player, the detail panel, Show other cores) stays there rather than being duplicated in a
  small dropdown.

`activeCore` defaults to the first player core once a card's cores (and any manual-player overrides)
are known, and is preserved by id across a same-card refresh (`openFolder`'s own logic) rather than
being silently reset to the default every time. Verified live against a mocked Tauri backend: the
widget's empty state, the populated state after opening a card, the dropdown's card/core lists, and
switching core (sidebar updates, navigates to Library, pre-fills the right media-root path) all
confirmed working.

## Real per-core artwork: icon.bin and the platform banner (2026-09-24)

Owner noticed the core artwork/dev icon weren't real and correctly guessed it needed a specialised
decoder. New `tau_core::icon` module, two decoders sharing one implementation:

- `decode_icon_bin`: `Cores/<id>/icon.bin`, 36x36 monochrome, 16 bits per pixel, stored rotated 90
  degrees CCW — documented in the sibling `tau-alpha` repo's `analogue-pocket-dev` skill
  (`references/sd-packaging-assets.md`), sourced from Analogue's own SD-packaging notes. That
  description left two things ambiguous (which byte of the 16-bit pixel holds the brightness, and
  which rotation direction undoes the stored one), resolved empirically before trusting them:
  decoded the real shipped `alfatreze.TAU` icon four ways (no rotation, CW, CCW, 180°) and compared
  each pixel-for-pixel against `tau-alpha/assets/branding/author-icon.png` — the same emblem drawn
  by hand — using a throwaway Python prototype (render as PGM, convert with macOS's `sips`, view with
  `Read`). The 90°-clockwise render was an exact silhouette match; brightness turned out to be the
  **first** byte of each pixel pair (a plain byte offset, not a 16-bit read at all — the "upper byte"
  language in the source doc was about big-endian byte order, not a machine word's usual low/high
  split, confirmed by checking the raw byte range: values only ever spanned 0-255 as little-endian
  16-bit words, which would make no sense for a 0xFF00-is-full-brightness format).
- `decode_platform_image`: **a second, distinct asset** — `Platforms/_images/<platform>.bin`, 521x165,
  the *real* per-platform artwork, shared by every core on that platform (not a second copy of the
  small icon). Found only because the owner's follow-up ("the card is still showing the developer
  icon instead of the main core image") made clear the icon and the "main artwork" were never meant
  to be the same picture. Confirmed the same encoding applies (rendered the real shipped `tau.bin`
  banner and got a clean "TAUα" wordmark on graph paper, not noise) — but doing that surfaced a real
  bug the square icon case couldn't: the shared rotation math used `height` where it needed the
  actual stored buffer's row length, which only happens to equal `height` when width == height. Fixed
  and re-verified against both the icon and the banner before trusting either.

Both decode to a grayscale+alpha PNG (opaque white where "on", transparent elsewhere, so it composites
over any card background colour) using the `png` crate already in `tau-core`'s dependency list — no
new dependency needed. New `read_core_icon` (`card + core_id`) and `read_platform_image` (`card +
platform`) Tauri commands, both `Option`-returning (`None`, not an error, when the file doesn't
exist). 4 engine tests: each decoder against its own real shipped file (valid PNG, exact promised
dimensions/colour type, not blank) and each rejecting a wrong-sized buffer rather than misreading it.

Wired correctly this time: the big player-card's main art square shows the **platform banner**
(`platformImages`, cached by platform id since it's shared across cores, e.g. TAU and TAU
Diagnostic fetch it once between them); a small copy of the **core icon** (`coreIcons`, cached by
core id) sits next to the developer name, replacing the generic bracket glyph there, and in the small
"Show other cores" list. A core/platform with no file keeps the existing monogram/bracket-glyph
fallback rather than showing nothing. Confirmed working against a real card in conversation (the real
`HarpMudd.Mp3Player` icon rendered correctly next to its dev name, not just Tau's own).

## Cover images: `TIM1` decode and encode (2026-09-26)

Reviewed tau-alpha's latest work (image-format study B-284 and the meter-module design B-274/B-294)
against this project's plan and wrote up a revised cross-project sequencing in
`docs/IMPLEMENTATION_PLAN.md` — cover images are ready to build now (a final format decision plus a
real tool already producing real files); meter presets are fully designed on tau-alpha's side but not
real yet (only mid-M0 of its own build order) and are tracked, not started, in
`docs/FIRMWARE_SYNC.md`'s new "Watched interfaces" section, to avoid repeating the two fixture-vs-
reality bugs already recorded there.

Built the cover-image half: new `tau_core::image` module. `decode_tim1` reads every payload shape
real tooling can produce (`rgb565`; `palette` at 8/6/4 bits per pixel, matching `tau_image.py`'s own
bit-packing bit for bit) into plain RGB8. `encode_cover_pal256`/`encode_pal256_bytes` produce the
newly decided default (`IMAGE_FORMATS.md` D-I01/D-I02, 2026-09-26: palette-256, 128 px on the long
side, proportional scale, no crop or letterbox) from a real JPEG or PNG source cover — own
from-scratch median-cut quantizer plus Floyd-Steinberg dithering and a plain Lanczos3 resize, since
the container format is the only shared contract, not the Python tool's exact quantizer output.
Three new dependencies, each measured before adding (`docs/DEPENDENCIES.md`): `zune-jpeg` (one
transitive crate, pure Rust) for JPEG source decode, reusing the existing `png` dependency for PNG
source decode; `resize` (`default-features = false` to drop its default `rayon` thread pool, not
needed for a single small cover) plus `rgb` (its own pixel-type dependency, needed directly to name
`RGB8` in this crate) for the resample step.

Verified against a real `.timg` file copied from a real album on tau-alpha's own card backup
(`testdata/images/README.md`, hash-verified) — decoded pixel values cross-checked against
`tau_image.py`'s own decoder, not invented from the format description. The encoder is round-tripped
through the same decoder against a real source JPEG (`testdata/images/cover455.jpg`, one of
`IMAGE_FORMATS.md`'s own comparison-table covers). 6 new tests, `cargo test --workspace` at 71 passing
(up from 65), `cargo clippy --all-targets` clean for this module.

**Honestly unfinished at the time:** no UI wiring, no Sync-plan option to write the sidecar, no
thumbnail shown anywhere. — carried over from tau-alpha's own status, not something this side
controls — *(superseded 2026-09-27: tau-alpha v0.5.0 ships the firmware reader, cover data slot 7; the
fast cover is hardware-confirmed)*. Originally: no firmware reader existed and no slot was assigned.

## Cover-sidecar writing wired into Sync (2026-09-26)

Closed the first half of the "Next" item above: `art_sidecar_pal256` is a new `PlanOptions` field
(default off, same shape as the existing `embed_covers`). `plan`/`plan_with_layout` detect one
`ArtSidecarItem` per **album folder** (not per track — a shared cover only needs encoding once,
matching `tools/sync_media.py --art-variants`'s own convention) whenever a discovered cover exists;
the destination path and the cover's SHA-256 are folded into the plan token, so a changed cover
invalidates a stale plan the same way an embedded-cover change already does. `execute`/
`execute_with_mirror` re-verify the cover hasn't changed since the plan, encode with
`image::encode_pal256_bytes`, write via the same durable-write helper the index uses, then read the
result back through `image::decode_tim1` before counting it — write-then-verify, the same discipline
every other write path here follows. New `SyncReport::art_sidecars_written` field.

Wired end to end: `tau-cli` gained `--art-sidecar`; the Tauri `plan_sync`/`execute_sync` commands and
`SyncPlanView` carry the count through; the Sync screen has a second checkbox next to "Add folder
cover art", with an inline note (originally "no firmware reader exists yet"; updated 2026-09-27 to say v0.5.0+ cores show it), and the plan-review line shows "· N art sidecar(s)" when non-zero. Verified
live in a browser against a mocked Tauri backend (checkbox toggle changes the mocked `plan_sync`
response, plan card renders the count and correct singular/plural). New engine test
(`art_sidecar_is_planned_once_per_album_and_written_verifiably`, against a real source JPEG) confirms
one sidecar for a two-track album, not two, and that the written file decodes back to 128×128.
`cargo test --workspace`: 72 passing. `npm run check`: 0 errors. `cargo clippy --all-targets`: clean
for every file this touched (collapsed three new nested-`if`s into `if`-let-chains, matching the
2024-edition style already elsewhere in the crate).

**Still open at the time:** no thumbnail is decoded and shown anywhere in the UI yet (Library/Cards).

## Cover preview: decode-and-show in the Sync plan review (2026-09-26)

*Integration note 2026-10-02: the Sync page this preview lived on was retired by the Library workbench. The engine pieces (`image::*`, `preview_art_sidecar`, `SyncPlanView.art_sidecar_previews`) are intact; the preview button has no surface until the Library review sheet gets one.*

Closed the "decode and show" half of the original plan. Rather than a Library/Cards thumbnail grid
(the Library screen is a flat, virtualised 12,000+-row track table with no album grouping at all —
not a natural fit, and a separate, already-tracked future item), the real point of contact is the
Sync plan review itself, right where the user is deciding whether to write these files: each planned
`art_sidecar_previews` entry gets a lazy "Preview" button (no eager thumbnail loading for a large
batch, same precedent as the Settings screenshot gallery) that decodes the *exact* pal256 quantizer
output the real write would produce and shows it inline.

New `tau_core::image::{rgb8_to_png, decode_tim1_to_png, preview_pal256_png}` — the same "decode
straight to PNG bytes" convention `icon::decode_icon_bin`/`icon::decode_platform_image` already
established, so a UI that can't render `TIM1` doesn't need to. New Tauri command
`preview_art_sidecar`; `SyncPlanView` gained `art_sidecar_previews` (folder name + cover path per
album, not the full per-track item list). Verified live in a browser against a mocked backend: two
mock albums, clicking "Preview" on one renders its thumbnail while the other stays lazy.

**A real bug found and fixed by this step's own tests, before it shipped further:**
`encode_pal256_bytes` was not deterministic — two calls on the exact same input byte-for-byte
produced two different (each individually valid) `TIM1` files. Root cause: `quantize_pal256`'s
median-cut palette build iterated a `HashMap` directly, whose iteration order depends on a
per-instance random seed (confirmed: Rust's default hasher reseeds per `HashMap::new()` call on the
same thread, not just once per process), not only on its contents. Fixed with one `sort_unstable()`
on the collected histogram before quantizing, making the rest of the pipeline a pure function of the
pixel data. Caught by a new test that specifically encodes the same real cover twice and compares
byte-for-byte (`encode_pal256_bytes_is_deterministic_across_repeated_calls`) — added because the
preview-matches-a-direct-encode test kept failing intermittently until the real cause was traced,
not assumed.

`cargo test --workspace`: 75 passing (up from 72). `npm run check`: 0 errors.
`cargo clippy --all-targets`: clean for every file this touched. (Also ran clippy on `src-tauri`
directly for the first time this session — found 2 pre-existing `too_many_arguments` findings on
`execute_sync`/`execute_core_move`, unrelated to this change: both already exceeded the default
7-argument threshold before today, `src-tauri` was simply never part of the workspace's own
documented "clean" claim. Not fixed here, flagged for whoever picks up that cleanup.)

## Playlist path pickers and page extraction (2026-09-26)

*Integration note 2026-10-02: the pickers were ported; the page extraction (`CardsView`/`SyncView`/`CompareView`) was deliberately not. See the integration section.*

Closed both remaining "Known issues" wiring gaps in one pass.

**Playlist pickers.** `output` (the export destination) is a real host filesystem path, so it got a
native save-dialog `Choose` button (`@tauri-apps/plugin-dialog`'s `save()`, filtered to `.m3u`,
mirroring `chooseFolder`'s existing cancel-returns-null precedent). `renameNewFile`/`createFile`/
`importDestFile` are filenames *within* the already-chosen media root, not arbitrary host paths — a
save dialog rooted anywhere would let a mistaken pick land outside it — so those three instead got a
shared `<datalist>` populated from the folder's own already-scanned `result.playlists`, no new backend
command needed (the cheaper path the scoping pass identified: the data was already there). Free typing
for a brand-new name still works alongside the suggestions.

**Page extraction.** Cards, Sync and Compare Cores were the last three inline pages in `App.svelte`,
per its own "Known issues" note. Extracted as `CardsView.svelte`, `SyncView.svelte`,
`CompareView.svelte`, following the exact `BackupView.svelte` pattern (props in, callback props out,
no internal Tauri calls) already used by seven other views. Behaviour-neutral by construction — same
markup, same bindings, same handlers, just moved. One real thing found and fixed doing this: Compare's
locally-scoped `<style>` rules (`.comparison`, `.copy-plan`, `.move-review`, …) had to move to
`CompareView.svelte` too — Svelte's per-component CSS scoping never applies a `<style>` block's rules
to another component's markup (the exact class of mistake `App.svelte`'s own comment already warns
about for `.jobs-panel`/`.settings-card`/etc.), caught immediately by `svelte-check`'s
`css_unused_selector` warnings rather than shipped silently unstyled.

**A real, pre-existing UI redundancy found while verifying this live, not fixed (out of scope for a
behaviour-neutral extraction):** reviewing a core copy shows *two* confirmation surfaces at once —
an inline `.copy-plan` card inside the Compare page itself, and a separate full-screen `.move-review`
modal layered on top of it (both gated on the same `coreCopyPlan`). The modal visually covers the
inline card, so this isn't a live bug (nothing is double-clickable), but the inline card's own
"Confirm and copy" button is dead markup the instant a plan exists, since the modal always appears at
the same time. Confirmed by screenshot during live verification. Worth deleting the inline `.copy-plan`
card in a future pass — the modal is the one real confirmation flow (it's the only one offering the
move option) — but that's a product decision, not a mechanical refactor, so left as-is here.

Verified live in a browser against a mocked Tauri backend: Cards (player-core grid, other-cores list,
detail panel), Sync (plan review including the art-sidecar preview built two commits ago), and Compare
(comparison result, the copy-plan card, and the move-review modal) all render and behave identically to
before extraction. Playlist datalist suggestions confirmed populated from real scanned filenames via a
direct DOM query, not just visual inspection. `cargo test --workspace`: 75 passing (unchanged, this was
a frontend-only pass). `npm run check`: 0 errors, 0 warnings.

## Meters: UI-structure scaffold, synthetic schema (2026-09-26)

*Integration note 2026-10-02: the Meters entry is labelled **Meter Lab** and sits in the main menu, below Playlists (it first went under Tools & settings, then moved out).*

Owner decision on how to start meter work given tau-alpha's instability (active hardware bug-hunting
that same day, M0 built but uncommitted on their side, no tagged release containing a real
`meters_schema.json`): **scaffold the UI structure and preview plumbing against a synthetic schema
now; swap in the real one once tau-alpha tags a release.** Zero dependency on their churn, zero rework
risk — this is pure frontend, no Tauri command, no engine code, nothing written anywhere.

New `ui/src/lib/meters/schema.ts`: `MeterSchema`/`MeterParam`/`MeterPreset` types mirroring
`docs/METER_MODULE_SPEC.md` section 4's real manifest shape field-for-field, so swapping in the real
`meters_schema.json` later is a data change, not a UI rewrite. Three synthetic meters (`winamp_bars`,
`winamp_scope`, `chladni`) — not arbitrary placeholders: their parameter shapes are drawn from
tau-alpha's own real, hardware-shipped `wviz_bars_cfg_t`/`wviz_scope_cfg_t` fields per that project's
audit trail, so the editor and preview exercise a realistic shape. Every placeholder is labelled as
such, loudly, in both the code comments and the on-screen UI copy — this is a scaffold, not a feature.

New `ui/src/lib/MetersView.svelte`: a meter list (cost-class chip per meter), a generated parameter
editor (range/checkbox/select per `param.type`, honouring `when` clauses to show/hide, e.g. "Peak fall
style"/"Peak hold" only when "Peak cap" is on), a preset dropdown that snaps to `CUSTOM` the instant any
value is hand-edited, and a Reset-to-template action. New `ui/src/lib/meters/MeterPreview.svelte`: a
`<canvas>` animation loop driven by synthetic (not real) audio-level data, dispatching by which
parameters a meter declares (`bands` → bar meter with attack/release easing and an optional peak dot;
`smooth`/`trail` → scrolling scope with fade; otherwise → a generic reactive placeholder) — explicitly
not tau-alpha's real preview stack (`METER_MODULE_SPEC.md` section 7's `tau_fb.js`/`tau_theme.js`/
`tau_audio.js`/`tau_ballistics.js`, which doesn't exist to vendor yet), captioned as such on screen, but
real enough that the editor and preview are genuinely live and interactive.

New "Meters" nav entry (labelled **Meter Lab** since 2026-10-02). Verified live in a browser: switching meters preserves each one's own
independent parameter state, toggling "Peak cap" off correctly hides its two dependent params and
removes the peak dot from the animating preview, and all three meters' distinct preview shapes (bars,
scope, rings) render and animate correctly. `npm run check`: 0 errors, 0 warnings.

## Firmware/app update feature: design, then connection detection + card icon (2026-09-26)

*Integration note 2026-10-02: the detection described here (`connection_kind`, `pocket`/`usb_storage`/`other`) was merged into the workbench's `device.rs` `detect_connection` (`direct_usb`/`card_reader`/`unknown`); the hardware-verified USB-descriptor match is kept, the duplicate command is gone. The icon swap is on the sidebar card only.*

Owner asked for a firmware upgrade feature (both a Tau Omega self-update check and browsing/
installing Tau Alpha core releases by channel/build), a transfer-safety warning for USB-connected
Pocket writes, and a connection-aware card icon — designed first with the `design-system` skill
(new pattern extending the existing dark-workbench system, not a new visual language) before any
code: [`docs/FIRMWARE_UPDATE_SPEC.md`](FIRMWARE_UPDATE_SPEC.md). Real research grounded the design
rather than guessing: Analogue's own cached developer docs give a hard ~700 KB/s–1 MB/s figure for
USB SD Access mode, and the Pocket's own on-device screen (read directly, with it connected) states
"<10MB suggested" — the spec's warning threshold. Scope and data-source decisions (two separate
update targets; GitHub releases API, a first network capability for this app) were confirmed with
the owner before writing anything.

**Built and hardware-verified this pass: USB connection detection + the card icon swap** — the
smallest, most self-contained piece, no network access, chosen deliberately as the starting point.
New `src-tauri` command `connection_kind(path)`: resolves a path's whole-disk BSD identifier via
`diskutil info`, and if its protocol is USB, walks `ioreg -l -w0`'s registry tree to the disk's
ancestor USB device and matches its `idVendor`/`USB Product Name` against the real Analogue Pocket
descriptor. That descriptor isn't published anywhere by Analogue — found empirically by reading
`ioreg` directly against the owner's actual mounted Pocket (idVendor `0x04D8`/Microchip Technology,
product name `"Analogue Pocket"`), with a real contrasting device (a CalDigit USB3 card reader, also
attached at that moment) confirming the match is discriminating, not just "any USB device."

**A real parsing bug was found and fixed by testing against the real device, not by inspection:**
the first implementation matched `ioreg` lines with `.trim()` then exact-string equality — but
`ioreg`'s own tree-drawing `|` characters aren't whitespace, so `.trim()` left a leading `|` on every
property line and the equality check silently matched nothing, on every single property, the entire
time. Caught immediately because the real-hardware test failed while the synthetic unit test (whose
fixture happened not to trigger the bug) passed — exactly the class of gap real-artifact testing
exists to catch. Fixed with `.contains`/`.split_once` instead of exact equality. Two `#[ignore]`d
regression tests lock in both real cases (`cargo test --features tau-core/serde -- --ignored`, needs
real hardware attached): the genuine Pocket detected correctly, and a real, differently-branded USB
storage device (a Raspberry Pi Pico in mass-storage mode) correctly *not* misdetected as one.

New `CardIcon.svelte` (Pocket glyph / a new generic SD-card glyph / the existing fallback for
unknown — still no Analogue logo, same trademark-avoidance constraint as before), wired into both
the sidebar active-card widget and the known-card tiles, fetched lazily and cached per path like
`coreIcons`/`platformImages` already are. Verified live in a browser against a mocked backend: two
known cards with different mocked connection kinds render visibly different icons.

`cargo test --workspace`: 75 passing (unchanged, this was `src-tauri`-only). `src-tauri`'s own test
suite (not previously run standalone this session): 3 passing including both hardware-gated tests.
`npm run check`: 0 errors, 0 warnings. `cargo clippy` clean for every file this touched (2
pre-existing, unrelated `too_many_arguments` findings in `src-tauri` noted, not fixed, same as
before).

**Not built yet:** sections 3a (Tau Omega self-update), 3b (the Firmware screen: per-card version
detection, channel/build picker, browsing GitHub releases), and 3c (the transfer-safety banner
itself, threaded through Sync/Backup/Package/Core-copy) — all still design-only in the spec, next in
line per the owner's own chosen build order.

## Library workbench, and the integration of two parallel sessions (2026-09-30 / 2026-10-02)

**Why this section exists.** Two sessions on two accounts worked from the same commit (`bd43af5`)
without seeing each other. One built the **Library workbench** (19 commits, 2026-09-30, branches
`claude/exciting-bell-emz8il` and `claude/jolly-meitner-jwdzwx`, identical). The other built the
Cards/Sync/Compare page extraction, the Meters scaffold, playlist pickers and the firmware-update
design with USB connection detection (3 commits, 2026-09-26, branch `omega-cards-meters-firmware`,
also on local `main` until merged). The Library workbench is the key work and is the **base**; the
other session's work was ported onto it as `integration/workbench-base`. Nothing was lost: every
original commit is still on `origin`.

**What the Library workbench is** (full design: [`LIBRARY_WORKBENCH_PLAN.md`](LIBRARY_WORKBENCH_PLAN.md);
build status table in its section 12): one card-centred screen replacing the old *Sync library* and
*Library* screens. The card's music on the right, a folder on this computer on the left, every add,
removal and tag/cover edit staged in a single **Pending changes** list, nothing touching the card until
**Start sync**, one review sheet (adds/updates/removals/edits, free space after, time estimate, the
slow-connection warning), live progress with measured speed and cancel, then a result and a **Sync
history** page. Engine: `tau-core` `workbench.rs` (album listing, selection planning, removal),
`changes.rs` (combined change sets), `tagedit.rs` (ID3v2.3/2.4 and FLAC tag and cover editing on the
card copy only, written to a temp name, read back, then renamed), `journal.rs` (change-set journal and
history), `cover.rs` (`extract_embedded_cover` for thumbnails). Shell: `src-tauri/src/device.rs`
(volume auto-detect, connection detection), workbench commands and preferences in `main.rs`.
UI: `Workbench.svelte`, `HistoryView.svelte`, `LibrarySettings.svelte`, `VirtualList.svelte`, a
dev-only Tauri mock (`dev-mock.ts`) and `ui/scripts/*.cjs` browser checks. Owner decisions D1
(tag/cover edits on the card copy only) and D2 (removal preference: back up, ask, or just remove;
default back up) are recorded in `SAFETY_RULES.md`. Review and usability material:
[`LIBRARY_UX_REVIEW.md`](LIBRARY_UX_REVIEW.md), [`usability/`](usability/) (**simulated** personas, not
real users, so treat their findings as a checklist, not evidence of demand).

**What was ported onto the workbench from the other session** (each its own commit):

| Ported | Notes |
|---|---|
| Backup/journal "outside the card" fix | Not from the other session: found while integrating. Four checks compared an unresolved path with the canonicalised card root, so on macOS (`/var` vs `/private/var`) a backup or journal could land inside the card. Shared helper `sync::backup_is_inside`; four `tau-core` tests that had been failing on macOS now pass. Linux CI had masked it. |
| Pocket confirmed by its real USB descriptor (macOS) | Merged into the workbench's `device.rs` (`usb_disk_is_pocket`: vendor `0x04D8`, product name "Analogue Pocket", found empirically 2026-09-26). The workbench's name-based guess stays as the fallback and as the Windows/Linux path; a name match alone no longer produces a "direct USB" verdict on macOS if the descriptor disagrees. The two `#[ignore]`d live-hardware tests moved to `device.rs`. |
| Card icon | `CardIcon.svelte` on the sidebar active card, driven by `detect_connection` (`direct_usb` → handheld, `card_reader` → SD card, `unknown` → handheld). |
| Playlist export "Choose" and name suggestions | Unchanged from the earlier session. |
| Meters scaffold | `MetersView.svelte` and `meters/`, now in the main menu, below Playlists. Still a synthetic schema, still waiting on a tau-alpha tagged release with a real `meters_schema.json`. |
| Opt-in cover sidecar | New checkbox in the Library review sheet, remembered per card. The workbench had hardcoded `art_sidecar_pal256: false`; the engine already honoured it. |

**Deliberately not ported:** the `CardsView`/`SyncView`/`CompareView` extraction. The workbench retired
the Sync and Recent-jobs pages and keeps Cards and Compare inline in `App.svelte`, so the menu stays as
the Library design has it. The `plan_sync`/`execute_sync` commands and `SyncPlanView` still exist in
`src-tauri` (and the cover-preview command `preview_art_sidecar`) but no page calls them now; the
cover preview in the Sync plan review therefore has no surface until the Library review sheet gains
one (not built).

**Known gaps and open items after integration**
- `connection_kind` (the earlier session's `pocket`/`usb_storage`/`other` command) was **not** carried
  over; `detect_connection` (`direct_usb`/`card_reader`/`unknown`) is the single detector.
- The transfer-safety warning (spec section 3c) is built for the Library review sheet only (10 MB
  threshold, same figure from the Pocket's own screen). Backup, Packages and Core-copy review cards
  have none.
- Tools & settings pages (Compare cores, Backup, Packages, Problems) keep their older wording.
- The workbench's own behaviour script (`ui/scripts/workbench-test.cjs`, documented as 122 checks)
  needs Playwright and was **not** run during integration; the integration was checked with
  `cargo test --workspace`, `npm run check` and a live click-through against the dev mock.
- Real hardware: the workbench's write paths (sync, removal, tag edits) and the new USB-descriptor path
  in `device.rs` have not been exercised on a real card since integration. `TEST_PLAN.md` item 5 lists
  the runs.
- `cargo fmt --check` reports pre-existing formatting drift; `cargo clippy` has the two known
  `too_many_arguments` findings in `src-tauri`.

## Correctness pass, cache experiment and safe eject (2026-10-02)

Found while integrating: a macOS hang report (a 71 s freeze) led to a performance and card-safety audit
(`PERFORMANCE_AUDIT.md`, three passes including a ledger red-team). Built so far, all on
`integration/workbench-base`, `cargo test` 117 in `tau-core` and 129 browser checks passing:
- **Freeze fixes:** covers read only the tag region (not whole files); every command except `cancel_job`
  runs off the window's main thread; one card write at a time (try-lock, a second is refused with a message).
- **Correctness:** cover-embedded copies are now actually verified (the old check could never fail); names
  differing only by case collide; execute refuses a run that cannot fit, counting whole clusters
  (`InsufficientSpace`, code 51); the index swap keeps `.tau-library.tdb.prev` and a confirmed sync recovers
  an interrupted swap; art sidecars are written temp, verify, rename; our own stale temp files are swept;
  album removal is two-phase (verify and back up everything in one pass, then delete) with a backup-space
  check.
- **No-cache verification and safe eject:** see `PERFORMANCE_AUDIT.md`, "Cache experiment and safe eject".
  `eject_card` and `readback_status` commands; the completion dialog asks for an eject.
- **Card space and sync bar** ([`CAPACITY_BAR_SPEC.md`](CAPACITY_BAR_SPEC.md), built): per-core and other-data breakdown, projected change before sync, a Details side panel. `card_breakdown` engine and command; 145 browser checks. Start sync now starts straight away unless there is a removal or the slow direct-USB warning to acknowledge (owner decision D3, `SAFETY_RULES.md`); covers are always embedded and the small cover file always written, and a cover that cannot be embedded warns instead of failing the sync. The sync progress now lives in the top bar (text area, progress bar, Cancel sync) with a circular progress per album, and a finished sync is a toast, not a dialog. The bottom Pending changes area is gone: the staged list lives in the Details panel as a plain list, Start sync and Clear all sit in the bar. Also fixed: UTF-16 ID3 tags read as a blank byte-order mark (266 of 341 albums in a real library listed with no title or artist, and the index written for them was blank); `decode_id3_text` now follows the Python reference, checked on all 5,202 real MP3s with zero differences. An index built before the fix needs a re-sync to refresh.
- **Ledger built** (`crates/tau-core/src/ledger.rs`, host side in `src-tauri/src/ledger_host.rs`): scan cache, copy provenance (skip an unchanged copy), canary re-hash, "Forget what the app remembers about this card" in the connection menu; Refresh tooltip shows files checked/read. Never influences deletion, overwrite or verification.
- **Not built:** the per-volume I/O governor, hash-while-copy for
  plain copies, the Library fit bar counting clusters, FAT-unsafe names (needs a decision with tau-alpha).
- **Hardware not yet exercised:** all of it. `TEST_PLAN.md` item 5 now lists the eject and read-back runs.

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


## Appearance scaffold (2026-10-02)
New **Appearance** page (nav, under Meter Lab) and engine `crates/tau-core/src/assets.rs`: edits up to 4 extra themes
(Dark + Light, 18 colour roles, background brightness), checks them with the firmware's own contrast rules (text on every
panel and on the background ramp behind all 19 device accents; Light accent on panel), shows each colour as RGB565-snapped,
opens an existing `tau-assets.bin` (every CRC verified) and saves one to a file the user picks, then reads it back.
- **Verified against a real artefact:** the writer's output is byte-identical to `tools/tau_assets.py pack` for the
  `sunset` example (fixtures in `crates/tau-core/testdata/assets/`, produced by that tool); contrast ratios match its report
  to 0.01; every byte flip and truncation of the file is refused. New error codes 52 (invalid theme) and 53 (bad file).
- **Not built:** installing to a card (needs the reviewed card-write flow and your approval), the `METR` meter-preset
  section (opening a file with one drops it on save, and the page says so), Figma import (no real `Tau Theme` variable
  collection exists to check an importer against), accent colour and live Pocket mode/theme selection (those are on the device).
- **Firmware status:** the container is documented as not yet read by a Pocket in its own doc; an installed file's `Info > THEME FILE` row is the proof to capture.

### Appearance install to a card (2026-10-02)
`tau_core::assets::{plan_install, execute_install}` and host commands `appearance_plan_install` / `appearance_install`; Appearance page button
"Install on <card>…" opens a review (themes, destination, what it replaces, which cores read it) and nothing is written until Install.
Order: confirmation token, refuse a backup inside the card, recover an interrupted install, re-plan and compare (card changed since review is refused),
back up the file it replaces (unless backups are off in Settings > Library), write `.tau-assets.bin.tmp`, read back through the cache-bypassing path and
require it to parse to the same bytes, swap in keeping the old file until the new one is in place (`sync::swap_in_file`, shared with the index swap),
read the result back. Holds the card write lock. 9 engine tests (install, replace with backup, unreadable existing file, wrong token / changed card /
unsafe backup, interrupted install, bad folder, no core asks for the file) and 4 browser checks.
- **Real-card read-only run** (ignored test `real_card_plan_is_read_only_and_finds_the_readers`, asserts the card is unchanged): found the three Tau cores
  (`tau`, `tau_diagnostic`, `tau_dev_67`) each already hold a 351-byte `tau-assets.bin` with one theme (SUNSET) **and a METR section**, which an install replaces
  without carrying METR over; the plan warns about exactly that. Cores that do not declare the slot (all non-Tau cores) are reported as ignoring it.
- **Not done:** an approved real write to a card and the Pocket reading it. Until then the format is as unproven on hardware as `THEME_FILE_FORMAT.md` says.
- **Limits:** writes `THEM` only (METR is dropped, with a warning); no "remove theme file" action yet.

**Real write, 2026-10-03 (owner-approved, `tau_dev_67` only).** Ran the real install path (ignored test `real_card_install_changes_only_the_theme_file`) against
`/Volumes/Pock/Assets/tau_dev_67/common`, theme OMEGA TEST, backup to `~/Downloads/tau-omega-card-backups/<plan id>/`. Result: the 351-byte SUNSET+METR file
(sha 9ac429e1...) was backed up byte-identical, the 144-byte file (sha 15c5d571...) was written and read back, no temp or `.prev` file left, and a before/after
walk of the whole card's `Assets` and `Cores` showed nothing else changed. **Real bug found:** macOS left a 4 KB AppleDouble `._tau-assets.bin` beside the file on
the exFAT card (SAFETY_RULES 7 says remove them; the install did not). Fixed (the install now removes `._` siblings of the three names it used), the stray was
removed from the card, and a second real run left nothing behind. The sync path has the same gap for the files it writes (`sync.rs` only skips `._*` when
reading); not fixed yet. **Still to do:** boot the Pocket and confirm `Info > THEME FILE` reads `1 LOADED` and the theme is listed (owner).

**Hardware confirmation, 2026-10-03 (owner-reported).** After the real write above, the Pocket loaded the theme: OMEGA TEST was available and `Info > THEME FILE` read
`1 LOADED`. So the `THEM` section Omega writes (byte-identical to tau-alpha's tool) is accepted by the firmware reader on real hardware, the first time this has been
shown. Not captured as an image; the installed file itself is `testdata`-worthy (144 bytes, sha 15c5d571...) and still sits on `tau_dev_67`.

**`._*` cleanup in sync (2026-10-03).** `sync::sweep_appledouble(media_root)` removes macOS AppleDouble stubs (regular files named `._*`, at most 64 KiB, so a
real file that happens to start with `._` and is larger is kept) from the Tau media root. It runs after `execute_with_mirror`, `execute_core_move` (source side
too) and `execute_changes_with` (the workbench), on success and on failure, and not when the confirmation token was refused (nothing was written then). It also
removes orphans left by deletions. Errors are ignored by design: housekeeping never fails a run. Two tests (the sweep itself; a confirmed sync removes a planted
stub while a refused one leaves it). **Not covered:** `package::execute_install` (writes across `Cores`/`Assets`/`Platforms`, a wider tree than the media root) and
the standalone playlist writers. **Unverified on a real card:** the tests use planted stubs; whether the Pocket card shows no `._` after a real sync is the next
real-card check (TEST_PLAN: look for `._*` under `Assets/<platform>/common` after a sync).

**Real-card check of the `._*` cleanup (2026-10-03, owner-approved, `tau_dev_67` only).** Ignored test `real_card_sync_and_remove_leave_no_appledouble_stubs`
ran the app's own path: add one real 2.1 MB MP3 album ("Omega Sweep Test") with covers/sidecar options on, then remove it again (backup to
`~/Downloads/tau-omega-card-backups`). Result: 0 `._*` files before, after the add, and after the removal; a before/after listing of the card's whole
`Assets` and `Cores` (excluding the rewritten index) was identical; `tau verify` on the rebuilt index says valid. **Caveat:** I did not run it without the fix,
so I cannot say macOS would have left stubs on this particular write (the theme install's rename did). The result shows the sync and removal paths end clean.

### Send diagnostics (2026-10-03)
Built per `DIAGNOSTICS_COLLECTOR.md` (see its "As built" section): `tau_core::diagnostics`, `diag_read`/`diag_zip`, a Send diagnostics page under Tools & settings.
Read-only on the card, zip written outside it, no track names, no other cores' settings. Tests: 5 engine, 4 browser, one ignored real-card test
(`real_card_diagnostics_read_and_zip_without_touching_the_card`) that passed on the owner's card. Open: `tau diag` CLI, an owner trial of the zip.


## 2026-10-09: adaptation steps A and B (Tau's release layout)

- New `tau_core::cardlayout`: one place for a core's declared platforms, `media_platform` (platform picked by the `tau-library.tdb` slot's bits [25:24]), "platform used by another core in any position", and "another core still reads this file from `common/`".
- `Core`/`CoreView`/`BuildIdentity`/`CoreRef` carry `platforms` and `media_platform`; library index status, the post-install library and data-slot checks, the card breakdown, and every UI media root now use the media platform (a Preview or Dev core reads `Assets/tau/common`).
- A: removing a core keeps `Assets/<p>` for every platform another core lists in any position; a dev/preview platform goes with its last core.
- B: a manifest's obsolete paths are acted on only inside the core's own areas (its `Cores/` folder, `Assets/<platform>/<core>/`, platform files nobody else uses) or for a build file moved out of `common/` that the package now owns and no other core reads. Anything else is left alone with a caution; same filter in `compat::check_card`.
- Tested on unit fixtures and the five packages built with Tau's own tools (`tests/new_layout.rs`: removing each core from a five-core card). Still to do: C-E.

## 2026-10-09: adaptation step C (channels and zip choice)

- `release_check::evaluate_for_core`: with the installed core's id, only releases that carry that core count (manifest `core_id` or `replaces` when the release's manifest is known, else the zip name `<core id with _>_<digit...>`, so `alfatreze.TAU_` never matches `TAU_Diagnostics_`/`TAU_Preview_`). A Stable user is no longer offered the Preview core as an Update; newer releases of other channels come back as `others` (information; installing one adds a separate core). Preview updates stay on Preview; a Stable release that overtakes it is reported in `others`.
- Same-day builds without a manifest are ordered by `core.json`'s full version (`0.7.0-dev.385` before `.386`). Channel is read from the version (`X.Y.Z` Stable, `-dev.` Dev, other suffix Preview), else from the core id.
- `compat` parses package `replaces`; the install plan suggests (never performs) removing replaced cores and `alfatreze.TAU DEV NN` builds. Diagnostics zips are recognised case-insensitively (old `TAU_DIAGNOSTIC`, new `TAU_Diagnostics`).
- Host: every Tau-family core on the card (`alfatreze.TAU` prefix) is checked, not only shortname `TAU`; the first with an Update wins. Checksum filtering uses the chosen release's own `SHA256SUMS.txt`. Banner shows `others`.
- Tested on constructed release lists (7 new unit tests) and a plan test for `replaces`. Not yet against a real published manifest (none carries `replaces` yet). Next: D.

## 2026-10-09: adaptation step D (pairing, ordering, display)

- Pairing: `pair_status_with` also checks the ROM's `TAUFWNEED` against the manifest package's `bitstream_features`; a missing feature is `PairStatus::MissingFeature`, refused like a CORE_VERSION mismatch (verdict Mismatch in the plan, Fail in the post-install check). Judged only when both sides state them; a manifest without `bitstream_features` (the current Tau packages) is not refused.
- Same-day pre-release builds without a manifest are ordered by `core.json`'s full SemVer (`0.7.0-dev.385` before `.386`), then by date.
- The manifest's `source` and per-package `rom_version` are parsed; an update's reasons now say "Firmware <stamped version>" and warn when the release was built from a dirty tree.
- The settings viewer takes persisted-id names from the newest cached manifest's `persist_registry` (new command `persist_names`), falling back to the built-in table.
- Tests: 5 new unit tests (feature refusal, notes, ordering, persist names). Still open: E (settings migration, uninstall by owned entries); real manifests with `bitstream_features` do not exist yet.

## 2026-10-09: adaptation step E (settings migration, uninstall that keeps your data)

- `tau_core::settings_migrate`: copies `Settings/<old core>/Interact/_core/interact_persist.json` to the replacing core (a manifest package whose `replaces` names the old core) only when `persist_changed_since` shows no id changed meaning since the old build (identified by hash). Refuses with the reason otherwise (unknown build, changed ids named from the registry, no manifest, destination already has settings). Plan -> token (covers the source bytes) -> execute with read-back verification -> rollback that removes only what it created. The old file is never touched. UI: "Carry over saved settings?" after an install that superseded a non-test core.
- `remove::plan_remove_with(card, core, keep_media, docs)`: with `keep_media` (the UI default, a checkbox), removing a core on a platform nobody else uses deletes only its own files (`Cores/<id>`, `Assets/<platform>/<id>`, plus the old-layout owned files in `common/` that the installed release's layout lists and no other core reads); the library, music, covers, theme file and platform files stay (`RemovePlan.media_kept`). `plan_remove` (whole-platform removal) is unchanged for callers that want it.
- Tests: 3 migration tests (copy + rollback, refusals, stale source), 1 removal test. Not yet run on a real card (use CARDWRITE; nothing here writes without a confirmed plan).
- This completes the adaptation plan A-E from `RELEASE_SYSTEM_IMPACT_2026-10-08.md`; open: a real card proof of E, and real published manifests carrying `replaces` and `bitstream_features`.
