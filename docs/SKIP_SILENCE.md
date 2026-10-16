# Skip silence (Tau Omega side)

The contract is tau-alpha's `docs/features/SKIP_SILENCE_SPEC.md` (player side built for MP3, Omega side per its section 4). Omega implements it as written; this file says how and where it differs.

## What Omega does
- Settings > Syncing music > **Skip silence** (on by default; it only writes small files beside the music, and the player skips silence only when its own switch is on). While on, the index scan measures each MP3 and FLAC once (a full decode) and keeps the result with the other cached tags, so a file is read again only when it changes.
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

## Review and hand edits (spec section 4, "Preview in Omega")
Settings > Syncing music > Skip silence > **Review tracks...** opens the list of every track with its lead and tail. Choosing a track decodes it and shows the waveform with the two cut marks: drag them or type seconds, tick **Never skip silence on this track**, or go **Back to detected**. A hand edit is written as typed (refused only if it would leave under 1 s of track, then the detected values stand), kept per card in the app's preferences, applied again by every later sync, and written to that folder's `tau-skip.bin` at once. It needs Skip silence on, and a sync once so the tracks are measured.

## MP3 encoder delay
Checked against a real LAME-encoded file (`crates/tau-audio/tests/mp3_silence.rs`, skipped when `lame` is not installed): the decoder removes the encoder delay and padding itself (the frame count equals the source), so a lead is counted from the first sample of the music exactly as the spec wants. A file without a LAME tag is not trimmed, as in the player.

## Not done
- Tracks past 340 in one folder are left out with a warning.
- FLAC numbers are written but the player ignores them for now (spec section 3, point 5).
- The waveform shows loudness in 400 steps and the cut marks snap to 10 ms by typing only through the number fields.
