# Merging the Cards page into Cores

Branch `cores-section`. Status: **decided and built 2026-10-15** (steps 1-5). Owner decisions: D1 all Tau cores under one TAU row (a system, like all GBA cores under `gba`); D2 Overview + History tabs; D3 the **openFPGA Library's own grouping, category > manufacturer > platform > core** (not the card's platform files, which sometimes put a manufacturer in the category); D4 "Set as player" dropped, the Library's target core is chosen from cores whose slots take mp3/flac (library cores first), any other core by hand; D5 the page is named **Card**; D6 overview extras (capacity, counts) after the merge. The body below is the plan as reviewed; where it says Media Players / card categories, the built version uses the Library snapshot (`crates/tau-core/src/taxonomy.rs`, `data/library_platforms.json`) with the card's file only for platforms the Library does not know (Tau).

## 1. What overlaps today

| Card page (`CardsView.svelte`, state in `App.svelte`) | Cores section (`CoresView`, `NavTree`, tabs, inspector) |
|---|---|
| Known cards quick open, ejected banner, "Open folder" | not there (and should stay on the card side) |
| Core updates list (`CoreUpdates`) | not there |
| "Player cores" big cards: platform art, author icon, tracks, Library ready / Legacy, Open library, Details dialog (with `RefreshLibrary`) | the same cores again, richer (files, saves, screenshots, settings, inspector) but **no library facts** |
| "Other cores" simple list + "Set as player" | the same cores again, grouped by platform |
| Details dialog (`detailCore`: id, developer, version, platform, category, library, index, tracks) | the inspector (everything but library/index/tracks) |
| Sync history is a separate page under Tools & settings | not there |

So the simple lists and the Details dialog are the redundant part; the unique parts are the card-level things (known cards, updates, history) and the **library facts of media-player cores** (tracks, index status, Open library, Refresh library).

