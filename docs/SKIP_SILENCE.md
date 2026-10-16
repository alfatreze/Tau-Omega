# Skip silence (Tau Omega side) and the request to tau-alpha

Tau-alpha's `docs/features/PLAYER_V2_SPEC.md` section 10.1 decided the split: detection is offline in Omega, the player only applies two numbers per track. This is what Omega writes, and what it needs tau-alpha to read.

## What Omega does
- Settings > Syncing music > **Skip silence** (off by default). While on, the scan measures each MP3 and FLAC once (a full decode) and keeps the result with the other cached tags, so a file is only read again when it changes.
- One pass measures nine levels (-80 to -40 dB in 5 dB steps) so changing the level setting never reads a file again. The setting picks the nearest level.
- `lead_skip` and `tail_skip` are sample frames at the **file's own rate**, counted **on top of** any encoder delay and padding the file already declares (so MP3 trim stays as the player does it today). A fully silent track is never trimmed.

| Setting | Default | Limits |
|---|---|---|
| Counts as silence below | -60 dB | -80 to -40 dB |
| Silence left at each end | 100 ms | 0 to 1,000 ms |
| Only skip at least | 2 s | 0.5 to 10 s |
| Never skip more than | 10 s | 1 to 60 s |

A skip is `silence - kept`, only when that is at least the minimum, and at most the cap. Values outside the limits are brought back, never refused.

## The request: TDET version 2
`TDET` (tau-alpha `docs/features/TRACK_DETAILS_SPEC.md`, `tools/tau_details.py`) has no spare bytes in its 32-byte record. Omega writes **version 2** only when at least one track has a skip, so an index without trimming is byte for byte version 1:

- Header: `TDET`, `u16 version = 2`, `u16 record size = 40`, `u32 count`, `u32 reserved`.
- Record: the existing 32 bytes unchanged, then `u32 lead_skip` (offset 32) and `u32 tail_skip` (offset 36), little endian, source-rate frames. 0 means nothing to skip.

What tau-alpha needs to do: accept version 2 in the loader (use the header's record size to step), apply the skips as a second trim as section 10.1 describes (`pairs_total` becomes the shortened length), and add the Playback > SKIP SILENCE switch. Until then a version 2 section is the only thing a current player might not accept: **if the current loader rejects an unknown version, tell Omega and it will keep writing version 1** (the setting then only measures).

Reference decoder for the tests: `crates/tau-core/tests/trim_index.rs` (a real FLAC with 5 s and 3 s of silence round-trips to the expected frames).
