# Tau Omega next release (draft; 0.5.0 is published)

## Playlists
- **Add to playlist…** in the Library (for albums on this computer and for albums already on the card): choose a playlist that exists, or name a new one. New albums are copied by the same sync; the playlist is written (tracks appended to the end, none twice) after the copy and the library index is read again, so the Tau player lists it.
- **Players that play only from playlists** (the HarpMudd MP3 player): playlists are now yours to choose, up to 256 tracks and 12 KB each. Albums added with the normal button go into the default `playlist.m3u`. Omega keeps `playlists.m3u` (the list of playlists) so names longer than 12 characters are remembered. The old behaviour (one playlist of every track on the card, which stopped a sync past 256 tracks) is gone. The format of `playlists.m3u` (one playlist file name per line) is assumed from the core's documentation and should be checked on a Pocket.

(The 0.5.0 notes are on its GitHub release.)
