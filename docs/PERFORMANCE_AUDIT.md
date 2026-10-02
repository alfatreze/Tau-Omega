# Performance and card-safety audit (2026-10-02)

Trigger: a macOS hang report showed the app frozen for 71 s in `album_thumbnails` on a Pocket in USB
mode (about 0.7 MB/s). This audit lists what was fixed, what the evidence says is next, and a design
for a cache that makes everything after the first scan cheap without ever trusting a stale answer
about the card.

## Status of the architect-review items (updated after the correctness pass)

| Item | State |
|---|---|
| D1 cover-embedded copies unverified | **Fixed**: bytes are hashed before writing and the read-back must match (`verify_written`, shared by plain and embedded copies; test proves it rejects wrong bytes) |
| D3 case-insensitive collisions | **Fixed**: the collision set folds case; test |
| D4 interleaved removal | **Fixed**: backup-volume space check, phase 1 backs up and verifies everything in one streaming pass over the card, phase 2 deletes, cancel ignored during the quick delete phase; test that a failed backup of file 2 leaves both files on the card. Card reads per removed file: 4 -> 2 (plan + one pass). The mirror and core-move routes still use the old per-file `backup_then_delete`; the destination copy is already verified there, so the exposure is lower, but they should move to the same helper |
| D5 index swap | **Fixed**: `.tau-library.tdb.prev` kept until the new index is in place, `recover_index` runs at the start of every confirmed sync, `index_needs_recovery` is ready for a UI banner; tests for every crash point |
| D6 capacity | **Fixed in the engine**: execute refuses before writing anything, counting whole clusters (`fs4::allocation_granularity`), embedded covers, sidecars and the index twice, plus the 16 MiB margin; new `ErrorCode::InsufficientSpace` (51) and message. **Not yet done:** the Library screen's fit bar still uses logical bytes, so it can say "fits" and then be refused |
| D8 sidecars written in place | **Fixed**: temp, verify against the intended bytes, rename |
| D9 leftover temp files | **Fixed**: `sweep_stale_temps` removes this tool's own temp files older than an hour from the folders a run writes to |
| D7 names FAT cannot hold | **Deferred on purpose**: `ascii_name` is held byte-for-byte to the Python reference by the conformance tests, so it cannot change here alone. Needs a decision with tau-alpha (trailing dots, reserved device names), or a plan-time warning that does not alter names |
| D2 no-cache read-back and safe eject | Not started (next) |
| D10 per-volume I/O governor | Not started |
| P2, P3, P4, P5, P7 and the ledger | Not started |

Known limit of D1: the embedded copy is built from a second read of the source, a few milliseconds
after the source hash check; a source rewritten in that window would be embedded as it is then. The
read-back check still proves what reached the card is what was built. Closing it needs the streaming
embed (P3).

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

## Ledger red-team (second pass, 2026-10-02)

The first design above trusted "size + mtime match". Checked against the code and against how FAT
cards behave, that is not enough on its own. Findings, worst first.

**Findings in the existing copy code that the ledger design must not inherit**
1. **Embedded-cover copies are never verified.** `copy_verified` (cover branch) ends with
   `if sha256_file(&temp)?.is_empty()`: a SHA-256 hex string is never empty, so the check is always
   false and verifies nothing. `embed_mp3_copy`/`embed_flac_copy` build the bytes in memory and
   `fs::write` them; only a successful `write`+`sync_all` stands behind the file. Embedding is the
   Library workbench's default, so this is the common path. Fix: have the embed functions return the
   bytes, write them durably, read back (no-cache, see the security finding) and compare against
   `sha256_bytes(bytes)`.
2. **Re-syncing an album always rewrites it when covers are embedded.** "Same" means card hash ==
   source hash, but an embedded copy differs from its source by design, so every existing file is
   `Update` and is copied again at card speed. This is also why the Library shows "Changed" so readily.
3. **The source is read twice per file at execute** (hash, then `io::copy`), and the card file is read
   back once. One streaming pass that hashes while copying, then the read-back, gives the same safety
   with one fewer full read. (A source that changes between the two reads is caught today only because
   the read-back is compared with the plan hash; that property must be kept.)

