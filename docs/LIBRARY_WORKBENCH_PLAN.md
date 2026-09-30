# Library workbench: plan for approval

Status: **proposal, nothing built yet.** Replaces the separate *Library* and *Sync library* screens with one card-centred screen.

## 1. Problem today
* **Library** is read-only and points at a folder you type or choose; it does not follow the card you picked.
* **Sync library** is a form: pick source folder(s), pick a destination, review, run. Sources are whole folders, there is no view of what is already on the card, no capacity picture, and the only feedback is a log location.
* Nothing lets you remove, rename, re-tag or re-cover what is already on the Pocket.

## 2. Mental model (borrowed from Zune)
Zune's core idea: **the device is a place you drag things to**, and a **sync list** is the staged set of changes. You build the list, look at the capacity bar, press Start. We adopt exactly that:

> **Everything you do is staged in one "Pending changes" list. Nothing touches the card until you press Start sync.**

This is also our existing safety rule (plan, review, confirm), so the UI and `SAFETY_RULES.md` agree instead of fighting.
Adds, removals, renames, tag edits and cover changes all live in that one list, so there is one thing to understand and one button to press.

## 3. Screen layout

```
[ Card: Pocket (E:)  v ]  [ Core: tau.omega v ]            Connected: direct USB (slow)
+-------------------------------------------------------------------------------------+
| 12.4 GB on Pocket | +412 MB queued | 18.1 GB free of 31 GB   [=====#####-------]    |
+---------------------------------------+---------------------------------------------+
| THIS COMPUTER                          | ON POCKET: tau.omega  (7,180 tracks)        |
| Source: ~/Music/Jazz  [change v]       | [Albums|Artists|Tracks|Playlists] [search]  |
| [Albums|Folders] [search]              |                                             |
|  [ ] Kind of Blue        On Pocket     |  Blue Train      Coltrane   62 MB   ...     |
|  [x] A Love Supreme      + Add         |  Giant Steps     Coltrane   58 MB   ...     |
|  [ ] Mingus Ah Um        Changed       |  (right-click / row menu: Remove, Rename,   |
|                                        |   Edit info, Change cover)                  |
|        drag albums right, or [Add 2 ->]                                              |
+---------------------------------------+---------------------------------------------+
| PENDING CHANGES (3 albums, 41 tracks)  Add 2 . Remove 1 . Edit 0    [Clear all] [Start sync] |
|  + A Love Supreme   -  412 MB [x]     - Old Compilation (remove)  [undo]            |
+-------------------------------------------------------------------------------------+
```

* **Top bar:** card and core selectors (the same selection the sidebar already holds); connection type; **capacity bar** always visible.
* **Left, "This computer":** browse a local folder as albums or folders; badges tell you each album's state: *On Pocket*, *Changed* (differs from the card copy), *New*.
* **Right, "On Pocket":** what the selected card+core actually holds, with Albums / Artists / Tracks / Playlists views and search. This is the current Library screen, but for the card.
* **Bottom, Pending changes:** the sync list. Each row has its own remove (x) and undo; **Clear all** empties it; the header counts adds, removals and edits.
* **Add:** checkbox selection then **Add** button, or drag albums/folders/tracks from left to right. Both work, and both are keyboard accessible (Enter/Space on selection, no drag needed).

### The numbers you asked for
"How many I added, how much space, versus what I'm using" is the capacity bar plus the Pending header:
`12.4 GB on Pocket + 412 MB queued = 12.8 GB, 17.7 GB free`. It turns amber above 90% and red if it will not fit, and **Start sync is disabled with the reason stated** if it will not fit. The figure is computed by the same planner that runs the sync, so it cannot disagree with the result (a known iTunes complaint, see section 8).

### Removing things
* Remove from the Pending list: `x` on a row, multi-select delete, or **Clear all**.
* Remove from the Pocket: row menu > Remove, or select and press Delete. It is staged as a strikethrough row and only applied on Start sync. **Undo** is available until then.
* Removing needs a safety net (section 6, decision D2).

