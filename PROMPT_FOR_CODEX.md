# Prompt for Codex: build Tau Omega

Paste everything between the two lines into Codex. Start with Phase T0 and T1, then continue phase by phase. Do not skip the reading step.

---

You are building **Tau Omega**, a cross-platform (macOS and Windows) desktop companion for the Tau music player core on the Analogue Pocket. Working directory: this folder (`Tau Omega`). The firmware repository `../tau-alpha/` is **read-only reference**: never edit it, never run anything from it that writes outside a temp directory.

## 1. Read first, in this order
1. `README.md`, `SPEC.md`, `ARCHITECTURE.md`, `DATA_FORMATS.md`, `SAFETY_RULES.md`, `ROADMAP.md`, `TEST_PLAN.md`, `REFERENCE_CODE.md` (all in this folder).
2. `../tau-alpha/docs/MEDIA_LIBRARY_0.4_SPEC.md` (index design), `../tau-alpha/tools/tau_library.py`, `../tau-alpha/tools/sync_media.py`, `../tau-alpha/tools/library_check.py`, `../tau-alpha/tools/decode_tau_diag_log.py`, `../tau-alpha/fw/library_core.h`, `../tau-alpha/sim/test_library_index.py`.
Do not re-derive what these record. If a document here and the Python reference disagree, the **Python reference wins**; write the discrepancy into `docs/DISCREPANCIES.md` and continue.

## 2. Product in one paragraph
A desktop app and CLI that (a) builds and syncs the Tau media library onto a Pocket SD card (scan music, read tags, ASCII-normalise names, embed and optimise covers in the copies, copy with verification, import playlists, build and verify `tau-library.tdb`), and (b) understands the whole card: it lists every core, shows which ones read the library, and copies or moves media between cores and cards, rebuilding each destination index (the index stores absolute paths, which include the platform folder). It also installs cores from zips, reads persisted settings and diagnostic records, and shows screenshots.

## 3. Stack (decided; deviate only with a written reason)
Tauri 2, a Rust workspace (`tau-core` library, `tau-cli` binary, `src-tauri` app), TypeScript + Svelte UI, SQLite (`rusqlite`) for the scan cache and journal. Layout and crates: `ARCHITECTURE.md`. The engine must work with **no UI** through the CLI; the UI only calls engine commands and renders events.

