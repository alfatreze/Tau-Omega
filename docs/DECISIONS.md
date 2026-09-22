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
