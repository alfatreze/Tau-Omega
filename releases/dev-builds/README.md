# Test builds (not releases)

Every build that is not a proper release goes in a folder here, named
`<next release>-dev.<counter>`, for example `0.4.0-dev.1`, `0.4.0-dev.2`.

* **Release goal** is the version the next release will carry (`TARGET` in this folder). The counter
  restarts at 1 for each new goal. When `v0.4.0` ships, set `TARGET` to the next goal (`0.5.0`).
* **Why this name.** `0.4.0-dev.N` is valid semver and sorts *before* `0.4.0`, so a test build can never be
  mistaken for the release. It is also the convention the Tau Alpha core uses for its own test builds
  (`0.6.0-dev.N`), so the two projects read the same. When a goal is close, builds can move to
  `-beta.N`, then `-rc.N`, before the plain `0.4.0`.
* **Channels** (as in Tau Alpha): these `-dev.N` builds are the Dev channel, local and never published. A published pre-release is a Preview (`v0.5.1-preview.N`, built by the Release workflow and published with `tools/publish_preview.sh`); a published release is Stable. See `docs/RELEASING.md`.
* **Releases** stay in `../v<version>/` (decision D-013). Nothing here is a release and nothing here is signed.
* The build id is also stamped into the app itself (Finder > Get Info shows `0.4.0-dev.N`).

Make one with [`tools/dev-build.sh`](../../tools/dev-build.sh) (it picks the next counter, builds, copies the app into
the folder, and writes `BUILD.txt` with the commit and a SHA-256 of the executable). Each folder holds the runnable
`Tau Omega.app` itself, not a zip. The apps are not committed (see `.gitignore`); each folder's `BUILD.txt` and
`SHA256SUMS.txt` are, so the history of test builds is recorded.

To try one: right-click `Tau Omega.app` > Open the first time (unsigned), and use a throwaway card first
(`TEST_PLAN.md` item 5).
