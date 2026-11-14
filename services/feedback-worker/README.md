# Tau Omega feedback service

A small Cloudflare Worker. The Send feedback page in Tau Omega posts a message here; the Worker checks it, stores any attachments, and files a **public** GitHub issue with the right labels. Omega never holds a GitHub token. Design and decisions: `../../docs/FEEDBACK_SERVICE.md` (D-025).

```
POST /v1/feedback   JSON { kind, area, title, text, basics, github, files:[{name, mime, data (base64)}] }  ->  201 { number, url }
GET  /v1/status     { ok, paused }
GET  /f/<key>       a stored attachment
```

## Test (no account needed)
```bash
cd services/feedback-worker && npm test
```
The tests use fakes for GitHub, storage and the counters.

## Deploy (once; needs your Cloudflare login and a GitHub token)
Use a **private test repository** first: set `GITHUB_REPO` in `wrangler.toml` to it, deploy, send a message from the app, then switch it to `alfatreze/Tau-Omega` and deploy again.

```bash
cd services/feedback-worker
npx wrangler login                                   # skip if you are already signed in (npx wrangler whoami)
npx wrangler r2 bucket create tau-omega-feedback-files
npx wrangler kv namespace create LIMITS              # paste the id it prints into wrangler.toml
npx wrangler secret put GITHUB_TOKEN                 # paste the token (below)
npx wrangler secret put IP_SALT                      # any long random text
npx wrangler deploy                                  # prints https://tau-omega-feedback.<you>.workers.dev
```
Then put that address in `PUBLIC_URL` in `wrangler.toml` (attachment links in issues use it) and run `npx wrangler deploy` again.

**The GitHub token:** GitHub > Settings > Developer settings > Fine-grained personal access tokens > Generate. Resource owner `alfatreze`, **only** the repository `Tau-Omega`, permission **Issues: Read and write** and nothing else, expiry one year (put the date in your calendar: when it expires, filing stops and the app says it could not file the message). Labels (`type: bug`, `area: marketplace`, `from: omega`...) are created by GitHub the first time they are used.

**Attachments are kept one year:** Cloudflare dashboard > R2 > `tau-omega-feedback-files` > Settings > Object lifecycle rules > delete objects after 365 days.

**Kill switch:** set `PAUSED = "1"` (in `wrangler.toml` and deploy, or in the dashboard under the Worker's variables). The app then shows "Feedback is paused".

## Limits (also in `src/rules.js`)
Title 80, description 10 to 4,000, up to 4 files, 5 MB each, 12 MB in all (png, jpg, gif, webp, txt, log, json, zip, each checked by its own bytes). Per sender 5 an hour and 20 a day, 200 a day for everyone, an identical message twice in a day is refused (`HOURLY`, `DAILY`, `GLOBAL_DAILY` in `wrangler.toml`). Counts are approximate (KV).

## Things to know
- The address under `workers.dev` contains your Cloudflare account's subdomain and is public once it is in the app. A custom address (for example `feedback.<your domain>`) can be added later with a `routes` entry.
- Attachments are public by design (the page tells the sender). A message that fails after its files were stored leaves those files in the bucket until the one-year rule removes them.
- Sender text is cleaned before it is filed: no `@mentions` (so nobody is notified through the service), no raw HTML, no pictures from other sites, links shown as words.
