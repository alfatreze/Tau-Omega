<script lang="ts">
  import { onMount } from 'svelte';
  import VirtualList from './VirtualList.svelte';
  import CoreInstall from './CoreInstall.svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { marketplaceCatalog, marketplaceCoreUpdates, marketplaceRelease } from './tau-api';
  import { categories, filterCores, size, sourceNote, statusLabel, statusMap, type Filters } from './marketplace';
  import { coreUpdateCount } from './marketStore';
  import type { CatalogCore, CatalogResult, CoreStatus, ReleaseResult } from './types';
  /** The card folder currently open (empty when none): used for the installed/update chips and for installing. */
  export let card = '';

  let result: CatalogResult | null = null; let loading = false; let error = '';
  let filters: Filters = { query: '', category: '', licence: false, installedOnly: false, sort: 'newest' };
  let statuses = new Map<string, CoreStatus>();
  let searchEl: HTMLInputElement;
  let selected: CatalogCore | null = null;
  let release: ReleaseResult | null = null; let releaseBusy = false; let releaseError = '';
  let announce = '';

  $: cores = result?.catalog.cores ?? [];
  $: installed = new Set([...statuses.keys()]);
  $: shown = filterCores(cores, filters, installed);
  $: cats = categories(cores);
  $: note = result ? sourceNote(result.source, result.fetched) : null;
  $: resetKey = `${filters.query}|${filters.category}|${filters.licence}|${filters.installedOnly}|${filters.sort}`;
  $: if (result) announce = `${shown.length} of ${cores.length} cores`;
  $: void loadStatuses(card);

  async function load(force = false) {
    loading = true; error = '';
    try { result = await marketplaceCatalog(force); }
    catch (e) { error = errorMessage(e); }
    finally { loading = false; }
  }
  async function loadStatuses(path: string) {
    if (!path) { statuses = new Map(); return; }
    try { const rows = (await marketplaceCoreUpdates(path, false)).rows; statuses = statusMap(rows); coreUpdateCount.set(rows.filter((r) => r.status === 'UpdateAvailable').length); } catch { statuses = new Map(); }
  }
  onMount(() => { void load(false); });

  async function open(core: CatalogCore) {
    selected = core; release = null; releaseError = '';
    releaseBusy = true;
    try { release = await marketplaceRelease(core.id, false); }
    catch (e) { releaseError = errorMessage(e); }
    finally { releaseBusy = false; }
  }
  function close() { selected = null; }
  const repoUrl = (c: CatalogCore) => `https://github.com/${c.repository.owner}/${c.repository.name}`;
  let copied = false;
  async function copyRepo() { if (!selected) return; try { await navigator.clipboard.writeText(repoUrl(selected)); copied = true; setTimeout(() => (copied = false), 1500); } catch { /* the address stays visible to select */ } }
  function onKey(e: KeyboardEvent) {
    if (e.key === '/' && !selected && document.activeElement?.tagName !== 'INPUT' && document.activeElement?.tagName !== 'SELECT') { e.preventDefault(); searchEl?.focus(); }
    if (e.key === 'Escape' && selected) { e.stopPropagation(); close(); }
  }
  const statusOf = (c: CatalogCore) => statuses.get(c.id.toLowerCase());
  const chip = (c: CatalogCore) => statusLabel(statusOf(c)?.status);
