# Tau Omega

Desktop manager for the Tau Analogue Pocket core: card inspection, music sync (with conversion and ReplayGain), library index, covers, core install/update, Halcyon EQ presets, report-code decoding. Rust engine (`crates/tau-core`, portable, no UI), Tauri host (`src-tauri`), Svelte UI (`ui`), CLI (`crates/tau-cli`). MIT OR Apache-2.0.

## Session start
Read `ROADMAP.md` (the one ordered list), then the top "LATEST" block of `docs/STATUS_HANDOFF.md` (state, next steps, hazards), then `docs/interop/OPEN.md` (what is open with Tau Alpha). Dated sections below the top block are history. Root the session in this folder.

## Scope
- Tau Omega work only. `../tau-alpha` (and its worktrees) is **read-only**: read it for facts, never edit, commit or reset there.
- Plan a Tau feature only once it is on Tau Alpha `main`. The two repos never share literal files; reference, do not copy (`docs/FIRMWARE_SYNC.md` lists what is read from Tau Alpha and how it is checked).
- Licence boundary (D-010): our code may flow into AGPL hosts (Pocket Sync), theirs is never copied back.

## Rules
- **Git commit attribution:** Every git commit must be authored and committed solely as **Abel Santos `<alfatreze@gmail.com>`** (matching the local `git config`). Never commit under an AI agent identity, and never append `Co-Authored-By:` or AI attribution trailers to commit messages or PR descriptions.
- **Push only when told "push".** Commit locally, then ask.
- **Card writes:** two test cards, **`Pock`** (the owner's main Pocket card: it holds real cores and music, so snapshot it first) and **`CARDWRITE`** (a scratch card for memory-write and other occasional tests, and for trying several cards in tandem). Never any other volume. Only after a shown plan and a clear yes; dry run first, backups outside the card. Never modify a source music file. Full list: `SAFETY_RULES.md`.
- **Ask before** a new dependency or any download.
- The latest build to test goes in `releases/` before it is announced.
- Releases: follow `docs/RELEASING.md` (version in four places: `Cargo.toml`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `ui/package.json`, and both lock files; notes in `docs/RELEASE_NOTES_X.md`; tag `vX.Y.Z`, CI makes a **draft**; put the notes in the draft's body, publish by hand after checking assets against `SHA256SUMS.txt` and `latest.json`). Installer signing is parked (owner, 2026-10-14).
- After a push, check CI (`gh run list --repo alfatreze/Tau-Omega`); Windows is the job that has failed before (path separators, unused-on-Windows variables, line endings of pinned fixtures). **Tag a release only on a commit whose five CI jobs are all green.**
- **Before committing, run `git branch --show-current`:** another session once switched this folder's branch and four commits landed on it. Feature work for another project belongs in a clone or worktree, never a checkout here.
- **Interop** (`docs/interop/`): a message counts as sent when it is committed, and the other session reads the folder in place, so show the owner a message before writing it there. Push is separate.
- **Tau Alpha's files are never created or edited by Omega:** the per-meter settings file `Saves/<platform>/<core>/tau-meter-settings.bin` is the Pocket's. The one allowed write is deleting a copy of the wrong size, after a plan and a verified backup outside the card.
- **Fixtures:** `testdata/compat`, `meter_bin`, `meter_settings`, `persist_layout` are byte copies from Tau Alpha (`.gitattributes` keeps their bytes; each has a README with the source and SHA-256). The rest of `testdata` is written by `testdata/gen_golden.py` (CI checks it changes nothing). Tests that read `../tau-alpha/release` or `../tau-alpha/work/...` skip when absent (so not in CI): run them locally before a release.
- Product text (UI, release notes, README) is plain, short, non-technical language for the user; technical notes stay short and separate.

## Channels and builds (same three as Tau Alpha)
Stable = published release `vX.Y.Z`; Preview = published pre-release `vX.Y.Z-preview.N` (tag, CI draft, `tools/publish_preview.sh`); Dev = local `X.Y.Z-dev.N` from `tools/dev-build.sh` (never published). `releases/dev-builds/TARGET` is the version dev builds lead to (0.6.2). Details: `docs/RELEASING.md`. All browser suites: `cd ui && bash scripts/run-all.sh` (needs `npm run dev`).

## Checks before a commit
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --features tau-core/serde`, `cd ui && npm run check`. Test build: `tools/dev-build.sh`.

## Standing decisions
Numbered in `docs/DECISIONS.md` (portable engine boundary D-001, plan-then-confirm D-004, host-side journals D-006, Pocket Sync as the portability target D-009). Conversion rules: `docs/CONVERSION_AND_REPLAYGAIN.md`. Product design: `docs/PRODUCT_DESIGN.md`. Do not re-litigate a decision listed there; add a new D-NNN instead.

## Where things go
- State and handoffs: `docs/STATUS_HANDOFF.md` (newest block first), order of work: `ROADMAP.md`.
- This file holds rules only: no execution log, no project history.

## Live dev (2026-10-15)
Instead of opening a new alpha for every change, run the live app: `cd src-tauri && cargo tauri dev` (starts Vite on :5173 and the Rust shell; UI edits hot-reload, Rust edits rebuild and restart the window). Only cut `tools/dev-build.sh` alphas when a build is meant to be kept or tested elsewhere.
