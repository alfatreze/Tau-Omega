# Firmware sync check — tau-alpha

> **Newer review: `docs/TAU_ALPHA_SYNC_PLAN_2026-10-26.md`** (against Tau Alpha v0.6.1 and its unreleased `main` of 2026-10-23: gap analysis and implementation plan). The sections below are the older record.

Tau Omega writes files that Tau's firmware reads, so its assumptions go stale whenever tau-alpha
ships. This is the standing record of when they were last checked and what is still open. **Re-run it
after every tau-alpha release**, and when the blit engine lands.

Last checked **2026-10-02** against tau-alpha **v0.6.0-alpha.1** (see the section below; the older notes are against **v0.5.0**, checked 2026-09-27) (tag `v0.5.0`, commit `63ea499`), by re-reading its docs
(`CROSS_PROJECT_INTERFACE.md`, `tools/tau_data_slots.py`, `THEME_FILE_FORMAT.md`, `CHANGELOG.md`) and the shipped
`dist/Cores/alfatreze.TAU/data.json`. **Not yet done: re-verifying our code against real files captured from a card**
(a `tau-assets.bin`, a `SR_T_METERCFG` record, a fresh `.timg`); until then the items below marked *re-verify* are
docs-level only.

## Re-check against v0.6.0-alpha.1 (2026-10-02)

Checked tau-alpha tag `v0.6.0-alpha.1` (`273b67b`) and its `main`. **Docs and shipped files only; nothing yet re-verified against a file captured from a card.**
- **Persist ids changed:** the shipped `interact.json` declares 24-26 "library 1..3", **27 "theme", 28 "theme mode"**. Omega labelled 27 "Library off": fixed
  (`App.svelte`, `DATA_FORMATS.md` section 4). A card last written by a 0.5.0 core can still hold a library-off value in id 27; the file carries no core version,
  so the label is right only for 0.6 cores. tau-alpha's `tools/tau_data_slots.py` docstring still says id 27 is retired: stale on their side (to report).
- **Check/QR tags:** firmware enum is now `1..22`. **Decoded since this check (2026-10-02, from 8 real captures):** 16 stack, 19 Info export, 20 meter config (values kept raw: widths come from the meter registry), 21 meter trace, 22 decode-stage split (4/6/7-field forms), plus profile 8 "BLIT TEST" and test 14 "Cold frame", which were missing. **Still undecoded, no real capture exists:** 15 Blit Test results, 17 Winamp config export, 18 Meter Sweep (kept as unknown entries and shown as "not decoded", not dropped). tau-alpha's own `VIZ_NAMES` table stops at Chladni (id 14), so a Meter Sweep decoded by it would mislabel Layered Wave (16); Omega should take names from the real registry, not that table.
- **Meters:** Layered Wave is meter id 16; the Meter slider range grew. Meter Lab still uses the synthetic schema; swap to the real `meters_schema.json` (open).
- **`tau-assets.bin` `THEM` section: hardware-confirmed 2026-10-03.** A file written by Omega's writer was loaded by a Pocket running `alfatreze.TAU_DEV_67` (`Info > THEME FILE` = `1 LOADED`, theme listed). `METR` is still unverified from Omega's side (not written). Earlier note: container unchanged in layout; `fw/assets_core.h` has a 3-line change after the tag, not yet read. The format is still not frozen until a card reads a real file.
  Omega now writes the `THEM` section (byte-identical to `tools/tau_assets.py`); it does not write `METR` and drops it on save.
- **Unchanged and still correct:** data slots 5-8, index v1, caps, persist ids 20-26.

## Verified correct — no action

- **Index v1**: magic, 128-byte header, `art_id` at offset 44, loader codes E10-E17. Matches
  `tau-alpha/docs/MEDIA_LIBRARY_0.4_SPEC.md`.
- **Caps**: 16,384 tracks / 2,048 albums / 1,024 artists / 64 playlists, 4 MiB file, 3 MiB strings,
  200-byte paths. Docs, engine constants and firmware agree.
- **No index v2 is planned.** Genres, years, search keys, UTF-8 and non-ASCII display text are all
  reserved-but-deferred; the format will not move under us in the near term.