## 4. Hard rules (also in `SAFETY_RULES.md`; violating any of them fails the task)
* Never modify a source music file. All conversions happen on copies.
* Every write to a card or media root is preceded by a **plan** and needs an explicit confirmation; the CLI defaults to dry-run and needs `--yes`.
* Verify every written file (SHA-256 or byte compare); write the index **last** through a temp file, re-parse it with the reader you wrote, then rename.
* Deletes need a second confirmation and a visible backup or trash. Moves are copy, verify, then a separate confirmed delete.
* Only write inside Tau-owned paths (`Assets/<platform>/common/`, the chosen core's folder, its platform json and image), or where the user explicitly picks another core. Never touch `System/`, other authors' cores, `Saves/`, `Memories/`, `Settings/` except in the clearly labelled backup/cleanup tools.
* Tests never use a real card. Any test that writes under `/Volumes/*` or a drive letter must fail the build.
* No network access by default, no telemetry.
* Report honestly: partial or skipped work is stated, warnings are never hidden in a success message.

## 5. Conformance is the definition of correct
The index writer, reader/verifier, sort keys, ASCII rules, tag readers (ID3v2.3/2.4, ID3v1, FLAC Vorbis/STREAMINFO, MP3 Xing/CBR duration) and playlist import must reproduce `tools/tau_library.py` **byte for byte**. Build the golden set first:
* Write `testdata/gen_golden.py` that calls the Python reference (import `../tau-alpha/tools/tau_library.py`) to produce: the fabricated tree from `sim/test_library_index.py` plus its index; `synth` indexes at 400, 7,180 and 16,384 tracks (with and without playlists); the corruption matrix inputs and their expected E-codes; ASCII and sort-key vectors (a table of input strings and expected outputs).
* Check the generated files into `testdata/`. Rust tests compare bytes and codes against them. Regenerate only from Python.

## 6. Deliverables by phase (stop at each exit test, report, then continue)
**T0 Skeleton.** Workspace, CI (GitHub Actions: fmt, clippy, test on macOS, Windows, Linux), Tauri shell, card and folder detection, `core.json`/`data.json`/`interact.json` parsing, library-capability detection (`data.json` slot with filename `tau-library.tdb`), fake-card fixture generator in `testkit`, read-only Cards and Cores screens. *Exit:* lists the cores of a fixture card and of a real card (read-only) correctly.
**T1 Index engine.** Scanner with incremental cache, tag readers, ASCII/naming rules, index writer/reader/verifier with the firmware's E-codes, playlist rules, `synth`, CLI (`tau scan|index|verify|report|synth --json`). *Exit:* byte-identical to the golden set; identical E-codes on the corruption matrix; 7,180-track synth verifies.
**T2 Sync.** Plan/execute/verify/journal/manifest; sync wizard; cover embedding (MP3 APIC on ID3v2.3 copies, FLAC PICTURE), optional optimisation, folder-cover discovery (`cover.jpg`, `folder.jpg`, `front.jpg`, `cover-*.jpg`); incremental re-runs; mirror with its own confirmation; progress and cancel. *Exit:* fixture sync leaves sources byte-identical, index verifies, re-run is a no-op, injected failures leave the previous index intact.
**T3 Multi-core.** Core list with sizes and index health; copy/move between cores and cards (playlist line rewriting, index rebuild for the new platform folder, duplicate detection, two-step delete); compare cores; cleanup tools (`._*`, `.DS_Store`, the five System caches after backup); backups; numbered-core lifecycle. *Exit:* copy and move verified on two fixture cores and across two fixture cards.
**T4 Library workbench.** Artists/Albums/Tracks/Playlists/Folders/Problems views (virtualised), device-accurate ordering and letter groups, playlist editor, Problems with fixes. *Exit:* the desktop order equals the firmware order for the same index.
**T5 Core install.** Zip/folder install with packaging checks, diff, backup, cache cleanup, rbf reversal, known-builds table. *Exit:* installs the two v0.3.0 zips from `../tau-alpha/release/` (read them, do not modify) onto a fixture card identically to the manual procedure documented in `../tau-alpha/docs/SESSION_HANDOFF_2026-09-21_RELEASE_0.3.md` section 4.
**T6 Diagnostics.** Settings viewer with names and the Pocket edition colour names, "last played" from the history words, record decoders, screenshot gallery, log viewer. *Exit:* decoders equal `decode_tau_diag_log.py` on provided samples.
**T7 Release.** Installers, signing/notarisation scripts (secrets from CI only), accessibility pass, docs, sample data. **T8** (art) only when told the firmware has it.
Details, screens and acceptance criteria are in `SPEC.md` and `ROADMAP.md`.

## 7. Working method
* Small, reviewable commits; each phase on its own branch; tests first for the engine; no dead code; no TODO without an issue in `docs/ISSUES.md`.
* Keep `docs/` in sync with behaviour; when you decide something the spec left open, record it in `docs/DECISIONS.md` with the reason.
* Prefer hand-written code for the index writer/reader and tag fields that must match the reference; use crates for everything else after checking maintenance and licence (record in `docs/DEPENDENCIES.md`).
* Everything user-facing is ASCII-safe for the device and Unicode-correct on the desktop (show the original and the converted name).
* UI copy is short and direct. Destructive modals name the volume, core, file count and bytes.
* Performance targets and platform notes are in `ARCHITECTURE.md` and `SPEC.md` section 6.

## 8. What to report after each phase
What was built, test results (which tests, how many, which conformance files), anything skipped or failing, discrepancies from the Python reference, decisions taken, and what you need from the owner (a real-card run, a signing certificate, a design choice). Do not claim hardware results; the owner runs hardware steps.

## 9. First task
Do T0 and T1. Start by creating the workspace and `testdata/gen_golden.py`, prove the Python reference runs from here, generate the golden set, then write the Rust index writer and reader against it. Report before starting T2.

---
