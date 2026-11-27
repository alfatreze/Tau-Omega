# Multi-Device Architecture & Game Bub Support (`D-026`)

**Status:** Approved 2026-11-26 (owner). Scope of first pass: **G0 + G1 + G2 + G3**. Hardware status: **unverified on physical Game Bub** (community-tested beta; no physical unit on hand).

## 1. Purpose
Tau Omega expands from an Analogue Pocket–only manager into a multi-device FPGA handheld manager, starting with **Game Bub** (`https://cores.gamebub.net/`, `https://docs.gamebub.net/`). All existing Analogue Pocket and Tau workflows remain unchanged.

## 2. Device Abstraction (`DeviceKind`)
`tau_core::device::DeviceKind`:
- `AnaloguePocket` (default)
- `GameBub`

### Card detection (`tau_core::device::detect_card`)
Because macOS and Windows filesystems are case-insensitive (`Cores/` and `cores/` resolve to the same directory), detection **never** relies on folder casing alone:
1. Inspects JSON shape of `core.json` files inside `Cores/` or `cores/`:
   - Top-level `"bitstreams"` or `"min_firmware"` → `GameBub`.
   - Top-level `"core"` (`core.metadata`) → `AnaloguePocket`.
2. Checks structural markers when no cores are installed yet:
   - `Assets/` or `Platforms/` or `System/Pocket/` → `AnaloguePocket`.
   - `system/gameboy.bios-dmg.bin`, `system/gameboy.bios-cgb.bin`, or `system/gba.bios.bin` → `GameBub`.
3. Otherwise returns `None` (blank/unknown card; UI lets the user choose the device).

## 3. Game Bub Contract (`docs.gamebub.net`, Framework v1.1-beta)
- **Filesystem:** FAT32 + MBR only (exFAT / GPT unsupported on Game Bub; Omega warns if non-FAT32 is detected, never formats).
- **Cores directory:** `/cores/<Author.CoreName>/` (lowercase `cores`; folder name matches `metadata.id`, max 32 chars).
- **Core files:**
  - `core.json` (required): `min_firmware`, `metadata` (`id`, `name`, `author`, optional `description`, `version`, `release_date`, `url`, `license`), `bitstreams[]` (`{ target, filename }`, e.g. `gamebub_rev4`, `gamebub_rev2`), optional `hardware` (`cartridge_enable`, `cartridge_selectable`).
  - `files.json` (optional, max 16 entries): `id`, `label`, exactly one of `filename` (fixed in `/cores/<id>/`) / `user_selected` / `dependent_on_0`, `extensions`, `optional`, `read_only`, `initialize`, `on_demand`, `address`, `max_size`/`exact_size`, `max_transfer_speed`.
  - `settings.json` (optional, max 16 entries): `id`, `label`, `address`, `mask`, `default`, `type` (`list` | `action` | `checkbox`), `items[]`/`value`.
- **Built-in BIOS (`/system/`):** `gameboy.bios-dmg.bin` (256 B), `gameboy.bios-cgb.bin` (2,304 B), `gba.bios.bin` (16,384 B) with documented SHA-1s.
- **Firmware:** `.uf2` flashed via USB drive (`GAME BUB` / `GameBubDock`), never written to the SD card.

## 4. Game Bub Core Directory (`https://cores.gamebub.net/cores.json`)
- Machine-readable JSON (`schema_version: 1`, `generated_at`, `default_target`, `targets`, `cores[]`) maintained at `github.com/gamebub/core-directory`.
- Carries `asset_size`, `asset_sha256`, `zip_layout` (`"cores/<id>"` or `"<id>"`), `core_path`, `targets`, `min_firmware`, `license`, `repo`, `download_url`.
- **Multi-core archives:** Some zips bundle multiple cores (e.g. `Minimig` + `Minimig040`, `PcEngine` + `PcEngineCD`). Omega's zip vetter and installer inspect all cores inside the zip and install either the requested core or all bundled cores cleanly into `/cores/<id>/`.

## 5. Phased Delivery
- **G0 — Device model & card detection:** `tau_core::device`, case-safe detection, UI device badge and Marketplace Pocket/Game Bub toggle (usable even without a card open).
- **G1 — Game Bub read-only inspector:** `tau_core::gamebub` (`core.json`, `files.json`, `settings.json`, `/system/` BIOS check, missing required fixed files).
- **G2 — Game Bub Marketplace:** `https://cores.gamebub.net/cores.json` fetched via `tau_net::marketplace` (ETag + TTL), parsed into `CatalogCore` with `device = GameBub`, `asset_sha256`, `targets`, `min_firmware`, plus update checks against `/cores/<id>/`.
- **G3 — Game Bub zip vetting & safe install/update/remove:** `tau_core::gamebub_package` + `tau_core::gamebub_install` supporting both `zip_layout` shapes, multi-core zips, SHA-256 verification, preserving user-supplied fixed assets/BIOS on update, confirmation token, SHA-256-verified backup outside the card, rollback, and `._*` sweep.
- **G4 (deferred until after community feedback):** `/system/` BIOS drag-and-drop helper, GBA `<rom>.json` override editor, target selector UI (`gamebub_rev4` vs `gamebub_rev2`).