- **Persist ids 24-27** (library history, index build id, Shuffle All seed, library-off) match
  `tau-alpha/tools/tau_data_slots.py` exactly.
- **Data slot 5** = `tau-library.tdb`, `deferload`, not required. Shipped cores never bundle the file
  — it is built from the user's own music, which is Tau Omega's job.
- **Thumbnails deferred on both sides.** 0.4 draws a numbered placeholder tile; our T8 correctly
  waits. Nothing we assume was dropped.

## Open conflicts — resolved by v0.5.0 (2026-09-27)

1. **RESOLVED: data slot 6 was double-booked.** As shipped, slot 6 is the cold image, the cover image is slot 7 (opened by name) and `tau-assets.bin` is slot 8. Original text follows.

   **(old)** Data slot 6 was double-booked. `MEDIA_LIBRARY_0.4_SPEC.md` section 3 reserved slot 6 for
   the art file; Phase G shipped the cold image `tau-cold.bin` in slot 6, and it is in the released
   core today. `IMAGE_FORMATS.md` (2026-09-26, D-I05) doesn't resolve this either — the container is
   explicitly still unfrozen and no slot number is assigned. Live, because the format decision that
   was blocking this (pixel format, next item) is now made.
2. **Firmware reader shipped in v0.5.0 (was: none), path `tau-art/cover_128.pal256.timg`, hardware-confirmed for MP3 and FLAC albums; container still formally unfrozen (D-I05). *Re-verify*: run our encoder's output through a card and compare with a `.timg` the firmware accepted.** Earlier: art pixel format is no longer "RGB565 assumed". tau-alpha ran a real
   study (`IMAGE_FORMATS.md`, owner decision D-I01/D-I02) and decided **palette-256 (`TIM1` container,
   CLUT + 8-bit indices) at 128 px on the long side, proportional scale, no crop or letterbox** — not
   raw RGB565. `tau_core::image` is built against this decision (see `IMPLEMENTATION_PLAN.md`'s new
   cross-project section). The container itself is still explicitly unfrozen (D-I05) and there is
   still no firmware reader — re-check both before trusting a byte layout across a tau-alpha update.

Both are recorded in `DATA_FORMATS.md` section 3 next to the design they affect.

## Interfaces that became real in v0.5.0 (re-verify against captured files before building)

- **Meter presets (`tau-assets.bin`, `METR` section, `SR_T_METERCFG`, `meters_schema.json`) and themes (`THEM`).**
  **Update 2026-09-27:** M0-M5 are built and v0.5.0 ships the `TAUA` container reader (data slot 8),
  `tools/meters_schema.json`, `SR_T_METERCFG` (tag 20) and `SR_T_METERTRACE` (tag 21); the container format is
  documented in `tau-alpha/docs/THEME_FILE_FORMAT.md` but explicitly not frozen until a Pocket has read a real file
  (a Pocket run of the release is pending, and only a 351-byte sample file exists). So the guidance below still
  applies in spirit: capture a real file from a card first. Original (2026-09-26) text follows: tau-alpha was only mid-**M0** (manifest generator, `meters_schema.json`
  emission just landed) — **M2** (the generic Configure page + `SR_T_METERCFG`) and **M4** (the
  actual `tau-assets.bin` container + the Omega hand-off) haven't happened, so there is no real
  container or schema file to verify a fixture against yet. Do not build the Omega-side editor,
  `.tmeter`/`.tmeterpack` handling, or the container writer until tau-alpha tags a release that
  actually contains `meters_schema.json` and a real `tau-assets.bin` — then re-run this check, copy
  a real captured container into `testdata/`, and build against that, per section 2's rule.

## Traps to respect

- **Persist ids 20-23 are overloaded**: legacy playlist state in every core, *and* the Check report
  reuses the same four variables. No flag distinguishes them — validate the TAUD1 CRC32 and fall back.
  Detail in `DATA_FORMATS.md` section 4.
- **"Persist word N" ≠ "variable id N".** tau-alpha's logs count declaration positions (words 8-11 are
  ids 20-23). The file carries ids; key on those.
