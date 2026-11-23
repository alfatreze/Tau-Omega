# Persist layout fixtures (real artefacts, copied read-only from Tau Alpha)

Source: Tau Alpha `docs/research/persist_layout/` (golden vectors, captures and the staged layout-2 files), reference `tools/persist_layout.py`. **`golden_vectors.json` was recopied 2026-11-23** (Tau Alpha `1088e776`, their message 031: **persist layout 2**, every shipped meter's `params_rev` is 1; `cases`/`resume` are layout-0 cards migrated to layout 2, `cases_v1`/`resume_v1` layout-1 cards, with the shipped meters' ids and a fixture of their parameter tables); the captures are from 2026-10-26 and 2026-11-07. `staged/` is Tau Alpha's `staged/` folder (11 files, byte for byte, its README has their SHA-256s): the real DEV 181 and DEV 185 captures, the layout-2 file the migration must make of each (both give the same file) and the same inputs cut after 3, 4 and 10 writes.
Used by `crates/tau-core/src/persist_layout.rs` (the Rust port is checked against them, including every `cases_v1` tuning), `crates/tau-core/tests/preset_files.rs` and `src-tauri/src/diag_cmds.rs`. Layout 1 = TAU DEV 182 to 219, v0.7.0, v0.7.1; layout 2 = v0.7.5.

| File | What it is |
|---|---|
| `golden_vectors.json` | Tau's golden vectors: 37 legacy register files and the words their migration writes, 32 interrupted-migration cases, the field table. |
| `dev181_interact_persist.json` | A real **legacy-layout** file (TAU DEV 181: volume 73, colour 7, meter 6, Space Grotesk, presets, last played). |
| `dev182_...`, `dev184_...` | Real **new-layout** files (they started from empty settings, so they are not migrations). |
| `dev213_interact_persist.json` | A real new-layout file from TAU DEV 213 (copied 2026-11-07 from the owner's `Pock`): volume 94 (the default: the saved word is 0), METER 3, no preset file chosen (MPID bits all 0), so it does **not** exercise the preset-id fields. |
| `dev213_after_omega_presets_interact_persist.json` | The file TAU DEV 213 wrote on the owner's Pocket **after Omega wrote meter 12's presets** (copied 2026-11-07 from `Pock`): the owner chose preset id 5 (`TEST ONE`) on WINAMP BARS and rebooted; word 6 holds `MPID0` = 6 (id + 1). **The first real capture of a preset-file id**; hardware-observed. |
| `dev185_after_migration_interact_persist.json` | The file TAU DEV 185 wrote after migrating `dev181` on a Pocket (hardware-confirmed 2026-10-26, "SET MIGRATED 1", second boot "0"): equals the reference migration of `dev181` word for word. |

What is NOT hardware-proven (Tau Alpha's own note): other legacy files, and a power loss in the middle of a migration (host-proven only).
