<script lang="ts">
  import { onDestroy } from 'svelte';
  import { onFileDrop } from './dropTarget';
  import { openManualAdd } from './manualAddStore';
  import { onMount } from 'svelte';
  import CoreInstall from './CoreInstall.svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { marketplaceArt, marketplaceCatalog, marketplaceCoreUpdates } from './tau-api';
  import { categoryCounts, filterCores, size, sourceNote, statusLabel, statusMap, type Filters } from './marketplace';
  import { coreUpdateCount } from './marketStore';
  import type { CatalogCore, CatalogResult, CoreStatus } from './types';
  /** The card folder currently open (empty when none): used for the installed/update chips and for installing. */
  export let card = '';

  let result: CatalogResult | null = null; let loading = false; let error = '';
  let filters: Filters = { query: '', category: '', installedOnly: false, sort: 'newest' };
  let statuses = new Map<string, CoreStatus>();
  let searchEl: HTMLInputElement;
  let selected: CatalogCore | null = null;
  /** The version chosen in the history ('' = the newest). */
  let chosen = '';
  /** Pictures by `kind:id`: undefined = not asked yet, null = the library has none. Fetched only for rows that are shown. */
  let art: Record<string, string | null | undefined> = {}; const asking = new Set<string>(); let queue: string[] = [];
  let announce = '';

  $: cores = result?.catalog.cores ?? [];
  $: installed = new Set([...statuses.keys()]);
  $: shown = filterCores(cores, filters, installed);
  $: counts = categoryCounts(cores, filters, installed);
  $: note = result ? sourceNote(result.source, result.fetched) : null;
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

  function wantArt(kind: 'Icon' | 'Banner', id: string) {
    const key = `${kind}:${id}`;
    if (art[key] !== undefined || asking.has(key)) return;
    asking.add(key); queue = [...queue, key]; pump();
  }
  let running = 0;
  function pump() {
    while (running < 4 && queue.length) {
      const key = queue[0]; queue = queue.slice(1); running++;
      const [kind, ...rest] = key.split(':');
      marketplaceArt(rest.join(':'), kind as 'Icon' | 'Banner').then((url) => { art = { ...art, [key]: url }; }, () => { /* offline: the initials stay and it is asked again next time */ })
        .finally(() => { running--; asking.delete(key); pump(); });
    }
  }
  /** A card asks for its pictures only when it scrolls near the screen (the catalog has hundreds of cores). */
  function visible(node: HTMLElement, id: string) {
    if (typeof IntersectionObserver === 'undefined') { wantArt('Icon', id); wantArt('Banner', id); return {}; }
    const io = new IntersectionObserver((entries) => { if (entries.some((e) => e.isIntersecting)) { wantArt('Banner', id); wantArt('Icon', id); io.disconnect(); } }, { rootMargin: '300px' });
    io.observe(node);
    return { destroy: () => io.disconnect() };
  }
  const toggleCategory = (c: string) => (filters.category = filters.category === c ? '' : c);

  function open(core: CatalogCore) { selected = core; chosen = ''; wantArt('Banner', core.id); wantArt('Icon', core.id); }
  function close() { selected = null; }
  const repoUrl = (c: CatalogCore) => `https://github.com/${c.repository.owner}/${c.repository.name}`;
  let copied = false;
  async function copyRepo() { if (!selected) return; try { await navigator.clipboard.writeText(repoUrl(selected)); copied = true; setTimeout(() => (copied = false), 1500); } catch { /* the address stays visible to select */ } }
  /** Arrow keys move between cards: Left/Right by one, Up/Down by a row (the number of cards that share the first card's top edge). */
  function cardKey(e: KeyboardEvent) {
    if (!['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight'].includes(e.key)) return;
    const cards = [...document.querySelectorAll<HTMLElement>('.mk-open')];
    const at = cards.indexOf(e.currentTarget as HTMLElement);
    const columns = Math.max(1, cards.filter((c) => c.offsetTop === cards[0].offsetTop).length);
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : e.key === 'ArrowDown' ? columns : -columns;
    const next = cards[at + step];
    if (next) { e.preventDefault(); next.focus(); next.scrollIntoView({ block: 'nearest' }); }
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === '/' && !selected && document.activeElement?.tagName !== 'INPUT' && document.activeElement?.tagName !== 'SELECT') { e.preventDefault(); searchEl?.focus(); }
    if (e.key === 'Escape' && selected) { e.stopPropagation(); close(); }
  }
  const yesNo = (v: boolean | null) => (v === true ? 'yes' : v === false ? 'no' : 'not stated');
  const statusOf = (c: CatalogCore) => statuses.get(c.id.toLowerCase());
  const chip = (c: CatalogCore) => statusLabel(statusOf(c)?.status);
  // A zip or folder dropped on this page opens the manual-add sheet with it.
  const offDrop = onFileDrop((e) => { if (e.type === 'drop' && e.paths.length === 1) openManualAdd(e.paths[0]); });
  onDestroy(offDrop);
