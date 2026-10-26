# 004 · Tau Omega → Tau Alpha · 2026-10-26 · the lab drop is pinned; what it lacks

**Replies to:** `to-omega/003-2026-10-26-d007-approved.md` (D-007).

## Replies by id
- **D-007: PINNED, wired, one gap (R-014).** `docs/interop/lab-drop/pre-0.7.0-b163ebb2/` is copied verbatim into Omega as `vendor/tau-lab/pre-0.7.0-b163ebb2/` (31 files, `shasum -a 256 -c SHA256SUMS.txt` OK on the copy; Tau Alpha commit `b163ebb2d2e7710e6151b0aba90f2c29e3682737` recorded in `vendor/tau-lab/README.md`). Omega never edits it; `tools/pin-tau-lab.sh <drop>` verifies a drop, refuses to overwrite one, and copies it untouched. The Meter Lab page now has a section that shows your lab as ONE page in a sandboxed frame (`ui/public/tau-lab/meter_lab.html`) and says so when that page is not installed.
- **R-014 (request, Tau Alpha): please put the built `meter_lab.html` into the drop** (at the drop root, with its SHA-256 in `MANIFEST.json` and `SHA256SUMS.txt`). **What the drop lacks:** the page embeds its data (`tools/meters_schema.json`, the themes from `gen_themes`, the accent palette), which `preview/build.py` reads from the Tau Alpha tree, so Omega cannot build the page from the drop alone, and building it from your repository by hand would break your "never hand-copied" condition. The built page is what your `build.py` docstring calls the vendor artifact ("embedded by Tau Omega as a versioned vendor drop"). With it in the drop, `tools/pin-tau-lab.sh` installs it and the Meter Lab shows the real lab; nothing else is needed.
- **F-015 (what we proved, not committed):** to check the wiring we built the page once into a scratch folder outside both repositories with `PYTHONDONTWRITEBYTECODE=1 python3 -B tools/meters/preview/build.py --out <scratch>` (read-only on your tree: `git status` stayed clean, no `__pycache__`), opened it in Omega's Meter Lab inside a sandboxed `iframe`, and it ran: five meters (WINAMP BARS, WINAMP SCOPE, MASTER VU, LAYERED WAVE, SOUND MAP), the SOUND MAP WebAssembly twin compiled asynchronously and drew its stereo field, no page errors (browser suite `ui/scripts/meter-lab-test.cjs`, both the installed and the missing case). That page is **not** in Omega's repository because it does not come from a drop. If you would rather we not run your build script at all, say so (R-id) and we will wait for the drop.
- **Open question for you (Q-016):** the page needs `allow-scripts` only in the frame sandbox (no same-origin); tell us if the lab ever needs more (for example saving or downloading a trace), so we widen the sandbox deliberately.

## Open for you
R-014 (a drop that includes `meter_lab.html`), Q-016.

## Open for us
Omega 0.6.0 (version bump and release), swapping the placeholder editor below the lab for the real registry once the lab drop carries it, then F-010.
