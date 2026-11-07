# Preset file fixtures (real artefacts, copied read-only from Tau Alpha and from the owner's card)

Source: Tau Alpha `main` at `4c610d63d4fe13dd245242663f915e5e495f4b4b` (2026-11-07), read-only; card files copied 2026-11-07 from the
owner's Pocket card `Pock`, `Assets/tau/common/tau-presets/` (installed there by Tau Alpha's `tools/install_factory_presets.py`, TAU DEV 213 era).

| Path | What it is |
|---|---|
| `meters_schema.json` | Tau Alpha `tools/meters_schema.json` (the registry: ids, parameters, ranges, defaults). |
| `meters/<key>/presets.json` | Tau Alpha `meters/<key>/presets.json`, the factory presets of the seven Player 2 meters that have parameters. |
| `card/tau-presets/<meter id>/index.tpi`, `p<id>.tpr` | The 51 real files on the card (the 58 `._*` AppleDouble stubs macOS left beside them were not copied). Proven identical to the output of Tau Alpha's `mpreset.py factory` run on the same commit. |

Used by `crates/tau-core/tests/preset_files.rs`: the card files must be parsed by Omega's reader, regenerated **byte for byte** by Omega's writer
from the parsed set, and also from `presets.json` through `factory_set`. The decoded persist capture is `../persist_layout/dev213_interact_persist.json`
(see its README row).

Hardware status: these files were *read by a Pocket* only in the sense that the card held them while TAU DEV 213 ran; no run on a Pocket that
reads a preset written by Omega has happened yet (see `docs/FIRMWARE_SYNC.md`).

| `card_after_omega_write/tau-presets/12/*` | Meter 12's six files as they were on `Pock` after Omega wrote the real-card test set (2026-11-07, `docs/REAL_CARD_PRESETS_TEST.md`): FLUID, CLASSIC 2, BOUNCY, (hidden) SNAPPY, TEST ONE. Written by Omega; read by Tau Alpha's `mpreset.py show`; shown on the Pocket by TAU DEV 213 (SNAPPY was still listed: the firmware ignores the hidden flag). |
