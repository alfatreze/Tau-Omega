# Omega in the core development loop (spec 2026-10-15, not built)

Owner request: consider how Omega can help a core developer's loop, using what Tau needed during its own development as the cues. Facts below are read from the Tau Alpha repository's records (`CLAUDE.md` execution log, `docs/CARD_INSTALL_PROCEDURE.md`, `tools/install_dev_core.py`, `tools/package_dev_build.py`; read-only, nothing copied: the rules here are written from Analogue's own documentation and from the incidents, not from Tau's code). The aim is **generic**: any openFPGA core author, Tau included.

## 1. What the loop looked like, and where it hurt

The loop: change the core, build (Quartus fit or firmware), package, install on a test card, run on the Pocket, read the result (photos of the screen, screenshots, saved settings), decide, repeat; many iterations a day, each a numbered test core on the card.

| # | Need | Incident in the log | What Omega can do |
|---|---|---|---|
| 1 | **Pre-flight check of a package** | B-141/B-142: a core vanished from the Pocket menu; the cause was a `description` of 264 characters (limit 63) after two wrong theories; B-141 also doubted a `-alpha.N` version, which the docs allow | A **core lint**: every documented limit and rule, with the doc reference, before anything is written (section 3) |
| 2 | **The Pocket's menu cache** | B-143: the five catalog cache files were never cleared for a whole session of installs, so new cores did not show | Install already clears them (D-card procedure); the dev view says so in the result ("Menu cache cleared, the Pocket will rebuild it") and the lint names the symptom when a core is "installed but missing" |
| 3 | **A bitstream that is bit-reversed twice** | B-353: "Bridge not responding" because a raw-to-Pocket conversion was applied to an already-converted file | The Builder converts raw `.rbf` to the Pocket's reversed format once, records both hashes, and **refuses a file that is already converted**; the lint compares the card's bitstream to the recorded build |
| 4 | **Firmware and bitstream must belong together** | B-394/B-448/B-563/B-582: black screens from a ROM built with other flags, or a ROM checking another core revision; fixed by a pairing marker and a gate | Omega cannot know a core's private markers, but it can **verify the card against the package it came from** (every file's hash) and show the `tau-compat.json`-style needs when a core ships one (already built for Tau) |
| 5 | **Stale index root after copying media** | B-136: a copied library index pointed at another core's folder; browsing worked, playing did not | The Library already rebuilds the index with the right root; the dev install does it as part of "carry media from the previous build" |
| 6 | **Numbered test cores pile up** | `TAU DEV NN`, `TAU_0_5_0_A_NN`: dozens, each removed by hand after a backup | **Build series**: cores whose names share a pattern are grouped; keep the newest N, remove the rest in one reviewed plan with verified backups |
| 7 | **Backups that grew to 34 GB** | B-542: 99% of it duplicate media | The removal and replace plans leave out media that is provably carried elsewhere (the rule is in Tau's procedure; Omega's `install_backup_root` gets the same option) and a **Backups manager** shows size per backup and prunes with confirmation |
| 8 | **Which card is the test card** | B-690: the owner's real card was lost, CARDWRITE became the test card, tools had to be re-pointed; Omega test namespaces were left on a Tau card | **Card roles**: a card is marked Test or Real (stored by Omega, not on the card); dev installs only go to Test cards, and a Real card needs an extra confirm naming it |
| 9 | **Reading the hardware result** | Every iteration ends in screenshots of a Check report (QR / pixel grid), saved settings, the Info page; read by hand, decoded with scripts | **Results**: watch `Memories/Screenshots`, decode new Check reports the moment the card is mounted (Omega already decodes QR and the pixel grid), show the verdict list, compare with the previous run, attach to the install that preceded them |
| 10 | **Settings the core saved** | `interact_persist.json` read by hand against `interact.json` (ids reused, a 16-entry cap that silently dropped settings, B-455/B-456) | Settings viewer already decodes by the core's own menu; the lint adds the **16-entry cap**, duplicate or colliding persist ids and the "reloadable" bit clutter as warnings |
| 11 | **Test media** | The same Test Album synced to each new core, index rebuilt, covers checked | A saved **test media set** per core family: one click "sync the test set and rebuild the index" after an install |
| 12 | **A record of what ran** | `AUDIT_TRAIL.md`: build, hashes, install, screenshots, verdict, per iteration | The existing journal gains **run notes**: install + its hashes + the results + one free-text line, exportable as a plain report |
| 13 | **From build output to package** | `package_dev_build.py`: reverse the bitstream, write core.json from a template with version and naming rules, copy the ROM, icon, platform files, zip | The **Builder** (section 4) does this for any core, without Python |
| 14 | **Always-on build fits** | Quartus runs on a VM for ~1.5 h; results collected by hand | Out of scope for Omega (no remote builds); the **Builds** inbox only needs the finished files |

