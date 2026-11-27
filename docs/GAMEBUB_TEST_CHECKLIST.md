# Game Bub Community Verification Checklist

Tau Omega's Game Bub support (`D-026`) was built directly from `https://docs.gamebub.net/` and `https://cores.gamebub.net/cores.json` without a physical Game Bub unit on hand. Please use this checklist when testing on real Game Bub hardware (Rev 4, firmware v1.1-beta+).

> **Safety reminder:** Always back up your microSD card before testing. Omega also writes its own SHA-256-verified backup outside the card before every install, update, or removal.

## Checks
1. **Card detection**
   - [ ] Insert a FAT32/MBR Game Bub microSD card and open it in Tau Omega.
   - [ ] Confirm Omega detects it as **Game Bub** (and lists any existing cores under `/cores/`).
2. **Marketplace browsing**
   - [ ] Switch Marketplace to **Game Bub** (works both with and without a card open).
   - [ ] Confirm cores from `cores.gamebub.net` load with version, author, licence, target (`gamebub_rev4`), and minimum firmware.
3. **Single-core install (`cores/<id>` layout, e.g. `Wenwald.2600` or `Retro-Bub.GG`)**
   - [ ] Download, review plan, and install onto the card.
   - [ ] Confirm `/cores/<id>/core.json` and `.bit` exist with no `._*` AppleDouble stubs.
   - [ ] Boot the core on Game Bub hardware.
4. **Single-core install (`<id>` layout, e.g. `elipsitz.N64` or `jeremy.SNES`)**
   - [ ] Download, review plan, and install onto the card.
   - [ ] Confirm Omega normalizes the archive into `/cores/<id>/` (not card root).
   - [ ] Boot the core on Game Bub hardware.
5. **Multi-core zip install (e.g. `apolkosnik.Minimig` / `Minimig040`, or `ToadyM.PcEngine` / `PcEngineCD`)**
   - [ ] Confirm the plan clearly lists the core(s) being installed from the shared zip.
   - [ ] Boot on Game Bub hardware.
6. **Update preserving user files**
   - [ ] Place a user BIOS/asset file inside `/cores/<id>/` (not shipped in the zip), then reinstall/update the core.
   - [ ] Confirm the user file survives untouched.
7. **Remove / rollback**
   - [ ] Remove a test core through Omega's reviewed plan and verify the backup outside the card contains the removed files.

Please report any discrepancy (zip layout, `core.json` field, firmware version, or boot issue) in a GitHub issue on `alfatreze/Tau-Omega`.
