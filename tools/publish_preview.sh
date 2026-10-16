#!/usr/bin/env bash
# Publishes a preview build and points the Preview channel at it.
#
#   git tag v0.5.1-preview.2 && git push origin v0.5.1-preview.2     (the Release workflow builds a DRAFT pre-release)
#   tools/publish_preview.sh v0.5.1-preview.2 [notes-file]           (checks the draft, publishes it, updates the preview feed)
#
# The Preview channel's feed is `latest.json` on the rolling pre-release `preview-channel`; Tau Omega on the Preview channel reads it (and the stable feed)
# and offers whichever is newer. Dev builds (0.5.1-dev.N) are never published: they are local test builds, as in Tau Alpha. Nothing here touches the stable feed: `releases/latest` only ever sees published, non-pre-release versions.
set -euo pipefail
REPO="${REPO:-alfatreze/Tau-Omega}"
TAG="${1:?usage: tools/publish_preview.sh <tag, e.g. v0.5.1-preview.2> [notes-file]}"
NOTES="${2:-}"
case "$TAG" in *-preview.*) ;; *) echo "$TAG is not a preview tag (they look like v0.5.1-preview.2); stable releases are published by hand and dev builds are never published." >&2; exit 1;; esac

info=$(gh release view "$TAG" --repo "$REPO" --json isDraft,isPrerelease,assets)
INFO="$info" python3 - "$TAG" <<'PY'
import json, os, sys
d = json.loads(os.environ["INFO"]); tag = sys.argv[1]
names = [a["name"] for a in d["assets"]]
need = ["latest.json", "SHA256SUMS.txt"]
missing = [n for n in need if n not in names]
for suffix in ("aarch64.app.tar.gz", "x64.app.tar.gz"):
    if not any(n.endswith(suffix) for n in names): missing.append(suffix)
if not any(n.endswith("-setup.exe") for n in names): missing.append("a Windows -setup.exe")
if not any(n.endswith(".AppImage") for n in names): missing.append("a Linux .AppImage")
sigs = [n for n in names if n.endswith(".sig")]
if len(sigs) < 4: missing.append(f"4 update signatures (found {len(sigs)})")
if missing: sys.exit(f"{tag} is not complete: missing {', '.join(missing)}")
if not d["isDraft"]: print(f"note: {tag} is already published; only the dev feed is updated")
PY

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
gh release download "$TAG" --repo "$REPO" -p latest.json -D "$tmp"
python3 - "$tmp/latest.json" "$TAG" <<'PY'
import json, sys
d = json.load(open(sys.argv[1])); tag = sys.argv[2]
if d["version"] != tag.lstrip("v"): sys.exit(f"latest.json says version {d['version']}, not {tag}")
if len(d["platforms"]) != 4: sys.exit(f"latest.json has {len(d['platforms'])} platforms, expected 4")
PY

if [ "$(echo "$info" | python3 -c 'import json,sys;print(json.load(sys.stdin)["isDraft"])')" = "True" ]; then
  if [ -n "$NOTES" ]; then gh release edit "$TAG" --repo "$REPO" --draft=false --prerelease --notes-file "$NOTES"
  else gh release edit "$TAG" --repo "$REPO" --draft=false --prerelease; fi
fi

# the rolling release that holds the feed (created once)
if ! gh release view preview-channel --repo "$REPO" >/dev/null 2>&1; then
  gh release create preview-channel --repo "$REPO" --prerelease --latest=false --title "Tau Omega preview channel" \
    --notes "The update feed (latest.json) of the Preview channel. It always points at the newest dev build; the builds themselves are the pre-releases of this repository. Not a download: use the newest pre-release instead." \
    --target main
fi
gh release upload preview-channel "$tmp/latest.json" --repo "$REPO" --clobber
echo "Preview channel now offers ${TAG#v}: https://github.com/$REPO/releases/download/preview-channel/latest.json"
