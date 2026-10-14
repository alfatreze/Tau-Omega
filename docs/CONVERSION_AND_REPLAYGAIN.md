# Converting for the Pocket, and ReplayGain

What Tau Omega does to music on its way to a card. The short version: **the Pocket plays MP3 and FLAC; anything it can play is copied byte for byte; anything it cannot play is converted into something it can; your originals are never changed.**

## What the Pocket plays
- MP3 at any rate it can carry.
- FLAC up to **48 kHz**, **mono or stereo**, 8/16/20/24-bit (tau-alpha's `fw/player.c`; the player refuses the rest with a "NO PLAY" message).

## What is done to each source file
| Source | Result |
|---|---|
| MP3 | Copied unchanged (ReplayGain tags may be added to the copy, see below). |
| FLAC the Pocket plays | Copied unchanged (same). |
| FLAC above 48 kHz, more than 2 channels, or an unsupported depth | **Converted to FLAC** (rate and channels folded, see Auto). |
| WAV, AIFF, ALAC (M4A), other lossless | **Converted to FLAC**. |
| AAC (M4A/AAC), Ogg Vorbis, other lossy | **Converted to MP3** through your own `lame` (see LAME). Converting a lossy file cannot add quality. |
| Anything this tool cannot read (Opus, WMA, ...) | Left out, with the reason listed in the review. |

Conversion can be turned off in Settings > Syncing music; unplayable files are then left out and listed. A converted file is a new file next to nothing else: same folder, same name, `.flac` or `.mp3`. Tags and the cover picture are carried over.

## Auto and Manual
- **Auto** (default): match the best quality the Pocket accepts. The rate goes to 44.1 kHz for the 44.1 family (88.2, 176.4) and 48 kHz for everything else (96, 192); the bit depth is kept up to 24 (8-bit becomes 16, 32-bit float becomes 24); more than two channels are folded to stereo; lossy files use LAME's best variable-rate setting (V0).
- **Manual**: the highest FLAC rate (44.1 or 48 kHz), the FLAC depth (16 or 24; reducing uses dither), the MP3 quality (V0..V4 or a constant 128..320 kbit/s), and "also shrink playable FLAC above these limits" (off).
- Surround folding: left gets front-left + 0.707 of the centre + 0.707 of each left surround/side; right mirrors; the LFE channel is left out; each side is divided by its weights' sum so nothing can clip (the result is quieter than the source; ReplayGain brings it back).
- Resampling is rubato's FFT resampler in 64-bit floating point (measured here: tones at 1, 10 and 19 kHz come out at the exact amplitude with the error at the 24-bit floor, about -139 dB).

## Checked against tau-alpha
Every FLAC this tool writes is decoded by tau-alpha's own bit-exact reference decoder (`tools/flac_verify.py`) and by ffmpeg in the tests done during development; the block size in STREAMINFO is written the way strict readers expect. The MP3s come out of LAME unchanged.

## ReplayGain
The Pocket reads `REPLAYGAIN_TRACK_GAIN/PEAK` and `REPLAYGAIN_ALBUM_GAIN/PEAK` (ID3 `TXXX` frames, Vorbis comments) and applies gain by lowering the volume; a positive gain needs the matching peak tag. Settings:
- **Keep existing tags and add what is missing** (default). Albums (a folder) that already carry all four tags are left alone; others are measured and only the missing tags are written.
- **Always measure again and replace the tags**.
- **Do not touch ReplayGain**.

Tags are written into the **copies only**. Measuring is EBU R128 integrated loudness (the `ebur128` crate) against the ReplayGain 2.0 reference of -18 LUFS; the peak is the highest sample of the file as it is written (so it is the converted file's peak for conversions). The album value is the duration-weighted energy mean of the tracks' loudness, the album peak the highest track peak. Measurements are remembered on this computer (not on the card) so a second sync does not measure again.

## LAME
LAME is a separate free program that makes MP3 files. Tau Omega runs it as its own process and never links it, so its licence stays its own. It is found on `PATH`, in the usual package-manager folders, or at a path you choose. If it is missing, Settings offers to install it: on a Mac with Homebrew the app runs `brew install lame` after you click; on Windows and Linux it shows the package-manager command (the app does not run installers that need administrator rights). Nothing is downloaded or installed without you asking.

## Covers
The cover file the Pocket reads (`tau-art/cover_128.pal256.timg`) is always made for each album, from a folder image or the picture embedded in the tracks. The songs' own tags are not touched for covers; embedding a cover into the copies is off by default.

## Not done yet
- A converted file's cover is carried over only when it is under about 120 KB (LAME's limit for MP3; FLAC always keeps it).
- Opus and WMA cannot be read.
- The CLI has no ledger, so conversions and tag changes are re-planned as updates on every run (the app remembers what it wrote).
