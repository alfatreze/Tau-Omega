# Meter bin v2 vectors (copied read-only from Tau Alpha)

Source: Tau Alpha `docs/research/meter_bin/` (golden vectors for the meter bin v2, plan P2.1), last changed by Tau Alpha commit `28233662`, copied 2026-11-22 when Tau Alpha `main` was `f7612f3a`. The 32 `*.bin` files and `expected.json` are byte-for-byte copies (every file's SHA-256 is in `expected.json` and is re-checked by the test). Their own README, with what each file is: Tau Alpha `docs/research/meter_bin/README.md`. Format: `docs/features/meters/EXTERNAL_METERS_SPEC.md` sections 2, 3 and 7. **Host-only: nothing here has run on a Pocket.**

Not copied: `packages/` (the seven `meter.json` + `meter.bin` the v2 bin was composed from). They are needed for the composer, not for the reader.

`*.reference.json` are what Tau Alpha's reference reader prints for six of the files, generated here with bytecode writing disabled (nothing in Tau Alpha changed):
```
PYTHONDONTWRITEBYTECODE=1 python3 -B -I tools/meter_bin.py info docs/research/meter_bin/<file>.bin --fw-abi 1.2 --tau 0.8
```
(`v2_seven`, `v2_reordered_hidden`, `v2_unavailable_rows`, `v2_empty`, `v2_tuning_collision`, `v1_seven`.)

Used by `crates/tau-core/tests/meter_bin.rs` (the Rust reader `tau_core::meter_bin` is checked against all of it).

## A difference noted in the published files (reported to Tau Alpha, `to-alpha/025`)
In every v2 vector the rows have flags `5` or `1` or `4`: **no row has the fallback bit (bit 1) set**, although Tau Alpha's README says Winamp Bars is "flagged as the fallback" and their message 027 says a bin without a fallback row is refused by newer firmware (code 20, which is not in the code table). `expected.json`'s `fallback_id` is `102d4d21726303d2f1c571f55e7e71c0`, which is not Winamp Bars's id (`7dddfe63121e904843088c075cc28ce9`, in `v2_seven.bin` row 0 and in their `packages/winamp_bars/meter.json`): the vectors look composed with a stale id. The Rust reader follows the vectors (it accepts these files, as `expected.json` says); `MeterBin::fallback()` tells a caller whether a bin has the row. The test `the_published_vectors_carry_no_fallback_flag_on_any_row` records this state and fails on purpose if the vectors are republished with the flag, so this note and the test are then updated.