- **Check is Diagnostic-Build-only** by standing decision. A card running the plain release core has
  no Check report, and the UI should say so rather than showing an error.
- **ASCII folding is a workaround, not a format rule.** It exists because tau-alpha BUG-001 (accented
  filenames fail to open) is still open. If that is fixed, our folding becomes needless data loss —
  revisit rather than cement.
- **Numbered test cores are `TAU_DEV_NN`** now; `TAU_PSRAM_NN` are retired.
- **Two decoders, easily confused**: `decode_tau_diag_log.py` (persisted settings, older diagnostic
  records) vs `decode_tau_suite.py` (the Check/TAUD1 report).

## Bug this check found — fixed 2026-09-22

Library capability detection never matched a real card. `slots_have_library` read `data.json`'s
`data` key as an array; the real APF layout is `{"data": {"data_slots": [...]}}`. Every shipped Tau
core — which does declare slot 5 — was reported as "legacy", so the media library, the product's
headline feature, was gated behind a check that could not pass on hardware.

It survived because **the test fixture had invented the flat shape** rather than being copied from a
real core, so the fixture agreed with the code and both disagreed with reality. There was also no
test over `inspect_card` at all.

Fixed by reading the real layout (still keying on the filename, never the slot id, and still
accepting the flat shape for older hand-written cores), rebuilding the fixture from the shipped v0.4.0
slot table, and adding `crates/tau-core/tests/card.rs`.

**The lesson is the durable part:** the index path is byte-exact against a Python oracle and was
flawless; the card path was checked against a fiction and was broken. *Fixtures for anything the
firmware or APF produces must be copied from real artefacts, not written from memory of the format.*

## Second bug this check found — fixed 2026-09-23

The lesson above wasn't followed for `read_persisted_settings` either. It looked for `variables` at
the JSON root; every real hardware-captured `interact_persist.json` nests it under `interact_persist`
(`{"interact_persist": {"magic": "APF_VER_1", "variables": [...]}}`). Its own test fixture used the
flat shape, so the test passed while the real Settings screen silently returned nothing from every
real card since it was written.

Found only by copying three real `interact_persist.json` captures from `tau-alpha/work/diagnostics/`
into `Tau Omega/testdata/interact_persist/` (see that folder's README for provenance and exact source
paths) instead of continuing to trust the hand-written fixture. Fixed by checking
`/interact_persist/variables` first, falling back to the bare top-level shape for older hand-written
fixtures/tools. Same real fixtures also back a new `tau_core::diag::{decode_check_summary,
read_check_summary}` — a byte-exact port of `tau-alpha/tools/decode_tau_suite.py`'s `unpack_words`
(the persisted 4-word Check-report summary at persist ids 20-23), cross-checked against that script's
own `--interact --json` output on the same files.

## New interface built this check — 2026-09-26

New `tau_core::image`: `TIM1` decode for every payload shape real tooling produces (`rgb565`,
`palette` at 8/6/4 bpp) and a palette-256 encoder for the newly decided default (D-I01/D-I02).
Verified against a real `.timg` file copied from a real album on tau-alpha's own card backup
(`testdata/images/README.md`), not a fixture invented from `IMAGE_FORMATS.md`'s prose — same
discipline as the two bugs above, applied before either the encoder or decoder was trusted. No bug
found this time (the format is new enough here that there was no prior, wrong assumption to catch),
but recorded per this file's own convention of logging what was checked and against what.

## Meter presets as files, persist layout v2 — checked 2026-11-07 (Tau Alpha `main` `4c610d63`)

Evidence labels: **host-tested** = Omega's code compared with Tau Alpha's reference output or real artefacts on this computer; **on a Pocket** = ran on
hardware. **Nothing in this section has been run on a Pocket by Omega.** Tau Alpha's own status line (`METER_PRESETS_EXTERNAL.md`) still says "nothing has
run on a Pocket yet" for the preset files.

