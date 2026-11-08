# Tau Omega v0.6.0

Tau Omega now has a **Meter Lab**: see each Tau meter drawn by the Pocket's own code and shape the presets it offers on your Pocket. It also writes **card names the Pocket can always open**, and follows Tau's newer release system.

## New and improved features

### Meter Lab (new)
- **One workspace.** Pick a meter from the row at the top (Winamp Bars, Winamp Scope, Chladni, Master VU, Layered Wave, Waterfall, Sound Map), see it drawn by the Pocket's own meter code, and change its settings next to the preview. Every change shows at once. There is one meter choice, not two.
- **Small view and Full screen.** A toggle above the preview switches between the meter's band of the Now Playing screen and the full-screen view. The picture is never scaled, so the bars stay sharp.
- **Three tabs.**
  - **Settings:** the name and every setting of the chosen preset, with Previous and Next to step through presets. DEFAULT is shown and says it cannot be changed.
  - **Presets:** the list of presets with search, up to 127 per meter, with Up, Down, Duplicate and Delete for the selected one.
  - **Common:** the preview options (colour theme, light or dark, accent colour, test sound, and the options of the meter on show).
- **Choose what goes on the Pocket.** Each preset has an "on the Pocket" tick. The list says how many will be on the Pocket, and buttons put all on or take all off (or just what a search shows). A preset you take off stays in Omega with its number reserved, ready to put back.
- **Factory presets, plainly.** "Add missing factory presets (n)" adds only what the list lacks and changes nothing else. "Start over with the factory presets…" (under More) replaces the whole list, after a confirmation.
- **Processing budget bar.** Under the preview: how many drawing commands the meter's worst recent frame costs against what its cost class allows. Green and "Under by 73 (73 %)" while it fits, red and "Over by 32 (8 %)" when it does not. It follows your changes within a fraction of a second.
- **Details panel.** The commands, processor time and cost of the meter, and what each option costs, as tables with the numbers lined up on the right.
- **Settings shows the chosen preset by name** for the Pocket's meter slot, read from the card.
- **Meters on the Pocket (preview only).** A design of the page where you will choose which meters are in the Pocket's list (seven rows). Every control is switched off and nothing is read from or written to your card: it waits for Tau Alpha to allow it.
- **Safe writes.** Presets are files on the card (`tau-presets/<meter>/`). Saving is a reviewed plan, a backup outside the card, a verified write (presets first, index last) and a rollback if anything fails. Each preset keeps its number for ever, so the Pocket's remembered choice never lands on the wrong preset.

### Card names the Pocket can open (new)
- The Pocket opens a file path as **code page 437**, one byte per character. Omega now writes only names it can open: accents the Pocket can draw are kept, other scripts are transliterated (Cyrillic, Greek, Hebrew, Arabic, Thai, kana and kanji, Korean), emoji and symbols are dropped, forbidden characters become `_`, Windows device names are made safe, and two names that come out the same are numbered ` (2)`, ` (3)`.
- Library titles follow the same rules, so no `?` titles.
- **Name sidecar** (`tau-names.json`, `tau-names.bin`): the original names and tags are kept beside the index for the Pocket to show where it can.

### Cores and Tau packages (improved)
- A Tau package that **replaces older Tau cores** now offers them for removal in the install plan. Nothing is removed automatically.
- A package that needs a **newer Omega** or a capability Omega lacks is refused at the plan, before anything is written, with the reason in plain words. Optional data-slot files are a warning, not an error.
- Tau's **build-bound files** (`tau.rom`, the cold start file, the meter pack file) are left alone by updates and refused by the file tools, as Tau's format requires.
- **Marketplace:** one card per core, whichever marketplaces list it; the source is shown as a tag.

### Smaller improvements
- **Eject cleans up first:** it sweeps the whole Pocket card of `._` stubs and `.DS_Store` files (other volumes and operating-system folders are left alone).
- **Command line:** `tau meter-presets-show`, `meter-presets-plan` and `meter-presets-install` run the same engine as the app.

## Fixes
- **Accented file names on an exFAT card.** macOS lists accented names in a decomposed form (`e` plus a mark), while the library index stores them whole. Two things went wrong: tracks sorted in the wrong order (`ö` as `o`), and a repeat sync planned every accented file as an update. Both now use the whole form.
- **A library index in a stale order** was reported as up to date. **Refresh library** now notices it and rebuilds the index and the name sidecar.

## Technical improvements
- **Lab embed contract 1.** The Meter Lab shows Tau Alpha's lab page exactly as Tau Alpha ships it. Omega adds nothing to the page: it speaks the lab's own messages (embed mode, options, size, cost, details). The page's contract number is checked, and a page that speaks another number is shown as it is, with a note.
- **Provenance check.** The hash of every WebAssembly meter module the lab compiled is compared with the manifest Tau Alpha shipped with it, and the page says if one differs.
- **Preset files** (`index.tpi`, `p<id>.tpr`): reader, writer and factory import are byte-identical to Tau's reference on all 51 factory files.
- **Card names and sidecar** are portable, UI-free modules of the engine (`card_name`, `name_sidecar`), checked against Tau Alpha's reference and corpus (62 cases); the sidecar is byte-identical to their writer. Two small pure-Rust dependencies were added (`deunicode`, `unicode-normalization`).
- **Library index** stores folder and file names as code page 437 bytes.
- **Meter pack bundles** are read and composed byte for byte as Tau's reference does (verified on a real bundle). Omega only reads them: it does not write the Pocket's meter file.
- **Saved settings** of the Tau core are decoded in both the older and the v0.7.0 layouts, checked against real captures.
- **Pinned lab drops** (`vendor/tau-lab/`), each verified against its checksums by a pin script; the app installs the newest one.
- **Tests:** 616 engine tests and 22 browser suites, all passing. Test folders are now unique per run, after a rare Windows-only test failure.
- **Build:** both lockfiles are complete (the Tauri shell's lockfile now includes the two new dependencies).

## What has run on a Pocket
Owner-observed on a Pocket: meter presets written by Omega list from the card, a custom preset is remembered across a reboot by its number, and a chosen preset that is removed falls back to DEFAULT (core TAU DEV 213); a library, card names and name sidecar written by Omega load with no `?` titles and the sidecar accepted (core TAU DEV 219). The Meter Lab page (its preview, tabs, budget bar and toggle) runs on the computer and is covered by browser tests, not by a Pocket.

## Known limits
- The lab is a **pre-0.7.0 drop** of Tau Alpha's lab; the final drop comes with Tau's v0.7.0 release.
- **Meters on the Pocket** is a design preview only (see above).
- CJK text is shown transliterated on the Pocket (the Pocket's font draws Latin-1 only); legacy Shift-JIS tags are not auto-detected.
- The replaces path (superseded cores) is tested on constructed manifests only: no real Tau release carries one yet.
- Installers are not signed (macOS asks for Open Anyway on first launch; Windows SmartScreen warns).
- Video is planned and ships switched off.
