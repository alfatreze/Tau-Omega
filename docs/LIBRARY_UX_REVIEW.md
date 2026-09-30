# Library workbench: UX review and recommendations

Reviewer stance: an outside UX reviewer, working from the shipped screens (`ui/scripts/workbench-states.cjs` output) and the code. Method: walk the main journeys, check each against established heuristics (visibility of system status, error prevention, recognition over recall, user control and freedom, consistency, accessibility) and against the references chosen for this feature (Zune sync list, iTunes/Finder capacity and mode complaints, MediaMonkey true sync). Evidence is my own inspection plus the reference reviews cited in `LIBRARY_WORKBENCH_PLAN.md`; **no real users have tried this yet**, so severity is a judgment, and the first thing to do with it is a small usability session (5 people, tasks below).

## What works (keep)
* **One mental model.** Stage everything, review, then Start sync. Counts, capacity and the tray all agree because they come from one source.
* **Capacity is always visible** and blocks the action with a stated reason, instead of failing late (the iTunes complaint).
* **Reversibility is visible.** Removals are marked, struck through and undoable; Clear all and per-item x exist; the first removal explains where the preference lives.
* **Right-sized quietness.** Auto-detect and refresh are unobtrusive; the slow-connection notice escalates once, then becomes a note.
* **Accessibility baseline is solid:** axe reports 0 violations, dialogs trap and return focus, the tab list is valid.

## P0: fix before anyone relies on it (correctness and safety gaps)
**Status: all six done** (see the notes marked *Done*). Two of these were things the plan promised and the first build did not do.

1. **Card unplugged while the Library screen is open: nothing tells you.** The app knows (the Cards screen shows a banner) but the workbench never receives it, so it shows a stale list and leaves Start sync enabled. *Plan section 7 promised: banner, pending list kept, Start sync disabled until reconnect.* Fix: pass `cardMounted` in, show a banner with a Reconnect action, disable Start sync, keep the pending list. *Done:* the Library screen shows a banner, keeps the pending list, and disables Start sync until the card is back.
2. **The Pocket library has hard limits the screen never mentions.** The index caps at 2,048 albums, 16,384 tracks and 1,024 artists (`lib.rs`). Nothing checks these before a sync, so an over-limit run would copy every file and then fail while writing the index. Fix: check at plan time, show "7,180 of 16,384 tracks" next to capacity, and block with a reason. *Done:* the engine refuses an over-limit change set at plan time (tracks exactly, albums estimated from folders); the screen shows "N of 16,384 tracks" and blocks Start sync with a reason.
3. **No "keep the Pocket connected" message while syncing**, and no safe-to-unplug confirmation after. Pulling a card mid-copy is the most likely real-world failure. Add a persistent line during the run and "You can disconnect the Pocket now" on completion. *Done.*
4. **The review sheet doesn't say *what* is changing.** It shows "13 files · 782 MB" over a dimmed pane; the album names live in the tray behind the overlay. Users approve numbers, not intent. List the albums (collapsible past five) with per-album track counts and sizes. *Done:* "What's changing" lists each album (open by default up to five).
5. **Raw engine errors reach the user** (for example a code and a Rust message). Map each error code to a plain sentence plus what to do next ("The card was disconnected. Nothing was lost. Reconnect it and press Start sync to continue."). *Done:* `explainError` maps the engine's error codes to plain sentences and next steps; unknown failures keep the original text.
6. **The plan promised a Delete-key shortcut for removal; it isn't built.** Also missing: the "N pending" badge on the sidebar card chip when you switch cards (section 7). Both are small; the second matters because per-card pending lists are otherwise invisible. *Done:* Delete removes the selected album; the sidebar chip and card switcher show "N pending".

