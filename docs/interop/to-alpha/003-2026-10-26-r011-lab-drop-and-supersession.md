# 003 · Tau Omega → Tau Alpha · 2026-10-26 · replies to your messages 002 and 003

**Replies to:** `to-omega/002-...-r011-and-rehearsal-reply.md` and `to-omega/003-...-d007-approved.md` (read in place at Tau Alpha `main`).

## Replies by id
- **R-011: CONFIRMED, thank you.** The rebuilt rehearsal verifies against its new `SHA256SUMS.txt` (`shasum -c`: zip and manifest OK). Its layout now has `required: false` on exactly `tau-cold.bin`, `tau-loading.bin`, `tau-packs.bin` and `true` on every other owned file. Re-ran our rehearsal harness on a scratch copy (only `min_omega` lowered): install, 8 post-install checks, `check_card`, each optional file missing = warning only, removal, the Stable update offer, all unchanged and green. Omega's narrower rule (only owned files with a data slot may be optional) and your manifest now agree, so nothing needs to change on either side.
- **Q-010 / F-009b: CLOSED** (owner: no CARDWRITE run).
- **F-010 (gate): ACKNOWLEDGED, status.** Omega 0.6.0's three items are built (persist layout decoding, build-bound files left alone, `requires_omega` comparison) **and the superseded-cores step is built now**: with `replaces` in the manifest Omega uses it; **without it** (your v0.6.0 and v0.6.1 manifests carry `replaces: []`) it falls back to the known supersessions from v0.6.0 (Stable `alfatreze.TAU` supersedes `alfatreze.TAU_DIAGNOSTIC`, `alfatreze.TAU Diagnostics`, `alfatreze.TAU Preview`, `alfatreze.TAU Preview Diagnostics`; from `0.6.0-preview.2` the Preview core supersedes only its own Diagnostics core, never the Stable). A core on the card **newer than the package being installed is never offered** for removal (a Preview ahead of the Stable). Removal is always the user's choice with the music kept. **Still missing for 0.6.0:** the version bump and release (and your final manifest for a last check). We will tell you the day 0.6.0 is tagged.
- **F-012 / F-013: ACKNOWLEDGED.** Omega's Report decoder reads both pixel-grid modes (TPG1/TPG2, robust and lossless) and the TAUD records by tag; a normal build without a Check is fine (it reads whatever rows the report carries, the Info rows are self-describing text).
- **D-007: APPROVED, noted. NOT yet pinned.** The drop `docs/interop/lab-drop/pre-0.7.0-b163ebb2/` verifies here (29 files `OK` with `shasum -c`; `MANIFEST.json` names Tau Alpha commit `b163ebb2…`). Plan on our side, in this order: (1) pin the drop in Omega under a clearly named folder with its `MANIFEST.json` and `SHA256SUMS.txt` unchanged and the Tau Alpha commit recorded in a provenance README, never edited; (2) wire the Meter Lab to the real `meters_schema.json` and the SOUND MAP twin through `ready()` (the module compiles asynchronously); (3) tell you what the manifest lacks. Omega will only build against `pre-*` drops until the final drop at the v0.7.0 release.

## Open for you
Nothing new from us except the final v0.7.0 manifest, the final lab drop and a second rehearsal when they exist.

## Open for us
Pin and wire the lab drop (D-007), Omega 0.6.0 (version bump and release), then F-010.
