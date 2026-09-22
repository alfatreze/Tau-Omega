# Decisions

## D-001 — portable engine boundary

The Rust workspace keeps card inspection, scan, tag parsing, index encoding and verification in `tau-core`. The Tauri layer only exposes read-only commands and the Svelte UI only renders their results. This preserves the selected frontend-independent architecture.

## D-002 — safety boundary for T0/T1

No card-writing engine is present. The only CLI writes are an explicitly named local index or synth output, both requiring `--yes`; sync, move, delete and install remain unavailable until their plan/confirmation model exists.

## D-003 — initial visual language

Start with one high-contrast mint action accent and an intentionally sparse Cards screen. Pocket-edition accent choices are deferred until each pair is contrast-tested.

## D-004 — T2 confirmation and recovery

The sync plan is pure and deterministic. Execution requires its exact token; the CLI additionally requires `--yes` and a host-side manifest location. CLI mirroring requires a second matching token and an external verified backup. Baseline JPEG cover embedding is an explicit plan feature whose cover hash is included in the confirmation token; conversion/optimisation, cancellation and filesystem journaling remain unavailable rather than being implemented with weaker safety guarantees.

## D-005 — copy-only cover helper

The portable engine can discover folder covers and embed a baseline JPEG into MP3 or FLAC copies, preserving source bytes. Existing MP3 non-art ID3 frames and FLAC audio/other metadata survive the rewrite. The reviewed sync plan includes the cover hash, so an artwork change expires the plan. PNG, progressive-JPEG conversion and image optimisation are deliberately unavailable: progressive inputs are rejected before a destination copy is written.

## D-006 — host-side sync journal

Every confirmed sync writes a durable JSON job report to the user-selected host path before media copying begins, then replaces it with `completed` or `failed` state. The journal is refused inside the card media root, so it remains available after a card is removed. Mirror jobs use the same lifecycle report while retaining their independent deletion confirmation and backup requirements.

## D-007 — comparison precedes multi-core mutation

The T3 foundation is a portable, read-only media-root comparator. It classifies each supported file by relative path and SHA-256 as identical, left-only, right-only or different. No index or card file is changed. Future copy and move plans will consume this comparison instead of inventing their own duplicate logic.

## D-008 — whole-library core copy before move

The first T3 mutation is a complete media-root copy. It preserves relative paths, verifies each destination file, rebuilds only the destination index with its own root prefix, creates a host-side journal, and never deletes from the source. The portable/CLI move path adds two matching confirmation tokens, an external verified backup, and a source-index rebuild after deletion; it is intentionally withheld from the desktop screen until its explicit deletion review UI is completed. Selection and cleanup remain unavailable.

## D-009 — the portability target is Pocket Sync, and the boundary is a Rust crate

`tau-core` is kept reusable so it can be adopted by **Pocket Sync** (Tauri + Rust + TypeScript, the
established third-party Analogue Pocket manager). Because that host is Rust, the integration boundary
is an ordinary **crate dependency** — not a plugin ABI, not a C ABI, not WASM, not a sidecar process.
Neither Pocket Sync nor any comparable manager exposes a plugin system, so the realistic path is an
upstream contribution or a fork. The engine is therefore designed to be **easy to vendor** — a clean
API and no host assumptions — rather than to be a loadable artefact.

A second candidate host, Feishin (Electron, GPL-3.0, a Navidrome/Jellyfin/Subsonic streaming client),
was assessed on 2026-09-22 and **dropped**: no local files to index, no plugin API, no Rust runtime,
and little audience overlap. Non-Rust hosts, and any abstraction over remote media sources, are out
of scope until a concrete second host exists. The `tau` CLI's JSON output stays the fallback surface
for a non-Rust consumer, but nothing is designed around that today.

## D-010 — licence boundary with integration targets

Tau Omega stays **MIT OR Apache-2.0**. Pocket Sync is **AGPL-3.0**. Permissive flows one way: our code
may be incorporated into theirs, and the combined work is theirs to distribute under AGPL. **Code,
snippets and structure must never be copied from an AGPL or GPL project back into this repository** —
that would relicense Tau Omega by contamination. This is the same constraint tau-alpha already lives
under with GPL RTL. Integration work is exactly when this gets crossed by accident, so it is recorded
here rather than assumed.

## D-011 — boundary correctness before more features

The 2026-09-22 portability audit (`PORTABILITY_AUDIT.md`) found the engine internals sound and the
gaps all at the boundary: domain rules duplicated in both front-ends, English prose used as the API,
and no progress or cancellation. Those three are prerequisites for any host adoption and get fixed
before new feature work, ahead of the feature order in `STATUS_HANDOFF.md`.

## D-012 — assessed candidate hosts, and why player features come to us instead

Register of hosts considered, so none is re-litigated. Neither rejection was technical.

| Candidate | Stack | Verdict |
|---|---|---|
| **Pocket Sync** | Tauri 2 + Rust + TS, AGPL-3.0 | **Target** (D-009). Same device, same users, real adoption. |
| **Feishin** | Electron + React, GPL-3.0 | Dropped 2026-09-22: streams from Navidrome/Jellyfin/Subsonic, so no local files to index; no Rust; no plugin API; little audience overlap. |
| **awesome-music-player** (S1avv) | Tauri 2 + Rust + React, `lofty`, MIT | Assessed 2026-09-22, **not adopted as a host** — but kept as a reference, below. |

`awesome-music-player` is the **best technical fit of the three** — our own stack, offline-first local
files (MP3/WAV/FLAC/M4A/OGG), and a tag pipeline of the same shape — yet the weakest project: 28
stars, 22 commits, three releases in a four-day burst (6-9 June 2026) and quiet since. It has no
plugin API and none on its roadmap, whose third-party plans are Last.fm, a remote-control protocol
and a proprietary cloud backend. So adoption would mean an upstream PR or a fork, into a project with
unproven maintenance and effectively no audience.

**The structural reason none of these worked as a host**, worth keeping because it applies to the
whole "plugin in a media player" idea and not to any one candidate: a Pocket SD-card sync feature is
niche for a *general-purpose* player's users, almost none of whom own an Analogue Pocket. The benefit
accrues to our users, not theirs, so the upstream case is weak on merit no matter how easy the
integration is. Pocket Sync is the exception precisely because its users are Pocket owners.

**The useful direction is inbound.** `awesome-music-player` is MIT, which makes it the one candidate
we may legally borrow *from* — the reverse of the AGPL/GPL projects in D-010. Tau Omega's own SPEC
already wants an in-app preview player (extensions list, item 16), and that is where this gets used:
as a reference implementation for adding player features here, rather than as a host to plug into.

**If code is actually copied, MIT still requires attribution** — carry the upstream copyright notice
and licence text. D-010 says never copy *from* copyleft; this is the complement: copying from a
permissive project is allowed *with* its notice preserved, not silently.
