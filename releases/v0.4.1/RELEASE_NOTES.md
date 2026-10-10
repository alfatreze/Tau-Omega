# Tau Omega v0.4.1

Published 2026-10-10: https://github.com/alfatreze/Tau-Omega/releases/tag/v0.4.1 (installers for macOS, Windows and Linux, signed updater bundles, `latest.json`). The macOS installers here are the exact files from that release (checksums verified). **Unsigned**: right-click the app > Open the first time. Test on a throwaway card (CARDWRITE), never a real one.
`0.4.0` was tagged but never published; 0.4.1 is the same plus the unified page layout.

## What is new since v0.3.0
- Pixel-grid report codes (TPG1/TPG2) are read as well as QR codes, in the QR viewer and Send diagnostics; report tags 23-27 are decoded (heap, CPU load, Info rows, now playing).
- Halcyon EQ page: user presets (control sliders, or an Equalizer APO / AutoEQ import) written to `tau-assets.bin`; proven on a Pocket (TAU Preview v0.6.0-preview.1).
- Install and update paths for Tau cores (first install, update from GitHub or a zip, Refresh library, settings migration, removal that keeps your music), adapted to Tau's release channels.
- `tau` command line: `--json` verbs for everything the app does (report codes, packages, remove, update, library refresh, workbench changes, themes, Halcyon presets, diagnostics).
- Tau Omega can update itself (Settings > Tau Omega updates); signed updates only, nothing installs without your click. 0.4.1 is the first version with the updater, so it has nothing newer to find until a later release.
- One page layout on every screen (eyebrow, 26px title, one line, like the Library); page titles match the navigation.
- Installers for macOS (dmg), Windows (setup .exe, .msi) and Linux (.AppImage, .deb); larger engine files split into modules (no behaviour change).

## Not done yet
Installer signing (no Apple Developer ID or Windows certificate yet, so macOS Gatekeeper and Windows SmartScreen warn on first run), and a real in-app update from one published version to the next (needs 0.4.2 or later).
