# GitHub releases fixture

`releases.json` is the real response of `gh api repos/alfatreze/Tau-Alpha/releases` fetched 2026-10-08, trimmed to the
fields Omega reads (`tag_name`, `name`, `prerelease`, `draft`, `published_at`, and per asset `name`, `size`,
`browser_download_url`). Not hand-written. It predates `tau-compat.json`, so no release in it carries one.
