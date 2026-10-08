# Real core release package fixture

`alfatreze.TAU_0.4.0_2026-09-22.zip` is copied verbatim from the sibling `tau-alpha` repo's
`release/` directory (2026-09-23) — a real, shipped Analogue Pocket core release package, not a
hand-built test archive. Same fixture policy as `testdata/interact_persist/README.md`: copied from a
real artefact, not written from memory of the format.

Contents (15 files, deflate-compressed, verified with `unzip -l`/`unzip -v`):

```
Assets/tau/alfatreze.TAU/TAU.json
Assets/tau/common/tau-cold.bin
Assets/tau/common/tau-loading.bin
Assets/tau/common/tau.rom
Cores/alfatreze.TAU/audio.json
Cores/alfatreze.TAU/bitstream.rbf_r
Cores/alfatreze.TAU/core.json
Cores/alfatreze.TAU/data.json
Cores/alfatreze.TAU/icon.bin
Cores/alfatreze.TAU/input.json
Cores/alfatreze.TAU/interact.json
Cores/alfatreze.TAU/variants.json
Cores/alfatreze.TAU/video.json
Platforms/tau.json
Platforms/_images/tau.bin
```

This is the same `Cores/<id>/*`, `Assets/<platform>/**`, `Platforms/*.json` shape
`tau_core::inspect_card` already reads from an installed card, which is what makes it possible to
plan an install/update by diffing this zip's entries against an existing staging-card folder.

## Added 2026-10-08 (roadmap item 1, update comparison)

Copied verbatim from the sibling `tau-alpha` repo's `release/` directory, read-only:

- `alfatreze.TAU_0.6.0_2026-10-04.zip` (v0.6.0-alpha.3) and `alfatreze.TAU_0.6.0_2026-10-07.zip`
  (the alpha.4 build): same `bitstream.rbf_r` (SHA-256 `ed977c71...6727`), different ROM and date. This is the
  real "firmware-only update" case. Both ROMs carry `TAUFWPAIR:4D50331A;`.
- `alfatreze.TAU_DIAGNOSTIC_0.6.0_2026-10-07.zip`: the second core of a release, a different core id.
- `alfatreze.TAU_0.4.0_2026-09-22.zip` (above) predates the pairing marker, which is what the "no marker" case uses.

No package carries the alpha number: `core.json` says `0.6.0` for all of them, so Omega compares by date and file hashes.
