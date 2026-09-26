# Data formats (authoritative summary)

Source of truth is `tau-alpha/docs/MEDIA_LIBRARY_0.4_SPEC.md` and the reference code `tau-alpha/tools/tau_library.py`
(writer, reader, verifier) and `tau-alpha/fw/library_core.h` (the firmware loader). This file restates everything Tau Omega needs so it
can be built without reading the firmware. **If this file and the reference code disagree, the reference code wins; report the difference.**
All multi-byte values are little-endian. Every section is 16-byte aligned.

## 1. Card and core layout (Analogue Pocket)
```
<card root>/
  Cores/<Author>.<Shortname>/        core.json  data.json  interact.json  input.json  video.json  audio.json  variants.json
                                     bitstream.rbf_r  icon.bin
  Assets/<platform>/common/          media and files shared by every instance of the platform
  Assets/<platform>/<Author>.<Shortname>/   per-core instance json
  Platforms/<platform>.json          Platforms/_images/<platform>.bin
  Settings/<Author>.<Shortname>/Interact/_core/interact_persist.json     (written by the Pocket when a core is quit)
  Memories/Screenshots/*.png         Memories/Save States/
  Saves/  System/  (System/*.bin are caches; System/Logs)
```
* `core.json` -> `core.metadata`: `platform_ids[0]` (the `<platform>` folder name, lowercase a-z0-9_, at most 15 chars), `shortname`,
  `author`, `version`, `date_release`, `description`. Core folder name is `<author>.<shortname>`.
* Tau cores: author `alfatreze`, shortname `TAU`, `TAU_DIAGNOSTIC`, numbered test builds `TAU_DEV_NN` (platform `tau_dev_NN`; older ones `TAU_PSRAM_NN`).
* A core **supports the library** if its `data.json` has a data slot whose `filename` is `tau-library.tdb` (slot id 5 in Tau builds).
  Detect by that, not by name, so future cores work.
* `data.json` slots seen in Tau: 1 firmware `tau.rom`, 2 audio file (mp3, flac, deferload), 3 playlist `playlist.m3u`, 4 `tau-loading.bin`,
  5 library index `tau-library.tdb`.
* Media roots: everything the library indexes lives under `Assets/<platform>/common/`. **Paths in the index are absolute card paths**
  (`/Assets/<platform>/common/<dir>/<file>`), so an index is bound to its platform folder.

## 2. `tau-library.tdb` (index) version 1
Location: `Assets/<platform>/common/tau-library.tdb`. Built from the destination folder (after ASCII conversion).

### Header, 128 bytes
| Off | Size | Field |
|---|---|---|
| 0 | 4 | magic `TLIB` = 0x42494C54 |
| 4 | 2 | format_version = 1 |
| 6 | 2 | min_reader_version = 1 |
| 8 | 4 | header_size = 128 |
| 12 | 4 | flags: bit0 art file present, bit1 playlists present, bit2 UTF-8 strings (0 = ASCII) |
| 16 | 4 | build_id = CRC32 of the body (bytes 128..end) |
| 20 | 4 | file_size |
| 24 | 4 | body_crc32 |
| 28 | 4 | reserved 0 |
| 32 | 8 | n_artists, n_albums, n_tracks, n_playlists (u16 each) |
| 40 | 4 | root string offset (`/Assets/<platform>/common/`) |
| 44 | 4 | art_id (CRC32 of the art file header; 0 = none) |
| 48 | 64 | 8 sections `{offset u32, length u32}`: artists, albums, tracks, strings, album_by_title, track_by_title, letters, playlists |
| 112 | 12 | reserved 0 |
| 124 | 4 | header_crc32 over bytes 0..123 |

Caps (reader rejects above): 16,384 tracks, 2,048 albums, 1,024 artists, 64 playlists, file 4 MiB, strings 3 MiB. Path limit 200 bytes.

### Records
* **Artist 8 B:** `name u32` (string offset), `first_album u16`, `n_albums u16`. Sorted by sort key; an artist's albums are contiguous.
* **Album 20 B:** `title u32`, `dir u32`, `artist u16`, `year u16`, `first_track u16`, `n_tracks u16`, `art u16` (0xFFFF none), `flags u16`.
  Sorted by (artist key, year, title key). An album's tracks are contiguous, in play order.
