# Core Marketplace and Core Updater (spec, 2026-10-14)

Branch `marketplace` (worktree `../tau-omega-marketplace`). Browse every openFPGA core for the Analogue Pocket, install one onto a card, and keep all installed cores up to date. Reuses Omega's reviewed install path; adds a catalog, a GitHub release resolver and two pages.

## 1. Facts (verified 2026-10-14 against the live service)

- **openFPGA Library API** (MIT, read-only, static JSON on GitHub Pages): `GET https://openfpga-library.github.io/analogue-pocket/api/v1/cores.json` -> `{"data":[Core]}`; the OpenAPI file is `.../api/v1/openapi.yaml`. 333 cores, 169 KB, all GitHub-hosted, 22 `requires_license`.
- `Core`: `identifier` (`author.Name`, equals the `Cores/<id>` folder), `platform` (display), `repository {platform:"github", owner, name}`, `requires_license`, `sponsor {github[], patreon, custom[]}` or null, `updaters {license{filename}, previous[{shortname, author, platform_id}]}` or null, `release {release_date, version, platform{category,name,manufacturer,year}, assets[{platform, filename, extensions[], core_specific}]}` (release may be null).
- **The API has no download URL, no checksum, no size, no description, no icon, no changelog.** The package is the newest GitHub release of `repository`: e.g. `https://github.com/agg23/openfpga-arduboy/releases/download/0.9.0/agg23.Arduboy.0.9.0.zip`, whose layout is the card root (`Cores/<id>/...`, `Assets/<platform>/common/`, optionally `Platforms/`). Resolution needs the GitHub API (`/repos/{owner}/{name}/releases/latest`): unauthenticated limit 60 requests/hour, so resolve lazily (on open of a core, or per installed core for the updater), cache, and use conditional requests (ETag). An optional personal token is a later setting, not in v1.
- `requires_license` cores need a user-supplied licence file (`updaters.license.filename`, e.g. `beta.bin`) placed in the core's folder; it must never be downloaded or invented.
- `release.assets` describes what the core expects in `Assets/<platform>/` (extensions, core-specific): useful to tell the user where their own ROMs go.
- Reuse already in Omega: `release_check` (GitHub release parsing, SHA256SUMS verification), `package::{inspect, plan_install}`, `install_plan::plan`, `install_exec::execute` (verified backup, rollback, catalog-cache clearing, `._` sweep), `update::assess`, card scan (`card::Core`: id, version, platforms), `ureq` in `src-tauri`, the Updates bar and Packages page.

## 2. Trust model (the part that needs care)

Tau releases are ours; these are arbitrary third-party bitstreams and zips. Rules:
1. **Plan then confirm, always** (SAFETY_RULES 2-3). Nothing is written to a card without a shown plan; the existing plan/backup/rollback executor is the only writer.
2. **The catalog is a hint, the zip is the truth.** Before planning, inspect the zip: it must contain exactly the expected `Cores/<identifier>/core.json` (author.shortname id matches the catalog identifier), only paths under `Cores/`, `Assets/`, `Platforms/` (anything else, absolute paths, `..`, symlinks: refuse), a size limit (default 200 MB, entry count limit), no executables outside what the Pocket reads.
3. **Integrity:** GitHub provides a per-asset SHA-256 `digest` for newer releases; use it when present and show "checksum verified by GitHub" vs "no checksum published" (the latter is allowed but labelled, never silent). Pin: the plan id covers the downloaded bytes' hash.
4. **Provenance shown:** author, repository link, release tag and date, licence requirement, sponsor links (opens in the browser; nothing is paid or sent by Omega).
5. **No auto-install, no auto-update.** The check is automatic (when the update setting is on), the change is always the user's.
6. **Overlap protection:** an update replaces `Cores/<id>` only; the user's saves, settings, `Assets/<platform>/common` media, other cores and the licence file are untouched (backup first, then replace). A catalog core whose `Assets` folder would overwrite existing user files is refused for those files.
7. Network scope: only `openfpga-library.github.io`, `api.github.com` and the release download hosts (`github.com`, `objects.githubusercontent.com`); one User-Agent, no identifiers.

## 3. Engine (`tau_core`, new modules; network stays in `src-tauri`)

