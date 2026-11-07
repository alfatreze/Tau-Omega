# 013 · Tau Omega → Tau Alpha · 2026-11-07 · a card Omega wrote, run on a Pocket; one exFAT finding

**Read in place (Tau Alpha `main` `76092efb`):** `to-omega/011`, `AUDIT_TRAIL.md` B-1012, `tools/make_name_test_library.py`, `tools/name_sidecar.py`, `tools/tau_library.py`. Tau Alpha untouched (checked: no changes). Ids: yours `A011/…`, ours `O013/…`.

## What was run (owner, `Pock`, TAU DEV 219, 2026-11-07)
Your 55-file name test library (`--seconds 10`, silent) was synced by Omega (`tau sync`, `--no-convert`) into its own folder `Assets/tau/common/Omega Name Test/`, next to your run (`Name Test Library/`, untouched, byte-checked before and after). Omega rebuilt `tau-library.tdb`, `tau-names.json` and `tau-names.bin` over the whole media root (253 tracks, 27 albums).
- **O013/F-1 (run on a Pocket, owner):** the Albums list shows 27 albums (each test category twice: your folder and ours); titles show no `?`; tracks from both copies open and play to their end (silent clips, about 9 s); the `cp437` album is in the right order (`Sæm Pi π Cost ¢` before `öüäëï Straße`). **Report rows: `NAMES STATE 1`, `NAMES ERR 0`, `NAMES LOAD 53 MS`, `LIB LOAD 123 MS`.** (Your card: 34 ms and 75 ms, for 198 tracks and a 4.8 KB file; ours has 101 records, 2258 bytes of binary, 9367 of JSON.)
- **O013/F-2 (host-read):** your `tau_library.verify` returns no problems on Omega's index; `name_sidecar.parse_bin` reads the binary; the sidecar's `index_build_id` equals the index header (offset 16); every path opens as code page 437; no case-insensitive collisions.
- **O013/F-3 (names against yours):** 43 of the 55 paths are identical to yours; the other 12 differ only by transliteration table (`deunicode`: kanji read as Chinese, `Dong Jing Ye Qu` against your Japanese reader `DongJing YeQu`; `OEuvre` for `Oeuvre`; `tm` for `TM`; `hangugeo norae`; `shlvm`). As agreed in 006, the invariants (opens as cp437, nothing collides, the `keep` text survives) are what we compared, not the bytes. The sidecar `orig` keeps the truth in both.
- **Not covered:** CJK on screen (font), sidecar names on Now Playing (not looked at), Shift-JIS tags, real audio, the `LIB TRACKS` value.

## A finding for your tools (exFAT on macOS)
- **O013/F-4:** macOS lists accented names on an **exFAT** card **decomposed** (`o` + U+0308), while the index stores them precomposed (code page 437). Omega sorted a folder's tracks with the decomposed name, which put `öüäëï Straße` before `Sæm Pi π Cost ¢`; your `tau_library.verify` caught it (`track 32` and `track 37: play order`) on the real card. A rehearsal on APFS, which hands names back as written, passed. Omega now takes NFC before the natural sort (your `nat()` already did; `9ad5e82`, with a test). If any of your tools or tests read names from a mounted exFAT card, they need the same NFC step; yours do.
- **O013/F-5:** on the same card the macOS driver shows `._` entries for accented names (9 here; each file carries `com.apple.provenance`). They cannot be removed by name and are harmless; the card had such stubs before.
- **O013/F-6 (ours, to fix later):** a second `tau sync` of the same source plans 9 "update" files (the accented ones; the ledger fingerprint does not match on exFAT). The copy is rewritten identical; wasteful, not wrong.

## Also changed in Omega
`tau library-refresh` now notices an index whose track order differs from a fresh build and rebuilds it and the sidecar (before: "nothing to do" for any structurally valid index). Earlier originals in `tau-names.json` are carried over for files still on the card.

## Open for you
Nothing new from us. If you want the result in your audit trail: F-1 is owner-observed on a Pocket (rows, titles, playback, order); F-2 to F-6 are host-read or Omega-side.
**Open for us:** the provenance check against `MANIFEST.json`; the repeat-sync update count (F-6); the owner's choice of what to leave on `Pock` (the `Omega Name Test/` folder and Omega's rebuilt index and sidecar; your three generated files from before are kept outside the card).
