# Interop mailbox with Tau Alpha

Adopted 2026-10-26 (proposed by Tau Alpha, `docs/interop/README.md` in their repository). Each project writes only to its own outbox and reads the other's, read-only:
- Tau Alpha writes `docs/interop/to-omega/NNN-YYYY-MM-DD-title.md` and keeps the ledger `docs/interop/LEDGER.md` (their repository).
- **Tau Omega writes `docs/interop/to-alpha/NNN-YYYY-MM-DD-title.md`** (this folder). Messages are short and numbered, headed `From`, `Date`, `Replies to`, with items by id (`Q-` question, `D-` decision, `R-` request, `F-` fact or correction) and ending `Open for you:` / `Open for us:`. Reply by id, not in prose.
- Fixtures are **referenced** by repository path and SHA-256, not copied (the exceptions are the real captures kept in `testdata/` with a provenance README, and Tau's `rehearsal-*` packages, which are read in place).
- **A message counts as sent when it is committed** (owner decision 2026-11-22, the same rule as Tau Alpha's `docs/interop/README.md`). The other session reads this folder in place, so a file here is visible to it at once, even before it is committed: **a message is written into this folder only after the owner has seen it**, and committing it is sending it. Pushing is a separate step (the usual "push only when told" rule); it only makes the links in a message resolve on GitHub. Headers say `Sent` with the date, never `draft`.
- The owner's job is one sentence per session: "read `docs/interop/` of the other project and answer the open ids".

This does not replace `docs/FIRMWARE_SYNC.md` (what Omega assumes of Tau's files) or `docs/TAU_ALPHA_SYNC_PLAN_2026-10-26.md` (the gap analysis and plan); messages here point at them.

**`OPEN.md`** is Omega's own list of what it still needs from Tau Alpha (with the message ids), updated whenever a message is sent or answered; read it first.

