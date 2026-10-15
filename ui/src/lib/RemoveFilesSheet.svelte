<script lang="ts">
  // Review before anything is deleted: which files, who else reads them, that each is copied away first. Then Remove.
  import { createEventDispatcher, onMount } from 'svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { executeCoreRemove, planCoreRemove } from './tau-api';
  import { plural, size } from './coresFormat';
  import type { RemoveFilesPlan, RemoveFilesReport } from './types';
  export let card: string; export let coreId: string; export let coreName = ''; export let files: string[];
  const dispatch = createEventDispatcher<{ close: void; done: RemoveFilesReport }>();

  let plan: RemoveFilesPlan | null = null; let report: RemoveFilesReport | null = null; let busy = false; let error = '';
  onMount(async () => {
    busy = true;
    try { plan = await planCoreRemove(card, coreId, files); } catch (e) { error = errorMessage(e); } finally { busy = false; }
  });
  async function remove() {
    if (!plan || plan.blocked) return;
    busy = true; error = '';
    try { report = await executeCoreRemove(card, coreId, files, plan.id); dispatch('done', report); }
    catch (e) { error = `Nothing more was removed. ${errorMessage(e)}`; }
    finally { busy = false; }
  }
</script>
<div class="rs-backdrop" role="presentation" on:click={() => !busy && dispatch('close')}></div>
<div class="rs" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="rs-title" use:modal on:keydown={(e) => e.key === 'Escape' && !busy && dispatch('close')}>
  <header class="rs-head"><div><p class="eyebrow">REMOVE FILES</p><h2 id="rs-title">{report ? 'Removed' : `Remove from ${coreName || coreId}`}</h2></div>
    <button class="quiet" data-autofocus disabled={busy} on:click={() => dispatch('close')}>{report ? 'Done' : 'Cancel'}</button></header>
  {#if report}
    <p class="rs-ok" role="status">{plural(report.removed, 'file')} removed ({size(report.bytes_removed)}). Copies are kept in <code>{report.backup_dir}</code> in case you need them back.</p>
  {:else}
    {#if error}<p class="rs-err" role="alert">{error}</p>{/if}
    {#if busy && !plan}<p class="rs-note" role="status">Checking the files…</p>{/if}
    {#if plan}
      <p class="rs-sum" role="status" aria-live="polite">{plural(plan.items.length, 'file')} · {size(plan.total_bytes)} to remove from the card</p>
      {#each plan.warnings as w}<p class="rs-warn" role="note">{w}</p>{/each}
      <ul class="rs-list" aria-label="Files to remove">
        {#each plan.items as it}
          <li><span class="rs-name" title={it.rel_path}>{it.rel_path}</span>
            {#if it.needed_by}<span class="chip warn">needed: {it.needed_by}</span>{/if}
            {#if it.shared_with.length}<span class="chip" title={it.shared_with.join(', ')}>also read by {plural(it.shared_with.length, 'other core')}</span>{/if}
            <span class="rs-size">{size(it.bytes)}</span></li>
        {/each}
      </ul>
      {#if plan.blocked}<p class="rs-note" role="status">{plan.blocked}</p>{/if}
      <p class="rs-safe">Nothing is removed until you press Remove. Each file is first copied to a backup folder outside the card and checked; if a copy does not verify, that file stays.</p>
      <div class="rs-actions"><button class="danger" disabled={busy || !!plan.blocked} on:click={remove}>{busy ? 'Removing…' : `Remove ${plural(plan.items.length, 'file')}`}</button></div>
    {/if}
  {/if}
</div>
<style>
  .rs-backdrop{position:fixed;inset:0;background:rgba(0,0,0,.55);z-index:100}
  .rs{position:fixed;z-index:101;top:50%;left:50%;transform:translate(-50%,-50%);width:min(680px,calc(100vw - 32px));max-height:calc(100vh - 48px);overflow:auto;background:#1a2325;border:1px solid #2c393a;border-radius:14px;padding:22px 24px;box-sizing:border-box}
  .rs-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px;margin-bottom:12px}.rs-head h2{margin:0}
  .rs-sum{font-size:13px;color:#dce6e4;margin:6px 0}.rs-note{font-size:12px;color:#c1d7d4;margin:8px 0}.rs-err{color:#ff9b9b;font-size:12.5px}
  .rs-warn{border:1px solid #5c5030;background:#241f12;color:#e5d9a8;border-radius:9px;padding:8px 11px;font-size:12px;margin:8px 0}
  .rs-list{list-style:none;margin:10px 0;padding:0;border:1px solid #2c393a;border-radius:9px;max-height:260px;overflow:auto}
  .rs-list li{display:flex;gap:8px;align-items:center;padding:8px 12px;border-bottom:1px solid #2c393a;font-size:12.5px}.rs-list li:last-child{border:0}
  .rs-name{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.rs-size{color:#8f9e9d;font-size:12px;flex:none}.chip.warn{background:#3a2e14;color:#e5d9a8}
.rs-actions .danger{background:#ff9b9b;color:#2a1212;font-weight:700;border-radius:9px;padding:10px 14px}.rs-actions .danger:disabled{opacity:.5}
  .rs-safe{font-size:11.5px;color:#8f9e9d;line-height:1.5;margin:8px 0}.rs-actions{display:flex;justify-content:flex-end}.rs-ok{font-size:13.5px;line-height:1.6;color:#dce6e4}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px;overflow-wrap:anywhere}
</style>
