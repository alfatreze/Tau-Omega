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
| **3** | **High** | **Obsolete `common/` files are removed without Tau's "still read by another core" rule.** Tau's installer keeps `Assets/<plat>/common/tau.rom` while another core on the platform reads it from `common/`; Omega's plan lists it for removal regardless (`install_plan::plan`), which would break an older pinned core. | reproduced with a second core whose ROM slot is not core-specific |
| **4** | Medium | **The update check ignores channels.** A Stable user is told "Tau v0.7.0-preview.1 is available (you have v0.6.0)" and the offered zip is `alfatreze.TAU Preview`, a **different core**, so "Update" adds a second core. Preview users are never checked (`installed_tau` only finds shortname `TAU`). | reproduced; `updates.rs installed_tau`, `release_check::evaluate` |
| **5** | Medium | **Zip naming.** Diagnostic detection looks for `TAU_DIAGNOSTIC`; new names are `TAU_Diagnostics_`, `TAU_Preview_`, `TAU_Preview_Diagnostics_`. The "recommended first" order is right only by luck of alphabetical order. | `release_check.rs` zip sort key |
| **6** | Medium | **`replaces` is not read.** After installing `TAU Diagnostics` the old `alfatreze.TAU_DIAGNOSTIC` stays (two diagnostic cores, double media in the old platform). `superseded_candidates` only knows `alfatreze.TAU_DEV_*`, not `alfatreze.TAU DEV NN`. | `compat.rs` has no `replaces`; reproduced pattern gap |
| **7** | Medium | **Feature pairing is unchecked.** The manifest now carries `bitstream_features` and the ROM `TAUFWNEED`; Omega checks only the CORE_VERSION pairing, so a ROM needing HALCYON on a bitstream without it ("NO UNIT") passes. | `update::pair_status_with` |
| 8 | Low | Without a manifest, same-day builds are still "ambiguous" although `core.json` now carries the full version (`0.7.0-dev.385` vs `0.7.0-dev.386`): `compare_tags("v"+version)` would order them. | `update::assess_with` |
| 9 | Low | The settings viewer labels persisted ids by hand (id 16 still "EQ"); the manifest's `persist_registry` has the names. | `diag.rs` |
| 10 | Low | Uninstall: Tau asks that a remove deletes exactly the `owned` entries of the installed layout and never `user`/`generated` files; Omega removes whole folders. Safe today except finding 1. | review L3 |

## Recommended order

1. **Safety first (findings 1, 2, 3):** one function that says where a core's media lives (the platform its library slot names, from `data.json` bits [25:24]),
   used by the card list, workbench, appearance, refresh, sync, health and post-install check; removal treats a platform as shared when any other core lists
   it in any position; port the "still read by another core" guard to the obsolete step.
2. **Channel-aware updates (4, 5, 6):** pick the release by the installed core's channel (Stable sees non-prerelease only, Preview sees pre-releases),
   find installed Tau cores by id prefix, recognise zips by core id from the manifest instead of name patterns, read `replaces` and offer to remove the old core.
3. **Pairing and ordering (7, 8, 9):** features check, tag ordering from `core.json`, registry names.

Each step gets a gated real-package test (`new_layout.rs` is the start) and, where it writes, the same plan / confirm / `CARDWRITE` proof as before.