## 2. Shape

A **Developer** section, hidden until switched on in Settings ("Show developer tools"), so ordinary users never see it. Four pages and one shared engine:

1. **Builds**: a watched folder (or a dropped package): lists packages that are ready (name, version, bitstream hash, size, what changed against the installed core), **Install to test card** (the existing plan, backup and rollback; one click to the plan, still a confirm), **Reinstall** (same core again), **Remove older builds** (the build series), and "carry media from the previous build" with the index rebuilt.
2. **Check** (the lint): drop or pick a package, a folder or an installed core; the full list of findings with severity, the rule, where it comes from and how to fix it (section 3). Runs automatically before every dev install and before the manual-add preview (`MARKETPLACE_SPEC.md` section 7).
3. **Results**: reports and screenshots since the last install (section 1, row 9), with the previous run beside it.
4. **Builder**: bitstream (+ optional firmware/data) to package (section 4).

## 3. Core lint (`core_lint`, shared engine)

Findings are `Error` (will not load or will mislead), `Warning`, `Note`; each has an id, a plain sentence, the documented rule it comes from, and a fix.

- **core.json:** valid JSON and the documented structure; `shortname` equals the folder's shortname and is at most 31 characters; `description` at most **63**; `author` at most 31; `url` at most 63; `version` at most 31 and SemVer-shaped (pre-release suffixes are fine); `date_release` a real date; platform ids exist as `Platforms/<id>.json` in the package or on the card; `framework.version_required` present.
- **Files present:** the seven JSON files, `icon.bin` of the documented size, the bitstream file (`*.rbf_r`) non-empty and a multiple of the expected block size; nothing outside `Cores/`, `Assets/`, `Platforms/`.
- **data.json:** slot ids unique; parameters sane (core-specific bit set on a slot whose file is in the core's folder); required slots with fixed names have their files in the package or are reported as "you supply this"; exact size consistent with the shipped file.
- **interact.json:** the **16-entry cap** (entries past it are silently dropped by the Pocket); persist ids unique and not colliding; unreachable `when` references; the clutter caused by reloadable bits.
- **Bitstream sanity:** not zero-filled; **not already converted** when the input is a raw file; hash recorded; compared with the card's copy when an installed core is checked.
- **Packaging:** names the card's file system can hold; no `._` or `.DS_Store` junk; zip layout (only for a zip).
- **Symptom helper:** "installed but not in the Pocket's menu" runs the checks that explain it (invalid json, over-long fields, stale menu cache, wrong folder name).

Each rule carries the Analogue documentation page it is based on, so a finding is never "because Omega says so".

## 4. Builder (bitstream to package)

Input: a raw Quartus `.rbf` (or `.sof` is refused: wrong file), optionally a firmware ROM and data files, an icon PNG, a template (a previous core.json, or "new core" with fields to fill). Output: a package folder and zip in the documented layout, plus a **build record** (raw hash, converted hash, inputs, version, date, tool version) kept by Omega and offered as a text file beside the zip. Rules: convert once and refuse an already-converted input; fill and validate `core.json` with the lint before writing; naming helpers for test series (`<name> DEV NN`, or the SemVer pre-release form `x.y.z-alpha.N`, counter increments per build); icon PNG to `icon.bin` (the existing codec, reversed); platform JSON and banner optional. The Builder never writes to a card; the package goes through the Builds page.

## 5. Safety

Dev writes follow the same plan, confirm, backup and rollback as everything else; the extra rules are: card roles (Test or Real), a Real card needs a second confirm that names it, no automatic writes of any kind (a watched folder only *lists*), nothing is deleted without a verified backup, and a failed lint Error blocks the install unless the developer explicitly overrides it for that build (the override is recorded in the run notes).

## 6. Build order and what is reused

| Step | What | Reuses |
|---|---|---|
| D0 | `core_lint` rules + CLI `tau core-lint <package\|folder\|core> [--json]`; tests from the incidents above (a 264-char description, a 17th interact entry, a double-converted bitstream) | `core_package::vet_zip`, `card` scan, `file_purpose`, `taxonomy` |
| D1 | Card roles + the dev-install guard | `begin_card_write`, card marker |
| D2 | Builds page: folder watch (listing only), install, reinstall, build series cleanup, backups manager | `install_exec`, `remove`, `install_backup_root`, `sync` for carried media |
| D3 | Results page: new screenshots since the last install, decoded and compared | `taud`, `tpg`, `diagnostics`, `screenshots` |
| D4 | Run notes in the journal and the plain report | `journal` |
| D5 | Builder | `icon` codec, `core_lint`, the zip writer used by package export |
| D6 | Test media sets and the "sync, rebuild index" button | Library sync and `tau_library` index writer |

## 7. Decisions needed

- **D-1** Hide the whole Developer section behind a Settings switch (recommended), or show it always?
- **D-2** Card roles stored by Omega (recommended: by volume name plus a marker file hash, nothing written to the card), or a marker file on the card?
- **D-3** Builder scope: include it (D5) or stop at lint, Builds and Results first (recommended: first, Builder after the others prove useful)?
- **D-4** Lint rules that are Tau-specific (the pairing marker) stay out of the generic engine and come from a core's own `tau-compat.json`-style manifest (recommended), or Omega hard-codes Tau's?

**Built 2026-10-15 (D0, engine and CLI):** `core_lint` (rules L01 to L17 over a package zip, a folder or an installed core; `tau core-lint <zip|folder>` and `tau core-lint --card <root> <id|--all>`). Checked against the 38 cores on the real card: it first reported false errors, which changed three rules: the **bitstream is the file `core.json` names** (`cores[].filename`; several cores ship `.rev`, and the card ignores case), the **63-character description limit is a warning** (agg23's NES and SNES cores run on a Pocket with 70 to 142 characters; the earlier "it hid the core" story was really the stale menu cache), and **duplicate interact ids are a warning** (the Amiga core shares one deliberately). Errors left on the real card: none. **Wired into the manual-add preview (2026-10-15):** the sheet shows a Core check section with each finding, its fix and its rule; an Error blocks the trust tick until "Install anyway" is ticked, warnings never block. Not yet wired into the dev install (D1/D2), no standalone Check page.

**Decisions 2026-10-15:** D-1 the Developer section is hidden behind Settings > Library > "Show developer tools" (off by default). D-2 card roles are stored **on the card** as a hidden `.omega-role` file (owner chose the file over Omega's own settings): its presence with `{"role":"test"}` makes a card a Test card; no file, or anything unreadable, means **Real**. Omega only writes or removes a file of that shape.

**Built 2026-10-15 (D1):** `tau_core::card_role` (read, set test, clear; 3 tests), host commands `card_role` / `set_card_role` (card write lock), pref `developer_tools`, the Card role switch on the Card overview (only with developer tools on), and the Real-card guard: with developer tools on, `UpdatePlanView` (used by Packages, the Marketplace install and manual add) asks for the card's name before "Confirm and install" unless the card is a Test card. `devtools-test.cjs` (12 checks). Not yet: the Builds, Results, Check and Builder pages themselves, and a real-card run (writing `.omega-role` to CARDWRITE needs approval).

**Built 2026-10-15 (D2, Builds page):** `tau_core::build_series` (naming schemes: a developer-defined list, each reading one field of a build (shortname, version, core id or file name) with a template of `{series}` `{n}` `{major}` `{minor}` `{patch}` `{pre}` `{any}` and literal text; the developer's schemes are tried first, then three built-ins (`TAU DEV 12`, SemVer pre-release, SemVer); a hand-written matcher with a step budget so no template can run away; SemVer ordering with pre-releases below the release; 6 tests) and `tau_core::builds` (scan a folder of zips and package folders one level down, place each build in its series, compare with the card's copy, and name the installed cores of the same series that are older; 3 tests). Host commands `builds_config`, `set_builds_config`, `try_scheme`, `builds_list`, `build_zip`; prefs `builds_dir` and `build_schemes` (preserved when Settings saves). UI: the **Builds** page, shown only with developer tools on: folder, naming-scheme editor with a live "try a name", builds grouped by series newest first with relation to the card, core-check chips, **Install to the card / Reinstall** (the usual reviewed plan and the Real-card confirmation), and **Remove N older** (the usual reviewed removal, one core at a time, each backed up). `builds-test.cjs` (23 checks). Not yet: a watched folder that refreshes by itself (Rescan is manual), the carry-media-from-previous-build option, the Results/Check/Builder pages, a real-card run. The default folder is whatever you choose: Omega cannot know where your checkout lives.
