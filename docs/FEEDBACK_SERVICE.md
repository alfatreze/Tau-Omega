# Feedback to GitHub issues: the service and what is left to build (scope, 2026-11-14)

**Status (2026-11-14).** The service is **written and tested but not deployed** (`services/feedback-worker/`, 9 tests, no account needed; deploy steps in its README). The sender side is built (the **Send feedback** page, `ui/src/lib/FeedbackView.svelte`, rules in `ui/src/lib/feedback.ts`, 11 logic and 21 browser checks). It cannot send yet: `feedback_info` reports `enabled: false`, so the page says "Sending is not connected yet". This document scopes the rest. Decision D-025: the route is an owner-run Cloudflare Worker (the owner already runs one for another project); Omega ships **no** GitHub token.

## 1. What a person sees (built)
Tools & settings → Send feedback. They choose *what it is* (Something is wrong / An idea / A question), *where in Omega* (11 areas), write a title (80 characters) and a description (10 to 4,000), optionally attach up to **4 files, 5 MB each, 12 MB in all** (png, jpg, gif, webp, txt, log, json, zip; a picture can also be pasted into the description), optionally add their Omega version and system, and optionally a GitHub name for a reply. A plain note beside Send says everything will be public (no box to tick, no preview step). After sending they see the issue number and a link.