**Holes in the first ledger design**
| # | Hole | Severity | Mitigation |
|---|---|---|---|
| H1 | **mtime may mean nothing on the Pocket.** In USB mode the Pocket's own FAT driver may stamp a constant or zero date. Then "mtime matches" degrades to "size matches". Not measured. | high until measured | Measure first: write a file twice over USB and compare. Detect degenerate mtimes at runtime (many entries share one value, or it is before 1990 or in the future) and switch the card to "mtime untrusted": no skip-copy decisions, tags still cached with a head+tail probe. |
| H2 | **Same size and same mtime, different content.** Tools that preserve timestamps (`cp -p`, `touch -r`, re-taggers), and FAT's 2-second resolution make this possible. | medium | Strengthen the fingerprint where the file system allows (local sources: dev, inode, size, mtime **and ctime**, which user code cannot set). For FAT: add a rolling canary (H3) and never use the ledger for safety decisions (invariant below). |
| H3 | **No way to notice slow drift.** | medium | **Rolling verification:** each run fully re-hashes the least-recently-verified card files up to a byte budget (for example 32 MB on direct USB, 256 MB on a reader). A mismatch marks the ledger untrusted for that card and rebuilds it. A "Verify card" button ignores the ledger entirely. |
| H4 | **Racy timestamps** (the git "racily clean" problem): a file modified in the same timestamp tick as its entry was recorded cannot be told apart. | medium | Record the stat taken **before** reading, re-stat after; if it changed, store nothing. Treat any entry whose mtime is within 2 s (plus margin) of the record time as unverified. |
| H5 | **Time zone and DST.** FAT stores local time with no zone; a DST change or travel shifts every mtime on the card by the same amount, invalidating the whole ledger (safe, but a cliff). | low, perf only | If at least 95 percent of entries differ by the same multiple of 30 minutes, rebase the ledger by that constant instead of discarding it. |
| H6 | **Wrong card, same key.** A path key collides for two cards both mounted as `/Volumes/Pock`; a volume UUID is shared by cloned cards. | medium | Key by volume UUID **plus** capacity and FAT serial; with no reliable id (some Linux and Windows cases) use no skip-copy decisions at all and cache tags only. Never key by path. |
| H7 | **Crash between card write and ledger write.** | medium | Record a hash only **after** that file's verified write; flush at the end and on cancel. The bad order (ledger first) is forbidden. A crash then costs a re-hash, never a false "Same". |
| H8 | **Provenance is missing**, which is what makes finding 2 above unsolvable. | design gap | Store, per card file, `(source sha, cover sha, embed version) -> output sha`. A re-sync is "Same" when the destination fingerprint is unchanged **and** the provenance matches. The embed and tag-parser versions are part of the key, so changing the algorithm invalidates correctly. |
| H9 | Stale entries for removed, renamed or case-changed files; FAT is case-insensitive. | low | Garbage-collect on each stat-walk; match on the as-listed path; two entries that collide case-insensitively are dropped. |
| H10 | Corrupt, partial or concurrently written ledger file; two app instances. | low | Temp file + atomic rename, a file lock, version + CRC; a write failure (disk full) is ignored, never fatal. |
| H11 | Poisoned ledger (someone edits the cache file). | out of scope | Same user, same trust boundary as the app itself; the invariant below limits the damage anyway. |

**Invariant the whole design rests on.** The ledger may influence *display* and *whether to skip a
copy*. It must never influence *deletion*, *overwrite* or *verification*: deletes still hash and
back up and verify against the hash before removing; overwrites do not depend on it; write
verification compares against freshly computed hashes. A wrong ledger can therefore cause a **missed
update**, never data loss, and rolling verification bounds how long a missed update can persist.

**Optimizations found**
- Directory-cluster reads dominate a stat-walk (about 100 bytes per file on FAT with long names, so
  roughly 700 KB and about a second for 7,000 files at USB speed), far cheaper than opening files.
- Keep direct USB single-threaded (the Pocket serves bulk transfers one at a time) and use a few
  workers for card readers, where queue depth helps random reads.
- Source hashing is local-disk work and parallelisable with std threads (no new dependency).
- Hash while copying (finding 3) and share one cached scan between `list_library` and
  `rebuild_index`.
