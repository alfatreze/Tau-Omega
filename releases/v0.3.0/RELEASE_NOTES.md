# Tau Omega v0.3.0

macOS build (aarch64), `.app` bundle zipped for distribution. The DMG bundler hit a local
Finder/AppleScript automation permission issue on this machine (macOS 26.6.2) unrelated to the app
itself; the `.app` is the shipped artifact for this release.

## What's new since v0.2.0

- **Library screen**: native folder picker, real track rows (title/artist/album from tags, filename
  fallback, duration, format), search + MP3/FLAC filter, virtualised track table for large libraries.
- **Problems**: expanded from duplicate detection only to five categories — duplicates, missing
  title/artist tags, ID3v2.2 tags (unsupported for cover embedding), path issues (non-ASCII names,
  over-length paths, on-card ASCII collisions), and folders with no cover art.
- **Job history**: persists across restarts through a configured reports directory instead of
  one-file-at-a-time manual loading; auto-lists every sync/copy/move journal with a full-detail view.
- **Playlists**: create, rename, reorder, and import (matching an external `.m3u`'s lines against
  the library by path or filename), all through the existing plan → review → confirm → execute
  safety model.
- **Storage planning**: checks whether a plan's write fits on the destination volume (free space,
  16 MiB safety margin), surfaced on the Sync and Compare-cores plan reviews.
- **Backup dry-run**: a new "Backup" page previews backing up an arbitrary folder onto another
  (new/updated/unchanged/destination-only counts, bytes to write) — preview only, no execute yet.
- **Firmware diagnostics**: decodes the persisted Check-report summary (profile, verdict, per-test
  pass/fail, timing/error counters) from a core's `interact_persist.json`, shown on the Settings page.
- **Core package install/update**: a new "Packages" page inspects a release `.zip` and plans
  installing or updating it onto a staging card folder, verifying every file's hash against the zip
  immediately before writing and reading it back to verify after.
- Fixed a real, previously-silent bug: the Settings screen never actually read a real card's
  `interact_persist.json` (it expected `variables` at the JSON root; the real file nests it under
  `interact_persist`) — found by testing against real hardware-captured fixtures instead of a
  hand-written one.

See `docs/STATUS_HANDOFF.md` in the repository for the full detail behind each item.