| What | Checked against | Result | Label |
|---|---|---|---|
| Preset file formats (`index.tpi`, `p<id>.tpr`), key hash, CRC, limits | `fw2/cores/mpreset_core.h` (firmware reader), `tools/player2/mpreset.py` (reference writer), `fw2/art/mpreset_host.c` (path: `<root>/tau-presets/<meter id>/index.tpi` or `p<id>.tpr`) | `tau_core::preset_files`: reader refuses what `mp_index_check`/`mp_preset_check` refuse; writer refuses what the reference refuses (+ more, below) | host-tested |
| The 51 real files on the owner's `Pock` (`Assets/tau/common/tau-presets/12..18`, copied 2026-11-07) | Omega's reader, then Omega's writer | every file parses; Omega's writer regenerates **all 51 byte for byte** from the parsed set (`tests/preset_files.rs`); the card's files equal `mpreset.py factory` output on this commit | host-tested |
| Factory import | `meters/<key>/presets.json` of the seven meters with parameters | `factory_set` rebuilds the card's files byte for byte (ids = positions, names upper-cased and cut to 15, only the meter's keys) | host-tested |
| Registry ranges and defaults | `tools/meters_schema.json`, `meter_desc.param_rows` | enum `0..len-1`, bool `0..1`, colours (`custom_*`, `color_green/yellow/red`) the whole 16-bit range, others `min..max`; default else `min` | host-tested |
| Persist layout v2 (B-968): 7 preset positions (`MPRE0..6`), 7 preset-file ids (`MPID0..6`, 7 bits, `id + 1`, words 6 and 15) | `tools/persist_layout.py`, `golden_vectors.json` re-pinned (was 2026-10-26, now `4c610d63`) | field table equals the reference; all 37 + 32 golden vectors pass; a real DEV 213 file (`dev213_interact_persist.json`) decodes (it has no preset chosen, so it does **not** exercise `MPID`) | host-tested; the `MPID` read is **not** checked on a real capture |
| Name of the chosen preset | `describe_with_names` with a resolver; test with a synthetic word (id 4 in slot 3) | shown as `AURORA (id 4)`, else `preset id 4` | host-tested (the slot -> meter lookup is not wired, see below) |
| Update must not overwrite the owner's presets | Tau Alpha **B-982** (newer than the brief's B-946..B-981): the release zip now carries the factory set at `Assets/tau/common/tau-presets/`, role `shared`, `required: false`, "seeded when missing" | Omega's install plan treats a meter folder that already has an `index.tpi` as the owner's: none of its files is written (reported as a caution), a meter with no `index.tpi` is seeded whole (`package::kept_for_user`; tests install a zip over an edited folder and over a missing one) | host-tested |
| Scan, problem finder, eject sweep | the real tree plus the 58 `._*` stubs the card had beside the files | nothing under `tau-presets/` is a track, a problem or removed; the eject sweep removes the stubs and keeps every preset file | host-tested |

### Differences and surprises found (to tell Tau Alpha)
1. `mpreset_core.h`'s header comment says the folder is `<meter key>`; the code, the Python and the card use the **manifest id** (a number). The brief and Omega follow the code.
2. `METER_PRESETS_EXTERNAL.md` §6.1 still says "200 presets per meter" and an 8-bit id; the code and header say **127** and ids **0..126** (7 bits, B-968). Omega follows the code.
3. The card's `tau-presets/` had **58 `._*` AppleDouble stubs** (macOS wrote them beside the files the installer copied). Omega's eject sweep now clears them; Tau's `install_factory_presets.py` could do the same.
4. Preset names in the factory set include `[` and `]` (`SILK [EX]`), so they are **not** limited to the `METR` set `A-Z 0-9 space _ -`. Omega allows printable ASCII, 1 to 15 characters (stricter than the reference, which only requires ASCII), and the UI should default to upper case like the factory set (not verified: whether the Pocket font has lower case).
5. `tau_compat.py` says the presets are `shared`, but `shared` in the manifest is a warning-only check in Omega (`compat::check_card`), which is what is wanted; the *install* rule above is Omega's own and is not in Tau's manifest. Worth confirming with Tau that "seeded when missing" means what Omega does (whole meter folder), or per file.
6. The persisted slot index (`MPRE`/`MPID` 0..6) is the meter's **table slot** (`p2.slot`: built-in Winamp Bars = 0, then the pack-only meters by slot), not the manifest id. Omega can derive slot -> manifest id from the pack descriptors on the card (`meter_packs`) plus the built-in meter; that lookup is not wired yet, so the Settings screen shows `preset id N` without a name.

