# Tau Omega 0.6.1

This release makes it possible to install **Tau 0.7.5** with Tau Omega (0.6.0 could not), and adds a better Marketplace page for every core and a real look at the meters on your Pocket.

## Fixes
- **Tau 0.7.5 can be installed.** Tau Omega 0.6.0 refused it with "Update Tau Omega first", because the new Tau saves its settings in a new way that 0.6.0 did not know. This release knows it, so Tau Omega's update bar and the Marketplace can install Tau 0.7.5 normally. Tau Omega still never changes your Pocket's saved settings.
- **Update notices only for what you use.** If you only have the normal Tau release, you are no longer told about a preview. A notice appears only for a kind of Tau that is on your card.
- **A broken settings file is explained.** If the file that holds your meters' saved settings is the wrong size, the Pocket stops with an error and Tau will not start. Tau Omega now says so, shows which file it is, and offers to delete it. Before it deletes anything it keeps a checked copy outside your card, and it never touches a file that works.
- **Building a core could occasionally fail** when two builds started at the same instant. Fixed.

## New and improved features

### The page for each core in the Marketplace
- **Install is the first thing you see**, next to the core's name, and there is a round button to close the page.
- **Under the name:** the developer with their icon, then the kind of device, who made the original and the year.
- **Two tabs.** *About* says what the core is, in its author's words from GitHub, with the newest version, **how many times it has been downloaded**, its licence, what is new in the newest release, and a **real link** to its GitHub page. *Details* has the requirements, the files it needs on your card, the other places that list it, and its earlier versions.
- **A strip of pictures** from the author's page sits under the core's picture. Click one to see it large, and move through them with the arrow keys.

### Meters on the Pocket
- **See your real meters.** The "Meters on the Pocket" page now shows the meters on your card, in the order the Pocket shows them. It tells you which list each Tau uses (your own meter file, the meters that come with it, or only the built-in one), which meters are hidden, which are greyed out because they need a newer Tau, and which cannot remember tuned settings.
- **It only reads.** Nothing on that page changes your card. Choosing which meters go on the Pocket is not available yet.

### Settings
- **Reads the new Tau settings.** The saved-settings viewer understands Tau 0.7.5's settings as well as the older kinds, and shows the selected meter by name.

## Behind the scenes
- Tau Omega reads the meter files and settings that Tau 0.7.5 uses, checked against Tau Alpha's own test files (all agree).
- The update to Tau 0.7.5 was tried with Tau's real release files on practice cards, with every kind of settings file: your saved settings stay exactly as they were.
- Many more automatic checks, all passing.

## Tested on a real Pocket
- Nothing new in this release was run on a real Pocket. The new screens were tested on a computer, and the Tau 0.7.5 update was tested on practice cards with the real release files.

## Good to know
- **Send feedback** is built but switched off until its service is set up, so it is not in the menu yet.
- Choosing which meters are on your Pocket, and installing meters, are not available yet.
- The installers are not signed. On macOS, choose Open Anyway the first time. Windows may show a warning.
- The update from 0.6.0 to 0.6.1 inside the app has not been tried end to end yet.
