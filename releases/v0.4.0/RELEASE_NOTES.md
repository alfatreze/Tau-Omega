# Tau Omega v0.4.0 (release candidate for local testing; GitHub draft, not yet published)

The macOS installers here are the exact files from the draft release `v0.4.0` on `alfatreze/Tau-Omega` (built by CI from tag `v0.4.0`, commit `7f9af27`), checksums verified. **Unsigned**: right-click the app > Open the first time. Test on a throwaway card (CARDWRITE), never a real one.
The Windows (`.exe`, `.msi`) and Linux (`.AppImage`, `.deb`) installers and the updater bundles are only in the GitHub draft.

## What is new since v0.3.0
- Pixel-grid report codes (TPG1/TPG2) are read as well as QR codes, in the QR viewer and Send diagnostics; report tags 23-27 are decoded (heap, CPU load, Info rows, now playing).
- Halcyon EQ page: user presets (control sliders, or an Equalizer APO / AutoEQ import) written to `tau-assets.bin`; proven on a Pocket (CARDWRITE, TAU Preview).
- Install and update paths for Tau cores (first install, update from GitHub or a zip, Refresh library, settings migration, removal that keeps your music), adapted to Tau's release channels.
- `tau` command line: `--json` verbs for everything the app does (report codes, packages, remove, update, library refresh, workbench changes, themes, Halcyon presets, diagnostics).
- Tau Omega can update itself (Settings > Tau Omega updates), signed updates only; nothing installs without your click.
- Installers for macOS, Windows and Linux; larger engine files split into modules (no behaviour change).

## Checks done
CI green on macOS, Windows and Linux (fmt, clippy, 298 engine tests, browser checks); signed updater bundles built; `latest.json` generated from the real artefacts. Not yet done: a real in-app update from one published version to the next, and installer signing (no Apple or Windows credentials yet).
