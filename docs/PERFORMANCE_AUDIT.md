# Performance and card-safety audit (2026-10-02)

Trigger: a macOS hang report showed the app frozen for 71 s in `album_thumbnails` on a Pocket in USB
mode (about 0.7 MB/s). This audit lists what was fixed, what the evidence says is next, and a design
for a cache that makes everything after the first scan cheap without ever trusting a stale answer
about the card.

## Fixed in this pass

| # | Problem | Fix | Evidence |
|---|---|---|---|
| 1 | `extract_embedded_cover` and `has_embedded_cover` read every audio file **in full** to find a picture in the first few KB | `cover::read_tag_region`: ID3 tag by its declared size, FLAC by walking block headers; audio never read; 16 MiB cap | test proves a cover is found with a 3 MB audio body and the region read is under 1 KB. Synthetic 60 albums x 8 MB: 263 ms (previously 480 MB of reads: about 11 minutes at USB speed) |
| 2 | Every Tauri command except two was a plain sync `fn`, which Tauri runs **on the window's main thread**: any scan, listing, thumbnail batch or sync froze the UI and its progress bar | all commands except `cancel_job` are `#[tauri::command(async)]` | hang report stack: `ipc::protocol::get` -> `main` -> `album_thumbnails` on the event-loop thread |
| 3 | Because commands can now overlap, two card writes could race | `CARD_WRITE` try-lock on the 9 execute commands; a second write is refused with a plain message | compile and unit tests; not exercised under concurrency |

## Where the time still goes (read from the code, local numbers measured)

1. **Full card re-scan, three times per sync.** `list_library` -> `scan_dir_with_progress` reads tags of
   every track (several small reads each). It runs on open, on the Library screen mount, on refresh,
   after every sync (`reloadCard`), and inside `sync::rebuild_index` at the end of every sync. On a
   7,000-track card over USB a one-album sync therefore re-reads the tags of the whole card about
   twice. Locally, 5,000 tracks take 0.35 s warm, so the cost is I/O latency on slow media, not CPU.
2. **Planning hashes both sides, repeatedly.** `plan_sync`/`plan_candidates` SHA-256s every source file
   and, when sizes match, the card copy (a full read of the card file). `execute_changes` re-plans
   (by design, it re-derives the plan to check the token) and `execute` verifies the source hash a
   third time. Toggling a review-sheet checkbox calls `replan`, which repeats all of it.
3. **Removal planning hashes every file to be deleted** (card reads) and the execute step does it again.
4. **Thumbnails are not cached.** Every card reload (`resetThumbs`) re-reads and re-decodes them.
5. **`detect_connection` dumps the whole IORegistry** (`ioreg -l -w0`: 7.1 MB, 0.43 s) for each new
   card. Acceptable, wasteful.
6. **Mount polling every 3 s** touches every volume under `/Volumes` (`is_dir` x2). A stalled network or
   sleeping USB volume can block that call for a long time; it was invisible when commands ran on the
   main thread only because the whole window froze instead. With the async fix, repeated polls could
   pile up threads.

## Security finding to verify (not confirmed by a test)

Write-then-verify reads the file back right after writing. On macOS that read is normally served from
the page cache, so it may verify **memory, not the card**. Recommendation: reopen for verification with
`fcntl(F_NOCACHE)` on macOS (`O_DIRECT`/`posix_fadvise` on Linux, `FILE_FLAG_NO_BUFFERING` on Windows)
after `sync_all`. This keeps the safety rule's intent (the bytes on the card are the bytes we meant).
It costs real read time on slow links, which is the point; the cache design below recovers the time
elsewhere.

## Proposed: a verification ledger (host cache), then an optional card-side manifest

**Ledger (host side, never on the card).** One file per card in the app's cache folder, keyed by the
volume identity (volume UUID from `diskutil`, falling back to path + capacity). Per file:
`rel, size, mtime_ns, tags, secs, fmt` and, when we wrote or verified it, `sha256`.

Rules that keep it safe:
- An entry is **used only if size and mtime both match** the file as it is now (one `stat`, no read).
  Anything else is re-read. A forged mtime can only make us skip a read, never skip a *write check*:
- A "Same, skip the copy" decision requires a ledger hash that came from **our own verified write or a
  full hash**; unknown means hash it. Deletions and backups still verify the backup copy against the
  hash before removing anything, so a wrong ledger value aborts instead of deleting.
- The ledger has a version and a CRC. Unreadable, wrong version or wrong card: ignored and rebuilt. It
  is a cache: deleting it is always safe.

Effects: listing, `rebuild_index`, reload after sync and refresh all become a stat-walk plus reads of
only new or changed files; planning stops re-reading card files that we verified last time.

**Stale-while-revalidate listing.** Show the cached listing immediately, then run a cancellable
background job that stat-walks the card and patches differences, with a quiet "checking for changes"
state. On direct USB, run it single-threaded and pause while a write is running.

**Card-side manifest (optional, phase 2).** A small `tau-omega/manifest.json` next to the existing
`tau-omega/edits.json`, written **only inside an already-confirmed sync** (temp file, read back,
rename), listing `rel, size, mtime, sha256` plus a ledger id. Value: a second computer can skip
hashing. Cost: one more thing on the card that a stale copy could mislead, so it stays advisory (same
size+mtime rule as the ledger) and the app never writes it outside a reviewed run. I would not build
this until the host ledger has proven itself.

## Ranked next steps

1. Ledger + incremental scan (items 1, 2 and 3 above): biggest effect, medium work, contained in `tau-core`.
2. `rebuild_index` and `list_library` share one cached scan so a sync scans the card at most once.
3. Coalesce `replan` (debounce, cancel the previous plan) and reuse source hashes by (path, size, mtime).
4. Persistent thumbnail cache keyed by (file, size, mtime); only fetch visible rows.
5. No-cache read-back verification (security finding), with a test that proves the verification read
   bypasses the cache where the OS allows it.
6. Poll hardening: skip non-local volumes, never start a poll while one is in flight.
7. Targeted `ioreg` query instead of the full dump (low priority).

Not recommended: weakening or skipping write verification to save time, trusting file size alone,
or writing a cache file to the card outside a confirmed sync.