Facts checked on the real card (`/Volumes/Pock`, read-only):
- 38 cores on 33 platforms. `Platforms/*.json` carries a `category`: Arcade 27, Console 12, Handheld 11, Computer 4, Sega 4, Nintendo 3, Others 2, **Media Players 5**.
- Tau's cores are already in the Cores tree, but spread over **five platform rows** (`TAU`, `TAU Dev`, `TAU DEV 105`, `TAU DEV 107`, `TAU Preview`), all category *Media Players*. The Preview and Dev cores list two platforms (`[tau_preview, tau]`) and read TAU's media in place, so their real media platform is `tau`. The Core record already has `media_platform` and `library_capable` for this.
- The category names are not all the same kind of word: Nintendo and Sega are manufacturers, the rest are device types. They come from the platform files (the community's data), not from us.

## 2. Target shape

Sidebar (top to bottom): **Card** · **Cores** (tree) · Library · Playlists · Meter Lab · Appearance · Halcyon EQ · Marketplace · Tools & settings (Sync history leaves this group).

- **Card** (page with two tabs, *Overview* and *History*). Overview = what is not about one core: known cards and quick open, ejected/reconnect banner, Open folder, capacity, counts, "needs attention" (missing files, updates, stale indexes), core updates (`CoreUpdates`). History = today's Sync history page, unchanged, as a tab. The player-core grid, the other-cores list and the Details dialog are **removed**.
- **Cores**: the tree gains a top level: **Category -> Platform -> Core** (for example Arcade > Galaga > core). The overview page shows the categories as sections of platform cards; the platform page and the core page (Files / Saves / Screenshots / Settings + inspector) stay as built.
- **Media-player cores** get their library facts in the same place as every core: a *Library* block at the top of the inspector (library ready or legacy, index status, tracks) with **Open library** and **Refresh library** (the existing `RefreshLibrary`), shown only when the core is library-capable.

## 3. Category grouping (the missing top level)

- Data: `PlatformInfo.category` already exists in the engine and in the tree payload; no new card reading. Group `PlatformNode`s by `platform.category` in the UI store (`coresStore.ts`), blank or missing category -> **Other**, the `_none` platform stays last.
- Order: fixed preference (Console, Handheld, Arcade, Computer, then the rest alphabetically, Media Players, Other last) so it is stable between cards; unknown category names are sorted in alphabetically.
- Names are shown as the platform files give them. Optional later (owner decision D3): a small normalisation table so Nintendo/Sega fold into Console/Handheld. Not recommended now: it would be our opinion overriding the data and would hide what the card really says.
- Tree: category rows are non-selectable-by-default group headers with a count (selecting one opens the overview scrolled to it); expand/collapse state persisted per category like today's per-platform state; the ARIA tree gets a third level (`aria-level`), arrow-key model extended (Right opens, Left goes to the parent).
- CLI/JSON: `tau cores` groups by category and the JSON gains the category on each node (already present on `platform`), so Omega and scripts see the same hierarchy.

## 4. How to show Tau

Tau belongs to its own defined category, **Media Players**, and is shown there like any other core, with the media-player extras (section 2). The remaining question is the five platform rows (D1):

- **A. As is**: five rows under Media Players. Honest but noisy, and it grows with every dev build.
- **B. Fold by media platform (recommended)**: inside a category, a core is placed under its **media platform** when it has one (`media_platform`, where it reads its library), else under its first platform. Tau, Tau Preview, Tau Preview Diagnostics, Dev cores all land under one **TAU** row; the core page shows which platform file each one declares. Other cores are unaffected (their media platform is empty or their own).
- **C. Flat**: Media Players lists cores directly with no platform level. Simple, but inconsistent with the other categories.

Whatever is chosen, the media-player test stays what it is today (`platform_category == 'Media Players'` or a manual override), so the Library page keeps finding its active core; the manual "Set as player" becomes a small switch in the inspector for a core that is not in that category (D4: keep, or drop it because the category now decides).

## 5. Where each existing feature goes

| Today | After |
|---|---|
| Known cards, quick open, ejected banner, Open folder | Card > Overview (unchanged); the sidebar card switcher stays, its "Manage cards & cores" link opens Card |
| `CoreUpdates` | Card > Overview; plus an "update available" dot on the core's tree row and inspector (data already in `coreUpdateCount`/the update check) |
| Player cards: art, author icon, name | Core page header and inspector (already there); category/platform art on the overview cards |
| Tracks, Library ready/Legacy, index status | Inspector "Library" block |
| Open library | Inspector button and the core page header action; keeps calling `openCoreLibrary` |
| `RefreshLibrary` (in the Details dialog) | Inspector "Library" block |
| Other cores list, "Set as player" | removed; manual-player switch per D4 |
| Details dialog (`detailCore`) | removed; the inspector replaces it (no data lost: id, developer, version, platform, category come from `CoreDetail`) |
| Sync history page and `openHistory(id)` callers | Card > History; `openHistory` routes to it |

## 6. Build order (each step is a commit; `make`-style checks after each)

1. **Category tree** (UI + store only): group by category, third tree level, overview sections, persisted expand state, keyboard/ARIA, mock data with categories. Tests: tree structure, keyboard, sections, no page errors, phone width. No engine change.
2. **Tau fold** (decision D1): grouping key = media platform. Pure function in `coresStore.ts` with unit-style Playwright coverage and a check against the real card (read-only: 38 cores, Tau cores under one row).
3. **Library block in the inspector**: tracks, index status, Open library, Refresh library for library-capable cores; needs the `Core` library fields next to `CoreDetail` (read from the same `inspect_card` result already in `App.svelte`, or one new field on `CoreRow`); engine test for the field, Playwright for the block and the buttons.
4. **Card page = Overview + History**: tabs, move `HistoryView` in, route `openHistory`, remove Sync history from the sidebar; add the overview items that exist already (capacity via `checkStorageCapacity`, counts, attention list). Tests updated: the nine Cards-page assertions in `workbench-test.cjs` move to the new page.
5. **Remove the redundant parts**: player grid, other-cores list, Details dialog and its state in `App.svelte` (`detailCore`, `showOtherCores`, `otherCores`), the now-unused CSS; `tau cores` CLI grouped by category; docs (`CORES_SECTION_SPEC`, `ROADMAP`, handoff, release notes).
6. Test build to `releases/dev-builds/` after steps 1-2 and again after step 5, so the tree can be tried before the Card page changes.

Verification each time: `cargo test --workspace`, clippy strict, `svelte-check`, the three Playwright suites (workbench, marketplace, cores), a read-only pass on `/Volumes/Pock`, and a mutation check that the Tau fold fails when the grouping key is changed back.

## 7. Risks

- `activeCore` and `playerCores` in `App.svelte` drive the Library page; the removal must keep them (they come from `cores`, not from the Cards page).
- Other screens deep-link to the Cards page or the history (`SyncToast`, `UpdateBanner`, settings `done`): all links are listed and redirected in step 4.
- A card with no `Platforms/` files has no categories: everything falls under Other, which must still look sensible (covered by a test card).
- Three tree levels on a phone-width layout and with 33+ platforms: the filter box must search across all levels (matches open their parents), as it does today for two.
- Categories come from files we do not control; a platform with a new or misspelled category simply becomes its own group (shown as written).

## 8. Decisions for the owner

- **D1 Tau rows**: A as is, **B fold under one TAU row (recommended)**, or C flat.
- **D2 Card page**: Overview + History tabs as above (recommended), or History stays a separate page and only the lists go.
- **D3 Category names**: show as the platform files say (recommended), or fold Nintendo/Sega into Console/Handheld with our own table.
- **D4 "Set as player"**: keep a small switch in the inspector, or drop it now that the category decides (recommended: drop, add back only if someone needs it).
- **D5 Name of the page**: "Card" (one page, one open card, with the known-cards list inside) or keep "Cards".
- **D6 Overview extras** (capacity, counts, attention list): include in step 4, or do the merge first and add them after (recommended: after).
