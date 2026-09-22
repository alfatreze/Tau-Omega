# Tau Omega: product and functional specification

## 1. Goals
1. Make the Tau media library **easy and safe**: choose music, choose a card and a core, see a plan, confirm, get a verified result.
2. Understand the card: **list every core** (Tau and others), show what each holds, and **copy or move media between cores** with the right index rebuilt.
3. Give development phases a home: install and update cores, read diagnostics and persisted settings, review screenshots, and keep a history of what was done to which card.
4. Never put user data at risk (see `SAFETY_RULES.md`).

## 2. Users
* **Listener:** owns a Pocket, wants their music on it, plays it with Tau. Wants a wizard: pick folder, pick card, done.
* **Collector / power user:** several cores, several albums and playlists, wants control over names, covers, playlists and what goes to which core.
* **Developer (the project owner):** installs numbered test cores, syncs media to each, reads diagnostic records and screenshots, cleans up superseded builds, builds release zips.

## 3. Scope
In: macOS and Windows desktop app plus a CLI; SD cards and folders acting as cards; MP3 and FLAC (the formats Tau plays); the index format v1; playlist import; cover embedding and optimisation for copies;
multi-core management; diagnostics readers; installers and signing.
Out (for now): streaming services, transcoding, editing tags in the user's files, network sync, mobile apps, playing audio inside the app (a small preview player is an optional later extra).

## 4. Concepts and vocabulary
* **Card:** a mounted volume with `Cores/` and `Assets/`. **Core:** `Cores/<Author>.<Shortname>` with `core.json`. **Platform:** the `Assets/<platform>` folder a core reads.
* **Media root:** `Assets/<platform>/common/` of a core. **Library:** the index `tau-library.tdb` plus the music under its media root.
* **Library-capable core:** its `data.json` has a slot with filename `tau-library.tdb`. **Legacy core:** plays files and `.m3u` only.
* **Source:** a music folder on the computer (read-only). **Staging:** a folder acting as a card, for building and testing without a card.
* **Plan:** the exact list of operations and their sizes, produced before anything is written. **Job:** the execution of a confirmed plan.

## 5. Functional requirements

### 5.1 Card detection and the Cards screen
* Detect Pocket cards on insertion and removal; list them with label, capacity, free space, filesystem, number of cores. Remember cards; name them ("Abel's Pocket").
* Also allow **opening any folder as a card** (staging) and creating an empty card skeleton for testing.
* Show warnings: read-only volume, low space, exFAT/FAT32 limits, macOS metadata files, unexpected structure.

### 5.2 Cores screen (multi-core)
* List every core on the card: author, name, version, date, platform id, description, size on disk, media size, whether library-capable, whether an index is present and its
  counts and age, whether the index matches the media (see 5.5 Health).
* Identify **Tau family** cores (author `alfatreze`, shortname starts `TAU`), numbered test builds `TAU_PSRAM_NN`, and the base release. Show other openFPGA cores as read-only rows with their media roots.
* Per core actions: open in Library view, sync media, rebuild index, verify, copy media to another core, move media, clear media, remove core (with backup), view persisted settings, view screenshots and logs.
* **Copy / move between cores:** choose source core and destination core (same card, or another card/staging). Pick everything or a selection (folders, albums, playlists).
  The engine copies files (verified), rewrites playlist lines, rebuilds the destination index (the root prefix changes with the platform folder), and for a *move* deletes the source only after a second confirmation.
  Cross-card copy is the same job with two volumes. Detect duplicates by hash and skip them. Preview shows bytes to write and free space after.
