# Reference code map (what to port, and where the truth is)

| Tau Omega feature | Reference | Notes |
|---|---|---|
| Index writer, reader, verifier, E-codes, sort keys, letters, playlists, synth generator | `tau-alpha/tools/tau_library.py` | Port function for function: `scan`, `build_index`, `parse`, `verify`, `report`, `synth`, `sort_key`, `nat`, `ascii_text`. Output must be **byte-identical** for the same input. |
| Firmware loader semantics (what the Pocket accepts) | `tau-alpha/fw/library_core.h` | Read-only; explains the size probe, CRC order, sampled walk, caps. |
| Test oracle for the loader | `tau-alpha/sim/test_library_fw.py`, `tau-alpha/tools/host/library_harness.c` | Runs the real firmware code in a RISC-V simulator against indexes; use as an occasional cross-check (needs the vendored toolchain), not in normal CI. |
| Index tests to reproduce | `tau-alpha/sim/test_library_index.py` | Fabricated MP3/FLAC files, corruption matrix, playlist rules, size budget. Reimplement in Rust with the same assertions. |
| Copy, verify, ASCII names, cover embedding, generated playlists, manifest, mirror | `tau-alpha/tools/sync_media.py` | Names: `ascii_part`, `ascii_rel`, `natural_key`; cover embed for MP3 (ID3v2.3 APIC) and FLAC (PICTURE): `embed_mp3`, `embed_flac`; `find_cover`; junk filters. |
| What the player does with a cover (size caps, progressive JPEG, reduce vs full decode) | `tau-alpha/tools/library_check.py`, `tau-alpha/fw/art.inc` | Constants (`ART_MAX_BYTES` 2 MiB, `ART_FULL_MAX` 1024, baseline JPEG only). Use them for pre-flight warnings. |
| Persisted settings (volume, colour, library history) | `tau-alpha/tools/decode_tau_diag_log.py`, `tau-alpha/fw/settings.inc`, `tau-alpha/fw/player.c` (`lib_h`, `lib_hb`, `lib_hs`) | `--interact` decodes `interact_persist.json`. **Two different decoders exist and are easy to confuse:** this one is the older per-variable/diagnostic-record reader. |
| Check report (TAUD1 record, QR, short code) | `tau-alpha/tools/decode_tau_suite.py`, `tau-alpha/fw/suite_core.h` | The Check decoder, and the only reference for ids 20-23 as a report (`--ids` defaults to `20,21,22,23`). Check exists **only in the Diagnostic Build**, never in the release core — do not expect a report on a card running plain `alfatreze.TAU`. |
| Core packaging, checks, rbf reversal, two-zip release | `tau-alpha/package.py`, `tau-alpha/tools/check_tau_package.py`, `tau-alpha/tools/make_release.py`, `tau-alpha/tools/package_sdram_stress.py` | Naming rule: `<Author>.<Core>_<Version>_<Date>.zip`; zip holds only `Cores`, `Platforms`, `Assets`; `rbf_r` is the bit-reversed `rbf`. |
| Card install procedure (backup, verify, clear caches, eject) | `tau-alpha/docs/SESSION_HANDOFF_2026-09-21_RELEASE_0.3.md` section 4 | Encode it as a guided "install core" job. |
| Owner rules | `tau-alpha/CLAUDE.md`, `tau-alpha/docs/SESSION_HANDOFF_2026-09-21_RELEASE_0.3.md` section 3 | Concise UI, numbered test cores `TAU PSRAM NN` never reused, two zips per release, every card write confirmed. |
| Analogue platform facts (core.json, data.json, interact.json, bridge, packaging) | The `analogue-pocket-dev` skill (`~/.claude/skills/analogue-pocket-dev`), `references/knowledge-base/` | Cite entry ids (KB-nnn) and their status when a behaviour depends on hardware. |

**Golden files:** generate them with the Python tools (`tau_library.py synth`, `test_library_index.py`'s fabricated tree) into `testdata/`
and check the generator script in with them. Rust tests compare bytes against these. When the format changes, regenerate from Python first.