## 2. How a message becomes an issue
```
Omega ──POST /v1/feedback (JSON)──▶ Worker ──GitHub REST: create issue──▶ alfatreze/Tau-Omega
                                           └──R2: attachments (random names)
```
- **The app sends fields, never finished text or labels,** as one JSON body with the files in base64 (a multipart body was planned; JSON is simpler for the Rust side and the body stays under 18 MB). Same shape as `FeedbackSend` (`ui/src/lib/types.ts`): `kind`, `area`, `title`, `text`, `basics {version, os, arch}` or null, `github`, `files[] {name, mime, data}`. The Worker writes the title, labels and body itself, so a hand-made request cannot set arbitrary labels or markup.
- **Labels** the Worker sets from fixed lists: `type: bug|idea|question`, `area: <lower-case area>`, `from: omega`. GitHub creates a label the first time it is used (the Worker's token has write access; labels would be ignored through a plain "new issue" link).
- **Body** (the Worker writes it): the text, an attachments line (images shown inline, other files as links), a rule, `Category: …`, `Sent from Tau Omega <version> (<system>, <arch>)` only when the sender chose it, `Reply on GitHub: @name` only when given.
- **Also:** `GET /v1/status` returns `{ok, paused}` so the app can say "paused" before the sender writes anything; `GET /f/<key>` serves a stored attachment.
- **Response:** `201 {number, url}`; also 409 for an identical message sent twice. Errors are `{code, message}` with a plain-language message the app shows as it is: 400 (what is wrong), 413 (too large), 429 (`retry_after` seconds), 503 (paused or daily cap reached), 502 (GitHub unreachable).

## 3. What the Worker must enforce (everything the app checks is re-checked)
| Rule | Value |
|---|---|
| Fields | `kind` in {bug, idea, question}; `area` in the 11-name list; title 1 to 80, text 10 to 4,000 (after trimming); `github` empty or `^[A-Za-z0-9](?:[A-Za-z0-9]\|-(?=[A-Za-z0-9]))*$` up to 39 |
| Files | at most 4; each at most 5 MiB; total at most 12 MiB; the whole request at most 13 MiB; extension in the allowed list **and the file's own first bytes agree** (PNG, JPEG, GIF, WebP signatures; txt/log/json valid UTF-8 with no NUL; zip starts `PK`) |
| Untrusted text | neutralise `@mentions` (so nobody can notify a team or user through us; the optional reply name is the only `@` the Worker writes), keep markdown but turn external images and links into plain text (no tracking pixels), cut anything over the limit |
| Rate | per address 5 per hour and 20 per day; global cap (suggest 200 per day) after which the Worker answers 503 "Feedback is paused for today"; the same title and text twice within 24 hours is refused as a duplicate |
| Kill switch | one setting that makes every request answer 503 "Feedback is paused" (the page shows the message; nothing else breaks) |

Turnstile or similar is **not** used (a desktop app has no page to show it on); the protection is the limits above, small caps and the kill switch. A shared header from the app proves nothing (it can be copied) and is not relied on.

## 4. Attachments
Stored in an R2 bucket under **random 128-bit names**, served from a separate cookie-free address with `X-Content-Type-Options: nosniff`; images inline, every other type as `Content-Disposition: attachment`. They are public, and the page says so before sending. Retention: suggest one year, and removal on request by issue number. Not stored: file names beyond the display name in the issue (the stored name is random).
**Zip is the one risky type** (a public address serving a stranger's zip). It is allowed because Omega's own diagnostics report is a zip; the decision is open (section 8) and the allowed list lives in one place on each side.

## 5. GitHub access
A **fine-grained token (or a GitHub App installation token) limited to `Issues: write` on `alfatreze/Tau-Omega` only**, kept as a Worker secret, never in Omega, never in a response or a log. If it leaks, the worst case is spam issues in one repository; rotate it and nothing else is exposed. A staging repository (`…-feedback-test`, private) takes the same Worker with its own token for end-to-end tests.

## 6. Privacy
- Omega sends only what is in the form, plus the version, system and architecture **only if ticked**. Never card names, file paths, library contents or the machine's name.
- The Worker keeps no copy of messages (GitHub and R2 are the record). Rate-limit counters use a hash of the address that expires within 24 hours. Logs: status codes and sizes only.
- The page already warns that everything is public. The service needs a short privacy note (what is kept, for how long, how to ask for removal); suggested text lives next to the page, in English only.

## 7. What is left to build, in order
1. **Worker: DONE** (about 250 lines in `services/feedback-worker/`, 9 tests with fakes). Not yet run against real GitHub: do one live run against a private test repository first.
2. **Setup** (the owner, once; commands in the Worker's README): the R2 bucket, the KV namespace, the two secrets (the GitHub token and a salt), the one-year lifecycle rule on the bucket, then the deployed address into `PUBLIC_URL`.
3. **Omega, Rust** (`src-tauri/src/feedback_cmds.rs`, `crates/tau-net`): `feedback_send` (re-check the limits, decode, build the multipart request, POST to the one fixed address with `ureq`, no redirects, 60 s limit, map errors to the plain messages), `feedback_open_issue(number)` (opens only `https://github.com/alfatreze/Tau-Omega/issues/<number>`), and `feedback_info.enabled` true once an address is built in. The page, the mock and the types already match. Tests with a scripted transport (the multipart body, each error code, the size caps).
4. **Switch on:** set the address, run the page against staging, then production; update the sidebar and README note on what leaves the computer (the update check, the marketplace pages and now this, only when the person presses Send).
No new dependency is expected (`ureq` is already in the app); multipart is written by hand (about 40 lines).

## 8. Choices made in the Worker (the defaults proposed to the owner; change in `rules.js` / `wrangler.toml`)
Zip allowed; the sender's markdown kept with links as words and pictures removed; attachments kept one year; a fine-grained token limited to Issues on `Tau-Omega`; the `workers.dev` address to start with. The original open questions follow.

## 9. Questions for the owner (none blocks the page)
1. **Zip attachments:** allowed (as built) or only pictures and text?
2. **Markdown policy:** keep the sender's markdown with links and images made plain (suggested), or put the whole text in a quote block?
3. **Retention** of attachments (suggest one year) and who handles removal requests.
4. **Token kind:** fine-grained personal token (simplest) or a GitHub App (better audit, a little more setup)?
5. **The address** of the service, and whether feedback for Tau Alpha's firmware should go to that repository instead (a "Something else" area today stays in Omega's).
