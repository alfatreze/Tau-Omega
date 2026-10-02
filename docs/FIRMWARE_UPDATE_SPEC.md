# Firmware/app update feature: UX/UI design

Produced with the `design-system` skill's "Extend" method (new pattern that fits the existing
system), against Tau Omega's real dark-workbench visual language and component set
(`.settings-card`, `.picker-row`, plan → review → confirm, `.difference-list`, `size()`), not a
new visual language. Scope confirmed with the owner (2026-09-26): **two targets** — Tau Omega
self-update, and Tau Alpha core firmware install/update — and release data comes from **GitHub's
releases API** (a first for this app: every prior feature has been fully local/offline).

## 1. Problem

Today, installing a core update means: know a release exists, download the right zip yourself,
remember which of the two zips is which (normal vs Diagnostic Build), find its version number,
and point the existing Packages screen at the file on disk. There is no way to see what version is
*currently on a given card*, no way to browse history (older stable releases, or alpha
pre-releases), and no channel/build-type picker. Tau Omega itself has no way to tell you a newer
version exists at all.

Separately: every write this app does (Sync, Backup, Package install, Core copy/move) assumes a
destination volume behaves like a normal, fast disk. Analogue's own documentation
(`analogue-pocket-dev` skill, `docs-snapshot/debugging-aids.txt` and `getting-started.txt`) says the
opposite for one real, common case: **when the Pocket itself is connected via USB-C and its SD card
is mounted through it ("USB SD Access"), transfer speed is ~700 KB/s–1 MB/s, and Analogue explicitly
recommends against large file transfers over it.** A user who doesn't know they're on that path
could sit through a multi-minute write with no explanation, or worse, disconnect impatiently mid-write.

## 2. Existing patterns and why they're not enough

| Existing pattern | Reused as-is | What's missing |
|---|---|---|
| `PackageView.svelte` (inspect zip → plan → confirm install/remove) | The whole write path: `package::plan_install`/`execute_install`, hash-verify-before-write | No version browsing, no per-card "what's installed now", no build/channel picker — needs a zip already on disk |
| `CoreView`/`inspect_card` (`version`, `id`, `platform` per installed core) | Per-card detection is already free — `Core.version` is read today, just not compared against anything | No "is this current" comparison, no fetched release list to compare against |
| Plan-review capacity line (Sync/Backup already show a fits/doesn't-fit line from `storage::check_capacity`) | The exact visual slot and pattern for a second advisory line next to a plan | No connection-type awareness at all today |
| Known-card tiles / sidebar widget (`CardsView.svelte`, generic device-icon SVG) | The tile/badge layout | Icon is one fixed generic glyph regardless of what's actually connected (documented as "a deliberate placeholder" when built) |

## 3. Proposed design

### 3a. Tau Omega self-update — Settings screen, new "Updates" card

A read-only check, not an auto-updater. Matches the project's own precedent: everything here is
"tell the user the truth and let them act," never a silent background mutation.

```
┌─ Tau Omega ───────────────────────────────────────────┐
│ You have          0.3.0                                │
│ Latest available  0.4.0  (2026-09-30)          [Check] │
│                                                         │
│ ⓘ A new version is available.                          │
│   [ View release notes ↗ ]   [ Download ↗ ]            │
└─────────────────────────────────────────────────────────┘
```

- **States:** *Not checked* (button only) → *Checking…* → *Up to date* (quiet, no chip) →
  *Update available* (the two link buttons appear) → *Check failed* (network error, English
  message, same `errorMessage()` convention as every other command).
- **No in-place replace.** `GET https://api.github.com/repos/alfatreze/Tau-Omega/releases/latest`,
  compare `tag_name` against the build's own version (already in `Cargo.toml`/`tauri.conf.json`).
  "Download" opens the release page in the system browser (Tauri's `opener` plugin or a plain
  `<a target="_blank">` via the shell-open capability) — **not** a downloaded/installed binary.
  Self-replacing a running signed app is `tauri-plugin-updater` territory: real code-signing and
  update-manifest infrastructure this project doesn't have yet. Flagged as a future enhancement,
  not built now.
- **Never automatic.** Checked only on an explicit button press — no background polling, no
  network call at app launch. Matches "Local only" being the app's own stated identity; the one
  new network capability this feature adds should be as opt-in and visible as possible.

### 3b. Tau Alpha core firmware — new "Firmware" screen, replacing the zip-picker half of Packages

Packages keeps its "install from a zip I already have" flow (still useful for a locally-built dev
core, e.g. `TAU DEV NN` test builds that never reach GitHub). The new flow is additive: pick a
release instead of a file.

```
┌─ Firmware ──────────────────────────────────────────────────────┐
│ Card: /Volumes/Pocket   [Choose]                                 │
│                                                                   │
│ On this card today                                               │
│  ● alfatreze.TAU              v0.4.0                             │
│  ● alfatreze.TAU_DIAGNOSTIC    v0.4.0                             │
│                                                                   │
│ Channel  ( Stable ▾ )     Build  ( Normal ▾ )                    │
│                                                                   │
│ Available versions                                    [Refresh] │
│  ○ v0.5.0-alpha.27   2026-09-26   pre-release      [Install]     │
│  ● v0.4.0            2026-09-22   current on card                │
│  ○ v0.3.0            2026-09-21                    [Install]     │
└───────────────────────────────────────────────────────────────────┘
```

- **Per-card detection (the explicit requirement):** re-inspects the *currently chosen* card only,
  the same `inspect_card` call every other screen already makes — no cross-card memory, no
  "last known version" cached from a different card. Re-runs on every card choice/refresh, same as
  `CardsView`'s own `openFolder`.
- **Channel filter:** Stable (GitHub `prerelease: false`) / Alpha (`prerelease: true`) / All. Default
  Stable — matches this app's own "boring by default" posture (Sync's own `mirror`/`embed_covers`
  default off is the same instinct).
