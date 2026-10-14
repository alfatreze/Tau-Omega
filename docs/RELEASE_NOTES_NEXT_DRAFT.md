# Tau Omega next release (draft, not for publishing yet)

## New: Marketplace
- **Marketplace** page: browse every core in the openFPGA Library (about 330), search and filter, and read where it comes from, what you must supply (a licence file, your own ROM folders) and its newest release. Nothing is installed until you review it.
- **Install a community core** through the same reviewed plan Tau cores use: Omega downloads the zip, checks it (paths, size, identity, GitHub's checksum when published), shows exactly what changes on your card, backs up what it replaces, installs, checks the result, and can undo it.
- **Core updates** on the Cards page: which community cores on the card have a newer release, with "Review all updates, one at a time". A count on the Marketplace item shows when updates exist. Checking follows the existing update setting; installing is never automatic.
- Your ROMs, saves, settings and licence files are never touched by an update.
- A release with no published checksum is allowed and says so.

## Command line
`tau catalog`, `tau catalog-core <id>`, `tau core-updates <card>`, `tau core-download <id> --out <folder>` (download, check, no card write) and `tau update-undo <card> --backup <folder>` (put a card back after an install).

## Fixes and changes
- Community cores are judged by their version, not by Tau's firmware files (an installed core was always reported as repaired, and a newer package as the same build).
- The post-install check treats a slot without an id and a file you must supply (a BIOS) as warnings for community cores.

## Not in this release
Core artwork (icons and platform images), Tau's own cores in the Marketplace, a GitHub token setting.
