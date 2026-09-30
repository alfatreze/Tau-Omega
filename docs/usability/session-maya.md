# Usability session: Maya (casual listener, 34, Mac)

Method: UI only (screenshots, mouse, keyboard). No source or DOM inspection. "Saw" = on screen; "Infer" = my guess as Maya.

## T1. Put two albums on the Pocket. Completed, about 9 actions. SEQ 5/7
- Opened Library and pressed the big green "Choose folder...". For a few seconds nothing changed and I clicked again. (minor; no "loading" feedback, and I can't tell which click worked.)
- Once my Music folder loaded, the two-column layout was clear. I ticked A Love Supreme and Mingus Ah Um, pressed "Add to Pocket ->". A toast said "2 albums added to Pending changes." and a green bar showed "+ 670 MB queued". This was easy.
- I then had to press "Start sync", then a "You're connected to the Pocket directly" dialog ("Transfers over this connection are slow ... meant for transfers under 10 MB; a card reader is much faster ... Are you sure?"), then "Sync anyway", then "Ready to sync to Pocket?", then "Confirm and start". Three confirmations for adding two albums.
- Expected: after ticking, the music copies. Got: two extra screens. The USB warning scared me ("slow", "may take a long time") yet the sync finished in seconds, so the warning felt wrong or exaggerated. (major: I nearly stopped, and the "Direct USB · slow" orange badge is jargon and alarming.)
- "Sync complete: 13 tracks copied. Index verified." Word "Index" means nothing to me. (minor)
- Small print I skipped: "Embed each folder's cover art into the copies" (tick box, on by default). No idea what it does. (minor)

## T2. Will Bitches Brew (1.4 GB) fit? Completed, about 5 actions. SEQ 5/7
- The top bar says "3.0 GB free of 8.0 GB", so I could work out it fits with roughly 1.6 GB left, but the app never says "this will fit". I searched for the album, ticked it and added it to pending to see the bar. It changed to "1.6 GB free of 8.0 GB". Good, but that only works by queueing it, which feels like "doing" it. I then used "Clear all". (minor)
- Bar colours (blue, green, hatched) have no legend. (cosmetic)

## T3. Remove Time Out, then change my mind. Completed, about 5 actions. SEQ 6/7
- Ticked Time Out on the Pocket side. A row of buttons appeared ("Rename / Edit info / Change cover / Remove"). Pressed Remove. The dialog said "Removals are only marked now. Nothing is deleted until you press Start sync, and you can undo until then." Reassuring. Pressed "Mark for removal". Time Out was struck through with "Will be removed", and Pending changes had an "Undo" button. Pressed Undo. Back to normal.
- Confusing: "Open settings / Cancel / Mark for removal" plus a note about "back up then remove" is too much text. I skimmed it. (minor)
- The toolbar only appears after ticking, so I did not know beforehand that I could rename or remove. I found it by luck. (minor)

## T4. Rename Blue Train and change its cover. Completed with mild difficulty, about 8 actions. SEQ 5/7
- Ticked Blue Train, pressed Rename. It opened an "Edit album" panel with Title, Artist, "Album artist (optional)", Year, Cover. I typed the new title and used "Choose image...". It showed a blue square and "cover.jpg".
- Cover note "Covers must be baseline JPEG under 2 MiB; anything else is flagged before the sync starts." is jargon. (minor)
- The button says "Add to Pending changes", not "Save". Expected the rename to happen. Afterwards the album still said "Blue Train" with a small "Edit pending" tag. I only saw the new name after a later sync. Nothing on screen told me I needed to sync. (major: I would have thought it was broken or done.)
- "Rename" and "Edit info" open what looks like the same panel. Not sure of the difference. (minor)

## T5. Sync a change (Moanin). Completed, about 8 actions. SEQ 5/7
- Before confirming I would have expected: "adds Moanin, changes Blue Train, a few minutes". The review screen said "Add 6 tracks - 322 MB / Edit 5 tracks (Pocket copies only)". The USB dialog only this time said "about 2 sec at your last speed". The time estimate was hidden in small print, not on the final confirm screen. (minor)
- "5 tracks (Pocket copies only)" for Blue Train, while the Pocket list says Blue Train has 4 tracks. Made me doubt it. (minor)
- Progress dialog was good: bar, "Copied 174 MB of 322 MB", speed, "Time left about 2 sec", and "Keep the Pocket connected until this finishes." The file name "Assets/tau/common/file-13.mp3" was meaningless to me but harmless. Result "6 tracks copied, 5 tracks edited. Index verified." and "You can disconnect the Pocket now." Nice.
- Speed 100+ MB/s contradicts the "slow" warnings. (minor)

## T6. What happened in my last sync, and where to look on a problem? Gave up (about 6 actions). SEQ 2/7
- Opened "Tools & Settings". Guessed "Recent jobs". Page says "Operations completed during this session, plus the durable history in your reports directory." but shows no list of jobs at all, only a "Reports directory" path field, "Choose", "Refresh", and "Load a journal from elsewhere". I pressed Refresh; still nothing. Words like "journal" and "reports directory" mean nothing to me. I did not know whether my two earlier syncs were missing or hidden. (blocker for this task)
- "Problems" page talks about "Duplicate content, missing tags, path issues" and "Choose a media root". I don't know what these mean and it seems to be about my computer's music, not sync problems. (major)
- The "?" help panel explains buttons but says nothing about history. Never found what changed or when. Infer: the history may exist but wasn't shown. Where to look if something went wrong: I don't know.

## T7. Card yanked mid-sync. Completed, about 5 actions. SEQ 4/7
- Pressed "Start sync", "Confirm and start" (this time the USB warning did not stop me, unclear why).
- Saw a dialog "Sync didn't finish: The card couldn't be reached. It may have been disconnected. Nothing was reported complete; reconnect it and try again." plus "Close". Then a red banner: "This card is disconnected. Your pending changes are kept; reconnect it to continue." with "Check again", and "Card disconnected" next to Start sync.
- What I think happened: my sibling pulled the card, the copy stopped, the album did not go on. What I'd do: plug it back in, press "Check again", then Start sync again. The messages did tell me that, and it was calm and clear.
- Worries: "Nothing was reported complete" is odd wording. It does not say whether half-copied files are on the card, or whether my other music is safe. Reassurance ("your existing music is safe") is missing. No error code or the "No such file or directory" text appeared, which is fine for me. (minor to major)
- The banner shifts the whole page down, so the button under my mouse moved. (cosmetic)

## Overall
Overall ease: 5/7.

Worked well: two-panel layout; "nothing happens until Start sync"; queue chips with an x; remove marked then Undo; live progress with "Time left"; the disconnect message and kept pending changes; the free-space bar.

Three things I'd change first:
1. Make history obvious and readable: a "Sync history" list (date, what was added, removed, changed, success or failed) with plain words. The current "Recent jobs" page shows nothing and speaks in "journals" and "reports directory".
2. Cut the confirm chain and the scary USB warning. One review screen with the time estimate, free-space result ("fits, 1.6 GB left") and what is changing. Drop the "slow" warning when it isn't slow.
3. Remove jargon (Index, media root, tags, baseline JPEG, core, journal), and make "Add to Pending changes" clear about needing a sync (say "Save edit; applies when you sync"). Show the new name straight away.

Trust with my real library: cautiously yes. The app keeps repeating that my files on the computer are never changed and that nothing is deleted without a confirm, and remove is undoable. But I could not see any record of what it did, and the failed-sync message didn't tell me what state the card was left in. I would try it with a small copy first.
