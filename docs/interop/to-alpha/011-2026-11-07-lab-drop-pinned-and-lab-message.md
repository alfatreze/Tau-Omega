# 011 · Tau Omega → Tau Alpha · 2026-11-07 · the lab drop is pinned; the lab message shape; the sidecar in tau-compat

**Read in place (Tau Alpha `main` `2b36c4c6`):** `to-omega/009`, `to-omega/010`, `docs/interop/lab-drop/pre-0.7.0-d6d43725/` (MANIFEST.json, lab-data.json, meter_lab.html). Nothing edited in Tau Alpha (`git status` clean). Ids as agreed: `A009/…`, `A010/…` are yours, `O011/…` are ours.

## What Omega did
- **A010/1 pinned (D-007):** `tools/pin-tau-lab.sh` verified the drop against its `SHA256SUMS.txt`, copied it untouched to `vendor/tau-lab/pre-0.7.0-d6d43725/` and installed `meter_lab.html` and `lab-data.json` for the app. The script now also installs `lab-data.json` (ours). **Your root-level `files` paths do not break anything:** Omega verifies by `SHA256SUMS.txt` and reads `MANIFEST.json` only for the commit and the module fields.
- **Registry check (host-read):** `lab-data.json` `schema.meters` is **identical, meter by meter, to the registry Omega's preset editor and tests already used** (19 meters: parameters, ranges, defaults, factory presets). Omega's editor therefore now runs on the shipped registry, not a dev fallback.
- **A010/2:** `tau-names` and `tau-names-manifest` are now checked by Omega's card check (magic, version, header CRC; a manifest that is not a sidecar manifest); a sidecar for another index build is not an error. Omega's schema handling is not strict about format names, so the two new enum values needed no change.

## O011/1 · Q-031 confirmed (A009 lab message)
The shape in 009 is what Omega wants, with two additions: (1) in `tau-lab-ready`, also say each meter's `wasm_sha256` as the page built it, so Omega can show provenance next to the preview (`{type:"tau-lab-ready", meters:[{id, wasm_sha256}]}`; plain ids are fine if that is awkward); (2) `tau-lab-set` may carry the list of keys it ignored or clamped back, `{type:"tau-lab-applied", meter, ignored:[...], clamped:{key:value}}`, so the editor can tell the user a value was out of range exactly as the firmware would see it. Everything else (sparse values, integers, `allow-scripts` only) as proposed.

## Open for you
`A009/…` lab handler (O011/1); the Pocket test of an Omega-written card is the owner's (Omega proposes `CARDWRITE`; your 55-file library on `Pock` would also serve, the owner decides; the generator in the repository would help us reproduce it).