| Module | Job |
|---|---|
| `catalog` | Parse `cores.json` (tolerant: null release/sponsor/updaters, unknown fields ignored), `Catalog { cores }`, search (id, name, author, platform, category), filter (category, manufacturer, year, requires_license, installed/update), sort, group by platform. Pure; tested on a captured real `cores.json` (`testdata/openfpga/cores.json`). |
| `catalog_release` | From a GitHub release JSON choose the zip for a catalog core (the asset whose name starts with the identifier, else the only `.zip`), read the SHA-256 digest, size and notes; reuse `release_check::parse_releases` types. |
| `core_version` | Compare two openFPGA versions that are not Tau's (`0.9.0`, `v1.2`, `2024.03.1`, `0.01b`, dates): SemVer when it parses, else numeric-dotted, else release date, else "different" (never "newer" on a guess). |
| `core_update` | Installed cores (from `card::inspect_card`) matched to catalog entries by identifier, then by `updaters.previous` (old ids that this core replaces: offer the replacement and migration, never auto-remove). Verdict per core: up to date / update available / newer than catalog / not in catalog / unknown. |
| `marketplace_install` | Download -> verify (size, digest) -> `package::inspect` + the trust checks above -> `install_plan::plan` -> existing executor. Licence-required cores: install without the licence, then show the exact file name and folder to put it in. |

Host (`src-tauri`): `marketplace_cmds.rs` with `catalog_fetch` (cached in the app cache, ETag, offline falls back to the cache with its age shown), `catalog_core_release` (one core), `updater_check` (installed cores x catalog, one release lookup per core with an update candidate, rate-limit aware), `marketplace_download`, plus the existing plan/execute commands. Card write lock as everywhere.

CLI parity (the project convention): `tau catalog [--search q] [--json]`, `tau catalog-core <id>`, `tau core-updates <card> [--json]`, `tau core-install-plan <id> --card <root>`; install via the existing `package-install` after a download.

## 4. UX (Design-system extension; reuses current tokens, no new colours)

Existing language: dark surface `#161f20`/`#1a2325`, border `#2c393a`, accent mint `#c1f0ad` for primary only, chips `.chip`, rows `.core-list article`, plan card `.plan-card`, bar `.update-bar`, nav buttons, focus-trapped sidebar, `VirtualList`.

**Navigation:** a new first-level item **Marketplace** (between Halcyon EQ and Tools) and **Core updates** as a section at the top of the Cards page (installed cores with a badge), plus a count chip on the nav item when updates exist. Not a modal.

**Marketplace page** (new component set `MarketplaceView`, `CoreCard`/`CoreRow`, `CoreDetail`, `FilterBar`):
- Header: title, one sentence ("Cores made by the community for the Analogue Pocket. Nothing is installed until you review it."), catalog age ("Updated 2 h ago", Refresh).
- Filter bar: search box (focus on `/`), Category chips (Console, Handheld, Computer, Arcade, Utility, ...), Platform manufacturer select, "Installed", "Needs a licence file" toggles, Sort (Newest release, Name, Platform). Chips use `.chip`; active chip uses the accent outline, not fill.
- List: virtualised rows (333 now, grows): icon placeholder (platform glyph or initials; real icons only after install, from `icon.bin` is not available pre-install), name, author · platform · year, version + release date, status chip (Installed, Update available, Needs licence), one action. No images fetched in v1 (privacy, offline, speed).
- **Detail panel** (side sheet, same pattern as `DetailsPanel`): what it is (platform, manufacturer, year, category), version and date, repository link, sponsor links, "You will need" (licence file name; where ROMs go: `Assets/<platform>/common` with the accepted extensions), the release notes (first 600 chars, plain text, link out), and the primary action **Review install** (card required: shows the card picker or "Open a card first").
- **Install flow** reuses `UpdatePlanView`: plan card with files, backup location, what is kept, the licence note, the checksum line ("Verified against GitHub's checksum" or "No checksum published for this release"), Confirm and install; result with the post-install check and Undo. Nothing new to learn.
- Empty/error states in plain words: offline with cache ("Showing the list from 3 days ago. Refresh when online."), offline without cache, GitHub rate limit ("GitHub allows 60 lookups an hour. Try again at 14:35."), no results (with the active filters named and a Clear), a core with no release ("The author has not published a download yet."), refused package (the exact reason).

**Core updates (on Cards):** per installed core a row: name, installed version, newest version and date, `Update available` / `Up to date` / `Not in the catalog` chip; header action **Check for updates** and an `Update all` that does NOT install: it builds one reviewed plan per core in sequence (each its own confirmation; one card, one lock). A migration line when `updaters.previous` matches ("This core replaces <old id>. Your old core stays until you remove it."). Tau's own cores keep using the existing Tau update bar and channels (the updater lists them but defers to that path).

