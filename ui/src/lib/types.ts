export type Warning = { code: string; message: string };
export type Core = { id: string; author: string; shortname: string; version: string; platform: string; platforms: string[]; media_platform: string; platform_category: string | null; library_capable: boolean; audio_formats: string[]; playlist: PlaylistSupport | null; index_status: string; tracks: number | null };
/** A core that plays music only from a playlist file: read from the core's own data slots (and the track limit its author documents). */
export type PlaylistSupport = { file: string; fixed_name: boolean; max_tracks: number | null; max_bytes: number | null; name_limit: number | null; list_file: string | null; formats: string[] };
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
export type ProblemKind = 'duplicate' | 'missing_tag' | 'unsupported_format' | 'path_issue' | 'missing_cover' | 'will_not_play' | 'replay_gain';
export type Problem = { kind: ProblemKind; files: string[]; message: string };
export type TrackRow = { rel: string; title: string; artist: string; album: string; secs: number; format: string };
export type LibraryScan = { tracks: TrackRow[]; playlists: Playlist[]; warnings: Warning[] };
/** The engine's own `tau_core::sync::SyncReport`, returned directly from
 * execute_sync/execute_core_copy/execute_core_move now that it implements
 * `Serialize` (P1-1) -- no hand-written result DTO duplicates it. */
export type SyncReport = { plan_id: string; copied: number; unchanged: number; bytes_written: number; deleted: number; art_sidecars_written: number; converted: number; replaygain_tagged: number; index_path: string; index_sha256: string; warnings: Warning[] };
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
export type RemovePlan = { id: string; card_root: string; core_id: string; platform: string; platform_shared: boolean; paths: string[]; files_to_remove: number; bytes_to_remove: number ; media_kept: boolean };
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
export type TaudEntries = { build: TaudBuild | null; memory: number[]; sdram: TaudCycles | null; sdram_raw: number[]; psram: TaudCycles | null; psram_raw: number[]; cold: number[]; time: number[]; audio: TaudAudio | null; audio_raw: number[]; library: number[]; settings: number[]; errors: number[]; notes: number[]; decode_profile: TaudDecodeProfile | null; decode_profile_raw: number[]; decode_sweep: TaudDecodeSweepEntry[]; stack: TaudStack | null; stack_raw: number[]; decode_profile2: TaudDecodeProfile2 | null; decode_profile2_raw: number[]; info_export: TaudInfoExport | null; meter_config: TaudMeterConfig | null; meter_trace: TaudTraceFrame[]; heap: TaudHeap | null; heap_raw: number[]; load: TaudLoad | null; load_raw: number[]; load2: TaudLoad2 | null; load2_raw: number[]; info_rows: TaudInfoRow[]; now_playing: TaudNowPlaying | null };
export type TaudHeap = { peak_bytes: number; heap_size: number; free_bytes: number };
export type TaudLoad = { busy_pct: number; busy_worst_pct: number; io_pct: number; secs: number; sub_fdct_pct: number; sub_hw_pct: number };
export type TaudLoad2 = { dec_pct: number; feed_pct: number; push_pct: number; ui_pct: number; wait_pct: number };
export type TaudInfoRow = { row: number; label: string; value: string };
export type TaudNowPlaying = { state: string; queue_pos: number; queue_len: number; title: string; artist: string; album: string };
export type TaudStack = { peak_bytes: number; stack_size: number; free_bytes: number };
export type TaudDecodeProfile2 = { d_pct: number; a_pct: number; x_pct: number; u_pct: number; t_pct: number; c1_pct: number; lpc_max_ms: number | null };
export type TaudInfoExport = { firmware: string; fpga_rev: string; window_read_cyc: number; free_ram: number; underruns: number; draw_stall_ms: number; load_ms: number; cpu_pct: number };
export type TaudMeterConfig = { meter_id: number; schema: number; preset: number | null; param_count: number; raw_hex: string };
export type TaudTraceFrame = { dt_ms: number; spec: number[]; wave: number[] };
export type TaudUnknownEntry = { tag: number; hex: string };
/** Mirrors `tau_core::taud::TaudReport`: the full Check report decoded from
 * a `TAUD1:` QR code (or its text payload directly), as opposed to
 * `CheckSummary`'s tiny 4-word persisted pass/fail summary. */
