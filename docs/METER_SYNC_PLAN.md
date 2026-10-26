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

## Update 2026-10-26 (later): Tau Alpha's answer (interop message 005)
- **Decided (their D-O01):** **P1**. `tau-packs.bin` becomes a user-selectable file in the core's own path (same `TPKB` format, ABI and slot rules); Omega may replace it with a bundle composed from Tau's packs; an update keeps it; `check-card` checks format and ABI instead of a hash. Their manifest role change is **not built yet**; until they send the exact key Omega keeps leaving the file alone. Zero extra meters is allowed (no file at all is tested; `count = 0` is untested on a Pocket).
- **Yes to Omega building bundles (Q-019)** by the rules in `TAU_PACKS_FORMAT.md` and `parse_packs`; the **pack library** (one `.tmpk` per meter with a manifest) is accepted in principle and does not exist yet. Limits: 8 slots of 64 KB, so seven extra meters besides Winamp Bars; the 65,536-byte assets limit is for `tau-assets.bin` only. `METR` replaces a meter's whole preset set (no append). `flags`/`order` will be applied by the firmware later (not in v0.7.0): do not write non-default values yet. The final lab drop (v0.7.0) will carry `meter_lab.html` and `lab-data.json`.
- **Built since (phase C/D engine):** `tau_core::meter_packs`: reads a `TPKB` bundle, splits it into packs, checks a pack and composes a bundle from chosen packs by the stated rules. Verified on the **real DEV 184 bundle**: it reads exactly as Tau's `parse_packs` does (7 packs, ids 0 to 6, ABI 1.0 to 1.2), recomposing all packs reproduces the file **byte for byte**, any subset (and an empty choice) is a valid bundle, and everything the firmware refuses is refused (wrong slot origin, ABI above 1.2 or another major, CRC, cut pack, id outside the slots, duplicate id, more than 8, bad magic or version, trailing bytes, out-of-order packs). Nothing writes a card yet.
- **Still waiting on Tau Alpha:** the P1 manifest key, the pack library asset, the final lab drop (`meter_lab.html`, `lab-data.json`), the firmware ceiling for the pack ABI minor per release (asked as Q-023).

