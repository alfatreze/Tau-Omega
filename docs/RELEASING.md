# Releasing Tau Omega (installers, signing)

**Installers.** `.github/workflows/release.yml` builds, for a tag `v*` or on demand: macOS Apple silicon and Intel `.dmg` (+ `.app`), Windows `.exe` (NSIS, per-user) and `.msi`, Linux `.AppImage` and `.deb`. For a tag it attaches them with `SHA256SUMS.txt` to a **draft** GitHub release; nothing is public until the draft is published by hand. Bundle metadata (publisher, category, descriptions, minimum macOS 11) is in `src-tauri/tauri.conf.json`. `build-app.yml` stays as the unsigned test build.
Checked 2026-10-10: `npx @tauri-apps/cli@2 build --bundles app,dmg` on macOS produced `Tau Omega_0.3.0_aarch64.dmg` (6.4 MiB, unsigned). The Windows and Linux bundles have not been built yet (they run on the CI runners).

**To make a release:** set the version in `Cargo.toml`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` (they must agree), commit, tag `vX.Y.Z`, push the tag, wait for the Release workflow, check the draft, publish.

**Signing (needs credentials only the owner holds; the workflow switches it on by itself when the secrets exist):**
- macOS: an Apple Developer ID Application certificate exported as `.p12` and base64 encoded -> secrets `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, plus `APPLE_ID`, `APPLE_PASSWORD` (an app-specific password) and `APPLE_TEAM_ID` for notarisation. Without them the dmg is unsigned and Gatekeeper warns on first open.
- Windows: not wired. It needs a code-signing certificate (or a signing service such as Azure Trusted Signing or SSL.com eSigner) and a `signCommand`/certificate thumbprint in the Tauri config. Unsigned builds trigger SmartScreen.
- Linux: no signing step; the checksums are the integrity check.
