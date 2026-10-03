<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import type { ConnectionKind, BackupPlan, CapacityCheck, CheckSummary, Comparison, Core, MediaScan, PackageManifest, PackagePlan, PackageReport, Plan, PlaylistPlan, Problem, RemovePlan, RemoveReport, ScreenshotEntry, Setting, TaudReport, HistoryEntry } from './lib/types';
  import { invoke } from './lib/backend';
  import { checkStorageCapacity, compareMedia, detectConnection, errorMessage, executeCoreCopy, executeCoreMove, executePackageInstall, executePlaylistImport, executePlaylistRename, executePlaylistWrite, executeRemoveCore, exportPlaylist, findProblems, getManualPlayers, getRecentCards, getReportsDir, inspectCard, inspectPackage, listMountedCards, listScreenshots, newJobId, planBackup, planCoreCopy, planPackageInstall, planPlaylistImport, planPlaylistRename, planPlaylistWrite, planRemoveCore, readCheckSummary, readCoreIcon, readImageDataUrl, readPersistedSettings, readPlatformImage, readQrReport, recordRecentCard, scanMedia, setManualPlayer, listHistory, pruneHistory } from './lib/tau-api';
  import SettingsView from './lib/SettingsView.svelte';
  import HistoryView from './lib/HistoryView.svelte';
  import PlaylistsView from './lib/PlaylistsView.svelte';
  import ProblemsView from './lib/ProblemsView.svelte';
  import BackupView from './lib/BackupView.svelte';
  import Workbench from './lib/Workbench.svelte';
  import CardIcon from './lib/CardIcon.svelte';
  import MetersView from './lib/MetersView.svelte';
  import AppearanceView from './lib/AppearanceView.svelte';
  import DiagnosticsView from './lib/DiagnosticsView.svelte';
  import LibrarySettings from './lib/LibrarySettings.svelte';
  import { modal, trap } from './lib/a11y';
  import { tick } from 'svelte';
  import PackageView from './lib/PackageView.svelte';
  let page: 'cards' | 'workbench' | 'compare' | 'history' | 'settings' | 'playlists' | 'problems' | 'backup' | 'package' | 'meters' | 'appearance' | 'diagnostics' = 'cards'; let cores: Core[] = []; let path = ''; let problemsPath = ''; let problems: Problem[] | null = null; let problemsNotice = 'Choose a media root to inspect it for problems.'; let problemsLoading = false; let playlistPath = ''; let playlistResult: MediaScan | null = null; let playlistNotice = 'Choose a media root to inspect playlists.'; let playlistOutput = ''; let selectedPlaylist = ''; let settingsPath = ''; let settings: Setting[] = []; let settingsNotice = 'Choose a persisted settings file to inspect it.';
  let backupSourcePath = ''; let backupDestPath = ''; let backupPlanResult: BackupPlan | null = null; let backupNotice = ''; let backupCapacity: CapacityCheck | null = null;
  let packageZipPath = ''; let packageCardPath = ''; let packageManifest: PackageManifest | null = null; let packageInspectNotice = ''; let packagePlan: PackagePlan | null = null; let packagePlanNotice = ''; let packageReport: PackageReport | null = null; let packageConfirmNotice = '';
  async function inspectPackageZip() { packageManifest = null; packagePlan = null; packageReport = null; packagePlanNotice = ''; packageConfirmNotice = ''; try { packageManifest = await inspectPackage(packageZipPath); packageInspectNotice = `${packageManifest.entries.length} files found. Nothing was changed.`; } catch (error) { packageInspectNotice = `Could not read this package: ${errorMessage(error)}`; } }
  async function reviewPackageInstall() { packagePlan = null; packageReport = null; packagePlanNotice = ''; packageConfirmNotice = ''; try { packagePlan = await planPackageInstall(packageZipPath, packageCardPath); packagePlanNotice = `Plan ${packagePlan.id} is ready for review. Nothing has been written.`; } catch (error) { packagePlanNotice = `Could not plan this install: ${errorMessage(error)}`; } }
  async function confirmPackageInstall() { if (!packagePlan) return; try { packageReport = await executePackageInstall(packageZipPath, packageCardPath, packagePlan.id); packageConfirmNotice = 'Installed and verified. Nothing else was changed.'; packagePlan = null; } catch (error) { packageConfirmNotice = `Nothing was reported as complete: ${errorMessage(error)}`; } }
  let removeCores: Core[] = []; let removeCoresNotice = ''; let removeCoreId = ''; let removePlan: RemovePlan | null = null; let removePlanNotice = ''; let removeReport: RemoveReport | null = null; let removeConfirmNotice = '';
  async function loadCoresToRemove() { removeCores = []; removeCoreId = ''; removePlan = null; removeReport = null; removePlanNotice = ''; removeConfirmNotice = ''; try { removeCores = await inspectCard(packageCardPath); removeCoresNotice = `${removeCores.length} core${removeCores.length === 1 ? '' : 's'} found. This is a read-only inspection.`; } catch (error) { removeCoresNotice = `Could not inspect this card: ${errorMessage(error)}`; } }
  async function reviewRemoveCore() { removePlan = null; removeReport = null; removeConfirmNotice = ''; try { removePlan = await planRemoveCore(packageCardPath, removeCoreId); removePlanNotice = `Plan ${removePlan.id} is ready for review. Nothing has been deleted.`; } catch (error) { removePlanNotice = `Could not plan this removal: ${errorMessage(error)}`; } }
  async function confirmRemoveCore() { if (!removePlan) return; try { removeReport = await executeRemoveCore(packageCardPath, removeCoreId, removePlan.id); removeConfirmNotice = 'Removed and verified.'; removePlan = null; await loadCoresToRemove(); } catch (error) { removeConfirmNotice = `Nothing was reported as complete: ${errorMessage(error)}`; } }
  // Where journals are written (the user's folder, or the app's default). Sync history reads from the same place.
  let reportsDir = ''; let historyEntries: HistoryEntry[] = []; let historyLoading = false; let historyNotice = ''; let historySelect = '';
  onMount(async () => {
    try { reportsDir = (await getReportsDir()) ?? ''; } catch { /* Compare cores then asks for a report file */ }
    pruneHistory().catch(() => { /* retention is best-effort */ });
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
    window.addEventListener('tau-pending', () => { pendingTick += 1; });
    window.addEventListener('tau-ejected', (event) => { ejectedSafely = (event as CustomEvent<string>).detail; cardMounted = false; });
    window.addEventListener('tau-reports-dir', async () => { try { reportsDir = (await getReportsDir()) ?? ''; } catch { /* keeps the previous folder */ } });
    // Auto-detect a card or Pocket being plugged in or removed: poll the cheap
    // mounted-volume list and re-read the open card when its state changes.
    detectTimer = setInterval(detectChange, 3000);
  });
  // Rarely-used tools and settings live in a collapsible sidebar section; it
  // stays open while one of its pages is showing and remembers the user's choice.
  const moreItems = ['compare', 'backup', 'package', 'problems', 'diagnostics', 'history', 'settings'];
  let moreOpen = false;
  try { moreOpen = localStorage.getItem('tau.nav.more') === '1'; } catch { /* per-viewer convenience */ }
  function toggleMore() { moreOpen = !moreOpen; try { localStorage.setItem('tau.nav.more', moreOpen ? '1' : '0'); } catch { /* ignore */ } }
  // The selected core's media root: what the Library screen lists and writes.
  // What each opened card is plugged in as (the Pocket itself or a reader), for the sidebar icon.
  let connectionKinds: Record<string, ConnectionKind> = {};
  function ensureConnectionKind(card: string) {
    if (!card || connectionKinds[card]) return;
    detectConnection(card).then((c) => (connectionKinds = { ...connectionKinds, [card]: c.kind })).catch(() => {});
  }
  $: ensureConnectionKind(path);
  $: mediaRoot = path && activeCore?.platform ? `${path.replace(/\/+$/, '')}/Assets/${activeCore.platform}/common` : '';
  // Below 720 px the sidebar is a menu opened with the button at the top left (it used to disappear
  // completely, which at 200% zoom removed all navigation).
  let navOpen = false; let navToggleEl: HTMLButtonElement;
  $: { page; navOpen = false; }
  $: if (navOpen) tick().then(() => document.querySelector<HTMLElement>('#sidebar nav button')?.focus());
  function onWindowKey(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    if (showHelp) { showHelp = false; return; }
    if (detailCore) { closeCoreDetail(); return; }
    if (navOpen) { navOpen = false; navToggleEl?.focus(); }
  }
  let cardRevision = 0;
  // "N pending" badge: the Library screen keeps each card+core's unsent changes in localStorage.
  let pendingTick = 0;
  function pendingFor(cardPath: string, _tick: number): number {
    try {
      const prefix = `tau.wb.pending.${cardPath.replace(/\/+$/, '')}/`;
      let total = 0;
      for (let i = 0; i < localStorage.length; i++) {
        const key = localStorage.key(i);
        if (key && key.startsWith(prefix)) total += (JSON.parse(localStorage.getItem(key) ?? '[]') as unknown[]).length;
      }
      return total;
    } catch { return 0; }
  }
  let detectTimer: ReturnType<typeof setInterval>;
  onDestroy(() => clearInterval(detectTimer));
  let refreshedAt = 'just now';
  async function detectChange() {
    let now: string[];
    try { now = await listMountedCards(); } catch { return; }
    const before = mountedCards.join('\n');
    if (now.join('\n') === before) return;
    const wasMounted = mountedCards.includes(path);
    mountedCards = now;
    if (path && now.includes(path) && !wasMounted) await forceRefresh();
    else if (path && wasMounted && !now.includes(path)) cardMounted = false; // it was in the mounted list a moment ago, so this is an eject on any OS
    else if (!path && now[0]) { path = now[0]; await openFolder(); refreshedAt = new Date().toLocaleTimeString(); cardRevision += 1; }
  }
  async function forceRefresh() {
    await refreshKnownCards();
    if (path.trim()) await openFolder();
    refreshedAt = new Date().toLocaleTimeString();
    cardRevision += 1;
  }
  async function loadHistory() {
    historyLoading = true;
    try { historyEntries = await listHistory(); historyNotice = historyEntries.length ? `${historyEntries.length} sync${historyEntries.length === 1 ? '' : 's'} stored.` : ''; }
    catch (error) { historyEntries = []; historyNotice = `Could not read the sync history: ${errorMessage(error)}`; }
    finally { historyLoading = false; }
  }
  /** Opens the sync history, optionally on one entry (a journal path, or 'latest'). */
  function openHistory(id = '') { historySelect = id; moreOpen = true; page = 'history'; loadHistory(); }
  const autoJournalPath = (kind: string) => `${reportsDir}/${Date.now()}-${kind}.json`;
  let notice = 'Open a card folder to inspect it. Tau Omega will not write anything at this stage.';
  let leftCore = ''; let rightCore = ''; let comparison: Comparison | null = null; let compareNotice = ''; let copyReportPath = ''; let coreCopyPlan: Plan | null = null; let moveSource = false; let moveBackupPath = ''; let approveDeletion = false;
  async function chooseFolder(target: 'card' | 'source' | 'destination' | 'left' | 'right' | 'manifest' | 'copyReport' | 'backup' | 'settings' | 'playlists' | 'problems' | 'library' | 'importSource' | 'backupSource' | 'backupDestination' | 'packageZip' | 'packageCard' | 'qrScreenshot' | 'screenshotCard') {
    const selected = await open({ directory: !['manifest', 'copyReport', 'settings', 'importSource', 'packageZip', 'qrScreenshot'].includes(target), multiple: target === 'source' });
    if (!selected) return;
    const value = Array.isArray(selected) ? selected.join('\n') : selected;
    if (target === 'card') { path = value; await openFolder(); } 
    if (target === 'left') leftCore = value; if (target === 'right') rightCore = value; if (target === 'copyReport') copyReportPath = value; if (target === 'backup') moveBackupPath = value; if (target === 'settings') settingsPath = value; if (target === 'playlists') playlistPath = value; if (target === 'problems') problemsPath = value; if (target === 'importSource') importSource = value; if (target === 'backupSource') backupSourcePath = value; if (target === 'backupDestination') backupDestPath = value; if (target === 'packageZip') packageZipPath = value; if (target === 'packageCard') packageCardPath = value; if (target === 'qrScreenshot') qrPath = value; if (target === 'screenshotCard') screenshotCardPath = value;
  }
  async function choosePlaylistOutput() {
    const chosen = await save({ defaultPath: playlistOutput || 'playlist.m3u', filters: [{ name: 'Playlist', extensions: ['m3u'] }] });
    if (chosen) playlistOutput = chosen;
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
  const settingLabel = (id: number) => ({ 10: 'Volume', 11: 'Colour index', 12: 'Repeat', 13: 'Shuffle', 15: 'Meter', 16: 'EQ', 18: 'Saved position', 19: 'Resume on', 24: 'Library history', 25: 'Index build ID', 26: 'Shuffle All seed', 27: 'Theme', 28: 'Theme mode' } as Record<number, string>)[id] ?? `Word ${id}`;
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
  // Set when Eject safely succeeded for this card, so the banner says "safe to unplug" instead of the alarming "no longer connected".
  let ejectedSafely = '';
  $: if (cardMounted) ejectedSafely = '';
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
    page = 'workbench';
  }

  let showHelp = false;
  async function compareCores() { comparison = null; compareNotice = ''; try { comparison = await compareMedia(leftCore, rightCore); compareNotice = `${comparison.differences.length} supported files compared. Nothing was changed.`; } catch (error) { compareNotice = `Could not compare these media roots: ${errorMessage(error)}`; } }
  let coreCopyCapacity: CapacityCheck | null = null;
  async function reviewCoreCopy() { coreCopyPlan = null; coreCopyCapacity = null; moveSource = false; approveDeletion = false; try { coreCopyPlan = await planCoreCopy(leftCore, rightCore); compareNotice = `Copy plan ${coreCopyPlan.id} is ready for review. The first core remains untouched.`; try { coreCopyCapacity = await checkStorageCapacity(rightCore, coreCopyPlan.bytes_to_write); } catch { /* capacity check is informational; a plan still reviews without it */ } } catch (error) { compareNotice = `Could not make a copy plan: ${errorMessage(error)}`; } }
  async function runCoreCopy() { if (!coreCopyPlan) return; if (moveSource && !approveDeletion) { compareNotice = 'Confirm that the source will be backed up and deleted before moving.'; return; } if (moveSource && !moveBackupPath.trim()) { compareNotice = 'Choose a visible external backup folder before moving.'; return; } const jobId = newJobId(); const manifest = reportsDir ? autoJournalPath(moveSource ? 'core_move' : 'core_copy') : copyReportPath; try { const result = moveSource ? await executeCoreMove(leftCore, rightCore, coreCopyPlan.id, moveBackupPath, manifest, jobId) : await executeCoreCopy(leftCore, rightCore, coreCopyPlan.id, manifest, jobId); compareNotice = `Verified ${moveSource ? 'move' : 'copy'}: ${result.copied} copied, ${result.unchanged} unchanged. Index: ${result.index_path}`; coreCopyPlan = null; if (reportsDir) await loadHistory(); } catch (error) { compareNotice = `Nothing was reported as complete: ${errorMessage(error)}`; if (reportsDir) await loadHistory(); } }
  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  async function reviewBackup() { backupPlanResult = null; backupCapacity = null; backupNotice = ''; try { backupPlanResult = await planBackup(backupSourcePath, backupDestPath); backupNotice = `${backupPlanResult.items.length} files compared. Nothing was changed. This is a preview only -- copying isn't enabled yet.`; try { backupCapacity = await checkStorageCapacity(backupDestPath, backupPlanResult.bytes_to_write); } catch { /* capacity check is informational; a plan still reviews without it */ } } catch (error) { backupNotice = `Could not plan this backup: ${errorMessage(error)}`; } }
