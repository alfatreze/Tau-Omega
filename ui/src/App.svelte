<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import type { Comparison, Core, Difference, DuplicateGroup, Job, LibrarySummary, MediaScan, Plan, Setting } from './lib/types';
  import { invoke } from './lib/backend';
  import { cancelJob, compareMedia, errorMessage, executeCoreCopy, executeCoreMove, executeSync, exportPlaylist, findDuplicates, inspectCard, newJobId, onProgress, planCoreCopy, planSync, readJournal, readPersistedSettings, scanMedia, summarizeLibrary } from './lib/tau-api';
  import SettingsView from './lib/SettingsView.svelte';
  import JobsView from './lib/JobsView.svelte';
  import PlaylistsView from './lib/PlaylistsView.svelte';
  import ProblemsView from './lib/ProblemsView.svelte';
  import LibraryView from './lib/LibraryView.svelte';
  let page: 'cards' | 'compare' | 'sync' | 'jobs' | 'settings' | 'playlists' | 'problems' | 'library' = 'cards'; let cores: Core[] = []; let path = ''; let jobs: Job[] = []; let journalPath = ''; let journalNotice = 'Choose a host-side Tau Omega journal to load it.'; let problemsPath = ''; let duplicateGroups: DuplicateGroup[] | null = null; let problemsNotice = 'Choose a media root to inspect duplicate files.'; let problemsLoading = false; let playlistPath = ''; let playlistResult: MediaScan | null = null; let playlistNotice = 'Choose a media root to inspect playlists.'; let playlistOutput = ''; let selectedPlaylist = ''; let settingsPath = ''; let settings: Setting[] = []; let settingsNotice = 'Choose a persisted settings file to inspect it.';
  let libraryPath = ''; let librarySummary: LibrarySummary | null = null; let libraryNotice = 'Choose a media root to inspect it.';
  let syncJobId = ''; let syncProgress = '';
  onProgress((event) => { if (event.job_id === syncJobId) syncProgress = `${event.stage}: ${event.done}${event.total ? ` / ${event.total}` : ''}${event.path ? ` (${event.path})` : ''}`; });
  let notice = 'Open a card folder to inspect it. Tau Omega will not write anything at this stage.';
  let sources = ''; let destination = ''; let manifestPath = ''; let embedCovers = false; let plan: Plan | null = null; let syncNotice = '';
  let leftCore = ''; let rightCore = ''; let comparison: Comparison | null = null; let compareNotice = ''; let copyReportPath = ''; let coreCopyPlan: Plan | null = null; let moveSource = false; let moveBackupPath = ''; let approveDeletion = false;
  async function chooseFolder(target: 'card' | 'source' | 'destination' | 'left' | 'right' | 'manifest' | 'copyReport' | 'backup' | 'settings' | 'playlists' | 'problems' | 'journal') {
    const selected = await open({ directory: !['manifest', 'copyReport', 'settings', 'journal'].includes(target), multiple: target === 'source' });
    if (!selected) return;
    const value = Array.isArray(selected) ? selected.join('\n') : selected;
    if (target === 'card') path = value; if (target === 'source') sources = value; if (target === 'destination') destination = value;
    if (target === 'left') leftCore = value; if (target === 'right') rightCore = value; if (target === 'manifest') manifestPath = value; if (target === 'copyReport') copyReportPath = value; if (target === 'backup') moveBackupPath = value; if (target === 'settings') settingsPath = value; if (target === 'playlists') playlistPath = value; if (target === 'problems') problemsPath = value; if (target === 'journal') journalPath = value;
  }
  async function loadSettings() { try { settings = await readPersistedSettings(settingsPath); settingsNotice = `${settings.length} persisted values loaded. Nothing was changed.`; } catch (error) { settings = []; settingsNotice = `Could not read settings: ${errorMessage(error)}`; } }
  async function scanPlaylists() { try { playlistResult = await scanMedia(playlistPath, newJobId()); playlistNotice = `${playlistResult.playlists.length} playlists found. Nothing was changed.`; } catch (error) { playlistResult = null; playlistNotice = `Could not scan this folder: ${errorMessage(error)}`; } }
  async function scanDuplicates() { problemsLoading = true; try { duplicateGroups = await findDuplicates(problemsPath); problemsNotice = `${duplicateGroups.length} duplicate groups found. Nothing was changed.`; } catch (error) { duplicateGroups = null; problemsNotice = `Could not scan this folder: ${errorMessage(error)}`; } finally { problemsLoading = false; } }
  async function inspectLibrary() { try { librarySummary = await summarizeLibrary(libraryPath, newJobId()); libraryNotice = 'Library inspected. Nothing was changed.'; } catch (error) { librarySummary = null; libraryNotice = `Could not inspect this folder: ${errorMessage(error)}`; } }
  async function loadJournal() { try { const journal = await readJournal(journalPath) as { state?: string; plan?: { files?: number }; result?: { copied?: number } }; jobs = [{ kind: 'Loaded journal', status: journal.state === 'completed' ? 'Completed' : journal.state ?? 'Unknown', detail: `${journal.result?.copied ?? 0} copied · ${journal.plan?.files ?? 0} planned` }, ...jobs]; journalNotice = 'Journal loaded. Nothing was changed.'; } catch (error) { journalNotice = `Could not load journal: ${errorMessage(error)}`; } }
  async function exportSelectedPlaylist() { try { await exportPlaylist(playlistPath, selectedPlaylist, playlistOutput); playlistNotice = `Exported ${selectedPlaylist}. Nothing in the source library was changed.`; } catch (error) { playlistNotice = `Could not export playlist: ${errorMessage(error)}`; } }
  const settingLabel = (id: number) => ({ 10: 'Volume', 11: 'Colour index', 12: 'Repeat', 13: 'Shuffle', 15: 'Meter', 16: 'EQ', 18: 'Saved position', 19: 'Resume on', 24: 'Library history', 25: 'Index build ID', 26: 'Shuffle All seed', 27: 'Library off' } as Record<number, string>)[id] ?? `Word ${id}`;
  const settingValue = (setting: Setting) => { if (setting.id !== 24 || typeof setting.value !== 'number') return JSON.stringify(setting.value); const word = setting.value >>> 0; const kinds: Record<number, string> = { 1: 'album', 2: 'artist', 3: 'playlist', 4: 'all tracks A–Z', 5: 'Shuffle All' }; return `${kinds[word & 7] ?? 'unknown'} · item ${word >>> 3 & 0x7ff} · queue ${word >>> 14 & 0x3fff}`; };
  async function openFolder() { if (!path.trim()) { notice = 'Enter a staging-card folder path first.'; return; } try { cores = await inspectCard(path); notice = `${cores.length} core${cores.length === 1 ? '' : 's'} found. This is a read-only inspection.`; } catch (error) { notice = `Could not inspect this folder: ${errorMessage(error)}`; cores = []; } }
  async function makePlan() { plan = null; syncNotice = ''; try { plan = await planSync(sources.split('\n'), destination, embedCovers); syncNotice = `Plan ${plan.id} is ready for review. Nothing has been written.`; } catch (error) { syncNotice = `Could not make a plan: ${errorMessage(error)}`; } }
  async function runSync() { if (!plan) return; syncJobId = newJobId(); syncProgress = 'starting…'; try { const result = await executeSync(sources.split('\n'), destination, plan.id, manifestPath, embedCovers, syncJobId); syncNotice = `Verified: ${result.copied} copied, ${result.unchanged} unchanged. Index: ${result.index_path}`; jobs = [{ kind: 'Library sync', status: 'Completed', detail: `${result.copied} copied · ${result.unchanged} unchanged` }, ...jobs]; plan = null; } catch (error) { syncNotice = `Nothing was reported as complete: ${errorMessage(error)}`; jobs = [{ kind: 'Library sync', status: 'Failed', detail: errorMessage(error) }, ...jobs]; } finally { syncProgress = ''; } }
  async function cancelSync() { if (syncJobId) await cancelJob(syncJobId); }
  async function compareCores() { comparison = null; compareNotice = ''; try { comparison = await compareMedia(leftCore, rightCore); compareNotice = `${comparison.differences.length} supported files compared. Nothing was changed.`; } catch (error) { compareNotice = `Could not compare these media roots: ${errorMessage(error)}`; } }
  async function reviewCoreCopy() { coreCopyPlan = null; moveSource = false; approveDeletion = false; try { coreCopyPlan = await planCoreCopy(leftCore, rightCore); compareNotice = `Copy plan ${coreCopyPlan.id} is ready for review. The first core remains untouched.`; } catch (error) { compareNotice = `Could not make a copy plan: ${errorMessage(error)}`; } }
  async function runCoreCopy() { if (!coreCopyPlan) return; if (moveSource && !approveDeletion) { compareNotice = 'Confirm that the source will be backed up and deleted before moving.'; return; } if (moveSource && !moveBackupPath.trim()) { compareNotice = 'Choose a visible external backup folder before moving.'; return; } const jobId = newJobId(); try { const result = moveSource ? await executeCoreMove(leftCore, rightCore, coreCopyPlan.id, moveBackupPath, copyReportPath, jobId) : await executeCoreCopy(leftCore, rightCore, coreCopyPlan.id, copyReportPath, jobId); compareNotice = `Verified ${moveSource ? 'move' : 'copy'}: ${result.copied} copied, ${result.unchanged} unchanged. Index: ${result.index_path}`; coreCopyPlan = null; } catch (error) { compareNotice = `Nothing was reported as complete: ${errorMessage(error)}`; } }
  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
