export type Warning = { code: string; message: string };
export type Core = { id: string; author: string; shortname: string; version: string; platform: string; platform_category: string | null; library_capable: boolean; index_status: string; tracks: number | null };
export type ArtSidecarPreview = { folder: string; cover_source: string };
export type Plan = { id: string; new_files: number; updates: number; unchanged: number; bytes_to_write: number; art_sidecars: number; art_sidecar_previews: ArtSidecarPreview[]; warnings: Warning[] };
export type Setting = { id: number; kind: string; value: unknown };
export type Job = { kind: string; status: string; detail: string };
/** Mirrors `tau_core::journal::JournalSummary`: one row for the Jobs history
 * list, built from a journal file in the configured reports directory. */
export type JournalSummary = { path: string; kind: string; state: string; recorded_at_unix: number; plan_id: string; destination: string; files: number; copied: number | null; deleted: number | null; error: string | null };
export type Difference = { relative: string; state: 'only_left' | 'only_right' | 'different' | 'identical'; left_bytes: number | null; right_bytes: number | null };
export type Comparison = { left: string; right: string; only_left: number; only_right: number; different: number; identical: number; differences: Difference[] };
export type Playlist = { name: string; tracks: number };
/** The Playlists page's own scan shape: unlike the count-only `Playlist`
 * above, it needs each track's resolved relative path (to reorder in place
 * and to target rename/reorder/import at a specific file). */
export type PlaylistDetail = { name: string; file: string; tracks: string[] };
export type MediaScan = { playlists: PlaylistDetail[]; warnings: Warning[] };
/** Mirrors `tau_core::playlist::PlaylistPlan`: a reviewed, not-yet-written
 * playlist change, following the same plan -> confirm -> execute shape as
 * `Plan`/`SyncReport`. */
export type PlaylistPlan = { id: string; file: string; previous_file: string | null; tracks: string[]; dropped: string[]; overwrites_existing: boolean };
export type ProblemKind = 'duplicate' | 'missing_tag' | 'unsupported_format' | 'path_issue' | 'missing_cover';
export type Problem = { kind: ProblemKind; files: string[]; message: string };
export type TrackRow = { rel: string; title: string; artist: string; album: string; secs: number; format: string };
export type LibraryScan = { tracks: TrackRow[]; playlists: Playlist[]; warnings: Warning[] };
/** The engine's own `tau_core::sync::SyncReport`, returned directly from
 * execute_sync/execute_core_copy/execute_core_move now that it implements
 * `Serialize` (P1-1) -- no hand-written result DTO duplicates it. */
export type SyncReport = { plan_id: string; copied: number; unchanged: number; bytes_written: number; deleted: number; art_sidecars_written: number; index_path: string; index_sha256: string; warnings: Warning[] };
/** What a rejected `invoke()` resolves to now that the engine's errors carry
 * a stable numeric `code` (see `tau_core::ErrorCode`) across the boundary
 * instead of a flattened English string. */
export type ApiError = { code: number; message: string };
/** Mirrors `tau_core::Progress`, delivered as a `"tau://progress"` window event. */
export type ProgressEvent = { job_id: string; stage: string; done: number; total: number; path: string | null };
/** Mirrors `tau_core::storage::{VolumeSpace, CapacityCheck}`. */
export type VolumeSpace = { total_bytes: number; available_bytes: number };
export type CapacityCheck = { space: VolumeSpace; bytes_needed: number; margin_bytes: number; fits: boolean };
/** Mirrors `tau_core::backup::{BackupItem, BackupPlan}`: a read-only dry-run
 * preview of backing up one folder onto another. There is no execute
 * command yet -- see STATUS_HANDOFF.md item 5. */
export type BackupItem = { relative: string; state: 'only_left' | 'only_right' | 'different' | 'identical'; bytes: number };
export type BackupPlan = { source: string; destination: string; items: BackupItem[]; new_files: number; updated_files: number; unchanged_files: number; destination_only_files: number; bytes_to_write: number };
/** Mirrors `tau_core::diag::CheckSummary`: a decoded firmware Check-report
 * summary from persist ids 20-23. `null` (not this type) means those ids
 * don't currently hold one -- the normal case, not an error. */
