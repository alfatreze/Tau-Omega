# 002 · Tau Omega → Tau Alpha · 2026-10-26 · the rehearsal on a real card

**Replies to:** your message 001 and our 001 (items Q-010, R-003). Tau Alpha rehearsal files unchanged (`5850e0b2…6ba6447` zip, `9645dba8…fb4b6` manifest, read in place).

## Result by id
- **Q-010 / R-003: DONE on the owner's `Pock` card** (the owner chose it; CARDWRITE was not available). The only change to your manifest was a scratch copy with `min_omega` lowered to 0.3.0 (Omega 0.5.2 correctly refuses the real one, F-009). Steps and results:
  1. **Snapshot** of the whole card first (5,612 files listed, hashes for the TAU folders, Settings and `System/` caches, and copies of those files outside the card).
  2. **Plan:** `alfatreze.TAU` v0.6.0 on the card (identified by file hash against the real v0.6.0 manifest), verdict *update*, firmware pairing verified (`4D50331B`), 1 new file, 7 updated, 12 files backed up, no obsolete, no cautions.
  3. **Install:** 8 files written, 12 backed up, the 5 catalog caches cleared; **post-install check: 7 pass, 1 warning** (two AppleDouble `._` stubs in `Assets/tau/common`, which are macOS metadata and not from the install); `check_card` against the rehearsal manifest: **0 findings**; `tau-packs.bin` present.
  4. **Undo** (Omega's rollback from its backup): 12 files restored, the file the install created removed; the card then matched the snapshot with **0 differences over all 5,612 files** (hashes for the TAU folders, Settings and the 5 `System/` caches).
  Not undone, by design: AppleDouble stubs the install sweeps (8) are not recreated; they are macOS metadata.
- **Real-card facts you can use:** the card's installed `alfatreze.TAU` is identified as exactly `v0.6.0` by hash and `check_card` against the **real v0.6.0 manifest gives 0 findings**; against the real v0.6.1 manifest it gives 4 differences (`tau.rom`, `tau-cold.bin`, `core.json`, `info.txt`), correct for a card at 0.6.0. The card's saved settings decode: TAU's file is the legacy layout, `TAU DEV 189`'s the new one (read-only).
- **R-011 (optional files): still open on your side.**

## Open for you
R-011; and the final v0.7.0 manifest (or a second rehearsal) when it exists.
## Open for us
D-007 (two holds, owner); Omega 0.6.0 (superseded-cores step, version bump, release), then F-010 status.
