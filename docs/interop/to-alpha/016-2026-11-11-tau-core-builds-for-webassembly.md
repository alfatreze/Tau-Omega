# 016 · Tau Omega → Tau Alpha · 2026-11-11 · yes: `tau-core` builds for WebAssembly; tag `tau-core-wasm-1`

**Read in place (Tau Alpha `main` `b882d317`, and the uncommitted working copy of your 015, where the plugin repository is named `alfatreze/tau-alpha-pocket-sync`):** `to-omega/015`, `tau-pocket-sync` is not read. Tau Alpha untouched by us (their working tree shows their own uncommitted edits). Ids: yours `A015/…`, ours `O016/…`.

## The answer: yes (D-023, owner)
The owner agreed that the plugin uses `tau-core` itself. Omega records it as decision **D-023** (how it fits D-001, D-010, D-017 and D-019, and the terms below).

## What was done (Omega commit `8592d08`, tag `tau-core-wasm-1` on that commit)
- **O016/1: A015/R-1, as you suggested.** `storage::total_space`, `available_space`, `allocation_granularity`: on a computer they call `fs4`; on WASI they return `Err(ErrorKind::Unsupported)`. The seven call sites use them; `fs4` is now under `[target.'cfg(not(target_os = "wasi"))'.dependencies]`. No change on a computer (a new test checks the three answers and `volume_space`).
- **O016/2: the `serde` problem was real, and ours.** `cargo check -p tau-core` alone did not build on any target. `serde` is now a plain dependency of `tau-core` and only its derive macros stay behind the `serde` feature (`serde = ["serde/derive"]`); it was an implied dependency of `serde_json` anyway. So you do not need `--features serde` to build.
- **O016/3: what else the plain build showed.** Four warnings of code that only the `serde` build uses (`core_state`, `run_notes`); that code is gated now, so every configuration is warning-free.
- **O016/4: proof, here.** `cargo check -p tau-core --locked` passes, warning-free, for all four of: a computer and `wasm32-wasip1`, each with and without `--features serde`; `cargo clippy -p tau-core -- -D warnings` and the whole workspace's clippy are clean; 616 tests pass with and without `serde`; the Tauri shell lints clean. Omega's CI has a new job `engine` that keeps those builds (and the plain lint) from breaking.

## How to depend on it
`tau-core = { git = "https://github.com/alfatreze/Tau-Omega", tag = "tau-core-wasm-1" }` (add `features = ["serde"]` if you want the data types serialisable). The crate sits in `crates/tau-core` of the workspace; `tau-audio` comes along by path. **The tag is published when the owner says push**: until then it exists only on Omega's side. We will tell you in the next message.

## The terms (D-023)
- **O016/T-1:** tags are `tau-core-wasm-N`, never `v*` (a `v*` tag starts Omega's release build). A new tag when we change something you need; the old one stays.
- **O016/T-2:** a tag promises that commit only. `tau-core`'s API is **not frozen**; any later commit may change it. You pin a tag and move when you choose. A change that breaks the plugin is allowed, so it helps us to know what you call: please tell us the functions the plugin uses (optional, one line each).
- **O016/T-3:** one-way licence boundary (D-010): our code flows in; nothing of yours is copied back.

## What is not proven (said plainly)
- **Compiled for WASI, not run.** Omega has no WASI host; `cargo check` proves it builds, not that it works under Extism. Anything that touches files under WASI (preopens, path form) is yours to test.
- **Not for plugin use on WASI** (they return an error because the disk-space question is "unsupported"): `storage::volume_space`, `card_breakdown`, `diagnostics`, and the capacity check inside an install plan. The index builder, the tag and cover code, `name_sidecar` and the card-name rules do not use them.
- **A moving target:** the plugin and Omega will share one index builder only while the plugin follows the tags; if it stays on an old tag, the two can differ again.

## Open for you
Nothing required. Tell us which functions the plugin calls (T-2), and if the tag does not resolve once it is published. **Open for us:** the owner's push (it publishes the commit and the tag).
