# Tau Omega

A desktop companion (macOS and Windows) for the **Tau** music player core on the Analogue Pocket. It builds and syncs the
**Tau media library** onto the Pocket's SD card, and manages Tau (and other openFPGA) cores and their media: it finds every
core on the card, shows what each one holds, and copies or moves media between them safely.

The Tau core cannot list a directory, so a host tool must build an index and put the files where the core expects them. Today that
is two Python scripts (`tau-alpha/tools/tau_library.py`, `tau-alpha/tools/sync_media.py`). Tau Omega is the product version of them
with a UI, plus the multi-core and card management the scripts do not have.

## Documents in this folder (read in this order)
| File | What it is |
|---|---|
| `PROMPT_FOR_CODEX.md` | The prompt to give Codex, phase by phase. Start here to run the project. |
| `SPEC.md` | Product and functional specification: users, requirements, screens, behaviours, non-goals. |
| `ARCHITECTURE.md` | Recommended stack (Tauri 2 + Rust core + TypeScript UI), crates, module layout, packaging, signing. |
| `DATA_FORMATS.md` | Authoritative byte formats: `tau-library.tdb`, the future art file, manifests, card and core layouts, persisted-settings words. |
| `SAFETY_RULES.md` | Rules for anything that touches an SD card or a music folder. Non-negotiable. |
| `ROADMAP.md` | Phases, extensions and ideas for later firmware and core phases (art, blit engine, PSRAM code, more cores). |
| `TEST_PLAN.md` | Conformance against the Python reference, fixtures, fake cards, UI tests, release checks. |
| `REFERENCE_CODE.md` | Map from every function Tau Omega must reproduce to the existing Python and firmware source. |

## Portability goal
`tau-core` is a UI-independent engine, kept reusable so it can also be adopted by **Pocket Sync**
(the established third-party Analogue Pocket manager) as an ordinary Rust crate dependency. That goal
and its licence boundary are recorded in `docs/DECISIONS.md` (D-009, D-010); the current gaps and the
work to close them are in `docs/PORTABILITY_AUDIT.md`.

## The existing project (read-only reference)
`../tau-alpha/` is the firmware, RTL, tools and docs repository. Do not modify it from Tau Omega work. The documents that matter:
`tau-alpha/docs/MEDIA_LIBRARY_0.4_SPEC.md` (index design and decisions), `tau-alpha/docs/MEDIA_LIBRARY_0.4_BRIEF.md`,
`tau-alpha/tools/tau_library.py`, `tau-alpha/tools/sync_media.py`, `tau-alpha/tools/library_check.py`,
`tau-alpha/fw/library_core.h` (the firmware reader that must accept what Tau Omega writes), `tau-alpha/sim/test_library_index.py`
and `tau-alpha/sim/test_library_fw.py` (the tests that define correct output).

## One-paragraph summary of what it does
Point it at a music folder (or several) and at a Pocket SD card. It scans, reads tags, normalises names to ASCII, optionally embeds
and optimises cover art in the copies, copies the music into a chosen Tau core's `Assets/<platform>/common/` folder, imports
your playlists, builds and verifies `tau-library.tdb`, and reports exactly what it changed. It also lists every core on the card,
shows which ones understand the library, and moves or copies media between them, rebuilding each destination's index because the
index stores absolute card paths.
