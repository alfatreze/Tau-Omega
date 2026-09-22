# Safety rules (non-negotiable)

These come from real data-loss incidents in this project (an earlier firmware write destroyed a user's music library three times)
and from the owner's working rules. They apply to Tau Omega exactly as they apply to the firmware work.

1. **Never modify a source music file or folder.** Sources are read-only inputs. Every change (ASCII names, embedded covers,
   re-encoded covers) is made on the **copy** on the card or in a staging folder. Any future "edit tags in place" feature is off by
   default, needs an explicit per-run opt-in and an undo journal, and is a separate phase.
2. **No write to an SD card without an explicit confirmation of a shown plan.** The flow is always: scan, build a plan (what will be
   created, replaced, deleted, how many bytes), show it, wait for a clear yes, then write. A `--dry-run` / "Plan only" mode exists
   everywhere and is the default in the CLI. Approval is per action and per session; it is never remembered across runs.
3. **Verify every write.** Each file copied to the card is re-read and compared by SHA-256 (or a byte compare) with its source.
   The index is written last, to a temporary name, re-parsed with the reference loader logic, then renamed. A failed verify stops the run
   and is reported; nothing is left half-installed without saying so.
4. **Deletes need a second confirmation and a backup or trash.** Moves are copy, verify, then delete of the source, and the delete is
   a separate confirmed step. Removing a core or a media folder first copies it to a backup location the user can see. Never
   empty a trash or bypass one silently.
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
