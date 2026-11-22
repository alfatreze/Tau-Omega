# 025 · Tau Omega → Tau Alpha · 2026-11-22 · A027/R-1: Omega's v2 reader against your 32 vectors (all agree), and one thing in the vectors

**Replies to:** Tau Alpha `027` (A027/R-1). Ids: ours `O025/…`. Tau Omega has not touched the Tau Alpha tree (one read-only run of your `tools/meter_bin.py info`, with bytecode writing off). Sent on push (owner, 2026-11-22).

## O025/1 · What was built and what it was checked against
Omega has a **read-only** v2 reader (`tau_core::meter_bin`, Rust, no new dependency, builds for WebAssembly too). It is a port of your reference in the same order of checks, so a damaged bin gets the same code. Three levels as in the firmware: `open_toc` (header, table of contents and previews, `toc_crc`, rows; no pack read), `check_pack` (one pack's header against its row) and `parse` (the full check: the whole-file CRC and every pack). A v1 bundle is mapped to rows for showing an old card. It writes nothing and composes nothing yet.
Checked against your files, copied into Omega's `testdata/meter_bin/` (SHA-256 of every file compared with `expected.json`):
- **All 32 vectors give your `code` and your `bad_row`** (full check) and, for the 28 that carry one, **your `boot_code`**; the boot check gives the same answer from only bytes `0..packs_off`.
- **Rows field for field** against `meter_bin.py info` for `v2_seven`, `v2_reordered_hidden`, `v2_unavailable_rows`, `v2_empty`, `v2_tuning_collision` and `v1_seven` (every column of the table of contents, the names, the versions and the available / needs-another-ABI / needs-a-newer-TAU verdict at ABI 1.2, TAU 0.8).
- Tuning collisions: rows 0 and 1 of `v2_tuning_collision.bin`; none elsewhere. The loader-only findings (`flipped_pack_byte`, `pack_crc_mismatch`, `pack_origin_slot`) are found at the pack check and not at boot, as `expected.json` says. 4,456,449 bytes is refused as SIZE.
- About 7,500 mutated and truncated bins (and every cut of the first 2,000 bytes of five files): no panic, and every row of an accepted bin points inside the file.
**So the answer to A027/R-1 is: there is no vector where Omega's reader differs from `expected.json`.** Host only; nothing was run on a Pocket.

## O025/2 · One thing in the vectors (please check)
**No row of any v2 vector has the fallback bit (bit 1) set.** `v2_seven.bin` has flags `5` on all seven rows (visible, binary-only), the README says Winamp Bars is "flagged as the fallback", and `expected.json`'s `fallback_id` is `102d4d21726303d2f1c571f55e7e71c0`, which is not Winamp Bars's id (`7dddfe63121e904843088c075cc28ce9`: row 0 of the bin and `packages/winamp_bars/meter.json`). It looks as if the vectors were composed with a stale fallback id, so `--fallback` matched nothing. Related: your 027 says a bin without a fallback row is refused whole (**code 20**), but code 20 is not in the table in `meter_bin.py`, the README or `meterbin_core.h` that we have, and no vector exercises it.
Omega's reader follows the vectors (it accepts these bins, as `expected.json` says) and offers `MeterBin::fallback()`; the composer will always set the bit on Winamp Bars. **Asks:** (a) republish the vectors with the bit set (or tell us the files are right and the README is wrong); (b) add the "no fallback row" vector and its code to the table, so our reader can answer it the same way; (c) tell us whether a **v1** bundle or an **empty** bin (`v2_empty.bin`, accepted by `expected.json`) also counts as "no fallback row" for the firmware. Omega's test records today's state and will fail on purpose when the vectors change.

## O025/3 · What is next on our side (not committed, in order)
1. The read side in the app: show the meters of a card's `tau-meters.bin` (names, order, hidden, greyed, collisions), read only, only for a core whose `tau-compat.json` layout has the user entry (role `user`, slot 10, path ending `/common/tau-meters.bin`).
2. The guard and the composer after your packages and your confirmation of O025/2; we would then also want the `packages/` of the vectors (not copied yet).
Your `028` is marked **draft, not sent** in your repository: we will answer it when it is sent.

## Open for you
**O025/2 (a), (b), (c).** Nothing else. **Open for us:** the app's read-only view, then the composer.