- A persistent thumbnail cache keyed like the ledger entries.

**Experiments needed before building** (all need the real Pocket or a card, none done)
1. mtime behaviour of files written to the Pocket over USB (H1): decides whether skip-copy decisions
   are possible there at all.
2. A real read-back cost with and without the no-cache read (security finding).
3. Real stat-walk time on a 7,000-file card over both USB and a reader.

## Architect review of the sync module (third pass, 2026-10-02)

Scope: `sync.rs` (plan, copy, index, mirror), `changes.rs` (combined change sets), `workbench.rs`
(removal), the ledger design, and how they behave on removable FAT/exFAT media, especially a Pocket in
USB mode (about 0.7 MB/s, its own warning says not for large transfers). Findings are from reading the
code unless marked otherwise; nothing here was run against a real card.

**A regression I introduced, now fixed.** `sha256_file` kept a 1 MiB buffer on the stack. Moving
commands to the thread pool put it on threads with much smaller stacks than the main thread. It is now a
heap buffer.

### Data-loss and corruption risks (ranked)

| # | Risk | Why | Fix |
|---|---|---|---|
| D1 | Embedded-cover copies unverified (covered above) | `is_empty()` on a hex digest | return the bytes, verify the read-back against their hash |
| D2 | **"Verified" may mean "read from memory"**, and there is **no safe-eject step** | read-back normally hits the page cache; `sync_all` per file does not flush the *directory entry* of a rename; nothing unmounts the volume; on a USB-mode Pocket the device itself may buffer | no-cache read-back (`F_NOCACHE`/`O_DIRECT`/no-buffering), a final flush, and a **"Safe to remove" state that unmounts (`diskutil unmount`/equivalent) before telling the user they can unplug**. This is the most likely real-world loss path: people unplug after "done" |
| D3 | **Case-insensitive collisions overwrite silently** | the plan's collision check uses a case-sensitive set; FAT is case-insensitive, so `Song.mp3` and `song.mp3` in one folder both "copy" and the second replaces the first | fold case in the collision set; report as a collision |
| D4 | **Removal order is interleaved** (back up file 1, delete file 1, back up file 2...) | a failure or cancel mid-album (host disk full, user cancel) leaves a half-deleted album; no check that the backup folder has room | two phases: back up and verify **all** files, check free space on the backup volume first, then delete; disable cancel during the delete phase or finish the current album |
| D5 | **Index replace is not atomic on FAT, and a stale index is the firmware's truth** | `rename` over the existing index can leave neither file if the card drops mid-rename; an interrupted sync/removal leaves the old index listing deleted files or missing new ones | keep the old index as a `.prev` until the new one is verified (recover on next open), and detect staleness at open (see below) |
| D6 | **Capacity check ignores cluster slack** | it compares free bytes with logical bytes; exFAT clusters can be 128 KB and each per-album sidecar rounds up, so "fits" can be false and the run hits no-space halfway, landing in D5 | round each file up to the volume's block size (`statvfs`) and keep a margin |
| D7 | **Names FAT cannot hold** | `ascii_name` replaces `<>:"|?*\` but not trailing dots or spaces (Windows and some macOS FAT drivers drop them, so the planned path and the real path differ), nor reserved device names | normalise trailing dot/space and reserved names at plan time |
| D8 | Art sidecars are written in place | a drop mid-write leaves a truncated `.timg` and destroys the previous good one | temp file + rename, like audio |
| D9 | Leftover `*.tau-omega-<pid>.tmp` and `.tau-library-*.tmp` are never cleaned | nothing sweeps them; they eat space and clusters | list them in the next plan as warnings and remove them as part of a confirmed run |
| D10 | Reads compete with a running write on the same slow link | listing, thumbnails and the focus/auto-refresh all run while a sync is writing | per-volume I/O governor: one heavy job per volume; background refresh and thumbnails pause during writes |

### Performance issues beyond the first two passes

| # | Issue | Evidence | Fix |
|---|---|---|---|
| P1 | **Removal reads each card file four times** | plan hash, execute pre-check hash, `copy_verified` source hash, `io::copy` | one streaming pass that hashes while copying to the backup and compares with the plan hash; with a ledger hash the plan-time read can go too. 1 GB removal at USB speed drops from about an hour to about 25 minutes |
| P2 | **The index is rebuilt (full scan) up to three times in one change set** | `sync::execute`, again after `reapply_for`, again in `tagedit::execute_edit` and `execute_removal` | rebuild once, at the end of the whole change set |
| P3 | **Cover embedding loads the whole file into RAM, twice** | `embed_*_copy` does `fs::read(source)` and builds a second `result` Vec; a 24-bit/96 kHz FLAC is 100-300 MB | stream: write the new tag, then `io::copy` the audio |
| P4 | Source read twice per file at execute | hash pass, then copy pass | hash while copying (see finding 3 above) |
| P5 | Copy buffer | `io::copy` between files uses a small buffer on macOS (estimate) | an explicit 1 MiB copy loop; matters on card readers, not on USB-mode Pocket |
| P6 | Plan re-reads card files whose size matches | `Same` test hashes the card file | ledger provenance (above) |
| P7 | Planning repeats on every checkbox toggle | `replan` | debounce and cancel the in-flight plan |

### Opportunities

1. **Stale-index detection at open, without reading file contents.** The index lists every track by
   path but stores no size or time, so it cannot validate files on its own. Compare its path set with a
   directory walk (names only). A mismatch (files not indexed, or indexed files missing, the signature
   of an interrupted sync) becomes a banner plus a reviewed **Repair index** plan. The index is also a
   cheap seed for the first listing (its text is ASCII-folded, so it cannot replace the ledger's raw tags).
2. **A frozen plan artifact.** The plan carries per-file fingerprints (local sources: size, mtime,
   ctime, inode) and hashes; execute re-checks the fingerprints, hashes while copying against the plan
   hash, and aborts on a mismatch. That keeps the safety property and removes the second and third
   planning passes.
3. **A card session object in `tau-core`**: volume identity, the ledger, the I/O governor and one shared
   scan, instead of each command rediscovering the card.
4. **An intent journal on the host with recovery.** Today the journal records what happened. Writing
   *intent* first (stage, verify, commit, index last) lets the next open say "an earlier sync was
   interrupted at file 12 of 40" and offer a repair as a reviewed plan, instead of the user finding
   out on the Pocket.
5. **`tau-cli bench-card`**: measure sequential read, sequential write, stat-walk time, mtime behaviour
   and read-back (cache vs no-cache) on a mounted card. It answers the three experiments in the
   previous section and produces real numbers for time estimates (the app today shows a rough guess).
6. **Optional, with the owner's approval:** write an empty `.metadata_never_index` at the volume root so
   macOS Spotlight does not index the card after every sync (it competes for the same slow link). It is a
   card write outside the media root, so it must be an explicit opt-in.

### What I would not do
Weaken any of the verification steps to save time; trust file size or mtime alone; write a manifest to
the card outside a confirmed run; parallelise I/O on a direct-USB Pocket.

## Ranked next steps

0. **Correctness first, no cache needed:** D1 (cover verification), D3 (case collisions), D4 (two-phase removal), D5 (index atomicity), D6 (cluster-aware capacity), then D2 (no-cache verify and safe eject). Original item: **fix the embedded-cover verification gap** (finding 1 of the red-team): it is a correctness hole in the default path, independent of any cache.
1. Ledger + incremental scan (items 1, 2 and 3 above), with the red-team mitigations (provenance, rolling verification, racy-timestamp handling, untrusted-mtime mode). Run the three experiments first.
2. `rebuild_index` and `list_library` share one cached scan so a sync scans the card at most once.
3. Coalesce `replan` (debounce, cancel the previous plan) and reuse source hashes by (path, size, mtime).
4. Persistent thumbnail cache keyed by (file, size, mtime); only fetch visible rows.
5. No-cache read-back verification (security finding), with a test that proves the verification read
   bypasses the cache where the OS allows it.
6. Poll hardening: skip non-local volumes, never start a poll while one is in flight.
7. Targeted `ioreg` query instead of the full dump (low priority).

Not recommended: weakening or skipping write verification to save time, trusting file size alone,
or writing a cache file to the card outside a confirmed sync.