</script>

<svelte:window on:keydown={onWindowKey} />
<main>
  <button class="nav-toggle" aria-label="Menu" aria-expanded={navOpen} aria-controls="sidebar" bind:this={navToggleEl} on:click={() => (navOpen = !navOpen)}><svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h16"/></svg></button>
  {#if navOpen}<div class="nav-backdrop" role="presentation" on:click={() => (navOpen = false)}></div>{/if}
  <aside id="sidebar" class:open={navOpen} aria-label="Primary navigation" use:trap={navOpen}><div class="brand"><span class="mark">τ</span><span>Tau Omega<small>Library companion</small></span></div>
  <div class="active-context">
    <button class="active-context-btn" on:click={() => showSwitcher = !showSwitcher} aria-expanded={showSwitcher} aria-label="Switch card or core">
      <span class="active-context-icon" aria-hidden="true"><CardIcon kind={connectionKinds[path]} /></span>
      <span class="active-context-text">
        <strong>{path ? cardName(path) : 'No card open'}</strong>
        <small>{activeCore ? (activeCore.shortname || activeCore.id) : (cores.length ? 'Choose a core' : 'Open a card to begin')}</small>
        {#if path && page !== 'workbench' && pendingFor(path, pendingTick)}<small class="ctx-pending">{pendingFor(path, pendingTick)} pending</small>{/if}
      </span>
      <svg class="active-context-chevron" class:open={showSwitcher} viewBox="0 0 24 24" fill="none" width="14" height="14" aria-hidden="true"><path d="m6 9 6 6 6-6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>
    </button>
    {#if showSwitcher}
      <div class="switcher-backdrop" role="presentation" on:click={() => showSwitcher = false}></div>
      <div class="switcher-panel">
        {#if knownCards.length}
          <p class="switcher-label">Switch card</p>
          {#each knownCards as card}<button class="switcher-row" class:active={path === card} on:click={() => switchToKnownCard(card)}>{cardName(card)}{#if pendingFor(card, pendingTick)}<span class="switcher-pending">{pendingFor(card, pendingTick)} pending</span>{/if}{#if mountedCards.includes(card)}<span class="switcher-dot" aria-hidden="true"></span>{/if}</button>{/each}
        {/if}
        {#if playerCores.length}
          <p class="switcher-label">Switch core</p>
          {#each playerCores as core}<button class="switcher-row" class:active={activeCore?.id === core.id} on:click={() => selectActiveCore(core)}>{core.shortname || core.id}</button>{/each}
        {/if}
        <button class="switcher-manage" on:click={() => { page = 'cards'; showSwitcher = false; }}>Manage cards &amp; cores →</button>
      </div>
    {/if}
  </div>
  <nav aria-label="Main">
    <button class:active={page === 'cards'} on:click={() => page = 'cards'}>Cards</button>
    <button class:active={page === 'workbench'} on:click={() => page = 'workbench'}>Library</button>
    <button class:active={page === 'playlists'} on:click={() => page = 'playlists'}>Playlists</button>
    <button class:active={page === 'meters'} on:click={() => page = 'meters'}>Meter Lab</button>
    <button class:active={page === 'appearance'} on:click={() => page = 'appearance'}>Appearance</button>
    <button class="nav-group" class:has-active={moreItems.includes(page)} aria-expanded={moreOpen} aria-controls="more-tools" on:click={toggleMore}><span>Tools &amp; settings</span><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class:open={moreOpen}><path d="m6 9 6 6 6-6"/></svg></button>
    {#if moreOpen || moreItems.includes(page)}
      <div class="nav-sub" id="more-tools">
        <button class:active={page === 'compare'} on:click={() => page = 'compare'}>Compare cores</button>
        <button class:active={page === 'backup'} on:click={() => page = 'backup'}>Backup</button>
        <button class:active={page === 'package'} on:click={() => page = 'package'}>Packages</button>
        <button class:active={page === 'problems'} on:click={() => page = 'problems'}>Problems</button>
        <button class:active={page === 'diagnostics'} on:click={() => page = 'diagnostics'}>Send diagnostics</button>
        <button class:active={page === 'history'} on:click={() => openHistory()}>Sync history</button>
        <button class:active={page === 'settings'} on:click={() => page = 'settings'}>Settings</button>
      </div>
    {/if}
  </nav><p class="offline">Local only<br/><span>No card writes without a reviewed plan.</span></p></aside>
  {#if page === 'cards'}<section class="page" id="cards"><header><div><p class="eyebrow">CARD LIBRARY</p><h1>Start with a card</h1><p class="lede">Inspect a Pocket card or a staging folder. Your music stays untouched.</p></div><button class="primary" on:click={() => chooseFolder('card')}>Open folder</button></header>
    {#if cardMounted === false}<div class="ejected-banner" role="status"><span>{ejectedSafely && ejectedSafely === path ? 'Safely ejected. You can unplug it now.' : 'This card is no longer connected.'}</span><button class="quiet" on:click={() => openKnownCard(path)}>Reconnect</button></div>{/if}
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
  {:else if page === 'workbench'}
    <Workbench cores={cores.map((c) => ({ id: c.id, shortname: c.shortname, platform: c.platform, library_capable: c.library_capable }))} cardPath={path} cardLabel={path ? cardName(path) : 'No card open'} coreLabel={activeCore ? (activeCore.shortname || activeCore.id) : ''} mediaRoot={mediaRoot} revision={cardRevision} refreshedAt={refreshedAt} refresh={forceRefresh} openSettings={() => { moreOpen = true; page = 'settings'; }} connected={cardMounted} ejectedSafely={!!ejectedSafely && ejectedSafely === path} openHistory={openHistory} />
  {:else if page === 'compare'}<section class="page compare-page"><header><div><p class="eyebrow">CORE TRANSFER</p><h1>Compare, then copy safely</h1><p class="lede">Compare two Tau media roots before copying the complete library to the second core.</p></div></header><section class="wizard" aria-labelledby="compare-title"><h2 id="compare-title">Compare two cores</h2><label for="left-core">Source media root</label><div class="picker-row"><input id="left-core" bind:value={leftCore} placeholder="/Volumes/Pocket/Assets/tau/common"/><button class="picker" on:click={() => chooseFolder('left')}>Choose</button></div><label for="right-core">Destination media root</label><div class="picker-row"><input id="right-core" bind:value={rightCore} placeholder="/Volumes/Pocket/Assets/tau-test/common"/><button class="picker" on:click={() => chooseFolder('right')}>Choose</button></div><label for="copy-report">Copy report location on this computer</label><div class="picker-row"><input id="copy-report" bind:value={copyReportPath} placeholder="/Users/me/Documents/tau-core-copy.json"/><button class="picker" on:click={() => chooseFolder('copyReport')}>Choose</button></div><button class="primary" on:click={compareCores}>Compare safely</button></section><p class="notice" role="status">{compareNotice}</p>{#if comparison}<section class="comparison" aria-labelledby="comparison-title"><div class="section-title"><div><p class="eyebrow">RESULT</p><h2 id="comparison-title">{comparison.differences.length} files compared</h2></div><span>Read-only</span></div><div class="comparison-counts"><span><b>{comparison.only_left}</b> only in source</span><span><b>{comparison.only_right}</b> only in destination</span><span><b>{comparison.different}</b> different</span><span><b>{comparison.identical}</b> matching</span></div><ul class="difference-list">{#each comparison.differences as item}<li><span class={`difference ${item.state}`}>{item.state === 'only_left' ? 'Source only' : item.state === 'only_right' ? 'Destination only' : item.state === 'different' ? 'Different' : 'Matching'}</span><span>{item.relative}</span><span>{item.left_bytes === null ? '—' : size(item.left_bytes)} / {item.right_bytes === null ? '—' : size(item.right_bytes)}</span></li>{/each}</ul><button class="primary" on:click={reviewCoreCopy}>Review full-library copy</button>{#if coreCopyPlan}<div class="copy-plan"><p class="eyebrow">READY FOR CONFIRMATION</p><h3>{coreCopyPlan.id}</h3><p>{coreCopyPlan.new_files} new · {coreCopyPlan.updates} updated · {coreCopyPlan.unchanged} unchanged · {size(coreCopyPlan.bytes_to_write)} to write</p>{#if coreCopyCapacity}<p class:capacity-ok={coreCopyCapacity.fits} class:capacity-bad={!coreCopyCapacity.fits}>{coreCopyCapacity.fits ? 'Fits' : 'Does not fit'} on the destination volume — {size(coreCopyCapacity.space.available_bytes)} free.</p>{/if}<button class="danger" on:click={runCoreCopy}>Confirm and copy</button></div>{/if}<p class="safety">The source is never changed. The destination files are verified and its index is rebuilt last. Move remains unavailable until its separate backup and deletion review is ready.</p></section>{/if}</section>
  {:else if page === 'history'}
    <HistoryView entries={historyEntries} loading={historyLoading} notice={historyNotice} select={historySelect} refresh={loadHistory} openSettings={() => { moreOpen = true; page = 'settings'; }} startSync={() => page = 'workbench'} />
  {:else if page === 'playlists'}
    <PlaylistsView bind:path={playlistPath} result={playlistResult} notice={playlistNotice} bind:output={playlistOutput} chooseOutput={choosePlaylistOutput} bind:selected={selectedPlaylist} choose={() => chooseFolder('playlists')} scan={scanPlaylists} exportList={exportSelectedPlaylist}
      {selectedPlaylistDetail} {reorderTracks} {moveTrack} {reorderPlan} {reorderNotice} {reviewReorder} {confirmReorder}
      bind:renameNewFile {renamePlan} {renameNotice} {reviewRename} {confirmRename}
      bind:createFile bind:createTracksText {createPlan} {createNotice} {reviewCreate} {confirmCreate}
      bind:importSource bind:importDestFile {importPlan} {importNotice} chooseImportSource={() => chooseFolder('importSource')} {reviewImport} {confirmImport} />
  {:else if page === 'problems'}
    <ProblemsView bind:path={problemsPath} {problems} notice={problemsNotice} loading={problemsLoading} choose={() => chooseFolder('problems')} scan={scanProblems} />
  {:else if page === 'backup'}
    <BackupView bind:sourcePath={backupSourcePath} bind:destinationPath={backupDestPath} chooseSource={() => chooseFolder('backupSource')} chooseDestination={() => chooseFolder('backupDestination')} review={reviewBackup} backupPlan={backupPlanResult} {backupNotice} capacity={backupCapacity} />
  {:else if page === 'package'}
    <PackageView bind:zipPath={packageZipPath} bind:cardPath={packageCardPath} chooseZip={() => chooseFolder('packageZip')} chooseCard={() => chooseFolder('packageCard')} inspect={inspectPackageZip} manifest={packageManifest} inspectNotice={packageInspectNotice} review={reviewPackageInstall} plan={packagePlan} planNotice={packagePlanNotice} confirm={confirmPackageInstall} report={packageReport} confirmNotice={packageConfirmNotice} loadCoresToRemove={loadCoresToRemove} removeCores={removeCores} removeCoresNotice={removeCoresNotice} bind:removeCoreId={removeCoreId} reviewRemove={reviewRemoveCore} removePlan={removePlan} removePlanNotice={removePlanNotice} confirmRemove={confirmRemoveCore} removeReport={removeReport} removeConfirmNotice={removeConfirmNotice} />
  {:else if page === 'meters'}
    <MetersView />
  {:else if page === 'diagnostics'}
    <DiagnosticsView cardPath={path} cardLabel={path ? cardName(path) : ''} />
  {:else if page === 'appearance'}
    <AppearanceView {mediaRoot} cardLabel={path ? cardName(path) : ''} />
  {:else if page === 'settings'}
    <SettingsView bind:settingsPath {settings} {checkSummary} notice={settingsNotice} choose={() => chooseFolder('settings')} read={loadSettings} done={() => page = 'cards'} label={settingLabel} value={settingValue} bind:qrPath chooseQr={() => chooseFolder('qrScreenshot')} decodeQr={decodeQrScreenshot} {qrReport} {qrNotFound} {qrNotice} bind:screenshotCardPath chooseScreenshotCard={() => chooseFolder('screenshotCard')} {browseScreenshots} {screenshots} {screenshotsNotice} {selectScreenshot} {selectedScreenshotPath} {screenshotPreviewUrl} {screenshotPreviewLoading} ><LibrarySettings openHistory={() => openHistory()} /></SettingsView>
  {/if}
</main>

<button class="help-fab" aria-label="Help" title="Help" on:click={() => showHelp = !showHelp}>?</button>
{#if showHelp}
  <div class="help-backdrop" role="presentation" on:click={() => showHelp = false}></div>
  <div class="help-panel" role="dialog" aria-modal="true" aria-labelledby="help-title" use:modal>
    <div class="help-panel-header"><h2 id="help-title">Using Tau Omega</h2><button class="quiet" aria-label="Close help" on:click={() => showHelp = false}>✕</button></div>
    <div class="help-panel-body">
      <p><strong>Open folder</strong> browses to a Pocket card or a plain staging folder (anything with <code>Cores</code> and <code>Assets</code>). A mounted Pocket and any card you've opened before show up under <strong>Known cards</strong> for one-click reopening.</p>
      <p>Cores that look like media players (Tau, or anything whose platform declares itself a "Media Players" core) get a large card here. Anything else can be shown with "Show other cores" and promoted with <strong>Set as player</strong>.</p>
      <p><strong>Library</strong> shows the selected card's music on the right and a folder on this computer on the left. Tick albums (or drag them across) to add them, mark albums on the Pocket to remove, rename or change their cover, then press <strong>Start sync</strong>. Everything you stage sits in <strong>Pending changes</strong> and nothing is written until you confirm. The card is detected automatically; the refresh button at the top right re-reads it. Rarely used tools and settings are under <strong>Tools &amp; settings</strong> in the sidebar.</p>
      <p>Every write anywhere in the app goes through <strong>plan → review → confirm</strong> — nothing is copied, moved, or deleted until you've seen exactly what will change and confirmed it.</p>
      <p>This is all read-only until you explicitly confirm a plan on Library, Compare cores, Backup, or Packages.</p>
    </div>
  </div>
{/if}

{#if detailCore}
  <div class="detail-backdrop" role="presentation" on:click={closeCoreDetail}></div>
  <div class="detail-panel" role="dialog" aria-modal="true" aria-labelledby="detail-title" use:modal>
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
  </div>
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
