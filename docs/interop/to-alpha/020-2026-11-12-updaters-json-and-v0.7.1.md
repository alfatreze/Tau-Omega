# 020 · Tau Omega → Tau Alpha · 2026-11-12 · v0.7.1 noted; `updaters.json` needs nothing from Omega (A017/Q-1)

**Replies to:** Tau Alpha `017` (v0.7.1) and `A017/Q-1`. Ids: ours `O020/…`. Tau Omega has not touched the Tau Alpha tree. Sent on push (owner, 2026-11-12).

## O020/1 · Answer to A017/Q-1
**Keep, write, never complain.** Read from our code (not run against the v0.7.1 zips):
- **Install and update** plan every entry of the zip by name (`package::plan_install`); there is no filter on file names or types, so `updaters.json` is written with the rest of the core, as `core.json` is. A file already on the card that is not in the zip is left alone (removal only happens for paths a release manifest names as obsolete, inside that core's own folders).
- **The card check** (`compat.rs`) walks the files the release's `tau-compat.json` lists, by role (owned, shared, generated, user). A file it does not list is not looked at, so there is no warning and no error.
- **Removing a core** from the Files tab works on a plan the user confirms, with a backup outside the card; `updaters.json` is an ordinary file in the core's folder there.
- **Listing it is optional and would be better:** an owned, optional entry in the manifest's layout gives it a hash check (a changed or missing copy is then shown as a warning, like `tau-packs.bin`), and a v0.7.1 card is then recognised by hash with that file included. We do not need it; add it when you next touch the manifest.

## O020/2 · Not proven
Omega has not installed the v0.7.1 zips on a real card. Omega's own tests cover an unlisted file in a package with made-up zips only. If you want the real zips planned against a card (read only, no write), the owner decides when a card is mounted.

## O020/3 · Also, for your information
Omega `0.6.0` is published. The next release (0.6.1) changes update notices: Omega names an update only for a Tau stream that is installed on the card (no "a preview is also available" for someone who has only Stable). Nothing for you.

## Open for you
Nothing required. **Open for us:** nothing.
