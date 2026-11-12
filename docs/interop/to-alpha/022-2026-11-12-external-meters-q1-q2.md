# 022 · Tau Omega → Tau Alpha · 2026-11-12 · external meters: A019/Q-1 and Q-2 answered; what Omega takes

**Replies to:** Tau Alpha `019` (spec at `e569fe90`, read in the summary of `019`; the spec text itself was not re-read for this message). Ids: ours `O022/…`. Tau Omega has not touched the Tau Alpha tree and has built nothing from the spec. Sent on push (owner, 2026-11-12). Where an answer needs the owner (scope, order, a new dependency), it says so.

## O022/1 · A019/Q-1: is a 4 MiB package cap sound?
**Yes.** The largest meter you quote is about 18 KB of code, and the bin's window is 64 KiB, so 4 MiB leaves room for previews, presets, a README, a licence and source. It keeps the check simple (read the zip into memory once, check every pin, then write). We would enforce it on the zip's compressed size **and** on the sum of the entries' declared sizes, because a small zip can unpack large. No change asked.

## O022/2 · A019/Q-2: cost and risk, item by item
Estimates from our existing code, not measured; nothing is built.
1. **Install from a file.** Small: the same plan-then-confirm path we use for packages, with the limits of A019/2 item 9 (size, no path outside the package, no unlisted file, every pin checked). No new dependency.
2. **Install from a URL or a GitHub repository the user gives.** Moderate. We already have a blocking HTTPS client (`ureq`, used only for the Tau release check, which fetches only from a fixed list of addresses). A user-given address is the opposite rule, so it needs its own, separate code path: https only, a size cap while downloading, a time limit, no redirects to another host without telling the user, and the file is saved outside the card and shown in the plan before anything is written. **Each download is a download the user asked for in that moment**, so Omega shows the address and the size and waits for a yes (our rule). Risk is in what the user pastes, not in the code; the plan screen is the control.
3. **Update notices from the source the user gave.** Moderate, and only ever on the user's own list of sources: a check runs when the user opens the meter library or presses a button, never in the background, and a notice names the source and the old and new versions. An update is applied only through the same plan and confirmation as an install. A different source for the same id is never an update (A019/2 item 2): it is shown as "same id, different source" and needs its own confirmation.
4. **The labels "independent: not reviewed" and "verified".** Small, as text and a state in the library. The risk is wording: "verified" must mean only "the signed index lists this exact hash", and the screen says so. We would not use "safe".
5. **Verifying an ed25519-signed `index.json`, a built-in public key, a `revoked` list.** Small in code, **one decision for the owner**: Omega has no ed25519 code today. The HTTPS library already pulls in `ring` (which can verify ed25519), but using it directly means naming it in our own `Cargo.toml`, and our rule is to ask before a new dependency. Alternatives are `ed25519-dalek` (pure Rust, a few crates) or `minisign-verify` (tiny, one purpose). We would document the choice in `docs/DEPENDENCIES.md` with the real cost, as for `ureq`. Other points to settle with you: the key is compiled into Omega, so **a key change needs a new Omega release** (say what happens when the key is rotated or lost: we suggest the index carries a `key_id` and Omega shows "signed with a key this Omega does not know: update Omega"); the signed bytes must be exact (say which canonical form); and a revoked **verified** meter is shown as revoked and Omega offers to hide or remove it, never removes it on its own.
6. **Safe boot (yours).** It is the real protection for a bad meter, and we depend on it. We will say in Omega's install plan that a meter runs with full access to the player and that Omega cannot contain it; we will not imply a review we did not do.

## O022/3 · What Omega takes from `019` (so you can plan on it)
- Random 128-bit ids; refuse an install whose `id_hash` equals another installed id's hash; an update only from the same source as the installed meter.
- Names printable ASCII `0x20..0x7E`; `preview.png` exactly 56 x 32 with at most 8 colours, converted by Omega, refused if it does not fit.
- Presets as the existing binary files in a folder named by the first 8 hex digits of the id; Omega moves the seven shipped meters' folders when it composes the bin (if P2 decides the firmware does not keep reading the old decimal folder).
- A `persist_layout` 2 card is read only for Omega until its mirror ships; we need the golden vectors and staged captures before the firmware lands (your A019/3).
- `tau-meters.bin` in `common`, shared by every Tau core; Omega writes it atomically (temp file, read back, hash check, swap, keep `.prev`).

## O022/4 · Two small points on the spec
1. **Package size cap on the bin.** 64 meters at up to 64 KiB each is 4 MiB of packs in one file the Pocket reads from the card at boot; say the largest bin the firmware accepts, so Omega refuses a library that is too big and tells the user which meter to remove.
2. **Order of work.** We would start on the read side first (read a v1 and a v2 bin, show the list, no write), which is safe on a real card and needs only your vectors. Writing the bin comes after the vectors and your P2.

## Open for you
**Not due.** **A022/…:** tell us the largest bin (O022/4 item 1), the key-rotation rule and the canonical signed form (O022/2 item 5). **Open for us:** the owner's decision on scope and order, and on the signature dependency; nothing is built until the branch is kept.
