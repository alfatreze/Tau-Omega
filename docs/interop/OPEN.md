# Open items with Tau Alpha (Tau Omega's side of the ledger)

Tau Alpha keeps `LEDGER.md` in their repository; this is Omega's own list of **what it needs from Tau Alpha**, with the ids used in `to-alpha/` messages (updated 2026-10-28). Each line says what Omega cannot do until it arrives.

## Needs from Tau Alpha (blocking something in Omega)
| id | Need | Blocks in Omega | Asked in | Their last answer |
|---|---|---|---|---|
| **Q-018** | The **P1 manifest change**: `tau-packs.bin` as a user-selectable file (new layout role/key in `tau-compat.json`, schema, and the exact key), with `check-card` validating format and ABI instead of a hash | writing a user's meter bundle to the card (phase D); until then Omega leaves the file alone | 005 | decided P1 (D-O01); manifest change "open for us" |
| **Q-019** | The **pack library**: one `.tmpk` per meter with a manifest (key, id, slot, ABI, SHA-256, size) as a release asset | the meter library view and composing bundles from Tau's packs (phase C/D) | 005, 006 | accepted in principle, not built |
| **R-014** | The built **`meter_lab.html`** at the root of a lab drop (hash in `MANIFEST.json`) | the real lab in the Meter Lab (phase E) | 004 | **done**: the drop of the v0.7.0 release is `pre-0.7.0-2db91494` (Tau Alpha 014: contract 1, lab sources unchanged since), pinned and installed in Omega 0.6.0 |
| **R-017** | **`lab-data.json`** (meters schema, themes, accent palette) in the lab drop | the preset builder (real parameters, ranges, enum names, defaults) and the `METR` writer in use (phase B) | 005 | accepted; same drop, not built |
| **Q-023** | The pack ABI **minor ceiling** per release (for example `requires_omega.packs_abi_minor`), or where else to read it | composing a bundle the card's firmware will not refuse (Omega assumes 1.2) | 006 | not answered yet |
| **R-024** | One real **support Report** screenshot of the normal core (67 rows) | proving the partial-Report decoding on a real capture | 006 | not answered yet |
| **F-023** | The result of testing a **`count = 0` bundle** on a Pocket, and which is right for "Winamp Bars only": no file, or an empty bundle | the "no extra meters" choice | 005, 006 | "not tested yet" |
| **Q-021** | The firmware applying `METR` **`flags`/`order`** (hide, reorder) | writing anything but default `flags`/`order` | 005 | scheduled (D-O03), not in v0.7.0 unless added |
| **F-005** | **When v0.7.0 will publish**, and the **final `tau-compat.json`** (and a second rehearsal against it) | Omega 0.6.0 must exist first (`min_omega 0.6.0`); Omega re-checks the final manifest | 003, 006 | gate stated; final manifest at the release |
| **R-025 (delivered 2026-11-07, pinned)** | A **new lab drop** at/after `696ff9ad`: built `meter_lab.html`, `lab-data.json` (registry, ranges, defaults, factory presets, themes), every `*_wasm.js` with its SHA-256, adapters | the preset editor (real parameters), the lab preview of presets, Omega 0.6.0 (owner: 0.6.0 includes the editor and the new lab) | 007 | not answered yet |
| **Q-026** | Does "seeded when missing" (B-982) mean per meter folder (Omega's rule) or per file? | confirming the install rule for `tau-presets/` | 007 | not answered yet |
| **Q-027** | Slot -> meter lookup for `MPRE`/`MPID` (built-in slot 0 + pack descriptors) is the intended way? | naming the chosen preset in the Settings screen | 007 | not answered yet |
| **Q-031** | How does the lab page accept a meter and a preset's values from the host (postMessage, URL hash)? | the live preview of a preset in the editor | 007 | not answered yet |
| **A006/R-024, R-025, Q-026** | Alpha message 006 (ids clash with ours, see `to-alpha/008`): name sidecar, card name rules, does Omega build the index | nothing: **built, answered (`to-alpha/008`) and run on a Pocket with a card Omega wrote 2026-11-07** (`NAMES STATE 1`, `NAMES ERR 0`; `to-alpha/013`) | 013 | pending |
| **A007/R-032** | Every meter's WebAssembly module in Omega's preview | waits for the lab drop (R-025 ours) | 007 | pending the drop |
| **A007/Q-034, Q-035** | Preset id numbering; show or check the module provenance | answered in `to-alpha/008` (no reserved range, hidden is Omega-only; show it, put SHA and clang in `MANIFEST.json`) | 008 | pending |
| **O014/R-1 to R-5** | Omega's asks about the lab: embed mode, options message, the detail area as data, per-meter view data, a per-frame cost message | nothing: **built by Tau Alpha (their 012, embed contract 1) and wired in Omega 2026-11-07** (`to-alpha/015`); their one fix (O015/F-1, `class`) is in the drop `pre-0.7.0-2db91494`, pinned 2026-11-08 | 015 | answered |
| **O014/Q-1, Q-2** | Is Winamp Bars slot 0 of the 8? When does the P1 key for a user-chosen `tau-packs.bin` (and the pack library) exist? (the design-only "Meters on the Pocket" page waits for it) | Q-1 answered (seven rows, Winamp Bars is row 0; Omega draws it so). **Q-2 still open**: a timing decision for the owner, no date (their 012) | 015 | Q-1 answered, Q-2 pending |
| **A015/R-1** | Let `tau-core` compile for WebAssembly (disk-space calls desktop-only) so Tau Alpha's Pocket Sync plugin can use it by git tag | nothing: **done 2026-11-11** (D-023, owner's yes): `8592d08`, tag `tau-core-wasm-1` (published with the owner's push) | 016 | answered; tag `tau-core-wasm-1` published 2026-11-11 (`to-alpha/017`) |
| **A016/R-1** | Make `tau-core`'s scan run on WebAssembly (it used threads, which WASI lacks) and give the plugin a new tag | nothing: **done 2026-11-11**: one sequential path when the scan has one thread (one thread on WASI), tests run in Node's WASI host in CI; tag `tau-core-wasm-2` (published 2026-11-11, `to-alpha/019`) | 018 | answered; tag `tau-core-wasm-2` published |
| **note** | Their `main` on GitHub holds only message `to-omega/001`; messages 002 to 005 (and the lab drop, rehearsal) exist only in their local checkout | Omega reads them from the local checkout on this machine; nobody else can | this file | their call to push |

## Owed by Omega (nothing from Tau Alpha needed)
- Omega 0.6.0: version bump, notes, release (gate for v0.7.0's `min_omega`).
- Second real-card rehearsal with both v0.7.0 zips on the owner's `Pock` (owner's go needed: it is a card write).
- Wire `METR` into the assets install plan (like `PRST`) and build the UI once R-017 arrives; the meter library once Q-019 arrives.

## Closed since the last list
R-011 (`required` rule, aligned), Q-010 (no CARDWRITE run), D-007 (lab drop approved and pinned), Q-016 (sandbox `allow-scripts` only), Q-020 (8 slots), Q-022 (`METR` replaces the whole set), F-009b (real-card rehearsal).
