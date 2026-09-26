# Implementation boundary

## Functional model

Tau Omega has one shared local operator surface. A user selects folders, reviews read-only results, creates explicit plans, then confirms any write. The Rust core is authoritative for scanning, plans, verification, playlists, diagnostics and duplicate groups; Tauri is an adapter and Svelte renders the results.

## Delivery order

1. **Safe, fixture-testable now:** playlist scan/export, duplicate detection, Problems view, job history from journals, typed frontend API, core tests, settings/history decode, library browsing based on scanned/indexed data.
2. **Safe dry-run now; validation later:** storage planning, core install/package preview, backup preview, tag-doctor plan, screenshot/log discovery.
3. **Blocked on supplied fixtures or physical validation:** diagnostic-record decoding, device-specific capability verification, real-card writes/eject, core package installation, screenshot format verification.

## Problems view states

| State | Meaning | Safe user action |
| --- | --- | --- |
| First use | No folder selected | Choose a local media root |
| Loading | Hashing/scanning | Wait; source remains untouched |
| Empty | No duplicate groups | Choose another folder |
| Results | Duplicate groups found | Review paths only |
| Failed | Folder/file could not be read | Retry after selecting an accessible folder |

## Cross-project interface order (tau-alpha's meter/image work, 2026-09-26)

tau-alpha's B-284 (image formats) and B-274/B-294 (meter module) sessions produced two new
Omega-facing interfaces at very different levels of readiness. Sequenced per
`docs/CROSS_PROJECT_INTERFACE.md`'s own rule (never build against a spec with no real artifact yet):

1. **Cover images (`TIM1`), started now.** `tau-alpha/docs/IMAGE_FORMATS.md` D-I01/D-I02 (2026-09-26)
   decided palette-256 at 128 px, proportional scale, no crop/letterbox — final enough to build
   against, and the tool that produces it (`tools/tau_image.py`) already exists with real output
   files to verify against. `tau_core::image` implements `TIM1` decode (all payload shapes real
   tooling produces: `rgb565`, `palette` at 8/6/4 bpp) and a palette-256 encoder (own quantizer, not
   required to match `tau_image.py` pixel-for-pixel — only the container shape is shared). Verified
   against a real fixture, not an invented one (`testdata/images/README.md`). **Done, 2026-09-26:**
   `art_sidecar_pal256` is a `sync::PlanOptions` field wired end to end (engine, `tau-cli`, the Tauri
   adapter, the Sync screen's checkbox and plan review) — one `.timg` sidecar planned per album
   folder, written and read back through `image::decode_tim1` at execute time. **Also done,
   2026-09-26:** decode-and-show, as a lazy "Preview" button in the Sync plan review itself (not a
   Library/Cards thumbnail grid — the Library screen's flat, virtualised track table has no album
   grouping and stays a separately-tracked future item). Along the way, found and fixed a real
   determinism bug in the palette quantizer (median-cut was iterating a `HashMap` directly, whose
   order isn't just a function of its contents) — a new test encodes the same real cover twice and
   checks for byte-for-byte agreement. **Caveat that changes nothing about priority but does change
   what this unlocks today:** no firmware reader exists yet (`IMAGE_FORMATS.md`'s own status line)
   and no data slot is assigned (D-I05, open) — writing these files to a real card has no effect on
   the Pocket itself yet.
2. **Meter presets (`tau-assets.bin`/`METR`), tracked, not started.** `METER_MODULE_SPEC.md`'s full
   Omega-facing design (§6, §20-22: the container, `.tmeter`/`.tmeterpack`, `meters_schema.json`,
   capture-from-QR) is decided (D-M01–M13) but tau-alpha is only at the start of its own build order
   (M0), with M2 (`SR_T_METERCFG`) and M4 (the actual container + hand-off) still ahead. Building the
   Omega side now would mean building against a spec with no real artifact — the exact mistake
   `CROSS_PROJECT_INTERFACE.md` §4 already caught twice for this project. See `docs/FIRMWARE_SYNC.md`'s
   new watched-interface entry; pick this up once tau-alpha tags a release containing
   `meters_schema.json`.

No new repo, and no dependency on tau-alpha's own Python tooling: both interfaces are reimplemented
in `tau-core` (Rust), the same pattern as `icon.rs`/`taud.rs` before them, verified against real
captured artifacts rather than shelling out to or forking the other project's tools.

## Accessibility baseline

Target WCAG 2.2 AA. Problems results use text labels in addition to colour, native buttons and inputs, visible focus, keyboard operation, status messages, and no automatic destructive action. Compliance still requires runtime testing.