</script>

<main><aside aria-label="Primary navigation"><div class="brand"><span class="mark">τ</span><span>Tau Omega<small>Library companion</small></span></div><nav><button class:active={page === 'cards'} on:click={() => page = 'cards'}>Cards</button><button class:active={page === 'compare'} on:click={() => page = 'compare'}>Compare cores</button><button class:active={page === 'sync'} on:click={() => page = 'sync'}>Sync library</button><button class:active={page === 'playlists'} on:click={() => page = 'playlists'}>Playlists</button><button class:active={page === 'problems'} on:click={() => page = 'problems'}>Problems</button><button class:active={page === 'jobs'} on:click={() => page = 'jobs'}>Recent jobs</button><button class:active={page === 'settings'} on:click={() => page = 'settings'}>Settings</button></nav><p class="offline">Local only<br/><span>No card writes without a reviewed plan.</span></p></aside>
  {#if page === 'cards'}<section class="page" id="cards"><header><div><p class="eyebrow">CARD LIBRARY</p><h1>Start with a card</h1><p class="lede">Inspect a Pocket card or a staging folder. Your music stays untouched.</p></div><button class="primary" on:click={openFolder}>Open folder</button></header>
    <section class="open-card" aria-labelledby="open-card-title"><div><p class="eyebrow">READ-ONLY</p><h2 id="open-card-title">Open a staging card</h2><p>Choose a folder containing <code>Cores</code> and <code>Assets</code>. You can review its cores before planning any changes.</p></div><div class="folder"><label for="card-path">Card or staging-folder path</label><div><input id="card-path" bind:value={path} placeholder="/Volumes/My Pocket"/><button on:click={() => chooseFolder('card')}>Choose</button><button on:click={openFolder}>Inspect</button></div></div></section><p class="notice" role="status">{notice}</p>
    {#if cores.length}<section aria-labelledby="core-title"><div class="section-title"><div><p class="eyebrow">DETECTED</p><h2 id="core-title">Cores on this card</h2></div><span>{cores.length} found</span></div><div class="core-list">{#each cores as core}<article><div class="core-icon">{core.id.split('.').at(-1)?.[0] ?? 'τ'}</div><div><h3>{core.id}</h3><p>{core.author || 'Unknown author'} · {core.version || 'Version unknown'} · {core.platform || 'No platform declared'} · {core.index_status}{core.tracks === null ? '' : ` · ${core.tracks} tracks`}</p></div><span class:capable={core.library_capable} class="chip">{core.library_capable ? 'Library ready' : 'Legacy core'}</span><button class="quiet" aria-label={`Open ${core.id}`}>View</button></article>{/each}</div></section>{:else}<section class="empty"><div class="empty-art">◒</div><h2>No card selected</h2><p>Open a card folder to see its cores and library health.</p></section>{/if}</section>
  {:else if page === 'sync'}<section class="page sync-page"><header><div><p class="eyebrow">SAFE SYNC</p><h1>Review before copying</h1><p class="lede">Sources are read-only. Every file is verified, then the index is published last.</p></div></header><section class="wizard" aria-labelledby="sync-title"><div class="steps" aria-label="Sync steps"><span class="current">1 Sources</span><span class:current={plan}>2 Plan</span><span>3 Confirm</span></div><h2 id="sync-title">Build a sync plan</h2><label for="sources">Source folders or files — one per line</label><div class="picker-row"><textarea id="sources" bind:value={sources} placeholder="/Users/me/Music/Album"></textarea><button class="picker" on:click={() => chooseFolder('source')}>Choose</button></div><label for="destination">Destination media root</label><div class="picker-row"><input id="destination" bind:value={destination} placeholder="/Volumes/Pocket/Assets/tau/common"/><button class="picker" on:click={() => chooseFolder('destination')}>Choose</button></div><label for="manifest">Report location on this computer</label><div class="picker-row"><input id="manifest" bind:value={manifestPath} placeholder="/Users/me/Documents/tau-sync-report.json"/><button class="picker" on:click={() => chooseFolder('manifest')}>Choose</button></div><label class="cover-option"><input type="checkbox" bind:checked={embedCovers}/> Add folder cover art to MP3 and FLAC copies <small>Baseline JPEG only. Your originals are never changed.</small></label><button class="primary" on:click={makePlan}>Review plan</button></section>
    {#if plan}<section class="plan-card" aria-labelledby="plan-title"><div><p class="eyebrow">READY FOR CONFIRMATION</p><h2 id="plan-title">{plan.id}</h2><p class="lede">{plan.new_files} new · {plan.updates} updated · {plan.unchanged} unchanged · {size(plan.bytes_to_write)} to write</p></div><button class="danger" on:click={runSync}>Confirm and sync</button>{#if syncProgress}<p class="notice" role="status">{syncProgress} <button class="quiet" on:click={cancelSync}>Cancel</button></p>{/if}{#if plan.warnings.length}<ul>{#each plan.warnings as warning}<li>{warning.message}</li>{/each}</ul>{/if}<p class="safety">This action targets the exact destination above. It copies, hashes, verifies, and writes the index last.</p></section>{/if}<p class="notice" role="status">{syncNotice}</p></section>
  {:else}<section class="page compare-page"><header><div><p class="eyebrow">CORE TRANSFER</p><h1>Compare, then copy safely</h1><p class="lede">Compare two Tau media roots before copying the complete library to the second core.</p></div></header><section class="wizard" aria-labelledby="compare-title"><h2 id="compare-title">Compare two cores</h2><label for="left-core">Source media root</label><div class="picker-row"><input id="left-core" bind:value={leftCore} placeholder="/Volumes/Pocket/Assets/tau/common"/><button class="picker" on:click={() => chooseFolder('left')}>Choose</button></div><label for="right-core">Destination media root</label><div class="picker-row"><input id="right-core" bind:value={rightCore} placeholder="/Volumes/Pocket/Assets/tau-test/common"/><button class="picker" on:click={() => chooseFolder('right')}>Choose</button></div><label for="copy-report">Copy report location on this computer</label><div class="picker-row"><input id="copy-report" bind:value={copyReportPath} placeholder="/Users/me/Documents/tau-core-copy.json"/><button class="picker" on:click={() => chooseFolder('copyReport')}>Choose</button></div><button class="primary" on:click={compareCores}>Compare safely</button></section><p class="notice" role="status">{compareNotice}</p>{#if comparison}<section class="comparison" aria-labelledby="comparison-title"><div class="section-title"><div><p class="eyebrow">RESULT</p><h2 id="comparison-title">{comparison.differences.length} files compared</h2></div><span>Read-only</span></div><div class="comparison-counts"><span><b>{comparison.only_left}</b> only in source</span><span><b>{comparison.only_right}</b> only in destination</span><span><b>{comparison.different}</b> different</span><span><b>{comparison.identical}</b> matching</span></div><ul class="difference-list">{#each comparison.differences as item}<li><span class={`difference ${item.state}`}>{item.state === 'only_left' ? 'Source only' : item.state === 'only_right' ? 'Destination only' : item.state === 'different' ? 'Different' : 'Matching'}</span><span>{item.relative}</span><span>{item.left_bytes === null ? '—' : size(item.left_bytes)} / {item.right_bytes === null ? '—' : size(item.right_bytes)}</span></li>{/each}</ul><button class="primary" on:click={reviewCoreCopy}>Review full-library copy</button>{#if coreCopyPlan}<div class="copy-plan"><p class="eyebrow">READY FOR CONFIRMATION</p><h3>{coreCopyPlan.id}</h3><p>{coreCopyPlan.new_files} new · {coreCopyPlan.updates} updated · {coreCopyPlan.unchanged} unchanged · {size(coreCopyPlan.bytes_to_write)} to write</p><button class="danger" on:click={runCoreCopy}>Confirm and copy</button></div>{/if}<p class="safety">The source is never changed. The destination files are verified and its index is rebuilt last. Move remains unavailable until its separate backup and deletion review is ready.</p></section>{/if}</section>{/if}
</main>

{#if page === 'jobs'}
  <JobsView {jobs} bind:journalPath notice={journalNotice} chooseJournal={() => chooseFolder('journal')} {loadJournal} startSync={() => page = 'sync'} />
{/if}
{#if page === 'playlists'}
  <PlaylistsView bind:path={playlistPath} result={playlistResult} notice={playlistNotice} bind:output={playlistOutput} bind:selected={selectedPlaylist} choose={() => chooseFolder('playlists')} scan={scanPlaylists} exportList={exportSelectedPlaylist} />
{/if}
{#if page === 'problems'}
  <ProblemsView bind:path={problemsPath} groups={duplicateGroups} notice={problemsNotice} loading={problemsLoading} choose={() => chooseFolder('problems')} scan={scanDuplicates} />
{/if}
{#if page === 'library'}
  <LibraryView bind:path={libraryPath} summary={librarySummary} notice={libraryNotice} choose={() => libraryNotice = 'Enter a media root path, then inspect it.'} scan={inspectLibrary} />
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
  .picker-row { display: flex; gap: 8px; align-items: stretch; }
  .picker-row input, .picker-row textarea { flex: 1; min-width: 0; }
  .picker { padding: 0 12px; background: #314244; color: #e8ecec; font-weight: 600; border-radius: 8px; }
  .jobs-panel { position: fixed; inset: 0 0 0 244px; z-index: 5; background: #111617; overflow: auto; }
  .settings-card { display: flex; justify-content: space-between; gap: 24px; align-items: center; max-width: 720px; margin-bottom: 14px; padding: 22px 24px; border: 1px solid #2c393a; border-radius: 12px; background: #1a2325; }
  .settings-card h2 { margin-bottom: 7px; }
  .settings-card p { margin: 0; color: #a6b3b2; line-height: 1.55; font-size: 13px; }
  .settings-values { display: grid; gap: 6px; margin-top: 14px; }
  .settings-values div { display: grid; grid-template-columns: 80px 100px 1fr; gap: 10px; align-items: center; padding: 9px 10px; background: #101617; border-radius: 7px; color: #c7d1d0; font-size: 12px; }
  .settings-values code { color: #9fb3aa; }
  .settings-values strong { font-weight: 500; overflow-wrap: anywhere; }
  @font-face { font-family: 'Space Grotesk'; src: url('/assets/SpaceGrotesk-VariableFont_wght.ttf') format('truetype'); font-style: normal; font-weight: 300 700; font-display: swap; }
  :global(:root), :global(body) { font-family: 'Space Grotesk', ui-sans-serif, system-ui, sans-serif; }
  .comparison { margin-top: 20px; border: 1px solid #2c393a; border-radius: 15px; padding: 29px 31px; background: #1a2325; }
  .comparison-counts { display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px; margin: 22px 0; }
  .comparison-counts span { padding: 12px; background: #101617; border: 1px solid #344244; border-radius: 9px; color: #aab8b7; font-size: 12px; }
  .comparison-counts b { display: block; color: #edf4f2; font-size: 21px; margin-bottom: 4px; }
  .difference-list { padding: 0; margin: 0; border: 1px solid #344244; border-radius: 9px; overflow: auto; max-height: 380px; }
  .difference-list li { min-width: 500px; display: grid; grid-template-columns: 100px 1fr 110px; gap: 12px; align-items: center; padding: 10px 12px; border-bottom: 1px solid #2c393a; font-size: 12px; color: #c7d1d0; }
  .difference-list li:last-child { border-bottom: 0; }
  .difference-list li > span:last-child { color: #8f9e9d; text-align: right; }
  .difference { font-size: 11px; font-weight: 700; }
  .difference.only_left, .difference.only_right { color: #d9c47e; }
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