* **Compare cores:** side-by-side of two cores' media roots (only in A, only in B, different), useful for keeping test cores in step.
* **Numbered test cores:** the app knows `TAU PSRAM NN` are never reused; when installing a new numbered core it offers to remove superseded ones after a verified backup, and copies media to the new one automatically (the developer's standard step).

### 5.3 Library screen (per core)
* Tabs: Artists, Albums, Tracks, Playlists, Folders, **Problems**. Virtualised lists, search, sort, filters. Shows exactly what the Pocket will show (same sort keys and letter groups) so the desktop view predicts the device.
* Detail panes: tags as read, ASCII form that will be used, path length, duration, format, bitrate, cover (size, dimensions, baseline/progressive, embedded or folder), warnings.
* **Problems:** files the player cannot use or will handle badly: progressive JPEG covers, covers over 2 MiB, covers that decode slowly (compressed size), FLAC above 48 kHz or 24-bit issues (from the firmware's limits), non-ASCII names that will be converted, name collisions after conversion, paths over 200 bytes, missing tags (tile will show 00), MP3s with unknown duration, files over 4 GiB on FAT32, duplicate tracks. Each problem has a suggested fix and an "apply to copies" action where one exists.
* Playlists tab: list, create, rename, reorder by drag, add from tracks, import `.m3u`, export `.m3u` for the core. Lists are stored as `.m3u` files in the media root (the tool never invents a private format). A list identical to its own folder is flagged "album list" and not imported (matches the index rule).

### 5.4 Sync wizard (music folder to a core)
1. Choose sources (folders/files; drag and drop) and the destination core.
2. Options: ASCII names (on, cannot be turned off for library builds), embed folder cover into copies (default on), optimise covers (max side, JPEG quality; default keeps originals), generate album playlists (off by default now that the library replaces them), import playlists (on), mirror (delete files in the destination that are not in the sources; off, needs its own confirmation), include/exclude patterns, keep folder structure or reorganise as `Artist/Album`.
3. **Plan:** counts and bytes of new, updated, identical and skipped files; renames (ASCII); warnings; the index that will result (tracks, albums, artists, playlists, size); free space after; estimated time. Nothing is written yet.
4. Confirm; run with progress and cancel; per-file verify; index built last and verified with the loader logic.
5. Report: what was written, what was skipped and why, warnings, the manifest saved on the host, and the reminder to eject. "Copy report" and "Open folder" buttons.
* Incremental: unchanged files (size, mtime, hash) are skipped; changed covers only are re-embedded.
* Interrupted runs are safe: re-running continues; a half-written index is never left in place.

### 5.5 Health and verification
* **Verify card:** re-hash media against the manifest, parse the index with the reference loader logic (all E-codes), check every indexed path exists and is within limits, check the art file id when present, check that `data.json` has the library slot, check the platform json and image exist.
* **Index freshness:** compare the index build id to the media (count, sizes, mtimes) and say "out of date" with a one-click rebuild.
* **Cleanup tools:** remove `._*` and `.DS_Store`, clear the five System caches (after backup), list orphans (`Assets/<platform>` folders with no core, cores with no platform image).

### 5.6 Cores install and update (developer features)
* Install a core from a release zip (`<Author>.<Core>_<Version>_<Date>.zip`) or from a build folder; validate against the packaging rules (only `Cores`, `Platforms`, `Assets`; folder equals `author.shortname`; version equals `core.json`; `bitstream.rbf_r` present and, when the raw `.rbf` is supplied, equal to its bit reversal), show the diff against what is installed, back up what will be replaced, install, verify, clear caches, offer to eject.
* Remove a core with everything that belongs to it (core, platform json and image, assets, settings) after a visible backup.
* Two-zip release helper: given the two release zips, install both (normal and diagnostic) side by side.
* Bitstream tools: reverse bits `.rbf` to `.rbf_r`, hash both, show which build a card holds by SHA-256 against a known-builds list.

### 5.7 Diagnostics, settings and screenshots
* **Settings viewer:** read `Settings/<core>/Interact/_core/interact_persist.json` and show named values (volume, colour edition name, repeat, shuffle, meter, EQ, resume, library switch), plus **"last played"**: decode the history words and resolve them against the matching index.
* **Diagnostic records:** decode the 64-byte diagnostic and PSRAM/SDRAM records stored in persisted words (same output as `decode_tau_diag_log.py --interact ...`), with PASS/FAIL and the evidence label.
* **Screenshots:** gallery of `Memories/Screenshots/*.png` per core (file time and the core that was running when known), zoom, copy, export, annotate later. Reads only.
* **Logs:** open `System/Logs`.
* **Backups:** back up and restore `Saves/`, `Memories/`, `Settings/` and whole cores to a chosen folder with verification.

### 5.8 Settings of the app
Theme (the 19 Pocket edition colours), default sources, default core, language, plan-only default, confirmation strictness, cache location, update checks (off by default), log level and "copy diagnostics".

## 6. Non-functional requirements
* **Correctness first:** index bytes identical to the Python reference for the same inputs; playlists, sort order and ASCII text identical.
* **Performance:** scan and hash 7,180 tracks (about 50 GB) with progress inside minutes on a SATA SSD to card copy limited by the card; tag reading does not read whole files; UI stays responsive (jobs off the UI thread); lists of 16,384 rows scroll smoothly.
* **Resilience:** survive card removal mid-job (detect, stop, report, no corruption of the previous state); survive sleep/wake; handle read errors per file.
* **Accessibility:** keyboard navigation, screen-reader labels, colour-contrast checks for all 19 accent colours (the firmware has known contrast issues on dark editions: do not copy them).
* **Privacy and offline:** see `SAFETY_RULES.md`.
* **Localisation:** ASCII text on the device; the desktop UI can show original (non-ASCII) tags and the converted form side by side.
* **Logging:** structured logs, a per-job journal and the manifest; nothing sensitive beyond file names.

## 7. Screens (wireframe list)
1. **Home / Cards:** cards as tiles, "Open folder as card", recent jobs.
2. **Card overview:** cores table, capacity bar (media, cores, free), warnings, actions (Cleanup, Backup, Verify).
3. **Core detail:** media summary, index card (counts, age, health), actions (Sync, Copy to..., Rebuild, Verify), persisted settings, screenshots.
4. **Library:** tabs and detail panes as in 5.3.
5. **Sync wizard:** 4 steps as in 5.4 with the plan table and confirmation.
6. **Copy/Move dialog:** source and destination cores, selection tree, plan and confirmation.
7. **Problems list:** grouped, with fix actions.
8. **Install core:** zip or folder, diff view, backup path, confirmation.
9. **Diagnostics:** settings viewer, records, screenshots, logs.
10. **Job history:** every job with its manifest, journal and undo where possible.
11. **App settings.**
Modal rules: every destructive or card-writing modal states the target volume and core, the number of files and bytes, and requires typing or a deliberate second click for deletes.

## 8. Acceptance criteria (release 1.0)
* Syncing the owner's real library (about 7,180 files) to a test core on a card yields an index the firmware loads (`LIBRARY 7180 TRK ...` on the Info page) and every file plays.
* The Rust index writer produces byte-identical output to the Python reference on the whole golden set, and rejects/accepts corrupted indexes with the same E-codes.
* Source folders are untouched (checked by hash before and after in tests).
* Copy and move between two cores on one card, and between two cards, produce working indexes on both sides; a move never deletes before verification and confirmation.
* Every card write in the UI is preceded by a plan and a confirmation; the CLI defaults to dry-run.
* Signed, notarised macOS build and a signed Windows installer; no network calls unless the user turns on update checks.
