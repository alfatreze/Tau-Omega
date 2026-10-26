# Tau Alpha review and Tau Omega implementation plan (2026-10-26)

Read-only review of `../tau-alpha` (nothing there was changed, fetched or reset) plus its published releases, compared with what Tau Omega does today. It replaces the stale "last checked 2026-10-02 against v0.6.0-alpha.1" in `docs/FIRMWARE_SYNC.md` (that file gets a short pointer to this one).

**Scope rule reminder (D-015):** a Tau feature is planned here only once it is on Tau Alpha `main`. Tau Alpha's local `main` is at `b506575b` (2026-10-23) and is **64 commits ahead of `origin/main`, i.e. not pushed**; the newest *published* release is `v0.6.1`. Everything marked "main, unreleased" below is therefore read from a local tree: plan against it, but verify against the real release package the day it ships (real artefacts before code).

## 1. What Tau Alpha has shipped (GitHub releases, verified)

| Release | Date | What matters to Omega |
|---|---|---|
| `v0.6.1` **Stable** | 2026-10-21 | Fallback screens (no/unreadable/empty library) that point at Tau Omega's address, Menu > HELP, `core.json` repository URL. No file or persist change. Now also **listed in the openFPGA Library** (`alfatreze.TAU`). |
| `v0.6.0` **Stable** | 2026-10-17 | First Stable of the new scheme: **Player 2**, one core `alfatreze.TAU` on platform `tau`. **No Diagnostics core any more** (diagnostics are inside the player). Fonts (Tamzen/Space Grotesk, writable font memory, CORE_VERSION rev 27), 14-icon strip, speed vs tempo rows, new FLAC decoder, gapless FLAC, double-buffered meters, Configure page, `METR` presets. New persist: id 29 (meter presets), word 18 bits 20-22 (font), ids 20-23 carry saved meter tuning. |
| `v0.6.0-preview.2` (pre-release) | 2026-10-17 | Same Player 2 as v0.6.0 on the Preview channel (`TAU Preview`, no Diagnostics core). |
| `v0.6.0-preview.1` | 2026-10-09 | Halcyon EQ only, pixel-grid reports only, channel scheme introduced. |

Checked today against real artefacts: the `tau-compat.json` of `v0.6.1` **parses in Omega's engine** (schema 2, `min_omega 0.3.0`, 1 package, 24 layout entries, `previous_release v0.6.0`); the zips of v0.6.0, v0.6.1 and preview.2 **match their `SHA256SUMS.txt`** (the v0.5.0 mismatch noted earlier is only that old release). `report_tags_max` is now **28**.

## 2. On Tau Alpha `main`, not yet released (Oct 22-23, audit B-799..B-836)

