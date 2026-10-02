# Card space and sync bar: design spec

Status: **built** (first in test build `0.4.0-alpha.2`; see "As built" at the end for where it differs from this design). Not yet run on a real card. Extends the existing dark workbench system (`ui/src/lib/Workbench.svelte`,
the `.wb-cap` bar) the same way [`FIRMWARE_UPDATE_SPEC.md`](FIRMWARE_UPDATE_SPEC.md) extended it: same tokens, same
dialog helper, plain-language copy. Written with the design-system "extend" method.

## Problem

Today the top bar says how full the card is and, once something is staged, shows `+ N queued` / `- N removing`.
Four gaps (from the owner's review of `0.4.0-alpha.1`):

1. **The card is one grey lump.** It cannot say how much of the card is Tau music, how that splits between cores, or how
   much is everything else (other cores, saves, ROMs, screenshots).
2. **The projected change is easy to miss.** The numbers are text next to the bar; the bar's own change segments
   are thin and unlabelled, and nothing says where the change lands (which core grows or shrinks).
3. **Sync is a separate place.** The total effect of what is staged lives in the tray and, exactly, only inside the
   review sheet after "Start sync".
4. **No place for detail before committing.** The only detailed view of what will be synced is the modal that opens after
   Start sync; there is no dismissable look at it while still deciding.

## Existing patterns

| Related piece | What it gives us | Why it is not enough |
|---|---|---|
| `.wb-cap` bar (`k-used`, `k-rm`, `k-add`) | used, queued and removing as one stacked bar, over-capacity state, Pocket track limit | one "used" colour, no per-core or other-data split, text-only legend |
| Pending changes tray | the list of staged albums and the Start sync button | holds the action but shows no space effect |
| Review sheet (`dialog = 'review'`) | exact counts from the real plan, slow-connection warning, free space after | only reachable after Start sync, and it is a plan (reads and hashes files), not a cheap preview |
| `modal` action in `a11y.ts` | focus in, Tab trap, focus back on close | reused as is by the new panel |
| Colour tokens in use | `#161f20` card, `#2c393a` border, `#232e30` track, `#5b7f8f` used, `#c1f0ad` add, hatched `#7a4a3d` remove, `#f0d59a` warning, `#ff9d8a` error | the new segments reuse these; one small cool-tone ramp is added for cores |

## Proposed design

### Anatomy

```
 8.8 GB on Pocket · 58 of 16,384 tracks (Pocket limit)        117.6 GB free  →  117.2 GB free after sync
 [ TAU 8.7 | TAU Diagnostic 0.1 |  Other data 0.3 | + added  | ······················ free ······················ ]
 ■ TAU 8.7 GB   ■ TAU Diagnostic 0.1 GB   ■ Other data 0.3 GB   ▨ Removing 0.1 GB   ■ Adding 0.4 GB      [Details]  [Start sync]
```

One card, three rows, always in this order:

1. **Headline row.** Left: what is on the card now plus the Pocket's track limit (unchanged behaviour). Right: free space now,
   and when anything is staged an arrow and the **free space after sync**. Over capacity turns the right side into the
   existing red "N too much for this card" message.
2. **The bar.** One stacked bar, widths proportional to the card's total size, left to right:
   `core media segments` | `other data` | `adding` | `free`. Removal is drawn **inside** the segment it comes from, as a hatched
   piece at that segment's right end, so the bar shows both what stays and what goes.
3. **Legend and actions.** A text legend with exact sizes (every segment, so nothing depends on colour or on a sliver being
   wide enough to see), then **Details** and the primary **Start sync** button on the right. This is the merge: the space
   projection and the sync action live in one card, and the bottom tray stops carrying Start sync.

### Segments

| Segment | Source | Look |
|---|---|---|
| **Core media**, one per Tau platform folder | size on disk of `Assets/<platform>/` (media plus instance files), rounded to the card's allocation unit. Cores that share a platform (for example `TAU` and `TAU_DIAGNOSTIC` share `tau`) are **one segment** labelled with both names and "(shared media)" | cool ramp `#5b7f8f`, `#6f9f9a`, `#7f8fbf`, `#9a86b8` (repeats after four, legend disambiguates) |
| **Other data** | used space (`total - available`) minus the Tau segments: other cores, saves, screenshots, system files, anything not Tau | `#3d4b4d`, no hatch |
| **Removing** | bytes staged for removal. **Always stacked at the end of the used part, just before Adding** (2026-10-02, owner), so what goes and what comes read as one stack; the bytes still come out of the active core's segment, which draws at its size after the removal | existing hatch (`#7a4a3d`/`#5b3a30`) |
| **Adding** | bytes staged to add, immediately after the used part | mint `#c1f0ad`; red `#ff9d8a` if it does not fit |
| **Free** | the rest | track `#232e30` |

A "Tau core" is a core whose `data.json` declares the library slot (`tau-library.tdb`) or whose id starts with `alfatreze.TAU`. This
is the rule `inspect_card` already uses for "library capable" plus the family prefix; it avoids guessing from names alone. Cores
that are neither count as Other data.

**Where a change lands.** The staged change belongs to the **active core** (the one the Library is working with, shown in the sidebar). Its
segment gets the hatched removal and the adding segment follows the used part. If the active core is not a Tau core, the
adding segment simply follows Other data.

### How the projection is computed (and why it is only an estimate here)

- Adding and removing bytes come from the album sizes already listed (no file is opened), plus half an allocation unit per track as
  the expected cluster waste, so the estimate tracks what the card will actually use. It is labelled **about**.
- The **exact** figure comes only from the real plan (`plan_changes`), which reads and hashes files, so it runs when the owner
  presses **Start sync** (the existing review sheet) and never on a checkbox toggle. This follows the performance audit
  (`PERFORMANCE_AUDIT.md`): do not re-plan on every change.
- Over-capacity is decided on the estimate with the existing margin, and the engine's own preflight (`InsufficientSpace`) is the backstop.

### Details panel (dismissable side panel)

Opened by the **Details** button or by clicking the bar. A right-hand panel (about 420 px; full width under 720 px) over a dimmed page.

| Section | Content |
|---|---|
| **Summary** | "About 412 MB will be added, 56 MB removed. Free space after: 117.2 GB." Tracks and albums after sync against the Pocket limit. |
| **By core** | for the active core: before, change and after (for example `TAU  8.7 GB → 9.0 GB`); other cores unchanged |
| **Adding** | albums to add: title, track count, size; first 8, then "Show all N" |
| **Removing** | albums to remove, same shape, with the backup preference ("Back up to ~/..., then remove") |
| **Editing** | title/artist/year/cover edits queued, one line each |
| **Heads-up** | warnings that can be known without a plan (over the track or album limit, nearly full, connection is direct USB and the change is over 10 MB with the Pocket's own advice) |
| **Footer** | **Start sync** (primary, same action as the bar) and **Close** |

Closing: the X, **Esc**, clicking outside, or **Close**. Focus returns to the control that opened it (the existing `modal` helper). The
panel changes nothing and writes nothing; it is a read-only view of the staged list plus the same Start sync.

### API (component)

`CapacityBar.svelte` (new, extracted from `Workbench.svelte`):

| Property | Type | Description |
|---|---|---|
| `space` | `{ total_bytes, available_bytes } \| null` | from `checkStorageCapacity` (fast, already loaded) |
| `breakdown` | `CardBreakdown \| null` | from the new `card_breakdown` command; `null` while measuring |
| `activePlatform` | `string` | which segment the staged change belongs to |
| `addBytes`, `removeBytes` | `number` | estimated on-disk bytes (see above) |
| `tracksAfter`, `limits` | numbers | existing Pocket library limits |
| `canStart`, `startReason` | `boolean`, `string` | Start sync enablement and why not (existing `canStart` logic) |
| `onDetails`, `onStart` | callbacks | open the panel; start the sync |

`DetailsPanel.svelte` (new): `open`, `adds`, `removes`, `edits`, `coreBefore/After`, `warnings`, `onClose`, `onStart`.

New engine command `card_breakdown(card) -> CardBreakdown { total, available, unit, cores: [{ id(s), label, platform, bytes_on_disk, files, shared_with }], tau_bytes, other_bytes }`.
Stat-walk only (names and sizes, no file contents), runs off the main thread, cancellable, cached per card until the card is refreshed or a sync finishes.

### States

| State | Behaviour |
|---|---|
| Measuring (breakdown not ready) | total and free shown at once from the fast query; core and other segments show a shimmer with "Measuring what is on the card…" (over USB this can take a minute, so it says so and never blocks anything) |
| Nothing staged | no adding or removing segments; headline right side shows free space only; **Start sync** disabled with its reason |
| Staged add | mint segment after the used part; right side shows "free → free after"; legend adds `Adding about 412 MB` |
| Staged removal | hatched piece inside the active core's segment; legend adds `Removing about 56 MB` |
| Add and remove | both; free after = free + removed - added |
| Over capacity | adding segment turns red; right side red "N too much for this card"; Start sync disabled with the reason |
| No Tau core on the card | no core segments; everything used is Other data |
| Shared platform | one segment for the platform, legend names every core that uses it |
| Very small segment | at least 2 px wide so it can be seen; the legend always has the exact size |
| Card ejected or disconnected | bar dims and shows the existing disconnected or "safely ejected" message; actions disabled |
| Estimate vs exact | the panel and bar say "about"; the review sheet after Start sync shows the exact plan numbers |

### Tokens

Colours: the existing ones listed above; the only additions are the four-step cool ramp for cores and `#3d4b4d` for Other data. Spacing and radius: the
existing card (`border-radius:12px`, `padding:14px 16px`), bar height raised from 12 px to 14 px so segment separators (1 px of the card colour) read. Type: the existing 13 px
secondary text for the headline and legend, 12 px for legend sizes.

### Accessibility

- The bar is a `role="img"` with a full text label that states every number ("8.8 GB used: TAU 8.7, TAU Diagnostic 0.1, other 0.3. About 412 MB will be added. About 117.2 GB free after sync."), updated politely (debounced) when the staged list changes.
- **Nothing is colour only.** The legend is real text with sizes; removal is hatched, not just red; adding is a different fill and also named in text; over capacity is stated in words.
- Contrast: legend and headline text keep the current AA colours; segment separators give adjacent fills a visible edge (the 3:1 non-text contrast rule is met by the separator against the card, not by comparing neighbouring fills).
- The panel is `role="dialog" aria-modal="true"` with a label, focus moves in on open, Tab is trapped, Esc and the X close it, focus returns to **Details**; reduced-motion users get no slide animation.
- Keyboard: **Details** and **Start sync** are normal buttons in the tab order after the legend; clicking the bar is a convenience, not the only way in.

### Copy

| Where | Text |
|---|---|
| Measuring | "Measuring what is on the card…" |
| Headline right (staged) | "117.6 GB free, 117.2 GB after sync" |
| Over capacity | "About 2.1 GB too much for this card" |
| Legend add / remove | "Adding about 412 MB" / "Removing about 56 MB" |
| Other data | "Other data (other cores, saves, screenshots, system)" |
| Shared | "TAU and TAU Diagnostic (shared media)" |
| Panel title | "What will change" |
| Panel footnote | "These sizes are estimates. Press Start sync to check every file exactly before anything is written." |

## Build plan

1. **Engine.** `card_breakdown` in `tau-core` (portable, std only) with tests on a fake card: shared platform, other data, cluster rounding, symlinks not followed, missing folders. Tauri command, off the main thread.
2. **`CapacityBar.svelte`.** Segments, projection, legend, states; replaces the `.wb-cap` markup; keeps the existing aria group label so current checks still find it.
3. **`DetailsPanel.svelte`.** Reuses `modal`; reads the staged list; no new backend calls.
4. **Merge.** Move **Start sync** into the bar (same label, so existing checks and docs still match); the tray keeps the list, Clear all and a "See details" link.
5. **Checks.** Mock backend returns a breakdown; new browser checks for the segments, the projection arithmetic, the panel's keyboard behaviour and the over-capacity state; engine tests above.

## As built (differences from the design above)

| Design said | Built | Why |
|---|---|---|
| **Start sync** moves into the bar and leaves the tray | **Built as designed (moved to the bar, 2026-10-02, at the owner's request).** It first stayed in the tray; the owner asked for it at the top. The tray keeps Clear all and the status notes; the panel has its own Start sync | the usability round had asked that keyboard users meet the pending items before Start sync. That is now answered another way: the button's description reads out what it will review ("Opens the review of: 2 albums to add, 1 to remove. Nothing is written until you confirm"), and it only opens the review sheet, which still lists everything and needs a second confirmation before anything is written |
| clicking the bar also opens the panel | only the **Details** button | a click handler on a non-interactive bar is a keyboard trap for no gain |
| estimate adds half an allocation unit per track | estimate uses logical album sizes | the waste is tens of MB on a 100 GB card; the engine's own preflight (`InsufficientSpace`) uses exact on-disk sizes at Start sync |
| `card_breakdown` finds Tau cores itself | the app passes `{id, shortname, platform, library_capable}` for each core | `inspect_card` reads every core's whole index; passing what the app already has avoids re-reading them over USB |
| the Pending changes area stays at the bottom | **Removed (2026-10-02, at the owner's request) to give the media lists the room.** The staged list moved into the Details panel as a plain list (no pills): one row per album with its own Remove / Undo / Discard, grouped Adding, Removing, Editing, under the summary and heads-up. The bar carries **Clear all** and the status notes (slow link, will not fit, over the limit, disconnected); the sync progress dock sits under the bar; after staging, keyboard focus goes to Start sync; the skip link is now "Skip to Start sync" | the list is for review and correction, not for watching, and the bar already shows the effect |
| Start sync opens the review sheet, then Confirm | **Start sync starts the sync** unless there is a decision to make (a removal, or the slow direct-USB warning), then the "Ready to sync" sheet appears. Covers are always on: the two cover checkboxes are gone. A cover that cannot be embedded warns (`CoverNotEmbedded`, once per album) and shows when the sync finishes | owner decision D3 in `SAFETY_RULES.md`: the staged list and the bar are the review, and a dialog that only repeats them and asks about covers is noise |
| everything else | as designed: per-platform segments (shared media is one segment), other data, hatched removal inside the active core's segment, mint adding segment, text legend, headline "free, N after sync", details panel with by-core before and after, adding, removing, editing, heads-up, estimate footnote | |

Decisions 2, 3, 4 and 5 kept their defaults. Not measured yet: how long `card_breakdown` takes on a real Pocket over USB and on a card reader (`TEST_PLAN.md`, item 5).

## Open questions (recommended default first)

1. **Where does Start sync live?** Default: in the bar, removed from the tray (this is the merge). Alternative: keep it in both places.
2. **What counts as a "Tau core"?** Default: library-capable cores plus ids starting `alfatreze.TAU`. Alternative: every core in the "Media Players" category.
3. **Does a tag-only edit show in the bar?** Default: no size effect, listed under Editing in the panel only.
4. **Panel or modal?** Default: right-hand side panel so the albums stay visible on the left. Alternative: reuse the centred dialog.
5. **Measuring over USB.** Default: measure in the background and show total and free immediately. Alternative: do not measure at all on direct USB (faster, less informative); the ledger (`PERFORMANCE_AUDIT.md`) would later make repeat measurements cheap.
