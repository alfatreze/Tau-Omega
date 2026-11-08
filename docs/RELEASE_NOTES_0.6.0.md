# Tau Omega 0.6.0

This release adds the **Meter Lab**, where you can watch your Tau meters and shape how they look on your Pocket. Songs and folders are now always named in a way your Pocket can open, so titles no longer turn into question marks.

## New and improved features

### Meter Lab (new)
- **See your meters.** Pick a meter and watch it move, drawn the same way your Pocket draws it.
- **Small or full screen.** Switch between the small meter you see while a song plays and the full-screen version.
- **Change a meter and see it straight away.** Every change you make shows in the preview at once.
- **Presets.** Save your favourite looks as presets, up to 127 for each meter. Rename them, copy them, put them in the order you like, delete the ones you don't want, and search when the list gets long.
- **Choose which presets go on your Pocket.** Tick the ones you want. The ones you untick are kept safe in Omega, and you can put them back whenever you like.
- **Factory presets, made clear.** "Add missing factory presets" brings back any factory presets you removed, and leaves everything else alone. "Start over with the factory presets" replaces your list, and asks you first.
- **Workload bar.** A simple bar shows how hard a meter works your Pocket compared with what it is allowed to use. It is green when there is room to spare and red when the meter asks for too much, and it tells you by how much.
- **More detail when you want it.** Open the details panel to see what the meter costs and what each setting adds, in a neat table.
- **Settings shows the preset your Pocket is using,** by name.
- **Choosing which meters are on your Pocket (preview only).** You can see how this will look, but nothing works yet. It needs a change from Tau Alpha first, and Omega will not touch your Pocket's meters until then.
- **Safe saving.** Before anything is written to your card, you see what will change. A backup is kept away from the card, and if anything goes wrong, everything is put back.

### Names that always work on your Pocket (new)
- Omega now gives your files names the Pocket can always open. Accents the Pocket can show are kept. Other writing, such as Russian, Greek, Japanese or Korean, is turned into letters the Pocket can show. Emoji and symbols are left out. If two names would end up the same, they are numbered (2), (3).
- Song titles no longer show question marks.
- Omega keeps the original names and tags in a small extra file next to your music, so the Pocket can show them where it can.

### Cores and Tau updates (improved)
- When a Tau update replaces older Tau cores, Omega now offers to remove the old ones. It never removes them on its own.
- If an update needs a newer version of Omega, Omega tells you in plain words and stops before changing anything.
- Files that belong to one particular Tau version are left alone by updates, and cannot be removed by accident.
- In the Marketplace, each core now appears once, even when more than one marketplace lists it.

### Smaller improvements
- **Eject tidies up first.** It clears the hidden leftover files macOS puts on the card, then ejects.
- **Command line.** There are new commands for working with presets, doing the same as the app.

## Fixes
- **Songs with accents in their names.** On some cards, songs with accents could play in the wrong order, and syncing the same music a second time copied those songs again for no reason. Both are fixed.
- **A music list in the wrong order.** "Refresh library" used to miss this. It now notices and fixes it.

## Behind the scenes
- The Meter Lab uses Tau Alpha's own meter viewer, unchanged.
- Omega checks that the meters it shows are the ones Tau Alpha published.
- Presets and names are saved the way Tau's own tools save them.
- More automatic checks, all passing.

## Tested on a real Pocket
- **Presets:** presets written by Omega appeared on the Pocket, a custom preset was still chosen after a restart, and when the chosen preset was removed, the Pocket went back to the standard one.
- **Names:** a music library written by Omega loaded with no question marks, and the Pocket accepted the extra file with the original names.
- The Meter Lab screen itself runs on your computer. It has been tested there, not on a Pocket.

## Good to know
- The meter viewer is a pre-release version from Tau Alpha. The final one arrives with Tau's next release.
- Choosing which meters are on your Pocket is a preview for now.
- Japanese, Chinese and Korean titles appear as letters on the Pocket, because its screen cannot draw those characters. Some older Japanese tags may not be recognised.
- Removing older Tau cores during an update has only been tried with made-up test updates, because no real Tau release asks for it yet.
- The installers are not signed. On macOS, choose Open Anyway the first time. Windows may show a warning.
- Video is planned, and is switched off in this release.
