# Real-card pass: the cores section and the developer tools (CARDWRITE)

Everything below was built and tested against the dev mock and synthetic cards. This is the first run on a real Pocket card. **CARDWRITE is the only test card**; the owner's real card (`Pock`) is only ever read. Each step that writes says so; none starts without a yes. Do it with the test build `releases/dev-builds/0.5.0-alpha.10` (or newer), and keep a backup folder outside the card (every Omega write makes one itself).

Result column: fill in as you go (ok / what happened). Anything that fails goes into `docs/AUDIT_TRAIL.md`-style notes in `docs/STATUS_HANDOFF.md`.

## A. Reading only (no card write)

| # | Do | Expect | Result |
|---|---|---|---|
| A1 | Open CARDWRITE; open Cores | Every core listed under its category > manufacturer > platform; Tau cores under one TAU row; no page error | |
| A2 | Open a core, then Files, Saves, Screenshots, Settings | Each tab fills; missing BIOS listed with its exact name and folder | |
| A3 | Developer tools on (Settings); Check > "Check every core on the card" | One line per core; counts plausible; `agg23.*` cores show no Errors (rules were tuned on `Pock`) | |
| A4 | Check > an installed core with a problem (pick `alfatreze.CARDWRITE02` if it has one) | Findings with the fix and the rule | |
| A5 | Results (after taking a Check screenshot on the Pocket) | The report decodes; the previous run of the same profile is compared | |

## B. Disable and enable (writes to the card: moves a core folder, clears the 5 menu caches)

| # | Do | Expect | Result |
|---|---|---|---|
| B1 | Settings > Cores the Pocket cannot use = "Ask me"; reopen the card | The dialog lists only cores that really lack a file or are broken, each with the fix; nothing is ticked that has no reason | |
| B2 | Hide one core from the dialog (a throwaway, not a Tau core) | It moves to `Disabled Cores/<id>`; Omega still lists it, tagged "off" under its platform; its Files tab works | |
| B3 | Boot the Pocket | The core is absent from the menu, no entry or error in Tools > Developer > Builds | |
| B4 | Enable it again from Omega (platform row **Enable**) | It is back in `Cores/`; the Pocket menu lists it and it starts; its saves are untouched | |
| B5 | Switch "Visible on the Pocket" off and on from the core's page | Stays on the page; banner and switch agree with the card | |

## C. Card role (writes one hidden file at the card root)

| # | Do | Expect | Result |
|---|---|---|---|
| C1 | Card > Overview > "This is a test card" | `.omega-role` appears; Real card prompts for the card name on installs, Test does not | |
| C2 | Make it Real again | The file is removed, and only that file | |

## D. Builds, lint gate, carry-media (writes an installed core, its backup, and copied media)

| # | Do | Expect | Result |
|---|---|---|---|
| D1 | Builds > choose a folder with two test-build zips | Grouped into a series, newest first; the card comparison is right | |
| D2 | Install the older one (reviewed plan; the type-the-name step on a Real card) | Plan lists files; install verifies; run note created | |
| D3 | Install the newer one | Carry-media panel appears; "Review the copy" shows new / already there / different; copy is verified; a different file is not overwritten | |
| D4 | Install a package the Check page reports an Error for | "Confirm and install" stays blocked until "Install anyway"; the override is recorded in the run note | |
| D5 | Remove older builds | Lists them; backs each up; keeps music and settings | |
| D6 | Boot the Pocket | The installed builds appear in the menu and start | |

## E. Builder (writes only a zip and a build record to a folder on the Mac)

| # | Do | Expect | Result |
|---|---|---|---|
| E1 | Builder with a real raw `.rbf` and an installed core as the template | Plan passes; the bitstream is converted once | |
| E2 | Compare the built `.rbf_r` with a known good one for the same input (a byte-by-byte bit-reversal check) | Identical | |
| E3 | Install the built zip through Builds on CARDWRITE | Boots on the Pocket | |

## Stop rules

* Any step that changes something unexpected on the card: stop, eject nothing, record what was seen, restore from the backup folder Omega printed.
* If B3 shows an error or a listing for a disabled core, the move method is wrong: fall back to renaming `core.json` (method C in `CORE_DISABLE_SPEC.md`) and redo B.
* Nothing in this pass touches `Pock`.
