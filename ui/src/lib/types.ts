export type Warning = { code: string; message: string };
export type Core = { id: string; author: string; version: string; platform: string; library_capable: boolean; index_status: string; tracks: number | null };
export type Plan = { id: string; new_files: number; updates: number; unchanged: number; bytes_to_write: number; warnings: Warning[] };
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
export type SyncReport = { plan_id: string; copied: number; unchanged: number; bytes_written: number; deleted: number; index_path: string; index_sha256: string; warnings: Warning[] };
/** What a rejected `invoke()` resolves to now that the engine's errors carry
 * a stable numeric `code` (see `tau_core::ErrorCode`) across the boundary
 * instead of a flattened English string. */
export type ApiError = { code: number; message: string };
/** Mirrors `tau_core::Progress`, delivered as a `"tau://progress"` window event. */
export type ProgressEvent = { job_id: string; stage: string; done: number; total: number; path: string | null };
