# Tau Omega

Desktop manager for the Tau Analogue Pocket core: card inspection, music sync (with conversion and ReplayGain), library index, covers, core install/update, Halcyon EQ presets, report-code decoding. Rust engine (`crates/tau-core`, portable, no UI), Tauri host (`src-tauri`), Svelte UI (`ui`), CLI (`crates/tau-cli`). MIT OR Apache-2.0.

## Session start
Read `ROADMAP.md` (the one ordered list), then the top "LATEST" block of `docs/STATUS_HANDOFF.md`. Dated sections below it are history. Root the session in this folder.

## Scope
- Tau Omega work only. `../tau-alpha` (and its worktrees) is **read-only**: read it for facts, never edit, commit or reset there.
- Plan a Tau feature only once it is on Tau Alpha `main`. The two repos never share literal files; reference, do not copy (`docs/FIRMWARE_SYNC.md` lists what is read from Tau Alpha and how it is checked).
- Licence boundary (D-010): our code may flow into AGPL hosts (Pocket Sync), theirs is never copied back.

## Rules
- **Push only when told "push".** Commit locally, then ask.
- **Card writes:** only on `CARDWRITE` (never `Pock`), only after a shown plan and a clear yes; dry run first. Never modify a source music file. Full list: `SAFETY_RULES.md`.
- **Ask before** a new dependency or any download.
- The latest build to test goes in `releases/` before it is announced.
- Releases: follow `docs/RELEASING.md` (version in four places, tag `vX.Y.Z`, CI makes a **draft**; publish by hand after checking assets against `SHA256SUMS.txt` and `latest.json`). Installer signing is parked (owner, 2026-10-14).
- After a push, check CI (`gh run list --repo alfatreze/Tau-Omega`); Windows is the job that has failed before (path separators, unused-on-Windows variables).

## Checks before a commit
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --features tau-core/serde`, `cd ui && npm run check`. Test build: `tools/dev-build.sh`.

## Standing decisions
Numbered in `docs/DECISIONS.md` (portable engine boundary D-001, plan-then-confirm D-004, host-side journals D-006, Pocket Sync as the portability target D-009). Conversion rules: `docs/CONVERSION_AND_REPLAYGAIN.md`. Product design: `docs/PRODUCT_DESIGN.md`. Do not re-litigate a decision listed there; add a new D-NNN instead.

## Where things go
- State and handoffs: `docs/STATUS_HANDOFF.md` (newest block first), order of work: `ROADMAP.md`.
- This file holds rules only: no execution log, no project history.
