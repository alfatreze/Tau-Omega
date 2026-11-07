# Real-card test: card names and the name sidecar, written by Omega, on the Pocket (plan for the owner's yes, 2026-11-07)

**Status: plan only.** Nothing has been written. Every card write waits for the owner's yes after the plan of step 3 is shown.
Evidence labels as in `REAL_CARD_PRESETS_TEST.md`: what the owner sees on the Pocket is **run on a Pocket**; what Omega decodes from the card is **host-read**.

## What this settles
Tau Alpha's open item (006, B-1012): the rules and the sidecar have run on a Pocket only for a card **Tau Alpha's tools** wrote. Not yet run: a card **Omega** wrote (cp437 names from `card_name`, index text from `latin1_text`, `tau-names.json` + `.bin` from `name_sidecar`). The Pocket should show: no `?` titles, every file openable, `NAMES STATE 1` and `NAMES ERR 0` in the Report rows, library and Now Playing without error.

## The source
Tau Alpha's generator `tools/make_name_test_library.py` (55 files, one per case of `sim/name_corpus.py`; run with `PYTHONDONTWRITEBYTECODE=1 python3 -B -I`, output **outside** both repos, in the session scratchpad). Source files are only read. A short audio (`--seconds 10`, silent or a track of the owner's choice) keeps the copy small.

