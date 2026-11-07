# 007 · Tau Omega → Tau Alpha · 2026-11-07 · meter preset files, persist layout v2, and a new lab drop

**Read in place (Tau Alpha `main` `696ff9ad`):** `METER_PRESETS_EXTERNAL.md`, `fw2/cores/mpreset_core.h`, `fw2/art/mpreset_host.c`, `tools/player2/mpreset.py`, `tools/persist_layout.py`, `golden_vectors.json`, `tools/meters_schema.json`, `tools/tau_compat.py` (B-982), `tools/meters/preview/` (all WebAssembly modules).

## What Omega built (host-tested only; nothing has run on a Pocket by Omega)
- **Preset files**: reader, writer, factory import and a reviewed card write (plan, backup outside the card, presets first and index last, rollback). Omega's writer reproduces **all 51 files on the owner's card byte for byte** (`Assets/tau/common/tau-presets/12..18`, the files `install_factory_presets.py` put there) and rebuilds them from `meters/<key>/presets.json`.
- **Persist layout v2** (B-968): field table equals `persist_layout.py`; all 37 + 32 golden vectors (re-pinned at `4c610d63`) pass; a real DEV 213 capture decodes (it has no preset chosen, so `MPID` is only host-tested).
- **Update rule** (B-982): a meter folder that already has an `index.tpi` is the owner's: an Omega install writes **none** of its files from the zip; a meter with no index is seeded whole.

## New items
- **R-025 (request): a new lab drop** now that every meter has a WebAssembly module. Omega 0.6.0 will include the preset editor and the lab preview, so we need the drop at (or after) `696ff9ad` with: the built `meter_lab.html` (R-014), **`lab-data.json`** (R-017: the meters registry with parameters, ranges, defaults, factory presets, themes, accents), every `*_wasm.js` module with its SHA-256 in the manifest (not only `sound_map_wasm.js`: `make_lab_drop.py` records one module today), and the adapters. Please say which commit it pins and whether the page can load the modules from the folder (our frame is `allow-scripts` only, so no network and no file reads outside the page).
- **Q-026 (question): "seeded when missing" in `tau_compat.py` (B-982).** Omega reads it per **meter folder**: if `tau-presets/<id>/index.tpi` exists on the card, none of that meter's files is written from a release (not even a `p<id>.tpr` that is missing, since the owner may have deleted that preset and an id is never reused for another one). Is that what you mean, or do you want per-file seeding? (Per file would bring back a deleted factory preset's file; it would stay unlisted, so harmless, but it could also overwrite an id the owner reused if the file is missing.)
- **Q-027 (question): the saved slot -> meter lookup.** `MPRE`/`MPID` are indexed by the meter **table slot** (`p2.slot`), not the manifest id. Omega derives slot -> manifest id from the built-in meter (slot 0) and the pack descriptors on the card. Please confirm that is the intended way, and that `p2.slot` of a pack-only meter is the slot in its descriptor (offset 34).
- **F-028 (finding): stale text.** `mpreset_core.h`'s header comment says the folder is `<meter key>`; code, Python and card use the **manifest id**. `METER_PRESETS_EXTERNAL.md` §6.1 still says 200 presets and an 8-bit id; the code says 127 and 0..126. Omega follows the code.
- **F-029 (finding): AppleDouble stubs in the installed tree.** The owner's `tau-presets/` had **58 `._*` files** beside the 51 real ones (macOS wrote them while copying). Harmless to the firmware (it opens exact names), but `install_factory_presets.py` could remove them after copying. Omega's eject now sweeps them.
- **F-030 (note): names.** The factory names use `[` and `]` (`SILK [EX]`), so they are not limited to the `METR` set `A-Z 0-9 space _ -`. Omega allows printable ASCII, 1 to 15 characters. Does the Pocket font have lower-case glyphs? (The editor will default to upper case until you say.)

- **Q-031 (question): previewing a preset in the lab.** Omega's editor holds a preset as `{parameter key: value}`. Can `meter_lab.html` take a meter and such a set from the host (for example a `postMessage` `{type, meter, values}`, or a URL hash), so Omega can show the chosen preset live? If not, please say how the lab expects presets to be loaded; until then Omega shows no live preview and says so.

- **F-032 (finding, from a real Pocket): `hidden` is not applied.** Omega wrote meter 12's `index.tpi` with SNAPPY hidden (entry flags bit 0, checked with your `mpreset.py show`); the owner saw it **still listed** on TAU DEV 213. `fw2/art/mpreset_host.c` `idx_name_at` reads only `e[0]` (id) and the name, never `e[1]`. The brief said the Pocket should show DEFAULT plus the visible presets. Is applying `hidden` planned (like D-O03 for `METR`), and in which release? Until then Omega saves the flag and tells the user the firmware still lists the preset. Everything else matched: names, order, the deleted preset gone, the custom preset.

## Omega 0.6.0 and your gate
The owner decided that Omega **0.6.0 includes the preset editor and the new lab**. `min_omega 0.6.0` therefore stays; v0.7.0 should not publish before that Omega does. We will tell you the date when the drop (R-025) has arrived and the end-to-end test on a Pocket has run.

## Open for you
R-025, Q-026, Q-027, plus the older P1 manifest key (Q-018), pack library (Q-019), Q-023, R-024, F-023, Q-021, F-005 (`OPEN.md`).
## Open for us
Editor UI, lab preview from the new drop, Omega 0.6.0, the end-to-end test on the owner's Pocket (a card write: the owner's go after a shown plan).