- **Build picker:** Normal vs Diagnostic Build — maps directly to picking one of the release's two
  published assets (`alfatreze.TAU_X.Y.Z_DATE.zip` vs `alfatreze.TAU_DIAGNOSTIC_X.Y.Z_DATE.zip`,
  the two-zip-per-release convention already established on the tau-alpha side). Selecting a row's
  "Install" downloads the chosen asset, verifies it against that release's own `SHA256SUMS.txt`
  (published alongside the zips today), then hands off to the **existing** `package::plan_install`
  → review → confirm flow unchanged — this screen only replaces "where the zip comes from," not
  the install mechanism itself.
- **Version comparison:** plain semver compare between the card's installed `core.json` version and
  each release's tag; the row for the installed version is marked "current on card" and its own
  Install button is replaced with a disabled state (nothing to install over itself). A version
  *older* than what's installed still shows a working "Install" button — reverting to a previous
  release is a legitimate, supported action here (explicitly asked for: "previous versions"), not
  hidden as a mistake.
- **Empty/error states:** no network → "Could not reach GitHub. Check your connection." with a
  Retry button, never a silent empty list; no releases published yet → explained plainly.

### 3c. Connection-type detection + transfer-safety warning (cross-cutting)

One new shared component, `TransferSafetyBanner`, dropped into every existing plan-review card that
already shows a byte count: Sync, Backup, Package install (including the new Firmware install path,
since it reuses the same `execute_install`), Core copy/move. Same visual slot the capacity-check line
already occupies (a second advisory line under the plan summary), not a new pattern.

```
┌─ READY FOR CONFIRMATION ───────────────────────────────┐
│ sync-plan-abc123                                       │
│ 5 new · 0 updated · 40.1 MB to write                    │
│ ⚠ This card is connected directly to your Pocket over  │
│   USB. Transfers here run at ~1 MB/s — this could take │
│   about 40 seconds. Removing the SD card and using a   │
│   card reader is much faster for transfers this size.  │
│                            [ Confirm and sync anyway ]  │
└─────────────────────────────────────────────────────────┘
```

- **Threshold:** warn at **≥10 MB** — not an estimate. The Pocket's own on-device screen, shown when
  USB SD Access is enabled, states this directly: "SD card mounted through USB / <10MB suggested /
  Not for large transfers" (owner-confirmed 2026-09-26, reading the device's own UI with it mounted).
  This supersedes the ~20 MB figure this spec originally derived from the ~700 KB/s–1 MB/s rate in
  Analogue's written docs — the device's own stated guidance is the more authoritative number, and
  the two agree closely enough (10 MB at ~1 MB/s is a ~10 second wait) that this isn't a contradiction,
  just a firmer source.
- **Never blocks.** The banner never disables the confirm button — same "warn, don't withhold"
  posture as the plan-review "does not fit" capacity line, which also just colors and labels rather
  than disabling. The button label itself changes to "…anyway" only when the banner is showing, so
  the acknowledgement is legible without a separate checkbox.