### Added later the same day (host-tested; nothing on a Pocket by Omega)
- **Card write** for preset sets (`preset_install`, CLI `meter-presets-show/plan/install`, app commands): plan with token, backup outside the card, presets first/index last, read-back of every file and of the folder, rollback on failure, recovery of an interrupted write, retired ids (never reused). Tests on scratch cards; five mutants (no rollback, optional backup, no removals, token ignoring the card, no backup write) all fail the suite.
- **Editor** (`MeterPresets.svelte`): list, new, duplicate, rename, hide, reorder, delete, factory marks ("factory", "factory, edited", "yours"), generated parameter forms from the registry (`when` conditions, groups, experimental, colours as RGB565), reviewed save. 30-check browser suite against the dev mock, which serves the real registry fixture.
- **Settings** names the chosen preset: the saved slot -> manifest id comes from the descriptors in the core's own `tau-packs.bin` (checked on the real DEV 184 bundle: slots 0..6 -> ids 12, 13, 14, 16, 15, 17, 18) and the built-in meter (slot 0 = 12); the name comes from the card's `index.tpi`.
- **Registry**: `MeterRegistry` reads names, defaults and the factory presets, and accepts the nested shape a `lab-data.json` may use (assumption until R-025 delivers the real file).
- **Real-card plan** `docs/REAL_CARD_PRESETS_TEST.md`; rehearsed on a scratch copy of the card's tree (3 files written, 1 removed, 3 backed up, undo restored the original bytes). A read-only snapshot of the card's presets and DEV 213 persist file is at `Tau Alpha/work/omega-card-backups/20261107-015302-presets` (52 SHA-256s; equals the pinned fixture).
- Tau Alpha's audit (B-973) says the owner saw DEV 213 list DEFAULT plus the factory presets from files on this card; DEV 213 is labelled "PRESET READ PROBE". Not yet observed by anyone: the remembered id across a reboot, a vanished preset, hidden presets being skipped.

