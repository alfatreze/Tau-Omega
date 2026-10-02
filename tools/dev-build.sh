#!/usr/bin/env bash
# Builds a TEST (non-release) copy of the app and files it under
# releases/dev-builds/<target>-alpha.<n>/ with the next counter. The folder holds the
# runnable Tau Omega.app itself (not a zip): open it straight from Finder.
#
#   tools/dev-build.sh            build, then package
#   NO_BUILD=1 tools/dev-build.sh file the bundle that is already built
#   tools/dev-build.sh 0.5.0      override the release goal for this build
#
# The release goal lives in releases/dev-builds/TARGET (the NEXT release's
# MAJOR.MINOR.PATCH). Bump it when a release ships. See releases/dev-builds/README.md.
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET="${1:-$(tr -d '[:space:]' < releases/dev-builds/TARGET)}"
n=1
for dir in releases/dev-builds/"$TARGET"-alpha.*; do
  [ -d "$dir" ] || continue
  last="${dir##*-alpha.}"
  [[ "$last" =~ ^[0-9]+$ ]] && [ "$last" -ge "$n" ] && n=$((last + 1))
done
NAME="$TARGET-alpha.$n"
OUT="releases/dev-builds/$NAME"
[ -e "$OUT" ] && { echo "refusing to overwrite $OUT" >&2; exit 1; }

DIRTY=clean
[ -n "$(git status --porcelain --untracked-files=no)" ] && DIRTY="UNCOMMITTED CHANGES in the working tree"

if [ "${NO_BUILD:-0}" != "1" ]; then
  (cd ui && npm run build)
  # The version override puts the build id into the app itself (Finder > Get Info).
  (cd src-tauri && cargo tauri build --bundles app \
      --config "{\"version\":\"$NAME\",\"build\":{\"beforeBuildCommand\":\"\"}}")
fi

APP="src-tauri/target/release/bundle/macos/Tau Omega.app"
[ -d "$APP" ] || { echo "no built app at $APP" >&2; exit 1; }
mkdir -p "$OUT"
ditto "$APP" "$OUT/Tau Omega.app"
(cd "$OUT" && shasum -a 256 "Tau Omega.app/Contents/MacOS/tau-omega" > SHA256SUMS.txt)
cat > "$OUT/BUILD.txt" <<INFO
Tau Omega test build $NAME   (NOT a release)
Release goal : $TARGET
Built        : $(date '+%Y-%m-%d %H:%M %Z')
Branch       : $(git rev-parse --abbrev-ref HEAD)
Commit       : $(git rev-parse --short HEAD)   $(git log -1 --format=%s)
Working tree : $DIRTY
Toolchain    : $(rustc --version)
App          : Tau Omega.app   (unsigned: right-click > Open the first time)
SHA-256      : $(cut -d' ' -f1 "$OUT/SHA256SUMS.txt")  (of Contents/MacOS/tau-omega)

What is in it: (fill in a line or two when sharing this build)
INFO
echo "Filed $OUT/Tau Omega.app"
