<script lang="ts">
  // Install several catalog cores in one reviewed step: each zip is downloaded and vetted in turn, then all of them go to the same install plan
  // (UpdatePlanView with `zips`). Nothing is written to the card before the plan is confirmed. A core that fails to download is left out and said so.
  import { onMount } from 'svelte';
  import UpdatePlanView from './UpdatePlanView.svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { size } from './marketplace';
  import { marketplaceDownload } from './tau-api';
  export let card = ''; export let cores: { id: string; name: string }[] = []; export let close: () => void; export let installed: () => void = () => {};

  type Row = { id: string; name: string; state: 'waiting' | 'downloading' | 'ok' | 'failed'; note: string; path: string };
  let rows: Row[] = cores.map((c) => ({ id: c.id, name: c.name, state: 'waiting', note: '', path: '' }));
  let reviewing = false; let done = false;
  $: ready = rows.filter((r) => r.state === 'ok');
  $: finished = rows.every((r) => r.state === 'ok' || r.state === 'failed');
  const cardName = (c: string) => c.split('/').filter(Boolean).pop() ?? c;

  onMount(async () => {
    for (const r of rows) {
      r.state = 'downloading'; rows = rows;
      try { const d = await marketplaceDownload(r.id, null); r.path = d.download.path; r.state = 'ok'; r.note = `${size(d.download.bytes)} · ${d.download.checksum === 'Verified' ? 'checksum verified' : 'no checksum published'}`; }
      catch (e) { r.state = 'failed'; r.note = errorMessage(e); }
      rows = rows;
    }
  });
  const onInstalled = () => { done = true; installed(); };
</script>
<svelte:window on:keydown={(e) => e.key === 'Escape' && close()} />
<div class="mi-backdrop" role="presentation" on:click={close}></div>
<div class="mi" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="mi-title" use:modal>
  <header class="mi-head"><div><p class="eyebrow">INSTALL TOGETHER</p><h2 id="mi-title">{cores.length} cores on {cardName(card)}</h2></div><button class="quiet" data-autofocus on:click={close}>{done ? 'Done' : 'Cancel'}</button></header>
  {#if !reviewing}
    <p class="mi-note">Omega downloads each core, checks it, and then shows one plan for all of them. Nothing is written yet.</p>
    <ul class="mi-list" aria-label="Cores to install">
      {#each rows as r}<li><b>{r.name}</b><span class:bad={r.state === 'failed'} class:ok={r.state === 'ok'}>{r.state === 'waiting' ? 'Waiting' : r.state === 'downloading' ? 'Downloading and checking…' : r.state === 'ok' ? `Ready · ${r.note}` : `Left out: ${r.note}`}</span></li>{/each}
    </ul>
    <div class="mi-actions"><button class="primary" disabled={!finished || !ready.length} on:click={() => (reviewing = true)}>{finished ? (ready.length ? `Review ${ready.length === 1 ? 'the install' : `${ready.length} installs`}` : 'Nothing could be downloaded') : 'Downloading…'}</button></div>
  {:else}
    <UpdatePlanView zips={ready.map((r) => r.path)} cardPath={card} on:installed={onInstalled} />
  {/if}
</div>
<style>
  .mi-backdrop{position:fixed;inset:0;background:rgba(0,0,0,.6);z-index:100}
  .mi{position:fixed;z-index:101;top:50%;left:50%;transform:translate(-50%,-50%);width:min(760px,calc(100vw - 32px));max-height:calc(100vh - 40px);overflow:auto;background:#1a2325;border:1px solid #2c393a;border-radius:14px;padding:20px 24px;color:#e8ecec}
  .mi-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px;margin-bottom:8px}.mi-head h2{margin:0}
  .mi-note{font-size:12.5px;color:#95a5a4;line-height:1.55}.mi-list{list-style:none;margin:10px 0;padding:0;border:1px solid #2c393a;border-radius:10px}
  .mi-list li{display:flex;justify-content:space-between;gap:12px;padding:9px 12px;border-bottom:1px solid #222e2f;font-size:13px}.mi-list li:last-child{border:0}
  .mi-list span{color:#95a5a4;text-align:right}.mi-list span.ok{color:#c5f7ad}.mi-list span.bad{color:#ff9b9b}
  .mi-actions{display:flex;justify-content:flex-end;margin-top:12px}
</style>