## 4. Start sync and progress
1. **Start sync** opens a compact review sheet: adds / updates / removals / edits, total bytes, free space after, warnings. One **Confirm** button. (Uses the existing plan id and confirmation token.)
2. **Progress panel** replaces the "log location": overall bar, current file, files done / total, MB copied, speed, time remaining, phase (Copying, Verifying, Writing index), **Cancel**. The window stays usable.
3. **Result:** "41 tracks added, 1 album removed, index verified", with **Show details** and a link to the saved journal (still written, now secondary). Failures list the file and reason, and say plainly that nothing was reported complete.
4. The left and right panes refresh after a run so the card view matches reality.

## 5. Slow-connection alert
Trigger: connected directly to the Pocket (its USB mode) **and** the Pending change writes more than **10 MB**. Shown when pressing Start sync, not while browsing:

> **You're connected to the Pocket directly.** Transfers over this connection are slow and this may take a long time (about *N* min for 412 MB). Continue, or use a card reader for big syncs?
> [Cancel] [Sync anyway]

Detection is the risky part; we cannot assume it. Plan, in order:
1. Read the USB device/vendor of the volume (macOS, Windows, Linux each have a way) and match the Pocket.
2. Fall back to measuring real write speed for the first seconds of the run; if it is below a threshold, show the same alert then, with a live estimate.
3. Let the user set "How is this card connected?" per card, remembered.
The estimate uses measured speed once known. **Needs your Pocket to verify** (hardware acceptance item).

## 6. Editing what is on the Pocket
Row or multi-select menu: **Rename, Edit info, Change cover**. All staged, all applied on Start sync, all on the **card copy only**; the source files on your computer are never modified.

| Action | What it does | Engine work |
|---|---|---|
| Rename | Renames album/artist/title (tag values), and file/folder names to the ASCII form the player needs. Playlists that point at renamed files are rewritten. | Tag write, path rewrite, playlist rewrite |
| Edit info | Title, artist, album artist, album, track no., disc, year. Multi-track editing (whole album at once). | New tag writer: ID3v2 for MP3, Vorbis comments for FLAC; write to temp, verify, replace |
| Change cover | Pick an image; validated (baseline JPEG, size cap, decode time) and embedded; live preview; problems shown before staging. | Reuse existing `embed_mp3_copy` / `embed_flac_copy` and cover validation |

After any of these the **index is rebuilt and verified last**, as for a normal sync.

Two consequences that need a decision (D1, D3):
* Today `SPEC.md` lists "editing tags in the user's files" as out of scope. Editing the **card copy** does not violate `SAFETY_RULES.md` rule 1 (sources untouched), but the spec should say so explicitly.
* **Re-sync would overwrite edits**, because the card copy no longer matches the source. We must remember edits per card and re-apply them (or skip those files) on the next sync. Otherwise users lose their fixes silently.

## 7. Cards and cores (state model)
* The **selected card + core** drives everything: the right pane, capacity, the Pending list, the source memory.
* Each (card, core) pair keeps its own **Pending list** and **remembered source folder(s)**, stored in the app data folder keyed by card and core id. Switching shows the right one; the sidebar card chip shows a "3 pending" badge if you leave with unsent changes.
* Each core's media root is its own library (as the engine already models it). The right pane always names it: "On Pocket: tau.omega".
* Later: the left pane's source can be **another card's library** (uses the existing `plan_core_copy`), so "copy this album from card A to card B" is the same drag.
* Card removed mid-session: banner "This card is no longer connected" (exists today), pending list is kept, Start sync disabled until reconnected.

## 8. Research summary and what it changed
Evidence is limited to reviews and forum threads (no formal studies found); treat it as direction, not proof. Pocket Sync (the closest neighbour) manages saves, cores and screenshots and has **no music library**, so there is no direct competitor to copy.

