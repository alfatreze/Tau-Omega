<script lang="ts">
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import type { BackupPlan, CapacityCheck, Comparison, Core, Difference, Job, JournalSummary, LibraryScan, MediaScan, Plan, PlaylistPlan, Problem, Setting } from './lib/types';
  import { invoke } from './lib/backend';
  import { cancelJob, checkStorageCapacity, compareMedia, errorMessage, executeCoreCopy, executeCoreMove, executePlaylistImport, executePlaylistRename, executePlaylistWrite, executeSync, exportPlaylist, findProblems, getReportsDir, inspectCard, listJournals, newJobId, onProgress, planBackup, planCoreCopy, planPlaylistImport, planPlaylistRename, planPlaylistWrite, planSync, readJournal, readPersistedSettings, scanLibrary, scanMedia, setReportsDir } from './lib/tau-api';
  import SettingsView from './lib/SettingsView.svelte';
  import JobsView from './lib/JobsView.svelte';
  import PlaylistsView from './lib/PlaylistsView.svelte';
  import ProblemsView from './lib/ProblemsView.svelte';
  import LibraryView from './lib/LibraryView.svelte';
  import BackupView from './lib/BackupView.svelte';
  let page: 'cards' | 'compare' | 'sync' | 'jobs' | 'settings' | 'playlists' | 'problems' | 'library' | 'backup' = 'cards'; let cores: Core[] = []; let path = ''; let jobs: Job[] = []; let journalPath = ''; let journalNotice = 'Choose a host-side Tau Omega journal to load it.'; let problemsPath = ''; let problems: Problem[] | null = null; let problemsNotice = 'Choose a media root to inspect it for problems.'; let problemsLoading = false; let playlistPath = ''; let playlistResult: MediaScan | null = null; let playlistNotice = 'Choose a media root to inspect playlists.'; let playlistOutput = ''; let selectedPlaylist = ''; let settingsPath = ''; let settings: Setting[] = []; let settingsNotice = 'Choose a persisted settings file to inspect it.';
  let backupSourcePath = ''; let backupDestPath = ''; let backupPlanResult: BackupPlan | null = null; let backupNotice = ''; let backupCapacity: CapacityCheck | null = null;
  let libraryPath = ''; let libraryScan: LibraryScan | null = null; let libraryNotice = 'Choose a media root to inspect it.'; let libraryLoading = false; let libraryJobId = ''; let libraryProgress = ''; let librarySearch = ''; let libraryFormat: 'all' | 'mp3' | 'flac' = 'all';
  let syncJobId = ''; let syncProgress = '';
  let reportsDir = ''; let historyNotice = 'Choose a folder to keep durable job history across restarts.'; let history: JournalSummary[] = []; let journalDetail: unknown = null; let journalDetailNotice = '';
  onMount(async () => {
    try {
      const dir = await getReportsDir();
      if (dir) { reportsDir = dir; await refreshHistory(); }
    } catch (error) { historyNotice = `Could not read the saved reports directory: ${errorMessage(error)}`; }
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
  let sources = ''; let destination = ''; let manifestPath = ''; let embedCovers = false; let plan: Plan | null = null; let syncNotice = '';
  let leftCore = ''; let rightCore = ''; let comparison: Comparison | null = null; let compareNotice = ''; let copyReportPath = ''; let coreCopyPlan: Plan | null = null; let moveSource = false; let moveBackupPath = ''; let approveDeletion = false;
  async function chooseFolder(target: 'card' | 'source' | 'destination' | 'left' | 'right' | 'manifest' | 'copyReport' | 'backup' | 'settings' | 'playlists' | 'problems' | 'journal' | 'library' | 'importSource' | 'backupSource' | 'backupDestination') {
    const selected = await open({ directory: !['manifest', 'copyReport', 'settings', 'journal', 'importSource'].includes(target), multiple: target === 'source' });
    if (!selected) return;
    const value = Array.isArray(selected) ? selected.join('\n') : selected;
    if (target === 'card') path = value; if (target === 'source') sources = value; if (target === 'destination') destination = value;
    if (target === 'left') leftCore = value; if (target === 'right') rightCore = value; if (target === 'manifest') manifestPath = value; if (target === 'copyReport') copyReportPath = value; if (target === 'backup') moveBackupPath = value; if (target === 'settings') settingsPath = value; if (target === 'playlists') playlistPath = value; if (target === 'problems') problemsPath = value; if (target === 'journal') journalPath = value; if (target === 'library') libraryPath = value; if (target === 'importSource') importSource = value; if (target === 'backupSource') backupSourcePath = value; if (target === 'backupDestination') backupDestPath = value;
  }
  async function loadSettings() { try { settings = await readPersistedSettings(settingsPath); settingsNotice = `${settings.length} persisted values loaded. Nothing was changed.`; } catch (error) { settings = []; settingsNotice = `Could not read settings: ${errorMessage(error)}`; } }
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
  async function openFolder() { if (!path.trim()) { notice = 'Enter a staging-card folder path first.'; return; } try { cores = await inspectCard(path); notice = `${cores.length} core${cores.length === 1 ? '' : 's'} found. This is a read-only inspection.`; } catch (error) { notice = `Could not inspect this folder: ${errorMessage(error)}`; cores = []; } }
  let syncCapacity: CapacityCheck | null = null;
  async function makePlan() { plan = null; syncCapacity = null; syncNotice = ''; try { plan = await planSync(sources.split('\n'), destination, embedCovers); syncNotice = `Plan ${plan.id} is ready for review. Nothing has been written.`; try { syncCapacity = await checkStorageCapacity(destination, plan.bytes_to_write); } catch { /* capacity check is informational; a plan still reviews without it */ } } catch (error) { syncNotice = `Could not make a plan: ${errorMessage(error)}`; } }
  async function runSync() { if (!plan) return; syncJobId = newJobId(); syncProgress = 'starting…'; const manifest = reportsDir ? autoJournalPath('sync') : manifestPath; try { const result = await executeSync(sources.split('\n'), destination, plan.id, manifest, embedCovers, syncJobId); syncNotice = `Verified: ${result.copied} copied, ${result.unchanged} unchanged. Index: ${result.index_path}`; jobs = [{ kind: 'Library sync', status: 'Completed', detail: `${result.copied} copied · ${result.unchanged} unchanged` }, ...jobs]; plan = null; if (reportsDir) await refreshHistory(); } catch (error) { syncNotice = `Nothing was reported as complete: ${errorMessage(error)}`; jobs = [{ kind: 'Library sync', status: 'Failed', detail: errorMessage(error) }, ...jobs]; if (reportsDir) await refreshHistory(); } finally { syncProgress = ''; } }
  async function cancelSync() { if (syncJobId) await cancelJob(syncJobId); }
  async function compareCores() { comparison = null; compareNotice = ''; try { comparison = await compareMedia(leftCore, rightCore); compareNotice = `${comparison.differences.length} supported files compared. Nothing was changed.`; } catch (error) { compareNotice = `Could not compare these media roots: ${errorMessage(error)}`; } }
  let coreCopyCapacity: CapacityCheck | null = null;
  async function reviewCoreCopy() { coreCopyPlan = null; coreCopyCapacity = null; moveSource = false; approveDeletion = false; try { coreCopyPlan = await planCoreCopy(leftCore, rightCore); compareNotice = `Copy plan ${coreCopyPlan.id} is ready for review. The first core remains untouched.`; try { coreCopyCapacity = await checkStorageCapacity(rightCore, coreCopyPlan.bytes_to_write); } catch { /* capacity check is informational; a plan still reviews without it */ } } catch (error) { compareNotice = `Could not make a copy plan: ${errorMessage(error)}`; } }
  async function runCoreCopy() { if (!coreCopyPlan) return; if (moveSource && !approveDeletion) { compareNotice = 'Confirm that the source will be backed up and deleted before moving.'; return; } if (moveSource && !moveBackupPath.trim()) { compareNotice = 'Choose a visible external backup folder before moving.'; return; } const jobId = newJobId(); const manifest = reportsDir ? autoJournalPath(moveSource ? 'core_move' : 'core_copy') : copyReportPath; try { const result = moveSource ? await executeCoreMove(leftCore, rightCore, coreCopyPlan.id, moveBackupPath, manifest, jobId) : await executeCoreCopy(leftCore, rightCore, coreCopyPlan.id, manifest, jobId); compareNotice = `Verified ${moveSource ? 'move' : 'copy'}: ${result.copied} copied, ${result.unchanged} unchanged. Index: ${result.index_path}`; coreCopyPlan = null; if (reportsDir) await refreshHistory(); } catch (error) { compareNotice = `Nothing was reported as complete: ${errorMessage(error)}`; if (reportsDir) await refreshHistory(); } }
  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  async function reviewBackup() { backupPlanResult = null; backupCapacity = null; backupNotice = ''; try { backupPlanResult = await planBackup(backupSourcePath, backupDestPath); backupNotice = `${backupPlanResult.items.length} files compared. Nothing was changed. This is a preview only -- copying isn't enabled yet.`; try { backupCapacity = await checkStorageCapacity(backupDestPath, backupPlanResult.bytes_to_write); } catch { /* capacity check is informational; a plan still reviews without it */ } } catch (error) { backupNotice = `Could not plan this backup: ${errorMessage(error)}`; } }
</script>

<main><aside aria-label="Primary navigation"><div class="brand"><span class="mark">τ</span><span>Tau Omega<small>Library companion</small></span></div><nav><button class:active={page === 'cards'} on:click={() => page = 'cards'}>Cards</button><button class:active={page === 'compare'} on:click={() => page = 'compare'}>Compare cores</button><button class:active={page === 'sync'} on:click={() => page = 'sync'}>Sync library</button><button class:active={page === 'playlists'} on:click={() => page = 'playlists'}>Playlists</button><button class:active={page === 'backup'} on:click={() => page = 'backup'}>Backup</button><button class:active={page === 'problems'} on:click={() => page = 'problems'}>Problems</button><button class:active={page === 'library'} on:click={() => page = 'library'}>Library</button><button class:active={page === 'jobs'} on:click={() => page = 'jobs'}>Recent jobs</button><button class:active={page === 'settings'} on:click={() => page = 'settings'}>Settings</button></nav><p class="offline">Local only<br/><span>No card writes without a reviewed plan.</span></p></aside>
  {#if page === 'cards'}<section class="page" id="cards"><header><div><p class="eyebrow">CARD LIBRARY</p><h1>Start with a card</h1><p class="lede">Inspect a Pocket card or a staging folder. Your music stays untouched.</p></div><button class="primary" on:click={openFolder}>Open folder</button></header>
    <section class="open-card" aria-labelledby="open-card-title"><div><p class="eyebrow">READ-ONLY</p><h2 id="open-card-title">Open a staging card</h2><p>Choose a folder containing <code>Cores</code> and <code>Assets</code>. You can review its cores before planning any changes.</p></div><div class="folder"><label for="card-path">Card or staging-folder path</label><div><input id="card-path" bind:value={path} placeholder="/Volumes/My Pocket"/><button on:click={() => chooseFolder('card')}>Choose</button><button on:click={openFolder}>Inspect</button></div></div></section><p class="notice" role="status">{notice}</p>
    {#if cores.length}<section aria-labelledby="core-title"><div class="section-title"><div><p class="eyebrow">DETECTED</p><h2 id="core-title">Cores on this card</h2></div><span>{cores.length} found</span></div><div class="core-list">{#each cores as core}<article><div class="core-icon">{core.id.split('.').at(-1)?.[0] ?? 'τ'}</div><div><h3>{core.id}</h3><p>{core.author || 'Unknown author'} · {core.version || 'Version unknown'} · {core.platform || 'No platform declared'} · {core.index_status}{core.tracks === null ? '' : ` · ${core.tracks} tracks`}</p></div><span class:capable={core.library_capable} class="chip">{core.library_capable ? 'Library ready' : 'Legacy core'}</span><button class="quiet" aria-label={`Open ${core.id}`}>View</button></article>{/each}</div></section>{:else}<section class="empty"><div class="empty-art">◒</div><h2>No card selected</h2><p>Open a card folder to see its cores and library health.</p></section>{/if}</section>
  {:else if page === 'sync'}<section class="page sync-page"><header><div><p class="eyebrow">SAFE SYNC</p><h1>Review before copying</h1><p class="lede">Sources are read-only. Every file is verified, then the index is published last.</p></div></header><section class="wizard" aria-labelledby="sync-title"><div class="steps" aria-label="Sync steps"><span class="current">1 Sources</span><span class:current={plan}>2 Plan</span><span>3 Confirm</span></div><h2 id="sync-title">Build a sync plan</h2><label for="sources">Source folders or files — one per line</label><div class="picker-row"><textarea id="sources" bind:value={sources} placeholder="/Users/me/Music/Album"></textarea><button class="picker" on:click={() => chooseFolder('source')}>Choose</button></div><label for="destination">Destination media root</label><div class="picker-row"><input id="destination" bind:value={destination} placeholder="/Volumes/Pocket/Assets/tau/common"/><button class="picker" on:click={() => chooseFolder('destination')}>Choose</button></div><label for="manifest">Report location on this computer</label><div class="picker-row"><input id="manifest" bind:value={manifestPath} placeholder="/Users/me/Documents/tau-sync-report.json"/><button class="picker" on:click={() => chooseFolder('manifest')}>Choose</button></div><label class="cover-option"><input type="checkbox" bind:checked={embedCovers}/> Add folder cover art to MP3 and FLAC copies <small>Baseline JPEG only. Your originals are never changed.</small></label><button class="primary" on:click={makePlan}>Review plan</button></section>
    {#if plan}<section class="plan-card" aria-labelledby="plan-title"><div><p class="eyebrow">READY FOR CONFIRMATION</p><h2 id="plan-title">{plan.id}</h2><p class="lede">{plan.new_files} new · {plan.updates} updated · {plan.unchanged} unchanged · {size(plan.bytes_to_write)} to write</p>{#if syncCapacity}<p class:capacity-ok={syncCapacity.fits} class:capacity-bad={!syncCapacity.fits}>{syncCapacity.fits ? 'Fits' : 'Does not fit'} on the destination volume — {size(syncCapacity.space.available_bytes)} free.</p>{/if}</div><button class="danger" on:click={runSync}>Confirm and sync</button>{#if syncProgress}<p class="notice" role="status">{syncProgress} <button class="quiet" on:click={cancelSync}>Cancel</button></p>{/if}{#if plan.warnings.length}<ul>{#each plan.warnings as warning}<li>{warning.message}</li>{/each}</ul>{/if}<p class="safety">This action targets the exact destination above. It copies, hashes, verifies, and writes the index last.</p></section>{/if}<p class="notice" role="status">{syncNotice}</p></section>
  {:else}<section class="page compare-page"><header><div><p class="eyebrow">CORE TRANSFER</p><h1>Compare, then copy safely</h1><p class="lede">Compare two Tau media roots before copying the complete library to the second core.</p></div></header><section class="wizard" aria-labelledby="compare-title"><h2 id="compare-title">Compare two cores</h2><label for="left-core">Source media root</label><div class="picker-row"><input id="left-core" bind:value={leftCore} placeholder="/Volumes/Pocket/Assets/tau/common"/><button class="picker" on:click={() => chooseFolder('left')}>Choose</button></div><label for="right-core">Destination media root</label><div class="picker-row"><input id="right-core" bind:value={rightCore} placeholder="/Volumes/Pocket/Assets/tau-test/common"/><button class="picker" on:click={() => chooseFolder('right')}>Choose</button></div><label for="copy-report">Copy report location on this computer</label><div class="picker-row"><input id="copy-report" bind:value={copyReportPath} placeholder="/Users/me/Documents/tau-core-copy.json"/><button class="picker" on:click={() => chooseFolder('copyReport')}>Choose</button></div><button class="primary" on:click={compareCores}>Compare safely</button></section><p class="notice" role="status">{compareNotice}</p>{#if comparison}<section class="comparison" aria-labelledby="comparison-title"><div class="section-title"><div><p class="eyebrow">RESULT</p><h2 id="comparison-title">{comparison.differences.length} files compared</h2></div><span>Read-only</span></div><div class="comparison-counts"><span><b>{comparison.only_left}</b> only in source</span><span><b>{comparison.only_right}</b> only in destination</span><span><b>{comparison.different}</b> different</span><span><b>{comparison.identical}</b> matching</span></div><ul class="difference-list">{#each comparison.differences as item}<li><span class={`difference ${item.state}`}>{item.state === 'only_left' ? 'Source only' : item.state === 'only_right' ? 'Destination only' : item.state === 'different' ? 'Different' : 'Matching'}</span><span>{item.relative}</span><span>{item.left_bytes === null ? '—' : size(item.left_bytes)} / {item.right_bytes === null ? '—' : size(item.right_bytes)}</span></li>{/each}</ul><button class="primary" on:click={reviewCoreCopy}>Review full-library copy</button>{#if coreCopyPlan}<div class="copy-plan"><p class="eyebrow">READY FOR CONFIRMATION</p><h3>{coreCopyPlan.id}</h3><p>{coreCopyPlan.new_files} new · {coreCopyPlan.updates} updated · {coreCopyPlan.unchanged} unchanged · {size(coreCopyPlan.bytes_to_write)} to write</p>{#if coreCopyCapacity}<p class:capacity-ok={coreCopyCapacity.fits} class:capacity-bad={!coreCopyCapacity.fits}>{coreCopyCapacity.fits ? 'Fits' : 'Does not fit'} on the destination volume — {size(coreCopyCapacity.space.available_bytes)} free.</p>{/if}<button class="danger" on:click={runCoreCopy}>Confirm and copy</button></div>{/if}<p class="safety">The source is never changed. The destination files are verified and its index is rebuilt last. Move remains unavailable until its separate backup and deletion review is ready.</p></section>{/if}</section>{/if}
</main>

{#if page === 'jobs'}
  <JobsView {jobs} bind:journalPath notice={journalNotice} chooseJournal={() => chooseFolder('journal')} {loadJournal} startSync={() => page = 'sync'} {reportsDir} {historyNotice} {history} {chooseReportsDir} {refreshHistory} detail={journalDetail} detailNotice={journalDetailNotice} openDetail={openJournalDetail} closeDetail={closeJournalDetail} />
{/if}
{#if page === 'playlists'}
  <PlaylistsView bind:path={playlistPath} result={playlistResult} notice={playlistNotice} bind:output={playlistOutput} bind:selected={selectedPlaylist} choose={() => chooseFolder('playlists')} scan={scanPlaylists} exportList={exportSelectedPlaylist}
    {selectedPlaylistDetail} {reorderTracks} {moveTrack} {reorderPlan} {reorderNotice} {reviewReorder} {confirmReorder}
    bind:renameNewFile {renamePlan} {renameNotice} {reviewRename} {confirmRename}
    bind:createFile bind:createTracksText {createPlan} {createNotice} {reviewCreate} {confirmCreate}
    bind:importSource bind:importDestFile {importPlan} {importNotice} chooseImportSource={() => chooseFolder('importSource')} {reviewImport} {confirmImport} />
{/if}
{#if page === 'problems'}
  <ProblemsView bind:path={problemsPath} {problems} notice={problemsNotice} loading={problemsLoading} choose={() => chooseFolder('problems')} scan={scanProblems} />
{/if}
{#if page === 'library'}
  <LibraryView bind:path={libraryPath} scan={libraryScan} rows={libraryRows} notice={libraryNotice} loading={libraryLoading} progress={libraryProgress} bind:search={librarySearch} bind:format={libraryFormat} choose={() => chooseFolder('library')} inspect={inspectLibrary} cancel={cancelLibraryScan} />
{/if}
{#if page === 'backup'}
  <BackupView bind:sourcePath={backupSourcePath} bind:destinationPath={backupDestPath} chooseSource={() => chooseFolder('backupSource')} chooseDestination={() => chooseFolder('backupDestination')} review={reviewBackup} backupPlan={backupPlanResult} {backupNotice} capacity={backupCapacity} />
{/if}
{#if false && page === 'jobs'}
  <section class="jobs-panel page" aria-labelledby="jobs-title"><header><div><p class="eyebrow">ACTIVITY</p><h1 id="jobs-title">Recent jobs</h1><p class="lede">Operations completed during this session.</p></div><button class="primary" on:click={() => page = 'sync'}>Start a sync</button></header>{#if jobs.length}<section class="core-list">{#each jobs as job}<article><div class="core-icon">{job.status === 'Completed' ? '✓' : '!'}</div><div><h3>{job.kind}</h3><p>{job.detail}</p></div><span class="chip" class:capable={job.status === 'Completed'}>{job.status}</span></article>{/each}</section>{:else}<section class="empty"><div class="empty-art">◷</div><h2>No jobs yet</h2><p>Reviewed syncs will appear here after they complete.</p></section>{/if}</section>
{/if}

{#if page === 'settings'}
  <SettingsView bind:settingsPath {settings} notice={settingsNotice} choose={() => chooseFolder('settings')} read={loadSettings} done={() => page = 'cards'} label={settingLabel} value={settingValue} />
{/if}
{#if false && page === 'settings'}
  <section class="jobs-panel page" aria-labelledby="settings-title"><header><div><p class="eyebrow">PREFERENCES</p><h1 id="settings-title">Settings</h1><p class="lede">Tau Omega keeps your library work local and reviewable.</p></div></header><section class="settings-card"><div><h2>Inspect persisted settings</h2><p>Choose a core’s <code>interact_persist.json</code> to read its saved values. This never writes to the card.</p><div class="picker-row"><input bind:value={settingsPath} placeholder="/Volumes/Pocket/Settings/.../interact_persist.json"/><button class="picker" on:click={() => chooseFolder('settings')}>Choose</button><button class="primary" on:click={loadSettings}>Read</button></div><p class="notice" role="status">{settingsNotice}</p>{#if settings.length}<div class="settings-values">{#each settings as setting}<div><span>{settingLabel(setting.id)}</span><code>{setting.kind}</code><strong>{settingValue(setting)}</strong></div>{/each}</div>{/if}</div></section><section class="settings-card"><div><h2>Safety defaults</h2><p>Card writes are disabled until you review and confirm an explicit plan. Move operations require a verified external backup and a second confirmation.</p></div><span class="chip capable">Enabled</span></section><section class="settings-card"><div><h2>Data handling</h2><p>Paths, reports, and job journals stay on this computer. Tau Omega does not upload your media or metadata.</p></div><span class="chip capable">Local only</span></section><button class="primary" on:click={() => page = 'cards'}>Done</button></section>
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
