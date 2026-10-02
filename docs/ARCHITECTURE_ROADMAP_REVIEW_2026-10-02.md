# Architecture and roadmap review — 2026-10-02

Scope: Tau Omega as of `integration/workbench-base` (alpha.7), read against tau-alpha at `v0.6.0-alpha.1` plus its `main`
(Cymo work, B-series to B-511). Method: read both roadmaps, `FIRMWARE_SYNC.md`, `CROSS_PROJECT_INTERFACE.md`, the shipped
`dist/Cores/alfatreze.TAU/interact.json` and `fw/suite_core.h`; ran `cargo fmt --check` and `cargo clippy` here. Findings marked
**(checked)** were verified in code or by running a tool; the rest are judgement.

## 1. Where Omega stands

Built and tested: the portable engine (index, scan with ledger, sync with provenance, workbench edits, packages, backup, comparison,
Check/QR decode up to tag 14, screenshots, `TIM1` images, theme file writer), a Library workbench with one card-space bar, safe eject,
read-back verification, history, and an Appearance editor. 141 Rust tests, 166 browser checks.
Not built: `tau-cli` beyond a stub, Send diagnostics (called *priority* in the roadmap), installing to a card from Appearance,
the meter-preset (`METR`) writer, a real meter schema (Meter Lab is synthetic), Pocket Sync adoption, installers/signing, watch mode.

## 2. Findings that need action

### 2.1 CI would fail today (checked)
`ci.yml` runs `cargo fmt --check` and `cargo clippy -D warnings`. Right now: **255 formatting diffs** (worst: `sync.rs` 37,
`workbench.rs` 27, `tagedit.rs` 27, `assets.rs` 24, `ledger.rs` 23) and **3 clippy errors** (`assets.rs:194` `if` with identical
blocks, `sync.rs:313/318` collapsible `if` and a redundant comparison, `workbench.rs:496`). Cheap and mechanical; do it before
anything else so the next real failure is visible.
CI also **does not run the 166-check browser suite** (`ui/scripts/workbench-test.cjs`); only `npm run check`. The suite has caught
most of this session's UI regressions, so it belongs in CI (needs a Chromium install step).

### 2.2 Firmware sync is a release behind (checked)
`FIRMWARE_SYNC.md` was last verified against **v0.5.0**. tau-alpha has since shipped `v0.6.0-alpha.1` and moved on. Concrete drift:
* **Persist id 27 is now "theme", 28 "theme mode"** in the shipped `interact.json`. Omega's Settings viewer still labels 27 as
  "Library off" (`App.svelte` `settingLabel`) and `DATA_FORMATS.md` documents the same. A card with a saved theme would be shown as
  "Library off = n". Fix: label 27/28, update the doc. (tau-alpha's own `tools/tau_data_slots.py` docstring still says id 27 is retired and
  never reused; that is stale on their side and worth telling them.)
* **Check/QR tags 15–22 are not decoded.** Firmware now emits `BLITTEST, STACK, WVIZCFG, METERSWEEP, INFOEXPORT, METERCFG, METERTRACE,
  DECPROF2`; `taud.rs` handles 1–14. Unknown tags are kept and shown raw, so nothing breaks, but the newest hardware evidence
  (decode-stage split, meter sweep, Info export) is unreadable in Omega. Per the cross-project rule, decode only against real captured
  QR screenshots, not the layout comments.
* New meters exist (Layered Wave is id 16; Meter slider max 16). Omega's Meter Lab still uses a synthetic schema.
Action: re-run the sync check against `v0.6.0-alpha.1`, fix the label, then decode tags 20, 21, 22 (the ones with active firmware work) from real captures.

### 2.3 Docs describe a different product than the one built
* `ARCHITECTURE.md` says its module tree is "intended" and the code is flat; true, but it also omits everything added since:
  `ledger`, `breakdown`, `assets`, `workbench`, host hooks (cache evictor, ledger locator), the card write lock, the eject path.
