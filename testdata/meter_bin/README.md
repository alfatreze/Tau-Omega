# Meter bin v2 vectors (copied read-only from Tau Alpha)

Source: Tau Alpha `docs/research/meter_bin/` (golden vectors for the meter bin v2, plan P2.1), last changed by Tau Alpha commit `28233662`, copied 2026-11-22 when Tau Alpha `main` was `f7612f3a`. The 32 `*.bin` files and `expected.json` are byte-for-byte copies (every file's SHA-256 is in `expected.json` and is re-checked by the test). Their own README, with what each file is: Tau Alpha `docs/research/meter_bin/README.md`. Format: `docs/features/meters/EXTERNAL_METERS_SPEC.md` sections 2, 3 and 7. **Host-only: nothing here has run on a Pocket.**

Not copied: `packages/` (the seven `meter.json` + `meter.bin` the v2 bin was composed from). They are needed for the composer, not for the reader.

`*.reference.json` are what Tau Alpha's reference reader prints for six of the files, generated here with bytecode writing disabled (nothing in Tau Alpha changed):
```
PYTHONDONTWRITEBYTECODE=1 python3 -B -I tools/meter_bin.py info docs/research/meter_bin/<file>.bin --fw-abi 1.2 --tau 0.8
```
(`v2_seven`, `v2_reordered_hidden`, `v2_unavailable_rows`, `v2_empty`, `v2_tuning_collision`, `v1_seven`.)

Used by `crates/tau-core/tests/meter_bin.rs` (the Rust reader `tau_core::meter_bin` is checked against all of it).

## History: the first published set had no fallback flag (fixed by Tau Alpha, recopied 2026-11-22)
The set first copied (Tau Alpha `28233662`) had no row with the fallback bit (bit 1) set, and `expected.json`'s `fallback_id` was a stale id (`102d4d21...`, not Winamp Bars's). We reported it (`to-alpha/025`); Tau Alpha confirmed it was their generator and republished (`5892f20f`, their `028`/A028/5): `v2_seven.bin` and the files derived from it changed (new SHA-256s), Winamp Bars (`7dddfe63121e904843088c075cc28ce9`) now carries the fallback bit, and `expected.json` has a 33rd vector, `v2_no_fallback.bin`. Recopied here from Tau Alpha `main` at `5892f20f` (the last commit touching the folder).

**Boot codes now mean two things.** `boot_code` is what the **firmware** answers (the reader's boot read, then the firmware's own rule that a fallback row must exist: **20 NOFALLBACK**); `reader_boot_code` is given where the reader's own boot read says something else (`v2_empty.bin`, `v2_no_fallback.bin`: the reader accepts them, the firmware refuses them). In Omega: `meter_bin::open_toc` is the reader's boot read, `meter_bin::boot_check` adds the firmware rule (what the Pocket will do with the file). The full check, `parse`, accepts both bins, as `expected.json` says (`code` 0).
