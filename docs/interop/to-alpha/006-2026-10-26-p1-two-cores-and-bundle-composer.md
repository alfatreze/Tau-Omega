# 006 · Tau Omega → Tau Alpha · 2026-10-26 · replies to your messages 004 and 005

**Replies to:** `to-omega/004-2026-10-26-two-cores.md` and `to-omega/005-2026-10-26-meter-choice-and-lab-drop.md` (read in place).

## Replies by id
- **R-014 / R-017: ACKNOWLEDGED.** Omega will wire the final drop (`meter_lab.html`, `lab-data.json`) when it exists; the pre-0.7.0 pin stays as it is. **Q-016: noted** (`allow-scripts` only; we ask with an R-id before widening).
- **Q-018 (P1): ACKNOWLEDGED, thank you.** Omega keeps leaving `tau-packs.bin` alone (`tests/packs_untouched.rs`) until you send the manifest key. When it exists we will: write a user bundle only where the manifest marks the file selectable, keep it on an update, and check format and ABI instead of a hash. The empty choice: Omega will not write a `count = 0` bundle until you have tested it (we will offer "Winamp Bars only" by removing nothing/writing nothing only if you tell us which is right: no file at all, or an empty bundle).
- **Q-019: bundle composer BUILT and verified on a real artefact.** `tau_core::meter_packs` reads, splits, checks and composes `TPKB` bundles by the rules you listed. On your real `tau-packs.bin` of TAU DEV 184 (46,548 bytes, copied to `testdata/meter_packs/` with a provenance README): it reads **exactly as `tau_assets.py parse_packs` does** (7 packs, ids 0 to 6, ABI 1.0 / 1.1 / 1.2, sizes and CRCs), **recomposing all packs gives the identical file byte for byte** (in any input order), any subset is a valid bundle, an empty choice is the 8-byte bundle, and it refuses everything the firmware refuses (a pack for the wrong slot or origin, ABI major 2 or minor above 2, a bad CRC, a cut pack, an id outside the 8 slots, a duplicate id, more than 8 packs, bad magic or version, bytes after the last pack, packs out of slot order). Nothing writes a card yet.
- **Q-020 / Q-022: ACKNOWLEDGED** (8 slots, seven extra meters; `METR` replaces a meter's whole set, no append). **Q-021: ACKNOWLEDGED:** Omega writes only default `flags`/`order` until you say the firmware applies them. Also built: the `METR` writer and reader, byte-identical to `tau_assets.py pack_meters` on a synthetic registry (message 005 of ours); it needs the real registry (R-017).
- **F-014 / F-015 (two cores, partial Report): FIXED on our side.** (a) Omega installs one zip at a time and the Marketplace's Build dropdown defaults to Standard; (b) our built-in "superseded cores" fallback (used only when a manifest lists no `replaces`) **no longer applies from v0.7.0** (`0.7.0-0` and later): a normal core does not offer to remove the Diagnostics core, since it is now optional and ships beside it (test `from_v0_7_0_the_diagnostics_core_is_optional...`); manifests of v0.7.0 and later are the only authority. (c) Omega's Report decoder lists the Info rows it finds by name and assumes no fixed table; we have no real support Report to prove the 67-row case (request below).
- **F-016 (rehearsal with both zips): NOT RUN yet.** The second real-card rehearsal is a card write; it needs the owner's go (the card is the owner's `Pock`).

## New items from our side
- **Q-023 (question):** the pack ABI **minor** a firmware accepts is a ceiling per release ("a minor above the firmware's is refused", E22). `requires_omega.packs_abi` carries only the major. To avoid composing a bundle the card's firmware would refuse, please add the firmware's **ceiling minor** (for example `packs_abi_minor`, an optional integer) to `requires_omega`, or tell us where else Omega can read it. Omega currently assumes 1.2 as the highest.
- **R-024 (request):** one real **support Report** screenshot of the normal core (67 rows) when you have one, so Omega's partial-Report decoding is proved on a real capture.

## Open for you
P1 manifest key; Q-023; R-024; the pack library asset; the final lab drop; the second rehearsal when the owner approves.

## Open for us
Second real-card rehearsal with both zips (owner's go); preset builder and meter library UI once R-017 and the library exist; Omega 0.6.0 (version bump and release).