export type CheckSummary = { profile: string; run: number; verdict: string; passed: string[]; failed: string[]; worst_access_cycles: number; cold_cycles_per_word: number; late_underruns: number; draw_stall_ms: number; last_load_s: number; library_error: number; cold_error: number; firmware_minor: number };
/** Mirrors `tau_core::package::{PackageEntry, PackageManifest}`: what a core
 * release zip declares, read without touching any card. */
export type PackageEntry = { path: string; bytes: number; sha256: string };
export type PackageManifest = { source: string; core_ids: string[]; entries: PackageEntry[] };
/** Mirrors `tau_core::package::{PackageItem, PackagePlan}`: a reviewed,
 * not-yet-written install/update, following the same plan -> confirm ->
 * execute shape as `Plan`/`PlaylistPlan`. */
export type PackageItem = { path: string; state: 'only_left' | 'only_right' | 'different' | 'identical'; bytes: number };
export type PackagePlan = { id: string; source: string; destination: string; items: PackageItem[]; new_files: number; updated_files: number; unchanged_files: number; bytes_to_write: number };
/** Mirrors `tau_core::package::PackageReport`, the result of `execute_package_install`. */
export type PackageReport = { written: number; unchanged: number; bytes_written: number };
/** Mirrors `tau_core::remove::RemovePlan`: what removing one installed core
 * would delete. `platform_shared` is true when another installed core still
 * uses the same platform, in which case the shared `Assets/<platform>`
 * files are deliberately left out of `paths`. */
export type RemovePlan = { id: string; card_root: string; core_id: string; platform: string; platform_shared: boolean; paths: string[]; files_to_remove: number; bytes_to_remove: number };
/** Mirrors `tau_core::remove::RemoveReport`, the result of `execute_remove_core`. */
export type RemoveReport = { removed_files: number; bytes_removed: number };
/** Mirrors `tau_core::taud::TaudTest`: one Check test's id, name, result and
 * raw value from a fully decoded TAUD1 QR report. */
export type TaudTest = { id: number; name: string; result: string; value: number; busy_permille: number | null; audio_full: boolean | null };
export type TaudBuild = { firmware: string; bitstream: string; flags: number; heap_gap: number };
export type TaudCycles = { read_min: number | null; read_avg: number; read_max: number; write_min: number | null; write_avg: number; write_max: number };
export type TaudAudio = { late_underruns: number; audio_full: boolean; stall_ms: number; window_s: number };
export type TaudDecodeProfile = { h_pct: number; i_pct: number; s_pct: number; r_pct: number };
export type TaudDecodeSweepEntry = { track: number; speed_pct: number; h_pct: number; i_pct: number; s_pct: number; r_pct: number; title: string };
export type TaudEntries = { build: TaudBuild | null; memory: number[]; sdram: TaudCycles | null; sdram_raw: number[]; psram: TaudCycles | null; psram_raw: number[]; cold: number[]; time: number[]; audio: TaudAudio | null; audio_raw: number[]; library: number[]; settings: number[]; errors: number[]; notes: number[]; decode_profile: TaudDecodeProfile | null; decode_profile_raw: number[]; decode_sweep: TaudDecodeSweepEntry[] };
export type TaudUnknownEntry = { tag: number; hex: string };
/** Mirrors `tau_core::taud::TaudReport`: the full Check report decoded from
 * a `TAUD1:` QR code (or its text payload directly), as opposed to
 * `CheckSummary`'s tiny 4-word persisted pass/fail summary. */
export type TaudReport = { format: number; profile: string; tests: TaudTest[]; entries: TaudEntries; unknown: TaudUnknownEntry[]; verdict: string };
/** Mirrors `tau_core::screenshots::ScreenshotEntry`: one screenshot found
 * under a card's `Memories/Screenshots/` folder. */
export type ScreenshotEntry = { path: string; filename: string; bytes: number; captured_at: string | null };
