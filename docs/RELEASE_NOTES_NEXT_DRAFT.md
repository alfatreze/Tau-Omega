# Tau Omega next release (draft, not for publishing yet)

## New: Marketplace
- **Marketplace** page: browse every core in the openFPGA Library (about 330) with its picture, search and filter, and see where it comes from, what Pocket firmware it needs, whether it supports the Dock, what you must supply (a licence file, your own ROM folders), the author's AI declaration if any, and its earlier versions. Nothing is installed until you review it.
- **Install a community core** through the same reviewed plan Tau cores use: Omega downloads the zip, checks it (paths, size, identity, GitHub's checksum when published), shows exactly what changes on your card, backs up what it replaces, installs, checks the result, and can undo it.
- **Core updates** on the Cards page: which community cores on the card have a newer release, with "Review all updates, one at a time". A count on the Marketplace item shows when updates exist. Checking follows the existing update setting; installing is never automatic.
- Your ROMs, saves, settings and licence files are never touched by an update.
- A release with no published checksum is allowed and says so.
- Install an **earlier version** of a core from its details ("Choose this version"); going back is a downgrade you confirm in the plan. Files that exist only in the newer version stay on the card.

## New: Cores
- **Cores** in the side menu: every core on the open card by platform, expandable, with a dot on cores that need a file. Pick a platform to see its cores, or a core to open it.
- Each core has **Files** (ROMs, BIOS, tapes and disks, shared by the platform or only for this core), **Saves** (saves and save states), **Screenshots** and **Settings** (missing BIOS, the settings the Pocket saved with their real names, saved controls and video, presets, what the core reads), and a details panel you can hide.
- **Drag files in** (or use Add files / Add folder): you see exactly what would be added, then each file is copied, read back and verified. Nothing is deleted; a replaced file is backed up outside the card first.
- Screenshots: the Pocket does not record which core took a screenshot, so Omega shows the ones taken near that core's save states as "likely this core", says why, and lets you see them all.

## Command line
`tau cores <card>`, `tau core-detail`, `tau core-files`, `tau core-settings`, `tau core-screens`, `tau core-add-plan` / `tau core-add` (add files, plan first), `tau catalog`, `tau catalog-core <id>`, `tau core-updates <card>`, `tau core-download <id> --out <folder>` (download, check, no card write) and `tau update-undo <card> --backup <folder>` (put a card back after an install).

## Fixes and changes
- Community cores are judged by their version, not by Tau's firmware files (an installed core was always reported as repaired, and a newer package as the same build).
- The post-install check treats a slot without an id and a file you must supply (a BIOS) as warnings for community cores.

## Not in this release
Tau's own cores in the Marketplace.
