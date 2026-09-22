# Roadmap: phases, extensions and ideas

Phases are ordered so each one is usable on its own and each later firmware phase has a place to land.

## App phases
| Phase | Deliverable | Exit test |
|---|---|---|
| **T0 Skeleton** | Workspace, CI on macOS/Windows/Linux, Tauri shell, card and folder detection, read-only Cards and Cores screens, core.json/data.json parsing, library-capability detection, fake-card fixtures. | Opens a real card and a fixture card; lists cores correctly; no writes anywhere. |
| **T1 Index engine** | `tau-core` scanner, tag readers, ASCII rules, index writer/reader/verifier, playlist rules, synth generator, `tau` CLI (`scan`, `index`, `verify`, `report`, `synth`). Golden files from the Python reference. | Byte-identical to Python on the golden set; all corruption cases give the same E-codes; the 7,180-track synth verifies. |
| **T2 Sync** | Plan/execute/verify/journal/manifest; sync wizard; cover embedding for MP3 and FLAC copies; optional cover optimisation; incremental runs; mirror with confirmation. | Sync a fixture library to a staging card; source hashes unchanged; index verifies; re-run is a no-op. First real-card run approved by the owner. |
| **T3 Multi-core** | Copy/move between cores and cards, compare cores, cleanup tools, backup/restore, numbered-core lifecycle (install new, migrate media, remove superseded after backup). | Copy A to B on one card and across cards; move with two-step delete; both indexes verify on the device. |
| **T4 Library workbench** | Library tabs, Problems with fixes, playlist editor, search/filter, device-accurate previews of the Pocket's lists. | The desktop lists match the Pocket screen order and letter jumps for the same index. |
| **T5 Core install** | Install/update cores from zip or folder, packaging checks, rbf reversal, diffs, known-builds table, two-zip install. | Installs the v0.3.0 pair from `release/*.zip` on a staging card exactly like the manual procedure. |
| **T6 Diagnostics** | Settings viewer with "last played", diagnostic record decoder, screenshot gallery, log viewer. | Decodes the recorded B-022/B-023 style records to the same JSON as `decode_tau_diag_log.py`. |
| **T7 Polish and release** | Installers, signing/notarisation, updater (opt-in), accessibility pass, documentation, sample data, crash reports (local only). | Release checklist in `TEST_PLAN.md` passes on both OSes. |
| **T8 Art (when firmware has the blit engine)** | Thumbnail pipeline, `tau-library-art.bin` writer (spec in `DATA_FORMATS.md` section 3), art id in the index header, preview of what the Pocket will show. | Firmware reads the file; index and art ids match; stale-art detection works. |

## TP Portability and Pocket Sync integration (added 2026-09-22)

TP0 sits **before T4**, because those defects get more expensive with every feature built on top.
Decisions D-009/D-010/D-011; findings and acceptance criteria in `docs/PORTABILITY_AUDIT.md`.

| Step | Deliverable | Exit test |
|---|---|---|
| **TP0 Boundary fixes** | Domain rules (card path prefix, index status) live only in `tau-core`; warnings and errors machine-readable, preserving E-codes; progress and cancellation hooks on scan/plan/execute. | A front-end branches on a failure without matching strings; a long scan reports progress and can be cancelled; no path or status logic remains outside the engine. |
| **TP1 API shape** | Optional `serde` feature on public types; no public entry point panics on caller-supplied input; `plan_with_*` collapsed into one options struct. | The Tauri `*View` DTO layer is gone; `build_index` returns `Err` on malformed caller data instead of panicking. |
| **TP2 Adoption** | `tau-core` consumed by Pocket Sync as a crate dependency (upstream contribution or fork). | Pocket Sync indexes a card using our engine, with its own UI driving plan → confirm → execute. |

Not doing: plugin ABI, C ABI, WASM, sidecar hosting, or a remote/streaming media-source abstraction.
Pocket Sync is Rust and reads local cards, so none of those are needed (D-009).

## Extensions worth building (pick by value)
1. **Card profiles / presets:** save a sync recipe (sources, options, target core) and rerun it in one click; per-card defaults.
2. **Watch mode:** watch a source folder and offer a sync when the card is next inserted ("3 new albums since last sync").
3. **Storage planner:** "will this fit?" with per-core budgets; suggest what to leave out; show the cost of covers.
4. **Smart playlists** built on the desktop from tags (recently added, by year, by genre) and exported as ordinary `.m3u` files the firmware already understands.
5. **Audiobook / long-file helper:** one-file playlists, chapter names, resume hints (relates to the firmware's resume feature).
6. **Loudness and gain tags (later):** analyse ReplayGain on the desktop, write gain into a sidecar or the index when firmware supports it.
7. **Duplicate finder** across sources and across cores; safe dedupe on copies only.
8. **Tag doctor:** guided fixes on the copies (missing track numbers so tiles are not `00`, multi-disc numbering, inconsistent album artists, "Various Artists" handling); optionally writes a sidecar rules file so re-syncs reapply the fixes.
9. **Format checker:** detects MP3 variants and FLAC settings the decoder cannot play in real time (48 kHz limit, block size, bit depth) and offers a copy-time conversion later.
10. **Multi-card sync:** the same library to several cards; a "clone card" job.
11. **Cores gallery:** show icons and platform artwork from the card, help write `core.json` fields for custom cores, validate JSON against Analogue's schemas.
12. **Save and memory management** for all cores: browse, back up, restore, prune.
13. **Firmware phase hooks:** a *capabilities file* per core version (`tau-caps.json` in the core folder or in Tau Omega's registry) so the app enables or hides features (art file, resume by second, index v2, new persisted words) for the firmware it finds.
14. **Send diagnostics (decided 2026-09-21, priority):** one click gathers the Tau cores' persisted-settings files, screenshots newer than the last test run, core/index/cold-image versions, card free space and filesystem and file hashes into one zip for support; decodes the on-screen report code (`tools/decode_tau_suite.py --code` logic ported to Rust) and shows the verdict before anything is sent; nothing is uploaded automatically. Earlier idea text follows. **Remote diagnostics:** export a zip with `interact_persist.json`, screenshots and the app's manifest for the developer ("send diagnostics"), matching the README's diagnostics section.
15. **CLI and scripting:** every feature available as `tau <verb> --json`, so the project's own scripts and CI can drive it (the existing `sync_media.py` workflow keeps working through a compatibility shim).
16. **In-app preview player:** play a track from the source or the copy to check tags and covers (optional, last).
17. **Localised UI, dark/light theme, Pocket-edition themed accent.**
18. **Plugin API** for third-party openFPGA cores that also read media (registry files plus a media-processor interface).

## Firmware and core phases the app must be ready for
* **Blit engine and thumbnails:** art file writer, art id, thumbnail sizes 32 and 92 (per the spec), preview.
* **Library resume by second, Diagnostic Build library page:** show them in the settings viewer once the words exist.
* **Cached PSRAM code / more RAM:** no app change, but new persisted words and diagnostic records will appear; the decoders must be table-driven so adding a field is data, not code.
* **Hardware audio and EQ:** EQ curves and presets in the settings viewer; later per-track gain.
* **Index v2** (genres, years, search keys, longer text, UTF-8): version-negotiated by the core's capability entry.
* **Other Analogue cores using the same media layout:** the core registry and the copy engine already handle them; only the indexer needs a per-family strategy.


## Added 2026-09-21 (tau-alpha B-063): Send diagnostics
See `DIAGNOSTICS_COLLECTOR.md`: read the Check's screenshots (QR) and persisted summary, decode with the Python reference format, and create one local zip for a bug report. Priority: with T6 (Diagnostics), before installers.
