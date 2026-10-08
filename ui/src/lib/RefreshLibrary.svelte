<script lang="ts">
  import { onMount } from 'svelte';
  import { modal } from './a11y';
  import { libraryHealth, planLibraryRefresh, executeLibraryRefresh, rollbackLibraryRefresh } from './tau-api';
  import { errorMessage } from './backend';
  import type { IndexState, RefreshPlan, RefreshReport } from './types';
  /** The core's media folder on the card: `<card>/Assets/<platform>/common`. */
  export let mediaRoot: string;
  export let coreName = '';
  /** Called after a refresh or an undo so the page can re-read the card. */
  export let onChanged: () => void = () => {};

  let health: IndexState | null = null;
  let open = false; let busy = false; let notice = '';
  let plan: RefreshPlan | null = null; let report: RefreshReport | null = null; let undone = false; let confirmUndo = false;

  onMount(loadHealth);
  async function loadHealth() { try { health = await libraryHealth(mediaRoot); } catch { health = null; } }
  $: badge = !health ? '' : !health.present ? 'No library yet' : !health.valid ? 'Library damaged' : !health.root_matches ? 'Library is for another core' : health.missing_files > 0 ? `${health.missing_files} listed track${health.missing_files === 1 ? '' : 's'} missing` : `${health.tracks} tracks`;
  $: bad = !!health && (!health.present || !health.valid || !health.root_matches || health.missing_files > 0);

  const REASON: Record<string, string> = { unsupported_format: 'Can’t play', non_ascii_name: 'Renamed', name_collision: 'Left out', path_too_long: 'Left out', tags_unreadable: 'Listed by name', old_tag_version: 'Listed by name', over_capacity: 'Too many' };

  async function start() {
    open = true; busy = true; notice = ''; report = null; undone = false; confirmUndo = false;
    try { plan = await planLibraryRefresh(mediaRoot); } catch (error) { plan = null; notice = `Could not check this library: ${errorMessage(error)}`; }
    finally { busy = false; }
  }
  async function run() {
    if (!plan || plan.refused) return;
    busy = true; notice = '';
    try { report = await executeLibraryRefresh(mediaRoot, plan.id); plan = null; await loadHealth(); onChanged(); }
    catch (error) { notice = `Nothing was left half-done. ${errorMessage(error)}`; }
    finally { busy = false; }
  }
  async function undo() {
    if (!report) return;
    busy = true; notice = '';
    try { await rollbackLibraryRefresh(mediaRoot, report.backup_dir); undone = true; confirmUndo = false; await loadHealth(); onChanged(); }
    catch (error) { notice = `The folder was not changed by this: ${errorMessage(error)}`; }
    finally { busy = false; }
  }
  const close = () => { open = false; plan = null; };
</script>

<div class="rl-row">
  <div><span>Library health</span><strong class:bad>{badge || '—'}</strong></div>
  <button class="quiet" on:click={start}>Refresh library…</button>
</div>

