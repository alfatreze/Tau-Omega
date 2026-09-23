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
