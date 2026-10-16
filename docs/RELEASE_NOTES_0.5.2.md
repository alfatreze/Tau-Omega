# Tau Omega v0.5.2

## Marketplace
- **Multiple sources.** The Marketplace now lists more than one source: the openFPGA Library and **Tau Alpha**. Source chips filter the list; each card shows where it comes from.
- **One Tau card** with **Channel** (Stable, Preview, Dev), **Build** (Standard, Diagnostics) and **Version** dropdowns in the details. Stable and Preview download from GitHub, are checked against the release's checksums, vetted, and installed through the same review as any core. **Dev** builds are local: the card offers "Install a local dev build…", which opens Add a core manually.
- The Tau card's category (Media Players), year and requirements are read from the release package itself, with its own icon and banner, and the publisher's GitHub profile picture as the fallback image.
- A skeleton of cards shows while the list loads (reduced-motion safe, hidden from screen readers).
- A release whose zip does not match its own checksum file is refused (Tau Alpha's v0.5.0 `alfatreze.TAU` zip currently does not, so it cannot be installed from the Marketplace until that release is fixed).

## macOS
- The app bundle is now signed ad hoc as a whole, so a browser-downloaded copy opens through System Settings > Privacy & Security > Open Anyway instead of failing as "damaged and can't be opened". It is still not a Developer ID signature; the first launch warns.

## Project
- README rewritten: Tau Omega is becoming a manager for the whole device (media, ROMs, saves, cores, settings, integrations), starting with the Pocket; the Pocket Sync integration goal was dropped (D-017).
- Internal: a flaky install test is fixed; new tests for the parallel scan and the per-folder library memory.
