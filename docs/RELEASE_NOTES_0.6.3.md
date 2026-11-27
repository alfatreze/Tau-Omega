# Tau Omega 0.6.3

This release adds **per-game Libretro artwork and Game Details**, **Light/Dark/System themes with custom accent colors**, **reorganized tabbed Settings**, and **Cores overview filters with platform artwork banners**.

## New and improved features

### Game artwork, Grid view & Game Details
- **Automatic game artwork.** ROM lists in Core Files and Game Bub core views automatically fetch boxart, title screens, and snapshots from Libretro thumbnail repositories on GitHub, cached locally.
- **List and Grid views.** Switch between a compact list and edge-to-edge boxart cover cards (remembered between sessions), filter by game category, and turn selection checkboxes on only when you want with **Select**.
- **Artwork picker.** Right-click or choose **Change image…** in Game Details to switch between boxart, title screens, and snapshots, search Libretro repositories directly, or reset to automatic artwork.
- **Game Details sidebar.** Selecting a game opens metadata (developer, publisher, genre/category, release date, and an artwork gallery), related saves and save states, and safe file removal.

### Cores overview & management
- **Overview filters and banners.** Filter the Cores overview by **Category** chips (just like the Marketplace), **Status** (`All`, `Enabled`, `Disabled`, `Needs files`), and **Manufacturer**. Platform overview cards now show each platform's artwork banner.
- **Easier-to-spot disabled cores.** Platforms with disabled cores carry an explicit warning badge, dashed border, and dimmed/desaturated artwork on all-disabled platforms, while disabled core rows have clearer visual distinction.
- **Core Inspector improvements.** Updated Core Details in the right sidebar matches Marketplace styling with core icon show/hide, an installed-version switcher, and safe core removal with shared-ROM backup prompts. Toggle the details panel with the header icon or `I`.

### Appearance, themes & Settings
- **Light, Dark, and System themes.** Choose System (follows your OS automatically), Light, or Dark with clean, neutral slate surfaces and zero startup flash.
- **Primary color selector.** Pick from 7 contrast-checked accent presets (Sage, Teal, Blue, Indigo, Berry, Coral, Amber) or choose any custom hex color—automatically adapted for accessible AA contrast in both Light and Dark modes.
- **Tabbed Settings & Help section.** Settings are organized into **General**, **Appearance**, **Music & Sync**, **Artwork**, and **Diagnostics** tabs. Documentation and **Send feedback** now live under the **Help** section in the sidebar.
- **UI polish.** Replaced nested scrollable panes with minimal single-scrollbar scrolling, moved transient feedback to floating bottom-right toasts, deduplicated shared Music library views in the card switcher, and removed outdated `TARGET CORE` labels.

## Good to know
- The installers are not signed. On macOS, choose Open Anyway the first time. Windows may show a warning.