### Not done yet from the brief (nothing hidden)
- **Lab and WebAssembly (brief section 4):** waits for the new lab drop (R-025: built page, `lab-data.json`, every module with its SHA-256). Omega has no hand-written Layered Wave drawing left (the synthetic placeholder editor and its preview were deleted today), so there is nothing to replace; the live preview of a preset needs the page's input API (Q-031); pinning and hash checks of each module follow the drop (`tools/pin-tau-lab.sh` verifies `SHA256SUMS.txt`; per-module hashes in the manifest are not yet checked).
- **Pack descriptors v3 (brief section 3):** Omega reads only the manifest id from a descriptor (offset 32; same in v1 and v3 on the files seen); a full reading of `tau_packs_info.py`'s versions 1 to 3 is not done.
- **Schema forms:** the editor is generated from the registry, but from the `meters_schema.json` fixture on the dev server; the installed app needs `lab-data.json` (R-025).
- **Real Pocket end to end** (definition of done): planned, not run (needs the owner's yes). **Omega 0.6.0**: not released.

### Real-card result, 2026-11-07 (Omega wrote meter 12 to `Pock`; owner on TAU DEV 213) — **run on a Pocket**
Plan and write as in `docs/REAL_CARD_PRESETS_TEST.md` steps 2 and 3 (3 files written, 1 removed, 3 backed up outside the card; meters 13 to 18 byte-identical to the snapshot; Tau Alpha's `mpreset.py show` reads the three written files correctly). Owner-observed on the Pocket: the PRESET row showed the new set as written, **except that the hidden preset (SNAPPY) was still listed**; everything else was as expected (names, order, the deleted preset gone, the custom preset). Not yet read back by Omega: the persisted id after a reboot (steps 5 to 7 wait for the card to be back on the Mac).
**Cause (read in Tau Alpha's code):** `fw2/art/mpreset_host.c` `idx_name_at` copies only the entry's id and name; the flags byte (bit 0 = hidden) is never read, so the firmware does not apply `hidden` for preset files (as with `METR`, D-O03). Omega's writer is right (bit 0 of the entry's second byte, as `mpreset_core.h` and `mpreset.py` define it). **Owner's decision (D-022): hidden is Omega-only** (the Pocket has no hidden presets; a hidden preset is removed from the card and kept in Omega's library with its id reserved), so nothing is asked of the firmware and the brief's "exactly the visible presets" is read as "what is on the card". Omega never writes the bit; a card with the bit set reads as visible. F-032 was withdrawn in interop message 007.
**Also observed (host-read from the Pocket's file):** after the owner chose TEST ONE on WINAMP BARS and rebooted, DEV 213's `interact_persist.json` holds `MPID0` = 6 (preset id 5): the choice was remembered by id. Pinned as `testdata/persist_layout/dev213_after_omega_presets_interact_persist.json`; Omega's decoder shows it as "TEST ONE (id 5)" (the first real capture of a preset-file id).

**Step 6 (2026-11-07), a chosen preset disappears:** Omega removed TEST ONE (id 5) from `tau-presets/12/` (index rewritten, `p5.tpr` removed; the same index also cleared SNAPPY's stray hidden bit; Tau Alpha's `mpreset.py` reads the result as FLUID, CLASSIC 2, BOUNCY, SNAPPY). **Run on a Pocket (owner):** after booting DEV 213 the PRESET row showed **DEFAULT**. The persist-file re-read is **not claimed**: the owner changed the preset after the boot, so the file no longer shows the cleared state.
**Step 7:** the factory set was written back (`index.tpi`, `p1.tpr`, `p3.tpr`; backup outside the card); **host-read:** all 51 files of `tau-presets/` match the snapshot's SHA-256 list; stubs swept; card ejected.


## Card names (code page 437) and the name sidecar — checked 2026-11-07 (Tau Alpha `main` `01bd0ce4`)
**Read:** `to-omega/006` and `007`, `CARD_NAME_RULES.md` (`90dab1d1…0292`), `NAME_SIDECAR_SPEC.md` (`398d0a4e…7146`), `tools/name_sidecar.py` (`5d795dae…6ae5`), `tools/sync_media.py`, `tools/tau_library.py`, `sim/name_corpus.py` (`944542b8…8e0d`). **Tau Alpha untouched** (Python run with `-B -I`, `git status` clean).
**Evidence labels:** *host-read* = Omega's output compared with Tau Alpha's reference on this machine; nothing here ran on a Pocket by Omega.
- **Fact used (Tau Alpha, hardware B-991..B-1000, `TAU DEV 216`):** the Pocket opens a path as code page 437 byte for byte; a character outside it cannot be opened. **Omega before:** folded every accent to ASCII (safe, lossy), dropped other scripts to nothing, refused collisions, wrote UTF-8 into the index pool (wrong for any non-ASCII name; it never arose because names were ASCII).
- **Now (host-read):** `card_name` ports `card_part`/`NameMap`; the 62-case corpus passes the invariants; sidecar files are **byte-identical** to the reference writer on those cases; `latin1_text` agrees on 56 of 82 strings and differs only where a transliteration table decides. The index pool stores names as cp437 bytes and display text as Latin-1; the reader decodes cp437.
- **Differences to tell Tau Alpha:** (1) `deunicode` instead of `anyascii`/`pykakasi` (owner's choice): kanji as Chinese, soft sign as `'`, spaced CJK; (2) their and our finding ids clash (R-024, R-025, Q-026): proposal to prefix ids with the message number; (3) title and album-directory fallbacks keep the reference's ASCII fold.
- **Not done, nothing hidden:** a Pocket test with a card Omega wrote (names with `é`, a transliterated folder, the sidecar): needs the owner's go on `CARDWRITE`. CJK on screen (Tau Alpha's font study). Shift-JIS tag decoding (not auto-detected by the reference either). The CLI `build-index` verb does not write a sidecar (it writes to an arbitrary path). `tau_compat.py` listing of the two files is Tau Alpha's.