export type TaudReport = { format: number; profile: string; tests: TaudTest[]; entries: TaudEntries; unknown: TaudUnknownEntry[]; verdict: string };
/** Mirrors `tau_core::screenshots::ScreenshotEntry`: one screenshot found
 * under a card's `Memories/Screenshots/` folder. */
export type ScreenshotEntry = { path: string; filename: string; bytes: number; captured_at: string | null };

/** Mirrors `tau_core::workbench::{AlbumInfo, TrackInfo, PlaylistInfo, LibraryListing}`. */
export type AlbumInfo = { id: string; dest_id: string; title: string; artist: string; year: string | null; tracks: number; bytes: number; has_cover: boolean };
export type TrackInfo = { rel: string; album_id: string; title: string; artist: string; album: string; secs: number; bytes: number; format: string };
export type PlaylistInfo = { name: string; file: string; tracks: number };
export type IndexLimits = { max_tracks: number; max_albums: number; max_artists: number };
export type LibraryListing = { limits: IndexLimits; albums: AlbumInfo[]; tracks: TrackInfo[]; playlists: PlaylistInfo[]; warnings: Warning[]; files_reused?: number; files_read?: number };
/** Mirrors `tau_core::tagedit::{FieldEdits, EditRequest}`. `null` leaves a field alone; `''` clears it. */
export type FieldEdits = { title: string | null; artist: string | null; album: string | null; album_artist: string | null; year: string | null };
export type EditRequest = { album_id: string; track: string | null; fields: FieldEdits; cover: string | null };
export type ChangeRequest = { library_root: string | null; add_albums: string[]; remove_albums: string[]; edits: EditRequest[]; options: { mirror: boolean; embed_covers: boolean; art_sidecar_pal256: boolean }; playlist?: { file: string; max_tracks: number | null; max_bytes: number | null; core_name: string } | null };
export type ChangePlanView = { id: string; new_files: number; updated_files: number; unchanged_files: number; bytes_to_write: number; removed_files: number; bytes_to_remove: number; edited_files: number; playlists_updated: number; warnings: Warning[]; converted_files: number; replaygain_files: number; skipped: { file: string; reason: string }[] };
export type ChangeReport = { phase: string; plan_id: string; copied: number; unchanged: number; bytes_written: number; edited: number; reapplied: number; deleted: number; playlists_updated: number; backup_dir: string | null; index_path: string; playlist_file?: string | null; playlist_tracks?: number };
export type ConnectionKind = 'direct_usb' | 'card_reader' | 'unknown';
export type ConnectionInfo = { kind: ConnectionKind; detail: string };
/** What the app tells the engine about a core, so the card breakdown does not re-read every index. */
export type CoreRef = { id: string; shortname: string; platform: string; media_platform: string; library_capable: boolean };
/** One Tau platform folder's share of the card (cores sharing a platform share a segment). */
export type MediaSegment = { platform: string; core_ids: string[]; shortnames: string[]; bytes_on_disk: number; files: number };
/** Where the card's space goes: Tau media per platform versus everything else (sizes on disk). */
export type CardBreakdown = { total_bytes: number; available_bytes: number; unit: number; segments: MediaSegment[]; tau_bytes: number; other_bytes: number };
/** The OS's answer to "unmount and eject this card". `ok` is true only when it is safe to unplug. */
export type EjectResult = { ok: boolean; message: string };
/** Whether read-backs after a write are checked against the card itself, not memory. */
export type ReadbackStatus = { checks_the_device: boolean; failed_evictions: number };
export type Mp3Quality = { vbr: number } | { cbr: number };
export type ConvertSettings = { enabled: boolean; mode: 'auto' | 'manual'; max_rate: number; flac_bits: number; reduce_playable: boolean; mp3: Mp3Quality; lame_path: string | null; lame_dir: string | null; analysis_cache: string | null; replaygain: 'off' | 'add_missing' | 'rebuild' };
export type LameInstallOption = { method: string; label: string; command: string; runnable: boolean };
export type LameStatus = { path: string | null; version: string | null; options: LameInstallOption[] };
export type Prefs = { convert: ConvertSettings; history_keep_last: number; history_keep_days: number; remove_mode: 'backup' | 'ask' | 'none'; backup_dir: string | null; remove_explained: boolean; slow_alert_suppressed: boolean; check_updates: boolean; update_notice_shown: boolean; card_marker: 'ask' | 'on' | 'off'; core_disable: 'ask' | 'auto' | 'never'; core_keep: Record<string, string[]>; developer_tools: boolean; speeds: Record<string, number>; connections: Record<string, string> };
export type PrefsView = Prefs & { default_backup_dir: string; reports_dir: string };

