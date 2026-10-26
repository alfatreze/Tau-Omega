# Interop mailbox with Tau Alpha

Adopted 2026-10-26 (proposed by Tau Alpha, `docs/interop/README.md` in their repository). Each project writes only to its own outbox and reads the other's, read-only:
- Tau Alpha writes `docs/interop/to-omega/NNN-YYYY-MM-DD-title.md` and keeps the ledger `docs/interop/LEDGER.md` (their repository).
- **Tau Omega writes `docs/interop/to-alpha/NNN-YYYY-MM-DD-title.md`** (this folder). Messages are short and numbered, headed `From`, `Date`, `Replies to`, with items by id (`Q-` question, `D-` decision, `R-` request, `F-` fact or correction) and ending `Open for you:` / `Open for us:`. Reply by id, not in prose.
- Fixtures are **referenced** by repository path and SHA-256, not copied (the exceptions are the real captures kept in `testdata/` with a provenance README, and Tau's `rehearsal-*` packages, which are read in place).
- The owner's job is one sentence per session: "read `docs/interop/` of the other project and answer the open ids".

This does not replace `docs/FIRMWARE_SYNC.md` (what Omega assumes of Tau's files) or `docs/TAU_ALPHA_SYNC_PLAN_2026-10-26.md` (the gap analysis and plan); messages here point at them.
