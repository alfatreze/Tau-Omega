# Tau Omega v0.6.0

Tau Omega now follows Tau's newer release system and gains a meter preset editor, Tau Alpha's own meter lab, and card names the Pocket can always open.

## Meter presets and the Meter Lab
- **Meter Lab** is one workspace: pick a meter from the row at the top, see it drawn by the Pocket's own meter code, and shape its presets next to the preview. Every change shows in the preview as you make it. There is one meter choice, not two.
- **Presets** are files on the card (`tau-presets/<meter>/`). Edit the settings of the 19 meters from Tau's own list, rename, duplicate, reorder and delete presets. Each preset keeps its number for ever, so the Pocket's remembered choice never lands on the wrong one. A meter can hold up to 127 presets: the list scrolls in its own area, has a search box, and works with the arrow keys.
- **Choose what goes on the card.** Each preset has an "on the Pocket" tick; the list says how many will be on the Pocket, and "Put all on the Pocket" / "Take all off the Pocket" act on all or on what a search shows. A preset that is not on the Pocket stays in Omega with its number reserved, ready to put back (the Pocket ignores a hidden flag, so it is simply not written to the card).
- **Factory presets, plainly:** "Add missing factory presets (n)" adds only the ones the list lacks and changes nothing else; "Start over with the factory presets…" (under More) replaces the whole list, after a confirmation.
- Every write is a reviewed plan, a backup outside the card, a verified write (presets first, index last) and a rollback if anything fails.
- **Settings shows the chosen preset by name** for the meter slot, read from the core's own files on the card.
- **Tau Alpha's real meter lab** is built in (a pinned copy of Tau Alpha's lab, shown as it is in a sandboxed frame): the real meter drawing code compiled to WebAssembly, with the real themes and registry. It shows the preset you are editing and tells you what the firmware would ignore or clamp. **Provenance check:** the hash of every module the lab compiled is compared with the manifest Tau Alpha shipped with it, and the page says if one differs.
- **CLI:** `tau meter-presets-show | meter-presets-plan | meter-presets-install` run the same engine as the app.
- Tested on a Pocket (owner, TAU DEV 213): the factory presets list from the card, a custom preset is remembered across a reboot by id, and a chosen preset that vanishes falls back to DEFAULT.

## Card names the Pocket can open, and the name sidecar
- The Pocket opens a file path as **code page 437**, one byte per character. Omega now writes only names it can open: accents the Pocket can draw are kept, other scripts are transliterated (Cyrillic, Greek, Hebrew, Arabic, Thai, kana and kanji, Korean), emoji and symbols are dropped, forbidden characters become `_`, Windows device names are made safe, and two names that come out the same are numbered ` (2)`, ` (3)`.
- The library **index text follows the same rules** (no `?` titles) and folder and file names are stored as code page 437 bytes.
- **Name sidecar** (`tau-names.json` and `tau-names.bin`): the original names and tags are kept beside the index for the Pocket to show where it can. Both files are written together, rebuilt with the index, and removed when there is nothing to record.
- Tested on a Pocket (owner, TAU DEV 219) with a card Omega wrote: titles show without `?`, tracks open and play, the sidecar loads (`NAMES STATE 1`, `NAMES ERR 0`).
- Refresh now also rebuilds an index whose track order is out of date (before, a valid-looking index counted as "nothing to do").

## Tau's newer release system
- **Superseded cores:** when a Tau package's manifest says it replaces older Tau cores, the install plan offers them for removal; nothing is removed automatically.
- **Requirements are checked at the plan:** a package that needs a newer Omega (`min_omega`) or a capability Omega lacks is refused before anything is written; optional data-slot files are a warning, not an error.
- **Build-bound files** (`tau.rom`, the cold start file, the pack library) are left alone by updates and refused by the file tools, as Tau's format requires.
- **Settings decode** the core's saved settings in both the old and the v0.7.0 layouts (checked against real captures), and meter pack bundles are read and composed byte-for-byte as Tau's reference does.
- **Marketplace:** one card per core id across sources; the source is a tag.

## Fixes
- Ejecting sweeps the whole Pocket card of `._` stubs and `.DS_Store` first (other volumes and OS folders untouched).
- On an exFAT card, macOS lists accented names in a decomposed form. Track order and the verification ledger now use the precomposed form, so accented files are ordered correctly and a repeat sync no longer plans them as updates.
- Several tests were made independent of one another (temp folders unique per run).

## Known limits
- The meter lab is a **pre-0.7.0 drop** of Tau Alpha's lab; the final drop comes with Tau's v0.7.0 release.
- CJK text is shown transliterated on the Pocket (the Pocket's font draws Latin-1 only); legacy Shift-JIS tags are not auto-detected.
- Installers are not signed (macOS asks for Open Anyway on first launch; Windows SmartScreen warns).
- The `replaces` path (superseded cores) is tested on constructed manifests only: no real Tau release carries one yet.
- Video is planned and ships switched off.
