# Cores section: platform tree, per-core Files / Saves / Screenshots / Settings (spec, 2026-10-15)

Branch `cores-section` (worktree `../tau-omega-cores`, built on `marketplace`). Owner request: a **Cores** entry in the menu that lists every core on the open card organised by platform, expandable in the side menu; selecting a platform shows its cores, selecting a core opens a **collapsible right panel with the core's details** and main **tabs: Files** (ROMs, tapes, BIOS...; drag and drop in), **Saves**, **Screenshots** (the core's, if it can be known) and **Settings** (saved settings, missing BIOS...). A file parser comes later: the Files model must leave room for it.

> **Status 2026-10-15: C0-C7 built.** Engine (`corehub`, `core_files`, `core_settings`, `core_screens`, `core_add`; 23 new tests plus a read-only run against a real card: 31 platforms, 38 cores, no errors), host commands, CLI (`tau cores`, `core-detail`, `core-files`, `core-settings`, `core-screens`, `core-add-plan`, `core-add`), UI (menu tree `NavTree`, `CoresView`, tabs, `CoreInspector`, `AddFilesSheet`, `dropTarget`), browser checks `ui/scripts/cores-test.cjs` (43, in CI). **Not verified on a real window:** Tauri's native drag-and-drop event (the mock uses the browser's drag events; the "Add files..." and "Add folder..." buttons use the native picker and cover the same flow) and a real card write (the engine's writes are proven on scratch cards; every refusal has a test; the read-side was run read-only on a real card). Mutation check of the add flow: the confirmation check and the backup-outside-the-card rule are caught by tests; the post-copy SHA-256 read-back cannot be exercised without fault injection, so it is covered by design, not by a test.

## 1. What the card actually holds (verified read-only on a real Pocket card, firmware 2.7)

| Thing | Where | Notes |
|---|---|---|
| Core | `Cores/<Author.Core>/` | `core.json` (metadata: author, shortname, description, url, version, date; framework: firmware, Dock, link port, sleep; `platform_ids`), `data.json` (slots), `interact.json` (menu), `icon.bin`... |
| Platform | `Platforms/<id>.json` + `Platforms/_images/<id>.bin` | `platform.{category,name,manufacturer,year}`. A core names one or more platform ids; the first is its main one. `Assets/<id>` exists whether or not a Platforms file does (about 80 folders on the test card). |
| Files for a core (ROMs, BIOS, tapes, disks) | `Assets/<platform>/common/` (shared by every core of the platform) and `Assets/<platform>/<Author.Core>/` (that core only) | Which one a slot reads is its parameter bits (`core_specific_file`, `platform_index`); `cardlayout` already decodes them. A slot with a fixed `filename` (a BIOS) with `required: true` and no file on the card is a **missing file**. |
| Saves | `Saves/<platform>/common/<game>.sav` or `Saves/<platform>/<Author.Core>/...` | Same rule as assets, for non-volatile slots. `Saves/pocket-sync-save-backup__*.zip` are Pocket Sync's backups (not a core's; listed separately, never as a save). |
| Save states | `Memories/Save States/<Author.Core>/<YYYYMMDD_HHMMSS>_USR_<hash>_<game>.sta` | Per core, with the game name in the file name. |
| Screenshots | `Memories/Screenshots/<YYYYMMDD_HHMMSS>.png` | Flat, one folder for every core. **Each file ends with a 600-byte block (after the PNG and two embedded Analogue pictures) that names the core**: magic `01 52 50 41`, author at 0x48, core short name at 0x68 (`Author.Core` is the `Cores/` folder), version 0x88, loaded file 0xA8 (256 B), platform id 0x1A8, platform name 0x1B8. Present in all 489 screenshots of the test card. |
| Saved settings | `Settings/<Author.Core>/{Interact,Input,Video}/_core/{interact,input,video}_persist.json` | `interact_persist` holds `{id, type, val}` per menu variable; the names come from the core's own `interact.json` (`variables[].name`, `id`, `type`, `graphical.min/max`, list `options`). |
| Presets (per game) | `Presets/<Author.Core>/{Input,Interact}/<platform>/<Author.Core>/<game>.json` | 1,004 files on the test card (arcade cores ship a preset per game). |

