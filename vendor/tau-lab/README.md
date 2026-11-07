# Pinned Tau Alpha meter lab (vendor drops)

Approved exception to "never share literal files" (Tau Alpha interop **D-007**, owner, 2026-10-26; see `docs/interop/`). Conditions, as Tau Alpha stated them:
- a folder here is a **verbatim drop** made by Tau Alpha's `tools/make_lab_drop.py`, never hand-copied, never edited (to change it, ask for a new drop);
- it keeps its `MANIFEST.json` and `SHA256SUMS.txt`; verify with `cd <folder> && shasum -a 256 -c SHA256SUMS.txt`;
- Omega records which Tau Alpha commit each drop pins (below). The committed WebAssembly module (SOUND MAP twin) is part of the exception.

| Folder | Tau Alpha commit | State |
|---|---|---|
| `pre-0.7.0-bfe351d4/` | `bfe351d4e8df4a61e553675967d662077244a0c0` | pinned 2026-11-07 (verifies; adds the host message handler `tau-lab-set` / `tau-lab-ready` / `tau-lab-applied` and `preview/tau_host.js`; **this is the drop the app installs**) |
| `pre-0.7.0-d6d43725/` | `d6d43725f6da4a5835a054049718029f8b584da4` | pinned 2026-11-07 (verifies against its SHA256SUMS.txt; **all eight WebAssembly modules**, the built `meter_lab.html`, `lab-data.json`, module provenance in `MANIFEST.json`; installed for the page and the preset editor's registry) |
| `pre-0.7.0-b163ebb2/` | `b163ebb2d2e7710e6151b0aba90f2c29e3682737` | pinned 2026-10-26 (29 files verify; a `pre-*` drop to build against; the final drop comes with v0.7.0) |

**What is wired and what is missing.** The Meter Lab page (`ui/src/lib/RealMeterLab.svelte`) shows the lab as ONE page, `ui/public/tau-lab/meter_lab.html`, in a sandboxed frame; when that file is absent it says so. The drop carries the lab's scripts but **not the data the page embeds** (`tools/meters_schema.json`, the themes and the accent palette are read from the Tau Alpha tree by `preview/build.py`), so the page cannot be built from the drop alone: Omega asked Tau Alpha to add the built `meter_lab.html` (with its SHA-256 in `MANIFEST.json`) to the drop (interop message `to-alpha/004`). A scratch build of that page, made outside the repository, was shown working in Omega's Meter Lab (five meters including the SOUND MAP WebAssembly twin) so the wiring is proven; it is not committed because it does not come from a drop.

`tools/pin-tau-lab.sh <drop folder>` pins a new drop (verifies it, copies it here untouched, installs its `meter_lab.html` for the page).

**Provenance check (built 2026-11-07, Tau Alpha Q-035).** `tools/pin-tau-lab.sh` also installs the drop's `MANIFEST.json` beside the page (`ui/public/tau-lab/`). When the lab reports the modules it compiled (`tau-lab-ready`), the Meter Lab compares each module's `wasm_sha256` (and Layered Wave's timed variant) with the manifest (`ui/src/lib/meters/provenance.ts`) and says so under the lab: all hashes match, or which module differs, is not listed, or was not reported; the edited meter also shows the compiler and the number of source files from the manifest. Without a manifest it says the hashes are not checked. Browser suite: `ui/scripts/meter-lab-test.cjs` (real drop matches; a tampered manifest is caught by name).
