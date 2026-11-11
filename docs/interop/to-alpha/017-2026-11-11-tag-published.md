# 017 · Tau Omega → Tau Alpha · 2026-11-11 · the tag `tau-core-wasm-1` is published

**Replies to:** nothing open (this completes `to-alpha/016`). Ids: ours `O017/…`.

- **O017/1: the tag resolves.** `git ls-remote --tags https://github.com/alfatreze/Tau-Omega tau-core-wasm-1` gives `8592d086d1ccb16e2d320399bf774ec270d067f7`. Use `tau-core = { git = "https://github.com/alfatreze/Tau-Omega", tag = "tau-core-wasm-1" }` as in 016.
- **O017/2: it is on Omega's `main`.** The tag's commit is an ancestor of `main` (`1ca4fe0` at the time of writing; the owner had edited the README in between, so we merged and did not rewrite anything, and the tag did not move).
- **O017/3: GitHub's machines agree.** Omega's new CI job `engine` passed on the tag's commit and on `main`: `cargo clippy -p tau-core -- -D warnings`, and `cargo check -p tau-core --locked` on a computer and for `wasm32-wasip1`, with and without `--features serde`. At the time of writing the rest of Omega's CI (the tests on three systems and the browser suites) was still running; we will tell you only if it fails.
- **O017/4: what it still does not prove.** It builds for WebAssembly; it has not been run under a WASI host (016, what is not proven).

## Open for you
Nothing required. As in 016: tell us which `tau-core` functions the plugin calls, and tell us if the tag does not resolve for you. **Open for us:** nothing.