* **Track 16 B:** `title u32`, `file u32` (file name relative to the album dir), `secs u16` (0 unknown), `tno u16` (disc << 10 | track),
  `album u16`, `fmt u8` (1 MP3, 2 FLAC), `flags u8`. Track id = index.
* **String pool:** NUL-terminated printable ASCII, de-duplicated, offset 0 = empty string. Offsets are relative to the section.
* **Path** of a track = root + album.dir + `/` + track.file (no extra slash when dir is empty).
* **album_by_title** u16[n_albums], **track_by_title** u16[n_tracks]: ids in sorted title order. **letters:** 3 x 27 u16 (artists,
  albums-by-title, tracks-by-title; `#`, A..Z): first position whose class >= that letter, so a letter jump is one lookup.
* **Playlists:** records 8 B `{name u32, first_item u16, n_items u16}` followed by a u16 array of track ids (items may repeat).
  Section length = 8 * n_playlists + 2 * total items.

### Ordering and text rules (must match the reference exactly)
* Sort key = (class, natural tokens): lowercase; drop a leading `the ` or `a ` if something remains; class 0 for a non-letter first
  character (digits, symbols), 1..26 for A..Z; then split into digit runs (compared as numbers) and text runs.
  Ties are broken by id, so output is deterministic.
* Tracks inside an album are ordered by (disc, track number, natural file name).
* Text is ASCII: NFKD-normalise, drop combining marks and every non-ASCII character, collapse whitespace, truncate titles/artists to 63.
* Album artist = TPE2 (or Vorbis ALBUMARTIST), else the first non-empty TPE1/ARTIST in the album, else `Unknown Artist`.
  Album title = TALB/ALBUM of the first track, else the folder name, else `Unknown Album`. Track title = TIT2/TITLE, else the file name
  without the leading digits, punctuation and extension.
* Year from TYER/TDRC/DATE (first four digits). Track/disc from TRCK/TPOS/TRACKNUMBER/DISCNUMBER (number before `/`); track clamp 1023, disc 63.
* Duration: MP3 = Xing/Info frame count x samples per frame / rate, else CBR estimate from the first frame header and the audio byte count;
  FLAC = STREAMINFO total samples / rate; unknown = 0; clamp 65,535.
* Tags: ID3v2.3 and 2.4 (TIT2 TPE1 TPE2 TALB TRCK TPOS TYER TDRC; text encodings 0-3), ID3v1 fallback, FLAC Vorbis comments. Frames other than
  text are skipped without reading their payload (covers can be megabytes).