{#if open}
  <div class="rl-backdrop" role="presentation" on:click={close}></div>
  <div class="rl-panel" role="dialog" aria-modal="true" aria-labelledby="rl-title" use:modal>
    <div class="rl-head"><h2 id="rl-title">Refresh library{coreName ? ` · ${coreName}` : ''}</h2><button class="quiet" aria-label="Close" on:click={close}>✕</button></div>
    <p class="rl-lede">Checks the music in this core’s folder against its library, and shows what would be fixed. Your music files are never changed, only renamed to their plain-letter form when the Pocket could not open them. Nothing happens until you confirm.</p>
    {#if busy && !plan && !report}<p role="status">Checking…</p>{/if}
    {#if notice}<p class="rl-error" role="alert">{notice}</p>{/if}

    {#if plan}
      <div class="rl-counts">
        <span><b>{plan.media_files}</b> MP3/FLAC files found</span>
        <span><b>{plan.would_index}</b> will be in the library</span>
        <span>now: <b>{plan.before.present ? (plan.before.valid ? `${plan.before.tracks} tracks` : 'damaged') : 'no library'}</b></span>
      </div>
      {#if plan.before.present && plan.before.valid && !plan.before.root_matches}<p class="rl-warn">The current library is rooted at <code>{plan.before.root}</code>, not this core, so its tracks cannot open. Rebuilding fixes that.</p>{/if}
      {#if plan.refused}
        <p class="rl-error" role="alert">{plan.refused}</p>
      {:else if plan.nothing_to_do}
        <p class="rl-ok">The library already matches the music: nothing to do.</p>
      {/if}
      {#if plan.findings.length}
        <h3>{plan.findings.length} file{plan.findings.length === 1 ? '' : 's'} need attention</h3>
        <ul class="rl-list">
          {#each plan.findings as f}
            <li><span class="tag" class:skip={f.skipped}>{REASON[f.reason] ?? f.reason}</span><span class="rel">{f.rel || 'The whole library'}</span><span class="why">{f.message}{#if f.fix} → <code>{f.fix}</code>{/if}</span></li>
          {/each}
        </ul>
      {/if}
      {#if !plan.refused && !plan.nothing_to_do}
        <div class="rl-safety">
          <p>{plan.renames.length ? `${plan.renames.length} name${plan.renames.length === 1 ? '' : 's'} will be changed to plain letters. ` : ''}{plan.playlist_fixes.length ? `${plan.playlist_fixes.length} playlist${plan.playlist_fixes.length === 1 ? '' : 's'} will be updated to follow. ` : ''}{plan.stubs_to_remove ? `${plan.stubs_to_remove} leftover ._ files are removed. ` : ''}The old library file and any playlist that changes are copied to a backup folder on this computer first, and the new library is checked against the files on the card. If anything goes wrong it all goes back by itself.</p>
          <button class="danger" disabled={busy} on:click={run}>{busy ? 'Refreshing…' : 'Confirm and refresh'}</button>
        </div>
      {/if}
    {/if}

    {#if report}
      <h3>{report.nothing_to_do ? 'Nothing was changed' : 'Library refreshed'}</h3>
      {#if !report.nothing_to_do}
        <div class="rl-counts"><span>before: <b>{report.before.present ? `${report.before.tracks ?? '?'} tracks` : 'none'}</b></span><span>after: <b>{report.after.tracks} tracks</b>, all files present</span><span>{report.renamed} renamed · {report.playlists_rewritten} playlists · {report.stubs_removed} stray files removed</span></div>
        <p class="rl-safety">Backup: <code>{report.backup_dir}</code></p>
        {#if !undone}
          {#if !confirmUndo}<button class="quiet" disabled={busy} on:click={() => (confirmUndo = true)}>Undo this refresh…</button>
          {:else}<span role="alert">Put the names and the old library back? <button class="danger" disabled={busy} on:click={undo}>Yes, undo</button> <button class="quiet" on:click={() => (confirmUndo = false)}>Keep it</button></span>{/if}
        {:else}<p class="rl-ok" role="status">Undone: the names and the old library are back.</p>{/if}
      {/if}
    {/if}
  </div>
{/if}

<style>
  .rl-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 8px 0; }
  .rl-row span { display: block; font-size: 11px; color: #8f9e9d; } .rl-row strong.bad { color: #f2c9a0; }
  .rl-backdrop { position: fixed; inset: 0; z-index: 30; background: rgb(5 9 10 / 72%); }
  .rl-panel { position: fixed; z-index: 31; inset: 5vh 0 auto 0; margin: 0 auto; width: min(760px, 94vw); max-height: 90vh; overflow: auto; display: grid; gap: 12px; padding: 22px 24px; border: 1px solid #455654; border-radius: 15px; background: #1a2325; color: #dce6e4; }
  .rl-head { display: flex; justify-content: space-between; align-items: center; } h2 { margin: 0; font-size: 18px; } h3 { margin: 6px 0 0; font-size: 14px; }
  .rl-lede { margin: 0; color: #aab8b7; font-size: 13px; line-height: 1.5; }
  .rl-counts { display: flex; flex-wrap: wrap; gap: 6px 18px; font-size: 13px; } .rl-counts b { color: #fff; }
  .rl-ok { color: #b9e9a5; margin: 0; } .rl-warn { color: #f2c9a0; margin: 0; font-size: 13px; } .rl-error { color: #ff9b9b; margin: 0; }
  .rl-list { list-style: none; margin: 0; padding: 0; border: 1px solid #344244; border-radius: 9px; max-height: 300px; overflow: auto; }
  .rl-list li { display: grid; grid-template-columns: 110px minmax(0, 1fr); gap: 4px 12px; padding: 9px 12px; border-bottom: 1px solid #2c393a; font-size: 12px; }
  .rl-list li:last-child { border-bottom: 0; } .rel { color: #fff; overflow-wrap: anywhere; } .why { grid-column: 2; color: #aab8b7; }
  .tag { font-size: 11px; font-weight: 700; color: #d9c47e; } .tag.skip { color: #f29b83; }
  .rl-safety { display: grid; gap: 8px; font-size: 13px; color: #aab8b7; } .rl-safety p { margin: 0; }
  button.danger { justify-self: start; padding: 11px 14px; border-radius: 8px; background: #5a352c; color: #f4cfc4; font-weight: 700; } button.danger:disabled { opacity: .5; }
</style>
