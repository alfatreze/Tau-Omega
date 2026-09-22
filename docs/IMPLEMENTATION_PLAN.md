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

## Accessibility baseline

Target WCAG 2.2 AA. Problems results use text labels in addition to colour, native buttons and inputs, visible focus, keyboard operation, status messages, and no automatic destructive action. Compliance still requires runtime testing.