### Playlist import rules (tool side)
* Read every `.m3u` under the media root. Lines are bare relative names (relative to the list's folder) or absolute card paths;
  `#` lines ignored. Lines that do not resolve to an indexed track are dropped and counted.
* A list that is exactly the audio files of its own folder, in order, is an **album list** (the tool used to generate them) and is **not** imported.
* Name = file stem (ASCII, 31 chars); the conventional `playlist.m3u` takes its folder's name, or `Playlist` at the root; duplicates get ` 2`, ` 3`.
* At most 64 lists and 16,384 entries each (truncate with a warning).

### Loader semantics (firmware) and error codes
Load order: header CRC (E11), size probe (E12), body CRC (E13), counts (E14), section ranges and lengths (E15), sampled record walk and
playlist checks (E17); E10 = no file, E16 = PSRAM proof failed. The firmware reads the file in 4 KiB windows and treats a read past the end as a failure:
the header's `file_size` must be exact (the last byte reads, one byte past it does not). Tau Omega must therefore write files whose size equals
`file_size` and must never append data.

## 3. Cover images: `TIM1` container, `tau-art/` sidecars (superseded 2026-09-26, `tau_core::image` built)

**This section previously described `tau-library-art.bin`, a single-file RGB565 thumbnail store per
`MEDIA_LIBRARY_0.4_SPEC.md` section 3. That design is superseded.** tau-alpha ran a real study
(`tau-alpha/docs/IMAGE_FORMATS.md`, owner decision 2026-09-26, D-I01/D-I02) and decided differently:
per-album `<album>/tau-art/cover_<size>.pal256.timg` sidecar files, not one big art-store file,
**palette-256** (an 8-bit-indexed `TIM1` container: 16-byte header, then a 256-entry RGB565 CLUT,
then one index byte per pixel), not raw RGB565, at **128 px on the long side, scaled proportionally
with no crop or letterbox** (a non-square cover stays non-square; the header carries the real width
and height).

`tau_core::image` implements this: `decode_tim1` (every payload shape real tooling can produce —
`rgb565` and `palette` at 8/6/4 bpp) and `encode_pal256_bytes`/`encode_cover_pal256` (the decided
default). Verified against a real `.timg` file from a real album, not invented from this section's
prose (`Tau Omega/testdata/images/README.md`).

> **Both open questions from the previous design are still open, just restated (`FIRMWARE_SYNC.md`
> "Open conflicts"):**
> 1. **The slot number is still unassigned and slot 6 is still double-booked** (Phase G's cold image
>    `tau-cold.bin` occupies it in the shipped core). Deciding the pixel format didn't resolve this.
> 2. **The container itself is explicitly not frozen** (`IMAGE_FORMATS.md` D-I05) and **no firmware
>    reader exists yet**. Writing `.timg` sidecars today is real, tested forward-prep and a real
>    decode path for our own UI's previews — it does not yet do anything on the Pocket itself.
>
> The reserved index fields are unaffected either way: `art_id` stays at offset 44 and the album
> record's `art` field stays `u16 = 0xFFFF` until a slot exists.

## 4. Persisted settings (read-only display)
`Settings/<core>/Interact/_core/interact_persist.json` holds `variables[]` `{id, type, val}`. Tau library builds use (id -> word):
10 volume, 11 colour index (0..18, Pocket edition colours), 12 repeat, 13 shuffle, 15 meter, 16 EQ, 18 saved position, 19 resume on,
20-23 legacy playlist name/hash, **24 library history**, 25 index build id (31 bits), 26 Shuffle All seed, 27 library off (1 = disabled).

> **Ids 20-23 are overloaded — verified on hardware files 2026-09-22.** Every core (release and
> Diagnostic Build alike) declares them as `(internal) list 1..4`, the legacy playlist state above.
> The **Check report reuses those same four variables** — `tau-alpha/tools/decode_tau_suite.py`
> defaults to `--ids 20,21,22,23`. Nothing in the JSON says which meaning is present, so a reader
> must **try the TAUD1 decode and fall back to playlist state when its CRC32 fails**; the CRC is the
> only discriminator. Decoding blind produces a plausible-looking but fictitious Check report.
> Note also that tau-alpha's own logs call these "persist words 8-11": that is the *declaration
> position* in `interact.json` (ids 10,11,12,13,15,16,19,18 occupy positions 0-7), not the id. Two
> numbering schemes for the same four values — always key on the id, which is what the file carries.
History word (id 24): bits 0-2 kind (1 album, 2 artist, 3 playlist, 4 all tracks A-Z, 5 Shuffle All), bits 3-13 id, bits 14-27 queue position.
Tau Omega can show "last played" by resolving kind and id against the index whose build id matches word 25.

## 5. Manifest written by every sync (JSON, kept on the host; nothing extra is written to the card)
```json
{ "tool": "tau-omega", "version": "x.y.z", "time": "2026-09-21T12:00:00Z", "card": "<volume id>", "core": "alfatreze.TAU",
  "platform": "tau", "sources": ["/Users/me/Music"], "index": {"sha256": "...", "tracks": 7180, "albums": 800, "artists": 300, "playlists": 3, "build_id": "1BD19340"},
  "files": [{"dest": "Artist/Album/01 Title.mp3", "sha256": "...", "bytes": 1234, "state": "new|update|same", "converted": false, "note": ""}],
  "warnings": [], "errors": [], "skipped": [{"src": "...", "why": "..."}] }
```
