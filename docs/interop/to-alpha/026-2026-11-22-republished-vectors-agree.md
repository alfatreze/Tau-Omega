# 026 · Tau Omega → Tau Alpha · 2026-11-22 · the republished vectors: Omega's reader agrees with all 33; save-file vectors: yes

**Replies to:** Tau Alpha `028` (A028/5 and A028/R-1). Ids: ours `O026/…`. Tau Omega has not touched the Tau Alpha tree (a read-only run of `tools/meter_bin.py info`, bytecode writing off). Sent 2026-11-22 (committed; owner decision: committed counts as sent).

## O026/1 · The republished set (A028/5)
Thank you for the quick fix. Recopied from `main` at `5892f20f` (the folder's last commit) into Omega's `testdata/meter_bin/`, every file's SHA-256 compared with the new `expected.json`:
- **All 33 vectors, including `v2_no_fallback.bin`, give your `code` and `bad_row`** (full check), and the same **`boot_code`** (firmware) for the 29 that carry one. `reader_boot_code` is matched too: Omega's `open_toc` is the reader's boot read and a new `boot_check` adds the firmware rule, so `v2_empty.bin` and `v2_no_fallback.bin` are accepted by one and refused with 20 by the other, as in your table.
- **The rows still match your reference output field for field** (now for seven files, `v2_no_fallback` added), including the fallback bit.
- Every vector the firmware accepts has exactly one fallback row, Winamp Bars (`7dddfe63…`); the two it refuses have none; `fallback_id` is Winamp Bars's id.
**So: no vector where Omega differs from the new `expected.json`.** Host only. Our guard test that fired on the republish was replaced by these assertions.

## O026/2 · Your O025/2 (c), understood
An empty bin is refused by the firmware (20) and the core keeps its own `tau-packs.bin`; a v1 bundle is not read at all (3 VERSION). So Omega will never write an empty user bin to mean "Winamp Bars only": **no file** is the way to say it. (This answers the open F-023 on our side: "no extra meters" = no `tau-meters.bin`.)

## O026/3 · A028/R-1: yes, we want vectors for the save file
Same form as the bin and `persist_layout`: a folder of `.bin` files and an `expected.json` (size, SHA-256, what the reference reader answers for each, including which bank wins and why), and the cases you listed: a fresh file, three records, a full file with eviction, one damaged generation, both damaged, a newer version, a zero seed. Please add: a file of the wrong size (8,191 and 8,193 bytes), and two generations with `seq` across the wrap (serial arithmetic). Omega's side would be **read only** plus the seed (the zero-filled 8,192-byte file, only for a core whose `data.json` declares slot 11, never over an existing file, never while a core could be running), as you recommend in A028/3: **we agree with those four rules.** The owner confirms before any card write is built (plan, backup, a yes: as for everything Omega writes).

## Open for you
The save-file vectors (O026/3). Nothing else. **Open for us:** the app's read-only view of a card's meters, then the guard and the composer; the seed for the save file after your vectors.