</script>
<svelte:window on:keydown={onKey} />
<section class="page" aria-labelledby="market-title">
  <header>
    <div>
      <p class="eyebrow">COMMUNITY CORES</p>
      <h1 id="market-title">Marketplace</h1>
      <p class="lede">Cores made by the community for the Analogue Pocket. Nothing is installed until you review it.</p>
    </div>
    <span class="mk-head-actions"><button class="quiet" on:click={() => openManualAdd()}>Add a core manually…</button><button class="quiet" disabled={loading} on:click={() => load(true)}>{loading ? 'Checking…' : 'Refresh'}</button></span>
  </header>

  <div class="mk-filters" role="search" aria-label="Filter cores">
    <label class="mk-search"><span class="sr">Search cores</span>
      <input bind:this={searchEl} type="search" placeholder="Search by name, author or system   ( / )" bind:value={filters.query} />
    </label>
    <label><span class="sr">Sort</span>
      <select bind:value={filters.sort}><option value="newest">Newest release</option><option value="name">Name</option></select>
    </label>
    <button class="chip toggle" aria-pressed={filters.installedOnly} disabled={!card} title={card ? '' : 'Open a card to filter by what it has'} on:click={() => (filters.installedOnly = !filters.installedOnly)}>On this card</button>
  </div>
  <div class="mk-tags" role="group" aria-label="Category">
    <button class="chip toggle" aria-pressed={!filters.category} on:click={() => (filters.category = '')}>All <span class="mk-count">{counts.all}</span></button>
    {#each counts.byCategory as [name, n]}<button class="chip toggle" aria-pressed={filters.category === name} on:click={() => toggleCategory(name)}>{name} <span class="mk-count">{n}</span></button>{/each}
  </div>
  <p class="mk-note" role="status" aria-live="polite">{#if error && result}<span class="err">{error}</span>{:else if error}{:else if loading && !result}Loading the list…{:else}{announce}{#if note}{' · '}{note}{/if}{/if}</p>

  {#if error && !result}
    <div class="empty"><h2>The list could not be loaded</h2><p>{error}</p><button class="primary" on:click={() => load(true)}>Try again</button></div>
  {:else if result && !shown.length}
    <div class="empty"><h2>No cores match</h2><p>{filters.query ? `Nothing matches “${filters.query}”` : 'Nothing matches these filters'}{filters.category ? ` in ${filters.category}` : ''}{filters.installedOnly ? ' on this card' : ''}.</p>
      <button class="quiet" on:click={() => (filters = { ...filters, query: '', category: '', installedOnly: false })}>Clear filters</button></div>
  {:else if result}
    <ul class="mk-grid" aria-busy={loading} aria-label="Cores">
      {#each shown as item (item.id)}
        <li class="mk-card" use:visible={item.id}>
          <button class="mk-open" on:keydown={cardKey} aria-label={`${item.name}, ${item.category}, version ${item.version || 'unknown'}. Show details`} on:click={() => open(item)}>
            <span class="mk-art" aria-hidden="true">
              {#if art[`Banner:${item.id}`]}<img src={art[`Banner:${item.id}`]} alt="" />{:else}<span class="mk-art-fallback">{item.name.slice(0, 2).toUpperCase()}</span>{/if}
            </span>
            <span class="mk-body">
              <strong class="mk-title">{item.name}</strong>
              <small class="mk-sub">{item.manufacturer || item.category}{item.year ? ` · ${item.year}` : ''}</small>
              <span class="mk-author">
                <span class="mk-avatar" aria-hidden="true">{#if art[`Icon:${item.id}`]}<img class="mk-icon" src={art[`Icon:${item.id}`]} alt="" width="28" height="28" />{:else}{item.author.slice(0, 1).toUpperCase()}{/if}</span>
                <span class="mk-author-name">{item.author}</span>
              </span>
            </span>
          </button>
          <div class="mk-foot">
            {#if item.category}<button class="chip toggle mk-tag" aria-pressed={filters.category === item.category} aria-label={`Show only ${item.category}`} on:click={() => toggleCategory(item.category)}>{item.category}</button>{/if}
            {#if chip(item)}<span class="chip" class:capable={statusOf(item)?.status === 'UpdateAvailable'}>{chip(item)}</span>{/if}
            <span class="mk-ver">{item.version || '—'}<small>{item.release_date}</small></span>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

{#if selected}
  <div class="mk-backdrop" role="presentation" on:click={close}></div>
  <div class="mk-sheet" role="dialog" aria-modal="true" aria-labelledby="mk-title" use:modal>
    {#if art[`Banner:${selected.id}`]}<img class="mk-banner" src={art[`Banner:${selected.id}`]} alt={`${selected.name} platform artwork`} />{/if}
    <header class="mk-sheet-head">
      <div><p class="eyebrow">{selected.category.toUpperCase()}</p><h2 id="mk-title">{selected.name}</h2><p class="mk-by">{selected.id}{selected.manufacturer ? ` · ${selected.manufacturer}` : ''}{selected.year ? ` · ${selected.year}` : ''}</p></div>
      <button class="quiet" data-autofocus on:click={close} aria-label="Close details">Close</button>
    </header>

    <dl class="mk-facts">
      <div><dt>Newest version</dt><dd>{selected.version || 'none published'}{selected.release_date ? ` · ${selected.release_date}` : ''}</dd></div>
      <div><dt>Source</dt><dd><code>{repoUrl(selected)}</code> <button class="quiet" on:click={copyRepo}>{copied ? 'Copied' : 'Copy link'}</button></dd></div>
      {#if statusOf(selected) && chip(selected)}<div><dt>On your card</dt><dd>{statusOf(selected)?.installed_version} · {chip(selected)}</dd></div>{/if}
    </dl>

    {#if selected.sponsor.links.length}
      <p class="mk-small">Support the author: {#each selected.sponsor.links as l, i}<code>{l.replace('https://', '')}</code>{i < selected.sponsor.links.length - 1 ? ', ' : ''}{/each}</p>
    {/if}

    <h3>Requirements</h3>
    <dl class="mk-reqs">
      <div><dt>Pocket firmware</dt><dd>{selected.firmware_required ? `${selected.firmware_required} or newer` : 'not stated'}</dd></div>
      <div><dt>Dock</dt><dd>{yesNo(selected.dock_supported)}</dd></div>
      <div><dt>Link port</dt><dd>{yesNo(selected.link_port)}</dd></div>
      <div><dt>Sleep</dt><dd>{yesNo(selected.sleep_supported)}</dd></div>
      {#if selected.requires_license}
        <div class="mk-req-key"><dt>Key file</dt><dd><code>{selected.license_filename || 'a file the author names'}</code> is not in the download: get it from the author (see Source above) and put it in <code>Cores/{selected.id}/</code> on the card. The core will not run without it.</dd></div>
      {/if}
      {#if selected.assets.length}
        <div><dt>Your own files</dt><dd>{#each selected.assets as a, i}<code>Assets/{a.platform}/{a.core_specific ? selected.id : 'common'}</code>{#if a.extensions.length} ({a.extensions.map((x) => '.' + x).join(' ')}){/if}{i < selected.assets.length - 1 ? ', ' : ''}{/each}</dd></div>
      {/if}
    </dl>
    {#if selected.declares_ai.length}<p class="mk-callout soft" role="note">The author says they used AI tools to build this core: {selected.declares_ai.join(', ')}.</p>{/if}
    {#if selected.history.length > 1}
      <details class="mk-history"><summary>Earlier versions ({selected.history.length - 1})</summary>
        <ul>{#each selected.history.slice(1) as h}<li><span>{h.version} · {h.release_date}</span>{#if card && selected.download_url}<button class="quiet" aria-pressed={chosen === h.version} on:click={() => (chosen = chosen === h.version ? '' : h.version)}>{chosen === h.version ? 'Use the newest instead' : 'Choose this version'}</button>{/if}</li>{/each}</ul>
        <p class="mk-small">An older version replaces a newer one on the card only if you confirm the downgrade in the plan. Your files are kept, as with any update.</p>
      </details>
    {/if}

    <h3>Install</h3>
    {#if !card}
      <p class="mk-small">Open a card first (Cards page) to install this core.</p>
    {:else if !selected.download_url}
      <p class="mk-small">The author has not published a download yet.</p>
    {:else}
      {#key chosen}<CoreInstall coreId={selected.id} {card} version={chosen} label={chosen ? `Download version ${chosen} and review` : 'Download and review'} on:installed={() => loadStatuses(card)} />{/key}
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
  .mk-tags{display:flex;flex-wrap:wrap;gap:8px;margin:0 0 10px}
  .mk-count{opacity:.65;font-weight:500;margin-left:4px}
  .mk-grid{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fill,minmax(270px,1fr));gap:16px}
  .mk-card{display:flex;flex-direction:column;background:#161f20;border:1px solid #2c393a;border-radius:12px;overflow:hidden}
  .mk-card:hover{border-color:#3d5254}
  .mk-open{display:flex;flex-direction:column;flex:1;background:transparent;color:inherit;text-align:left;padding:0;border-radius:0}
  .mk-open:focus-visible{outline:2px solid #c1f0ad;outline-offset:-2px}
  .mk-art{display:block;width:100%;aspect-ratio:521/165;background:#000;overflow:hidden}
  .mk-art img{display:block;width:100%;height:100%;object-fit:cover}
  .mk-art-fallback{display:grid;place-items:center;height:100%;color:#c1f0ad;font-weight:700;font-size:22px;letter-spacing:.06em}
  .mk-body{display:flex;flex-direction:column;gap:4px;padding:12px 14px 10px}
  .mk-title{font-size:15px;line-height:1.3;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .mk-sub{font-size:12px;color:#95a5a4;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .mk-author{display:flex;align-items:center;gap:8px;margin-top:6px;min-width:0}
  .mk-avatar{flex:none;width:28px;height:28px;display:grid;place-items:center;border-radius:7px;background:#273638;color:#c1f0ad;font-weight:700;font-size:12px;overflow:hidden}
  .mk-author-name{font-size:12.5px;color:#c7d1d0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .mk-foot{display:flex;flex-wrap:wrap;align-items:center;gap:6px;padding:9px 14px 12px;border-top:1px solid #2c393a;background:#141c1d}
  .mk-tag{padding:4px 10px;font-size:11px}
  .mk-ver{margin-left:auto;font-size:12px;color:#c7d1d0;display:flex;flex-direction:column;align-items:flex-end;line-height:1.3}.mk-ver small{font-size:11px;color:#95a5a4}
  .mk-icon{display:block;width:28px;height:28px;border-radius:7px;image-rendering:pixelated}
  .mk-banner{display:block;width:100%;height:auto;border-radius:10px;margin:0 0 16px;background:#000}
  .mk-reqs{display:grid;gap:8px;margin:6px 0 4px}.mk-reqs>div{display:grid;grid-template-columns:130px 1fr;gap:10px;align-items:baseline}.mk-reqs dt{font-size:12px;color:#95a5a4}.mk-reqs dd{margin:0;font-size:12.5px;color:#dce6e4;overflow-wrap:anywhere;line-height:1.5}
  .mk-req-key{border:1px solid #5c5030;background:#241f12;border-radius:9px;padding:9px 11px}.mk-req-key dt{color:#e5d9a8;font-weight:700}
  .mk-history{margin:10px 0;font-size:12px;color:#a6b3b2}.mk-history summary{cursor:pointer}.mk-history ul{margin:8px 0;padding:0;list-style:none}.mk-history li{display:flex;justify-content:space-between;align-items:center;gap:10px;padding:4px 0}
  .mk-backdrop{position:fixed;inset:0;background:rgba(0,0,0,.5);z-index:100}
  .mk-sheet{position:fixed;top:0;right:0;bottom:0;width:min(560px,100%);overflow:auto;z-index:101;background:#1a2325;border-left:1px solid #2c393a;padding:26px 28px 40px;box-sizing:border-box}
  .mk-sheet h3{font-size:14px;margin:22px 0 8px}
  .mk-sheet-head{display:flex;justify-content:space-between;gap:16px;align-items:flex-start;margin-bottom:14px}.mk-sheet-head h2{margin:0}.mk-by{color:#95a5a4;font-size:12px;margin:4px 0 0}
  .mk-facts{display:grid;gap:10px;margin:0 0 14px}.mk-facts div{display:grid;gap:2px}.mk-facts dt{font-size:11px;letter-spacing:.08em;color:#9fb3aa;font-weight:700}.mk-facts dd{margin:0;font-size:13px;color:#dce6e4;overflow-wrap:anywhere}
  .mk-small{font-size:12px;color:#a6b3b2;line-height:1.55;margin:6px 0}
  .mk-callout{border:1px solid #3b5a58;background:#16292a;border-radius:9px;padding:10px 12px;font-size:12px;line-height:1.5;color:#dce6e4;margin:10px 0}.mk-callout.soft{border-color:#5c5030;background:#241f12;color:#e5d9a8}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
  @media(max-width:720px){.mk-reqs>div{grid-template-columns:1fr;gap:2px}.mk-grid{grid-template-columns:1fr}.mk-sheet{padding:20px 16px 32px}}
</style>
