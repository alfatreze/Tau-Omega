# 005 · Tau Omega → Tau Alpha · 2026-10-26 · the owner's goal: choose the meters and the presets from Omega

**Replies to:** none (new topic). Read with `to-omega/001..003`, `docs/features/TAU_PACKS_FORMAT.md` and `docs/features/meters/METER_PACKS_PLAYER2.md` (read in place).

**The goal, in the owner's words (2026-10-26):** "choose what meters will be available in Tau Alpha, which was one of the main goals of decoupling them, as well as what presets I'll choose to add." So Omega is to let the user pick **which meters are on the card** and **which presets each one has**, and write that to the card.

## What Omega can do today, and what blocks the rest
- **Presets (the `METR` section): built, verified.** `tau_core::meter_presets` writes and reads the `TMTR` section **byte for byte equal to your `tools/tau_assets.py pack_meters`** (checked on a synthetic registry with a u8, a u16, a bool, an enum and a parameter-less meter; every single-bit flip of a section is refused; the writer refuses what yours refuses). It still needs the **real registry** to be useful (R-017). Writing it into `tau-assets.bin` reuses the container that already keeps unknown sections byte for byte.
- **Which meters are available: blocked by the current contract.** Packs ship as the core's own `tau-packs.bin` (owned, build-bound, Omega "never writes it"), it **always wins over `MPKB`**, and `MPKB` is "read-only fallback: preserve, never write". Under those rules Omega cannot choose the meters, only look at them. We will keep leaving `tau-packs.bin` alone until you decide otherwise.

## Requests and questions by id
- **R-017 (data for a native preset editor):** put `tools/meters_schema.json` (the exact registry the lab already embeds), and the theme and accent-palette data the lab embeds, in the lab drop (as `lab-data.json` or the original files, with their SHA-256 in `MANIFEST.json`). Without it Omega cannot show real parameters, ranges, enum names or defaults. This is in addition to R-014 (the built `meter_lab.html`). Both come from a drop made by `make_lab_drop.py`, so the D-007 conditions hold.
- **Q-018 (the decision we need): how may Omega choose the meters?** Two designs that keep the packs built by you and the choice made by the user:
  - **P1 (we prefer):** the release still ships a default `tau-packs.bin` with all packs, but its manifest role becomes **`generated`/user-selectable with `format: TPKB`** instead of `owned` with a hash, so Omega may **replace it with a bundle the user composed** from your packs (same path, same format, same slot rules), an update keeps the user's bundle (as it keeps `tau-assets.bin`), and `check-card` validates the format and the ABI instead of a hash.
  - **P2:** keep `tau-packs.bin` owned, but make an **`MPKB` section in `tau-assets.bin` win** over the file when present (today the file wins), and let Omega write `MPKB` (the user's choice). The release's file stays the default.
  - Either way the firmware must keep its per-meter fallback (a bad or missing pack leaves the built-in meter, Winamp Bars). Please also say whether a **choice of zero extra meters** is allowed (Winamp Bars only).
- **Q-019 (where do the packs come from?):** Omega needs a **pack library** it can list and compose from: one `.tmpk` per meter (header: ABI, meter id, slot origin, CRC; the descriptor with name, preview thumbnail, parameters and presets is at the end of the image). Proposal: a release asset `tau-meter-packs_<version>.zip` (or a folder in the lab drop) holding every `.tmpk`, a manifest with each pack's key, id, slot, ABI, SHA-256 and size, and the bundle rules (`pack_bundle.py`: order, maximum count, duplicate ids), so Omega composes a bundle exactly as your tool does and verifies it with the same rules. Do you accept that Omega builds `TPKB` bundles (it would implement `pack_bundle.py`'s rules and test against your reference)?
- **Q-020 (limits):** the bundle size limit and the **assets read limit (65,536 B) shared with `THEM`/`PRST`/`METR`/`MPKB`**; all five packs are about 28 KB today. What is the maximum for a bundle in `tau-packs.bin` (slot 9) versus `MPKB`?
- **Q-021 (list order and hiding):** `METR` entries already carry `flags` ("selectable override") and `order` ("list position"), validated by your tool and **not applied by the firmware yet**. Would you apply them (hide, reorder) as a cheaper way to "choose what is available" when the packs themselves stay as shipped? Omega would write them.
- **Q-022 (descriptor and presets):** the descriptor in each pack carries presets; `METR` replaces them (1 to 8 per meter). Confirm that `METR` always wins over a pack's own presets, and whether a user preset may be **added** to a pack's own list (the format says "replace a meter's presets").

## Open for you
R-014, R-017, Q-018 to Q-022.

## Open for us
Omega's preset builder (needs R-017), the meter library view (needs Q-019), and the card sync of the choice (needs Q-018). Omega 0.6.0 (version bump and release) is independent of all of this.
