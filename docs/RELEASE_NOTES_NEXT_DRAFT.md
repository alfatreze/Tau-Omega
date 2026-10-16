# Tau Omega 0.5.0 (release notes; a draft until it is published)

## New: Marketplace
- **Marketplace** page: browse every core in the openFPGA Library (about 330) with its picture, search and filter, and see where it comes from, what Pocket firmware it needs, whether it supports the Dock, and what you must supply (a licence file, a BIOS).
- **Install a community core** through the same reviewed plan Tau cores use: Omega downloads the zip, checks it (paths, size, identity, GitHub's checksum when published), shows exactly what changes on your card, backs up what it replaces, and verifies every file it writes. A release with no published checksum is allowed and says so.
- **Install several together:** tick the cores you want (a selection bar appears), or use "Update all together" on Core updates, or add more zips on the Packages page. They are downloaded and checked one by one and reviewed as one plan.
- **Core updates:** which community cores on the card have a newer release, and a count on the Marketplace item. Checking follows the existing update setting. Your ROMs, saves, settings and licence files are never touched by an update.
- Install an **earlier version** of a core from its details; going back is a downgrade you confirm in the plan.
- **Add a core manually** from a zip, a folder or a GitHub address, with a preview built from the core itself and a trust check before anything is installed.
- After any install or update the Cores list refreshes by itself.

## New: Cores
- **Cores** in the side menu: every core on the open card grouped like the openFPGA Library (category, manufacturer, platform), with a dot on cores that need a file. All Tau builds sit under one TAU row.
- Each core has **Files** (ROMs, BIOS, tapes and disks, shared by the platform or only for this core), **Saves**, **Screenshots** and **Settings**, with a details panel. **Drag files in** (or Add files / Add folder): you see exactly what would be added, then each file is copied, read back and verified. Files can be removed from the Files tab after a review, with every file backed up outside the card first.
- **Hide cores the Pocket cannot use** (a missing BIOS, broken files): moved to `Disabled Cores/`, reversible, nothing rewritten. Ask me, hide automatically, or never suggest it.
- The Cards page is now **Card** (Overview and History).

## New: Library and sync
- **Faster library loading.** A music folder on this computer or on a network share is remembered by where it is, so a repeat load only reads files that changed. The Library page shows the last list at once, then checks for changes behind it. Files are read several at a time, and a plain listing no longer reads the technical details the card's index needs.
- **Players that play only from a playlist** (such as the HarpMudd MP3 player) are recognised from their own settings. Omega keeps `playlist.m3u` in step with the tracks on the card and keeps to the core's limits: 256 tracks, a 12 KB playlist file and a 12-character playlist name. The Library says so and stops a sync that would go over.
- **Skip silence** (on by default, under Advanced on the Library page and in Settings > Syncing music): Omega measures the silence at the start and end of each MP3 and FLAC and writes a small `tau-skip.bin` next to the tracks. Your music files are never changed. The Pocket skips the silence when the player's own Skip silence switch is on (a player version that supports it). Settings: silence level, silence kept at each end, and the smallest silence worth skipping. **Adjust tracks on the card** shows each track's waveform, lets you drag or type the cut points, or mark a track as never skipped.
- The storage bar now compares the selected core, other cores' media, other data and what you are adding.
- A first sync with Skip silence on shows "Measuring silence" and decodes each track once; later syncs reuse the result.

## New: tools for core developers (off by default, Settings > Show developer tools)
Builds (a folder of test builds grouped into series with your own naming schemes, installed through the reviewed plan with the card's media carried over), Results (Check screenshots read and compared with the last run), Check (rules a core must pass), and Builder (a Quartus `.rbf` plus a template core becomes an installable package). A card can be marked a test card. Typing the card's name before installing is off; switch it on in Settings if you want it.

## Look and feel
- The side menu keeps the app name and card selector fixed; the menu is the only thing that scrolls.
- Scroll bars are slim, in the primary green, appear only where content overflows and only while you scroll or point at them, and are never longer than 96 px.

## Command line
`tau cores <card>`, `tau core-detail`, `tau core-files`, `tau core-settings`, `tau core-screens`, `tau core-add-plan` / `tau core-add`, `tau core-disable` / `tau core-enable` / `tau cores-disabled` / `tau core-health`, `tau catalog`, `tau catalog-core <id>`, `tau core-updates <card>`, `tau core-download <id> --out <folder>`, `tau update-undo <card> --backup <folder>`.

## Fixes
- Installing the same core again after an undo or a removal no longer stops with "this plan was already run once": the earlier backup is kept under a new name.
- Community cores are judged by their version, not by Tau's firmware files (an installed core was always reported as repaired, and a newer package as the same build).
- The post-install check treats a slot without an id and a file you must supply (a BIOS) as warnings for community cores.

## Known limits
- Skip silence needs a player that reads `tau-skip.bin`; until then it only writes the files. More than 340 tracks in one folder are left out of the file with a warning.
- Tested on macOS only; Windows and Linux builds are produced but their card handling (write lock, eject, read-back) has not been exercised.
- Tau's own cores are not offered in the Marketplace.