/** Extra fields the engine now records in a sync journal (see `tau_core::journal::JournalSummary`). */
export type HistoryEntry = JournalSummary & { error_code: number | null; started_at_unix: number | null; duration_secs: number | null; edited: number | null; bytes_written: number | null; bytes_per_sec: number | null; phase: string | null; context: HistoryContext | null };
/** What the Library screen stores with each sync so history can describe it in words. */
export type HistoryContext = { card?: string; core?: string; connection?: string; items?: { kind: 'add' | 'remove' | 'edit'; title: string; artist?: string; tracks?: number; bytes?: number; note?: string; label?: string }[] };
export type ChangeResult = ChangeReport & { journal: string };

/** One album's cover thumbnail: a base64 PNG, or null when it has no readable picture. */
export type Thumbnail = { id: string; png_base64: string | null; /** Only the dev mock sets this; the engine always returns PNG. */ mime?: string };

/** One staged change as the Library's Details panel lists it. */
export type StagedItem = {
  key: string;
  kind: 'add' | 'remove' | 'edit';
  label: string;
  artist?: string;
  tracks?: number;
  bytes?: number;
  /** Accessible name of the row's button, e.g. "Remove Moanin from pending changes". */
  action: string;
  /** Visible text of the row's button: Remove, Undo or Discard. */
  verb: string;
};

/** What the top bar shows while a sync runs: the same card, with the text area carrying the sync's progress. */
export type SyncView = {
  title: string;
  phase: string;
  /** Bytes copied so far and in total (total 0 = not known yet). */
  done: number;
  total: number;
  /** "12 MB of 83 MB · 2.1 MB/s · about 2 min left". */
  line: string;
  note: string;
  slow: string;
  /** Planning, before any copying: only the bar and a status line are shown, with no Cancel. */
  preparing?: boolean;
};

// ---- Appearance (theme file) ----------------------------------------------------------------------
export type PolarityInput = { bg_luma: number; colors: Record<string, string> };
export type ThemeInput = { name: string; dark: PolarityInput; light: PolarityInput };
export type ContrastCheck = { polarity: 'dark' | 'light'; text: string; against: string; worst: number; needed: number; ok: boolean };
export type ThemeReport = { problems: string[]; checks: ContrastCheck[]; dark_snapped: Record<string, string>; light_snapped: Record<string, string> };
export type ExistingAssets = { bytes: number; sha256: string; themes: string[]; other_sections: string[]; readable: boolean };
export type ThemeFileReader = { core_id: string; version: string; declares_slot: boolean };
export type AssetsInstallPlan = { id: string; destination: string; bytes: number; sha256: string; themes: string[]; presets: string[]; existing: ExistingAssets | null; readers: ThemeFileReader[]; interrupted_install: boolean; warnings: string[] };
export type AssetsInstallReport = { destination: string; bytes_written: number; replaced: boolean; backup: string | null };

// ---- Send diagnostics ----
export type DiagFile = { path: string; bytes: number; sha256: string };
export type DiagIndex = { tracks: number; albums: number; artists: number; playlists: number; build_id: number };
export type DiagCore = { id: string; version: string; date_release: string; platform: string; persist_path: string | null; check: CheckSummary | null; check_note: string | null; files: DiagFile[]; index: DiagIndex | null; media_files: number; media_bytes: number };
export type DiagShot = { filename: string; captured_at: string | null; report: TaudReport | null; note: string | null };
export type DiagReading = { card: string; total_bytes: number; free_bytes: number; cores: DiagCore[]; shots: DiagShot[]; shots_examined: number; notes: string[] };
export type DiagView = { reading: DiagReading; summary: string };

/** One downloadable file of a GitHub release. */
export type ReleaseAsset = { name: string; url: string; size: number };
export type GithubRelease = { tag: string; title: string; prerelease: boolean; published: string; assets: ReleaseAsset[] };
/** The answer to "is there a newer Tau release". `newer` is null when no card was open to compare with. */
export type UpdateCheck = { latest: GithubRelease; newer: boolean | null; installed_release: string | null; zips: ReleaseAsset[]; manifest: ReleaseAsset | null; sums: ReleaseAsset | null; message: string; channel: 'stable' | 'preview' | 'dev' | null; others: string[] };
export type Downloaded = { name: string; path: string; bytes: number };

