# Pinned Tau Alpha meter lab (vendor drops)

Approved exception to "never share literal files" (Tau Alpha interop **D-007**, owner, 2026-10-26; see `docs/interop/`). Conditions, as Tau Alpha stated them:
- a folder here is a **verbatim drop** made by Tau Alpha's `tools/make_lab_drop.py`, never hand-copied, never edited (to change it, ask for a new drop);
- it keeps its `MANIFEST.json` and `SHA256SUMS.txt`; verify with `cd <folder> && shasum -a 256 -c SHA256SUMS.txt`;
- Omega records which Tau Alpha commit each drop pins (below). The committed WebAssembly module (SOUND MAP twin) is part of the exception.

| Folder | Tau Alpha commit | State |
|---|---|---|
| `pre-0.7.0-b163ebb2/` | `b163ebb2d2e7710e6151b0aba90f2c29e3682737` | pinned 2026-10-26 (29 files verify; a `pre-*` drop to build against; the final drop comes with v0.7.0) |

**What is wired and what is missing.** The Meter Lab page (`ui/src/lib/RealMeterLab.svelte`) shows the lab as ONE page, `ui/public/tau-lab/meter_lab.html`, in a sandboxed frame; when that file is absent it says so. The drop carries the lab's scripts but **not the data the page embeds** (`tools/meters_schema.json`, the themes and the accent palette are read from the Tau Alpha tree by `preview/build.py`), so the page cannot be built from the drop alone: Omega asked Tau Alpha to add the built `meter_lab.html` (with its SHA-256 in `MANIFEST.json`) to the drop (interop message `to-alpha/004`). A scratch build of that page, made outside the repository, was shown working in Omega's Meter Lab (five meters including the SOUND MAP WebAssembly twin) so the wiring is proven; it is not committed because it does not come from a drop.

`tools/pin-tau-lab.sh <drop folder>` pins a new drop (verifies it, copies it here untouched, installs its `meter_lab.html` for the page).
