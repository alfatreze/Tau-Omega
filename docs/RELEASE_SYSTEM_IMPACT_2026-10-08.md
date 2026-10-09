# Tau Alpha's new release system: what it changes for Tau Omega (checked 2026-10-08)

**Source (read-only):** Tau Alpha `main` at `6423e42`. The `release-system` branch was merged into `main` by `dc9d21d` and pushed, and its worktree
is gone, so everything below is on `main` (D-015 satisfied). Read: `docs/features/RELEASE_SYSTEM_SPEC.md`,
`docs/features/RELEASE_SYSTEM_REVIEW_2026-10-08.md`, `tools/tau_compat.py`, `tools/tau_layout.py`, `tools/make_release.py`, `tools/omega_compat.json`,
`docs/schemas/tau-compat.schema.json`, `CHANGELOG.md`, `docs/features/CROSS_PROJECT_INTERFACE.md`.

## What changed on the Tau side (verified in the code)

1. **Channels, one core per build.** Stable = `alfatreze.TAU` + `alfatreze.TAU Diagnostics` on platform `tau`. Preview = `alfatreze.TAU Preview` +
   `alfatreze.TAU Preview Diagnostics` on platform `tau_preview`. Dev = `alfatreze.TAU DEV NN` on platform `tau_dev`. **Core ids now contain
   spaces**; zip names turn spaces into `_` (`alfatreze.TAU_Preview_Diagnostics_<ver>_<date>.zip`). The channel comes from the tag (`vX.Y.Z` = Stable,
   any suffix = Preview, `-dev.` = Dev, never published).
2. **Preview and Dev cores declare two platforms** (`["tau_preview","tau"]`, `["tau_dev","tau"]`) and read the library index, `tau-assets.bin`, covers and music
   from **`Assets/tau/common/`** (data slot parameter bits [25:24] = 1). Their own build files stay under `Assets/<first platform>/<core id>/`.
3. **Build-bound files are core-specific** (`tau.rom`, `tau-cold.bin`, `tau-loading.bin` in `Assets/<platform>/<core id>/`, data slots 1/4/6 bit 1). The old
   `common/` copies are listed `obsolete` for `alfatreze.TAU`. The inert `TAU.json` is gone (obsolete too).
4. **Full SemVer in `core.json`** (`0.7.0-preview.1`, `0.7.0-dev.385`); the ROM carries the full version plus the commit (`TAUVER`).
5. **`tau-compat.json` (schema 2)** is written by `make_release.py`, attached to the GitHub release and listed in `SHA256SUMS.txt`: the layout contract,
   `replaces` (cores a channel core supersedes, today `alfatreze.TAU Diagnostics` replaces `alfatreze.TAU_DIAGNOSTIC`), `rom_version`,
   `bitstream_features`, `persist_registry`, `previous_release`, `source`. Dev packages get their own manifest beside a `_dev.zip`.
6. **No published release carries any of this yet.** The GitHub releases (latest `v0.6.0-alpha.3`) have no `tau-compat.json`; the first release made with
   the new tools is the next one (`CHANGELOG.md` "Unreleased").

## How this was checked

