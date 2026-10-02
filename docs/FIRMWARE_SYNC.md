# Firmware sync check — tau-alpha

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
- **Check/QR tags:** firmware enum is now `1..22` (`SR_T_BLITTEST` 15 through `SR_T_DECPROF2` 22). `taud.rs` decodes 1-14; the rest are kept as unknown entries
  and shown raw, not dropped. Open: decode 20 (METERCFG), 21 (METERTRACE), 22 (DECPROF2) and 17-19 from real captures.
- **Meters:** Layered Wave is meter id 16; the Meter slider range grew. Meter Lab still uses the synthetic schema; swap to the real `meters_schema.json` (open).
- **`tau-assets.bin`:** container unchanged in layout; `fw/assets_core.h` has a 3-line change after the tag, not yet read. The format is still not frozen until a card reads a real file.
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
