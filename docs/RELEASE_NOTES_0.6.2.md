# Tau Omega 0.6.2

This release adds **Game Bub** support alongside the Analogue Pocket, and lets you change the picture of any platform or core, with community artwork packs.

## New and improved features

### Game Bub
- **Game Bub cards are recognised.** Open a Game Bub card and Tau Omega shows its cores, installs and removes them (with a checked backup outside the card first), and lists the built-in BIOS files.
- **A page for each Game Bub core** with Files, Saves and Settings tabs. Games are found by the file types the core says it plays, so a shared folder of ROMs still shows only that core's games. Saves and save states appear when a game with the same name sits beside them. Files that cannot be matched are tucked away, not mixed in.
- **Add or remove games** with a review first. Anything replaced or removed is copied outside the card and checked.
- **Pin a folder** for a core if you keep each system in its own place.
- **Game Bub firmware** can be checked and downloaded, like the Pocket's.
- **Game Boy colour palettes** can be installed and removed.
- The Marketplace has a **Game Bub** list with a simpler install card.

### Pictures for platforms and cores
- **Change image** on any platform or core. Pick from community artwork packs, or choose your own PNG or JPEG. For the Pocket, your picture is turned into the Pocket's own banner format and you see exactly how it will look before anything is written.
- **Artwork packs in Settings.** Browse each creator's sets by category and maker, search them, and apply a whole set, one category, one maker, or a single platform to your card. Replaced banners are backed up first.
- Packs are saved on your computer after the first time, so opening them again is quick.

### Cores and menu
- **Platforms the app knows about** now cover 75 systems, so names, years and makers are the same everywhere.
- **Simpler menu.** Platforms with one core open that core straight away.
- Clear empty screens when there is nothing to show yet.
- A dedicated Tau Omega update window, and the music section is now called **Music**.

## Behind the scenes
- New engine parts for platforms, per-device core handling, artwork and Game Bub files, with automatic checks for each.
- Cores that ship harmless extra files (a README or an INSTALL.txt) are no longer refused.

## Good to know
- Game Bub support is new. It was tested with practice folders and sample cores on a computer, not on a real Game Bub.
- Artwork was tested on a computer and practice cards, not on a real Pocket.
- The installers are not signed. On macOS, choose Open Anyway the first time. Windows may show a warning.
