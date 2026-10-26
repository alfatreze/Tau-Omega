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
| G1 | **Persist meanings change in v0.6.2** (every area word; id 10/11/12/15/16 are now "(internal) ..." words, ids 20-33 tuning/last-played). | Settings viewer labels ids from a hard-coded table in `ui/src/lib/App.svelte` (10 "Volume", 11 "Colour index", ... 28 "Theme mode") and decodes by those meanings. | A v0.6.2 card shows wrong or misleading values; **settings migration** between cores could copy words whose meaning differs. | **P0** |
| G2 | `tau-packs.bin` = data slot 9, core-specific, optional, user-invisible; `tau-assets.bin` may also carry an `MPKB` section. | No code knows slot 9. Install/update/remove/carry follow the manifest layout, `tau-assets.bin` read-modify-write keeps unknown sections byte for byte. | A file the manifest lists as `owned` but Omega's tools ignore is fine; a path that **Refresh library, Workbench or a media tool treats as stray** is not. Needs proof, not assumption. | **P0** |
| G3 | Stable `v0.6.x` is **one core, `alfatreze.TAU`, no Diagnostics core**; the v0.6.0 manifest has **`replaces: []`** (the changelog only says "remove TAU_DIAGNOSTIC / TAU Diagnostics / TAU Preview"). | Removal suggestions come from the manifest `replaces` and the `TAU DEV NN` rule only. The Marketplace Tau card offers a Diagnostics Build only where a channel lists one (correct). | A user upgrading from 0.5 or a preview keeps dead Diagnostics/Preview cores and nothing suggests removing them. | **P0** (small) |
| G4 | Report tag **28 `trackinfo`**; new Report/Info rows (statistics, pack counters, `PACK SRC`, `PACKS RUN`). | `taud` decodes tags 1-14, 16, 19-27; 28 and the new rows are unknown. | New screenshots decode partly; Send diagnostics shows less. | P1 (needs real captures first) |
| G5 | Meter **packs** and Winamp-only built-in meter; `meters_schema.json`, pack format, `tools/player2/build_packs.py`. | Meter Lab is a synthetic-schema preview; METR presets preserved but not edited; no view of installed packs. | Omega says nothing about what meters a card has; no path to the Omega-chosen bundles they propose. | P1 (read-only first) |
| G6 | `FONT` / `TFNT` format accepted by firmware, Omega writing is "later". | `TAUA` writer preserves unknown sections; no font tooling. | None now. | P3 |
| G7 | Latin-1 library names planned (BUG-001) with a sync-tool change; index version unchanged so far. | `scan::ascii_name` folds names to ASCII and refuses collisions. | Premature change would break older cores; waiting is safe. | P2 (verify when on main) |
| G8 | `TIM1` freeze undecided; gapless and skip-silence sidecars; `TDET` details. | All implemented and golden-tested against Tau's reference (CI re-clones Tau Alpha `main`, green). | None. | done |
| G9 | `requires_omega`: `min_omega 0.3.0`, `assets_sections METR/PRST/THEM`, `report_tags_max 28`. | `compat` reads schema 1-2, the real v0.6.1 file parses. Omega does not yet **compare its own version/capabilities with `requires_omega`** before offering an install. | A future release that needs a newer Omega installs without a warning. | P1 |
| G10 | Marketplace: the Library now lists `alfatreze.TAU`. | One merged card, listings as tags, default marketplace setting (2026-10-22). | none | done |

## 5. Implementation plan

Each step is host-tested first, uses a real artefact when one exists, and writes nothing to a card without the plan/confirm path. Order is the proposal; the owner sets it.

### Step 1 (P0, before the v0.6.2 release): persist registry and the narrowed fields (G1)
1. Read `persist_registry` from the installed release's `tau-compat.json` (cached, already parsed by `compat`) and from `tools/persist_registry.json`'s shape; the settings viewer takes **names and meanings from the registry**, the hard-coded table stays only as the fallback for cores without a manifest.
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
