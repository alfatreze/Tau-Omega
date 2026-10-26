# Meter selection and preset building (plan, 2026-10-26)

**Owner's goal:** from Tau Omega, choose **which meters are available** in the Tau player (the point of decoupling the meters from the core) and **which presets** each has, and write that to the card.

## What Tau Alpha ships today (read in place)
- Meters are **packs**: one `.tmpk` per meter, bundled as `TPKB`; the core's own `tau-packs.bin` (data slot 9, `owned`, build-bound, optional) carries the default bundle; an `MPKB` section of `tau-assets.bin` is a fallback that **always loses** to the file; Omega "never writes either". Pack descriptors (name, thumbnail, parameters, presets) sit at the end of each pack image. Only Winamp Bars is built in.
- Presets are the `METR` section of `tau-assets.bin` (`TMTR`, 1 to 8 named presets per meter, a default preset; entries also carry `flags` and `order`, not applied by the firmware yet).
- Registry: `tools/meters_schema.json` (not in Omega yet: interop R-017).

## What Omega has built
`tau_core::meter_presets` (commit with this plan): the `METR` section writer and reader, **byte-identical to Tau's `tools/tau_assets.py`** on a synthetic registry, with the same refusals; tests `tests/meter_presets.rs`; fixtures `testdata/meter_presets/` (synthetic, README says so). The container already keeps unknown sections; wiring `METR` into `AssetsEdit` (like `PRST`) is a small next step.

## Phases
| Phase | What | Needs |
|---|---|---|
| **A (built)** | `METR` writer and reader, verified against Tau's reference | nothing |
| **B** | Preset builder in the Meter Lab: real meters and parameters from the registry, name/describe/save presets (1 to 8 per meter, default), write to `tau-assets.bin` through the reviewed plan (`AssetsEdit` + install plan, backup, undo) | the real registry (R-017) |
| **C** | Meter library: list the packs (and their descriptors, thumbnails) Omega can offer, with ABI compatibility against the card's core (`requires_omega.packs_abi`) | pack library (Q-019) |
| **D** | Choose the meters that are on the card and write the bundle (P1: replace `tau-packs.bin` with the user's `TPKB`; or P2: `MPKB` wins) | Tau's contract (Q-018, Q-020) |
| **E** | Meter Lab shows the real lab (R-014: built page in the drop), previews the chosen meters and presets before anything is written | R-014 |

Until Q-018 is answered Omega keeps leaving `tau-packs.bin` alone (D-019 and `tests/packs_untouched.rs`). Phases B and E do not depend on it.
