# Usability session: cross-persona synthesis

For the product owner to analyse **before any P1 work**. Nothing here has been acted on.

## How far to trust this
* The testers were three AI agents playing personas (Maya: casual, mouse; Sam: power collector, shortcuts; Ren: cautious, keyboard-only at 200% zoom) in a browser against the **mock** backend. They had no access to code or docs. Full reports: `session-maya.md`, `session-sam.md`, `session-ren.md`; protocol: `SESSION_PLAN.md`.
* They are good at surfacing confusing wording, dead ends, missing feedback and keyboard problems. They are **not** real users: their feelings, ratings and timings are relative, not absolute, and they cannot tell you how a real Pocket behaves.
* I checked each claim against the code. **Section 3 lists what turned out to be mock artefacts**, so you don't spend time on them.
* Convergence matters most: something all three hit independently is a strong signal; something only one hit is a hypothesis for real users.

## 1. Scoreboard (Single Ease Question, 1 = very hard, 7 = very easy)
| Task | Maya | Sam | Ren | Read |
|---|---|---|---|---|
| T1 Add two albums | 5 | 6 | 4 | Works; Ren's cost is keyboard flow |
| T2 Will 1.4 GB fit? | 5 | 4 | 3 | **App never says it.** All three did the sum themselves |
| T3 Remove, then undo | 6 | 7 | 4 | Best-rated task; Ren's cost is tab order |
| T4 Rename + cover | 5 | 6 | 4 | Works; two clarity problems (see 2.4, 2.5) |
| T5 Sync and watch | 5 | 5 | 5 | Progress praised; warning chain criticised |
| T6 Find sync history | **2** | **2** | **1** | **All three gave up** |
| T7 Card pulled mid-sync | 4 | 6 | 5 | Calm and clear; missing reassurance |
| Overall ease | 5 | 5 | 4 | |
| Trust with a real library | "cautiously yes, small copy first" | "partially, not for 3,000 albums yet" | "3 of 5, keep my own backup" | |

## 2. Findings ranked by convergence
Severity is the testers' word, adjusted where I could verify. **Status** says what I confirmed in the code.

### All three testers (strong signals)
1. **No way to see what a sync did (T6: everyone gave up). Blocker.** "Recent jobs" shows nothing, speaks in "journals" and "reports directory", and offers no "last sync" anywhere else. *Status: real bug, not only a mock effect.* Library syncs are journalled to a default app folder, but "Recent jobs" only reads a folder the user chose themselves, so on a real install those journals are never listed. The page's wording is also jargon (Maya, Sam, Ren).
2. **The app doesn't tell you whether an album will fit (T2). Major.** Numbers exist ("3.0 GB free of 8.0 GB"), but only after queueing the album, which the task said not to do. Sam expected "Fits, 1.6 GB left" when ticking; Ren couldn't see the bar and the album at once at 200% zoom. *Status: real (the design shows capacity after staging, not on selection).*
3. **The "slow" USB warning felt wrong and the sync chain felt long.** Maya counted three confirmations for two albums (Start sync, USB alert, review, confirm). All three saw the sync finish in seconds after being warned it "may take a long time", and none got a time estimate before confirming. *Status: split.* The finish speed is a **mock artefact** (the mock progresses at 100+ MB/s). The **missing time estimate on a first sync and the two dialogs in a row are real** (an estimate only appears from a previous measured sync).
4. **An edit doesn't show its result until after the sync.** After "Add to Pending changes", the row still says the old title with "Edit pending"; the chip says "(title, cover)" not the new name. Maya thought it was broken; Sam and Ren couldn't verify what they typed. Maya also expected a "Save". *Status: real.*
5. **"Rename" and "Edit info" open the same panel.** All three noticed and asked what the difference is. *Status: real (already in the UX review as P1 item 10).*
6. **A failed sync doesn't say what state the card is in.** "Nothing was reported complete" was called odd (Maya) and insufficient (Sam, Ren): is my existing music safe? are half-copied files left? Ren: "biggest fear". *Status: real wording gap.* (The engine does keep existing files and writes the index last, so the reassurance can be true and specific.)

### Two testers
7. **"Changed" badge is unexplained (Sam, Ren).** "Blue Train ... Changed" stays after syncing, with no indication of what differs. *Status: real; the badge compares track counts only, and the mock keeps the source and card counts different on purpose, so part of the persistence is a fixture effect. The lack of any explanation is real.*
8. **Jargon (Maya; Sam and Ren partly).** "Index verified", "Embed each folder's cover art into the copies", "baseline JPEG", "media root", "journal", "core", "16,384" (a track limit shown without a label for what it is). *Status: real.*
9. **The two-dialog warning has a risky default (Ren).** In the USB alert and the progress dialog, focus starts on "Sync anyway" / "Cancel sync". *Status: real: my dialog helper focuses the primary button; for the alert the safe choice (Cancel) should be the default, and Enter must not cancel a running sync by accident.*