/** Mirrors `tau_core::update` / `install_plan` / `install_exec` (serde output). */
export type UpdateVerdict = 'new_install' | 'same_build' | 'update' | 'same_date_different_build' | 'older' | 'mismatch';
export type PairStatus = 'no_marker' | 'cannot_verify' | 'not_applicable' | { verified: { core_version: string } } | { mismatch: { accepts: string[]; bitstream: string } } | { missing_feature: { missing: string[]; bitstream_has: string[] } };
export type BuildIdentity = { core_id: string; shortname: string; version: string; date_release: string; platform: string };
export type UpdateAssessment = { verdict: UpdateVerdict; reasons: string[]; installed: BuildIdentity | null; package: BuildIdentity; pair: PairStatus; installed_release: string | null; package_release: string | null };
export type BackupFile = { path: string; bytes: number };
export type InstallPlan = {
  id: string; update: { cores: UpdateAssessment[]; files_replaced: string[]; user_files_kept: string[]; persist_changed: number[] | null };
  files: PackagePlan; backup: BackupFile[]; backup_bytes: number; obsolete_to_remove: string[]; caches_to_clear: string[]; stubs_to_sweep: string[];
  user_files_kept: string[]; superseded_candidates: string[]; capacity: { space: { available_bytes: number }; bytes_needed: number; margin_bytes: number; fits: boolean } | null;
  nothing_to_do: boolean; refused: string | null; cautions: string[]; allow_downgrade: boolean;
};
export type CheckItem = { name: string; status: 'pass' | 'warn' | 'fail'; detail: string };
export type PostInstallReport = { core_id: string; items: CheckItem[]; verdict: 'pass' | 'warn' | 'fail'; summary: string };
export type InstallReport = { backup_dir: string; files_written: number; bytes_written: number; files_backed_up: number; obsolete_removed: number; caches_cleared: number; stubs_swept: number; nothing_to_do: boolean; checks: PostInstallReport[] };
export type RollbackReport = { restored: number; created_removed: number };

/** Mirrors `tau_core::refresh`. */
export type RefreshReason = 'unsupported_format' | 'non_ascii_name' | 'name_collision' | 'path_too_long' | 'tags_unreadable' | 'old_tag_version' | 'over_capacity';
export type RefreshFinding = { rel: string; reason: RefreshReason; message: string; fix: string | null; skipped: boolean };
export type IndexState = { present: boolean; valid: boolean; tracks: number | null; root: string | null; root_matches: boolean; missing_files: number; albums_without_cover_file: number };
export type CoverFilePlan = { id: string; albums: number; have_cover_file: number; items: { source_folder: string; destination: string; cover_source: string; from_embedded: boolean }[]; no_picture: string[] };
export type RefreshPlan = { id: string; media_root: string; root_prefix: string; before: IndexState; media_files: number; would_index: number; findings: RefreshFinding[]; renames: { from: string; to: string }[]; playlist_fixes: { file: string; lines: number }[]; stubs_to_remove: number; refused: string | null; nothing_to_do: boolean };
export type RefreshReport = { backup_dir: string; renamed: number; playlists_rewritten: number; stubs_removed: number; before: IndexState; after: IndexState; index_path: string; nothing_to_do: boolean };

/** The Spotlight setting and whether the card holding a path has the `.metadata_never_index` marker. */
export type MarkerStatus = { setting: 'ask' | 'on' | 'off'; is_card: boolean; present: boolean };

/** Mirrors `tau_core::settings_migrate`. */
export type MigrationPlan = { id: string; card_root: string; from_core: string; to_core: string; source: string; dest: string; allowed: boolean; reasons: string[]; changed_ids: [number, string][]; bytes: number };
export type MigrationReport = { dest: string; bytes: number; created_dirs: string[] };

/** Mirrors `tau_core::halcyon::HalcyonPreset`: six control positions, or raw Q2.22 biquads (`[b0, b1, b2, a1, a2]`). */
export type HalcyonPreset =
  | { kind: 'control'; name: string; controls: [number, number, number, number, number, number] }
  | { kind: 'raw'; name: string; preamp: number; stages: number[][] };
export type ApoImport = { preset: HalcyonPreset; preamp_db: number; peak_gain_db: number; filters: number };

/** Mirrors `omega_update::OmegaUpdate` (Tau Omega's own updater, not the Tau core update check). */
export type OmegaUpdate = { current: string; version: string; notes: string | null; date: string | null };

