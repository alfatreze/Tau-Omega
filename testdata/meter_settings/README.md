# Meter settings save file vectors (copied read-only from Tau Alpha)

Source: Tau Alpha `docs/research/meter_settings/` (golden vectors for `tau-meter-settings.bin`, format `TMSF` v1, their message 029), last changed by Tau Alpha commit `65a884b2`, copied 2026-11-23. The 18 `*.bin` files and `expected.json` are byte-for-byte copies (every file's SHA-256 is in `expected.json` and is re-checked by the test). Their README says what each file is. Format: Tau Alpha `docs/features/meters/METER_SAVE_FILE_SPEC.md` section 4; reference `tools/meter_settings.py`. **Host-only for everything except what the TAU DEV 230 build wrote on a Pocket** (a file with four records after a Quit and after a direct power-off, B-1132).

Used by `crates/tau-core/tests/meter_settings.rs` (the Rust reader `tau_core::meter_settings` is checked against all of it).

`wrong_size_8191.bin` and `wrong_size_8193.bin` are the files that make the Pocket stop at start with "Error in framework file id [11] size bad" (Tau Alpha 032): Omega warns about such a file on a card and offers to delete it, after a plan and a verified backup outside the card. Omega never creates, edits or overwrites a settings file.

## A difference noted (reported to Tau Alpha, `to-alpha/027`)
`expected.json` says `newer_version.bin` is `locked` with **no active bank and no records** ("the whole file is ignored"), as their message 029 says. Their Python `tools/meter_settings.py parse()` returns status `locked` but **still returns the other generation as active (bank 1, 2 records)**. Omega follows `expected.json` and the spec.
