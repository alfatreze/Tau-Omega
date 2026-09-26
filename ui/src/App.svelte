<script lang="ts">
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import type { BackupPlan, CapacityCheck, CheckSummary, Comparison, Core, Difference, Job, JournalSummary, LibraryScan, MediaScan, PackageManifest, PackagePlan, PackageReport, Plan, PlaylistPlan, Problem, RemovePlan, RemoveReport, ScreenshotEntry, Setting, TaudReport } from './lib/types';
  import { invoke } from './lib/backend';
  import { cancelJob, checkStorageCapacity, compareMedia, errorMessage, executeCoreCopy, executeCoreMove, executePackageInstall, executePlaylistImport, executePlaylistRename, executePlaylistWrite, executeRemoveCore, executeSync, exportPlaylist, findProblems, getManualPlayers, getRecentCards, getReportsDir, inspectCard, inspectPackage, listJournals, listMountedCards, listScreenshots, newJobId, onProgress, planBackup, planCoreCopy, planPackageInstall, planPlaylistImport, planPlaylistRename, planPlaylistWrite, planRemoveCore, planSync, previewArtSidecar, readCheckSummary, readCoreIcon, readImageDataUrl, readJournal, readPersistedSettings, readPlatformImage, readQrReport, recordRecentCard, scanLibrary, scanMedia, setManualPlayer, setReportsDir } from './lib/tau-api';
  import SettingsView from './lib/SettingsView.svelte';
  import JobsView from './lib/JobsView.svelte';
  import PlaylistsView from './lib/PlaylistsView.svelte';
  import ProblemsView from './lib/ProblemsView.svelte';
  import LibraryView from './lib/LibraryView.svelte';
  import BackupView from './lib/BackupView.svelte';
  import PackageView from './lib/PackageView.svelte';
  let page: 'cards' | 'compare' | 'sync' | 'jobs' | 'settings' | 'playlists' | 'problems' | 'library' | 'backup' | 'package' = 'cards'; let cores: Core[] = []; let path = ''; let jobs: Job[] = []; let journalPath = ''; let journalNotice = 'Choose a host-side Tau Omega journal to load it.'; let problemsPath = ''; let problems: Problem[] | null = null; let problemsNotice = 'Choose a media root to inspect it for problems.'; let problemsLoading = false; let playlistPath = ''; let playlistResult: MediaScan | null = null; let playlistNotice = 'Choose a media root to inspect playlists.'; let playlistOutput = ''; let selectedPlaylist = ''; let settingsPath = ''; let settings: Setting[] = []; let settingsNotice = 'Choose a persisted settings file to inspect it.';
  let backupSourcePath = ''; let backupDestPath = ''; let backupPlanResult: BackupPlan | null = null; let backupNotice = ''; let backupCapacity: CapacityCheck | null = null;
  let packageZipPath = ''; let packageCardPath = ''; let packageManifest: PackageManifest | null = null; let packageInspectNotice = ''; let packagePlan: PackagePlan | null = null; let packagePlanNotice = ''; let packageReport: PackageReport | null = null; let packageConfirmNotice = '';
  async function inspectPackageZip() { packageManifest = null; packagePlan = null; packageReport = null; packagePlanNotice = ''; packageConfirmNotice = ''; try { packageManifest = await inspectPackage(packageZipPath); packageInspectNotice = `${packageManifest.entries.length} files found. Nothing was changed.`; } catch (error) { packageInspectNotice = `Could not read this package: ${errorMessage(error)}`; } }
  async function reviewPackageInstall() { packagePlan = null; packageReport = null; packagePlanNotice = ''; packageConfirmNotice = ''; try { packagePlan = await planPackageInstall(packageZipPath, packageCardPath); packagePlanNotice = `Plan ${packagePlan.id} is ready for review. Nothing has been written.`; } catch (error) { packagePlanNotice = `Could not plan this install: ${errorMessage(error)}`; } }
  async function confirmPackageInstall() { if (!packagePlan) return; try { packageReport = await executePackageInstall(packageZipPath, packageCardPath, packagePlan.id); packageConfirmNotice = 'Installed and verified. Nothing else was changed.'; packagePlan = null; } catch (error) { packageConfirmNotice = `Nothing was reported as complete: ${errorMessage(error)}`; } }
  let removeCores: Core[] = []; let removeCoresNotice = ''; let removeCoreId = ''; let removePlan: RemovePlan | null = null; let removePlanNotice = ''; let removeReport: RemoveReport | null = null; let removeConfirmNotice = '';
  async function loadCoresToRemove() { removeCores = []; removeCoreId = ''; removePlan = null; removeReport = null; removePlanNotice = ''; removeConfirmNotice = ''; try { removeCores = await inspectCard(packageCardPath); removeCoresNotice = `${removeCores.length} core${removeCores.length === 1 ? '' : 's'} found. This is a read-only inspection.`; } catch (error) { removeCoresNotice = `Could not inspect this card: ${errorMessage(error)}`; } }
  async function reviewRemoveCore() { removePlan = null; removeReport = null; removeConfirmNotice = ''; try { removePlan = await planRemoveCore(packageCardPath, removeCoreId); removePlanNotice = `Plan ${removePlan.id} is ready for review. Nothing has been deleted.`; } catch (error) { removePlanNotice = `Could not plan this removal: ${errorMessage(error)}`; } }
  async function confirmRemoveCore() { if (!removePlan) return; try { removeReport = await executeRemoveCore(packageCardPath, removeCoreId, removePlan.id); removeConfirmNotice = 'Removed and verified.'; removePlan = null; await loadCoresToRemove(); } catch (error) { removeConfirmNotice = `Nothing was reported as complete: ${errorMessage(error)}`; } }
  let libraryPath = ''; let libraryScan: LibraryScan | null = null; let libraryNotice = 'Choose a media root to inspect it.'; let libraryLoading = false; let libraryJobId = ''; let libraryProgress = ''; let librarySearch = ''; let libraryFormat: 'all' | 'mp3' | 'flac' = 'all';
  let syncJobId = ''; let syncProgress = '';
  let reportsDir = ''; let historyNotice = 'Choose a folder to keep durable job history across restarts.'; let history: JournalSummary[] = []; let journalDetail: unknown = null; let journalDetailNotice = '';
  onMount(async () => {
    try {
      const dir = await getReportsDir();
      if (dir) { reportsDir = dir; await refreshHistory(); }
    } catch (error) { historyNotice = `Could not read the saved reports directory: ${errorMessage(error)}`; }
    await refreshKnownCards();
    // Prefer a Pocket that's actually mounted right now over a merely
    // remembered path -- that's what the user almost certainly wants to see
    // first. Fall back to the most recently opened card otherwise, so the
    // app never opens to an empty Cards screen once it has seen any card.
    const toOpen = mountedCards[0] ?? recentCards[0];
    if (toOpen) { path = toOpen; await openFolder(); }
    // Catches an eject the app wasn't open to see happen live -- "possibly
    // only a refresh, in case I eject the card" is exactly what regaining
    // focus already means: the user just switched back to this window. This
    // component lives for the whole app session, so the listener is never
    // removed.
    window.addEventListener('focus', checkCardMounted);
  });
  async function chooseReportsDir() {
    const selected = await open({ directory: true });
    if (!selected || Array.isArray(selected)) return;
    reportsDir = selected;
    try { await setReportsDir(reportsDir); await refreshHistory(); } catch (error) { historyNotice = `Could not save this directory: ${errorMessage(error)}`; }
  }
  async function refreshHistory() {
    if (!reportsDir) { history = []; return; }
    try { history = await listJournals(reportsDir); historyNotice = `${history.length} job${history.length === 1 ? '' : 's'} found. Nothing was changed.`; }
    catch (error) { history = []; historyNotice = `Could not list this directory: ${errorMessage(error)}`; }
  }
  async function openJournalDetail(path: string) { journalDetailNotice = ''; try { journalDetail = await readJournal(path); } catch (error) { journalDetail = null; journalDetailNotice = `Could not read this journal: ${errorMessage(error)}`; } }
  function closeJournalDetail() { journalDetail = null; journalDetailNotice = ''; }
  const autoJournalPath = (kind: string) => `${reportsDir}/${Date.now()}-${kind}.json`;
  onProgress((event) => { if (event.job_id === syncJobId) syncProgress = `${event.stage}: ${event.done}${event.total ? ` / ${event.total}` : ''}${event.path ? ` (${event.path})` : ''}`; if (event.job_id === libraryJobId) libraryProgress = `${event.stage}: ${event.done}${event.total ? ` / ${event.total}` : ''}${event.path ? ` (${event.path})` : ''}`; });
  let notice = 'Open a card folder to inspect it. Tau Omega will not write anything at this stage.';
  let sources = ''; let destination = ''; let manifestPath = ''; let embedCovers = false; let artSidecar = false; let plan: Plan | null = null; let syncNotice = '';
  // Lazy per-cover preview cache: cover_source -> data URL, or 'error' on
  // failure. Loaded on click, not eagerly for every album in the plan (same
  // "no eager thumbnail loading" precedent as the Settings screenshot
  // gallery), since a plan can cover a large batch of albums at once.
  let artPreviews: Record<string, string> = {};
  let artPreviewsLoading: Record<string, boolean> = {};
  async function loadArtPreview(coverSource: string) {
    if (artPreviews[coverSource] || artPreviewsLoading[coverSource]) return;
    artPreviewsLoading = { ...artPreviewsLoading, [coverSource]: true };
    try {
      artPreviews = { ...artPreviews, [coverSource]: await previewArtSidecar(coverSource) };
    } catch (error) {
      artPreviews = { ...artPreviews, [coverSource]: 'error' };
    } finally {
      artPreviewsLoading = { ...artPreviewsLoading, [coverSource]: false };
    }
  }
  let leftCore = ''; let rightCore = ''; let comparison: Comparison | null = null; let compareNotice = ''; let copyReportPath = ''; let coreCopyPlan: Plan | null = null; let moveSource = false; let moveBackupPath = ''; let approveDeletion = false;
  async function chooseFolder(target: 'card' | 'source' | 'destination' | 'left' | 'right' | 'manifest' | 'copyReport' | 'backup' | 'settings' | 'playlists' | 'problems' | 'journal' | 'library' | 'importSource' | 'backupSource' | 'backupDestination' | 'packageZip' | 'packageCard' | 'qrScreenshot' | 'screenshotCard') {
    const selected = await open({ directory: !['manifest', 'copyReport', 'settings', 'journal', 'importSource', 'packageZip', 'qrScreenshot'].includes(target), multiple: target === 'source' });
    if (!selected) return;
    const value = Array.isArray(selected) ? selected.join('\n') : selected;
    if (target === 'card') { path = value; await openFolder(); } if (target === 'source') sources = value; if (target === 'destination') destination = value;
    if (target === 'left') leftCore = value; if (target === 'right') rightCore = value; if (target === 'manifest') manifestPath = value; if (target === 'copyReport') copyReportPath = value; if (target === 'backup') moveBackupPath = value; if (target === 'settings') settingsPath = value; if (target === 'playlists') playlistPath = value; if (target === 'problems') problemsPath = value; if (target === 'journal') journalPath = value; if (target === 'library') libraryPath = value; if (target === 'importSource') importSource = value; if (target === 'backupSource') backupSourcePath = value; if (target === 'backupDestination') backupDestPath = value; if (target === 'packageZip') packageZipPath = value; if (target === 'packageCard') packageCardPath = value; if (target === 'qrScreenshot') qrPath = value; if (target === 'screenshotCard') screenshotCardPath = value;
  }
  let checkSummary: CheckSummary | null = null;
  async function loadSettings() { checkSummary = null; try { settings = await readPersistedSettings(settingsPath); settingsNotice = `${settings.length} persisted values loaded. Nothing was changed.`; try { checkSummary = await readCheckSummary(settingsPath); } catch { /* summary is informational; settings still load without it */ } } catch (error) { settings = []; settingsNotice = `Could not read settings: ${errorMessage(error)}`; } }
  let qrPath = ''; let qrReport: TaudReport | null = null; let qrNotFound = false; let qrNotice = '';
  async function decodeQrScreenshot() { qrReport = null; qrNotFound = false; qrNotice = ''; try { const result = await readQrReport(qrPath); if (result) { qrReport = result; qrNotice = `Decoded: ${result.profile} · ${result.verdict}.`; } else { qrNotFound = true; qrNotice = 'No QR code found in this image.'; } } catch (error) { qrNotice = `Could not decode this screenshot: ${errorMessage(error)}`; } }
  let screenshotCardPath = ''; let screenshots: ScreenshotEntry[] = []; let screenshotsNotice = '';
  async function browseScreenshots() { screenshots = []; try { screenshots = await listScreenshots(screenshotCardPath); screenshotsNotice = screenshots.length ? `${screenshots.length} screenshots found. This is a read-only listing.` : 'No screenshots found on this card.'; } catch (error) { screenshotsNotice = `Could not list screenshots: ${errorMessage(error)}`; } }
  let selectedScreenshotPath = ''; let screenshotPreviewUrl: string | null = null; let screenshotPreviewLoading = false;
  async function selectScreenshot(path: string) {
    selectedScreenshotPath = path; qrPath = path; screenshotPreviewUrl = null; screenshotPreviewLoading = true;
    const [previewResult, decodeResult] = await Promise.allSettled([readImageDataUrl(path), (async () => { await decodeQrScreenshot(); })()]);
    if (previewResult.status === 'fulfilled') screenshotPreviewUrl = previewResult.value;
    screenshotPreviewLoading = false;
  }
  async function scanPlaylists() { try { playlistResult = await scanMedia(playlistPath, newJobId()); playlistNotice = `${playlistResult.playlists.length} playlists found. Nothing was changed.`; } catch (error) { playlistResult = null; playlistNotice = `Could not scan this folder: ${errorMessage(error)}`; } }
  async function scanProblems() { problemsLoading = true; try { problems = await findProblems(problemsPath); problemsNotice = `${problems.length} problem${problems.length === 1 ? '' : 's'} found. Nothing was changed.`; } catch (error) { problems = null; problemsNotice = `Could not scan this folder: ${errorMessage(error)}`; } finally { problemsLoading = false; } }
  async function inspectLibrary() { libraryLoading = true; libraryJobId = newJobId(); libraryProgress = 'starting…'; librarySearch = ''; libraryFormat = 'all'; try { libraryScan = await scanLibrary(libraryPath, libraryJobId); libraryNotice = `${libraryScan.tracks.length} tracks found. Nothing was changed.`; } catch (error) { libraryScan = null; libraryNotice = `Could not inspect this folder: ${errorMessage(error)}`; } finally { libraryLoading = false; libraryProgress = ''; } }
  async function cancelLibraryScan() { if (libraryJobId) await cancelJob(libraryJobId); }
  $: libraryRows = (libraryScan?.tracks ?? []).filter((row) => (libraryFormat === 'all' || row.format.toLowerCase() === libraryFormat) && (!librarySearch.trim() || `${row.title} ${row.artist} ${row.album} ${row.rel}`.toLowerCase().includes(librarySearch.trim().toLowerCase())));
  async function loadJournal() { try { const journal = await readJournal(journalPath) as { state?: string; plan?: { files?: number }; result?: { copied?: number } }; jobs = [{ kind: 'Loaded journal', status: journal.state === 'completed' ? 'Completed' : journal.state ?? 'Unknown', detail: `${journal.result?.copied ?? 0} copied · ${journal.plan?.files ?? 0} planned` }, ...jobs]; journalNotice = 'Journal loaded. Nothing was changed.'; } catch (error) { journalNotice = `Could not load journal: ${errorMessage(error)}`; } }
  async function exportSelectedPlaylist() { try { await exportPlaylist(playlistPath, selectedPlaylist, playlistOutput); playlistNotice = `Exported ${selectedPlaylist}. Nothing in the source library was changed.`; } catch (error) { playlistNotice = `Could not export playlist: ${errorMessage(error)}`; } }

  $: selectedPlaylistDetail = playlistResult?.playlists.find((p) => p.name === selectedPlaylist) ?? null;
  let reorderTracks: string[] = [];
  $: if (selectedPlaylistDetail && reorderTracks.length === 0) reorderTracks = [...selectedPlaylistDetail.tracks];
  $: if (!selectedPlaylistDetail) reorderTracks = [];
  let reorderPlan: PlaylistPlan | null = null; let reorderNotice = '';
  function moveTrack(index: number, delta: -1 | 1) {
    const target = index + delta;
    if (target < 0 || target >= reorderTracks.length) return;
    const copy = [...reorderTracks];
    [copy[index], copy[target]] = [copy[target], copy[index]];
    reorderTracks = copy;
    reorderPlan = null;
  }
  async function reviewReorder() { if (!selectedPlaylistDetail) return; reorderPlan = null; reorderNotice = ''; try { reorderPlan = await planPlaylistWrite(playlistPath, selectedPlaylistDetail.file, reorderTracks); reorderNotice = `Plan ${reorderPlan.id} is ready for review. Nothing has been written.`; } catch (error) { reorderNotice = `Could not plan this change: ${errorMessage(error)}`; } }
  async function confirmReorder() { if (!reorderPlan || !selectedPlaylistDetail) return; try { await executePlaylistWrite(playlistPath, selectedPlaylistDetail.file, reorderTracks, reorderPlan.id); reorderNotice = 'Saved. Nothing else was changed.'; reorderPlan = null; await scanPlaylists(); } catch (error) { reorderNotice = `Nothing was reported as complete: ${errorMessage(error)}`; } }

  let renameNewFile = ''; let renamePlan: PlaylistPlan | null = null; let renameNotice = '';
  async function reviewRename() { if (!selectedPlaylistDetail || !renameNewFile.trim()) return; renamePlan = null; renameNotice = ''; try { renamePlan = await planPlaylistRename(playlistPath, selectedPlaylistDetail.file, renameNewFile.trim()); renameNotice = `Plan ${renamePlan.id} is ready for review. Nothing has been written.`; } catch (error) { renameNotice = `Could not plan this rename: ${errorMessage(error)}`; } }
  async function confirmRename() { if (!renamePlan || !selectedPlaylistDetail) return; try { await executePlaylistRename(playlistPath, selectedPlaylistDetail.file, renameNewFile.trim(), renamePlan.id); renameNotice = 'Renamed. Nothing else was changed.'; renamePlan = null; renameNewFile = ''; selectedPlaylist = ''; await scanPlaylists(); } catch (error) { renameNotice = `Nothing was reported as complete: ${errorMessage(error)}`; } }

  let createFile = ''; let createTracksText = ''; let createPlan: PlaylistPlan | null = null; let createNotice = '';
  async function reviewCreate() { createPlan = null; createNotice = ''; const tracks = createTracksText.split('\n').map((line) => line.trim()).filter(Boolean); try { createPlan = await planPlaylistWrite(playlistPath, createFile.trim(), tracks); createNotice = `Plan ${createPlan.id} is ready for review. Nothing has been written.`; } catch (error) { createNotice = `Could not plan this playlist: ${errorMessage(error)}`; } }
  async function confirmCreate() { if (!createPlan) return; const tracks = createTracksText.split('\n').map((line) => line.trim()).filter(Boolean); try { await executePlaylistWrite(playlistPath, createFile.trim(), tracks, createPlan.id); createNotice = 'Created. Nothing else was changed.'; createPlan = null; createFile = ''; createTracksText = ''; await scanPlaylists(); } catch (error) { createNotice = `Nothing was reported as complete: ${errorMessage(error)}`; } }

  let importSource = ''; let importDestFile = ''; let importPlan: PlaylistPlan | null = null; let importNotice = '';
  async function reviewImport() { importPlan = null; importNotice = ''; try { importPlan = await planPlaylistImport(playlistPath, importSource, importDestFile.trim()); importNotice = `Plan ${importPlan.id} is ready for review: ${importPlan.tracks.length} matched, ${importPlan.dropped.length} dropped.`; } catch (error) { importNotice = `Could not plan this import: ${errorMessage(error)}`; } }
  async function confirmImport() { if (!importPlan) return; try { await executePlaylistImport(playlistPath, importSource, importDestFile.trim(), importPlan.id); importNotice = 'Imported. Nothing else was changed.'; importPlan = null; importSource = ''; importDestFile = ''; await scanPlaylists(); } catch (error) { importNotice = `Nothing was reported as complete: ${errorMessage(error)}`; } }
  const settingLabel = (id: number) => ({ 10: 'Volume', 11: 'Colour index', 12: 'Repeat', 13: 'Shuffle', 15: 'Meter', 16: 'EQ', 18: 'Saved position', 19: 'Resume on', 24: 'Library history', 25: 'Index build ID', 26: 'Shuffle All seed', 27: 'Library off' } as Record<number, string>)[id] ?? `Word ${id}`;
  const settingValue = (setting: Setting) => { if (setting.id !== 24 || typeof setting.value !== 'number') return JSON.stringify(setting.value); const word = setting.value >>> 0; const kinds: Record<number, string> = { 1: 'album', 2: 'artist', 3: 'playlist', 4: 'all tracks A–Z', 5: 'Shuffle All' }; return `${kinds[word & 7] ?? 'unknown'} · item ${word >>> 3 & 0x7ff} · queue ${word >>> 14 & 0x3fff}`; };
  async function openFolder() {
    if (!path.trim()) { notice = 'Enter a staging-card folder path first.'; return; }
    const previousActiveId = activeCore?.id;
    try {
      cores = await inspectCard(path);
      notice = `${cores.length} core${cores.length === 1 ? '' : 's'} found. This is a read-only inspection.`;
      cardMounted = true;
      // Keep the same core selected across a refresh of the same card;
      // otherwise the reactive default-pick below chooses a fresh one.
      activeCore = cores.find((core) => core.id === previousActiveId) ?? null;
      recordRecentCard(path).then(() => refreshKnownCards()).catch(() => { /* remembering a card is best-effort, not a reason to fail the open */ });
    } catch (error) { notice = `Could not inspect this folder: ${errorMessage(error)}`; cores = []; activeCore = null; }
  }
  const cardName = (cardPath: string) => cardPath.split('/').filter(Boolean).pop() ?? cardPath;
  let mountedCards: string[] = []; let recentCards: string[] = []; let manualPlayers: string[] = [];
  $: knownCards = [...mountedCards, ...recentCards.filter((card) => !mountedCards.includes(card))];
  async function refreshKnownCards() {
    try { mountedCards = await listMountedCards(); } catch { mountedCards = []; }
    try { recentCards = await getRecentCards(); } catch { recentCards = []; }
    try { manualPlayers = await getManualPlayers(); } catch { manualPlayers = []; }
  }
  async function openKnownCard(card: string) { path = card; await openFolder(); }

  // Whether the currently open card is still there: null when the open path
  // isn't a removable volume at all (a plain staging folder never "ejects"),
  // so there is nothing to warn about either way.
  let cardMounted: boolean | null = null;
  async function checkCardMounted() {
    if (!path || !cores.length || !path.startsWith('/Volumes/')) return;
    try { mountedCards = await listMountedCards(); cardMounted = mountedCards.includes(path); } catch { /* a failed check isn't reason to flip to "ejected" */ }
  }

  // Takes `players` explicitly (rather than closing over `manualPlayers`)
  // so the `$:` statements below, which only track variables they reference
  // directly, see the real dependency and re-run when the list changes --
  // a closure over an outer variable is invisible to Svelte's reactivity.
  const isPlayerCore = (core: Core, players: string[]) => players.includes(core.id) || core.platform_category === 'Media Players' || (core.platform_category == null && (core.id.toLowerCase().includes('tau') || core.platform.toLowerCase().includes('tau')));
  $: playerCores = cores.filter((core) => isPlayerCore(core, manualPlayers));
  $: otherCores = cores.filter((core) => !isPlayerCore(core, manualPlayers));
  let showOtherCores = false;
  async function toggleManualPlayer(core: Core) { try { await setManualPlayer(core.id, true); manualPlayers = [...manualPlayers, core.id]; } catch (error) { notice = `Could not set ${core.id} as a player: ${errorMessage(error)}`; } }

  // Real per-core artwork (Cores/<id>/icon.bin, decoded server-side). `null`
  // means "asked, no icon" (most cores don't ship one) -- distinct from
  // "haven't asked yet" (key absent), so a core is only ever fetched once.
  let coreIcons: Record<string, string | null> = {};
  async function ensureCoreIcon(core: Core) {
    if (core.id in coreIcons) return;
    coreIcons = { ...coreIcons, [core.id]: null };
    try { const icon = await readCoreIcon(path, core.id); if (icon) coreIcons = { ...coreIcons, [core.id]: icon }; } catch { /* falls back to the monogram/bracket placeholder */ }
  }
  $: cores.forEach(ensureCoreIcon);

  // The real per-core artwork: a platform's own banner
  // (Platforms/_images/<platform>.bin), not a second copy of the small
  // icon. Shared by every core on that platform, so cached by platform id,
  // not core id -- two cores on the same platform (e.g. TAU and TAU
  // Diagnostic) fetch it once between them.
  let platformImages: Record<string, string | null> = {};
  async function ensurePlatformImage(core: Core) {
    if (!core.platform || core.platform in platformImages) return;
    platformImages = { ...platformImages, [core.platform]: null };
    try { const image = await readPlatformImage(path, core.platform); if (image) platformImages = { ...platformImages, [core.platform]: image }; } catch { /* falls back to the monogram placeholder */ }
  }
  $: cores.forEach(ensurePlatformImage);

  // The core the rest of the app is "working with" -- shown in the sidebar
  // and switchable there, so nowhere else has to guess. Defaults to the
  // first player core once the card's cores (and any manual overrides) are
  // known, but never overrides a choice the user or a card-refresh already
  // made (see `openFolder`'s own preserve-by-id logic).
  let activeCore: Core | null = null;
  $: if (!activeCore && playerCores.length) activeCore = playerCores[0];
  let showSwitcher = false;
  function selectActiveCore(core: Core) { activeCore = core; showSwitcher = false; openCoreLibrary(core); }
  async function switchToKnownCard(card: string) { showSwitcher = false; await openKnownCard(card); }

  let detailCore: Core | null = null;
  function showCoreDetail(core: Core) { detailCore = core; }
  function closeCoreDetail() { detailCore = null; }
  function openCoreLibrary(core: Core) {
    activeCore = core;
    if (!core.platform) { notice = `${core.id} has no declared platform, so its media root can't be located.`; return; }
    libraryPath = `${path.replace(/\/+$/, '')}/Assets/${core.platform}/common`;
    page = 'library';
    inspectLibrary();
  }

  let showHelp = false;
  let syncCapacity: CapacityCheck | null = null;
  async function makePlan() { plan = null; syncCapacity = null; syncNotice = ''; try { plan = await planSync(sources.split('\n'), destination, embedCovers, artSidecar); syncNotice = `Plan ${plan.id} is ready for review. Nothing has been written.`; try { syncCapacity = await checkStorageCapacity(destination, plan.bytes_to_write); } catch { /* capacity check is informational; a plan still reviews without it */ } } catch (error) { syncNotice = `Could not make a plan: ${errorMessage(error)}`; } }
  async function runSync() { if (!plan) return; syncJobId = newJobId(); syncProgress = 'starting…'; const manifest = reportsDir ? autoJournalPath('sync') : manifestPath; try { const result = await executeSync(sources.split('\n'), destination, plan.id, manifest, embedCovers, artSidecar, syncJobId); syncNotice = `Verified: ${result.copied} copied, ${result.unchanged} unchanged, ${result.art_sidecars_written} art sidecars. Index: ${result.index_path}`; jobs = [{ kind: 'Library sync', status: 'Completed', detail: `${result.copied} copied · ${result.unchanged} unchanged` }, ...jobs]; plan = null; if (reportsDir) await refreshHistory(); } catch (error) { syncNotice = `Nothing was reported as complete: ${errorMessage(error)}`; jobs = [{ kind: 'Library sync', status: 'Failed', detail: errorMessage(error) }, ...jobs]; if (reportsDir) await refreshHistory(); } finally { syncProgress = ''; } }
  async function cancelSync() { if (syncJobId) await cancelJob(syncJobId); }
  async function compareCores() { comparison = null; compareNotice = ''; try { comparison = await compareMedia(leftCore, rightCore); compareNotice = `${comparison.differences.length} supported files compared. Nothing was changed.`; } catch (error) { compareNotice = `Could not compare these media roots: ${errorMessage(error)}`; } }
  let coreCopyCapacity: CapacityCheck | null = null;
  async function reviewCoreCopy() { coreCopyPlan = null; coreCopyCapacity = null; moveSource = false; approveDeletion = false; try { coreCopyPlan = await planCoreCopy(leftCore, rightCore); compareNotice = `Copy plan ${coreCopyPlan.id} is ready for review. The first core remains untouched.`; try { coreCopyCapacity = await checkStorageCapacity(rightCore, coreCopyPlan.bytes_to_write); } catch { /* capacity check is informational; a plan still reviews without it */ } } catch (error) { compareNotice = `Could not make a copy plan: ${errorMessage(error)}`; } }
  async function runCoreCopy() { if (!coreCopyPlan) return; if (moveSource && !approveDeletion) { compareNotice = 'Confirm that the source will be backed up and deleted before moving.'; return; } if (moveSource && !moveBackupPath.trim()) { compareNotice = 'Choose a visible external backup folder before moving.'; return; } const jobId = newJobId(); const manifest = reportsDir ? autoJournalPath(moveSource ? 'core_move' : 'core_copy') : copyReportPath; try { const result = moveSource ? await executeCoreMove(leftCore, rightCore, coreCopyPlan.id, moveBackupPath, manifest, jobId) : await executeCoreCopy(leftCore, rightCore, coreCopyPlan.id, manifest, jobId); compareNotice = `Verified ${moveSource ? 'move' : 'copy'}: ${result.copied} copied, ${result.unchanged} unchanged. Index: ${result.index_path}`; coreCopyPlan = null; if (reportsDir) await refreshHistory(); } catch (error) { compareNotice = `Nothing was reported as complete: ${errorMessage(error)}`; if (reportsDir) await refreshHistory(); } }
  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  async function reviewBackup() { backupPlanResult = null; backupCapacity = null; backupNotice = ''; try { backupPlanResult = await planBackup(backupSourcePath, backupDestPath); backupNotice = `${backupPlanResult.items.length} files compared. Nothing was changed. This is a preview only -- copying isn't enabled yet.`; try { backupCapacity = await checkStorageCapacity(backupDestPath, backupPlanResult.bytes_to_write); } catch { /* capacity check is informational; a plan still reviews without it */ } } catch (error) { backupNotice = `Could not plan this backup: ${errorMessage(error)}`; } }
