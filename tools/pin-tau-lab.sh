#!/usr/bin/env bash
# Pins a Tau Alpha meter lab drop (interop D-007): verifies it, copies it verbatim into vendor/tau-lab/<name>/ and installs its built page
# (meter_lab.html, when the drop has one) for the Meter Lab. Never edits the drop. Usage: tools/pin-tau-lab.sh <path to the drop folder>
set -euo pipefail
cd "$(dirname "$0")/.."
src="${1:?usage: tools/pin-tau-lab.sh <drop folder>}"
[ -f "$src/MANIFEST.json" ] && [ -f "$src/SHA256SUMS.txt" ] || { echo "not a lab drop (MANIFEST.json and SHA256SUMS.txt are required)" >&2; exit 1; }
(cd "$src" && shasum -a 256 -c SHA256SUMS.txt >/dev/null) || { echo "the drop does not verify against its SHA256SUMS.txt" >&2; exit 1; }
name="$(basename "$src")"
dest="vendor/tau-lab/$name"
[ -e "$dest" ] && { echo "$dest already exists; drops are never overwritten" >&2; exit 1; }
cp -R "$src" "$dest"
(cd "$dest" && shasum -a 256 -c SHA256SUMS.txt >/dev/null) || { rm -rf "$dest"; echo "copy did not verify" >&2; exit 1; }
commit="$(python3 -c "import json,sys;print(json.load(open('$dest/MANIFEST.json')).get('tau_alpha_commit','?'))")"
echo "pinned $dest (Tau Alpha commit $commit): add a row to vendor/tau-lab/README.md"
if [ -f "$dest/meter_lab.html" ]; then
  mkdir -p ui/public/tau-lab && cp "$dest/meter_lab.html" ui/public/tau-lab/meter_lab.html
  [ -f "$dest/lab-data.json" ] && cp "$dest/lab-data.json" ui/public/tau-lab/lab-data.json && echo "installed ui/public/tau-lab/lab-data.json (the preset editor's registry)"
  cp "$dest/MANIFEST.json" ui/public/tau-lab/MANIFEST.json && echo "installed ui/public/tau-lab/MANIFEST.json (the module hashes the app checks the lab against)"
  echo "installed ui/public/tau-lab/meter_lab.html (sha256 $(shasum -a 256 "$dest/meter_lab.html" | cut -c1-16))"
else
  echo "this drop has no meter_lab.html: the Meter Lab keeps saying the page is not installed" >&2
fi
