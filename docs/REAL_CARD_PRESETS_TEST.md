# Real-card test: meter presets on the Pocket (plan for the owner's yes, 2026-11-07)

**Status (2026-11-07): all 7 steps run** (results in `docs/FIRMWARE_SYNC.md`). Step 6: the PRESET row showed DEFAULT on the Pocket (owner); the persist re-read was not valid because the owner changed the preset afterwards. Step 7: `tau-presets/` is byte-identical to the snapshot again (51 files). Every card write waits for the owner's yes.
Evidence labels: what the owner sees on the Pocket is **run on a Pocket**; what Omega decodes from the card afterwards is **host-read from a Pocket's file**.

## The card and the core (read-only facts, checked 2026-11-07)
- Card `Pock`, exFAT, `/Volumes/Pock`, 108 GiB free, volume UUID `58BDB0BD-E127-3410-AFC3-4C1C348A48CB`. The only volume touched.
- Core **TAU DEV 213** (`0.6.1-dev.213`, "PRESET READ PROBE"): per Tau Alpha's audit (B-973) the owner saw the PRESET row list DEFAULT plus the factory presets from files on this card. **Not yet checked by anyone:** the remembered choice across a reboot, a vanished preset, whether hidden presets are skipped (this test does).
- `Assets/tau/common/tau-presets/12..18`: 51 files, **identical** to Tau Alpha's `mpreset.py factory` output (compared byte for byte today); 58 harmless `._*` stubs beside them.
- The test meter is **Winamp Bars (id 12)**: it is the built-in meter (no pack needed), five factory presets, small files. Layered Wave (id 16) is not used for the card write.

## What will be written (exactly)
Only inside `Assets/tau/common/tau-presets/12/`. The rehearsal on a scratch copy of this exact tree gave: **3 files written** (`index.tpi`, `p1.tpr`, `p5.tpr`), **1 removed** (`p3.tpr`), **3 backed up** (`index.tpi`, `p1.tpr`, `p3.tpr`), `p0`, `p2`, `p4` unchanged.

| Preset | Change |
|---|---|
| id 0 FLUID, id 2 BOUNCY | unchanged |
| id 1 CLASSIC | renamed **CLASSIC 2** |
| id 3 SLOW FADE | **deleted** (its file removed; the id is never reused) |
| id 4 SNAPPY | *(first run: the index's hidden bit was set; the Pocket ignored it, so SNAPPY stayed listed. Hidden is now Omega-only, D-022: the app removes a hidden preset from the card and keeps it)* |
| id 5 TEST ONE | **new custom** (FLUID's values with `bands` = 8) |

## Steps
Each step that touches the card is a dry run (plan) first, then the write with the plan's token. All commands use the built CLI (`tau`) from this repo, the same engine as the app.

1. **Snapshot (read-only copy to the Mac, outside the card).** `Assets/tau/common/tau-presets/` (the whole folder) and `Settings/alfatreze.TAU DEV 213/Interact/_core/interact_persist.json` to `~/Downloads/DEV PROJECTS/Tau Alpha/work/omega-card-backups/<timestamp>-presets/`, with a SHA-256 list. Nothing is written to the card.
2. **Plan.** `tau meter-presets-show` and `tau meter-presets-plan` with `tools/preset_scenario.py`'s set. Expected: meters `[12]`, 3 writes, removals `[p3.tpr]`, 3 files backed up. If the plan differs from this, stop and show it.
3. **Write** (owner's yes): `tau meter-presets-install ... --confirm <plan id> --backup-dir <outside the card> --yes`. The engine takes the backup first and checks it reads back, writes each file next to its place, reads it back, swaps it in (presets first, index last), re-reads the folder and puts everything back if any step fails. Then `tau meter-presets-show` must list `FLUID, CLASSIC 2, BOUNCY, (hidden) SNAPPY, TEST ONE`; eject with `diskutil eject /Volumes/Pock`.
4. **On the Pocket (owner).** Start TAU DEV 213, Configure, Meter WINAMP BARS: the PRESET row should offer **DEFAULT, FLUID, CLASSIC 2, BOUNCY, TEST ONE** in that order (SLOW FADE gone; SNAPPY was still listed on the first run because the hidden bit is ignored). Choose **TEST ONE**, switch the Pocket off and on, start the core again: the PRESET row should still say TEST ONE (remembered by id 5).
5. **Back on the Mac.** With the card mounted, read-only: Omega decodes `interact_persist.json` and should show "Meter slot 0 preset: TEST ONE (id 5)".
6. **A chosen preset disappears.** Plan and write the set minus id 5 (`tools/preset_scenario.py ... --without 5`), eject, boot: the meter should run on **DEFAULT** and the remembered choice be cleared (a re-read of the persist file shows no preset for slot 0).
7. **Put it back.** Write `tools/preset_scenario.py ... --undo` (the set exactly as step 1 found it), or copy the snapshot back if the owner prefers; compare against the snapshot's SHA-256 list: `tau-presets/` must be byte-identical to the factory files again. Eject.

## If something goes wrong
- The engine refuses before writing for: a wrong token, a backup folder inside the card, a card that changed since the plan, a folder the firmware would refuse. A failure in the middle of a write puts every file back and says so.
- Any surprise on the Pocket (the PRESET row empty, a crash, wrong order): do not write again; read `tau meter-presets-show`, the persist file and the Report rows MPX; restore step 7 from the snapshot.
- A pulled card mid-write leaves `*.old` or `*.part` files in the meter folder; the next plan says "interrupted" and puts back what was left.

## What the owner's observations settle (and what they do not)
Steps 4 and 6 are the Pocket-side parts of Tau Alpha's definition of done (list shows exactly the visible presets in index order, the choice survives a reboot, deleting the chosen preset resets to DEFAULT). Step 5 is Omega decoding a real persisted preset id (not yet exercised by any capture). **Not covered here:** the lab preview of Layered Wave against the firmware's golden commands (needs the new lab drop, R-025), audio during a preset change, other meters.
