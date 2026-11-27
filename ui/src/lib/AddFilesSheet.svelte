<script lang="ts">
  // Review before anything is written: what each dropped file would become on the card, what is skipped or refused, then Add.
  import { createEventDispatcher, onMount } from 'svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { executeCoreAdd, planCoreAdd } from './tau-api';
  import { plural, size } from './coresFormat';
  import type { AddDestination, AddPlan, AddReport } from './types';
  export let card: string; export let coreId: string; export let platform: string; export let platformName = ''; export let coreName = '';
  export let sources: string[];
  /** Where the files go to start with: the shared folder (most cores read it) or this core only. */
  export let destination: AddDestination = 'Shared';
  const dispatch = createEventDispatcher<{ close: void; done: AddReport }>();

  let plan: AddPlan | null = null; let report: AddReport | null = null; let busy = false; let error = ''; let replace = false;

  async function review() {
    busy = true; error = '';
    try { plan = await planCoreAdd(card, coreId, platform, destination, sources, replace); }
    catch (e) { plan = null; error = errorMessage(e); }
    finally { busy = false; }
  }
  async function add() {
    if (!plan || plan.blocked) return;
    busy = true; error = '';
    try { report = await executeCoreAdd(card, coreId, platform, destination, sources, replace, plan.id); dispatch('done', report); }
    catch (e) { error = `Nothing more was written. ${errorMessage(e)}`; }
    finally { busy = false; }
  }
  onMount(review);
  const stateText = (s: AddPlan['items'][number]['state']) => (typeof s === 'object' ? `Not added: ${s.Refused}` : s === 'New' ? 'New' : s === 'Identical' ? 'Already there, identical' : replace ? 'Different: will be replaced' : 'Different: kept as it is');
  $: toWrite = plan ? plan.new_files + (plan.replace ? plan.different : 0) : 0;