* `ROADMAP.md` (T0–T8, TP0–TP2) no longer matches the work: T4 is done in a different shape (the workbench), the extension list
  mixes done and undone items, and items built this month (ledger, card-space bar, appearance, read-back) are nowhere in it.
* `SAFETY_RULES.md` lacks the invariants this session established: the ledger never influences deletion, overwrite or verification;
  read-back after write must bypass the OS cache; index swap is recoverable; every card write holds the volume lock.
Action: rewrite the roadmap as one ordered list (tau-alpha's `docs/ROADMAP.md` is the right model: one list, status per row), add the
missing modules and invariants.

### 2.4 Shape of the code (judgement)
* `sync.rs` 1,969 lines, `lib.rs` 1,854, `main.rs` 1,100 (about 60 commands in one file), `Workbench.svelte` 973, `App.svelte` carries
  34 top-level state variables for the older pages. Nothing is broken, but the Pocket Sync goal (easy to vendor) and assistant-driven
  edits both get worse as files grow. Suggested seams: `sync.rs` into plan / execute / verify / index swap; `main.rs` by domain (card,
  library, sync, diagnostics, appearance); move the pre-workbench pages' state out of `App.svelte`.
  Do this **after** CI is green and in small, behaviour-preserving commits; the test suites are the safety net.
* `dev-mock.ts` re-implements engine behaviour in TypeScript (progress paths, plan warnings, now a partial contrast check). It is the
  right tool for UI tests but it can silently drift from the engine. Keep mocks shape-only where possible; the Rust tests are the authority.

## 3. Roadmap proposal

**A. Now, small and safe (about a day)**
1. Make CI green (fmt, 3 clippy fixes) and add the browser suite to CI.
2. Fix persist labels 27/28 and update `DATA_FORMATS.md`; re-run the firmware sync check against `v0.6.0-alpha.1`.
3. Rewrite `ROADMAP.md` and refresh `ARCHITECTURE.md`/`SAFETY_RULES.md` as in 2.3.

**B. Next, needs real artefacts or a decision**
4. Decode QR tags 20–22 (and 15–19 as needed) from real captured screenshots; ask for captures of a Meter Sweep, Info export and a
   Check with DECPROF2.
5. Appearance → card install, through the normal plan → review → confirm path (it is a one-file write to a core's `common` folder; the
   package and sync machinery already do this). Gate: one real card run showing `Info > THEME FILE` reads `1 LOADED`; capture the file.
6. Send diagnostics (roadmap priority since 2026-09-21, still unbuilt): gather Check screenshots, persist files, versions, card space and
   hashes into one zip, decode before sending. It reuses the QR decoder and screenshot listing already built.
7. Meter Lab on the real `meters_schema.json` plus a `METR` writer (a data swap by design). Gate: a real `tau-assets.bin` with a `METR`
   section captured from a card.
8. Ledger follow-ups: per-volume I/O governor, hash-while-copy, cluster-aware Library fit bar, a card-speed benchmark. Gate: first real
   timing numbers (never measured).

**C. Later**
9. Module split (2.4). 10. `tau-cli` parity with the app, `--json` everywhere (also the Pocket Sync surface). 11. Installers, signing,
opt-in updater. 12. Pocket Sync adoption (TP2). 13. Watch mode, smart playlists, loudness tags, localisation.

**Decisions I need from you**
* Push `integration/workbench-base` and fast-forward `main`? (Still pending; nothing is on origin.)
* Is Send diagnostics (6) really ahead of the Appearance install (5)? The roadmap says priority; the work just done points the other way.
* Widen the "Tau core" rule to every Media Players core (today `HarpMudd.Mp3Player` media counts as "Other data")?
* Should tau-alpha be told about the stale id-27 note and the Layered Wave / Cymo surfaces, via the usual change log there?

## 4. What I did not check
Cross-platform behaviour (Windows/Linux, only macOS exercised); real-Pocket write speed and the eject order; whether tau-alpha's `main`
changed any wire format after `v0.6.0-alpha.1` beyond the tags and persist ids above; Pocket Sync's current API.
