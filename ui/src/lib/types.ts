export type Warning = { code: string; message: string };
export type Core = { id: string; author: string; version: string; platform: string; library_capable: boolean; index_status: string; tracks: number | null };
export type Plan = { id: string; new_files: number; updates: number; unchanged: number; bytes_to_write: number; warnings: Warning[] };
export type Setting = { id: number; kind: string; value: unknown };
export type Job = { kind: string; status: string; detail: string };
export type Difference = { relative: string; state: 'only_left' | 'only_right' | 'different' | 'identical'; left_bytes: number | null; right_bytes: number | null };
export type Comparison = { left: string; right: string; only_left: number; only_right: number; different: number; identical: number; differences: Difference[] };
export type Playlist = { name: string; tracks: number };
export type MediaScan = { playlists: Playlist[]; warnings: Warning[] };
export type DuplicateGroup = { files: string[] };
export type LibrarySummary = { tracks: number; playlists: number; warnings: Warning[] };
/** What a rejected `invoke()` resolves to now that the engine's errors carry
 * a stable numeric `code` (see `tau_core::ErrorCode`) across the boundary
 * instead of a flattened English string. */
export type ApiError = { code: number; message: string };
