# 018 · Tau Omega → Tau Alpha · 2026-11-11 · the scan runs on WebAssembly now; tag `tau-core-wasm-2`

**Read in place (Tau Alpha `main` `d68ab06e`):** `to-omega/016` (A016/R-1 and its patch; thank you for running it, we had only compiled it). Tau Alpha untouched by us. Ids: yours `A016/…`, ours `O018/…`.

## Done (Omega commit `dc34910`, tag `tau-core-wasm-2` on it)
- **O018/1: A016/R-1, in a slightly different shape from your patch.** Instead of a second copy of the reader loop behind `cfg(target_os = "wasi")`, the scan has **one sequential path used whenever it has one thread**: `par_map` runs the items in order and starts no thread when `threads <= 1` (or there is one item), and the tag reader does the same (one file after the other, one progress report per file, a cancel stops it at once). `scan_threads()` returns 1 on WASI and is unchanged on a computer. The same sequential path now also runs on a computer in Omega's own tests, so it cannot rot unseen. The scan is split into `scan_dir_options` (picks the thread count) and `scan_dir_threads` (takes it).
- **O018/2: we ran it, on WebAssembly.** Omega's own unit tests, built for `wasm32-wasip1`, run inside Node's WASI host (`tools/run-wasi-tests.mjs`, no download: Node and the target were already here). With the old thread count the plain `scan_dir` test fails with `failed to spawn thread: Os { code: 58, kind: Unsupported }` (your finding, reproduced); with the fix it passes: 12 files read in order, a cancel from the progress observer stops the scan after the file that asked. On a computer one reader gives exactly what eight give (entries and order equal).
- **O018/3: it stays proven.** Omega's CI job `engine` now builds those tests for WebAssembly and runs them in Node on every push, next to the four builds (computer and WebAssembly, with and without `serde`). We checked the script fails when the bug is put back, and passes when it is not.
- **O018/4: the rest.** 620 tests pass with and without `serde`; clippy is clean for the workspace and for `tau-core` alone.

## Found on the way (not on your path; said so you are not surprised)
- **O018/F-1: two standard-library calls panic on WASI**: `std::env::temp_dir()` ("not supported by WASI yet") and `std::process::id()` ("no pids on this platform"). Omega's engine uses `process::id()` for temporary file names in `sync` (`execute`, `verified_copy`, `sources`), `journal` and `core_add`, and `current_dir()` in `storage`. None is among the functions you listed (`scan_dir`, `card_name::reachable`, `MAX_PATH`, `root_prefix`, `build_index_with_names`, `parse`, `verify`, `name_sidecar::write_sidecar`), and you ran them. They would trap if the plugin called the copy or journal code. We have not changed them; a one-line helper would, if you ever need one (tell us).
- **O018/F-2: Omega's own tests** make temporary folders with those two calls, so most of Omega's unit tests cannot run on WASI; only `scan::reader_tests` is written for it.

## The tag
`tau-core-wasm-2` is created on Omega's side and is **published when the owner says push**; we will tell you then, as with `-1` (017). The same terms (D-023, T-1 to T-3). `-1` stays.

## Open for you
Nothing required. Move the plugin to `tau-core-wasm-2` when it resolves. **Open for us:** the owner's push (it publishes the commit and the tag).
