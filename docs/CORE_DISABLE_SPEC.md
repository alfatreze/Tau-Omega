# Disabling cores so the Pocket does not list them (investigation and spec, 2026-10-15)

Owner request: a way to disable cores so the Pocket does not see them, for cores that have no files they can read or play, or lack the mandatory files they need; a setting, plus a dialog asking the user when such a core is detected (with a "do not remind me" tick); and a manual way to enable a core again for the ones every check misses (a core that needs no files, say).

**Status:** the detection (`core_health`, `tau core-health`) and the move itself (`core_state`, `tau core-disable` / `core-enable` / `cores-disabled`) are built and tested on synthetic cards; detection was run read-only on the real card. **Built 2026-10-15 (UI):** the setting (Settings › Library: Ask me / Hide automatically / Never suggest), the dialog on card open, the "Visible on the Pocket" switch in a core's header, the Disabled section on the Cores overview (Enable, Enable all; enabling also adds the core to the card's keep-enabled list), host commands `core_suggestions`, `disabled_cores`, `plan_core_state`, `execute_core_state` (card write lock), `core_keep`; Playwright `disable-test.cjs` (27 checks). **Not built:** the tree chip for disabled cores (they are listed in the Disabled section instead), the "enable anyway" warning for a core that is still flagged (the saved reasons are shown beside it), a real-card run of the UI. **Hardware check done 2026-10-15 on CARDWRITE:** both the move to `Disabled Cores/` (method A, ZXSpectrum) and the `core.json.off` rename (method C, Arduboy) hide the core from the openFPGA menu with no entry or error in Tools > Developer > Builds; the control core stayed listed. **Decision: A** (clean `Cores/`, one visible place, `omega-disabled.json` records when and why; C leaves a folder without `core.json` that other tools read as broken). Restore check (both return and start, saves unchanged) is still owed.

## 1. What decides whether the Pocket lists a core

Facts from Analogue's developer documentation (in the `analogue-pocket-dev` skill): the Pocket lists the folders of `/Cores/<Author.Core>/` (the folder name must equal `author.shortname` in `core.json`); a core whose JSON is invalid "also disappears from the openFPGA menu" (Tools > Developer > Builds shows why); the menu is built from catalog caches in `/System/` that Omega already clears after installs. Nothing in the documentation describes a hide or disable flag, and Pocket Sync has no such feature (a search found none; Pupdate can uninstall cores, which is deletion, not hiding). So the only lever is **where the core's files are**.

## 2. Mechanisms compared

| Option | What it does | Risk |
|---|---|---|
| **A. Move `Cores/<id>` to `Disabled Cores/<id>`** (card root, outside `Cores/`) | One rename on the same volume: instant, atomic, nothing copied or rewritten; the Pocket scans `Cores/` only | Small: that the Pocket ignores an unknown root folder (very likely, unverified) |
| B. Move into `Cores/.disabled/<id>` | Same rename | The Pocket may read `Cores/.disabled` as a core folder with no `core.json` and show a load error in Developer > Builds |
| C. Rename `core.json` to `core.json.off` | Core folder stays in place | A core folder with no `core.json` may be listed as broken |
| D. Edit `core.json` | Would need a field that does not exist | Not possible, and it changes the author's files |

**Recommendation: A.** Everything the user cares about stays put and untouched: saves (`Saves/`), settings (`Settings/<id>`), presets, save states, the core's assets and the shared platform files; only the core's own folder moves, so enabling restores exactly the previous state. A small `Disabled Cores/omega-disabled.json` records when and why Omega disabled each core (a core moved there by hand is still listed, with no reason).

**Hardware check to run before the feature is offered (CARDWRITE, with approval):** (1) move one harmless core (a copy or a core with no saves), clear the menu caches, boot: it is absent from the openFPGA menu and Tools > Developer > Builds shows no error; (2) move it back: it is listed and starts; (3) saves, settings and Dock behaviour unchanged. If (1) shows an error or a listing, fall back to B or C and re-test; the move code is the same.

Interactions to remember: a Pocket Sync or Pupdate run may reinstall a core it does not find in `Cores/` (their update flows install selected cores); Omega's installs of a core id that is disabled enable it first as part of the same reviewed plan; deleting a disabled core is the existing removal flow pointed at the holding folder.

