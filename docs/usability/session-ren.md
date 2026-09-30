# Usability session: Ren (keyboard only, 200% zoom, cautious)

Setup: 720x450 viewport, Pocket "Pocket" connected by direct USB, keyboard only. Key-press counts include Tab and Shift+Tab steps and are approximate.

## Summary table

| Task | Outcome | ~Key presses | SEQ (1-7) |
|---|---|---|---|
| T1 Put two albums on the Pocket | Completed with difficulty (queued at once; actually copied only in T5's sync) | ~35 | 4 |
| T2 Will Bitches Brew fit, by how much | Completed with difficulty (no on-screen answer; I worked it out myself) | ~30 | 3 |
| T3 Remove Time Out, then undo | Completed with difficulty | ~50 | 4 |
| T4 Rename Blue Train | Completed with difficulty | ~40 | 4 |
| T5 Sync a change | Completed | ~55 | 5 |
| T6 What happened in last sync / where to look | Gave up (no history found) | ~75 | 1 |
| T7 Sync interrupted by card removal | Completed (understood the outcome) | ~35 | 5 |

Overall ease: 4 of 7.

## T1. Two albums onto the Pocket
- Start screen: "Start with a card", "Open folder" and a "Known cards" tile "Pocket / Available now". Tab went Open folder, Pocket card, refresh, then two "player cores" (omega, player2). I picked "omega" (7180 tracks) as the only one with music. Not explained why two exist. Minor.
- Opening omega gave a long page I had to scroll. "Choose your music folder / Choose folder…" was easy to find (Tab x3 from top).
- After the folder chooser closed, focus went back to a generic "LIBRARY" region at the top of the page, not to the new list. This happens after nearly every action (see cross-cutting).
- Album list: each row is one Tab stop ("Select Moanin"), focus ring clearly visible (white outline). Space ticks. Good. Rows already on the Pocket are greyed with "On Pocket"; queued ones say "Queued". Very clear.
- "Add 2 to Pocket →" comes after all 11 rows, so I overshot it once (Tab landed on "Albums" beyond it). Minor.
- Result: toast "2 albums added to Pending changes." and a Pending changes area: "2 albums to add (11 tracks, 618 MB)" with chips. Expected: albums on the Pocket. Got: a queue plus "Changes are only made when you press Start sync." I liked the honesty, but a button called "Add to Pocket" that does not put anything on the Pocket is slightly misleading. Minor. The toast vanished quickly; easy to miss at low vision. Minor.

## T2. Will Bitches Brew (1.4 GB) fit?
- Expected: ticking the album (without adding) would tell me. Saw only "1 selected" and "Add 1 to Pocket →". No fit check.
- The only numbers are in the bar at the very top of the page: "4.3 GB on Pocket, + 618 MB queued, 3.1 GB free of 8.0 GB". I did the sum myself: 3.1 GB free minus 1.4 GB = about 1.7 GB spare (before any overhead). I could not see the bar and the album at once at 200% zoom. Major for a cautious user.
- Scrolling: Home/PageUp acted on the inner album list first, not the page (4 PageUps to reach the top). Major at 200% zoom.
- I unticked Bitches Brew afterwards so nothing was left selected.

## T3. Remove Time Out, then change my mind
- Ticking Time Out on the Pocket list revealed a toolbar "1 selected, Rename, Edit info, Change cover, Remove". Discoverable. But the toolbar sits BEFORE the list in tab order, so I had to Shift+Tab 3 times upward to reach "Remove". Minor.
- "Remove" opened "Remove from the Pocket?": "Removals are only marked now. Nothing is deleted until you press Start sync, and you can undo until then. By default the files are copied to a backup folder on this computer before they're deleted." plus where to change that. Exactly what Ren needs (reversible and backed up). Best moment of the session. Focus landed on "Mark for removal" with a clear ring; other buttons "Open settings", "Cancel".
- Afterwards: toast "1 album marked for removal. Nothing is deleted until you sync.", row struck through with "Will be removed", chip "Time Out 344 MB Undo".
- Undo: focus was thrown to the top of the page. Reaching "Undo removing Time Out" took about 15 Tabs, and it comes AFTER "Clear all" and "Start sync" in the tab order. I overshot once; a slip could land on Start sync. Major. Undo worked and the album returned to normal.

## T4. Rename Blue Train to "Blue Train (Remaster)"
- Ticked Blue Train on the Pocket list, went up to "Rename" (5 Shift+Tabs). It opens a side panel titled "Edit album" (I expected a small rename box). "Rename" and "Edit info" seem to open the same panel; I could not tell the difference. Minor.
- Title field focused with visible ring. I pressed End and typed " (Remaster)". Enter in the field did nothing, with no feedback. I had to Tab through Artist, Album artist, Year, "Choose image…", "Cancel" to "Add to Pending changes", which was off screen until focused. The label "Add to Pending changes" for an edit is odd. Minor to major.
- Good: focus stays inside the panel; text "Changes apply to the copy on the Pocket only. Your files on this computer stay as they are, and your edits are re-applied on future syncs." is reassuring.
- Cover: reachable by keyboard from the toolbar ("Change cover") and in the panel ("Choose image…"; note "Covers must be baseline JPEG under 2 MiB; anything else is flagged before the sync starts"). Not used.
- After adding: toast "Edit added to Pending changes.", tag "Edit pending", chip "Blue Train (title)". The Pocket showed the new name only after T5's sync. On the computer side "Blue Train" still shows "Changed" after the sync, which I did not understand.

## T5. Sync a change
- I had 3 queued changes (2 adds, 1 rename). "Start sync" is at the bottom of the page; about 30 Tabs from the top. Shift+Tab from the top wraps to the end and was quicker, but not obvious. Major.
- Dialog 1: "You're connected to the Pocket directly. Transfers over this connection are slow and this may take a long time. The Pocket's USB mode is meant for transfers under 10 MB; a card reader is much faster for larger syncs. Are you sure?" with a "Don't ask again" checkbox. I read it in full. My expectation: very slow, maybe many minutes for 618 MB; no estimate was given. Default focus is on "Sync anyway", not "Cancel". Major for Ren.
- Dialog 2: "Ready to sync to Pocket?" Add 11 tracks, 618 MB; Edit 5 tracks (Pocket copies only); Free after 3.1 GB of 8.0 GB; "What's changing (3)" list; checked box "Embed each folder's cover art into the copies"; "Your music on this computer is never changed. Every file is verified, then the index is written last." Excellent. Missing: how long, and what happens if interrupted.
- Progress: "Syncing to Pocket", Step Copying, "129 MB of 618 MB", speed "211.1 MB/s", "Time left about 3 sec", current file path, "Keep the Pocket connected until this finishes." Focus was on "Cancel sync" (an accidental Enter cancels). It finished in seconds, contradicting the "slow" warning.
- Done: "Sync complete. 11 tracks copied, 5 tracks edited. Index verified. You can disconnect the Pocket now." Focus was left on the page behind; no ring on "Done" until I pressed Tab. Major (focus).
- Afterwards the bar said "4.9 GB on Pocket, 32 tracks, 3.1 GB free", which matched.

## T6. What happened in my last sync and when? Where to look if it went wrong?
- Gave up after about 75 presses. I found NO sync history, log, "last synced" time or report anywhere. The "Sync complete" dialog was the only record and it goes away on "Done".
- The "?" Help panel ("Using Tau Omega") says "Rarely used tools and settings are under Tools & settings in the sidebar" and mentions "Compare cores, Backup, Packages". I never saw a sidebar at 200% zoom and nothing like it is in the Tab order. I infer it may be hidden at this width; I cannot know.
- The Help panel is a keyboard problem in itself: focus is not trapped (Tab wandered into the page behind it; 22 Tabs to reach "Close help"), Escape did not close it while focus was behind it, and it only scrolled with PageDown once focus was on its Close button. Major.
- "If something went wrong, where would I look?" I do not know. Blocker for Ren's core need (trust).

## T7. Sync interrupted (card pulled)
- Queued "A Love Supreme" (268 MB); same two dialogs as T5. Final dialog: "Add 4 tracks 268 MB, Free after 2.8 GB of 8.0 GB".
- After the failure and disconnect: dialog "Sync didn't finish": "The card couldn't be reached. It may have been disconnected. Nothing was reported complete; reconnect it and try again." Button "Close". Focus was NOT on Close (it was on the page region); I had to Tab. Minor to major.
- What I think happened: the copy stopped because the card went away. What I would do: reconnect and try again. What I could not learn: whether half-copied files are left on the Pocket, whether the existing music on the Pocket is safe (Ren's biggest fear), what "Nothing was reported complete" means for what was already copied. No detail on the cause. Major for Ren.
- After Close: red banner "This card is disconnected. Your pending changes are kept; reconnect it to continue." with "Check again"; "Card disconnected" next to Start sync; queued album still in Pending changes. Calm and reassuring. But "Start sync" still looks fully enabled while disconnected, and the top bar still counted the queued album ("36 of 16,384 tracks", "2.8 GB free"). Minor.

## Cross-cutting keyboard and accessibility findings
1. Focus is thrown to the top "LIBRARY" region after almost every action (folder chosen, add to pending, mark for removal, dialogs closing, sync complete/failed). Tab/Shift+Tab then start from an unpredictable place. Major; this cost the most presses.
2. The page is long; Home/End/PageUp/PageDown act on whichever inner list has focus. Major at 200% zoom.
3. "Clear all" and "Start sync" come BEFORE the pending chips and Undo buttons in tab order. Major (easy to hit the wrong thing).
4. Every album is its own Tab stop (11 on the computer side, 6 on the Pocket side); anything after a list is ~10 Tabs away, with no skip. Minor to major.
5. Info badges ("Direct USB · slow") are Tab stops with no action. Minor.
6. Focus ring is clearly visible everywhere (white outline). Exceptions: on the "Sync complete" and "Sync didn't finish" dialogs focus was hidden behind the dialog. Major at those points.
7. Dialogs (remove, direct-USB, review, progress, edit panel) trap focus properly. The Help panel does not.
8. Text is readable at 200%, no horizontal scroll, no cut-off content. The floating "?" button overlaps content at the right edge. Cosmetic.

## What worked well
- "Nothing happens until Start sync", repeated in words in every dialog.
- The removal dialog: marked-only, undoable, backed up to a folder on the computer, and says where to change it.
- The review dialog before syncing (counts, free space after, list of changes, "Your music on this computer is never changed", "Every file is verified, then the index is written last").
- Clear row status labels: "On Pocket", "New", "Queued", "Will be added", "Will be removed", "Edit pending".
- "Sync complete ... Index verified. You can disconnect the Pocket now."
- Calm handling of the pulled card, with pending changes kept.

## Three things to change first
1. Return focus somewhere sensible after every action and dialog (the control just used, or the dialog's main button), never to the page top. Put Undo/remove chips before Start sync, and add a way to jump straight to Pending changes / Start sync.
2. Add a visible sync history ("Last sync: date, time, what changed, result") reachable by keyboard, and make sure the sidebar/"Tools & settings" is reachable at 200% zoom. In the failure dialog say exactly what state the Pocket is in (existing music safe? partial files?).
3. Show whether a ticked album will fit, and by how much, next to the list; give a time estimate in the direct-USB warning; make the safe choice (Cancel) the default focus in that warning and the progress dialog.

## Would Ren trust it with the real library?
Partly, cautiously. The wording on removal, backup, verification and "your music on this computer is never changed" is the best I have seen and speaks directly to my fear after the corrupted SD card; nothing was deleted without a review step, and undo worked. But I could not find any record of what a sync did, the failure message did not say whether the Pocket's existing music was safe, and the "slow" warning contradicted what actually happened. I would try a small copy of the library first and keep my own backup of the card. Trust: 3 of 5.
