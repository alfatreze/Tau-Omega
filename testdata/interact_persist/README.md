# Real interact_persist.json fixtures

Copied verbatim from real Analogue Pocket hardware captures in the sibling `tau-alpha` repo's
`work/diagnostics/` tree (2026-09-23), not written from memory of the format — see
`docs/FIRMWARE_SYNC.md`'s closing lesson: *fixtures for anything the firmware or APF produces must
be copied from real artefacts, not written from memory of the format.*

- `check_all_passed.json` — `work/diagnostics/library-0.4/card-replaced-dev32-2026-09-22/core28/
  Settings_28/Interact/_core/interact_persist.json`. Persist ids 20-23 decode as a Check summary:
  `USER CHECK`, run 5, all checks passed.
- `check_some_failed.json` — `work/diagnostics/library-0.4/card-replaced-17-2026-09-21/core16/
  Settings_16/Interact/_core/interact_persist.json`. `USER CHECK`, run 2, SDRAM window test and
  SDRAM read/write cost failed.
- `legacy_no_check.json` — `work/diagnostics/library-0.4/card-replaced-dev33-2026-09-22/core32/
  Settings_32/Interact/_core/interact_persist.json`. Persist ids 20-23 hold this core's legacy
  playlist state instead (`docs/FIRMWARE_SYNC.md`'s overloaded-ids trap) — a real example of the
  case `decode_check_summary` must reject rather than misdecode.

All three ground-truth decodes were cross-checked against `tau-alpha/tools/decode_tau_suite.py
--interact <file> --json`, the Python reference this project's own decoder is a byte-exact port of.