- **Meter packs:** every meter except Winamp Bars is a loadable **pack** (descriptor carried by the pack, pack ABI 1.2, per-pack cycle limits and three strikes, D-M20). Packs ship as the core's own **`tau-packs.bin` = data slot 9** (core-specific, optional, loaded before `tau-assets.bin`; the file wins over an `MPKB` section). Seven meters now (Waterfall with colour maps and horizontal flow, SOUND MAP, ...).
- **Persistence by area** (`docs/features/PERSISTENCE_LAYOUT.md`): one register word per area, all 16 persist variables used (**id 33 new**), presets for 20 meters, tuning pool, a **resumable migration** from the legacy layout. `tools/persist_registry.json` records the new meanings **since v0.6.2**. Volume is 7 bits now (no longer a whole word). The Core Settings sliders for Colour, Volume, Repeat, Meter and Halcyon are gone.
- **Audio statistics block** (RMS, correlation, centroid, crest, flux, onset) feeding meters; new Info/Report rows; **report tag 28 `trackinfo`** (track details page).
- Last played track and colour restore fixes; Now Playing tag shows the Halcyon preset; `make test-host-fast`.
- Their stated next steps: a Pocket run of a package carrying `tau-packs.bin` (DEV 184), then the **v0.6.2 release** (version bump, `--previous` zip, owner's approval). The handoff says in so many words: **"Tau Omega must leave `tau-packs.bin` alone and mask the persist fields."**

## 3. Planned or parked on their side (from their roadmap and specs)

Hardware watchdog for a stuck pack (RTL + fit); more than seven meters ("views inside one pack", **Omega-chosen bundles** proposed); an Omega preview twin for SOUND MAP and Chladni (WebAssembly of the same C proposed); the `FONT` section of `tau-assets.bin` and a `TFNT` record "shared with Tau Omega" (firmware accepts it, Omega writing it is "a later phase"); freezing the `TIM1` cover container (D-I05, owner); Latin-1 names in the library (closes BUG-001; changes how sync folds names); track details v2 / skip silence for FLAC; Japanese/CJK, MOD/tracker, 720p output (last). None of these is in a release yet.

## 4. Gap analysis: what Omega does today against that

| # | Tau Alpha fact | Omega today (verified in code) | Risk if left | Priority |
|---|---|---|---|---|
| G1 | **Persist meanings change in v0.6.2** (every area word; id 10/11/12/15/16 are now "(internal) ..." words, ids 20-33 tuning/last-played). | Names come from the cached release manifests' registry (`persist_names`), with a hard-coded fallback table in `ui/src/lib/App.svelte`; **values are decoded by the old meanings** (a whole-word volume, theme/mode bits, ReplayGain bits) and nothing knows the layout version. Settings migration compares meanings by `since` only. | A v0.6.2 card shows labelled but wrongly decoded values; migration across layouts is only as safe as the manifest's `persist_ids_changed`. | **P0** |
| G2 | `tau-packs.bin` = data slot 9, core-specific, optional, user-invisible; `tau-assets.bin` may also carry an `MPKB` section. | No code knows slot 9. Install/update/remove/carry follow the manifest layout, `tau-assets.bin` read-modify-write keeps unknown sections byte for byte. | A file the manifest lists as `owned` but Omega's tools ignore is fine; a path that **Refresh library, Workbench or a media tool treats as stray** is not. Needs proof, not assumption. | **P0** |
| G3 | Stable `v0.6.x` is **one core, `alfatreze.TAU`, no Diagnostics core**; the v0.6.0 manifest has **`replaces: []`** (the changelog only says "remove TAU_DIAGNOSTIC / TAU Diagnostics / TAU Preview"). | Removal suggestions come from the manifest `replaces` and the `TAU DEV NN` rule only. The Marketplace Tau card offers a Diagnostics Build only where a channel lists one (correct). | A user upgrading from 0.5 or a preview keeps dead Diagnostics/Preview cores and nothing suggests removing them. | **P0** (small) |
| G4 | Report tag **28 `trackinfo`**; new Report/Info rows (statistics, pack counters, `PACK SRC`, `PACKS RUN`). | `taud` decodes tags 1-14, 16, 19-27; 28 and the new rows are unknown. | New screenshots decode partly; Send diagnostics shows less. | P1 (needs real captures first) |
| G5 | Meter **packs** and Winamp-only built-in meter; `meters_schema.json`, pack format, `tools/player2/build_packs.py`. | Meter Lab is a synthetic-schema preview; METR presets preserved but not edited; no view of installed packs. | Omega says nothing about what meters a card has; no path to the Omega-chosen bundles they propose. | P1 (read-only first) |
| G6 | `FONT` / `TFNT` format accepted by firmware, Omega writing is "later". | `TAUA` writer preserves unknown sections; no font tooling. | None now. | P3 |
| G7 | Latin-1 library names planned (BUG-001) with a sync-tool change; index version unchanged so far. | `scan::ascii_name` folds names to ASCII and refuses collisions. | Premature change would break older cores; waiting is safe. | P2 (verify when on main) |
| G8 | `TIM1` freeze undecided; gapless and skip-silence sidecars; `TDET` details. | All implemented and golden-tested against Tau's reference (CI re-clones Tau Alpha `main`, green). | None. | done |
| G9 | `requires_omega`: `min_omega 0.3.0`, `assets_sections METR/PRST/THEM`, `report_tags_max 28`. | `compat` parses `requires_omega` but **nothing compares `min_omega`, `assets_sections` or `report_tags_max` with Omega's own capabilities** (checked: no use outside the parser) before an install is offered. | A future release that needs a newer Omega installs without a warning. | P1 |
| G10 | Marketplace: the Library now lists `alfatreze.TAU`. | One merged card, listings as tags, default marketplace setting (2026-10-22). | none | done |

## 5. Implementation plan

Each step is host-tested first, uses a real artefact when one exists, and writes nothing to a card without the plan/confirm path. Order is the proposal; the owner sets it.

### Step 1 (P0, before the v0.6.2 release): persist registry and the narrowed fields (G1)
1. Names already come from the cached manifest registry; extend that to **meanings** (the registry carries a `meaning` number and `since` per id, plus `what` in Tau's own `tools/persist_registry.json`): decode each id by the layout its `since` says, keeping the hard-coded table only as the fallback for cores without a manifest.
2. Decode by layout version: legacy words (<= v0.6.1) vs the **area words of v0.6.2** (`SW_PLAYBACK` layout-version bits 30-27, appearance, audio with volume bits 6-0, meters, tuning slots A/B, last played). Mask the narrowed fields; show "legacy layout" / "converted" instead of guessing.
3. Settings migration between cores: refuse (with the reason) when the two cores' registries disagree on any id's meaning (`since` differs); never copy words across layouts. Test with the real v0.6.1 registry against a synthetic v0.6.2 registry.
4. Acceptance: a captured `interact_persist.json` from a v0.6.2 core (owner's DEV 184/183 card, read-only copy kept outside the card) decodes to the same values the Pocket shows; unknown ids show "Word N" with the registry name.

### Step 2 (P0): `tau-packs.bin` is left alone, everywhere (G2)
1. Add data slot 9 to Omega's slot knowledge (name, core-specific, optional) so card/core views list it as the core's packs, not as stray or unknown.
2. Make an automated test over every Omega tool that touches a core folder (Refresh library, workbench plan, media sync, removal with keep-media, settings migration, core copy/move, carry, theme install, `tau-assets.bin` install): a synthetic package with `tau-packs.bin` and an `MPKB` section comes out byte-identical.
3. Compat card check: a missing optional `tau-packs.bin` is not a failure; a changed one is reported as "different build", not "damaged".
4. Acceptance: the test passes red-then-green (a mutant that deletes the file is caught); the real v0.6.2 package is run through `check-card` on CARDWRITE the day it is published.

### Step 3 (P0, small): superseded Tau cores (G3)
1. Built-in knowledge, versioned: cores `alfatreze.TAU_DIAGNOSTIC`, `alfatreze.TAU Diagnostics`, `alfatreze.TAU Preview`, `alfatreze.TAU Preview Diagnostics` are superseded by `alfatreze.TAU` **from v0.6.0 on** (the changelog states it; the manifest does not). The install/update plan offers their removal (keep-media default on), never automatically.
2. Ask the owner (note for Tau Alpha, not a patch) to put these ids in `replaces` of the next manifest so the rule moves to data; Omega then prefers the manifest.
3. Acceptance: upgrading a synthetic card that holds `alfatreze.TAU` 0.5.0 plus `TAU_DIAGNOSTIC` to the real v0.6.1 package lists the Diagnostics core for removal with the music kept.

### Step 4 (P1): capability check and report tags (G9, G4)
1. Compare `requires_omega` (min version, `assets_sections`, `report_tags_max`) with Omega's own capabilities before an install is offered; a release needing more says so in plain words and links the update.
2. Tag 28 `trackinfo` and the new Report rows: **first collect real screenshots** (owner's Pocket card, DEV 183/184), then decode with fixtures, then show them in Send diagnostics.
3. Acceptance: every tag up to `report_tags_max` either decodes or is shown as "not understood by this Omega" (never silently dropped).

### Step 5 (P1): meter packs, read-only (G5)
1. Parse `tau-packs.bin` (format from their pack documents once frozen; verify against the file in a real DEV 184 package) and show a Packs section in the core details: pack names, versions, ABI, which meters they provide, integrity state.
2. Meter Lab: replace the synthetic schema with the real `meters_schema.json` (open since 2026-10-02) and mark SOUND MAP / Chladni as "no preview twin yet".
3. Do **not** write packs or bundles until Tau Alpha freezes the format and says Omega owns authoring (their "Omega-chosen bundles" is a proposal); record the decision request in `docs/DECISIONS.md` as an open question.

### Step 6 (P2/P3): library names and fonts (G7, G6)
When Latin-1 names reach a release (and the index version question is answered), relax `ascii_name` behind a per-core capability flag so older cores keep ASCII; add a collision test. Fonts: write the `FONT` section only after the `TFNT` record is documented as final.

### Housekeeping alongside
- `docs/FIRMWARE_SYNC.md`: replace the 2026-10-02 header with this review's date and a pointer here.
- `ROADMAP.md` section 0 table: refresh rows for release packages (single core, no Diagnostics), data slots (9 added), persist ids (v0.6.2 layout), tags (28), `tau-assets.bin` sections.
- When v0.6.2 ships: re-run steps 1-3 against the real package and manifest before announcing support.

## 6. Notes for the owner (Tau Alpha side, read-only here)
- `main` has 64 unpushed commits; Omega's CI clones Tau Alpha's GitHub `main`, so it does not see them yet.
- The v0.6.0 and v0.6.1 manifests carry `replaces: []`; adding the superseded core ids would let Omega act from data (step 3).
- `docs/CURRENT_STATUS.md` in Tau Alpha still has the 2026-10-10 header; `tools/tau_data_slots.py` docstrings predate id 33 and slot 9 (not checked in detail here).

## 7. Order and effort (proposal)
Steps 1-3 first (about two working sessions: they are what a v0.6.2 card could break); step 4 with the first real captures; step 5 after the pack format is frozen; step 6 later. Nothing here needs a card write beyond the normal CARDWRITE acceptance runs.

## 8. Tau Alpha's answer (2026-10-26) and what it changes

**Corrections to read this plan with:** the next release is **v0.7.0, not v0.6.2** (read "v0.6.2" above as v0.7.0; final tag not decided, a preview may come first), v0.7.0 will carry more than what is on `main` today (so nothing here is a frozen manifest, and no v0.6.2/v0.7.0 manifest may be hard-coded), and `origin/main` is now at `9c6c30ea` (pushed 2026-10-26; the v0.7.0 correction `f6605a92` is local to them until their next push; their registry already says `since v0.7.0` for every id).

**Answered by Tau Alpha (facts we now plan against):**
- Persist: every id from 10 to 33 changes meaning against v0.6.1 (the manifest's `persist_ids_changed` is the full list, checked by a rehearsal with the v0.6.1 zip as `--previous`); ids 31/32 first ship in v0.7.0; ids 24-26 are **retired** (registry `retired` block, not in the manifest registry, old values to be ignored). Word layout: 0 playback + layout version (bits 30-27), 1 appearance, 2 audio (volume bits 6-0), 5/6/15 meters, tuning slots in ids 20-23 and 27-29+33, last played 31/32; **bit 30 of words 1, 2, 5, 6, 15 is a "converted" mark: mask it.** Migration is in the firmware (old to new lossless, new to old lossy: a v0.6.1 core reading a new card shows defaults and rewrites the old layout on its first save). **Rules for Omega:** never copy persist words from a new-layout core to an old-layout one; never copy ids 20-23 or 27-29 between cores whose registry `meaning`/`since` differ; old to new may pre-seed.
- `replaces` is now data: a Stable `alfatreze.TAU` lists `alfatreze.TAU_DIAGNOSTIC`, `alfatreze.TAU Diagnostics`, `alfatreze.TAU Preview`, `alfatreze.TAU Preview Diagnostics` (minus an id the same release ships); `alfatreze.TAU Preview` replaces only `alfatreze.TAU Preview Diagnostics`. **Compare versions before offering a removal** (a Preview newer than the installed Stable is not obsolete).
- `tau-packs.bin`: layout role `owned`, slot 9, `required: false`, core-specific (`Assets/<platform>/<core id>/tau-packs.bin`); a card without it works (falls back to the `MPKB` section of `tau-assets.bin`, else one meter); the file always beats `MPKB`. Format frozen with pack ABI 1.2: `docs/features/TAU_PACKS_FORMAT.md` (their repo). Omega reads it, never writes it, never merges it into `tau-assets.bin`, never deletes it.
- Report: the Player 2 report is tag 26 (`SR_T_INFOTEXT`, one self-describing row per line), **no new tag numbers** (`report_tags_max` 28). `requires_omega` on main: index v1, sections `METR`/`PRST`/`THEM` (`MPKB` is read-only fallback: preserve, never write), read limit 65,536, `min_omega 0.3.0`.
- Latin-1 names are **not** in the next release; index version stays 1; keep folding to ASCII (G7 closed for now, they will warn before it ships).

**Verified by Omega the same day (real artefacts, read-only):**
- The three real Player 2 Report screenshots (`docs/research/p2_reports/`, DEV 181/182/184) decode in Omega's `tau report-code` to **210, 216 and 217 rows, identical to Tau's own `table.txt` row for row (643 rows, 0 mismatches)**, including the pack, statistics and `PACK SRC` rows. No capture contains tag 28 (`trackinfo`): tag 28 stays undecoded until one exists.
- Schema-2 layout entries (role, slot, `required`, `format`) and `persist_ids_changed` / `persist_registry` have been understood since **Omega v0.4.1** (the compat module was published in it); today's engine parses the real v0.6.1 manifest. A slot-9 entry needs nothing new to parse.
- **Finding:** `required: false` is honoured only for `user` and `generated` entries. For an `owned` file both Omega's `check_card` and Tau's own `tools/tau_compat.py check-card` report a missing file as an **error**, so a card whose `tau-packs.bin` was deleted reads "missing" as a failure although the core works without it. Omega matches Tau's reference on purpose; changing it means changing both (Tau's first). Same for `tau-cold.bin` and `tau-loading.bin` (`required: false` too): asked of Tau whether that is intended.
- The manifest's `persist_registry` carries only ids, so the `retired` block never reaches Omega's parser (which rejects non-numeric keys): safe.

**Plan changes:** step 1 now has the exact word layout and the bit-30 mask, plus the retirement of 24-26 (decode as "retired, ignored"); step 2 can build its slot-9 test from the real `layout` entry shape; step 3 becomes data-driven plus a version comparison (the built-in list is only the fallback for old manifests); step 5 (read-only packs viewer) is unblocked by the frozen format; step 4's report part shrinks to tag 28 and waits for a capture; G7 is closed. Asked of Tau Alpha: a Python reference (`tools/persist_layout.py`) with golden vectors (legacy words to new words) and real `interact_persist.json` captures before/after migration; the real `meters_schema.json` changes (Waterfall Colours/Direction, SOUND MAP) for the Meter Lab; and a decision on `min_omega` (raise it to the Omega release that ships steps 1-2: we will name the number).

## 9. Progress (2026-10-26)

**Step 1 built** (`tau_core::persist_layout`, commit `8fbfb52`): Rust port of Tau's `tools/persist_layout.py`, checked against Tau's golden vectors (field table, 37 migration cases, 32 interrupted-migration cases) and real captures kept in `testdata/persist_layout/` with a provenance README. **TAU DEV 181's real legacy file migrates to exactly the file TAU DEV 185 wrote on a Pocket** (hardware-confirmed by Tau Alpha 2026-10-26). Two mutants (converted mark, a shifted field) are caught. Settings > Inspect persisted settings now shows a Tau core's file decoded by its layout (new, or legacy with a note that the player converts it itself); another core's words are never decoded by Tau's layout. Settings migration between cores now also refuses to carry settings to an **older** release. Not hardware-proven (Tau's own note, repeated in the README): other legacy files and a power loss mid-migration. Not done: the registry-driven `meaning` display for ids of cores without a manifest, and the bit-30 mask is applied only through the layout decoder.

**Step 2 built** (`tests/packs_untouched.rs`, same commit series): Tau cores' build-bound files (`tau-packs.bin`, `tau.rom`, `tau-cold.bin`, `tau-loading.bin` in `Assets/<platform>/<core id>/`) survive an update whose package does not ship them, are replaced only by a package that ships them, and the Files tab refuses to remove them or to add a file of that name by hand (two mutants caught). Known limits: the Files tab still labels firmware and packs "BIOS" (the existing slot-name rule); `check_card` still reports a missing optional owned file as an error, matching Tau's reference until Tau decides.

**Next:** step 3 (superseded cores from the manifest `replaces`, with the version comparison), step 4 (`requires_omega` comparison, which is what `min_omega 0.6.0` depends on), step 5 (read-only packs viewer from the frozen format and the real DEV 184 file).

**Step 4 built** (2026-10-26, interop message 001): `requires_omega.persist_layout` and `packs_abi` are parsed and compared with what this Omega supports (`OMEGA_PERSIST_LAYOUT = 1`, `OMEGA_PACKS_ABI = 1`), `min_omega` is compared with the running version, and an install whose release needs more is **refused at the plan** with the reason. The real v0.7.0 rehearsal package (Tau Alpha `docs/interop/rehearsal-v0.7.0/`) is refused by 0.5.2 as designed; with only `min_omega` lowered in a scratch copy it installs cleanly, passes the post-install check, and removal works (see `docs/interop/to-alpha/001-...`). `check_card` reports a missing optional owned **data-slot** file as a warning (narrower than Tau's reference: their manifest marks core.json/data.json/bitstream.rbf_r `required: false` too, asked of Tau as R-011). Still open: the real CARDWRITE rehearsal (card not mounted, owner), step 3 (superseded cores), step 5 (packs viewer), Omega 0.6.0 itself (version bump and release), the two held decisions (wasm twin, lab drop).

