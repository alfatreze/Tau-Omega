# Safety rules (non-negotiable)

These come from real data-loss incidents in this project (an earlier firmware write destroyed a user's music library three times)
and from the owner's working rules. They apply to Tau Omega exactly as they apply to the firmware work.

1. **Never modify a source music file or folder.** Sources are read-only inputs. Every change (ASCII names, embedded covers,
   re-encoded covers) is made on the **copy** on the card or in a staging folder. Editing tags or covers is allowed **only on the
   card copy** (owner decision D1, 2026-09-30): it goes through a reviewed plan, each file is rewritten to a temporary name, read
   back to prove the new tags took, and only then renamed into place; the edit is recorded in `tau-omega/edits.json` on the card so a
   later sync re-applies it instead of silently undoing it. "Edit tags in place" on the user's own source files stays out of scope.
2. **No write to an SD card without an explicit confirmation of a shown plan.** The flow is always: scan, build a plan (what will be
   created, replaced, deleted, how many bytes), show it, wait for a clear yes, then write. A `--dry-run` / "Plan only" mode exists
   everywhere and is the default in the CLI. Approval is per action and per session; it is never remembered across runs.
3. **Verify every write.** Each file copied to the card is re-read and compared by SHA-256 (or a byte compare) with its source.
   The index is written last, to a temporary name, re-parsed with the reference loader logic, then renamed. A failed verify stops the run
   and is reported; nothing is left half-installed without saying so.
4. **Deletes need a second confirmation and a backup or trash.** Moves are copy, verify, then delete of the source, and the delete is
   a separate confirmed step. Removing a core or a media folder first copies it to a backup location the user can see. Never
   empty a trash or bypass one silently.
   *Owner decision D2 (2026-09-30), Library workbench:* removing individual albums from a card follows a preference in
   Settings > Library: **back up then remove** (the default), **ask each time**, or **just remove**. A removal is only *marked* until
   the user reviews and confirms the sync (the second step), can be undone until then, and the first removal explains the
   preference and where to change it. Whole-core removal and core moves are unchanged and always back up first. Because "just
   remove" relaxes this rule, it is never the default and is only ever set by the user.
5. **Never write outside the Tau-owned paths** unless the user picks another core explicitly: `Assets/<platform>/common/`,
   the core's own folder under `Cores/`, and `Platforms/<platform>.json`. Never touch `System/`, other authors' cores, `Saves/`,
   `Memories/` or `Settings/` except in the clearly labelled backup and cleanup tools, which read first and confirm.
6. **The five `System/*.bin` cache files** (`core_viewby_platform`, `corelist_cache`, `cores_cache`, `platform_viewby_category`,
   `platforms_cache`) are safe to delete after cores are added or removed so the Pocket rebuilds them. Back them up first.
7. **macOS metadata:** `._*`, `.DS_Store` and `.fseventsd` are never copied. Files it creates on the card (`._*`) are removed after a
   write. Leave `.Spotlight-V100` / `.fseventsd` alone.
8. **FAT rules:** FAT32 has a 4 GiB file limit and no case sensitivity; exFAT is common. Long names are fine, but the Tau core opens
   files by a path of at most 200 bytes and only ASCII names. Refuse or shorten, never silently truncate.
9. **Ejecting:** the app tells the user to eject and, where the OS allows, offers to. Writes are flushed (`fsync` on files,
   `sync` on the volume) before saying "done".
10. **No network by default.** No telemetry, no accounts, no cloud. Update checks are opt-in and only fetch a version manifest.
11. **Fixtures and tests never use a real card.** Tests run on temporary directories and on fake-card fixtures. Any test that would
    write to `/Volumes/*` or a drive letter fails the build.
12. **Privacy:** the app reads only folders the user selected. Nothing is uploaded. Tag text stays local.
13. **Honest reporting:** if a step is skipped, failed or only partly done, the report says so plainly. No "success" summary hides a
    warning. Results that came from real hardware carry a label; results from tests say they are from tests.
