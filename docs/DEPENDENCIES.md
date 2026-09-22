# Dependencies

`tau-core` is deliberately kept to a handful of small, permissive crates. It has to be embeddable in
another application (see `DECISIONS.md` D-009), and **every dependency here becomes a constraint on
every future host**. Tag parsing, cover handling and the index codec are hand-written rather than
pulled in, which is also what keeps the index byte-exact against the Python reference.

Adding one needs a reason that survives that test. `ARCHITECTURE.md` originally surveyed ~20 crates
(tokio, rusqlite, lofty, image, tracing, walkdir …); none were adopted and the lean result is the
better outcome — do not "restore" that list to match the survey.

## `tau-core` (the portable engine)

| Dependency | Purpose | Licence / review |
|---|---|---|
| `crc32fast` | Index CRC32 header and body fields | MIT OR Apache-2.0; small, widely used. |
| `sha2` | SHA-256 for sync verification, duplicate grouping, plan tokens and backup checks | MIT OR Apache-2.0; RustCrypto, widely used. |
| `serde_json` | Read-only `core.json` / `data.json` / `interact_persist.json` parsing | MIT OR Apache-2.0; small, widely used. |
| `serde` (optional, feature `serde`) | `Serialize`/`Deserialize` on every public data type (`PORTABILITY_AUDIT.md` P1-1, done 2026-09-22) | MIT OR Apache-2.0; already an implied dependency of `serde_json`, which every consumer of this crate already builds. Not in the default feature set: verified the default `cargo build -p tau-core` does not compile `serde`/`serde_derive`/`syn` at all, vs. `cargo build -p tau-core --features serde`, which does. |

`ErrorCode` and the fieldless enums (`WarningCode`, `CopyState`, `DifferenceState`, `IndexStatus`,
`Stage`) do not use the derive's default wire shape: `ErrorCode` has a hand-written
`Serialize`/`Deserialize` pair so it stays the plain `u16` every hand-written boundary (CLI exit
codes, the Tauri adapter's former `ApiError`) already used, and the fieldless enums use
`#[serde(rename_all = "snake_case")]` so they match `WarningCode::as_str()` and the hand-written wire
strings (`"only_left"`, `"new"`, …) that predate this feature, rather than the derive's own PascalCase
default. Checked by `crates/tau-core/tests/serde_feature.rs` (only compiled with `--features serde`).

## Front-ends (not part of the portable engine)

| Dependency | Purpose | Licence / review |
|---|---|---|
| `tauri` 2, `tauri-plugin-dialog` | Desktop shell and native file pickers | MIT OR Apache-2.0. Ships in the macOS bundle today. |
| Svelte, Vite, TypeScript, `@tauri-apps/api` | UI build and Tauri bridge | MIT. |

`tau-cli` has no dependency beyond `tau-core`, which is the check that the engine really is
front-end independent — if the CLI ever needs something the engine can't give it, that is a signal.

## Licence boundary

Tau Omega is **MIT OR Apache-2.0**. Integration targets are copyleft (Pocket Sync is AGPL-3.0), so
code may flow from here to there but **never the reverse** — see `DECISIONS.md` D-010.
