# Tau Omega

**One desktop manager for your Analogue devices: their SD cards, media, ROMs, saves, cores and settings, in one place.**
It starts with the **Analogue Pocket** (and the **Tau** music player core, which it was born for) and is being built to grow past
both: more devices, more kinds of library, and a plugin-friendly engine that other developers can build on.

> **Status: v0.5.1, in the middle of a reorganisation.** The app began as a companion for the Tau core and is being widened into a
> general device manager. Everything it did before is kept and is still supported in full; the layout, naming and some screens are
> moving while that happens, and the Tau-specific parts are being separated from the general ones. Expect screens to change between
> releases. Nothing that touches a card changes meaning without a reviewed plan.

macOS, Windows and Linux. Free and open source (MIT OR Apache-2.0). Releases: <https://github.com/alfatreze/Tau-Omega/releases>
(installers, signed in-app updates, `SHA256SUMS.txt`). The installers are not yet code-signed, so macOS and Windows warn on first run.

## What it does today

**Cards and cores**
- Finds the SD card, lists **every core and platform** on it, and shows what each holds, in a platform → core tree with counts and
  per-core details (Files, Saves, Screenshots, Settings tabs).
- **Marketplace:** browse the community openFPGA Library, install a core through a reviewed plan, and see which installed cores have
  updates (one at a time, with downgrade and undo).
- **Add a core manually** from a zip, a folder or a GitHub address, with a preview and a strict trust check before anything is written.
- **Install and update Tau itself** (first install, update from GitHub or a zip, channel-aware: Stable, Preview, Dev), with
  firmware/bitstream pairing checks, a post-install check, settings migration and an uninstall that keeps your music.
- **Disable** cores the Pocket should not list (moved to `Disabled Cores/`, reversible), suggested by a core health check.
- **Files and saves per core:** drag ROMs, BIOS files and media onto a core; browse saves, save states and screenshots (attributed to the
  core that made them).

**Music**
- **Sync** your music folders to the Pocket: scan, read tags, normalise names the player can open, copy, import playlists, build and
  verify the library index, and report exactly what changed. Source files are never modified.
- **Convert** what the Pocket cannot play (hi-res or unsupported FLAC, WAV, M4A and more) to FLAC or MP3 on the copies, with
  **ReplayGain** added; **cover files** for every album from a folder image or the art in the tracks.
- **Skip silence:** measures lead-in and tail silence per track so the player can skip it (on by default, adjustable).
- **Library workbench:** problems with plain reasons and fixes (names, paths, duplicates, covers, tags), tag editing on the copies,
  **named playlists** (add albums to a playlist from the library, on the computer or already on the card), search and filters.
- **Card-space bar** and a verification ledger that makes repeat scans fast, plus fast loading of local and network libraries.
- Copy, move and compare media **between cores and cards**; backups outside the card.
- Players that only play from playlists (such as the HarpMudd MP3 core) are supported, with per-playlist limits.

**Tau extras**
- **Halcyon EQ:** edit presets, import Equalizer APO / AutoEQ profiles, write them to `tau-assets.bin`, preview the response curve.
- **Appearance:** theme editor and install for the Tau core.
- **Meters** preview lab (synthetic until a final meter set ships).
- **Send diagnostics:** reads the Tau Check report codes from screenshots (QR and pixel-grid), shows the verdict, and builds a zip for
  a bug report. Nothing is uploaded.

**Safety, by design**
- Every card write is **plan → review → confirm → verify**, with backups kept outside the card, a journal, rollback, a write lock and
  a safe eject. See `SAFETY_RULES.md`. Tests that write to a real card refuse any volume not named `CARDWRITE`.

**Command line:** the `tau` CLI does everything the app does, with `--json` output (`tau --help`).

## Where it is going
- A **manager for the whole device**, not one core: media libraries (music first, then others), ROM sets, saves, screenshots, firmware
  and settings, for the Pocket first and further Analogue devices after it.
- **Integrations** with the places the community already uses (the openFPGA Library, GitHub releases, other managers' formats), through
  the same reviewed-plan path.
- **Extensible and developer friendly:** a portable engine with no UI in it (`tau-core`), a plain CLI and JSON surface, documented byte
  formats, and clear extension points. Extension APIs are not frozen yet; the reorganisation is where they take shape.

This replaces the earlier goal of being adopted by Pocket Sync as a library. That was dropped because the two projects started to overlap
heavily, Pocket Sync was not as flexible as these goals need, and we do not want to push AI-written code into another project or
try to share compact features without the know-how to do it well. Decisions D-009 and D-017 in `docs/DECISIONS.md` record it; D-010's
licence boundary still holds (our code may flow into AGPL projects, theirs is never copied back).
The next ordered list of work is `ROADMAP.md`.

## For developers
Rust engine (`crates/tau-core`, portable and UI-free), audio conversion (`tau-audio`), network access (`tau-net`), Tauri 2 host
(`src-tauri`), Svelte UI (`ui`), CLI (`crates/tau-cli`).

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --features tau-core/serde
cd ui && npm install && npm run check          # browser suites: npm run dev, then bash scripts/run-all.sh
cd src-tauri && cargo tauri dev                 # live app with hot reload
tools/dev-build.sh                              # a local test build (Dev channel), filed in releases/dev-builds/
```
Release steps: `docs/RELEASING.md`. Contributor and agent rules: `CLAUDE.md`.

## Documents
| File | What it is |
|---|---|
| `ROADMAP.md` | The one ordered list of what is next. |
| `docs/STATUS_HANDOFF.md` | Current state and handoff, newest block first. |
| `docs/DECISIONS.md` | Numbered standing decisions (do not re-litigate; add a new one). |
| `docs/PRODUCT_DESIGN.md`, `SPEC.md` | Product design and functional specification. |
| `ARCHITECTURE.md`, `docs/ARCHITECTURE_ROADMAP_REVIEW_2026-10-02.md` | Stack, crates and module layout. |
| `DATA_FORMATS.md` | Byte formats: library index, assets container, cover files, card and core layouts. |
| `SAFETY_RULES.md` | Rules for anything that touches an SD card or a music folder. |
| `docs/MARKETPLACE_SPEC.md`, `docs/CORES_SECTION_SPEC.md`, `docs/CORE_DISABLE_SPEC.md` | Cores: marketplace, per-core pages, disabling. |
| `docs/CONVERSION_AND_REPLAYGAIN.md`, `docs/SKIP_SILENCE.md` | Music conversion, ReplayGain and silence skipping. |
| `docs/FIRMWARE_SYNC.md` | What is read from the Tau Alpha repository and how it is checked. |
| `docs/RELEASING.md` | Installers, channels, the in-app updater, publishing. |
| `TEST_PLAN.md`, `docs/REAL_CARD_PASS.md` | Tests and the real-card checklist. |

## Related project
[Tau Alpha](https://github.com/alfatreze/Tau-Alpha) is the Tau core itself (firmware, RTL, tools). Tau Omega reads from it and never
changes it; the two repositories never share files. A Tau feature is planned here only once it is on Tau Alpha's `main`.
