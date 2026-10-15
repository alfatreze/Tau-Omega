<script lang="ts">
  // The Cores page. Three views, chosen by the side-menu tree: all platforms (overview), one platform (its cores), one core (tabs + details).
  // A core opens on Files; the right-hand details panel can be hidden and the choice is remembered.
  import { onDestroy, tick } from 'svelte';
  import CoreFilesTab from './CoreFilesTab.svelte';
  import CoreInspector from './CoreInspector.svelte';
  import CoreSavesTab from './CoreSavesTab.svelte';
  import CoreScreenshotsTab from './CoreScreenshotsTab.svelte';
  import CoreSettingsTab from './CoreSettingsTab.svelte';
  import { errorMessage } from './backend';
  import { coreDetail, coreFiles, readPlatformImage } from './tau-api';
  import CoreUpdates from './CoreUpdates.svelte';
  import Icon from './Icon.svelte';
  import { categoryIcon } from './iconFor';
  import { coreUpdateCount } from './marketStore';
  import { coresGroups, coresSelection, coresTab, coresTreeStore, findCore, inspectorOpen, refreshCores } from './coresStore';
  import { plural } from './coresFormat';
  import type { Core, CoreDetail, CoreFiles } from './types';
  export let card = '';
  /** The card-level core records (library facts) and what to do with them. */
  export let cores: Core[] = []; export let openLibrary: (c: Core) => void = () => {}; export let reloadCard: () => void = () => {};

  type Tab = 'files' | 'saves' | 'screens' | 'settings';
  const TABS: [Tab, string][] = [['files', 'Files'], ['saves', 'Saves'], ['screens', 'Screenshots'], ['settings', 'Settings']];
  let tab: Tab = 'files';
  let detail: CoreDetail | null = null; let files: CoreFiles | null = null; let error = ''; let loadedFor = '';
  let filesTab: CoreFilesTab | undefined; let banner: string | null = null; let bannerFor = '';

  $: node = $coresTreeStore.find((n) => n.platform.id === $coresSelection.platform);
  $: row = $coresSelection.core ? findCore($coresTreeStore, $coresSelection.core)?.core : undefined;
  $: if (card && $coresSelection.core && `${card}|${$coresSelection.core}` !== loadedFor) void load($coresSelection.core);
  $: if (!$coresSelection.core) { detail = null; files = null; loadedFor = ''; }
  $: if (card && node && node.platform.has_image && bannerFor !== `${card}|${node.platform.id}`) { bannerFor = `${card}|${node.platform.id}`; banner = null; readPlatformImage(card, node.platform.id).then((u) => (banner = u)).catch(() => (banner = null)); }

  async function load(id: string) {
    loadedFor = `${card}|${id}`; error = ''; detail = null; files = null; tab = 'files';
    try { [detail, files] = await Promise.all([coreDetail(card, id), coreFiles(card, id)]); }
    catch (e) { error = errorMessage(e); }
  }
  async function reload() { if (!$coresSelection.core) return; try { files = await coreFiles(card, $coresSelection.core); void refreshCores(card); } catch (e) { error = errorMessage(e); } }
  async function addMissing(folderRel: string) { tab = 'files'; await tick(); await filesTab?.chooseFor(folderRel); }

  function tabKey(e: KeyboardEvent) {
    const i = TABS.findIndex(([t]) => t === tab);
    const next = e.key === 'ArrowRight' ? (i + 1) % TABS.length : e.key === 'ArrowLeft' ? (i + TABS.length - 1) % TABS.length : e.key === 'Home' ? 0 : e.key === 'End' ? TABS.length - 1 : -1;
    if (next < 0) return;
    e.preventDefault(); tab = TABS[next][0];
    tick().then(() => document.getElementById(`cores-tab-${tab}`)?.focus());
  }
  function onKey(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    if (e.key.toLowerCase() === 'i' && !e.metaKey && !e.ctrlKey && !e.altKey && $coresSelection.core && t?.tagName !== 'INPUT' && t?.tagName !== 'SELECT' && t?.tagName !== 'TEXTAREA') { e.preventDefault(); inspectorOpen.update((v) => !v); }
  }
  // A category page (`c:Console`) or a manufacturer page (`m:Console/Nintendo`), chosen in the menu.
  $: grp = (() => {
    const key = $coresSelection.group ?? ''; if (!key || $coresSelection.platform || $coresSelection.core) return null;
    const [kind, rest] = [key.slice(0, 1), key.slice(2)]; const cat = kind === 'c' ? rest : rest.split('/')[0]; const maker = kind === 'm' ? rest.slice(cat.length + 1) : '';
    const g = $coresGroups.find((x) => x.name === cat); if (!g) return null;
    const makers = maker ? g.makers.filter((m) => m.name === maker) : g.makers;
    return makers.length ? { cat: g.name, maker, makers, cores: makers.reduce((n, m) => n + m.cores, 0), platforms: makers.reduce((n, m) => n + m.platforms.length, 0) } : null;
  })();
  // Search lives here, on the Cores page: cores, platforms, manufacturers and categories.
  let query = '';
  $: q = query.trim().toLowerCase();
  $: hits = q ? $coresGroups.flatMap((g) => g.makers.flatMap((m) => m.platforms.flatMap((n) => n.cores
    .filter((c) => `${c.id} ${c.shortname} ${c.author} ${n.platform.name} ${m.name} ${g.name}`.toLowerCase().includes(q) || n.platform.name.toLowerCase().includes(q))
    .map((c) => ({ core: c, platform: n.platform, category: g.name, maker: m.name }))))) : [];
  const pickPlatform = (id: string) => coresSelection.set({ platform: id, core: '' });
  const pickCore = (platform: string, core: string) => coresSelection.set({ platform, core });
  onDestroy(() => {});