### One tester (hypotheses to check with people)
10. **Bulk selection (Sam).** Ctrl/Cmd+A, Shift-click ranges and click-anywhere-on-row don't work; no sort or filter; would be painful with 3,000 albums. *Status: real (UX review P1 item 7).*
11. **Toolbar/undo reachability by keyboard (Ren).** Focus is thrown to the page top after most actions; the pending chips and Undo come *after* "Clear all" and "Start sync" in tab order; Home/PageUp scroll the inner list; each album is its own Tab stop. *Status: focus-to-top is likely the focused control being removed from the page (e.g. after Add or Mark); tab order is real (DOM order).*
12. **The Help panel isn't a proper dialog (Ren).** No focus trap, Escape doesn't close it when focus is behind it. *Status: real; my dialog helper was never applied to it.*
13. **The sidebar disappears at 200% zoom (Ren).** *Status: real and pre-existing:* the app hides the whole sidebar at 720 px wide or less, which is exactly a 1440 px screen at 200% zoom. With the sidebar hidden, **Cards, Playlists and Tools & settings are unreachable**, so history, settings and every tool go with it.
14. **Disabled buttons don't look disabled (Ren, "Start sync still looks fully enabled while disconnected").** *Status: real:* there is no global disabled style for the main buttons, so "Start sync" and "Add to Pocket" look identical when blocked (they are blocked, but nothing shows it).
15. **Small things:** toasts vanish quickly and cover the header (Ren); the banner pushes the page down under the pointer (Maya); the toolbar shifts the list when a selection appears (Sam); the "Direct USB · slow" pill is jargon and looks alarming (Maya); "Add to Pocket" doesn't put anything on the Pocket yet (Ren, Sam; the queue message made it clear); bar colours have no legend (Maya); the file path in the progress dialog is meaningless (Maya).

## 3. Mock artefacts: do **not** act on these
* "Edit 5 tracks" for an album shown with 4 tracks (Maya, Sam, Ren): the mock invents `5 x edits`. Real plans report real counts.
* "Sync finished in seconds despite the slow warning" and "223 MB/s": the mock emits fast progress.
* "Choose reports directory selects my Music folder" (Sam): the simulated picker returns the music folder for every folder choice.
* "Nothing changed for a few seconds after Choose folder" (Maya): the simulated picker resolves instantly; the real picker and scan will behave differently. Worth watching on real hardware, not fixing from this.
* Empty "Recent jobs" list *in the mock* (the mock returns no journals). The real cause is the bug in finding 1, separately.

## 4. What worked (all three, keep it)
* Staging: "nothing happens until Start sync" repeated in plain words; queue chips with an ×; Clear all.
* Removal: marked, struck through, "Undo", and the first-time dialog (marked-only, backed up, where to change it). Ren: "best moment of the session".
* The review sheet ("What's changing", counts, "Free after", "Your music on this computer is never changed", "Every file is verified, then the index is written last").
* Live progress (bytes, speed, time left, "Keep the Pocket connected", "You can disconnect the Pocket now").
* Calm card-pulled handling: plain words, pending changes kept, a banner with "Check again".
* Clear row labels ("On Pocket", "New", "Queued", "Will be added", "Will be removed", "Edit pending").
* Visible focus ring, dialogs that trap focus, readable at 200% (aside from the sidebar).

## 5. Questions for you to decide
1. **History.** Where should a person look after a sync, and in what words? (A "Last sync" line on the Library screen plus a plain-language "Sync history" list? Keep "Recent jobs" for power users?) And should the default journal folder be shown in Recent jobs automatically?
2. **Fit check.** Show "Fits, 1.6 GB left" on selection, or only after staging? (Sam and Ren both want it before committing.)
3. **The dialog chain.** One review sheet that carries the slow-connection warning and a time estimate, or keep two steps? If one, what's the estimate when there's no previous speed (say "unknown" plainly, or use a conservative published figure)?
4. **Edit wording and result.** Replace "Add to Pending changes" for edits with something like "Save (applies when you sync)" and show the new name immediately in the list?
5. **Failure reassurance.** How much detail is right ("Your existing music is safe. Files copied so far were kept; the library index wasn't changed")?
6. **Navigation at large zoom.** Keep the sidebar hidden under 720 px but provide a menu button, or never hide it?
7. **Order of work.** The severity here differs a little from the earlier UX review: **history, the fit check and the sidebar/keyboard defects moved up**; bulk selection stays important but only one persona raised it. Do you want P1 reordered?

## 6. What I would put first (for you to accept or change)
1. Fix history (real bug + plain wording + "last sync" on the Library screen).
2. Fit check on selection; show the edit's result immediately; one Edit button.
3. Keyboard and zoom: sidebar reachable at 200%, sensible focus after actions and dialogs, undo/pending before Start sync in tab order, safe default focus in the USB alert and progress dialog, Help panel as a proper dialog, disabled buttons that look disabled.
4. Failure and warning wording (card state after a failure; estimate or "unknown" in the USB alert; merge the dialogs).
5. Jargon pass on labels and small print.
6. Then the P1 items from the UX review: bulk selection and filters, artwork, non-blocking progress.

## 7. Method notes
Each session took about 5 to 9 minutes and 60 to 110 tool calls. One tester's cleanup command killed its own shell (harmless; the browser was stopped from here). Running the same session with real Pocket owners, using the same tasks, is still the next step for anything that will decide the roadmap.
