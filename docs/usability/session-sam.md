# Usability session: Sam (power collector, 41)

Method: UI only, 1440x900 browser. "Saw" = on screen; "Infer" = my guess as the persona.

## Summary table
| Task | Outcome | Actions (approx.) | SEQ |
|---|---|---|---|
| T1 Add two albums | Completed (staged, then synced during T5) | 7 | 6 |
| T2 Will Bitches Brew fit? | Completed with difficulty (app did not tell me; I did the sum) | 4 | 4 |
| T3 Remove Time Out, then undo | Completed | 4 | 7 |
| T4 Rename Blue Train + new cover | Completed | 5 | 6 |
| T5 Sync, predict, watch | Completed | 5 | 5 |
| T6 Find last sync history | Gave up (nothing showed) | 6 | 2 |
| T7 Card yanked mid-sync | Completed (understood) | 5 | 6 |

## T1 Put two albums on the Pocket
- Expected: a folder list next to the Pocket list. Saw "This computer" / "Choose your music folder" and a "Choose folder..." button. Easy.
- Ctrl+A in the list only highlighted the page text, not albums. Shift-click on checkboxes ticked just one row, no range. Clicking a row did nothing, only the small checkbox works. (major for a 3,000-album library.) Delete key did work later (T3).
- Ticked A Love Supreme and Head Hunters, pressed "Add 2 to Pocket". Toast: "2 albums added to Pending changes." Right pane showed green "Will be added" rows, bar showed "+ 648 MB queued". I like this a lot.
- Confusion: after "Add" nothing is on the Pocket yet. "Queued" and "Pending changes" made this clear enough, but the task wording "put onto" made me hunt for a finish button; "Start sync" is at the bottom. (cosmetic)
- Computer list shows "Blue Train ... Changed" with different size/track count (5 tracks 290 MB vs 4 tracks 230 MB on Pocket) and no explanation of what changed. Still "Changed" after I synced. (major: I can't see what differs; power users want a diff.)
- Storage line reads "21 of 16,384 tracks". I don't know what 16,384 is (a track limit?). (minor)
- With 3,000 albums I expect: the list is a scroll box, search exists (good), but no sort, no filter by artist/genre, no Select all, no range-select. Would be painful.

## T2 Will Bitches Brew (1.4 GB) fit?
- Searched "Bitches" (search filters instantly). Ticked it: only "1 selected". No fit check shown. I did the sum myself: header said "3.0 GB free of 8.0 GB" (that already includes queued items), so 3.0 - 1.4 = about 1.6 GB spare, fits. Numbers are rounded to 0.1 GB so my margin has some error.
- Expected: a line such as "Fits, 1.6 GB left" when selected. Instead I would have to add it to Pending to see the bar move, which the task told me not to do. (major)
- Nice: the header bar already subtracts what's queued, so it is honest.

## T3 Remove Time Out, then change my mind
- Ticked Time Out on the Pocket side; a bar appeared with "Rename / Edit info / Change cover / Remove". Pressed Delete first (habit): a dialog opened: "Removals are only marked now. Nothing is deleted until you press Start sync, and you can undo until then." and mentioned a default backup. Excellent, exactly the reassurance I want.
- "Mark for removal": row got strikethrough plus "Will be removed", bar shows "- 344 MB removing". Chip has "Undo". One click, back to normal.
- Small point: the toolbar shifts the list down when something is selected (jumpy). (cosmetic)

## T4 Rename Blue Train + new cover
- Selected Blue Train, "Rename" opened an "Edit album" panel with title, artist, album artist, year, cover in one place. "Edit info" is probably the same form, so the two buttons seem redundant (minor). Appended " (Remaster)", chose image, "Add to Pending changes".
- Good text: "Changes apply to the copy on the Pocket only. Your files on this computer stay as they are."
- Problems: after adding, the row still says "Blue Train" with a tag "Edit pending"; the chip says "Blue Train (title, cover)". Neither shows the new name, so I can't verify what I typed before syncing. (minor to major)
- Does the "Changed" flag on my computer's Blue Train mean my edit conflicts with something? Unclear.

## T5 Sync, expectation, watch
- Pressed Start sync. First dialog: "You're connected to the Pocket directly ... slow ... meant for transfers under 10 MB; a card reader is much faster." I had 648 MB, so I expected a long wait, and nothing told me how long. Buttons "Cancel" / "Sync anyway" (fine), plus "Don't ask again".
- Second dialog "Ready to sync to Pocket?" listed Add 8 tracks, Edit 5 tracks (Pocket copies only), Free after 3.0 GB, and an expandable "What's changing (3)". Best screen in the app for me. But: it says "Edit 5 tracks" while the Pocket list showed Blue Train with 4 tracks; the new title isn't listed; the ticked option "Embed each folder's cover art into the copies" is unexplained (what does it do to my files?).
- Two confirmation dialogs in a row feels like a lot (minor).
- Progress: "Copying", "324 MB of 648 MB", "223.0 MB/s", "Time left: about 2 sec", current filename, "Keep the Pocket connected until this finishes.", "Cancel sync". It finished in seconds, far faster than the "slow" warning implied, so the warning felt inaccurate. Final: "Sync complete. 8 tracks copied, 5 tracks edited. Index verified. You can disconnect the Pocket now." Very clear. Blue Train (Remaster) then showed on the Pocket.
- Expected before confirm: an estimated time. Never given. (major for big syncs)

## T6 What happened in my last sync; where to look if it went wrong
- Looked under "Tools & settings", picked "Recent jobs". Text said "Operations completed during this session, plus the durable history in your reports directory." but the list was empty. "Refresh" did nothing visible. I then pressed "Choose" for the reports directory: it set the path to "/Users/me/Music" (my music folder) and said "0 jobs found. Nothing was changed." and "No history yet". I had just done a sync, so this looks broken. I gave up. I also checked "Problems", which is about library health (duplicates, missing tags), not sync failures.
- What I would want: a list with date/time, albums added/removed/edited, result. The only record I got was the transient "Sync complete" dialog. (blocker for trust: no audit trail)
- Worry: the Reports directory picker landing on my music library folder is risky and unexplained (major).
- Where would I look if it went wrong? I don't know. "Recent jobs" probably, but it is empty; jargon like "journal", "cores", "reports directory" means nothing to me.

## T7 Card yanked during sync
- Added Mingus Ah Um, Start sync, Sync anyway, Confirm and start. Dialog "Sync didn't finish": "The card couldn't be reached. It may have been disconnected. Nothing was reported complete; reconnect it and try again." Then a red banner: "This card is disconnected. Your pending changes are kept; reconnect it to continue." plus "Check again" and "Card disconnected" beside Start sync.
- What I think happened: card got pulled, copy stopped, nothing marked as done, my queue is still there. Next: plug it back, press "Check again", then Start sync again.
- Good: plain language, no panic, queue preserved. Gaps: "Nothing was reported complete" doesn't say whether partial files were left on the Pocket or cleaned up, or whether the earlier contents are safe. A power user wants to know. No technical detail or log link. The "Direct USB · slow" tag stayed and the lists look normal, so the screen still looks half-connected. (minor to major)

## Keyboard summary
- Works: typing in search, Ctrl+A / Delete inside the search box, Delete on a selected Pocket album (opens the remove dialog).
- Doesn't work: Ctrl+A to select all albums, Shift-click range select, clicking anywhere on a row to select it.

## Overall
- Overall ease: 5/7.
- Worked well: staged changes with a clear "nothing happens until Start sync" message; Undo; the "Ready to sync" summary; live progress with speed and time; calm disconnect handling; the storage bar that includes queued items.
- Three things to change first:
  1. A findable, working sync history (date/time, what changed, result) with a safe default location, not my music folder.
  2. Bulk-selection and scaling tools: Select all, Shift-click ranges, sort, filters, a fit check ("fits, 1.6 GB left") before adding; show new names in pending items.
  3. Explain "Changed" (show a diff), give a time estimate before sync, reconcile the "slow" USB warning with actual speed, and say what state the card is in after a failed sync.
- Trust with my real library: partially. The safety design (nothing written until confirm, backup before delete, originals untouched, verification) is exactly right and I'd try it with a copy. But without a history log and without scalable selection I would not put 3,000 albums through it yet.
