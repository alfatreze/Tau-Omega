# Skip silence (Tau Omega side)

The contract is tau-alpha's `docs/features/SKIP_SILENCE_SPEC.md` (player side built for MP3, Omega side per its section 4). Omega implements it as written; this file says how and where it differs.

## What Omega does
- Settings > Syncing music > **Skip silence** (off by default). While on, the index scan measures each MP3 and FLAC once (a full decode) and keeps the result with the other cached tags, so a file is read again only when it changes.
- Silence is measured on the larger of |L| and |R| in **10 ms windows**; a window is silent below the level (default -60 dBFS). One pass measures nine levels (-80 to -40 dB in 5 dB steps) so changing the level never reads a file again; the setting picks the nearest.
- **Lead** is the silent run touching sample 0, **tail** the run touching the end. A run is written only when it lasts at least the minimum (2 s) and at least its margin plus 1 s. The margin stays in place: 200 ms before the first sound, 500 ms after the last. Silence inside a track is never touched. A silent track, or one where both ends would leave under 1 s, writes nothing.
- Units are source sample pairs at the file's own rate, counted from the first sample of the music (after the MP3 encoder delay).
- One **`tau-skip.bin`** per album folder (spec section 2): `TSK1`, count, CRC-32, then FNV-1a hash of the card file name with lead and tail (12 bytes each, 340 entries at most). Written after every sync or edit that rebuilds the index; a folder with nothing to skip has no file, and a folder with no tracks left loses its file. With the setting off nothing is written and existing files are left alone.
- The byte layout, the two example hashes and the 36-byte example file are tests (`trim::tests`), equal to the spec's bytes.

| Setting | Default | Limits |
|---|---|---|
| Counts as silence below | -60 dB | -80 to -40 dB |
| Silence left before the first sound | 200 ms | 0 to 1 s |
| Silence left after the last sound | 500 ms | 0 to 2 s |
| Only skip silences of at least | 2 s | 1 to 10 s |

Values outside the limits are brought back, never refused.

## Not done yet (spec section 4)
- The waveform preview with draggable cut marks, per-track "never skip" override and hand-typed values.
- Tracks past 340 in one folder are left out with a warning.
- FLAC numbers are written but the player ignores them for now (spec section 3, point 5).
- Measured only on generated files (WAV and FLAC); an MP3 check against the player's own gapless trim is still to do. The decoder used may or may not already remove the MP3 encoder delay, which would shift the lead by about 1,100 pairs.
