# Usability session plan: Library workbench

**What this is.** A moderated-style session run by three AI testers, each playing a defined persona, driving the real UI in a browser against the mock backend (`npm run dev`), with no access to the source code or docs.

**What it is not.** Real users. These are simulated personas: good for finding confusing labels, dead ends, missing feedback and unsafe-feeling moments; not evidence of how real people feel, how long things really take, or whether the Pocket behaves as the mock does. Treat findings that all three testers hit as strong signals, single-persona findings as hypotheses to check with real people, and the ratings as relative, not absolute.

## Setup given to every tester
* A Pocket is connected ("Pocket", core "omega") and a music folder on the computer is available (`/Users/me/Music`, chosen through the app's folder picker).
* The Pocket is connected **directly** (its USB mode) unless a task says otherwise.
* Browser window 1440 x 900 unless the persona says otherwise. Fresh browser profile (no saved state).
* Testers may only use the app's UI (screenshots, page text, keyboard and mouse). They may **not** read the repository, docs or source, and get no hints about how the app is built.

## Tasks (same for all personas, in this order, no coaching)
| # | Task (as written to the tester) | What we're looking for |
|---|---|---|
| T1 | "Put two albums from your music folder onto the Pocket." (Any two albums you'd like to listen to.) | Can they find the folder step, choose albums, understand staging, finish a sync? Do they trust "Pending changes"? |
| T2 | "You want to add *Bitches Brew* (about 1.4 GB). Work out, without doing it, whether it will fit on the Pocket, and say by how much." | Is capacity readable? Do they find the bar/numbers? Do they understand queued vs free? |
| T3 | "You decide you don't want *Time Out* on the Pocket any more. Take it off. Then change your mind before anything is deleted." | Discoverability of removal, safety feeling, undo. |
| T4 | "The title of *Blue Train* is wrong on the Pocket. Rename it to *Blue Train (Remaster)* and give it a new cover image." | Edit path, drawer clarity, cover step, understanding that only the Pocket copy changes. |
| T5 | "Sync a change (add any album), knowing you're plugged straight into the Pocket. Before you confirm, tell us what you expect will happen and how long it will take." | Slow-connection alert clarity, expectation setting, progress reading, "keep connected" message. |
| T6 | "Find out what happened in your last sync: what changed and when. If something had gone wrong, where would you look?" | Findability of history/logs after the redesign. |
| T7 | "Halfway through a sync your little sibling yanks the card out. (We'll simulate it.) Explain what you think happened and what you'd do next." | Error and recovery wording. (Testers are told what the moderator did, then observe the screen.) |

Between tasks the tester reports, in their own words, what they *expected* to see next.

## Protocol
* Think aloud: before each action, one line of what they're trying and why.
* Budget: give up on a task after 25 actions and say why.
* Report each task: outcome (completed / completed with difficulty / gave up), number of actions, confusing moments with exact on-screen words, expectation vs what happened, a Single Ease Question score (1 = very hard, 7 = very easy), and severity (blocker / major / minor / cosmetic) for each problem.
* At the end: overall ease (1 to 7), what they liked, what they'd change first, and whether they'd trust the app with their real library.

## Personas
1. **Maya, casual listener (34, Mac).** Bought a Pocket for retro games and wants her ~200 MP3 albums on it. Only ever synced with Spotify and the Music app. Doesn't know what "index", "FLAC", "core" or "ASCII" mean and doesn't want to. Impatient with jargon; will click the most obvious thing; skims warnings.
2. **Sam, power collector (41, Windows and Mac).** 3,000+ FLAC albums, meticulous tags, long-time MediaMonkey and foobar2000 user. Wants control, bulk actions, keyboard shortcuts, and to see exactly what will change. Distrusts tools that hide details. Will notice missing sort/filter.
3. **Ren, cautious keyboard-only user.** Lost a music library to a corrupted SD card once, reads every warning, and works with the **keyboard only** (no mouse) at **200% zoom** (720 x 450 CSS viewport). Needs to know what is safe, reversible and backed up before acting; relies on focus order and visible focus.

## Outputs
One report per persona in this folder (`session-maya.md`, `session-sam.md`, `session-ren.md`), then a cross-persona synthesis for the product owner to analyse before any further build work.