## Decision for the owner before step 3 (the one real risk on `Pock`)
`Pock` already holds **Tau Alpha's** run of this same library (55 files, its own index and sidecar). Omega's table (`deunicode`) differs from theirs for some scripts (kanji, soft sign), so for those cases Omega would write **different folder or file names** next to theirs, and the index and sidecar are rebuilt over the whole media root. Options:
- **A (recommended): a separate destination folder** (e.g. `Assets/tau/common/media/Omega Name Test/`, not the folder Tau Alpha's run used). The index covers both libraries; no file of theirs is touched, and removal afterwards is one folder (plan, backup, remove) plus an index rebuild.
- **B: the same destination.** Cases where both tables agree are unchanged; the others become additional files. Closest to "a card Omega wrote", but mixes the two sets and cleanup is harder.
- **C: `CARDWRITE` instead** (scratch card, nothing real on it). Cleanest, but it is not the card the owner has been testing on.

## Steps
1. **Read-only look at `Pock` (mount it).** Find the media root and the folder Tau Alpha's run used; list `tau-library.tdb`, `tau-names.json`, `tau-names.bin`; run `tau library-health` and `tau verify`. Free space. Nothing written.
2. **Snapshot outside the card** (to `~/Downloads/DEV PROJECTS/Tau Alpha/work/omega-card-backups/<timestamp>-names/`): the three generated files, the Tau Alpha test folder's listing with SHA-256, and `Settings/.../interact_persist.json`. Nothing written to the card.
3. **Plan (dry run).** `tau plan <generated library> --dest <media root>/<test folder> --no-convert` (files are already MP3) and show the plan: the file list with each card name next to its original, the collisions numbered, any file left out, the index and sidecar rebuild, the bytes. **If a name differs from what the corpus invariants allow (every result openable as code page 437, nothing collides, the `keep` text survives), stop and show it.**
4. **Write (owner's yes).** `tau sync ... --confirm <plan id> --yes --manifest <outside the card>`; the engine copies, rebuilds the index and writes both sidecar files (temp file, re-read, rename). Then, still mounted: `tau verify`, `tau library-health`, read the sidecar back with Tau Alpha's reader (`name_sidecar.parse_bin` and the `tau_library.verify`, read-only) and compare the sidecar's `index_build_id` with the index header offset 16. Sweep `._*` stubs, eject.
5. **On the Pocket (owner).** Boot the core the owner last used (TAU DEV 219 had the sidecar). Check: Library lists the new albums with no `?`; open a track from several categories (accents, Cyrillic, CJK, emoji, a numbered collision, an edge name such as `CON`); Now Playing shows the title and artist; Report rows **NAMES STATE 1**, **NAMES ERR 0**. Tell me anything odd (a title shown as `?`, a track that will not play, a crash).
6. **Back on the Mac (read-only).** If the owner wants it, decode the Report rows / persist file to record the evidence as host-read.
7. **Clean up (owner's choice).** Keep the test folder, or remove it with a plan, a backup outside the card and an index rebuild, then compare the generated files with the step-2 snapshot (the index and sidecar differ only by the rebuild; Tau Alpha's folder is byte-identical).

## If something goes wrong
The engine refuses before writing for a wrong token, a card that changed since the plan, a backup folder inside the card, or a name the Pocket cannot open. A failure mid-write restores the index and both sidecar files (the `GENERATED` backup) and says so. A pulled card mid-write leaves `*.part` files; the next plan says so. Any surprise on the Pocket: do not write again; read the Report rows, then restore from the step-2 snapshot.

## What it does not cover
CJK on screen (font; the index text shows the transliteration), Shift-JIS tags, audio behaviour, the preset items of message 007.

## Run notes (2026-11-07, option A chosen by the owner; steps 1 to 3 run, nothing written)
- `Pock`: media root `Assets/tau/common`, index valid (198 tracks, 0 missing files), Tau Alpha's run is in `Name Test Library/` (63 files with its own `playlist.m3u` per folder), 108 GiB free. Snapshot outside the card: `omega-card-backups/20261107-names-snapshot/` (index, both sidecar files, checksums, the persist files, `name-test-library.sha256`).
- A source folder's **contents** go to the media root, so the generated tree is wrapped in a folder named `Omega Name Test`; the categories then land under `Omega Name Test/`. (Without the wrapper they would land in the media root.)
- **Rehearsal on a scratch copy** (same command, destination outside the card): 55 files copied, 0 warnings, 0 skipped; index valid, 55 tracks, 0 missing; sidecar written (44 track records, JSON 9367 bytes, bin 2258 bytes). Tau Alpha's `name_sidecar.parse_bin` reads the bin, `index_build_id` equals the index header, `tau_library.verify` returns no problems, every path opens as code page 437, no case-insensitive collisions.
- **Against Tau Alpha's own names:** 43 of 55 paths identical; 12 differ only in transliteration (e.g. kanji read as Chinese `Dong Jing Ye Qu` vs their Japanese reader `DongJing YeQu`, `OEuvre` vs `Oeuvre`, `tm` vs `TM`). This is the expected table difference (D-007/R-025: compare invariants, not bytes); the sidecar `orig` keeps the truth.
- **Effect on the card's generated files:** the index and the sidecar are rebuilt over the **whole** media root by Omega (198 + 55 tracks). If Pock should go back to Tau Alpha's index and sidecar, restore the snapshot's three files.

## Run notes, write (2026-11-07, owner's yes)
- **Written:** `Omega Name Test/` with 55 files (8,990,542 bytes) by `tau sync` (verified, 55 copied); index 253 tracks, valid, 0 missing; both sidecar files rebuilt. Tau Alpha's `Name Test Library/` is byte-identical to the step-2 snapshot (checked twice).
- **Found on the real card, fixed (host-read; Tau Alpha's `tau_library.verify`):** `track 32` and `track 37: play order`. Cause: macOS lists accented names on exFAT decomposed, so Omega's `natural()` sorted `ö` as `o`; the index stores precomposed names and the reference sorts NFC. A rehearsal on APFS cannot show this. Fix `9ad5e82` (`natural()` takes NFC) with a regression test.
- **Index and sidecar rewritten** (`tau library-refresh`, backup of the three generated files outside the card in `omega-card-backups/20261107-names-refresh/`): the refresh plan used to say `nothing_to_do` for an index that was structurally valid but in a stale order; it now compares the stored track order with a fresh build (`RefreshPlan.index_stale`, test added). After the rewrite `tau_library.verify` returns no problems and the sidecar's `index_build_id` equals the index header (101 records; earlier originals were carried over by `earlier_origins`).
- **Known, not fixed:** a repeat `tau sync` of the same source plans 9 "update" files (the ones with accented names; the ledger's fingerprint does not match on exFAT). Harmless (the same copy is rewritten) but wasteful; to look at later. 9 `._` stubs stay under `Omega Name Test/`: they carry macOS's own `com.apple.provenance` attribute and cannot be removed from here; the card carried such stubs before.
- **Not yet run:** the Pocket part (steps 5 and 6): the owner boots TAU DEV 219 and checks the lists, playback and the Report rows `NAMES STATE` / `NAMES ERR`.

## Result (2026-11-07, owner on the Pocket, TAU DEV 219) — run on a Pocket
A card whose files, index and name sidecar **Omega wrote** (253 tracks, 55 of them Omega's `Omega Name Test/`, beside Tau Alpha's own run of the same library):
- The Albums list shows all 27 albums (screenshot `20261107_220252`); the names appear twice (Tau Alpha's folder and Omega's), as expected.
- Titles look right (no `?`); tracks from both copies open and play to their end (the clips are silent by design, about 9 s); the `cp437` album is in the right order (`Sæm Pi π Cost ¢` before `öüäëï Straße`, the order the NFC fix gave).
- Report rows: **`NAMES STATE 1`, `NAMES ERR 0`, `NAMES LOAD 53 MS`, `LIB LOAD 123 MS`** (the sidecar is loaded and accepted; Tau Alpha's own card gave 34 ms / 75 ms for 198 tracks and a 4.8 KB file).
- Host-read: Tau Alpha's `tau_library.verify` returns no problems on this index; `index_build_id` equals the index header.
- **Not covered:** CJK on screen (font), the track-level sidecar names shown on Now Playing (not looked at), Shift-JIS tags, real audio (silent files), `LIB TRACKS` value (not read).
- Left on the card for the owner to keep or remove (step 7): `Assets/tau/common/Omega Name Test/` (55 files), and the rebuilt `tau-library.tdb`, `tau-names.json`, `tau-names.bin`. Tau Alpha's three generated files before the test are in `omega-card-backups/20261107-names-snapshot/`.

## Step 7 done (2026-11-07, owner's yes): the test folder is removed
`Omega Name Test/` (55 files) was copied to `omega-card-backups/20261107-names-test-folder/` (hash-verified against the card), then removed from `Pock`; `tau library-refresh` rebuilt the index and sidecar (before: 253 tracks and 55 files missing; after: **198 tracks, 0 missing**, sidecar 51 records, `build_id` equal to the index, Tau Alpha's `tau_library.verify` returns no problems). Tau Alpha's `Name Test Library/` is byte-identical to the step-2 snapshot. Note: the index and sidecar on the card are now **Omega-built** (not Tau Alpha's); Tau Alpha's own three files from before the test are in `20261107-names-snapshot/` if the owner wants them back.

## The repeat-sync quirk (fixed in the ledger; not yet seen on a card)
A second `tau sync` of the same source planned 9 "update" files, the ones with accented names. Cause: the verification ledger keys every file by its path string. A sync records a copy under the precomposed name; a later scan of the exFAT card lists it decomposed, so the entry was lost (the complete scan dropped the "missing" precomposed key) and a converted copy (ReplayGain added) can never hash-equal its source, so the plan fell back to "update". Fix: every ledger key is NFC (lookup, record, scan, and keys loaded from an older ledger, where an entry that knows its provenance wins). Tests: `ledger::tests::a_file_is_one_entry_whichever_unicode_form_its_name_comes_in` and `an_older_ledger_with_both_forms_loads_as_one_entry_keeping_the_provenance`. **Verified by the unit tests only:** the real-card confirmation (a plan after a sync that shows the accented files as "same") needs a card sync, which was not done again; the next owner sync of accented names will show it.