- **Below threshold, or a plain SD-card-reader mount, or connection type unknown: no banner at all.**
  Silence is the correct state for the common case; this app already doesn't editorialize about
  things that are fine.

**Detection is a real open technical question, not a design one — see section 5.**

### 3d. Card icon: Pocket vs SD card vs unknown

The existing device-icon glyph (`CardsView.svelte`'s known-card tiles, the sidebar active-card
widget) is one fixed generic "card" SVG today, built as an explicit placeholder ("no Analogue
Pocket icon or logo asset was available to embed... reproducing Analogue's actual trademarked logo
... isn't something to fabricate," per `STATUS_HANDOFF.md`). That constraint is unchanged — this is
about which *generic* glyph shows, not adding Analogue's real logo:

- **Detected as the Pocket itself (USB SD Access):** a small handheld-device outline (the existing
  glyph already looks like this — keep it for this case, since it was originally drawn to suggest a
  Pocket).
- **Detected as a plain SD/microSD card (any other mounted volume, or a staging folder):** a new,
  distinct SD-card outline glyph (simple rectangle-with-notched-corner, the universal SD-card
  pictogram — genuinely generic, no trademark concern).
- **Unknown/undetermined:** keep today's existing generic glyph as the fallback, unchanged.

Same badge slot the known-card tile already uses for "Available now"/"Recently used" — add the icon
swap there and in the sidebar widget, no new layout.

## 4. Architecture and open decisions

- **New capability: outbound network access**, to `api.github.com` only (`releases` endpoints for
  both repos) and to each release's own asset/`SHA256SUMS.txt` download URLs. This is genuinely new
  for this app — everything else has been local-only — and should be its own reviewed dependency
  entry (`DEPENDENCIES.md`) once built: likely Tauri's own `http` plugin (already sandboxed by
  Tauri's capability system) rather than adding a Rust HTTP client crate directly, keeping the
  "no dependency pulls in a heavy transitive graph" discipline `tau-core` already holds itself to —
  though the actual HTTP call belongs in `src-tauri`, not `tau-core`, the same "front-end does
  presentation and I/O, the engine does domain logic" split every other feature already follows.
  Downloaded zips are verified against the release's own published `SHA256SUMS.txt` before ever being
  handed to `package::plan_install`, matching the "verify against a real artifact, never trust a
  network response blindly" discipline the rest of this app already lives by.
- **Caching:** cache a release-list fetch for a short period (e.g. the session, or a few minutes) so
  clicking between the Firmware screen and elsewhere doesn't re-hit the GitHub API on every visit —
  GitHub's unauthenticated rate limit is real (60 requests/hour per IP) and worth respecting from the
  first version of this feature, not added later after someone hits it.
- **Version compare:** a small semver-ish comparator good enough for this project's own tag shapes
  (`vX.Y.Z` and `vX.Y.Z-alpha.N`), not a general semver dependency — same "hand-write it, it's not
  worth a crate" instinct the index codec and tag parsing already follow.

## 5. Detecting Pocket-via-USB from a plain SD reader — resolved, empirically confirmed

Analogue's own cached developer docs have no USB vendor/product ID or volume-label info for this —
confirmed absent, not just unsearched (section 4 of the original draft of this spec). Resolved
instead with a real, live measurement (2026-09-26): the owner's Pocket was mounted directly via
USB-C with SD Access on and its on-device screen read exactly "SD card mounted through USB / <10MB
suggested / Not for large transfers" (the source of section 3c's threshold). With it still
connected, `ioreg -p IOUSB -l -w0` was read directly (not asked of the owner — reachable from this
session) and found the real descriptor:

```
USB Product Name  = "Analogue Pocket"
USB Vendor Name   = "Microchip Technology Inc."
idVendor          = 1240   (0x04D8 -- Microchip Technology's real, USB-IF-assigned vendor id)
idProduct         = 51737  (0xCA19)
USBSpeed          = 1      (full-speed -- consistent with the documented ~1 MB/s ceiling)
```

The same USB tree, at the same moment, also showed a real contrasting case: a `CalDigit`-branded
"Card Reader" device (idVendor 8584, `USBSpeed 4`/SuperSpeed) attached via a dock — a genuine
different-vendor, different-speed USB storage device on the same machine, confirming the signature
is discriminating rather than accidentally matching any USB mass storage device.

**Detection algorithm (macOS; Windows/Linux fall back to "unknown", same posture as
`list_mounted_cards`'s existing `/Volumes/*`-only check):**

1. For a chosen destination path, resolve its whole-disk BSD identifier (`diskutil info <path>`,
   the `Part of Whole`/`Device Identifier` fields — same call `list_mounted_cards` already makes,
   extended to read a couple more fields it doesn't use today).
2. If `Protocol` isn't `USB`, this is not a USB storage path at all (an internal drive, a network
   volume, an SD card in a built-in slot) — no banner, regardless of size.
3. If `Protocol` is `USB`, walk the IORegistry from that `IOMedia` node up to its ancestor
   `IOUSBHostDevice` (`ioreg -r -c IOMedia -l`, matching `BSD Name`/`BSD Unit`, then the parent
   chain's `idVendor`/`idProduct`/`USB Product Name`) and compare against the confirmed signature
   above (`idVendor == 1240` and product name `"Analogue Pocket"` — matching on both, not just the
   product string, since a string alone is a weaker signal to hang a warning on).
4. Match → **Pocket-via-USB**, banner logic in section 3c applies. Any other USB device (the
   CalDigit reader, any other brand) → **plain USB storage**, no banner, ever — a fast third-party
   reader must never be warned about; false positives here cost more trust than an occasional missed
   warning on some future unusual device.
5. Detection failure of any kind (a command errors, a field is missing, running on a non-macOS host)
   → **unknown, no banner** — the same fail-safe posture already decided in this section, kept.

Implementation note: this is host-specific I/O with no domain logic, so it belongs in `src-tauri`
(shelling out to `diskutil`/`ioreg`, both already-installed macOS system tools — no new dependency),
the same split every other host-specific check (`list_mounted_cards`, the recent-cards file) already
follows; `tau-core` stays free of any `process::Command` use, matching its own audited property.

**Built and hardware-verified, 2026-09-26.** `connection_kind` (`src-tauri`) implements exactly this
algorithm. **A real parsing bug was found and fixed by testing against the actual mounted Pocket, not
by inspection:** the first implementation matched `ioreg` property lines with `.trim()` then exact
string equality/`strip_prefix` — but `ioreg`'s tree-drawing `|` characters are not whitespace, so
`.trim()` left a leading `|` in front of every property line and the equality check silently matched
nothing, ever. Two `#[ignore]`d regression tests (`src-tauri/src/main.rs`, run explicitly with real
hardware attached) lock this in: the real Pocket is detected as `Pocket`, and a real, differently
branded USB storage device (a Raspberry Pi Pico in mass-storage mode, confirmed attached to the same
machine at the same time as the genuine contrasting case) is correctly *not* detected as one. Section
3d's icon swap is also built (`CardIcon.svelte`, wired into the sidebar widget and known-card tiles)
and verified live against a mocked backend. Sections 3a/3b/3c (self-update, the Firmware screen, the
transfer-safety banner) are still design-only — nothing beyond this section has been built yet.

**Integration update, 2026-10-02.** The Library workbench (built in parallel) shipped its own
detector and a slow-connection warning. Reconciled as follows: there is one detector, `detect_connection`
in `src-tauri/src/device.rs`, returning `direct_usb` / `card_reader` / `unknown` (my
`pocket` / `usb_storage` / `other`, respectively, with `unknown` as the fail-safe no-banner state). The
hardware-verified USB-descriptor match above lives there now, and the two `#[ignore]`d live-hardware tests
moved with it. Section 3c is **partly built**: the Library review sheet shows the warning at the same
10 MB threshold (from the Pocket's own screen); the Backup, Packages and Core-copy review cards still have
none. Section 3d's icon swap is built on the sidebar card (`CardIcon.svelte`); the known-card tiles are
inline in `App.svelte` rather than a `CardsView` component after the integration. 3a and 3b remain design-only.

## 6. Accessibility

Same brief as every other screen (`PRODUCT_DESIGN.md`): text status in addition to color (the
transfer-safety banner's icon is never the only signal — the sentence carries the meaning), native
`<select>`/`<button>` controls for the channel/build pickers, visible focus, and the banner is a
polite live region like every other `.notice[role=status]` already is. The connection-type icon
swap is decorative (the same information is already in the tile's own text badge), so it needs no
separate alt text beyond what `aria-hidden` already does for the existing glyph.
