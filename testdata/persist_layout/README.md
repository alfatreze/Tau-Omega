# Persist layout fixtures (real artefacts, copied read-only from Tau Alpha)

Source: Tau Alpha `docs/research/persist_layout/` (golden vectors and captures) as of Tau Alpha `main` 2026-10-26, reference `tools/persist_layout.py`.
Used by `crates/tau-core/src/persist_layout.rs` (the Rust port is checked against them) and `src-tauri/src/diag_cmds.rs`.

| File | What it is |
|---|---|
| `golden_vectors.json` | Tau's golden vectors: 37 legacy register files and the words their migration writes, 32 interrupted-migration cases, the field table. |
| `dev181_interact_persist.json` | A real **legacy-layout** file (TAU DEV 181: volume 73, colour 7, meter 6, Space Grotesk, presets, last played). |
| `dev182_...`, `dev184_...` | Real **new-layout** files (they started from empty settings, so they are not migrations). |
| `dev185_after_migration_interact_persist.json` | The file TAU DEV 185 wrote after migrating `dev181` on a Pocket (hardware-confirmed 2026-10-26, "SET MIGRATED 1", second boot "0"): equals the reference migration of `dev181` word for word. |

What is NOT hardware-proven (Tau Alpha's own note): other legacy files, and a power loss in the middle of a migration (host-proven only).