</script>

<main><aside aria-label="Primary navigation"><div class="brand"><span class="mark">τ</span><span>Tau Omega<small>Library companion</small></span></div>
  <div class="active-context">
    <button class="active-context-btn" on:click={() => showSwitcher = !showSwitcher} aria-expanded={showSwitcher} aria-label="Switch card or core">
      <span class="active-context-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none"><rect x="4" y="2" width="16" height="20" rx="3" stroke="currentColor" stroke-width="1.6"/><rect x="7" y="5" width="10" height="7" rx="1" stroke="currentColor" stroke-width="1.4"/><circle cx="9" cy="16.5" r="1.1" fill="currentColor"/><circle cx="15" cy="16.5" r="1.1" fill="currentColor"/><circle cx="12" cy="19.2" r="1.1" fill="currentColor"/></svg></span>
      <span class="active-context-text">
        <strong>{path ? cardName(path) : 'No card open'}</strong>
        <small>{activeCore ? (activeCore.shortname || activeCore.id) : (cores.length ? 'Choose a core' : 'Open a card to begin')}</small>
      </span>
      <svg class="active-context-chevron" class:open={showSwitcher} viewBox="0 0 24 24" fill="none" width="14" height="14" aria-hidden="true"><path d="m6 9 6 6 6-6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>
    </button>
    {#if showSwitcher}
      <div class="switcher-backdrop" role="presentation" on:click={() => showSwitcher = false}></div>
      <div class="switcher-panel">
        {#if knownCards.length}
          <p class="switcher-label">Switch card</p>
          {#each knownCards as card}<button class="switcher-row" class:active={path === card} on:click={() => switchToKnownCard(card)}>{cardName(card)}{#if mountedCards.includes(card)}<span class="switcher-dot" aria-hidden="true"></span>{/if}</button>{/each}
        {/if}
        {#if playerCores.length}
          <p class="switcher-label">Switch core</p>
          {#each playerCores as core}<button class="switcher-row" class:active={activeCore?.id === core.id} on:click={() => selectActiveCore(core)}>{core.shortname || core.id}</button>{/each}
        {/if}
        <button class="switcher-manage" on:click={() => { page = 'cards'; showSwitcher = false; }}>Manage cards &amp; cores →</button>
      </div>
    {/if}
  </div>
  <nav><button class:active={page === 'cards'} on:click={() => page = 'cards'}>Cards</button><button class:active={page === 'compare'} on:click={() => page = 'compare'}>Compare cores</button><button class:active={page === 'sync'} on:click={() => page = 'sync'}>Sync library</button><button class:active={page === 'playlists'} on:click={() => page = 'playlists'}>Playlists</button><button class:active={page === 'backup'} on:click={() => page = 'backup'}>Backup</button><button class:active={page === 'package'} on:click={() => page = 'package'}>Packages</button><button class:active={page === 'problems'} on:click={() => page = 'problems'}>Problems</button><button class:active={page === 'library'} on:click={() => page = 'library'}>Library</button><button class:active={page === 'jobs'} on:click={() => page = 'jobs'}>Recent jobs</button><button class:active={page === 'settings'} on:click={() => page = 'settings'}>Settings</button></nav><p class="offline">Local only<br/><span>No card writes without a reviewed plan.</span></p></aside>
  {#if page === 'cards'}<section class="page" id="cards"><header><div><p class="eyebrow">CARD LIBRARY</p><h1>Start with a card</h1><p class="lede">Inspect a Pocket card or a staging folder. Your music stays untouched.</p></div><button class="primary" on:click={() => chooseFolder('card')}>Open folder</button></header>
    {#if cardMounted === false}<div class="ejected-banner" role="status"><span>This card is no longer connected.</span><button class="quiet" on:click={() => openKnownCard(path)}>Reconnect</button></div>{/if}
    {#if knownCards.length}<section class="known-cards" aria-labelledby="known-cards-title"><p class="eyebrow">QUICK OPEN</p><h2 id="known-cards-title">Known cards</h2><div class="known-card-list">{#each knownCards as card}<button class="known-card" class:active={path === card} on:click={() => openKnownCard(card)}><span class="known-card-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none"><rect x="4" y="2" width="16" height="20" rx="3" stroke="currentColor" stroke-width="1.6"/><rect x="7" y="5" width="10" height="7" rx="1" stroke="currentColor" stroke-width="1.4"/><circle cx="9" cy="16.5" r="1.1" fill="currentColor"/><circle cx="15" cy="16.5" r="1.1" fill="currentColor"/><circle cx="12" cy="19.2" r="1.1" fill="currentColor"/></svg></span><span class="known-card-body"><strong class="known-card-name">{cardName(card)}</strong><span class="known-card-badge" class:mounted={mountedCards.includes(card)}>{mountedCards.includes(card) ? 'Available now' : 'Recently used'}</span><span class="known-card-path">{card}</span></span></button>{/each}</div></section>{/if}
    <p class="notice" role="status">{notice}</p>
    {#if cores.length}
      <section aria-labelledby="player-title">
        <div class="section-title"><div><p class="eyebrow">MEDIA PLAYERS</p><h2 id="player-title">Player cores on this card</h2></div><button class="quiet refresh-btn" aria-label="Refresh this card" title="Refresh" on:click={openFolder}><svg viewBox="0 0 24 24" fill="none" width="16" height="16"><path d="M20 12a8 8 0 1 1-2.34-5.66M20 4v5h-5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg></button></div>
        {#if playerCores.length}
          <div class="player-grid">
            {#each playerCores as core}
              <article class="player-card">
                <button class="player-card-main" on:click={() => openCoreLibrary(core)}>
                  <div class="player-card-art" aria-hidden="true">{#if platformImages[core.platform]}<img src={platformImages[core.platform]} alt="" />{:else}<span>{(core.shortname || core.id).slice(0, 2).toUpperCase()}</span>{/if}</div>
                  <h3>{core.shortname || core.id}</h3>
                  <div class="player-card-dev">{#if coreIcons[core.id]}<img class="player-card-dev-icon" src={coreIcons[core.id]} alt="" />{:else}<svg viewBox="0 0 24 24" fill="none" width="13" height="13" aria-hidden="true"><path d="M8 6 3 12l5 6M16 6l5 6-5 6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>{/if}<span>{core.author || 'Unknown developer'}</span></div>
                  <p class="player-card-meta">{core.tracks === null ? 'No index yet' : `${core.tracks} tracks`}</p>
                </button>
                <div class="player-card-footer">
                  <span class:capable={core.library_capable} class="chip">{core.library_capable ? 'Library ready' : 'Legacy core'}</span>
                  <button class="quiet" aria-label={`Details for ${core.id}`} title="Details" on:click={() => showCoreDetail(core)}>
                    <svg viewBox="0 0 24 24" fill="none" width="16" height="16"><circle cx="12" cy="12" r="9" stroke="currentColor" stroke-width="1.6"/><path d="M12 11v5.5M12 8v.01" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>
                  </button>
                </div>
              </article>
            {/each}
          </div>
        {:else}<section class="empty"><div class="empty-art">τ</div><h2>No player cores found</h2><p>This card has {cores.length} other core{cores.length === 1 ? '' : 's'} installed. Show them below to set one as a player.</p></section>{/if}
      </section>
      {#if otherCores.length}
        <section aria-labelledby="other-core-title">
          <div class="section-title"><div><p class="eyebrow">EVERYTHING ELSE</p><h2 id="other-core-title">Other cores on this card</h2></div><label class="show-all"><input type="checkbox" bind:checked={showOtherCores}/> Show {otherCores.length} other core{otherCores.length === 1 ? '' : 's'}</label></div>
          {#if showOtherCores}<div class="core-list">{#each otherCores as core}<article><div class="core-icon">{#if coreIcons[core.id]}<img src={coreIcons[core.id]} alt="" />{:else}{core.id.split('.').at(-1)?.[0] ?? '?'}{/if}</div><div><h3>{core.id}</h3><p>{core.author || 'Unknown author'} · {core.version || 'Version unknown'} · {core.platform || 'No platform declared'}</p></div><button class="quiet" on:click={() => toggleManualPlayer(core)}>Set as player</button></article>{/each}</div>{/if}
        </section>
      {/if}
    {:else}<section class="empty"><div class="empty-art">◒</div><h2>No card selected</h2><p>Open a card folder to see its cores and library health.</p></section>{/if}</section>
  {:else if page === 'sync'}<section class="page sync-page"><header><div><p class="eyebrow">SAFE SYNC</p><h1>Review before copying</h1><p class="lede">Sources are read-only. Every file is verified, then the index is published last.</p></div></header><section class="wizard" aria-labelledby="sync-title"><div class="steps" aria-label="Sync steps"><span class="current">1 Sources</span><span class:current={plan}>2 Plan</span><span>3 Confirm</span></div><h2 id="sync-title">Build a sync plan</h2><label for="sources">Source folders or files — one per line</label><div class="picker-row"><textarea id="sources" bind:value={sources} placeholder="/Users/me/Music/Album"></textarea><button class="picker" on:click={() => chooseFolder('source')}>Choose</button></div><label for="destination">Destination media root</label><div class="picker-row"><input id="destination" bind:value={destination} placeholder="/Volumes/Pocket/Assets/tau/common"/><button class="picker" on:click={() => chooseFolder('destination')}>Choose</button></div><label for="manifest">Report location on this computer</label><div class="picker-row"><input id="manifest" bind:value={manifestPath} placeholder="/Users/me/Documents/tau-sync-report.json"/><button class="picker" on:click={() => chooseFolder('manifest')}>Choose</button></div><label class="cover-option"><input type="checkbox" bind:checked={embedCovers}/> Add folder cover art to MP3 and FLAC copies <small>Baseline JPEG only. Your originals are never changed.</small></label><label class="cover-option"><input type="checkbox" bind:checked={artSidecar}/> Write a tau-art cover thumbnail alongside each album <small>tau-alpha's decided format (palette-256, 128px); no firmware reader exists yet, so this has no effect on the Pocket today.</small></label><button class="primary" on:click={makePlan}>Review plan</button></section>
    {#if plan}<section class="plan-card" aria-labelledby="plan-title"><div><p class="eyebrow">READY FOR CONFIRMATION</p><h2 id="plan-title">{plan.id}</h2><p class="lede">{plan.new_files} new · {plan.updates} updated · {plan.unchanged} unchanged · {size(plan.bytes_to_write)} to write{plan.art_sidecars ? ` · ${plan.art_sidecars} art sidecar${plan.art_sidecars === 1 ? '' : 's'}` : ''}</p>{#if syncCapacity}<p class:capacity-ok={syncCapacity.fits} class:capacity-bad={!syncCapacity.fits}>{syncCapacity.fits ? 'Fits' : 'Does not fit'} on the destination volume — {size(syncCapacity.space.available_bytes)} free.</p>{/if}{#if plan.art_sidecar_previews.length}<div class="art-preview-list">{#each plan.art_sidecar_previews as preview (preview.cover_source)}<div class="art-preview-item"><span class="art-preview-folder" title={preview.cover_source}>{preview.folder}</span>{#if artPreviews[preview.cover_source] === 'error'}<span class="art-preview-error">Could not preview</span>{:else if artPreviews[preview.cover_source]}<img class="art-preview-thumb" src={artPreviews[preview.cover_source]} alt="Palette-256 preview of the {preview.folder} cover"/>{:else}<button class="quiet" disabled={artPreviewsLoading[preview.cover_source]} on:click={() => loadArtPreview(preview.cover_source)}>{artPreviewsLoading[preview.cover_source] ? 'Loading…' : 'Preview'}</button>{/if}</div>{/each}</div>{/if}</div><button class="danger" on:click={runSync}>Confirm and sync</button>{#if syncProgress}<p class="notice" role="status">{syncProgress} <button class="quiet" on:click={cancelSync}>Cancel</button></p>{/if}{#if plan.warnings.length}<ul>{#each plan.warnings as warning}<li>{warning.message}</li>{/each}</ul>{/if}<p class="safety">This action targets the exact destination above. It copies, hashes, verifies, and writes the index last.</p></section>{/if}<p class="notice" role="status">{syncNotice}</p></section>
  {:else if page === 'compare'}<section class="page compare-page"><header><div><p class="eyebrow">CORE TRANSFER</p><h1>Compare, then copy safely</h1><p class="lede">Compare two Tau media roots before copying the complete library to the second core.</p></div></header><section class="wizard" aria-labelledby="compare-title"><h2 id="compare-title">Compare two cores</h2><label for="left-core">Source media root</label><div class="picker-row"><input id="left-core" bind:value={leftCore} placeholder="/Volumes/Pocket/Assets/tau/common"/><button class="picker" on:click={() => chooseFolder('left')}>Choose</button></div><label for="right-core">Destination media root</label><div class="picker-row"><input id="right-core" bind:value={rightCore} placeholder="/Volumes/Pocket/Assets/tau-test/common"/><button class="picker" on:click={() => chooseFolder('right')}>Choose</button></div><label for="copy-report">Copy report location on this computer</label><div class="picker-row"><input id="copy-report" bind:value={copyReportPath} placeholder="/Users/me/Documents/tau-core-copy.json"/><button class="picker" on:click={() => chooseFolder('copyReport')}>Choose</button></div><button class="primary" on:click={compareCores}>Compare safely</button></section><p class="notice" role="status">{compareNotice}</p>{#if comparison}<section class="comparison" aria-labelledby="comparison-title"><div class="section-title"><div><p class="eyebrow">RESULT</p><h2 id="comparison-title">{comparison.differences.length} files compared</h2></div><span>Read-only</span></div><div class="comparison-counts"><span><b>{comparison.only_left}</b> only in source</span><span><b>{comparison.only_right}</b> only in destination</span><span><b>{comparison.different}</b> different</span><span><b>{comparison.identical}</b> matching</span></div><ul class="difference-list">{#each comparison.differences as item}<li><span class={`difference ${item.state}`}>{item.state === 'only_left' ? 'Source only' : item.state === 'only_right' ? 'Destination only' : item.state === 'different' ? 'Different' : 'Matching'}</span><span>{item.relative}</span><span>{item.left_bytes === null ? '—' : size(item.left_bytes)} / {item.right_bytes === null ? '—' : size(item.right_bytes)}</span></li>{/each}</ul><button class="primary" on:click={reviewCoreCopy}>Review full-library copy</button>{#if coreCopyPlan}<div class="copy-plan"><p class="eyebrow">READY FOR CONFIRMATION</p><h3>{coreCopyPlan.id}</h3><p>{coreCopyPlan.new_files} new · {coreCopyPlan.updates} updated · {coreCopyPlan.unchanged} unchanged · {size(coreCopyPlan.bytes_to_write)} to write</p>{#if coreCopyCapacity}<p class:capacity-ok={coreCopyCapacity.fits} class:capacity-bad={!coreCopyCapacity.fits}>{coreCopyCapacity.fits ? 'Fits' : 'Does not fit'} on the destination volume — {size(coreCopyCapacity.space.available_bytes)} free.</p>{/if}<button class="danger" on:click={runCoreCopy}>Confirm and copy</button></div>{/if}<p class="safety">The source is never changed. The destination files are verified and its index is rebuilt last. Move remains unavailable until its separate backup and deletion review is ready.</p></section>{/if}</section>
  {:else if page === 'jobs'}
    <JobsView {jobs} bind:journalPath notice={journalNotice} chooseJournal={() => chooseFolder('journal')} {loadJournal} startSync={() => page = 'sync'} {reportsDir} {historyNotice} {history} {chooseReportsDir} {refreshHistory} detail={journalDetail} detailNotice={journalDetailNotice} openDetail={openJournalDetail} closeDetail={closeJournalDetail} />
  {:else if page === 'playlists'}
    <PlaylistsView bind:path={playlistPath} result={playlistResult} notice={playlistNotice} bind:output={playlistOutput} bind:selected={selectedPlaylist} choose={() => chooseFolder('playlists')} scan={scanPlaylists} exportList={exportSelectedPlaylist}
      {selectedPlaylistDetail} {reorderTracks} {moveTrack} {reorderPlan} {reorderNotice} {reviewReorder} {confirmReorder}
      bind:renameNewFile {renamePlan} {renameNotice} {reviewRename} {confirmRename}
      bind:createFile bind:createTracksText {createPlan} {createNotice} {reviewCreate} {confirmCreate}
      bind:importSource bind:importDestFile {importPlan} {importNotice} chooseImportSource={() => chooseFolder('importSource')} {reviewImport} {confirmImport} />
  {:else if page === 'problems'}
    <ProblemsView bind:path={problemsPath} {problems} notice={problemsNotice} loading={problemsLoading} choose={() => chooseFolder('problems')} scan={scanProblems} />
  {:else if page === 'library'}
    <LibraryView bind:path={libraryPath} scan={libraryScan} rows={libraryRows} notice={libraryNotice} loading={libraryLoading} progress={libraryProgress} bind:search={librarySearch} bind:format={libraryFormat} choose={() => chooseFolder('library')} inspect={inspectLibrary} cancel={cancelLibraryScan} />
  {:else if page === 'backup'}
    <BackupView bind:sourcePath={backupSourcePath} bind:destinationPath={backupDestPath} chooseSource={() => chooseFolder('backupSource')} chooseDestination={() => chooseFolder('backupDestination')} review={reviewBackup} backupPlan={backupPlanResult} {backupNotice} capacity={backupCapacity} />
  {:else if page === 'package'}
    <PackageView bind:zipPath={packageZipPath} bind:cardPath={packageCardPath} chooseZip={() => chooseFolder('packageZip')} chooseCard={() => chooseFolder('packageCard')} inspect={inspectPackageZip} manifest={packageManifest} inspectNotice={packageInspectNotice} review={reviewPackageInstall} plan={packagePlan} planNotice={packagePlanNotice} confirm={confirmPackageInstall} report={packageReport} confirmNotice={packageConfirmNotice} loadCoresToRemove={loadCoresToRemove} removeCores={removeCores} removeCoresNotice={removeCoresNotice} bind:removeCoreId={removeCoreId} reviewRemove={reviewRemoveCore} removePlan={removePlan} removePlanNotice={removePlanNotice} confirmRemove={confirmRemoveCore} removeReport={removeReport} removeConfirmNotice={removeConfirmNotice} />
  {:else if page === 'settings'}
    <SettingsView bind:settingsPath {settings} {checkSummary} notice={settingsNotice} choose={() => chooseFolder('settings')} read={loadSettings} done={() => page = 'cards'} label={settingLabel} value={settingValue} bind:qrPath chooseQr={() => chooseFolder('qrScreenshot')} decodeQr={decodeQrScreenshot} {qrReport} {qrNotFound} {qrNotice} bind:screenshotCardPath chooseScreenshotCard={() => chooseFolder('screenshotCard')} {browseScreenshots} {screenshots} {screenshotsNotice} {selectScreenshot} {selectedScreenshotPath} {screenshotPreviewUrl} {screenshotPreviewLoading} />
  {/if}
</main>

<button class="help-fab" aria-label="Help" title="Help" on:click={() => showHelp = !showHelp}>?</button>
{#if showHelp}
  <div class="help-backdrop" role="presentation" on:click={() => showHelp = false}></div>
  <section class="help-panel" aria-labelledby="help-title">
    <div class="help-panel-header"><h2 id="help-title">Using Tau Omega</h2><button class="quiet" aria-label="Close help" on:click={() => showHelp = false}>✕</button></div>
    <div class="help-panel-body">
      <p><strong>Open folder</strong> browses to a Pocket card or a plain staging folder (anything with <code>Cores</code> and <code>Assets</code>). A mounted Pocket and any card you've opened before show up under <strong>Known cards</strong> for one-click reopening.</p>
      <p>Cores that look like media players (Tau, or anything whose platform declares itself a "Media Players" core) get a large card here. Anything else can be shown with "Show other cores" and promoted with <strong>Set as player</strong>.</p>
      <p>Every write anywhere in the app goes through <strong>plan → review → confirm</strong> — nothing is copied, moved, or deleted until you've seen exactly what will change and confirmed it.</p>
      <p>This is all read-only until you explicitly confirm a plan on Sync, Compare cores, Backup, or Packages.</p>
    </div>
  </section>
{/if}

{#if detailCore}
  <div class="detail-backdrop" role="presentation" on:click={closeCoreDetail}></div>
  <aside class="detail-panel" aria-labelledby="detail-title">
    <div class="detail-panel-header"><h2 id="detail-title">{detailCore.shortname || detailCore.id}</h2><button class="quiet" aria-label="Close details" on:click={closeCoreDetail}>✕</button></div>
    <div class="detail-panel-body">
      <div><span>Core ID</span><strong>{detailCore.id}</strong></div>
      <div><span>Developer</span><strong>{detailCore.author || 'Unknown'}</strong></div>
      <div><span>Version</span><strong>{detailCore.version || 'Unknown'}</strong></div>
      <div><span>Platform</span><strong>{detailCore.platform || 'None declared'}</strong></div>
      {#if detailCore.platform_category}<div><span>Category</span><strong>{detailCore.platform_category}</strong></div>{/if}
      <div><span>Library</span><strong>{detailCore.library_capable ? 'Library ready' : 'Legacy core'}</strong></div>
      <div><span>Index</span><strong>{detailCore.index_status}</strong></div>
      <div><span>Tracks</span><strong>{detailCore.tracks ?? '—'}</strong></div>
    </div>
    <button class="primary" on:click={() => { if (detailCore) openCoreLibrary(detailCore); closeCoreDetail(); }}>Open library</button>
  </aside>
{/if}

{#if page === 'compare' && coreCopyPlan}
  <section class="move-review" aria-labelledby="move-review-title">
    <div class="move-review-card">
      <p class="eyebrow">TRANSFER CONFIRMATION</p>
      <h2 id="move-review-title">Copy or move this library?</h2>
      <p>{coreCopyPlan.new_files} new · {coreCopyPlan.updates} updated · {coreCopyPlan.unchanged} unchanged · {size(coreCopyPlan.bytes_to_write)} to write</p>
      <label><input type="checkbox" bind:checked={moveSource}/> Move after the destination copy verifies</label>
      {#if moveSource}
        <label for="move-backup">External backup folder</label>
        <div class="picker-row"><input id="move-backup" type="text" bind:value={moveBackupPath} placeholder="/Users/me/Documents/Tau Backups"/><button class="picker" on:click={() => chooseFolder('backup')}>Choose</button></div>
        <label><input type="checkbox" bind:checked={approveDeletion}/> I understand every source file will be backed up, then removed after verification.</label>
      {/if}
      <div class="move-actions"><button on:click={() => coreCopyPlan = null}>Cancel</button><button class="danger" on:click={runCoreCopy}>{moveSource ? 'Confirm, backup and move' : 'Confirm and copy'}</button></div>
    </div>
  </section>
{/if}

<style>
  /* .picker-row/.picker/.jobs-panel/.settings-card/.settings-values/
     .comparison-counts live in styles.css, not here: they're rendered by
     child view components (JobsView, PlaylistsView, ProblemsView,
     SettingsView, LibraryView), and Svelte's per-component CSS scoping
     never applies a <style> block's rules to another component's markup --
     a scoped rule here silently does nothing for them. Confirmed by
     driving the app in a real browser: every one of those screens rendered
     as an inert, unstyled, non-overlaying block until this moved. */
  @font-face { font-family: 'Space Grotesk'; src: url('/assets/SpaceGrotesk-VariableFont_wght.ttf') format('truetype'); font-style: normal; font-weight: 300 700; font-display: swap; }
  :global(:root), :global(body) { font-family: 'Space Grotesk', ui-sans-serif, system-ui, sans-serif; }
  .comparison { margin-top: 20px; border: 1px solid #2c393a; border-radius: 15px; padding: 29px 31px; background: #1a2325; }
  .difference-list { padding: 0; margin: 0; border: 1px solid #344244; border-radius: 9px; overflow: auto; max-height: 380px; }
  .difference-list li { min-width: 500px; display: grid; grid-template-columns: 100px 1fr 110px; gap: 12px; align-items: center; padding: 10px 12px; border-bottom: 1px solid #2c393a; font-size: 12px; color: #c7d1d0; }
  .difference-list li:last-child { border-bottom: 0; }
  .difference-list li > span:last-child { color: #8f9e9d; text-align: right; }
  .difference { font-size: 11px; font-weight: 700; }
  .difference.only_left, .difference.only_right { color: #d9c47e; }
  .capacity-ok { color: #b9e9a5; font-size: 12px; margin: 6px 0 0; }
  .capacity-bad { color: #f29b83; font-weight: 600; font-size: 12px; margin: 6px 0 0; }
  .difference.different { color: #f29b83; }
  .difference.identical { color: #b9e9a5; }
  .comparison > .primary { margin-top: 18px; }
  .copy-plan { margin-top: 18px; padding: 18px; background: #101617; border: 1px solid #344244; border-radius: 9px; }
  .copy-plan h3 { margin: 0 0 6px; }
  .copy-plan p:not(.eyebrow) { color: #aab8b7; font-size: 13px; }
  .copy-plan .danger { padding: 11px 14px; background: #c1f0ad; color: #142015; font-weight: 700; }
  .move-review { position: fixed; inset: 0; z-index: 10; display: grid; place-items: center; padding: 24px; background: rgb(5 9 10 / 72%); }
  .move-review-card { width: min(560px, 100%); display: grid; gap: 14px; padding: 28px; border: 1px solid #455654; border-radius: 15px; background: #1a2325; box-shadow: 0 24px 80px rgb(0 0 0 / 40%); }
  .move-review-card > p:not(.eyebrow) { color: #aab8b7; margin: 0; }
  .move-review-card label { color: #dce6e4; font-size: 13px; line-height: 1.5; }
  .move-review-card input[type='text'] { width: 100%; color: #e8ecec; background: #101617; border: 1px solid #3b4a4b; border-radius: 8px; padding: 11px 12px; font: inherit; font-size: 13px; }
  .move-actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 4px; }
  .move-actions button:first-child { padding: 11px 14px; background: #314244; color: #e8ecec; }
  .move-actions .danger { padding: 11px 14px; background: #c1f0ad; color: #142015; font-weight: 700; }
  @media (max-width: 720px) { .comparison { padding: 22px 18px; } .comparison-counts { grid-template-columns: repeat(2, 1fr); } }
</style>