</script>
<div class="as-backdrop" role="presentation" on:click={() => !busy && dispatch('close')}></div>
<div class="as" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="as-title" use:modal on:keydown={(e) => e.key === 'Escape' && !busy && dispatch('close')}>
  <header class="as-head"><div><p class="eyebrow">ADD FILES</p><h2 id="as-title">{report ? 'Added' : `Add to ${coreName || coreId}`}</h2></div>
    <button class="quiet" data-autofocus disabled={busy} on:click={() => dispatch('close')}>{report ? 'Done' : 'Cancel'}</button></header>

  {#if report}
    <p class="as-ok" role="status">{plural(report.written + report.replaced, 'file')} added and verified ({size(report.bytes_written)}).
      {#if report.skipped_identical}{plural(report.skipped_identical, 'file')} already there.{/if}
      {#if report.skipped_different}{plural(report.skipped_different, 'different file')} kept as they were.{/if}
      {#if report.refused}{plural(report.refused, 'file')} not added.{/if}
      {#if report.backup_dir}<br />Replaced files were copied to <code>{report.backup_dir}</code> first.{/if}</p>
  {:else}
    <fieldset class="as-dest"><legend>Where should they go?</legend>
      <label><input type="radio" bind:group={destination} value="Shared" on:change={review} disabled={busy} /> <span><strong>Shared by every {platformName || platform} core</strong> <small>Assets/{platform}/common: where most cores look</small></span></label>
      <label><input type="radio" bind:group={destination} value="CoreOnly" on:change={review} disabled={busy} /> <span><strong>Only {coreName || coreId}</strong> <small>Assets/{platform}/{coreId}</small></span></label>
    </fieldset>
    {#if error}<p class="as-err" role="alert">{error}</p>{/if}
    {#if busy && !plan}<p class="as-note" role="status">Checking the files…</p>{/if}
    {#if plan}
      <p class="as-sum" role="status" aria-live="polite">{plural(plan.new_files, 'new file')}{plan.identical ? ` · ${plan.identical} already there` : ''}{plan.different ? ` · ${plural(plan.different, 'different file')}` : ''}{plan.refused ? ` · ${plan.refused} not added` : ''} · {size(plan.bytes_to_write)} to write into <code>{plan.folder_rel}</code></p>
      {#each plan.warnings as w}<p class="as-warn" role="note">{w}</p>{/each}
      {#if plan.different}
        <label class="as-replace"><input type="checkbox" bind:checked={replace} on:change={review} disabled={busy} /> Replace the {plural(plan.different, 'different file')} already there <small>(the old {plan.different === 1 ? 'one is' : 'ones are'} copied to a backup folder outside the card first)</small></label>
      {/if}
      <ul class="as-list" aria-label="Files">
        {#each plan.items as it}
          <li class:bad={typeof it.state === 'object'}><span class="as-name">{it.source.split('/').pop()}</span><span class="as-state">{stateText(it.state)}</span><span class="as-size">{size(it.bytes)}</span></li>
        {/each}
      </ul>
      {#if plan.blocked}<p class="as-note" role="status">{plan.blocked}</p>{/if}
      <p class="as-safe">Nothing is written until you press Add. Each file is copied to a temporary name, read back from the card and compared before it is put in place. Nothing is deleted.</p>
      <div class="as-actions"><button class="primary" disabled={busy || !!plan.blocked || !toWrite} on:click={add}>{busy ? 'Adding…' : `Add ${plural(toWrite, 'file')}`}</button></div>
    {/if}
  {/if}
</div>
<style>
  .as-backdrop{position:fixed;inset:0;background:var(--shadow);z-index:100}
  .as{position:fixed;z-index:101;top:50%;left:50%;transform:translate(-50%,-50%);width:min(720px,calc(100vw - 32px));max-height:calc(100vh - 48px);overflow:auto;background:var(--bg-card);border:1px solid var(--border);border-radius:14px;padding:22px 24px;box-sizing:border-box}
  .as-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px;margin-bottom:12px}.as-head h2{margin:0}
  .as-dest{border:0;padding:0;margin:0 0 12px;display:grid;gap:8px}.as-dest legend{font-size:12px;color:var(--text-secondary);font-weight:700;margin-bottom:6px;padding:0}
  .as-dest label{display:flex;gap:10px;align-items:flex-start;font-size:13px;color:var(--text);padding:8px 10px;border:1px solid var(--border);border-radius:9px;cursor:pointer}
  .as-dest label span{display:grid;gap:2px}.as-dest small,.as-replace small{font-size:11.5px;color:var(--text-muted)}
  .as-sum{font-size:13px;color:var(--text);margin:6px 0}.as-note{font-size:12px;color:var(--text-secondary);margin:8px 0}.as-err{color:var(--danger-text);font-size:12.5px}
  .as-warn{border:1px solid var(--warn-border);background:var(--warn-bg);color:var(--warn-text);border-radius:9px;padding:8px 11px;font-size:12px;margin:8px 0}
  .as-replace{display:block;font-size:12.5px;color:var(--text);margin:8px 0}
  .as-list{list-style:none;margin:10px 0;padding:0;border:1px solid var(--border);border-radius:9px;max-height:260px;overflow:auto}
  .as-list li{display:grid;grid-template-columns:1fr auto auto;gap:12px;padding:8px 12px;border-bottom:1px solid var(--border);font-size:12.5px;align-items:baseline}.as-list li:last-child{border:0}
  .as-list li.bad{color:var(--danger-text)}.as-name{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.as-state{color:var(--text-secondary);font-size:12px}.as-list li.bad .as-state{color:var(--danger-text)}.as-size{color:var(--text-muted);font-size:12px}
  .as-safe{font-size:11.5px;color:var(--text-muted);line-height:1.5;margin:8px 0}.as-actions{display:flex;justify-content:flex-end}.as-ok{font-size:13.5px;line-height:1.6;color:var(--text)}
  code{font-size:11.5px;background:var(--bg-input);border-radius:5px;padding:1px 5px}
</style>
