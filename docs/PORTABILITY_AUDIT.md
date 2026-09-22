# Portability audit — 2026-09-22

Audit of the engine against the stated goal: **`tau-core` must be reusable inside Pocket Sync**
without a rewrite. Findings are evidence-backed against the tree at the time of writing.

Scope note: an earlier candidate target (Feishin, an Electron/GPL streaming client with no local
files) was assessed and **dropped**. See `DECISIONS.md` D-009. Nothing below is written for it.

## Verdict

The engine is in better shape for embedding than its own `ARCHITECTURE.md` asks for. Every
significant gap is at the **boundary**, not in the core. Nothing found requires re-architecting.

## What holds up

| Property | Evidence |
|---|---|
| No runtime lock-in | `tau-core` depends on `crc32fast`, `serde_json`, `sha2`. No tokio, no async, no GUI, no Tauri. |
| No process ownership | No `println!`/`eprintln!`, no `process::Command`, no `process::exit`, no `std::env` in the engine. |
| No hidden state | No `static mut`, no `lazy_static`/`OnceLock`, no `thread_local`, no thread spawning. |
| Front-end independence is real, not claimed | Two front-ends already consume the engine (Tauri app + `tau` CLI). |
| The safety model is already portable | `plan()` returns a plain `SyncPlan` value; `execute()` takes it plus a token. This is a data contract, not a UI flow — better than the docs describe it. |
| The data contract is pinned | Byte-exact conformance against the Python reference, plus a corruption suite asserting firmware E-codes. |
| Index parsing is defensively ordered | `parse()` length-checks the header first, then validates magic/version/size, then **verifies the body CRC before any offset-driven traversal**, mirroring the firmware loader. |

Byte-level `try_into().unwrap()` calls in the tag and cover parsers were individually checked and
are all guarded by prior length checks — infallible by construction, not latent panics.

## Findings

### P0-1 — domain logic has leaked into both front-ends

The rule mapping a media root to its card path prefix (`/Assets/<platform>/common/`) is implemented
**twice**: once in `tau-cli` (`fn prefix`) and again inline in the Tauri adapter. The adapter
additionally decides `library_capable` presentation and the `"Index ready"` / `"No index"` status,
and reads/parses the index itself to count tracks.

Why it matters: this is a domain rule that determines paths written into the index. With two hosts
it is already duplicated; Pocket Sync would make three copies, and any divergence between them
corrupts real cards. It also contradicts decision D-001.

**Fix:** move root-prefix derivation and core/index status into `tau-core`. Front-ends call, never
re-derive.

### P0-2 — the engine speaks English prose as its API

`SyncPlan.warnings`, `SyncReport.warnings` and `Scan.warnings` are `Vec<String>` of human sentences.
`TauError` carries a numeric code, but **`TauError::code()` is called only by the conformance test** —
never by the UI, the CLI, or the Tauri adapter. All 34 boundary sites flatten errors with
`.to_string()`.

Why it matters: the tests prove the E-codes are correct and the product then throws them away. A
host cannot branch on failure, localise, or render its own styling without regexing English.

**Fix:** structured warnings (code + fields) and error propagation that preserves the code. English
rendering belongs in front-ends.

### P0-3 — no progress, no cancellation

There are no progress or cancellation hooks anywhere in the engine. `ARCHITECTURE.md` states jobs
"are cancellable, emit progress events (files, bytes, current path)" — this was never built.

Why it matters: scanning and hashing a multi-thousand-track library is one long blocking call a host
cannot report on or interrupt. Any host UI freezes. This is the hardest blocker for embedding.

**Fix:** a callback/observer parameter on the long-running entry points (scan, plan, execute).
Trait-object or `FnMut` — no runtime dependency, works for every host shape.

### P1-1 — core types are not serialisable

No `serde` derives on any public type. The Tauri adapter therefore hand-writes **13 `*View` DTO
structs** to get data to its UI, and every future host repeats that work.

**Fix:** `serde` behind an optional cargo feature. Zero cost when disabled; deletes the DTO layer
when enabled.

