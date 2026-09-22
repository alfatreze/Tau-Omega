# Tau Omega product design — T0/T1

## Functional model

| Area | Model |
|---|---|
| Listener surface | Select a source and a card/core, review an immutable plan, then confirm a verified job. |
| Developer surface | Inspect every core, understand capability and index health, then later install, compare and diagnose. |
| Shared objects | Card, core, platform, media root, source, plan, job, index, report. IDs and platform paths are authored only by the Rust engine. |
| Safety constraints | Sources are always read-only; plans are pure; every future write needs a new confirmation; index is last; deletes require backup and a second confirmation. |
| T0/T1 outcome | Read-only card/core discovery and a local index engine. No user filesystem mutation beyond an explicit CLI index or synth output with `--yes`. |

## Information architecture

`Cards` is the home screen and the safe entry point. A selected card leads to `Card overview`, then a core leads to `Core detail` and `Library`. Future workflow actions originate from a core: `Sync`, `Copy/Move`, `Install`, `Diagnostics`; `Jobs` provides the audit trail. `Settings` is app-local.

The app never makes a generic file manager of an SD card. Unknown cores remain visible but read-only until a user explicitly targets a supported core.

## Flow inventory

1. **Inspect card (T0):** Open a folder → validate `Cores/` + `Assets/` → parse each `core.json` and `data.json` → show capability chips and warnings → select a core.
2. **Build index (T1 CLI):** Scan a destination media root → show counts/warnings → explicit `--yes` → encode → re-parse → write output. This is intentionally a local/staging action only.
3. **Future sync:** Choose sources and destination → options → plan → confirmation → copy/verify → atomically publish index → report/eject.
4. **Future move:** Plan copy → confirmation → copy/verify/reindex → show separate deletion confirmation + backup → delete or leave source intact.

## Screen inventory and execution brief

| Screen | Flow step | Primary content | T0/T1 state |
|---|---|---|---|
| Cards | Inspect 1–4 | Clear open-folder affordance, cards/cores and short privacy promise | Implemented scaffold |
| Card overview | Inspect 4 | Capacity, core table, warnings, verify/backup entry points | Designed, deferred |
| Core detail | Select core | Index health and future actions | Designed, deferred |
| Sync wizard | Future sync | Sources, options, reviewed plan, running job, report | Designed, deferred |
| Jobs | Future jobs | Immutable journal/report, warnings, recovery | Designed, deferred |

Visual direction: a quiet dark workbench, one high-contrast mint action color, dense information only after a card is selected. The interface exposes safety state in words (for example, “Read-only inspection”), never color alone. There is no decorative “Pocket edition” palette in the initial shell because contrast must be checked before each accent is enabled.

## State inventory

| Surface | State | Trigger | Safe recovery |
|---|---|---|---|
| Card open | Empty / valid / invalid / read-only / removed | Folder selection or volume change | Explain the condition, retain the entered path, offer retry; never infer another target. |
| Core list | Loading / partial / ready / malformed metadata | Parsing core files | Show valid cores and a per-core warning; no write action is enabled. |
| Index | Missing / valid / stale / malformed (`E11`–`E17`) | Read or verification | Explain code and offer a future plan/rebuild entry; preserve the current index. |
| Future plan | Draft / reviewed / expired / executing / cancelled / partial / completed | Input change, confirmation, job event | A changed card/source expires the plan. Cancellation retains the prior index. |
| Future deletion | Pending second confirmation / backed up / completed | Copy+verify then user confirmation | Exact target, volume, file count and bytes are restated; backup location is visible. |

Authoritative lifecycle state is the Rust job runner and its journal, not the UI. A removed volume or changed source invalidates a plan before execution; a late UI event may update display only and cannot resurrect a plan.

## Accessibility brief

Target: WCAG 2.2 AA implementation and testing on macOS and Windows. This is a design target, not a conformance claim.

- Keyboard order follows the visual order: sidebar, page heading/action, folder field, inspection result, core rows.
- Native labels, buttons and status messages are used; the inspection result has a polite live-region role.
- Visible focus and a minimum 44×44 px hit area are required for future icon actions. The present screen has text-labelled controls.
- The mint action color is paired with labels and never the only indication of library capability or write safety. Future 19 accent colors require WCAG contrast checks against their actual foreground/background pairs; APCA can supplement the review but does not establish WCAG conformance.
- No timed confirmation, motion-dependent progress or drag-only operation. Reduced motion removes nonessential progress animation.
- Test matrix: keyboard-only card opening and row selection; VoiceOver/Narrator labels and status announcements; 200% zoom/reflow; light/dark/high-contrast modes; color-vision review; error recovery after card removal.

## Open risks

- T1 currently proves the encoder against Python-oracle fixtures. The CLI synthetic-data generator will be replaced by a literal port of Python's seeded generator before declaring complete synthethic-input parity.
- Full Unicode NFKD coverage needs a dedicated normalization dependency and expanded vectors before arbitrary tag-text parity is claimed.
- Tauri package compilation and the UI build pass locally; signed installers, a production app icon, and hardware testing remain release work.
- The Sync screen can optionally add a folder's baseline JPEG artwork to MP3/FLAC destination copies. The artwork hash is part of the reviewed plan. Conversion/optimisation, cancellation and a persistent SQLite job journal are deliberately deferred. CLI mirror is available only with both a second plan-token confirmation and an external, SHA-256-verified backup folder; the desktop UI deliberately does not expose it yet.

## T2 sync design update

Routing ledger: `states → plan / confirmation / recovery transitions`; `accessibility → keyboard, status and confirmation treatment`. The sync wizard is a shared surface: listeners use the short source-to-card path, while developer usage retains the same immutable plan and local report.

The implemented flow is: source paths + explicit `Assets/<platform>/common` destination → read-only plan → exact plan token displayed → deliberate **Confirm and sync** action → verified result and host-side report. A changed source or destination changes the token and blocks execution. The engine copies each file to a temporary sibling, SHA-256 verifies it, then renames it; it scans the resulting destination and publishes the index only after the media work succeeds. An unchanged rerun verifies the existing valid index and performs no writes.

The wizard's empty state explains required paths; its failure state preserves entered paths and explains that nothing was reported as complete. A host-side JSON job report is created before execution and transitions to `completed` or `failed`, so a disconnected card does not erase the recovery context. Its status message is announced to assistive technology. The confirmation action names the target path, file counts and byte total in visible text; it is a labelled button, keyboard-operable, and does not depend on color or animation.

For CLI mirror, the plan includes the exact deletion count. Execution transitions from reviewed to copy-verified, then only on a second matching token to backup-verified and deleted; the index is rebuilt afterward. A missing or mismatched second token is a safe blocked state: no target deletion occurs.

## T3 multi-core design update

Comparison is a read-only preparation screen, not a mutation shortcut. It presents four mutually exclusive results—only in the first core, only in the second core, different, and matching—so a later copy or move plan can state exactly what it will do. The portable comparator is implemented in the engine, CLI, and an accessible desktop comparison screen. From that screen the user can review and confirm a complete source-to-destination copy: paths are preserved, every destination file is verified, the destination index is rebuilt last, and the source has no write path. Selection and move confirmation are not enabled yet.
