# Test plan

## Principles
Tests never touch a real card or the user's music. They use temporary directories, generated fixtures and **fake cards** (a folder with `Cores/`, `Assets/`, `Platforms/`, `System/`, `Settings/`).
Anything that writes to `/Volumes/*` or a drive letter fails the build. Hardware results are recorded separately and labelled as hardware.

## Layers
1. **Unit (Rust):** names/ASCII, sort keys, tag readers on fabricated files, index writer/reader, section arithmetic, CRCs, playlist rules, path limits, plan builder, journal.
2. **Conformance against the Python reference** (`tau-alpha/tools/tau_library.py`):
   * A generator script builds fixtures with the Python tools: the fabricated tree from `sim/test_library_index.py`, `tau_library.py synth` at 400, 7,180 and 16,384 tracks, with and without playlists.
   * Rust output must equal the Python `.tdb` byte for byte; the Rust reader must return the same counts, strings and orders; the corruption matrix (bad magic, header CRC, version, truncation, appended bytes, body flips, cap, section range/alignment, track offsets, playlist range/item) must produce the same E-codes.
   * A nightly job runs Python and Rust over any real-library copy the developer points at (read-only) and diffs the reports.
3. **Firmware cross-check (optional, local):** run `tau-alpha/sim/test_library_fw.py` over indexes written by Tau Omega to prove the firmware loader accepts them (needs the vendored RISC-V toolchain).
4. **Sync engine:** on fake cards: new, update, same, mirror delete, ASCII collisions (`Nausicaä` vs `Nausicaa`), NFD names, long paths, `._` files, read-only destination, disk full, card removed mid-run (simulated), interrupted run then re-run, cancel, verify failure injection. Assert source hashes unchanged after every run.
5. **Copy/move between cores and cards:** playlist line rewriting, index root change, duplicates, two-step delete, backup before delete, move interrupted before delete.
6. **Covers:** MP3 and FLAC embedding into copies, baseline vs progressive JPEG, size caps, optimise settings, the "reduce vs full decode" decision matches `library_check.py`.
7. **Core install:** zip validation (bad zip name, extra folders, version mismatch, missing rbf_r), diff, backup, install, remove superseded, cache cleanup.
8. **Diagnostics decoders:** golden `interact_persist.json` samples and 64-byte records decode to the same JSON as `decode_tau_diag_log.py`; history word round trip with an index.
9. **UI:** component tests, and end-to-end tests driven through Tauri's WebDriver on fixture cards; screenshot tests for the plan table and confirmation modals; accessibility checks (axe) on every screen; all 19 accent colours checked for contrast.
10. **Performance:** 7,180-track scan+plan, 16,384-row list scroll, 50 GB copy throughput on a temp volume.
11. **Cross-platform matrix:** macOS (Apple silicon and Intel), Windows 11 (and 10), Linux CI for the core crate.

## Owner acceptance runs (hardware, need approval)
1. Sync the throwaway copy of the real library to `TAU PSRAM NN`; Info shows `<N> TRK`; play random tracks; browse; playlists; history restore.
2. Copy media to a second core; both boot and browse.
3. Negative index cases on the Pocket (missing file, flipped byte, wrong version, truncated, oversize): expected `OFF Ennn`, playback unaffected.
4. Install both release zips on a fresh card image; compare with the manual procedure byte for byte.

## Release checklist
Tests green on all three OSes; conformance job green; no network calls in a packet capture with updates off; signed and notarised artefacts verified (`spctl`, `signtool verify`);
installer smoke test on clean VMs; checksums published; CHANGELOG and compatibility table updated; the two docs (`SPEC.md`, `DATA_FORMATS.md`) match the shipped behaviour.