</script>
<svelte:window on:keydown={onKey} />
<section class="page cores" aria-labelledby="cores-title">
  {#if !card}
    <header><div><p class="eyebrow">CORES</p><h1 id="cores-title">Cores</h1><p class="lede">Open a card to see the cores on it, organised by platform.</p></div></header>
  {:else if $coresSelection.core}
    <header class="cp-head">
      <div>
        <nav class="crumbs" aria-label="Where you are"><button class="quiet" on:click={() => coresSelection.set({ platform: '', core: '' })}>Cores</button><span aria-hidden="true">›</span><button class="quiet" on:click={() => pickPlatform($coresSelection.platform)}>{node?.platform.name ?? $coresSelection.platform}</button></nav>
        <h1 id="cores-title">{row?.shortname ?? detail?.shortname ?? $coresSelection.core}</h1>
        <p class="lede">{$coresSelection.core}{detail?.version ? ` · version ${detail.version}` : ''}</p>
      </div>
      {#if detail}<button class="quiet" aria-expanded={$inspectorOpen} aria-controls="core-details" on:click={() => inspectorOpen.update((v) => !v)} title="Show or hide the details (I)">{$inspectorOpen ? 'Hide details' : 'Show details'}</button>{/if}
    </header>
    {#if error}<div class="empty"><h2>This core could not be read</h2><p>{error}</p><button class="primary" on:click={() => load($coresSelection.core)}>Try again</button></div>
    {:else if !detail || !files}<p class="cores-note" role="status">Reading the card…</p>
    {:else}
      <div class="cores-grid" class:no-inspector={!$inspectorOpen}>
        <div class="cores-main">
          <div class="tabs" role="tablist" aria-label="Core sections" tabindex="-1" on:keydown={tabKey}>
            {#each TABS as [id, label]}
              <button id={`cores-tab-${id}`} role="tab" class:current={tab === id} aria-selected={tab === id} aria-controls={`cores-panel-${id}`} tabindex={tab === id ? 0 : -1} on:click={() => (tab = id)}>{label}{#if id === 'files' && files.missing.length}<span class="tab-dot" role="img" aria-label="Needs attention"></span>{/if}</button>
            {/each}
          </div>
          <div id={`cores-panel-${tab}`} role="tabpanel" aria-labelledby={`cores-tab-${tab}`} tabindex="-1">
            {#if tab === 'files'}<CoreFilesTab bind:this={filesTab} {card} {detail} {files} changed={reload} />
            {:else if tab === 'saves'}<CoreSavesTab {files} />
            {:else if tab === 'screens'}{#key detail.id}<CoreScreenshotsTab {card} coreId={detail.id} />{/key}
            {:else}{#key detail.id}<CoreSettingsTab {card} {detail} {files} {addMissing} changed={reload} />{/key}{/if}
          </div>
        </div>
        {#if $inspectorOpen}<div id="core-details" class="cores-side"><CoreInspector {card} {detail} {files} lib={cores.find((c) => c.id === detail?.id) ?? null} {openLibrary} libraryChanged={reloadCard} on:close={() => inspectorOpen.set(false)} /></div>{/if}
      </div>
    {/if}
  {:else if grp}
    <header><div>
      <nav class="crumbs" aria-label="Where you are"><button class="quiet" on:click={() => coresSelection.set({ platform: '', core: '' })}>Cores</button>{#if grp.maker}<span aria-hidden="true">›</span><button class="quiet" on:click={() => coresSelection.set({ platform: '', core: '', group: `c:${grp.cat}` })}>{grp.cat}</button>{/if}</nav>
      <p class="eyebrow">{grp.maker ? 'MANUFACTURER' : 'CATEGORY'}</p><h1 id="cores-title"><span class="ov-ic"><Icon name={categoryIcon(grp.cat)} size={22} /></span>{grp.maker || grp.cat}</h1>
      <p class="lede">{grp.maker ? `${grp.cat} · ` : ''}{plural(grp.platforms, 'platform')}, {plural(grp.cores, 'core')} on this card</p></div></header>
    {#each grp.makers as m (m.name)}
      {#if !grp.maker && m.name}<h3 class="ov-maker"><button class="quiet" on:click={() => coresSelection.set({ platform: '', core: '', group: `m:${grp.cat}/${m.name}` })}>{m.name}</button></h3>{/if}
      <ul class="ov-grid" aria-label={`${grp.cat}${m.name ? ', ' + m.name : ''}`}>
        {#each m.platforms as n (n.platform.id)}
          <li><button class="ov-card" on:click={() => pickPlatform(n.platform.id)} aria-label={`${n.platform.name}, ${plural(n.cores.length, 'core')}. Open`}>
            <span class="ov-pi"><Icon name={categoryIcon(grp.cat)} size={24} /></span><strong>{n.platform.name}</strong><small>{n.platform.year || ''}</small>
            <span class="ov-n">{plural(n.cores.length, 'core')}{#if n.cores.some((c) => c.missing)}<span class="tab-dot" role="img" aria-label="Needs attention"></span>{/if}</span>
          </button></li>
        {/each}
      </ul>
    {/each}
  {:else if node}
    <header><div>
      <nav class="crumbs" aria-label="Where you are"><button class="quiet" on:click={() => coresSelection.set({ platform: '', core: '' })}>Cores</button></nav>
      <p class="eyebrow">{node.platform.category ? node.platform.category.toUpperCase() : 'PLATFORM'}</p><h1 id="cores-title">{node.platform.name}</h1>
      <p class="lede">{[node.platform.manufacturer, node.platform.year || ''].filter(Boolean).join(' · ')}{node.platform.manufacturer || node.platform.year ? ' · ' : ''}{plural(node.cores.length, 'core')} on this card</p></div></header>
    {#if banner}<img class="plat-banner" src={banner} alt="" />{/if}
    <ul class="core-list pv-list" aria-label={`Cores for ${node.platform.name}`}>
      {#each node.cores as c (c.id)}
        <li>
          <button class="pv-open" aria-label={`${c.shortname}, version ${c.version}. Open`} on:click={() => pickCore(node.platform.id, c.id)}>
            <span class="core-icon" aria-hidden="true">{c.shortname.slice(0, 2).toUpperCase()}</span>
            <span class="pv-main"><strong>{c.shortname}</strong><small>{c.id}</small></span>
            <span class="pv-ver">{c.version}</span>
            {#if c.missing}<span class="chip warn">Needs {plural(c.missing, 'file')}</span>{/if}
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <header><div><p class="eyebrow">YOUR CARD</p><h1 id="cores-title">Cores</h1><p class="lede">Every core on this card, grouped like the openFPGA Library: category, manufacturer, platform. Pick a platform to see its cores, or a core to see its files, saves, screenshots and settings.</p></div></header>
    <label class="ov-search"><span class="sr">Search cores, platforms and manufacturers</span>
      <input type="search" placeholder="Search cores, platforms, manufacturers" bind:value={query} /></label>
    {#if q}
      {#if hits.length}
        <ul class="core-list pv-list" aria-label="Search results">{#each hits as h (h.core.id)}
          <li><button class="pv-open" aria-label={`${h.core.shortname}, ${h.platform.name}. Open`} on:click={() => { query = ''; pickCore(h.platform.id, h.core.id); }}>
            <span class="core-icon" aria-hidden="true"><Icon name={categoryIcon(h.category)} size={20} /></span>
            <span class="pv-main"><strong>{h.core.shortname}</strong><small>{h.category} › {h.maker} › {h.platform.name} · {h.core.id}</small></span>
            <span class="pv-ver">{h.core.version}</span>{#if h.core.missing}<span class="chip warn">Needs {plural(h.core.missing, 'file')}</span>{/if}
          </button></li>{/each}</ul>
      {:else}<div class="empty"><h2>Nothing matches “{query}”</h2><p>Try a core name, a platform or a manufacturer.</p></div>{/if}
    {:else}
    <div class="tabs ov-tabs" role="tablist" aria-label="Cores sections" tabindex="-1">
      <button id="cores-ov-tab-browse" role="tab" class:current={$coresTab === 'browse'} aria-selected={$coresTab === 'browse'} aria-controls="cores-ov-panel" tabindex={$coresTab === 'browse' ? 0 : -1} on:click={() => coresTab.set('browse')}>Platforms</button>
      <button id="cores-ov-tab-updates" role="tab" class:current={$coresTab === 'updates'} aria-selected={$coresTab === 'updates'} aria-controls="cores-ov-panel" tabindex={$coresTab === 'updates' ? 0 : -1} on:click={() => coresTab.set('updates')}>Updates{#if $coreUpdateCount}<span class="chip capable ov-n-chip">{$coreUpdateCount}</span>{/if}</button>
    </div>
    <div id="cores-ov-panel" role="tabpanel" aria-labelledby={`cores-ov-tab-${$coresTab}`} tabindex="-1">
    {#if $coresTab === 'updates'}<CoreUpdates {card} />
    {:else}
    {#if !$coresTreeStore.length}<div class="empty"><h2>No cores on this card</h2><p>Install some from the Marketplace.</p></div>
    {:else}
      {#each $coresGroups as g (g.name)}
        <section class="ov-cat" aria-labelledby={`ov-cat-${g.name.replace(/\W+/g, '-')}`}>
          <h2 id={`ov-cat-${g.name.replace(/\W+/g, '-')}`}><span class="ov-ic"><Icon name={categoryIcon(g.name)} size={20} /></span>{g.name} <small>{plural(g.cores, 'core')}</small></h2>
          {#each g.makers as m (m.name)}
            {#if m.name}<h3 class="ov-maker">{m.name}</h3>{/if}
            <ul class="ov-grid" aria-label={`${g.name}${m.name ? ', ' + m.name : ''}`}>
              {#each m.platforms as n (n.platform.id)}
                <li><button class="ov-card" on:click={() => pickPlatform(n.platform.id)} aria-label={`${n.platform.name}, ${plural(n.cores.length, 'core')}. Open`}>
                  <span class="ov-pi"><Icon name={categoryIcon(g.name)} size={24} /></span><strong>{n.platform.name}</strong><small>{n.platform.year || ''}</small>
                  <span class="ov-n">{plural(n.cores.length, 'core')}{#if n.cores.some((c) => c.missing)}<span class="tab-dot" role="img" aria-label="Needs attention"></span>{/if}</span>
                </button></li>
              {/each}
            </ul>
          {/each}
        </section>
      {/each}
    {/if}
    {/if}
    </div>
    {/if}
  {/if}
</section>
<style>
  .cores-note{font-size:13px;color:#95a5a4}
  .cp-head{display:flex;justify-content:space-between;align-items:flex-start;gap:16px}
  .crumbs{display:flex;align-items:center;gap:2px;font-size:12px;color:#8f9e9d;margin:0 0 8px -8px}.crumbs .quiet{padding:4px 8px;font-size:12px}
  .cores-grid{display:grid;grid-template-columns:minmax(0,1fr) 330px;gap:22px;align-items:start}.cores-grid.no-inspector{grid-template-columns:minmax(0,1fr)}
  .cores-side{position:sticky;top:16px}
  .tabs{display:flex;gap:4px;border-bottom:1px solid #2c393a;margin:0 0 16px}
  .tabs [role=tab]{position:relative;padding:10px 14px;font-size:13.5px;color:#a4b0b0;background:transparent;border-radius:8px 8px 0 0}
  .tabs [role=tab]:hover{color:#eff5f4;background:#1a2325}.tabs [role=tab].current{color:#c1f0ad;box-shadow:inset 0 -2px 0 #c1f0ad}
  .tabs [role=tab]:focus-visible{outline:2px solid #c1f0ad;outline-offset:-2px}
  .tab-dot{display:inline-block;width:7px;height:7px;border-radius:50%;background:#e5c88a;margin-left:7px;vertical-align:middle}
  .plat-banner{display:block;width:min(521px,100%);border-radius:10px;margin:0 0 18px;background:#000}
  .pv-list{list-style:none;margin:0;padding:0}.pv-list li{display:block;padding:0;border-bottom:1px solid #2c393a}.pv-list li:last-child{border:0}
  .pv-open{width:100%;display:flex;align-items:center;gap:13px;padding:13px 16px;background:transparent;color:inherit;text-align:left}.pv-open:hover{background:#1c282a}.pv-open:focus-visible{outline:2px solid #c1f0ad;outline-offset:-2px}
  .pv-main{flex:1;min-width:0;display:grid;gap:3px}.pv-main strong{font-size:14px}.pv-main small{font-size:12px;color:#95a5a4;overflow:hidden;text-overflow:ellipsis}.pv-ver{font-size:12.5px;color:#c7d1d0}
  .chip.warn{background:#241f12;color:#e5d9a8}
  .ov-search{display:block;margin:0 0 16px}.ov-search input{width:100%;box-sizing:border-box;font:inherit;font-size:13.5px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:10px;padding:10px 14px}
  .sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
  .ov-tabs{margin-bottom:18px}.ov-n-chip{margin-left:7px;padding:1px 7px;font-size:11px}
  .ov-ic{display:inline-grid;place-items:center;width:30px;height:30px;border-radius:9px;background:#1d2a2b;color:#c1f0ad;margin-right:10px;vertical-align:middle}.ov-pi{color:#c1f0ad;margin-bottom:2px}
  .ov-cat{margin:0 0 26px}.ov-cat h2{font-size:15px;margin:0 0 10px}.ov-cat h2 small{font-size:12px;font-weight:400;color:#8f9e9d;margin-left:6px}.ov-maker{font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:#9fb3aa;margin:14px 0 8px}
  .ov-grid{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fill,minmax(210px,1fr));gap:12px}
  .ov-card{width:100%;display:grid;gap:4px;padding:14px 16px;background:#161f20;border:1px solid #2c393a;border-radius:12px;color:inherit;text-align:left}.ov-card:hover{border-color:#3d5254}.ov-card:focus-visible{outline:2px solid #c1f0ad;outline-offset:2px}
  .ov-card strong{font-size:14.5px}.ov-card small{font-size:12px;color:#95a5a4;min-height:16px}.ov-n{font-size:12px;color:#c7d1d0;margin-top:6px}
  @media(max-width:1100px){.cores-grid{grid-template-columns:minmax(0,1fr)}.cores-side{position:static}}
</style>