Consequences: Files, Saves, Save states, Settings, Presets and Screenshots are all **exact per core**. A screenshot without the block (older firmware, edited file) is the only guess, and is labelled "probably" (taken within 10 minutes of one of the core's save states). Correction 2026-10-15: an earlier draft of this spec said screenshots carry no core; that was wrong, the block sits after IEND.

## 2. How other tools do it (and what we take)

- **Pocket Sync** (the community manager): a flat feature menu (Cores, Saves, Screenshots, Platforms...). Saves and screenshots are card-wide lists with backup/export and bulk delete; there is no per-core hub and no drop target. **Take:** the card-wide views stay reachable (Screenshots "all"), and its backup zips must not be mistaken for saves. **Add:** a per-core hub.
- **OpenEmu, Playnite, LaunchBox, EmulationStation** (platform-centric game managers): a source list of systems, a grid or list of what is in the selected system, an info pane. **Take:** platform as the primary grouping, counts on the tree, the selected item's details beside the list.
- **Finder / VS Code / Apple HIG "sidebar + content + inspector":** a tree in the sidebar, tabs for content kinds, an inspector that can be hidden. **Take:** this three-pane pattern exactly (tree, tabs, collapsible inspector), keyboard model of a tree (arrows, Left/Right to collapse/expand, type-ahead optional), the inspector state remembered.
- **Steam / Playnite "missing" badges:** surface what blocks a game from running before the user hits it. **Take:** a Settings-tab "Needs attention" list (missing BIOS) and a count dot in the tree.

## 3. Architecture

Engine (portable `tau_core`, no UI, each function plan-first where it writes):

| Module | Job |
|---|---|
| `corehub` | `platform_tree(card)`: installed cores grouped by main platform id with the platform's name/category/maker/year; `core_detail(card, id)`: core.json facts, platforms, data slots, folder paths and whether they exist, firmware/Dock, catalog-free. |
| `core_files` | `list(card, id)`: the folders a core reads/writes (assets common, assets core, saves, save states, presets), each with entries (relative path, size, modified, kind guessed from the slot extensions: rom / bios / save / state / other), totals and truncation; `missing(card, id)`: required slots with a fixed filename that are absent. `plan_add` / `execute_add`: add files to one of the two asset folders (section 5). |
| `core_screens` | `list(card, id)`: all screenshots with their time, and the subset attributed to the core (section 4.3); thumbnails through the existing `image::thumbnail_png`. |
| `core_settings` | `read(card, id)`: saved settings decoded against the core's `interact.json` (name, kind, value, the option label for a list, min/max for a slider), plus the Input and Video files present and the number of presets. Read-only. |

Host (`src-tauri/src/core_cmds.rs`) thin wrappers; CLI parity (`tau cores`, `tau core-detail`, `tau core-files`, `tau core-screens`, `tau core-settings`, `tau core-add-plan` / `tau core-add`). UI: `coresStore.ts` (selected platform/core, inspector open), `CoresView.svelte` (header, tabs, platform view), `CoreInspector.svelte`, tabs `CoreFiles`, `CoreSaves`, `CoreScreenshots`, `CoreSettings`, a `NavTree` inside the existing sidebar, a `dropTarget.ts` helper (Tauri window drag-drop event, HTML5 fallback for the browser mock).

Why this shape: the listing logic is portable and testable on synthetic cards (like everything else here), nothing in the UI knows folder rules, and the future file parser plugs into `core_files` entries (`kind`, `slot`, later `parsed`) without touching the UI.

## 4. Design (design-system "extend" format)

### 4.1 Problem
The Pocket's files are organised by folder and system, but people think by core ("my GBA stuff"). Today Omega shows cores in two flat lists and has no view of a core's ROMs, saves or settings, and no way to drop a ROM where the core will find it.

### 4.2 Existing patterns reused
| Existing | Reused for |
|---|---|
| Sidebar nav buttons, `nav-group` expander ("Tools & settings"), `nav-sub` | The Cores entry and its platform/core tree |
| `.page` header (eyebrow, title, lede), tabs/steps look (`.steps`) | Cores page header and tab bar |
| `.core-list article`, `.chip`, `.chip.capable`, `.settings-card`, `.quiet`, `.primary` | Core rows, status chips, panels, buttons |
| `modal` focus trap, plan-review pattern (`UpdatePlanView`, `RefreshLibrary`) | The "add files" review before any write |
| Marketplace sheet pattern (`mk-sheet`, `Requirements` list) | The inspector's facts list |
| `VirtualList` | Long file lists (thousands of ROMs) |

### 4.3 Proposed design
**Layout (desktop):** `sidebar (menu with the tree)` | `main: header + tabs + tab content` | `inspector (core details, 320 px, collapsible)`. Below 1100 px the inspector becomes a slide-over from the right; below 720 px the tree is the existing collapsing menu.

**Menu tree.** "Cores" is a nav group with a chevron (state remembered). Open: a filter box (shown when there are more than 12 platforms), then platforms sorted by name, each a tree node with a chevron, the platform name and a count; expanding shows its cores (icon-initial, shortname, a small dot when something needs attention). Click a **platform name** = open the platform view (all its cores as rows with version, size of files, status); click a **core** = open the core. Active node uses the existing active nav style. Keyboard: Up/Down move, Right expands or enters, Left collapses or goes to the parent, Enter opens, Home/End.

**Platform view.** Header: platform name, maker, year, category, banner if present. A table of its cores (name, version, author, files, saves, attention) and a "Shared files" summary (`Assets/<platform>/common`): count, size, with the drop target (files dropped here are available to every core of the platform).

**Core page.** Header: core icon, name, version, author, platform breadcrumb (`Cores > Game Boy Advance > Spiritualized.GBA`), buttons: Open in Marketplace (when in the catalog), inspector toggle. Tabs (`role="tablist"`, arrows, Home/End):
- **Files**: two folder sections, *Shared by every GBA core* (`Assets/gba/common`) and *Only this core* (`Assets/gba/Spiritualized.GBA`), each a list (name, kind chip, size, date) with search, sort and a **drop zone** (see below). A "Needs attention" strip lists missing required files (BIOS) with the exact name and folder, and a drop on it fills them in. Entries carry `kind` and `slot` now; a parser column is reserved for later.
- **Saves**: save files (with the game name, size, date and which folder), save states (with game name and time), and a note when Pocket Sync backup zips are on the card (counted, not listed as saves).
- **Screenshots**: a thumbnail grid. Default filter **"This core"**: screenshots whose embedded block names this core (badge "this core", caption shows the game), plus older block-less files taken within 10 minutes of one of its save states (badge "probably"). "All screenshots" shows everything with the owning core in the caption; the large view shows core, version, platform and game.
- **Settings**: *Needs attention* (missing BIOS and any required file not on the card), *Saved settings* (decoded name = value, e.g. "Rumble Strength 5", with list option labels), *Controls and video* (which saved files exist and when changed), *Presets* (count and folder), *What this core reads* (its data slots: name, accepted extensions, required, file name, shared or core-only).

**Inspector (right, collapsible):** core icon, name, description, version and date, author and URL (text + copy, no external opener yet), Pocket firmware, Dock, link port, sleep, platform(s) with names, folders on the card with sizes, a catalog line when known ("Update available in the Marketplace"), counts (ROMs, saves, states). Collapse button in its header and `I` key; open/closed state saved in the browser storage per user.

**Drag and drop.** Dropping files or folders anywhere on the Files tab highlights the two folder zones; dropping on a zone chooses the destination (default for a dropped file when released on the tab background: shared folder, as most cores read from it; the review dialog shows and lets you switch). Nothing is written on drop: a **review sheet** shows each file (name, size, destination, state: new / identical already there / different already there / refused), warnings (extension no slot of this core accepts, name longer than the card allows, not enough space), then **Add N files**; differing files are skipped unless "Replace" is ticked (replaced files are backed up outside the card first). Folders dropped keep their folder name. Each copy is verified (SHA-256 re-read). A result line with Undo is not offered (adding is non-destructive); replaced files can be restored from the backup path shown.

### 4.4 States
| Element | Default | Hover/focus | Loading | Empty | Error |
|---|---|---|---|---|---|
| Tree node | label + count | row highlight, focus ring | skeleton rows while the card is read | "No cores on this card" with a link to the Marketplace | red "Could not read the card" with Retry |
| Tab content | list | row highlight | spinner + "Reading the card…" | per tab: "No ROMs yet. Drop files here." / "No saves yet. Play the core and save in game." / "No screenshots for this core." / "Nothing saved yet." | inline red message, the rest of the page stays usable |
| Drop zone | dashed border, hint text | accent border while dragging, label "Drop to add to <folder>" | "Copying 3 of 12" with progress | (same as empty) | "Not added: <reason>" per file |
| Inspector | open | | | | |
| Review sheet | | | busy button "Adding…" | | refused rows in red, Add disabled when nothing can be added |

### 4.5 Tokens
No new colours: surface `#161f20`/`#1a2325`, border `#2c393a`, text `#e8ecec`/`#a6b3b2`, accent mint `#c1f0ad` for primary and the active node only, warning `#e5c88a` for "needs attention", error `#ff9b9b`. Spacing 8/12/16/24, radius 8/12, same type sizes as the Marketplace (11-15 px, 26 px titles).

### 4.6 Accessibility
Tree: `role="tree"`/`treeitem`/`group` with `aria-expanded`, `aria-selected`, roving tabindex, the keyboard model above. Tabs: `role="tablist"/"tab"/"tabpanel"` with `aria-controls`. Inspector: `role="complementary"` labelled "Core details", toggle with `aria-expanded`. Drop zones are real buttons too ("Add files…" opens the file picker) so dragging is never the only way. Status chips always carry text. Lists announce counts with `aria-live="polite"`. Focus returns to the invoking control after the review sheet closes. Targets 24 px minimum, 44 px for primary actions.

### 4.7 Copy rules
"Core", "platform", "shared by every <platform> core", "only this core"; "BIOS" only when the slot says so, else "required file"; never "asset". Screenshots: "Each screenshot records which core took it." "Probably" only for older files without that record.

### 4.8 Open questions (answered by building the simplest thing and listing it)
- Screenshot attribution is exact from the embedded block; the save-state time heuristic remains only for files without it.
- Removing files (added 2026-10-15): Files tab, asset folders only (`core_remove`, `tau core-remove-plan` / `core-remove`). Select rows, review (shared-folder readers, "needed to start" flag), then each file is copied outside the card, SHA-256 verified, and only then deleted; a wrong token, a changed file or an inside backup folder removes nothing. Saves and save states are not removable here. No rename/export of saves and screenshots; Pocket Sync already does bulk save backup.
- No parser yet: entries expose `kind` and `slot`; a `parsed` field is the extension point.

## 5. Safety (SAFETY_RULES.md)
Adding files is a card write: **plan first, shown, confirmed**; the host takes the card write lock; every copy goes to a temporary name, is read back and compared by SHA-256, then renamed; nothing is deleted; a replaced file is copied to a backup folder outside the card first (and listed); capacity is checked before the first byte; sources are never modified; names are kept as they are (no renaming) but a name the card cannot hold is refused with the reason; macOS `._` companions created by the copy are swept as the installer does; only the two asset folders of the chosen core's platform are valid destinations (the plan refuses any other path, `..`, absolute paths, symlinks in the destination tree). Tests on scratch cards prove every refusal; a real-card write is the owner's to run.

## 6. Build order
C0 spec (this); C1 engine `corehub` + `core_files` read side + `core_settings` + `core_screens`, tests on synthetic cards and a read-only run on the real card; C2 `plan_add`/`execute_add` + tests; C3 host commands + CLI; C4 UI shell (tree, page, tabs, inspector, store, mock); C5 Files tab with drag and drop and the review sheet; C6 Saves, Screenshots, Settings tabs; C7 browser checks, accessibility pass, docs, roadmap, test build.

## Update 2026-10-15: grouping and the Card page
- Tree and overview follow the openFPGA Library: **category > manufacturer > platform > core** (Console > Nintendo > SNES > core). The grouping comes from a built-in snapshot of `api/v3/platforms.json` (`taxonomy.rs`); a platform the Library does not know (Tau) uses its own `Platforms/<id>.json`, with nothing known it is "Other". Category order: Console, Handheld, Computer, Arcade, Arcade Multi, Media Players, others alphabetical, Other last.
- Tau (library-capable cores and ids starting `alfatreze.TAU`) is one **TAU** row under Media Players / alfatreze, whatever platform each build declares (`tau`, `tau_preview`, `tau_dev`...).
- The Card page (menu entry **Card**) has Overview (known cards, core updates, needs-attention list, music libraries) and History (the sync history, no longer a menu entry). The player-core grid, the other-cores list, the Details dialog and "Set as player" are gone; a library core's tracks, index, Open library and Refresh library are in the core's details panel.
- The Library page's **Target core** is chosen from cores whose data slots accept mp3/flac (library cores first), every other core by hand; `Core.audio_formats` carries it.
- Core updates moved (2026-10-15): the Card overview no longer lists them; it shows an **Updates** figure among the totals (a button to Cores > Updates when any are available). **Cores > Updates** is a tab of the Cores overview and lists only the cores that need one (a newer release or a replacement). The check runs once per opened card when the update check is on (`updatesStore.ts`).
- File purposes and icons (2026-10-15): every file in a core's asset folders gets a **group** (BIOS and system, Palettes, Disks, Memory and saves, Game setup, Game data, Other files), a plain-language **purpose** and its **source** (the core's own file list, a short built-in list of known files, or a labelled guess) from `file_purpose.rs`. Files a core needs to run (`support`: BIOS, palettes, configuration, unlisted files in the core's own folder) are listed in **Settings**, grouped, with the explanation; **Files** keeps the games. Icons are Phosphor Duotone (MIT, vendored, `ui/src/lib/icons/`), chosen from six candidates: categories in the tree and overview, file groups and file types in the lists.
