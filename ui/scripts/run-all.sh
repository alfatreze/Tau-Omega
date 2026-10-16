#!/usr/bin/env bash
# Runs every browser suite against the dev mock, one after another, and prints each suite's last line.
# Usage: cd ui && (npm run dev &) && sleep 4 && bash scripts/run-all.sh      (stop vite afterwards: pkill -f vite)
# A suite that fails prints its error instead of "N checks passed". The workbench suite alone takes about 7 minutes.
cd "$(dirname "$0")/.."
export NODE_PATH=$(npm root -g)
for t in cores workbench marketplace manual disable devtools builds results check builder install-review trim playlist-core playlist-add omega-channel library-footer multi-install saved-list scrollbar; do
  echo "== $t"; node scripts/$t-test.cjs 2>&1 | tail -1 | cut -c1-120
done
echo ALLDONE
