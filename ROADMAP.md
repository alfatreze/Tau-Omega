# Tau Omega roadmap: the one ordered list of what is next

**Rewritten 2026-10-08. This is Tau Omega work only.** Tau Alpha (the firmware and RTL repository) is read-only from here: nothing in this document asks for a change there,
and **a Tau Alpha feature enters this roadmap only once it is on Tau Alpha `main`** (not a branch, not a worktree, not a design doc). Features still in flight on other
Tau Alpha branches (loadable meter packs, the RAM diet, 720p output) are deliberately not planned here; when one reaches `main` it gets its own row. The order is the owner's to set.
Rationale for the earlier state of the app: `docs/ARCHITECTURE_ROADMAP_REVIEW_2026-10-02.md`; the integration record: `docs/FIRMWARE_SYNC.md`.

## 0. What Tau Alpha `main` offers that Tau Omega integrates with (checked 2026-10-08, `main` = `f2d3373`, read-only)

| Surface on Tau Alpha `main` | What it is today | Tau Omega coverage |
|---|---|---|
| **Release packages** | Two zips per release, `alfatreze.TAU_<ver>_<date>.zip` and `alfatreze.TAU_DIAGNOSTIC_...`, 15 files each (`Cores/`, `Assets/<platform>/common/{tau.rom,tau-cold.bin,tau-loading.bin}`, `Assets/<platform>/alfatreze.TAU/`, `Platforms/` + `_images/`), plus `SHA256SUMS.txt`; published as GitHub **pre-releases** on `alfatreze/Tau-Alpha` (latest `v0.6.0-alpha.3`; alpha.4 built, not yet tagged). **The alpha number appears nowhere inside a package**: `core.json` says `0.6.0` for every alpha and only `date_release` differs | Package inspect, plan and install from a zip (hash-checked, confirmation token, tested on a real card 2026-09-23). No version comparison, no update semantics, no GitHub access |
| **Firmware/bitstream pairing** | The ROM carries plain-text markers `TAUFWPAIR:<core versions>;` and `TAUFWNEED:<features>;` (`fw/player.c`); a mismatched pair black-screens with no message (four times so far) | Not read. Omega can refuse a bad pair before writing a card |
| **Data slots** | 1 firmware, 2 audio file, 4 loading artwork (`tau-loading.bin`), 5 library index `tau-library.tdb`, 6 cold image, 7 cover image, 8 `tau-assets.bin`. Slot 3 (legacy playlist) is gone | Library detection by filename (done); slot 4 and 6 are package files (no special handling) |
| **Library index** | Format v1, unchanged; host-built (the Pocket never builds one); `root` is baked in per core | Engine builds, verifies and rebuilds it (byte-exact with the Python reference) |
| **Persisted settings** (`interact_persist.json`) | ids 10 volume, 11 colour, 12 repeat, 15 meter, **16 Halcyon EQ preset** (was the legacy EQ), 20-23 legacy list/Check summary, 24-26 library, 27 theme, 28 theme mode (bit 0 polarity; ReplayGain mode in bits 4-5 of the same word) | Settings viewer labels 10-13, 15, 16 as "EQ" (**stale: now the Halcyon preset**), 24-28; ReplayGain and polarity bits not unpacked |
| **Check / report tags** | `SR_T_*` 1-27 on `main` (new since Omega last looked: 23 heap, 24 load, 25 load2, 26 infotext, 27 nowplaying) | Decoded: 1-14, 16, 19, 20, 21, 22. **Not decoded: 15, 17, 18, 23-27** (no captures yet for 15/17/18; captures exist on the owner's card for some of the new ones) |
| **Report view** | **The pixel grid (TPG) is the default view** of every report page since 2026-10-06 (robust grid up to 6,059 B, lossless up to 259,184 B; QR is one press away). Reference decoder: `tools/decode_tau_suite.py --grid`/`--table` | **Omega decodes QR only**, so most new screenshots are unreadable to it (Send diagnostics found 2 reports in the 12 newest screenshots on the real card) |
| **`tau-assets.bin`** (data slot 8) | Container `TAUA` with `THEM` themes, `METR` meter presets and **`PRST` Halcyon presets** (format `HALCYON_DATA_FORMAT.md`, read limit now 64 KiB); `tools/tau_assets.py` is the reference | Omega writes **`THEM` only** (hardware-confirmed 2026-10-03) and, since 2026-10-08 (merged), **keeps `METR` and `PRST` byte for byte when it saves**; no `PRST`/`METR` writer; no Halcyon importer (the spec assigns the APO/AutoEQ import to Omega) |
| **Cover art** | `TIM1` palette-256 at 128 px in `<album>/tau-art/`, reader hardware-confirmed for MP3 and FLAC; container still unfrozen by the owner (D-I05) | Encoder, decoder and sidecar writing built and tested |
| **Names and paths** | Firmware opens ASCII paths only (issue 001), 200-byte limit | Omega folds names on the copy and refuses collisions |

**Not on `main`, so not planned here:** loadable meter packs (data slot 9, `meter-builder`), the RAM-diet build switches (`ram-diet`), 800x720 output (`test/720`), gapless playback,
Bluetooth output, hardware gain in releases. Omega's Meter Lab stays on its synthetic schema until a real meter set is final on `main`.

## 1. NEXT WORK, in order

### 1. Install and update paths (owner request 2026-10-08, TOP PRIORITY)

Four scenarios. Each ends with an acceptance run on a throwaway card, then the owner's real card (dry run first, write only after approval). Everything here is Omega code;
what it reads from Tau Alpha is the release package layout and the two ROM markers listed above.

| # | Scenario | What exists in Omega | What to build |
|---|---|---|---|
| **Plan stage and executor built 2026-10-08** (`tau_core::install_plan`, `install_exec`; verified backup, rollback; scratch-tested): shared by 1a and 1c; UI built, manifests passed in; **real-card install and rollback proven on `CARDWRITE` 2026-10-08** (first install and an update, each rolled back byte-identical; a Pocket boot test is still open) | |
| **1a** | **Existing Pocket card, no Tau yet** (a card full of other cores gets Tau for the first time) | `package::{inspect, plan_install, execute_install}` (zip to card, merge, per-file hash before and after, plan token); `remove` with backup; card write lock; safe eject | (1) A guided first-install flow in the app: pick the two zips (or one), see what will be added and that nothing existing is touched, install, then "add your music" leads straight into the Library workbench. (2) The shared post-install check (below). (3) Catalog cache clearing (the five `System/*.bin`, backed up first, SAFETY_RULES 6): package install does not do it today. (4) AppleDouble `._` sweep for the files a package install writes (sync, workbench, theme install sweep already; package install does not, and the owner's card shows `._alfatreze.TAU*` stubs under `Cores/`). (5) A clear first-boot explanation: with no library the core says "Library file not found"; Omega offers the sync right after install. A truly blank card is the same flow (we never format or partition) |
| **1b** | **Version update from GitHub** | **Built 2026-10-08 (owner decision: the check is automatic, the update never is):** `tau_core::release_check` (parse, newest, newer-than-installed, `SHA256SUMS` verification, tested on a real captured release list), `src-tauri/src/updates.rs` (`ureq`), commands `update_check`/`update_download`, one-time notice, a Settings off switch, an alert with an Update button that downloads the verified zip and opens the package page | Fetch `tau-compat.json` into the 1c comparison automatically; the update itself (backup, replace, cache clearing, rollback) |
| **1c** | **Version update from a zip file** | Package inspect/install and removal | **Update semantics**: read the installed core's identity (`core.json` version and date, file hashes, ROM markers), read the zip's, and report one of: *new install*, *same build*, *update*, *older (downgrade)*, *mismatch* with the reason. An update replaces the core and firmware files only: media, `tau-library.tdb`, `tau-assets.bin` (the user's themes), saves and settings are kept. Back up what is replaced outside the card, verify, offer rollback from that backup, offer to remove superseded numbered test cores. Refuse a firmware/bitstream pair the Pocket would refuse (parse `TAUFWPAIR`/`TAUFWNEED`; port of `tools/check_fw_bitstream_pair.py`) |
| **1d (built 2026-10-08, scratch-tested; real card run owed)** | **Fix/refresh files already on a core** (owner, same day: files copied by hand must show up in the library) | Scan with ledger, index build/verify, `problems` (duplicates, name/path/cover checks), workbench plans, the AppleDouble sweep | A per-core **Refresh library** action and a library health badge: scan the core's media folder, list every file the library would skip with a plain reason (non-ASCII name, path over 200 bytes, ASCII-fold collision, unsupported format or sample rate, a `._` stub, unreadable tags), fix what is fixable on the card copy (renames recorded as the workbench already does), rebuild `tau-library.tdb` rooted at this core, verify, report **before and after counts**. Same plan, review, confirm path; nothing outside the core's media folder is touched |

**Shared post-install check** (engine function, used by 1a, 1c and 1d; the executable form of the manual checklist that Tau Alpha keeps): hashes of `bitstream.rbf_r`, `tau.rom`, `tau-cold.bin` against the package; ROM pairing markers;
`core.json` valid and name lengths within the Analogue limits; data slots declared; library index present, valid, rooted at this core; catalog caches cleared; no `._` or temp files; one-line verdict and a saved report.

**1b design.** Engine stays dependency-light: the network code lives in the host crate `src-tauri`, not in `tau-core`. A user-initiated button (never automatic by default): `GET https://api.github.com/repos/alfatreze/Tau-Alpha/releases`
(unauthenticated; pre-releases included, since every Tau release so far is one), choose the newest, download its two zips and `SHA256SUMS.txt` to the app cache, verify the checksums, then hand the zips to 1c. Offline-first and private: only that request leaves the machine, no
identifiers, results explained in plain words (rate limit, offline, no newer release). **New dependency to flag before it is added:** an HTTPS client. Candidates: `ureq` with rustls (small, blocking, the right size here) against `reqwest`
(heavier, pulls an async runtime) or `tauri-plugin-http`; I recommend `ureq`, and the cost (transitive crates, binary size) is measured and reported before it goes in.
**Version identity caveat:** a package does not carry the alpha number, so Omega compares by `date_release` plus the SHA-256 of the ROM and bitstream, and takes the human label (`v0.6.0-alpha.4`) from the GitHub tag it downloaded from; a zip opened from disk shows date and build id instead.
**Compatibility table:** a small JSON that ships inside Omega (and can be refreshed from a release) mapping a release to what it needs from Omega (index version, `tau-assets.bin` sections, report tags). Unknown newer releases are offered with a warning, never blocked silently.

**Owner decisions for item 1:** (1) is the update check a button only, or also an off-by-default check at launch; (2) the compatibility table: shipped inside Omega, or fetched from the release; (3) after an update, does Omega re-verify the library index automatically (my default: yes, read-only, and offer a rebuild if the build id no longer matches); (4) the HTTPS client choice above.

### 1e. Adapt to Tau Alpha's new release system (DONE 2026-10-09/10; evaluation and plan in `docs/RELEASE_SYSTEM_IMPACT_2026-10-08.md`)

Tau's format is fixed, Omega adapted (no change on Tau Alpha). Built, tested and pushed:
- **A** each core's media platform (the platform its library slot reads, `tau` for Preview and Dev cores) is used for the card list, workbench, sync, Appearance, Refresh, health badge and post-install check; removal keeps `Assets/<platform>` while any other core lists it in any position.
- **B** a manifest's obsolete paths are acted on only inside the core's own areas (or a build file moved out of `common/` that no other core reads); everything else is left alone with a caution. Same filter in `check_card`.
- **C** channel-aware update check (a Stable user is never offered the Preview core; other channels are information), zips chosen from the manifest (`core_id` or `replaces`), every `alfatreze.TAU*` core on the card is checked, removal suggestions from `replaces` and `TAU DEV NN`.
- **D** feature pairing (`TAUFWNEED` against `bitstream_features`), same-day builds ordered by the full `core.json` version, firmware version and dirty-tree notes, persist names from the manifest registry.
- **E** settings migration for a replaced core (only when no saved setting changed meaning) and an uninstall that keeps the library, music and theme file (checkbox, default on).
- Proven on the real `CARDWRITE` card (install, remove with and without keep-media, migration, card restored byte-identical) and against the real `v0.6.0-preview.1` release (installs, all checks pass, offered to the right channel).
- **Still open:** a real release whose manifest carries `replaces` (v0.6.0-preview.1 has none); a Pocket boot test of a core installed by Omega in the new layout; without a manifest, keep-media leaves the old-layout `tau.rom`/`tau-cold.bin`/`tau-loading.bin` in `common/`.

### 2. Read the pixel-grid report codes (decoder + QR viewer + Send diagnostics DONE 2026-10-10; tags 23-27 and settings-viewer labels DONE too, see below)

**Built:** `tau_core::tpg` (TPG1 and TPG2, modes L and R, exact 400x360 screenshots only), checked against the six real 2026-10-04 grid captures: each grid record equals its QR record byte for byte; a damaged grid fails its CRC. `taud::read_screenshot_report` tries the grid then the QR; the QR viewer and Send diagnostics use it. **Tags 23-27 decoded** (heap, load, load2, Info rows, now playing; shown in the report card). Only 26/27 have a real capture (`20261008_223548.png`, the Info export from Tau's probe-2 folder); 23-25 are tested on a hand-built record because the 2026-10-04 grids predate the final tag numbers. Settings viewer: id 16 reads "Halcyon EQ preset", id 28 unpacks polarity and ReplayGain. Resized/recompressed copies are not read (tpg.py can resample them; not needed for real screenshots).


Since 2026-10-06 the firmware shows reports as a pixel grid first and a QR second, so Omega's Send diagnostics and QR viewer see only the minority of screenshots. Build a Rust decoder for both grid layouts (robust: 4x4 cells, 2 bits per channel; lossless), reference `tools/decode_tau_suite.py --grid shot.png --table` and the format notes in `docs/features/BARCODE_STUDY.md` on Tau Alpha `main` (read-only).
Verify against real captures first (the owner's card should hold grid screenshots from 2026-10-06 on: check, then copy a few with their provenance, per the rule that fixtures come from real artefacts), then plug it into the QR viewer and Send diagnostics so one "read this screenshot" works for either view.
Then decode the tags `main` now emits that Omega skips (23 heap, 24 load, 25 load2, 26 infotext, 27 nowplaying; 15, 17, 18 stay undecoded until a capture exists). Also in the Settings viewer: label persist id 16 as the Halcyon EQ preset and unpack the polarity and ReplayGain bits of the theme-mode word.

### 3. `tau-assets.bin` completeness (so saving never destroys what the card holds)

**Read-modify-write: built and merged to `main` 2026-10-08 (archive tag `archive/taua-roundtrip`).** `assets::pack_assets_keeping` replaces only `THEM` and carries every other section (`METR`, `PRST`, anything newer) byte for byte and in place. It is used by the card install and by Export, which keeps the sections of the file the themes were opened from, or of the file it overwrites. A newer container version is refused instead of overwritten, and the firmware limits (8 sections, 64 KiB) are enforced. Tests: real METR+PRST round trip, order, no-THEM file, newer version, limits, a section-dropping mutant caught by 3 tests. Tau Alpha's shared fixture `docs/schemas/fixtures/tau-assets-roundtrip.bin` is checked when the sibling checkout has it; it is skipped until that lands on Tau Alpha `main` (D-015). Tau Alpha's release manifest (`tau-compat.json` schema 2, on its `release-system` branch) marks the format `preserve_unknown_sections`.
**Halcyon `PRST` writer, reader, APO/AutoEQ importer and the Halcyon EQ page BUILT 2026-10-10** (`tau_core::halcyon`, `assets::AssetsEdit`/`pack_assets_edit`/`plan_install_edit`/`execute_install_edit`, `HalcyonView.svelte`): byte-identical to Tau's `tools/halcyon_assets.py` on the fixtures in `testdata/halcyon/` (two imported profiles, a mixed control+raw file), every bit flip refused, writer refuses every unsafe case, a preset edit keeps `THEM`/`METR` byte for byte (and the reverse), a file Omega wrote parses with Tau's own `tau_assets.parse`. Page: six control sliders, APO import with headroom message and response curve, open/save/install through the same reviewed, backed-up, read-back install as themes. **Not yet done: the real-card proof (a Pocket loading a preset Omega wrote; needs CARDWRITE + a Pocket, with the Diagnostics > HALCYON / Info row as the check).** The original brief follows:

Original brief: add what `main` defines and the spec assigns to Omega: the **Halcyon `PRST` section** writer and reader (control presets: six int8 controls; raw biquad presets: Q2.22 coefficients; the name, size and stability rules are the writer's job because the firmware does not clamp), an **APO/AutoEQ profile importer**, and a Halcyon preset page next to Appearance (the six sliders, the response curve, validation messages in plain words). Check the byte layout against `tools/halcyon_assets.py` output exactly as was done for themes (a fixture generated by the reference tool, byte-identical test), and the real-card proof is the Pocket loading the preset (the Info row), as with the theme file.
The `METR` section (per-meter presets) waits: its meaning depends on the meter work that is not on `main` yet.

### 3b. Release manifests (DONE: engine merged 2026-10-08, wired into the update check, install plan, post-install check and card check by step 1e)

`tau_core::compat` identifies the installed release by hash, applies the union rule for changed settings, checks a card against the release, and reads Tau's core-specific layout. The GitHub check fetches the manifest next to the zips and a dev package's own `tau-compat.json` is read next to a local zip. Remaining: only the `replaces` verification above.

### 4. Hardening what is built (small, no new surface)

**2026-10-10:** `cargo clippy --all-targets -D warnings` is clean (the 12+3 lints were tests and doc lists). CI on GitHub has been **red on Windows only** since at least 2026-10-09 (macOS, Linux and the UI job pass): four tests assumed `/` in paths (`install_exec` snapshots, one `workbench` assertion); fixed in the tests and **pushed: run 38039861361 is green on all four jobs (ui incl. the browser-suite step, Windows, Linux, macOS)**, so the CI browser step is proven.

- Prove the CI browser-suite step on GitHub (it was added locally and has not been seen to run) and keep `cargo fmt`, `clippy` and the 174-check suite green.
- Ledger follow-ups: per-volume I/O governor, hash-while-copy for plain copies, a Library fit bar that counts whole clusters, a card-speed benchmark (`tau-cli bench-card`); first real card timing numbers are the gate.
- Send diagnostics trial by the owner (zip contents) and its `tau diag` CLI verb.
- Windows and Linux: only macOS has been exercised; the card write lock, eject and cache-bypass read-back need a pass on each.
- Split the large engine and host files (`sync.rs`, `lib.rs`, `main.rs`, the pre-workbench state in `App.svelte`) in small behaviour-preserving commits, after CI is proven.
- Keep `docs/FIRMWARE_SYNC.md` current after every Tau Alpha release (this roadmap's section 0 is the live summary).

### 5. Later

`tau-cli` parity with `--json` everywhere (also the Pocket Sync surface); installers, signing and the opt-in updater for Omega itself; Pocket Sync adoption (TP2); watch mode, smart playlists, loudness tags, localisation; the `TIM1` container freeze when the owner decides (D-I05); Meter Lab on the real meter registry and the `METR` writer, **only after the meter work is on Tau Alpha `main`**.

## 2. Built and verified (so it is not re-derived)

Engine: index, scan with the verification ledger, sync with provenance, workbench edits, packages and removal, backup, comparison, journal, storage, `taud` (QR) and `diag`, screenshots, `TIM1`, **theme file writer and card install (hardware-confirmed 2026-10-03)**, **AppleDouble sweep after sync, core move, workbench and theme install (real-card checked)**, **Send diagnostics (read, summary, zip; real-card read-only run)**.
App: Library workbench with one card-space bar and sync progress, Appearance editor with install review, Send diagnostics page, Meter Lab (synthetic), history, settings. 160 Rust tests, 174 browser checks, clippy and fmt clean (2026-10-03). Newest test build: `releases/dev-builds/0.4.0-alpha.7` (Appearance, Preparing sync); alpha.8 would carry install-from-card, `._` sweep and Send diagnostics.

## 3. Rules for this roadmap

1. **Tau Omega only.** Never edit the Tau Alpha repository from here; read it for integration facts. Anything Omega needs changed there goes to the owner as a note, not a patch.
2. **Integrate with Tau Alpha `main` only.** A format, tag or file layout is planned here when it ships on `main` and has a real captured artefact to verify against (the project rule: fixtures are copied from real artefacts, never written from memory).
3. This is the only file that orders Omega work; specs and handoffs say what exists. Update `docs/STATUS_HANDOFF.md` when a fact changes.
4. Card writes only after a shown plan and the owner's approval; new dependencies are flagged with their real cost before they are added.

---
*Everything below is the original 2026-09-22 roadmap, kept for the ideas it lists. Where it disagrees with the tables above, the tables win.*

## Original phases, extensions and ideas

Phases are ordered so each one is usable on its own and each later firmware phase has a place to land.

## App phases
| Phase | Deliverable | Exit test |
|---|---|---|
| **T0 Skeleton** | Workspace, CI on macOS/Windows/Linux, Tauri shell, card and folder detection, read-only Cards and Cores screens, core.json/data.json parsing, library-capability detection, fake-card fixtures. | Opens a real card and a fixture card; lists cores correctly; no writes anywhere. |
| **T1 Index engine** | `tau-core` scanner, tag readers, ASCII rules, index writer/reader/verifier, playlist rules, synth generator, `tau` CLI (`scan`, `index`, `verify`, `report`, `synth`). Golden files from the Python reference. | Byte-identical to Python on the golden set; all corruption cases give the same E-codes; the 7,180-track synth verifies. |
| **T2 Sync** | Plan/execute/verify/journal/manifest; sync wizard; cover embedding for MP3 and FLAC copies; optional cover optimisation; incremental runs; mirror with confirmation. | Sync a fixture library to a staging card; source hashes unchanged; index verifies; re-run is a no-op. First real-card run approved by the owner. |
| **T3 Multi-core** | Copy/move between cores and cards, compare cores, cleanup tools, backup/restore, numbered-core lifecycle (install new, migrate media, remove superseded after backup). | Copy A to B on one card and across cards; move with two-step delete; both indexes verify on the device. |
| **T4 Library workbench** | Library tabs, Problems with fixes, playlist editor, search/filter, device-accurate previews of the Pocket's lists. | The desktop lists match the Pocket screen order and letter jumps for the same index. |
| **T5 Core install** | Install/update cores from zip or folder, packaging checks, rbf reversal, diffs, known-builds table, two-zip install. | Installs the v0.3.0 pair from `release/*.zip` on a staging card exactly like the manual procedure. |
| **T6 Diagnostics** | Settings viewer with "last played", diagnostic record decoder, screenshot gallery, log viewer. | Decodes the recorded B-022/B-023 style records to the same JSON as `decode_tau_diag_log.py`. |
| **T7 Polish and release** | Installers, signing/notarisation, updater (opt-in), accessibility pass, documentation, sample data, crash reports (local only). | Release checklist in `TEST_PLAN.md` passes on both OSes. |
| **T8 Art (when firmware has the blit engine)** | Thumbnail pipeline, `tau-library-art.bin` writer (spec in `DATA_FORMATS.md` section 3), art id in the index header, preview of what the Pocket will show. | Firmware reads the file; index and art ids match; stale-art detection works. |

## TP Portability and Pocket Sync integration (added 2026-09-22)

TP0 sits **before T4**, because those defects get more expensive with every feature built on top.
Decisions D-009/D-010/D-011; findings and acceptance criteria in `docs/PORTABILITY_AUDIT.md`.

| Step | Deliverable | Exit test |
|---|---|---|
| **TP0 Boundary fixes** | Domain rules (card path prefix, index status) live only in `tau-core`; warnings and errors machine-readable, preserving E-codes; progress and cancellation hooks on scan/plan/execute. | A front-end branches on a failure without matching strings; a long scan reports progress and can be cancelled; no path or status logic remains outside the engine. |
| **TP1 API shape** | Optional `serde` feature on public types; no public entry point panics on caller-supplied input; `plan_with_*` collapsed into one options struct. | The Tauri `*View` DTO layer is gone; `build_index` returns `Err` on malformed caller data instead of panicking. |
| **TP2 Adoption** | `tau-core` consumed by Pocket Sync as a crate dependency (upstream contribution or fork). | Pocket Sync indexes a card using our engine, with its own UI driving plan → confirm → execute. |

Not doing: plugin ABI, C ABI, WASM, sidecar hosting, or a remote/streaming media-source abstraction.
Pocket Sync is Rust and reads local cards, so none of those are needed (D-009).

## Extensions worth building (pick by value)
1. **Card profiles / presets:** save a sync recipe (sources, options, target core) and rerun it in one click; per-card defaults.
2. **Watch mode:** watch a source folder and offer a sync when the card is next inserted ("3 new albums since last sync").
3. **Storage planner:** "will this fit?" with per-core budgets; suggest what to leave out; show the cost of covers.
4. **Smart playlists** built on the desktop from tags (recently added, by year, by genre) and exported as ordinary `.m3u` files the firmware already understands.
5. **Audiobook / long-file helper:** one-file playlists, chapter names, resume hints (relates to the firmware's resume feature).
6. **Loudness and gain tags (later):** analyse ReplayGain on the desktop, write gain into a sidecar or the index when firmware supports it.
7. **Duplicate finder** across sources and across cores; safe dedupe on copies only.
8. **Tag doctor:** guided fixes on the copies (missing track numbers so tiles are not `00`, multi-disc numbering, inconsistent album artists, "Various Artists" handling); optionally writes a sidecar rules file so re-syncs reapply the fixes.
9. **Format checker:** detects MP3 variants and FLAC settings the decoder cannot play in real time (48 kHz limit, block size, bit depth) and offers a copy-time conversion later.
10. **Multi-card sync:** the same library to several cards; a "clone card" job.
11. **Cores gallery:** show icons and platform artwork from the card, help write `core.json` fields for custom cores, validate JSON against Analogue's schemas.
12. **Save and memory management** for all cores: browse, back up, restore, prune.
13. **Firmware phase hooks:** a *capabilities file* per core version (`tau-caps.json` in the core folder or in Tau Omega's registry) so the app enables or hides features (art file, resume by second, index v2, new persisted words) for the firmware it finds.
14. **Send diagnostics (decided 2026-09-21, priority):** one click gathers the Tau cores' persisted-settings files, screenshots newer than the last test run, core/index/cold-image versions, card free space and filesystem and file hashes into one zip for support; decodes the on-screen report code (`tools/decode_tau_suite.py --code` logic ported to Rust) and shows the verdict before anything is sent; nothing is uploaded automatically. Earlier idea text follows. **Remote diagnostics:** export a zip with `interact_persist.json`, screenshots and the app's manifest for the developer ("send diagnostics"), matching the README's diagnostics section.
15. **CLI and scripting:** every feature available as `tau <verb> --json`, so the project's own scripts and CI can drive it (the existing `sync_media.py` workflow keeps working through a compatibility shim).
16. **In-app preview player:** play a track from the source or the copy to check tags and covers (optional, last).
    Reference available: `awesome-music-player` (S1avv) is MIT, Tauri 2 + Rust + React, offline-first
    local playback with `lofty` tags — the same stack as ours, so it is borrowable for this and for
    wider player features. Assessed as a host and rejected; kept as a reference (D-012). Copying from
    it is permitted **with its copyright notice carried**.
17. **Localised UI, dark/light theme, Pocket-edition themed accent.**
18. **Plugin API** for third-party openFPGA cores that also read media (registry files plus a media-processor interface).

## Firmware and core phases the app must be ready for
* **Blit engine and thumbnails:** art file writer, art id, thumbnail sizes 32 and 92 (per the spec), preview.
* **Library resume by second, Diagnostic Build library page:** show them in the settings viewer once the words exist.
* **Cached PSRAM code / more RAM:** no app change, but new persisted words and diagnostic records will appear; the decoders must be table-driven so adding a field is data, not code.
* **Hardware audio and EQ:** EQ curves and presets in the settings viewer; later per-track gain.
* **Index v2** (genres, years, search keys, longer text, UTF-8): version-negotiated by the core's capability entry.
* **Other Analogue cores using the same media layout:** the core registry and the copy engine already handle them; only the indexer needs a per-family strategy.


## Added 2026-09-21 (tau-alpha B-063): Send diagnostics
See `DIAGNOSTICS_COLLECTOR.md`: read the Check's screenshots (QR) and persisted summary, decode with the Python reference format, and create one local zip for a bug report. Priority: with T6 (Diagnostics), before installers.