## 3. Detecting a core that cannot work (`core_health`)

A core is **Broken** when the lint finds an unreadable `core.json`, a missing required JSON file or no bitstream (the fatal rules L01, L02, L09, L11), and **Unusable** when a file it requires is not on the card: a fixed-name file (a BIOS) or, for a required game slot, no file at all with an accepted extension in the slot's folder. Slots that are fed another way are not judged by their own files: a core with an instance-JSON slot (parameter bit 4) loads its other slots from the instance file, and a `deferload` slot is loaded by the core. A core whose slots are all optional is reported "needs no files" and can never be flagged for missing files.

Run read-only on the real card (41 cores): 35 healthy (one needing no files: Pong), **6 unusable, all genuinely without games**: SG-1000, Game and Watch, PC Engine, Pokemon Mini, WonderSwan and the Neo Geo Overdrive build (no per-game JSON). The first version of the rule wrongly flagged the main Neo Geo core and the PC Engine CD core (ROM parts and discs come in through the instance file); handling instance slots fixed both, which is exactly the kind of peculiarity the manual override exists for.

**Why the verdict is only ever a suggestion:** no static check can know a core's private habits (a core that loads from a cartridge adapter, one that boots a built-in program, one that reads files by a name the slots do not list). So: nothing is ever disabled without the user's choice or an explicit "disable automatically" setting, every disabled core is one click from enabled, and a core can be marked "keep enabled" so it is never suggested again.

## 4. The setting and the dialog

**Setting** (Settings > Cards, "When a core cannot work"): **Ask me** (default), **Disable it automatically** (still reversible, listed in the Disabled section with the reason) or **Never suggest**. Stored in Omega's own preferences, never on the card.

**Dialog**, shown when a card opens and cores are flagged that are not disabled and not marked keep-enabled (once per card per session, again when a new core becomes flagged): title "Some cores cannot work yet", a list with one row per core (icon, name, the plain reason and what would fix it, a checkbox, ticked by default), buttons **Disable selected** and **Not now**, and a tick **Don't remind me about these cores again** (marks the unticked ones "keep enabled"; ticked cores are disabled). The dialog says the plain consequence: "The Pocket will not list a disabled core. Nothing is deleted, and you can enable it again from Cores." Disabling goes through the usual plan and confirm (one list of moves, a token), under the card write lock.

**Manual control, always available:** in a core's header a switch **Visible on the Pocket**; on the Cores overview a **Disabled** section (name, version, when, why Omega disabled it) with **Enable** per core and **Enable all**; the tree shows a disabled core dimmed with a "disabled" chip under its platform. Enabling a core that is still flagged warns ("It still lacks gba_bios.bin. Enable anyway?") and never blocks; enabling offers **Keep enabled** so it is not suggested again. Cores flagged for a reason that a later install fixes (the BIOS arrives) are not re-disabled; a disabled core whose files become complete is offered "Enable" in the dialog on the next open.

## 5. Engine, host, CLI

| Piece | Job |
|---|---|
| `core_health` (done) | verdict and reasons per core; `tau core-health <card>` |
| `core_state` (done) | `plan` / `execute` (move, verify, roll back a failed move, record reasons, clear the five menu caches), `list_disabled`; `tau core-disable`, `core-enable`, `cores-disabled` (plan by default, `--confirm <token> --yes` to move) |
| `exemptions` (next) | "keep enabled" ids per card, in Omega's app data (card volume name plus core id) |
| host (next) | `core_health_card`, `core_state_plan`, `core_state_execute` (card write lock), the setting in prefs |
| UI (next) | the dialog, the header switch, the Disabled section, Settings row, the tree chip; Playwright coverage for the flows |

## 6. Decisions needed

- **S-1** Holding folder name and place: `Disabled Cores/` at the card root (recommended, visible, outside `Cores/`), or a hidden dot folder.
- **S-2** Default of the setting: **Ask me** (recommended).
- **S-3** Should Tau's own cores ever be suggested for disabling (recommended: yes, same rules; the health check passes them today)?
- **S-4** Approve the CARDWRITE hardware check in section 2 before any UI is built on the move.