## P1: biggest usability wins
7. **Real libraries are hundreds of albums; selection is one tick at a time.** Add filter chips (All / New / Changed / On Pocket), "Select all shown", shift-click range, and make the whole row toggle selection (today only a 13 px checkbox does; Fitts). Add sort (artist, title, recently added) and group-by-artist.
8. **There is no artwork anywhere.** For a music library the cover is the scanning cue, and it was the heart of the Zune reference. Rows show a placeholder note. Extract and cache thumbnails (embedded or folder cover), lazily. Also shows the *current* cover in the edit drawer, which today shows nothing to compare against.
9. **Progress blocks the whole app.** Over direct USB a sync can take a long time; a modal that hides everything is the wrong container. Dock progress as a bar in the tray (with Cancel and details) and let people keep browsing. Keep the modal only for the review step.
10. **Three buttons, one destination.** Rename, Edit info and Change cover all open the same drawer. Replace with one **Edit…** (cover as a clear thumbnail button inside), keeping Rename only where it means something different (a single track).
11. **Show the device form of text as you type.** The engine folds names to ASCII for the Pocket ("Sigur Rós" becomes "Sigur Ros"). Preview "On the Pocket: Sigur Ros" under each field so nothing surprises the user later. `device_name` already exists in the engine.
12. **Merge the two consecutive dialogs.** First sync over 10 MB shows the slow alert and then the review sheet. Show the alert as a banner inside the review sheet (same words, same "don't ask again"): one decision point instead of two.
13. **Clear all has no undo**, and the toast that reports it vanishes after 3.6 s. Give destructive-feeling actions an Undo in the toast and keep the toast on screen until dismissed when it has an action.
14. **Old logs are now hard to find.** The complaint that started this was a "useless log location"; the new screen shows none. Add "View in Recent jobs" on the result and a small history entry point (last sync, when, result).

## P2: polish
15. **Card and core context is not a control.** The header "Pocket / omega" looks clickable and isn't; switching lives in the sidebar. Make the header the switcher (or add a visible Change).
16. **Connection chip needs an affordance and plain words.** Add a chevron, and say "Connected directly · slow" with a tooltip "Use a card reader for large syncs". Explain "Auto / This is the Pocket / Card reader" in the menu.
17. **Capacity bar has no legend** (blue used, green queued, hatched removing). Add a compact legend or tooltips, and show "other files on the card" separately from music.
18. **Vocabulary:** the review says "files", the tray says "tracks". Use tracks and albums throughout; "Add / update" becomes "New" and "Updated".
19. **Target sizes:** the chip x and undo buttons are 22 px high and the checkbox is about 13 px; WCAG 2.2 asks for 24 px minimum. Enlarge the hit area without enlarging the visual.
20. **Keyboard model:** add Ctrl/Cmd+A, arrow-key row navigation, Enter to add, and a visible focus ring on rows. Drag is optional but everything must remain doable without it (it is, via the button).
21. **Scale:** lists are not virtualised (the old Library screen was). At 2,000 albums this will lag. Reuse the windowing already written for the old view.
22. **Narrow windows:** the panes stack below 1100 px; verify at the app's minimum window size, and keep the tray pinned.
23. **Restore path:** removals are backed up but there is no "restore" action. Even a link to the backup folder from the result would close the loop.

## Suggested usability session (before more building)
Five people who own a Pocket. Tasks, no coaching: (a) put two albums from a folder on the card; (b) work out whether a 3 GB album will fit; (c) remove an album you no longer want, then change your mind; (d) fix a wrong album title and change its cover; (e) sync over the Pocket's USB mode and say what you expected to happen. Watch for: hesitation at Start sync, where they look for the log, whether they trust "Pending changes", and what they do when the card is unplugged.

## Suggested order
1. P0 items 1 to 6 (about a day of work, mostly small).
2. P1 items 7, 8 and 9 (largest perceived quality gain), then 10 to 14.
3. Run the usability session after P0 and before P1 so P1 is guided by observation rather than opinion.


## Round 2 status (after the usability session and the owner's decisions)
Built: **bulk selection** (click anywhere on a row, Shift-click ranges, Ctrl/Cmd+A, "Select all N shown", Escape to clear), **filters** (All / New / Changed / On Pocket, with counts) and **sort** (artist, title, largest, smallest), an explanation on every *Changed* row ("Pocket has 4 tracks"), **windowed lists** (2,000+ albums stay fast; the old 300-track cap on the Tracks tab is gone), **cover artwork** on both panes and the current cover shown next to the new one in the edit drawer, **non-blocking progress** (docked in the tray; the queue is locked while a sync runs, but you can keep browsing), one **Edit...** button, a plain-language wording pass (library list instead of index, "Connected directly", "Pocket limit", "Free space afterwards", "put each album's cover picture inside the copied songs"), sync history, one review sheet with the slow-connection warning and estimate, and the fit check. Still open: the Home/PageUp scroll behaviour reported at 200% zoom, a restore action for removed albums, and wording on the older Tools pages (Compare cores, Backup, Packages, Problems), which keep their original engineer-facing language.