/** Mirrors `tau_core::catalog` / `catalog_release` / `core_update` / `core_package` and `tau_net::marketplace` (serde output). */
export type CatalogAsset = { platform: string; filename: string; extensions: string[]; core_specific: boolean };
export type ReleaseSummary = { version: string; release_date: string; download_url: string };
export type CatalogCore = {
  id: string; author: string; name: string; repository: { owner: string; name: string }; sponsor: { links: string[] };
  category: string; manufacturer: string; year: number; platform_id: string;
  version: string; release_date: string; download_url: string; requires_license: boolean; license_filename: string;
  replaces: string[]; declares_ai: string[]; firmware_required: string; dock_supported: boolean | null; link_port: boolean | null; sleep_supported: boolean | null;
  assets: CatalogAsset[]; history: ReleaseSummary[];
};
export type MarketSource = 'Network' | 'NotModified' | 'Cache' | { Offline: string };
export type CatalogResult = { catalog: { cores: CatalogCore[] }; source: MarketSource; fetched: number };
export type ArtKind = 'Icon' | 'Banner';
export type CoreStatusKind = 'UpToDate' | 'UpdateAvailable' | 'NewerThanCatalog' | 'Unknown' | 'NotInCatalog' | 'Excluded' | { ReplacedBy: string };
export type CoreStatus = { id: string; installed_version: string; latest_version: string; release_date: string; status: CoreStatusKind; manual_url?: string | null };
export type CoreUpdates = { rows: CoreStatus[]; source: MarketSource; fetched: number };
export type Vetted = { core_id: string; version: string; date_release: string; platform_ids: string[]; firmware_required: string; dock_supported: boolean | null; files: number; uncompressed_bytes: number; asset_files: number };
export type MarketplaceDownload = { download: { path: string; bytes: number; sha256: string; checksum: 'Verified' | 'NotPublished' }; vetted: Vetted };

/** Mirrors `tau_core::corehub` / `core_files` / `core_settings` / `core_screens` / `core_add` (serde output). */
export type PlatformInfo = { id: string; name: string; category: string; manufacturer: string; year: number; has_image: boolean };
export type CoreRow = { id: string; shortname: string; author: string; version: string; platform: string; platforms: string[]; missing: number; disabled: boolean };
export type PlatformNode = { platform: PlatformInfo; cores: CoreRow[] };
export type SlotInfo = { id: string; name: string; required: boolean; filename: string; extensions: string[]; nonvolatile: boolean; core_specific: boolean; platform: string; size_exact: number | null; size_maximum: number | null };
export type CoreDetail = {
  id: string; shortname: string; author: string; description: string; url: string; version: string; date_release: string; firmware_required: string;
  dock_supported: boolean | null; link_port: boolean | null; sleep_supported: boolean | null; platforms: PlatformInfo[]; slots: SlotInfo[]; has_icon: boolean; has_interact: boolean;
};
export type FolderRole = 'AssetsShared' | 'AssetsCore' | 'SavesShared' | 'SavesCore' | 'SaveStates';
export type EntryKind = 'Rom' | 'Bios' | 'Tape' | 'Disk' | 'Save' | 'State' | 'Other';
export type FolderRef = { role: FolderRole; platform: string; label: string; rel_path: string };
export type FileEntry = { rel_path: string; name: string; bytes: number; modified: number; kind: EntryKind; slot: string | null; game: string | null; taken: string | null; group: string; purpose: string; purpose_source: string; support: boolean };
export type FolderListing = { folder: FolderRef; exists: boolean; entries: FileEntry[]; total_files: number; total_bytes: number; truncated: boolean };
export type MissingFile = { slot: string; filename: string; folder_rel: string; size_exact: number | null };
export type BackupZip = { name: string; bytes: number; modified: number };
export type CoreFiles = { folders: FolderListing[]; missing: MissingFile[]; save_backups: BackupZip[] };
export type SettingValue = { id: number; name: string; kind: string; value: number; display: string; default_display: string; known: boolean };
export type SavedFile = { group: string; rel_path: string; bytes: number; modified: number };
export type CoreSettings = { values: SettingValue[]; files: SavedFile[]; presets: number; presets_folder: string; unreadable: string | null };
export type ShotMeta = { core: string; version: string; loaded_file: string; platform_id: string; platform_name: string };
export type Shot = { file: string; taken: string; bytes: number; meta: ShotMeta | null; recorded: boolean; likely: string | null };
export type ScreenshotView = { shots: Shot[]; total: number; recorded: number; likely: number; with_meta: number; states: number; window_secs: number };
export type AddDestination = 'Shared' | 'CoreOnly';
export type AddState = 'New' | 'Identical' | 'Different' | { Refused: string };
export type AddItem = { source: string; dest_rel: string; bytes: number; sha256: string; state: AddState };
export type AddPlan = {
  id: string; core_id: string; platform: string; destination: AddDestination; folder_rel: string; replace: boolean; items: AddItem[];
  new_files: number; identical: number; different: number; refused: number; bytes_to_write: number; warnings: string[]; blocked: string | null;
};
export type AddReport = { written: number; replaced: number; skipped_identical: number; skipped_different: number; refused: number; bytes_written: number; backup_dir: string | null };

