# Video section plan (Tau V: TNV3 / TVC / TAC), 2026-10-30

Owner request 2026-10-30: a **Video** section in Omega, modelled on the music sync (scan, convert on copies, write to the card, verify),
that transcodes to Tau V's own format. Standing decision: D-020. This is a plan; nothing here is built.

## 0. Where Tau V is (read-only facts, checked 2026-10-30)

Source: `../tau-nu/.claude/worktrees/codec-alternatives` (branch `worktree-codec-alternatives`, last commit `18e74f2d`, not pushed; `tau-nu`
`main` is still the older MPEG-4 `.tnv` path). Tau V is a sibling repo, treated like Tau Alpha: **read-only, nothing copied** (D-010).

| Piece | Spec (normative) | Reference tools | State |
|---|---|---|---|
| Container **TNV3** (video + one audio + subtitle tracks, CRC, index) | `docs/TNV3_CONTAINER_V0.md` draft 1.4 | `tools/tnv3_ref/tnv3.c` (mux/demux/check/info), independent Python demuxer, 9 valid + 15 must-reject vectors | draft |
| Video **TVC** (I/P/B, qpel, 4x4, slices, levels 1-3) | `docs/TVC_BITSTREAM_V0.md` draft 5 | `tools/tvc_ref/tvc_enc.c` (537 lines), `tvc_dec.c`, `tvc.h`; 19 positive + 3 negative vectors, C and Python decoders bit-exact | draft |
| Audio **TAC** (MDCT, 48 kHz, 96-128 kbit/s) | `docs/TAC_BITSTREAM_V0.md` | `tools/tac_ref/tac.c` (410 lines) | draft |
| Subtitles **TST** | `docs/TST_SUBTITLES_V0.md` | `tools/tst_ref/` | draft |
| End-to-end film encoder | none | `tools/tnv3_ref/encode_film.py`: ffmpeg decode and scale, segments encoded in parallel, TAC audio, `tnv3 mux`, `tnv3 check` | works |
| **Player on a Pocket** | | | **Nothing decodes on hardware yet** (RTL decoder bit-exact in simulation; display path, SD reader, firmware, audio RTL not built) |

Levels (from `encode_film.py`): L1 400x240, L2 800x464, L3 800x720. Size: about 100-280 MB/hour at VMAF 90 for 800x450, a 2 hour film
0.4-0.8 GB at QP 30. **Encoding is slow: 0.6-0.9x real time on 12 cores** (1.3-1.9 hours for a 2 hour film).

## 1. What this means for the plan

1. **The format is not frozen and no player exists.** Omega can build everything up to "a valid, checked `.tnv3` on the card" and prove it
   against Tau V's references, but cannot show a picture on a Pocket until Tau V's firmware lands. Every Omega piece is therefore gated behind
   byte-exact tests against Tau V's own vectors, and the format version is read from the header (refuse anything but version 0 draft 1.x).
2. **Encoding takes hours**, unlike music. The job model matters more than the UI: background job, per-segment progress, cancel, **resume**,
   a cache of finished segments outside the card (the reference already cuts into 480-frame segments, which is the natural unit).
3. **The source decoder is the missing dependency.** Omega's audio path uses symphonia (pure Rust). There is no comparable pure-Rust decoder for
   the H.264/H.265/AV1/VP9 sources people will have. See section 3.

## 2. Modelled on music sync

| Music sync (today) | Video section |
|---|---|
| Scan source folders, read tags | Scan folders: probe container, codec, size, fps, duration, audio and subtitle streams |
| Plan: what to copy, convert, skip, with reasons | Plan per file: encode (level, resolution, fps, QP), already TNV3 (copy), unsupported (reason); estimated size and **estimated encode time** |
| Convert on the copy, source never modified | Encode to a work folder on the computer, never the source |
| Copy to card, build and verify the library index | Copy `.tnv3` to the card, **`tnv3 check` equivalent verification of the written file** (CRC, index) |
| Verification ledger, fast repeat scans | Same ledger keyed by source size/mtime and encoder settings, so a finished encode is never redone |
| Library workbench (problems, tags, playlists) | Library of encoded films: title, year, poster, duration, subtitle tracks; problems: too large for the level, wrong fps, no audio |
| Card-space bar | Same, plus per-film size |
| Cover art `tau-art/` | Poster frame (and a thumbnail from the stream) |

Everything that writes the card keeps plan, review, confirm, verify, backups outside the card, journal (D-004, D-006, `SAFETY_RULES.md`). Test
cards: `Pock` (snapshot first) and `CARDWRITE`. Never modify a source file.

**Open: where the player reads films from.** Tau V's README still says `.tnv`; the TNV3 docs I read define the file but I did not find a card folder
convention, a library index or a poster format. This is the first question for Tau V (section 6).

## 3. Decisions the owner needs to make

