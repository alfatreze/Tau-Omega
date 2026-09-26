# Real TIM1 cover-image fixtures

Same fixture policy as `testdata/interact_persist/README.md`, `testdata/packages/README.md` and
`testdata/screenshots/README.md`: copied from a real artefact, never invented from memory of the
format. See `tau-alpha/docs/CROSS_PROJECT_INTERFACE.md` §2 and `docs/FIRMWARE_SYNC.md` for why —
this project has been bitten twice by a fixture that agreed with the code instead of with reality.

## `cover_128.pal256.timg`

Copied byte-for-byte (`sha256` verified: `e85f8ceb4447e3a9abe8f53fe23a041763617a8c888cc18c834d05b71316f9fd`)
from a real album on the sibling `tau-alpha` repo's own card backup:
`work/card-backups/20260926-100952/alfatreze.TAU_0_5_0_A_22/Assets/tau_0_5_0_a_22/common/
Nausicaa of the Valley of the Wind Soundtrack/tau-art/cover_128.pal256.timg` — produced by that
project's real `tools/sync_media.py --art-variants` against a real cover, per its decided default
(`docs/IMAGE_FORMATS.md`, D-I01/D-I02, 2026-09-26): palette-256, 128 px long side.

Cross-checked against `tau-alpha/tools/tau_image.py`'s own `unpack_header` before copying (the
reference decoder, not a guess): `fmt=2 (palette)`, `bpp=8`, `w=128`, `h=128`, `ncolors=256`,
`payload_len=16896` (512-byte CLUT + 16,384 one-byte indices, matching `w*h`). This is the known-good
decode a Rust reader here should reproduce exactly.

## `cover455.jpg`

A real 455×455 baseline JFIF cover (`tau-alpha/work/test-music/build/cover455.jpg`, one of the actual
test covers `IMAGE_FORMATS.md`'s own comparison tables were built from). Used as encoder input: decode
→ resize to 128 px long side → palette-quantize → re-pack as `TIM1`, then decode our own output back
and sanity-check it (this project's own quantizer is not required to match `tau_image.py`'s pixel-for-
pixel — only the container *format* is shared, per that doc's own "not frozen" status, D-I05).