export type RemoveItem = { rel_path: string; bytes: number; shared_with: string[]; needed_by: string | null };
export type RemoveFilesPlan = { id: string; core_id: string; items: RemoveItem[]; total_bytes: number; warnings: string[]; blocked: string | null };
export type RemoveFilesReport = { removed: number; bytes_removed: number; backup_dir: string };

/** Adding a core that is not in the openFPGA Library (docs/MARKETPLACE_SPEC.md section 7). */
export type ManualClass = { kind: string; ok: boolean; reason: string };
export type ManualNeed = { slot: string; filename: string; extensions: string[]; required: boolean; size_exact: number | null; size_maximum: number | null; core_specific: boolean; folder_rel: string; in_package: boolean; you_supply: boolean; purpose: string };
export type ManualPlatform = { id: string; name: string; category: string; manufacturer: string; in_package: boolean };
export type ManualPreview = {
  core_id: string; shortname: string; author: string; description: string; url: string; version: string; date_release: string; firmware_required: string;
  dock_supported: boolean | null; link_port: boolean | null; sleep_supported: boolean | null; platforms: ManualPlatform[]; needs: ManualNeed[]; settings: string[]; info_text: string;
  has_bitstream: boolean; sha256: string; zip_bytes: number; files: number; uncompressed_bytes: number; asset_files: number; warnings: string[]; icon: string | null; banner: string | null;
};
export type ManualProvenance = { kind: string; owner: string; repo: string; tag: string; asset: string; bytes: number; sha256: string; checksum: string; packed: string | null };
export type ManualReleaseChoice = { tag: string; name: string; published: string; prerelease: boolean; zips: string[] };
export type ManualInspect =
  | { status: 'ready'; preview: ManualPreview; zip_path: string; provenance: ManualProvenance; readme: { text: string; images: string[] } | null; in_library: boolean; installed_version: string | null; different_source: boolean; lint: LintReport }
  | { status: 'choose'; owner: string; repo: string; releases: ManualReleaseChoice[]; tag: string };
export type ManualInspectArgs = { input: string; tag?: string | null; asset?: string | null; readme: boolean; card?: string | null };

/** Why a core cannot work on this card, and what would make it work. */
export type HealthReason = { code: string; message: string; fix: string };
export type CoreHealth = { id: string; shortname: string; verdict: 'Healthy' | 'Broken' | 'Unusable'; reasons: HealthReason[]; needs_no_files: boolean };
export type CoreSuggestions = { mode: 'ask' | 'auto' | 'never'; cores: CoreHealth[] };
/** A core moved out of `Cores/` so the Pocket does not list it. */
export type DisabledCore = { id: string; shortname: string; version: string; disabled_at: number; reasons: string[] };
export type StatePlan = { token: string; enable: boolean; ids: string[]; from: string[]; to: string[]; blocked: string | null; notes: string[] };
export type StateReport = { moved: number; caches_cleared: number };

export type LintFinding = { id: string; severity: 'Error' | 'Warning' | 'Note'; message: string; rule: string; fix: string };
export type LintReport = { core_id: string; findings: LintFinding[]; errors: number; warnings: number; notes: number };