### P1-2 — a public function panics on caller-supplied input

`build_index` is public and consumes `Entry` values whose fields are all public. It then does
`entries[i].tags["_tno"].parse::<u16>().unwrap()` — a map index plus an unwrap on an internal
invariant that only the engine's own scanner establishes.

Why it matters: a host feeding its own library in — precisely the integration being planned — panics
instead of getting an error.

**Fix:** validate and return `Err`; no public entry point may panic on caller data.

### P1-3 — telescoping constructors

`plan` → `plan_with_options` → `plan_with_features` → `plan_with_layout`, four deep at T2/T3, before
any API is public. Each new capability adds another.

**Fix:** one entry point taking an options struct with `Default`, before the surface is frozen.

### P2-1 — plan token is thin and leaks a phase label

The plan id is a SHA-256 truncated to **32 bits**, formatted `T2-xxxxxxxx`. The hash covers the right
inputs (sources, hashes, cover hashes, deletions, flags, prefix), but truncation is thin for a token
that may be persisted or handed across a process boundary, and `T2-` embeds an internal roadmap phase
label into a durable identifier.

### P2-2 — no fuzz target for the index parser

`parse()` is well-ordered and CRC-gated, and the corruption suite covers realistic damage. A
crafted-but-CRC-valid file driving offsets is not covered, and the `u16_at`/`u32_at` helpers panic
rather than return.

### Addendum 2026-09-22 — fixture fidelity

A follow-up check against the firmware repo (`FIRMWARE_SYNC.md`) found and fixed a live bug that this
audit's method would not have caught: library capability detection never matched a real card, because
the test fixture invented a `data.json` shape instead of copying a real one. The engine's *portability*
was fine; its *fidelity to the platform* was not.

Worth recording next to the findings above, because the two failure modes look alike and are not:
the index path is byte-exact against a Python oracle and was flawless, while the card path was
verified against a fiction. **Any fixture standing in for something the firmware or APF produces must
be derived from a real artefact.**

## Documentation drift

| Doc claim | Reality |
|---|---|
| `ARCHITECTURE.md` module tree (`card/ cores/ scan/ tags/ names/ index/ art/ …`) | Flat files; `lib.rs` is a 1,276-line monolith holding card inspection, ASCII rules, scanning, tag parsing, index build/parse/verify. |
| `ARCHITECTURE.md` dependency list (tokio, rusqlite, lofty, image, tracing, walkdir, …) | Three dependencies. The lean result is **better** than the plan — the doc is what is wrong. |
| "Jobs are cancellable, emit progress events" | Not implemented (P0-3). |
| Five "Extension points" (core registry, versioned index writers, media processors, exporters, card adapters) | None exist as interfaces. These are the portability seams, so they are either work or they are fiction. |

`ARCHITECTURE.md` has been corrected for these. The drift mattered because the plan explicitly drives
a code-generating assistant from these documents.

## Remediation order

Do P0 before adding features; it is cheapest now and all three are prerequisites for any Pocket Sync
work.

1. **P0-1** move leaked domain rules into `tau-core`; both front-ends call it. *Done when* no path
   prefix or index-status logic exists outside the engine.
2. **P0-2** structured warnings and code-preserving errors. *Done when* a front-end can branch on a
   failure without string matching, and `code()` is used in the product, not only in tests.
3. **P0-3** progress and cancellation on scan/plan/execute. *Done when* a long scan reports progress
   and can be cancelled from a front-end.
4. **P1-1** optional `serde` feature; retire the `*View` layer.
5. **P1-2** no panics on caller input at public entry points.
6. **P1-3** collapse `plan_with_*` into an options struct.
7. **P2** widen the plan token, drop the `T2-` prefix, add a parser fuzz target.

## Explicitly not doing

- No plugin ABI, no C ABI, no WASM. Pocket Sync is Rust; a crate dependency is the boundary.
  Revisit only if a non-Rust host is ever adopted.
- No source abstraction for remote/streaming libraries. It was designed for the dropped Feishin
  target; with Pocket Sync the source is a local filesystem on both sides.
