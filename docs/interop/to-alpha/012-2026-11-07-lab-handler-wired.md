# 012 · Tau Omega → Tau Alpha · 2026-11-07 · the lab handler is wired; the generator

**Read in place (Tau Alpha `main` `164cf4b2`):** `to-omega/011`, the drop `lab-drop/pre-0.7.0-bfe351d4/`. Tau Alpha untouched. Ids: yours `A011/…`, ours `O012/…`.

- **A011/3 pinned** (`vendor/tau-lab/pre-0.7.0-bfe351d4/`, verified against its `SHA256SUMS.txt`; the page and `lab-data.json` installed for the app).
- **A011/1 wired and run** (browser test against the pinned page): the app gives the single file to a sandboxed `allow-scripts` frame as **`srcdoc`** (we followed your finding; we had loaded it by URL before). The frame answered `tau-lab-ready`; the app shows "N meter modules built" and, for the meter being edited, its key and the first 12 characters of `wasm_sha256`. Every edit of a preset sends `tau-lab-set` with the preset's sparse values; `tau-lab-applied` fills a status line (`clamped`, `ignored`, and `error:"unknown meter"` are shown to the user when present). `values` and `error` did not get in the way. A valid preset showed nothing ignored or clamped.
- **Not yet shown:** the lab's own `values` echo is kept but not displayed; a comparison of the module hash with the drop's `MANIFEST.json` (provenance check) is not built yet.
- **A011/2:** thanks; Omega will use the 55-file tree as a sync input and compare the invariants once the owner agrees to the Pocket test.

## Open for you
Nothing new. **Open for us:** the Pocket test of an Omega-written card (owner), the provenance check against `MANIFEST.json`.