/** Developer builds (the Builds page). */
export type SchemeField = 'core_id' | 'shortname' | 'version' | 'file';
export type BuildScheme = { name: string; field: SchemeField; template: string };
export type BuildsConfig = { dir: string | null; builtin: BuildScheme[]; custom: BuildScheme[] };
export type Placement = { scheme: string; series: string; order: [number, number, number, number]; pre: string | null };
export type BuildEntry = { path: string; kind: 'zip' | 'folder'; file: string; core_id: string; shortname: string; version: string; date_release: string; bytes: number; modified: number; placement: Placement | null; lint_errors: number; lint_warnings: number; error: string | null };
export type BuildRelation = 'not_installed' | 'same' | 'newer' | 'older' | 'different';
export type InstalledCore = { id: string; shortname: string; version: string };
export type BuildRow = { entry: BuildEntry; relation: BuildRelation; older: InstalledCore[] };
export type BuildList = { dir: string; rows: BuildRow[] };
export type SchemeTry = { error: string | null; placement: Placement | null };

/** Developer results (the Results page). */
export type DiffChange = 'better' | 'worse' | 'changed';
export type DiffLine = { kind: 'verdict' | 'test' | 'build' | 'metric'; label: string; before: string; after: string; change: DiffChange };
export type ResultShot = { filename: string; path: string; captured_at: string | null; bytes: number; since_install: boolean; report: TaudReport | null; problem: string | null };
export type ResultRun = { profile: string; latest: string; previous: string | null; diff: DiffLine[]; since_install: boolean };
export type ResultsView = { since: string | null; shots: ResultShot[]; runs: ResultRun[] };

/** Run notes (the Results page's second tab). */
export type RunResult = { profile: string; captured_at: string; verdict: string; failing: string[]; firmware: string; bitstream: string; changes: DiffLine[] };
export type RunNote = { id: string; card: string; stamp: string; core_id: string; shortname: string; version: string; package: string; package_sha256: string; bitstreams: [string, string][]; backup_dir: string; files_written: number; text: string; lint_overrides: string[]; results: RunResult[]; results_at: string | null };

/** The Check page. */
export type CheckResult = { title: string; lint: LintReport; symptoms: LintFinding[] };
export type CardCheckRow = { id: string; shortname: string; errors: number; warnings: number; notes: number };

/** Carrying media from the previous build. */
export type CarryKind = 'folder' | 'library' | 'shared';
export type CarryCandidate = { kind: CarryKind; from_id: string; from_name: string; platform: string; source_rel: string; dest_rel: string; files: number; bytes: number; note: string };
export type CarryItem = { rel: string; bytes: number; state: 'new' | 'identical' | 'different' };
export type CarryPlan = { id: string; source_rel: string; dest_rel: string; items: CarryItem[]; new_files: number; identical: number; different: number; bytes_to_write: number; blocked: string | null };
export type CarryReport = { copied: number; identical: number; left_different: number; bytes_written: number };

/** The Builder: a raw bitstream plus a template core becomes an installable package. */
export type TemplateSource = { kind: 'card'; card: string; id: string } | { kind: 'zip'; path: string } | { kind: 'folder'; path: string };
export type ExtraFile = { src: string; dest: string };
export type Fit = 'contain' | 'cover' | 'stretch';
export type PlatformEdit = { id: string; name: string; category: string; manufacturer: string; year: number | null };
export type TemplateEdits = { icon_fit: Fit; author: string | null; url: string | null; firmware_required: string | null; dock_supported: boolean | null; link_port: boolean | null; sleep_supported: boolean | null; platform_ids: string[] | null; platform: PlatformEdit | null; banner: { path: string; fit: Fit } | null };
export type BuildSpec = { rbf: string; template: TemplateSource; shortname: string; version: string; date_release: string; description: string | null; icon_png: string | null; extra: ExtraFile[]; edits: TemplateEdits };
export type TemplateViewData = { core_id: string; shortname: string; author: string; description: string; url: string; version: string; date_release: string; firmware_required: string; dock_supported: boolean | null; link_port: boolean | null; sleep_supported: boolean | null; platform_ids: string[]; platform: PlatformEdit | null; files: [string, number][]; icon: string | null; banner: string | null };
export type TemplateInfo = { core_id: string; shortname: string; author: string; version: string; description: string; platforms: string[] };
export type PlanFile = { rel: string; bytes: number; origin: 'bitstream' | 'template' | 'generated' | 'icon' | 'banner' | 'extra'; sha256: string };
export type BuildPlan = { core_id: string; files: PlanFile[]; raw_sha256: string; converted_sha256: string; bitstream_bytes: number; lint: LintReport; blocked: string | null };
export type BuildResult = { zip: string; record: string; zip_bytes: number; zip_sha256: string; plan: BuildPlan };
export type NextName = { scheme: string; next: string };
