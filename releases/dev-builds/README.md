# Test builds (not releases)

Every build that is not a proper release goes in a folder here, named
`<next release>-alpha.<counter>`, for example `0.4.0-alpha.1`, `0.4.0-alpha.2`.

* **Release goal** is the version the next release will carry (`TARGET` in this folder). The counter
  restarts at 1 for each new goal. When `v0.4.0` ships, set `TARGET` to the next goal (`0.5.0`).
* **Why this name.** `0.4.0-alpha.N` is valid semver and sorts *before* `0.4.0`, so a test build can never be
  mistaken for the release. It is also the convention the Tau Alpha core uses for its own test builds
  (`0.6.0-alpha.N`), so the two projects read the same. When a goal is close, builds can move to
  `-beta.N`, then `-rc.N`, before the plain `0.4.0`.
* **Releases** stay in `../v<version>/` (decision D-013). Nothing here is a release and nothing here is signed.
* The build id is also stamped into the app itself (Finder > Get Info shows `0.4.0-alpha.N`).

Make one with [`tools/dev-build.sh`](../../tools/dev-build.sh) (it picks the next counter, builds, zips, and writes
`BUILD.txt` with the commit and a SHA-256). The `.zip` files are not committed (see `.gitignore`); each folder's
`BUILD.txt` and `SHA256SUMS.txt` are, so the history of test builds is recorded.

To try one: unzip, right-click `Tau Omega.app` > Open the first time (unsigned), and use a throwaway card
first (`TEST_PLAN.md` item 5).
