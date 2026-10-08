# Real Check/QR screenshot fixtures

Copied verbatim from the sibling `tau-alpha` repo's `work/diagnostics/check-qr-2026-09-21/
screenshots/` (2026-09-23), which in turn were recovered from the Pocket's own `Memories/Screenshots`
save folder on the mounted card — not written or rendered from memory of the format. Byte-identical
to the card originals (`cmp`-verified on the tau-alpha side before this copy). Same fixture policy as
`testdata/interact_persist/README.md` and `testdata/packages/README.md`: copied from a real artefact.

This closes the gap `docs/STATUS_HANDOFF.md` and `docs/FIRMWARE_SYNC.md` previously described as "no
real hardware screenshot fixtures found yet" — that was wrong; the Pocket's screenshot folder had
never been checked. See `tau-alpha/docs/AUDIT_TRAIL.md` B-133 for the full recovery story and
`tau-alpha/work/diagnostics/check-qr-2026-09-21/screenshots/README.md` for the file-by-file mapping
against that repo's own audit entries (B-058, B-060, B-062, B-067, B-069).

## What's here (13 files, all real Analogue Pocket screen captures)

Three kinds of Check page, across four different runs:

- **Progress page** (`20260921_224303.png`) — a Check in progress (`CHECK RUNNING`).
- **Result pages** (`20260921_220631.png`, `20260921_224310.png`, `20260921_230855.png`,
  `20260921_231104.png`, `20260921_231815.png`) — plain text pass/fail summary plus a 36-character
  short code. Covers both `ALL CHECKS PASSED` and two distinct real failures: an `SDRAM read/write
  cost FAIL` and a `PLAYBACK FAIL` (one late underrun, tau-alpha's own still-open finding).
- **QR pages** (`20260921_220638.png`, `20260921_224317.png`, `20260921_230900.png`,
  `20260921_231110.png`, `20260921_234254.png`, `20260921_234957.png`) — the full `TAUD1:` record,
  base64url-encoded, one QR per run. Versions 9-11 across these six, both run 1 and run 2 of the same
  session, and three different profiles (the base seven tests only; the base seven plus `STRESS
  R1-R3`; and the base seven plus `STRESS R1-R3` and a 5-minute soak).
- **One non-Check screenshot** (`20260921_231822.png`) — the now-playing screen, captured by a
  since-fixed firmware bug (Menu+Start closed the settings menu instead of opening the QR page for
  that run). Kept because it's a real artefact of a documented bug, and separately usable as a plain
  "now playing" UI fixture if one is ever needed.

## Why this matters for Tau Omega

This is the fixture `STATUS_HANDOFF.md`'s deferred item ("Screenshot/log discovery... no real
hardware screenshot fixtures found yet") needed. It unblocks two things, neither built yet:

1. **Decoding a photographed/screenshotted QR page** into the same `TAUD1` report
   `tau_core::diag::read_check_summary` already reads from the tiny 4-word persisted summary — the
   QR carries the *full* report (per-test values, not just the pass/fail verdict). This needs an
   image-decoding + QR-reading crate, which is a real dependency-cost decision (per
   `DECISIONS.md`'s existing `fs4`/`zip` precedent) to make explicitly with the owner before adding.
2. **General screenshot discovery**, if a future feature wants to find/import Pocket screenshots from
   a card automatically.

Every QR page here was cross-checked with tau-alpha's own `tools/decode_tau_suite.py --qr` before
being copied in, so the expected decoded values are already known and documented on the tau-alpha
side — a future Rust decoder here can be tested against these exact files with known-correct output.


## Added 2026-10-02: captures for the newer QR tags (8 files)

Copied byte-for-byte (`cmp`-verified) from the mounted Pocket's own `Memories/Screenshots`, read-only. All are QR pages from builds after
the 2026-09-21 set above. Expected values in the `taud.rs` tests were cross-checked against tau-alpha's `tools/decode_tau_suite.py --qr --json`.

| File | What it is | Firmware |
|---|---|---|
| `20260927_005032.png`, `20261002_191502.png` | Info-page export (`SR_T_INFOEXPORT`, tag 19; profile "none", no tests) | 0.4.0, 0.6.0 |
| `20260927_165345.png` | Winamp Scope meter config (`SR_T_METERCFG`, tag 20) | 0.5.0 |
| `20260927_232959.png` | Blit Test run with 20 recorded meter frames (`SR_T_METERTRACE`, tag 21; profile 8) | 0.5.0 |
| `20260927_233104.png` | USER CHECK with a stack reading (`SR_T_STACK`, tag 16) | 0.5.0 |
| `20260928_115822.png` | decode-stage split, **4-field** form (earliest, kept raw) | 0.5.0 |
| `20260928_135722.png` | decode-stage split, **6-field** form (after B-361) | 0.5.0 |
| `20260929_000253.png` | decode-stage split, **7-field** form (after B-381, adds worst LPC call) | 0.5.0 |

No capture exists yet for tag 15 (Blit Test results), 17 (Winamp config export) or 18 (Meter Sweep); those stay "not decoded" until one does.

## TPG1 pixel-grid captures (2026-10-04)

Three real Pocket screenshots of the **same** USER CHECK report (231-byte record), from `TAU_DEV_BARCODE_02` (tau-alpha branch
`barcode-study`, `TAU_TPG=1`), copied byte-for-byte from the card's `Memories/Screenshots`:

| File | Shows |
|---|---|
| `20261004_190900.png` | the ordinary QR code (the reference: decodes with `--qr`) |
| `20261004_190911.png` | TPG1 **mode L** (lossless): the record in the first pixel row, RGB565 values |
| `20261004_190919.png` | TPG1 **mode R** (robust): 4x4 cells, 2 bits per channel, 24 pixel rows at the top |

tau-alpha's `tools/decode_tau_suite.py --grid <file>` decodes both grids to the same record as the QR code, byte for byte (checked
2026-10-04). The format is defined in tau-alpha's `tools/tpg.py` docstring and `docs/features/BARCODE_STUDY.md`; it is **not frozen**
until more real captures exist. A Rust decoder here must be tested against these files, not against a fixture written from the doc.

## TPG2 captures (2026-10-04, `TAU_DEV_BARCODE_04`)

tau-alpha moved from `TPG1` (stream from pixel (0, 0), the three captures above) to `TPG2`: the stream is a **centred square block** whose side
comes from a fixed ladder, and **robust mode is the default view** (lossless when the report is bigger than 6,059 B). Six real captures, two reports,
each shown in all three views, copied byte-for-byte from the card's `Memories/Screenshots`:

| Report | Robust grid | Lossless grid | QR code |
|---|---|---|---|
| Info page export (883-byte record, all 34 Info rows) | `20261004_194758.png` (160 px block) | `20261004_194804.png` (64 px block) | `20261004_194810.png` (QR version 25) |
| USER CHECK with context (421-byte record) | `20261004_194856.png` (112 px block) | `20261004_194901.png` (64 px block) | `20261004_194907.png` (QR version 16) |

For each report the three records are byte-identical (checked with tau-alpha's `tools/decode_tau_suite.py --grid` and `--qr`, 2026-10-04); the
Info export decodes to 34 rows (`--grid <file> --table`), the Check one carries the now-playing entry. An Omega decoder should support both
layouts and be tested against these files. Format: tau-alpha `tools/tpg.py` docstring and `docs/features/BARCODE_STUDY.md`; still **not frozen**.