| # | Question | Options | Recommendation |
|---|---|---|---|
| V-A | **Where does the encoder run?** | (a) bundle Tau V's C tools (`tvc_enc`, `tac`, `tnv3`) as sidecar executables built in CI; (b) port TVC encoder, TAC encoder and TNV3 muxer to a new Rust crate `tau-video` (about 2,000 lines of reference C), verified bit-exact against Tau V's vectors | **Revised 2026-11-05 (owner): (a).** The pure-Rust encoder diverged from the C one, so the C encoder stays the reference and is wrapped by a CLI (`Tau codecs/tau-video-cli`, over the `tau-video` crate's C-encoder adapter). Omega drives that CLI as a **sidecar process** with a stable JSON contract; the pure-Rust parts of `tau-video` (container inspect/check, TVC decode, player) can be linked in-process. See `docs/TAU_VIDEO_CRATE_REQUIREMENTS.md` section 0. |
| V-B | **How are sources decoded?** | (a) require the user's own `ffmpeg` on PATH (detected, never downloaded); (b) bundle ffmpeg sidecar (LGPL/GPL build, redistribution and patent questions); (c) pure Rust decoders (little coverage) | **(a)** first, with a clear "ffmpeg not found" state; revisit (b) after you decide on licensing. A download or new dependency needs your explicit yes (CLAUDE.md). |
| V-C | **Does Tau Omega license/vendor Tau V's code?** | Port from the specs and test against the vectors (our own code), versus copying the C | Port from the specs; no literal copy (D-010 spirit, and the two repos never share files). Tau V is MIT, so a verbatim copy with provenance is possible if you want it faster: your call. |
| V-D | **Plan before Tau V `main`?** | ROADMAP says a feature is planned only once on the sibling's `main` | Approved by this request; recorded as D-020 with the gate "no card writes of `.tnv3` until Tau V has a player release that opens it". |

## 4. Milestones (each ends in a commit with tests; no card write before M4)

| M | Deliverable | Verified by |
|---|---|---|
| M0 | Questions to Tau V (section 6); decisions V-A to V-C answered | owner |
| M1 | `tau-video` crate: **TNV3 reader, checker and info** (header, chunks, index, CRC, audio interleave bounds, subtitle area), refusing unknown versions | Tau V's 9 valid and 15 must-reject vectors, fixtures by path + SHA-256 with a provenance README (as `testdata/meter_packs`) |
| M2 | Source probing and the **Video** view: scan, plan table, level and size/time estimates, ffmpeg detection (no encode yet); skeleton and empty states | browser suite like `marketplace-test.cjs` |
| M3 | Encoder: TVC encode, TAC encode, mux, segment cache, background job with progress, cancel, resume; CLI `tau video encode` | bit-exact against Tau V's vectors; a short real clip round trip with Tau V's own `tvc_dec` and `tnv3 check` as the oracle (run locally, not in CI) |
| M4 | Card sync: plan, review, copy, verify written file, ledger, space bar, rollback of the copy | tests with scratch dirs; **real card only with your go**, on `CARDWRITE` first |
| M5 | Library: poster, metadata, problems, playlists of films; subtitles (SRT/ASS to TST) | after Tau V settles TST |
| M6 | Release in a minor version, tag in `docs/RELEASING.md` terms | |

## 5. Risks

- **Format churn**: drafts 1.1 to 1.4 in three days. Mitigation: one `tau-video` crate, spec version in every fixture, a pin like D-019 for Tau V's vectors.
- **Encode time**: hours per film on 12 cores; far slower on a laptop. Mitigation: estimate before the plan is confirmed, resume, a low-effort preset (Tau V lists speed work as an encoder task), pause when on battery.
- **Port correctness**: encoder decisions are not normative (only the bitstream is). Check validity with the decoder and `tnv3 check`, not byte equality with `tvc_enc` output, except for the fixed vectors.
- **Licensing of ffmpeg and of any bundled tools** (V-B).
- **Scope of codec choice**: Tau V also keeps an MPEG-4 `.tnv` path on `main` and AV1 experiments in `codec-phase0`. This plan targets TNV3 only; the older path is not planned.

## 6. Questions for Tau V (to write up as an interop message once you confirm the channel)

1. Card folder, file naming and any index or poster format the player will read for TNV3.
2. Which drafts are intended to freeze first (container 1.4, TVC 5, TAC, TST) and when a player release is expected.
3. Whether Tau V will publish its vectors with SHA-256 so Omega can pin them (as Tau Alpha did for the lab drop).
4. Encoder presets: what Tau V wants Omega's defaults to be (QP 30, 96 kbit/s, 800 wide, level auto).
5. Whether subtitle track import (SRT, ASS) belongs in the host encoder (`tools/tst_ref`) as it does today.

Crate requirements for the encoder/decoder library: `docs/TAU_VIDEO_CRATE_REQUIREMENTS.md`.