</script>
<svelte:window on:keydown={onKey} />
<section class="page" aria-labelledby="market-title">
  <header>
    <div>
      <p class="eyebrow">COMMUNITY CORES</p>
      <h1 id="market-title">Marketplace</h1>
      <p class="lede">Cores made by the community for the Analogue Pocket. Nothing is installed until you review it.</p>
    </div>
    <button class="quiet" disabled={loading} on:click={() => load(true)}>{loading ? 'Checking…' : 'Refresh'}</button>
  </header>

  <div class="mk-filters" role="search" aria-label="Filter cores">
    <label class="mk-search"><span class="sr">Search cores</span>
      <input bind:this={searchEl} type="search" placeholder="Search by name, author or system   ( / )" bind:value={filters.query} />
    </label>
    <label><span class="sr">Category</span>
      <select bind:value={filters.category}><option value="">All categories</option>{#each cats as c}<option value={c}>{c}</option>{/each}</select>
    </label>
    <label><span class="sr">Sort</span>
      <select bind:value={filters.sort}><option value="newest">Newest release</option><option value="name">Name</option></select>
    </label>
    <button class="chip toggle" aria-pressed={filters.licence} on:click={() => (filters.licence = !filters.licence)}>Needs a licence file</button>
    <button class="chip toggle" aria-pressed={filters.installedOnly} disabled={!card} title={card ? '' : 'Open a card to filter by what it has'} on:click={() => (filters.installedOnly = !filters.installedOnly)}>On this card</button>
  </div>
  <p class="mk-note" role="status" aria-live="polite">{#if error}<span class="err">{error}</span>{:else if loading && !result}Loading the list…{:else}{announce}{#if note} · {note}{/if}{/if}</p>

  {#if error && !result}
    <div class="empty"><h2>The list could not be loaded</h2><p>{error}</p><button class="primary" on:click={() => load(true)}>Try again</button></div>
  {:else if result && !shown.length}
    <div class="empty"><h2>No cores match</h2><p>{filters.query ? `Nothing matches “${filters.query}”` : 'Nothing matches these filters'}{filters.category ? ` in ${filters.category}` : ''}{filters.licence ? ' that need a licence file' : ''}{filters.installedOnly ? ' on this card' : ''}.</p>
      <button class="quiet" on:click={() => (filters = { ...filters, query: '', category: '', licence: false, installedOnly: false })}>Clear filters</button></div>
  {:else if result}
    <div class="mk-list">
      <VirtualList items={shown} rowHeight={68} key={(c) => c.id} label="Cores" {resetKey} let:item>
        <div class="mk-row" role="listitem">
          <button class="mk-open" aria-label={`${item.name}, ${item.category}, version ${item.version || 'unknown'}. Show details`} on:click={() => open(item)}>
            <span class="core-icon" aria-hidden="true">{item.name.slice(0, 2).toUpperCase()}</span>
            <span class="mk-main"><strong>{item.name}</strong><small>{item.author} · {item.manufacturer || item.category}{item.year ? ` · ${item.year}` : ''}</small></span>
            <span class="mk-ver"><span>{item.version || '—'}</span><small>{item.release_date}</small></span>
          </button>
          {#if item.requires_license}<span class="chip">Needs a licence file</span>{/if}
          {#if chip(item)}<span class="chip" class:capable={statusOf(item)?.status === 'UpdateAvailable'}>{chip(item)}</span>{/if}
        </div>
      </VirtualList>
    </div>
  {/if}
</section>

{#if selected}
  <div class="mk-backdrop" role="presentation" on:click={close}></div>
  <div class="mk-sheet" role="dialog" aria-modal="true" aria-labelledby="mk-title" use:modal>
    <header class="mk-sheet-head">
      <div><p class="eyebrow">{selected.category.toUpperCase()}</p><h2 id="mk-title">{selected.name}</h2><p class="mk-by">{selected.id}{selected.manufacturer ? ` · ${selected.manufacturer}` : ''}{selected.year ? ` · ${selected.year}` : ''}</p></div>
      <button class="quiet" data-autofocus on:click={close} aria-label="Close details">Close</button>
    </header>

    <dl class="mk-facts">
      <div><dt>Latest in the list</dt><dd>{selected.version || 'unknown'} · {selected.release_date || 'no date'}</dd></div>
      <div><dt>Source</dt><dd><code>{repoUrl(selected)}</code> <button class="quiet" on:click={copyRepo}>{copied ? 'Copied' : 'Copy link'}</button></dd></div>
      {#if statusOf(selected) && chip(selected)}<div><dt>On your card</dt><dd>{statusOf(selected)?.installed_version} · {chip(selected)}</dd></div>{/if}
    </dl>

    {#if selected.requires_license}
      <p class="mk-callout" role="note"><strong>You will need a licence file.</strong> This core does not include it. After installing, put <code>{selected.license_filename || 'the licence file the author names'}</code> in <code>Cores/{selected.id}/</code> on the card.</p>
    {/if}
    {#if selected.assets.length}
      <p class="mk-small">Put your own files in {#each selected.assets as a, i}<code>Assets/{a.platform}/{a.core_specific ? selected.id : 'common'}</code>{#if a.extensions.length} ({a.extensions.map((x) => '.' + x).join(' ')}){/if}{i < selected.assets.length - 1 ? ', ' : ''}{/each}.</p>
    {/if}
    {#if selected.sponsor.links.length}
      <p class="mk-small">Support the author: {#each selected.sponsor.links as l, i}<code>{l.replace('https://', '')}</code>{i < selected.sponsor.links.length - 1 ? ', ' : ''}{/each}</p>
    {/if}

    <h3>Newest release</h3>
    {#if releaseBusy}<p class="mk-small" role="status">Looking up the release…</p>
    {:else if releaseError}<p class="err" role="alert">{releaseError}</p>
    {:else if release}
      <p class="mk-small"><strong>{release.release.tag}</strong> · {release.release.published}{release.release.prerelease ? ' · pre-release' : ''} · {release.release.download.name} ({size(release.release.download.size)})</p>
      {#if release.release.notes}<p class="mk-notes">{release.release.notes}</p>{/if}
      {#if !release.release.download.sha256}<p class="mk-callout soft" role="note">No checksum published for this release. The file is what GitHub serves, nothing more can be verified.</p>{/if}
      {#if typeof release.source === 'object'}<p class="mk-small">Saved copy shown. {release.source.Offline}</p>{/if}
    {/if}

    <h3>Install</h3>
    {#if !card}
      <p class="mk-small">Open a card first (Cards page) to install this core.</p>
    {:else if !release}
      <p class="mk-small">Waiting for the release.</p>
    {:else}
      <CoreInstall coreId={selected.id} {card} on:installed={() => loadStatuses(card)} />
    {/if}
  </div>
{/if}
<style>
  .sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
  .mk-filters{display:flex;flex-wrap:wrap;gap:10px;align-items:center;margin:0 0 8px}
  .mk-filters input,.mk-filters select{font:inherit;font-size:13px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:10px 12px}
  .mk-search{flex:1 1 280px}.mk-search input{width:100%}
  .chip.toggle{cursor:pointer;border:1px solid transparent;padding:8px 12px;font-size:12px}
  .chip.toggle[aria-pressed="true"]{border-color:#c1f0ad;color:#c5f7ad;background:#16292a}
  .chip.toggle:disabled{opacity:.45;cursor:not-allowed}
  .mk-note{font-size:12px;color:#95a5a4;min-height:18px;margin:0 0 10px}
  .err{color:#ff9b9b}
  .mk-list{display:flex;flex-direction:column;height:min(62vh,640px);border:1px solid #2c393a;border-radius:12px;overflow:hidden;background:#161f20}
  .mk-row{height:68px;display:flex;align-items:center;gap:10px;padding:0 12px 0 0;border-bottom:1px solid #2c393a;box-sizing:border-box}
  .mk-open{flex:1;min-width:0;height:100%;display:flex;align-items:center;gap:13px;padding:0 0 0 16px;background:transparent;color:inherit;text-align:left}
  .mk-open:hover{background:#1c282a}
  .mk-open:focus-visible{outline:2px solid #c1f0ad;outline-offset:-2px}
  .mk-main{flex:1;min-width:0;display:grid;gap:3px}.mk-main strong{font-size:14px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.mk-main small,.mk-ver small{font-size:12px;color:#95a5a4;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .mk-ver{display:grid;gap:3px;text-align:right;font-size:13px;color:#c7d1d0;max-width:130px}
  .mk-backdrop{position:fixed;inset:0;background:rgba(0,0,0,.5);z-index:100}
  .mk-sheet{position:fixed;top:0;right:0;bottom:0;width:min(560px,100%);overflow:auto;z-index:101;background:#1a2325;border-left:1px solid #2c393a;padding:26px 28px 40px;box-sizing:border-box}
  .mk-sheet h3{font-size:14px;margin:22px 0 8px}
  .mk-sheet-head{display:flex;justify-content:space-between;gap:16px;align-items:flex-start;margin-bottom:14px}.mk-sheet-head h2{margin:0}.mk-by{color:#95a5a4;font-size:12px;margin:4px 0 0}
  .mk-facts{display:grid;gap:10px;margin:0 0 14px}.mk-facts div{display:grid;gap:2px}.mk-facts dt{font-size:11px;letter-spacing:.08em;color:#9fb3aa;font-weight:700}.mk-facts dd{margin:0;font-size:13px;color:#dce6e4;overflow-wrap:anywhere}
  .mk-small{font-size:12px;color:#a6b3b2;line-height:1.55;margin:6px 0}.mk-notes{font-size:12px;color:#c7d1d0;white-space:pre-wrap;border-left:2px solid #344244;padding-left:10px;margin:8px 0}
  .mk-callout{border:1px solid #3b5a58;background:#16292a;border-radius:9px;padding:10px 12px;font-size:12px;line-height:1.5;color:#dce6e4;margin:10px 0}.mk-callout.soft{border-color:#5c5030;background:#241f12;color:#e5d9a8}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
  @media(max-width:720px){.mk-ver{display:none}.mk-sheet{padding:20px 16px 32px}}
</style>