| Reference | What users say | What we do |
|---|---|---|
| [Zune software reviews](https://m.gsmarena.com/reviewcomm-539p2.php), [Engadget](https://www.engadget.com/2006-11-15-zune-review.html) | Clean, fluid UI; sync list plus capacity picture; auto-sync when the library exceeds the device | Adopt sync list and capacity bar. **Skip** auto-selection by rating: our users choose deliberately. |
| [iTunes/Finder sync complaints](https://kirkville.com/how-i-would-fix-itunes-part-6-fix-syncing/), [Macworld](https://www.macworld.com/article/225493/6-itunes-problems-apple-needs-to-fix.html) | Manual vs automatic mode confusion; capacity bar disagreeing with reality; unreliable, opaque failures | **One mode** (staged list, explicit Start). Capacity from the real plan. Plain-language errors. |
| [MediaMonkey vs foobar2000 vs MusicBee](https://hardforum.com/threads/media-monkey-vs-foobar2000.1617599/) | MediaMonkey does true sync (removes what you removed) but has a learning curve; foobar is a simple "send to" without removal; MusicBee users struggle choosing the destination | True sync via staged removals, but visible and undoable. Destination is **implicit** (selected card + core), never a folder picker. |

Principles applied: staged changes with undo instead of confirmations everywhere; immediate visible feedback on every action (counts, capacity); progressive disclosure (details behind "Show details"); drag **and** keyboard/button paths; empty states that say what to do next; no destructive action without a visible undo or backup.

I did not find a product-design skill in this environment, so I applied these principles directly. If you have a specific skill or a design system you want followed, tell me and I will use it in Phase 0.

## 9. Engine and backend changes needed
| Need | Today | Change |
|---|---|---|
| Add individual albums/tracks | Plans take whole source folders | Plan accepts an include-list of paths |
| See size per album on card | Track rows have no size or path-on-disk | Add bytes, cover flag, album key to the scan |
| Removal of tracks | Only whole-core removal, and mirror-delete needing a backup folder | Per-item removal plan with backup/undo policy (D2) |
| Progress | stage, done, total, path | Add bytes done, speed, ETA |
| Tag writing | None (readers only) | New writer module with tests on fabricated files |
| Cover change | Embedding into copies exists | Extend to in-place on card copy |
| Edit memory | None | Per-card edit manifest re-applied on sync |
| Connection type | None | USB detection per OS plus measured speed |
| Persisted pending list | None | App-data store keyed by card+core |

Path to keep `tau-core` clean: all rules stay in the engine (TP0 boundary), the UI only renders plans.

## 10. Phases (each ends in a mergeable state, reviewed by screenshots before you pull)
| Phase | Delivers | Verification |
|---|---|---|
| **P0 Design in the browser** | Clickable prototype of the new screen on the existing dev mock, all states (empty, adding, over capacity, syncing, done, error, slow warning). No engine changes. | Screenshots for your approval. Nothing to break. |
| **P1 Browse and stage** | Card+core selector wiring, two panes, capacity bar, Pending list, add/remove/clear, remembered sources and per-card state, drag and button. Reads only. | Component tests, screenshot tests, fake-card Rust tests for scan sizes. |
| **P2 Sync and progress** | Item-level plan, review sheet, progress with bytes/speed/ETA, cancel, result, slow-connection alert, old Sync screen retired. | Fake-card sync tests (incl. interrupted, disk full), source hashes unchanged, re-run is a no-op. **Hardware run by you.** |
| **P3 Remove from Pocket** | Staged removal, undo, backup policy, index rebuild. | Fake-card tests; playlist cleanup checked. |
| **P4 Rename, edit info, cover** | Tag writer, rename with playlist rewrite, cover change, edit memory across re-sync. | Byte-level tests on fabricated MP3/FLAC, round-trip through the reference index. **Hardware run by you.** |
| **P5 Polish** | Keyboard paths, accessibility (axe), empty states, help text, docs (`SPEC.md`, `TEST_PLAN.md`), cross-card copy as a source. | Full test plan pass. |

Suggested order of value: P0, P1, P2 gives you what you described first (browse, add, capacity, sync with progress, slow alert). P3 and P4 follow.

## 11. Decisions (approved 2026-09-30)
* **D1. Tag/cover editing:** allowed on the **card copy only**. `SPEC.md` to state this explicitly (P4).
* **D2. Removal safety:** a **user preference in Settings** (Settings > Library > Removing from the Pocket: ask each time / back up then remove / just remove). The **first** removal shows a one-time explanation, including where to change the preference later. Default: back up to a computer folder, then remove.
* **D3. Edits vs re-sync:** edits are **re-applied** on later syncs (per-card edit manifest).
* **D4. Old screens:** Library and Sync library are **retired** when P2 lands. Until then they remain, and the new screen is "Library workbench".
* **D5. Slow-connection alert:** shown the **first time** with a "Don't ask again" checkbox. After that, only a **small contextual note next to Start sync** ("Direct connection: about 5 min"). The note shows whenever the rule applies, even before the checkbox is ticked.
* **D6. Playlists:** stay on the Playlists page; paths are rewritten automatically on rename/remove.
* **Added: auto-detect and refresh.** Omega detects a card or the Pocket being connected or removed and refreshes on its own (polls the mounted-volume list every 3 s, in addition to the existing refresh on window focus). A subtle refresh icon at the top right forces a re-read. Implemented in P0 in `App.svelte`; verify on real hardware (Pocket USB mode may appear differently from a card reader).

## 12. Status (P0 to P5 built; hardware not yet exercised)

| Phase | State | Where |
|---|---|---|
| P0 prototype | Done, superseded by the real screen | |
| P1 browse and stage | Done: card and local listings, Albums/Artists/Tracks/Playlists, capacity from the real volume, pending list saved per card and core, drag or button | `ui/src/lib/Workbench.svelte`, `crates/tau-core/src/workbench.rs` |
| P2 sync and progress | Done: album-level plan, one confirmation for adds/removals/edits, journal, live progress with measured speed, cancel, slow-connection alert and note, Sync library and old Library screens retired | `changes.rs`, `journal.rs`, `src-tauri/src/main.rs` |
| P3 remove from Pocket | Done: staged, undoable, backup per preference, playlists rewritten, index rebuilt | `workbench::plan_removal`/`execute_removal`, Settings > Library |
| P4 rename, edit info, cover | Done: album title/artist/album artist/year, track title, cover; ID3v2.3/2.4 and FLAC; edits recorded on the card and re-applied after a re-sync | `crates/tau-core/src/tagedit.rs` |
| P5 polish | Partly done: keyboard-reachable controls, states and empty states, docs and tests. Not done: automated accessibility audit (axe), cross-card copy as a source | |
| Auto-detect | Done in code for macOS, Linux and Windows (polls mounted volumes every 3 s, refreshes on window focus, refresh button); not verified on hardware | `ui/src/App.svelte`, `src-tauri/src/device.rs` |
| Tools and settings menu | Done: Compare cores, Backup, Packages, Problems, Recent jobs and Settings sit in a collapsible section; Library preferences live in Settings | `ui/src/App.svelte`, `LibrarySettings.svelte` |

**Decisions taken while building (worth a look):**
* *Album = a folder that directly contains audio.* A multi-disc release with `Disc 1`/`Disc 2` sub-folders shows as two albums.
* *Rename = tag edit.* Renaming an album, artist or track changes its tags on the card copy; file and folder names are not changed, so playlists keep working.
* *"On Pocket" vs "Changed"* compares the number of tracks in the matching folder, not file contents (cover embedding changes bytes). The review sheet shows the exact new/updated/unchanged counts from the real plan.
* *An album cannot be added and edited or removed in the same run;* the engine refuses it and the UI blocks it. Sync first, then edit.
* *The 10 MB rule* is taken from the Pocket's own USB screen. Speed estimates are never invented: the app shows time only from the last measured transfer on that card.
* *Tag writing refuses tags it cannot rewrite safely* (ID3 unsynchronisation, extended headers, footers) rather than risk damaging a file, and the file is left untouched.
* *Existing reader limitation:* the index reader stops UTF-16 text at its first zero byte, so UTF-16 ID3 tags read as blank. Not changed here, because the index must match the Python reference byte for byte; edits that need Unicode in an old v2.3 tag upgrade that tag to v2.4 (UTF-8), which the reader handles.
* *Safety rule 4* now records your D2 decision, with backup as the default (see `SAFETY_RULES.md`).

**Run and verify:** `cargo test --workspace`, `cargo test -p tau-core --features serde`, `cd ui && npm run check`, and with `npm run dev` running: `NODE_PATH=$(npm root -g) node scripts/workbench-test.cjs` (42 checks) and `scripts/workbench-states.cjs` for screenshots. Hardware checks are in `TEST_PLAN.md` (Owner acceptance runs, item 5).
