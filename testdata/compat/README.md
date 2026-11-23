# Real `tau-compat.json` files (copied read-only from Tau Alpha releases)

`tau-compat_v0.7.5.json`: the asset of Tau Alpha's **v0.7.5** release (published 2026-11-23, tag `996122085b93a9c0c2478b88ff86d9801c1c9cf9`), SHA-256 `5a8edac594d223b74c16f1fe39ff5be50dcefe87c107c4be7b169a5b29ffbd94`, which is the value in that release's `SHA256SUMS.txt`. Copied from `tau-alpha/release/tau-compat.json` (their local copy of the release assets).

It says `persist_layout` 2, `packs_abi` 1, `packs_abi_minor` 2, `min_omega` 0.6.0 and `meter_settings_version` 1, and lists `Saves/tau/<core>/tau-meter-settings.bin` as a user file (slot 11, format `TMSF` v1, 8,192 bytes). Omega 0.6.0 refused this release because it only knew `persist_layout` 1; the test `crates/tau-core/tests/compat_v075.rs` pins that Omega now accepts it.