Omega's engine was run against **packages made by Tau's own tools**: `tau_compat.build_dev` and `tau_layout.retarget` applied to `dist/` (the same
firmware as alpha.4) for all five cores (Stable, Diagnostics, Preview, Preview Diagnostics, Dev), versions stamped like the packagers do. Gated tests:
`crates/tau-core/tests/new_layout.rs` (`TAU_NEW_LAYOUT_DIR`, `TAU_DIFF_*`). The two gated tests from the earlier merge now run and pass against `main`
(`tau_alpha_round_trip_fixture_keeps_metr_and_prst`, `a_real_tau_alpha_manifest_parses_and_its_package_checks_clean`). **Limits:** no real
`make_release.py` run (two firmware builds), no Pocket run of my own (Tau's probe results B-672/B-673 are taken from its docs), version labels in the
scratch packages are mine.

## Confirmed working

- Schema 2 manifests of all five cores parse; plan, install, post-install check, "release layout" check and `identify_installed` all pass with no findings.
- A card with the shipped **old layout** (ROM in `common/`) updated by a new-layout package migrates cleanly: the old `common/` files are backed up and
  removed, all 14 files match, every check passes.
- Tau's Python `check-card` and Omega's `compat::check_card` give the same findings on a damaged card (missing ROM = error, stray file = warning).
- Shared `Platforms/*.json` between TAU and TAU Diagnostics install as unchanged. Core ids with spaces work through plan, install, rollback and removal.
- Removing any core with all five installed is safe (`Assets/tau` is kept while a sibling uses it).
- The shared TAUA round-trip fixture (METR + PRST) is kept byte for byte.

## Findings (all reproduced; none fixed yet)

| # | Severity | Finding | Evidence |
|---|---|---|---|
| **1** | **High: data loss** | **Removing TAU deletes the library and music that Preview and Dev cores read.** `plan_remove` shares a platform only when another core lists it **first**; a Dev or Preview core lists `tau` second, so with TAU + a Dev core installed, removing `alfatreze.TAU` plans `Assets/tau` (all media), `Platforms/tau.json` and its image, with `platform_shared = false`. | reproduced |
| **2** | **High: wrong folder** | **Every media path uses `platform_ids[0]`.** For Preview and Dev cores the library, `tau-assets.bin` and music are in `Assets/tau/common`, but Omega computes `Assets/tau_dev/common` / `tau_preview/common`: the card list shows "No index yet", the Library workbench and sync write music to a folder the core never reads, the theme install (`appearanceInstall`) writes `tau-assets.bin` where the core does not look, Refresh library and the health badge inspect the wrong folder, the post-install check says "no library yet" while one exists. | reproduced: index `NoIndex` for the Dev core with a valid library in `tau/common`; `App.svelte:73` and `:430` |
| **3** | Medium | **Obsolete `common/` files are removed without Tau's "still read by another core" rule.** Tau's installer keeps `Assets/<plat>/common/tau.rom` while another core on the platform reads it from `common/`; Omega's plan lists it for removal regardless (`install_plan::plan`), which would break an older pinned core. | reproduced with a second core whose ROM slot is not core-specific |
| **4** | Medium | **The update check ignores channels.** A Stable user is told "Tau v0.7.0-preview.1 is available (you have v0.6.0)" and the offered zip is `alfatreze.TAU Preview`, a **different core**, so "Update" adds a second core. Preview users are never checked (`installed_tau` only finds shortname `TAU`). | reproduced; `updates.rs installed_tau`, `release_check::evaluate` |
| **5** | Medium | **Zip naming.** Diagnostic detection looks for `TAU_DIAGNOSTIC`; new names are `TAU_Diagnostics_`, `TAU_Preview_`, `TAU_Preview_Diagnostics_`. The "recommended first" order is right only by luck of alphabetical order. | `release_check.rs` zip sort key |
| **6** | Medium | **`replaces` is not read.** After installing `TAU Diagnostics` the old `alfatreze.TAU_DIAGNOSTIC` stays (two diagnostic cores, double media in the old platform). `superseded_candidates` only knows `alfatreze.TAU_DEV_*`, not `alfatreze.TAU DEV NN`. | `compat.rs` has no `replaces`; reproduced pattern gap |
| **7** | Medium | **Feature pairing is unchecked.** The manifest now carries `bitstream_features` and the ROM `TAUFWNEED`; Omega checks only the CORE_VERSION pairing, so a ROM needing HALCYON on a bitstream without it ("NO UNIT") passes. | `update::pair_status_with` |
| **11** | **High: data loss** | **A manifest can make Omega delete any card file.** `obsolete` paths are removed as listed: a crafted manifest planned the removal of `Saves/..`, a music file and a screenshot (reproduced). Tau's own installer trusts the same list, and the manifest is an unsigned release asset or a file beside a local zip. Tau's format is fixed, so the limit has to live in Omega. | reproduced with a throwaway test (deleted) |
| 8 | Low | Without a manifest, same-day builds are still "ambiguous" although `core.json` now carries the full version (`0.7.0-dev.385` vs `0.7.0-dev.386`): `compare_tags("v"+version)` would order them. | `update::assess_with` |
| 9 | Low | The settings viewer labels persisted ids by hand (id 16 still "EQ"); the manifest's `persist_registry` has the names. | `diag.rs` |
| 10 | Low | Uninstall: Tau asks that a remove deletes exactly the `owned` entries of the installed layout and never `user`/`generated` files; Omega removes whole folders. Safe today except finding 1. | review L3 |

## Adaptation plan: Tau's format is fixed, Omega adapts (owner, 2026-10-09: no change to Tau Alpha)

Nothing below asks Tau for anything. Every rule is derived from what Tau already publishes (`core.json`, `data.json`, `tau-compat.json`).

**A. Where a core's files live (findings 1, 2, 3).**
1. `Core` gains `platforms` (all of `platform_ids`) and `media_platform`: the platform the **library slot** (`tau-library.tdb`) reads from, i.e. `platform_ids[(parameters >> 24) & 3]`. One function `media_root(card, core)` (engine) returns `Assets/<media_platform>/common`; the card list, workbench, sync, Appearance (`tau-assets.bin` is read from the platform of its own slot, same rule), Refresh, health badge, `index_status` and the post-install library check all call it. `Core.platform` stays the **build** platform (first id) for the core-specific folder.
2. Removal: a platform is shared when **any other core lists it in any position**. A platform's `common/` is never removed while another core names it. The `tau_dev` platform goes only with its last core.
3. Obsolete files in `common/` (Tau's moved ROM, cold image, loading art): removed only when (a) the same package owns the same file name in its core-specific folder (it moved, it did not vanish) and (b) no other core on that platform still has a slot naming it without the core-specific bit (Tau's own rule, ported from `still_read_by_other_core`).

**B. What a manifest may delete (finding 11).** An obsolete path is accepted only inside the core's own areas: `Cores/<core>/`, `Assets/<platform>/<core>/`, the platform files of a platform no other core uses, or the `common/` move case in A3. Anything else, and anything that matches a `user` or `generated` entry of the same layout or a media/save/settings/system/memories path, is **dropped from the plan and shown as a warning**, never deleted. Same filter for `compat::check_card` (an out-of-scope obsolete entry is ignored, not reported as stray).

**C. Which release and which zip (findings 4, 5, 6).**
1. Channel comes from the version: `X.Y.Z` Stable, `-dev.` Dev, any other suffix Preview (Tau's rule). An installed core's channel = its `core.json` version, falling back to its platform (`tau` Stable, `tau_preview` Preview, `tau_dev` Dev).
2. The update check offers the newest release **of the installed core's channel**; other channels appear as information ("Preview 0.7.0-preview.1 is also available"), never as an Update. A Preview user is told when the Stable release of the same X.Y.Z appears ("switch channel"), because SemVer orders the preview below it.
3. The zip to download is chosen from the manifest: the `packages[]` entry whose `core_id` equals the installed core's id, or whose `replaces` contains it. Only without a manifest (old releases) does it fall back to names, and then recognises `Diagnostics` case-insensitively. The first install of a channel offers the normal core first, Diagnostics second.
4. Installed Tau cores are found by family (`alfatreze.TAU` prefix, as `breakdown` does), not by shortname `TAU`.
5. `replaces`: parsed; after an install the plan lists the superseded core (for example `alfatreze.TAU_DIAGNOSTIC`) as a **suggested removal with backup**, never automatic. The numbered-test-core pattern also accepts `alfatreze.TAU DEV NN`.

**D. Pairing, ordering, display (findings 7, 8, 9).** `pair_status_with` also checks `rom_needs` against `bitstream_features` (a missing feature is a refusal, as Tau's own gate); without a manifest, order same-day builds by `compare_tags("v"+core.json version)`; show `rom_version` and `source.dirty`; settings viewer takes persisted-id names from `persist_registry` when a manifest is known.

**E. Later, optional.** Settings migration for a renamed core (`TAU_DIAGNOSTIC` to `TAU Diagnostics` orphans its settings file): copy only when `persist_registry` shows no id changed meaning since the old release; uninstall by the installed layout's `owned` entries (review L3).

**Build order.** A1+A2+B first (the unsafe ones, one PR-sized change each with a test against the real five-package set), then A3, then C, then D, then E. Each step: gated real-package test, then plan / confirm, and a `CARDWRITE` proof for anything that writes.

**Not needed from Tau.** No schema change, no new key, no new release asset. The one thing Omega cannot learn from the files is an unsigned manifest's honesty; B bounds the damage instead.
