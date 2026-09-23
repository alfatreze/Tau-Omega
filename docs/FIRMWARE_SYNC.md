# Firmware sync check — tau-alpha

Tau Omega writes files that Tau's firmware reads, so its assumptions go stale whenever tau-alpha
ships. This is the standing record of when they were last checked and what is still open. **Re-run it
after every tau-alpha release**, and when the blit engine lands.

Last checked **2026-09-22** against tau-alpha **v0.4.0** (Phase G cold code shipped; Phase F blit
engine is that project's active work item).

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

## Open conflicts — tau-alpha must decide

1. **Data slot 6 is double-booked.** `MEDIA_LIBRARY_0.4_SPEC.md` section 3 reserves slot 6 for
   `tau-library-art.bin`; Phase G shipped the cold image `tau-cold.bin` in slot 6, and it is in the
   released core today. The art file has no slot. Live, because the blit engine is next.
2. **Art pixel format unconfirmed.** The firmware spec says "RGB565 assumed; verify against
   `fw/art.inc` before freezing". Do not encode thumbnails until it is frozen.

Both are recorded in `DATA_FORMATS.md` section 3 next to the design they affect.

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