**States for every interactive piece** (documented per the system): default, hover, focus-visible (existing outline), active, disabled (`.45` opacity, `cursor:not-allowed`), loading (inline `RingProgress`, label changes to a verb: "Checking…", "Downloading…"), error (red `#ff9b9b` line below the control, `role=alert`), success (mint chip, `role=status`).

**Accessibility:** list is a `role="list"` of rows with the action as a real button named "Review install, <core>"; filter chips are toggle buttons with `aria-pressed`; the detail sheet is a focus-trapped dialog (Escape closes, focus returns to the row); status changes announced with `aria-live="polite"`; colour is never the only signal (every chip has text); minimum 44 px targets in the row action; works at 200% zoom with the existing collapsing sidebar; keyboard: `/` search, arrows move through rows, Enter opens detail.

**Copy rules:** say "core" not "bitstream"; "needs a licence file" not "requires_license"; never "safe"; say what leaves the machine (one request to the public list, one to GitHub for the release you opened).

## 5. Build order (each step host-tested, plan-first, no card write without approval)

**M4 finding (2026-10-15):** Omega's update logic assumed Tau's `bitstream.rbf_r`/`tau.rom` pair, so a community core was judged wrongly (an installed generic core was always "repaired", and a newer package was reported as the same build). Fixed with `PairStatus::NotApplicable` and a version-based verdict (`core_version`, unordered versions are never guessed); proven by `tests/generic_core.rs` (new install, same build, update with the user's ROM and licence file kept, exact rollback, downgrade refused, unorderable versions). **Real-card run done 2026-10-15 (CARDWRITE, owner-approved):** the real Arduboy 0.9.0 zip (downloaded with `tau core-download`, no checksum published, vetted) planned as `same_build` (the card already had it; the only difference was an empty `.gitkeep`), installed with an outside-card backup, then undone with the new `tau update-undo`; the card matched the before-snapshot byte for byte, twice. The first run found a real defect: the post-install check failed the core because its `data.json` slot has no `id` (Arduboy omits it and runs on Pocket hardware); for community cores a missing slot id and a user-supplied required file are now warnings, Tau's own cores keep the strict check. A fresh install of a core that is not already on the card still has not been run on the real card.

**Status 2026-10-14:** M0, M1, M2 and M3 built (M3: `MarketplaceView.svelte`, `marketplace.ts`, mock backend; browser-checked at desktop and 375 px; nothing downloaded or installed against a real card yet).

- **M0** engine `catalog` + fixture + `core_version`, tests (this commit series).
- **M1** `catalog_release` + trust checks on a zip + `core_update` matching, tests on the real Arduboy zip (committed small fixture) and synthetic hostile zips (traversal, symlink, wrong id, oversize).
- **M2** host commands + CLI (`tau catalog`, `core-updates`), network layer with cache/ETag/rate-limit handling.
- **M3** Marketplace page + detail + filters (dev-mock first, browser-checked, svelte-check).
- **M3b** artwork: remote-zip reader (range requests), `icon.bin`/platform image decode, cache by release tag, "Download all artwork".
- **M4** install flow through the existing plan/executor, licence note; one real install of a small core onto `CARDWRITE` with approval.
- **M5** Core updates section, `Update all` as sequential plans, count badge, migration note; one real update on `CARDWRITE`.
- **M6** polish: offline states, keyboard pass, a11y pass, docs, roadmap entry, release notes.

## 6. Decisions (owner, 2026-10-14)

1. **Scope:** show every catalog core. The catalog carries no compatibility data; compatibility (`core.json` `version_required`, Dock support) is only known from the zip, so it appears as a line in the install plan, not as a filter.
2. **No checksum:** allowed, with a small notice ("No checksum published for this release") in the plan and on the detail panel.
3. **GitHub token:** not in v1 (lookups are lazy and cached; 60/hour is enough). Revisit only if rate limits bite.
4. **Tau's own cores:** skipped for now; a shared approach with the existing Tau update path is a later item.
5. **Icons and shots:** bring them, and download again only when a core's release changes. Sources: the catalog has no images and no screenshots anywhere; the only art is inside each release zip (`Cores/<id>/icon.bin`, `Platforms/_images/<platform>.bin`). Plan (M3b): read just those entries from the remote zip with HTTP range requests (central directory at the end, then the two entries), decode the Pocket image format, cache by `(core id, release tag)` in the app cache, fetch lazily for visible rows and on an explicit "Download all artwork" action, never block the list on it. If a host does not support range requests, fall back to the whole zip only when the user opens that core. No third-party screenshot scraping (READMEs are untrusted, vary, and would add a second network scope).
