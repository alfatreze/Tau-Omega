# Tau Omega v0.5.1

## Playlists
- **Add to playlist…** in the Library (for albums on this computer and for albums already on the card): choose a playlist that exists, or name a new one. New albums are copied by the same sync; the playlist is written (tracks appended to the end, none twice) after the copy and the library index is read again, so the Tau player lists it.
- **Players that play only from playlists** (the HarpMudd MP3 player): playlists are now yours to choose, up to 256 tracks and 12 KB each. Albums added with the normal button go into the default `playlist.m3u`. Omega keeps `playlists.m3u` (the list of playlists) so names longer than 12 characters are remembered. The old behaviour (one playlist of every track on the card, which stopped a sync past 256 tracks) is gone. The format of `playlists.m3u` (one playlist file name per line) is assumed from the core's documentation and should be checked on a Pocket.

## Updates for Tau Omega itself
- **Channels** (Settings > Tau Omega updates), the same three as Tau Alpha: **Stable** (published releases), **Preview** (published pre-releases) and **Dev** (local test builds installed by hand, no offers); **Automatic** follows the build you run. Choosing a different channel from your build offers a switch to that channel's newest build even when its version is lower (Stable on a test build offers the newest release); your settings are kept.

## Library
- A short footer with two properly laid out buttons (Add to playlist…, Add N to Analogue Pocket). Albums can be dragged to the Pocket with the pointer.
- Faster loading: local and network libraries are remembered by folder path (a saved list shows at once while the folder is checked), the scan reads several files at a time, and the folder listing no longer reads track details.
- The storage bar shows the selected core against other cores and other data. Slimmer scroll bars.
- Install confirmations: no typed card name by default (an opt-in switch under developer tools). The Cores list refreshes after every install or update. Installing the same core again after an undo no longer stops.

## Fixes
- Internal: a flaky install test (parallel tests shared scratch folders) is fixed; new tests for the parallel scan and the per-folder library memory.

(The 0.5.0 notes are on its GitHub release.)
