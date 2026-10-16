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
  import { coreDetail, coreFiles, coreKeep, readPlatformImage } from './tau-api';
  import { disabledList, refreshDisabled, setCoresEnabled } from './coreStateStore';
  import CoreUpdates from './CoreUpdates.svelte';
  import { onFileDrop } from './dropTarget';
  import { openManualAdd } from './manualAddStore';
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
  // Hiding from the Pocket: the switch in a core's header, and the Disabled section on the overview.
  $: if (card) void refreshDisabled(card);
  let stateBusy = false; let stateNote = '';
  /** The open core is in Disabled Cores: still here, still fixable, just not listed by the Pocket. */
  $: isDisabled = !!row?.disabled;
  let confirmEnable = false;
  async function setVisible(id: string, on: boolean) {
    if (on && files?.missing.length && !confirmEnable) { confirmEnable = true; return; }
    confirmEnable = false; stateBusy = true; stateNote = '';
    try {
      const why: [string, string[]][] = on ? [] : [[id, (files?.missing ?? []).map((m) => `It needs ${m.filename} (${m.slot}).`)]];
      await setCoresEnabled(card, [id], on, why);
      if (on) await coreKeep(card, [id], true);
      await reload(); reloadCard();
      stateNote = on ? 'This core is visible on the Pocket again.' : 'This core is now hidden from the Pocket. It stays here so you can fix what it needs.';
    } catch (e) { stateNote = `Nothing was changed. ${errorMessage(e)}`; }
    finally { stateBusy = false; }
  }
  async function showCores(ids: string[]) {
    stateBusy = true; stateNote = '';
    try { await setCoresEnabled(card, ids, true); await coreKeep(card, ids, true); reloadCard(); stateNote = `${plural(ids.length, 'core')} enabled again. ${ids.length === 1 ? 'It' : 'They'} will not be suggested for hiding again.`; }
    catch (e) { stateNote = `Nothing was changed. ${errorMessage(e)}`; }
    finally { stateBusy = false; }
  }
  const when = (t: number) => (t ? new Date(t * 1000).toLocaleDateString() : 'unknown date');
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
  // A zip or folder dropped on the overview, a platform or a category page opens the manual-add sheet (the Files tab keeps its own drop target).
  const offDrop = onFileDrop((e) => { if (e.type === 'drop' && !$coresSelection.core && e.paths.length === 1) openManualAdd(e.paths[0]); });
  onDestroy(offDrop);
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
<section class="page cores" class:has-core={!!$coresSelection.core && !!card} aria-labelledby="cores-title">
  {#if !card}
    <header><div><p class="eyebrow">CORES</p><h1 id="cores-title">Cores</h1><p class="lede">Open a card to see the cores on it, organised by platform.</p></div></header>
  {:else if $coresSelection.core}
    <header class="cp-head">
      <div>
        <nav class="crumbs" aria-label="Where you are"><button class="quiet" on:click={() => coresSelection.set({ platform: '', core: '' })}>Cores</button><span aria-hidden="true">›</span><button class="quiet" on:click={() => pickPlatform($coresSelection.platform)}>{node?.platform.name ?? $coresSelection.platform}</button></nav>
        <h1 id="cores-title">{row?.shortname ?? detail?.shortname ?? $coresSelection.core}</h1>
        <p class="lede">{$coresSelection.core}{detail?.version ? ` · version ${detail.version}` : ''}</p>
      </div>
      <div class="cp-actions">
      {#if detail}<button class="vis" class:off={isDisabled} role="switch" aria-checked={!isDisabled} disabled={stateBusy} on:click={() => setVisible(detail?.id ?? '', isDisabled)} title={isDisabled ? 'Turn on to show this core in the Pocket’s menu again' : 'Turn off to hide this core from the Pocket’s menu (nothing is deleted; it stays here)'}><span class="vis-knob" aria-hidden="true"></span>Visible on the Pocket</button>{/if}
      {#if detail}<button class="quiet" aria-expanded={$inspectorOpen} aria-controls="core-details" on:click={() => inspectorOpen.update((v) => !v)} title="Show or hide the details (I)">{$inspectorOpen ? 'Hide details' : 'Show details'}</button>{/if}
      </div>
    </header>
    {#if isDisabled && detail}<p class="dis-banner" role="status"><button class="primary dis-on" disabled={stateBusy} on:click={() => setVisible(detail?.id ?? '', true)}>Enable on the Pocket</button><span><strong>Disabled on the Pocket.</strong> The Pocket does not list this core. {files && files.missing.length ? `It still needs ${files.missing.map((m) => m.filename).join(', ')}: add ${files.missing.length === 1 ? 'it' : 'them'} in Files, then switch “Visible on the Pocket” on.` : 'Switch “Visible on the Pocket” on to bring it back.'}</span></p>{/if}
    {#if confirmEnable && files}<p class="dis-warn" role="alert">It still lacks {files.missing.map((m) => m.filename).join(', ')}, so it may not start. Enable it anyway? <button class="quiet" on:click={() => setVisible(detail?.id ?? '', true)}>Enable anyway</button><button class="quiet" on:click={() => (confirmEnable = false)}>Cancel</button></p>{/if}
    {#if stateNote}<p class="cores-note" role="status">{stateNote}</p>{/if}
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
          <div class="cores-scroll" id={`cores-panel-${tab}`} role="tabpanel" aria-labelledby={`cores-tab-${tab}`} tabindex="-1">
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
            <span class="ov-n">{plural(n.cores.length, 'core')}{#if n.cores.some((c) => c.disabled)}{' '}· {n.cores.filter((c) => c.disabled).length} disabled{/if}{#if n.cores.some((c) => c.missing)}<span class="tab-dot" role="img" aria-label="Needs attention"></span>{/if}</span>
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
        <li class:has-en={c.disabled}>
          <button class="pv-open" aria-label={`${c.shortname}, version ${c.version}. Open`} on:click={() => pickCore(node.platform.id, c.id)}>
            <span class="core-icon" aria-hidden="true">{c.shortname.slice(0, 2).toUpperCase()}</span>
            <span class="pv-main"><strong>{c.shortname}</strong><small>{c.id}</small></span>
            <span class="pv-ver">{c.version}</span>
            {#if c.disabled}<span class="chip warn">Disabled on the Pocket</span>{/if}
            {#if c.missing}<span class="chip warn">Needs {plural(c.missing, 'file')}</span>{/if}
          </button>
          {#if c.disabled}{#if c.missing}<button class="quiet pv-en" aria-label={`Open ${c.shortname} to add what it needs, then enable it`} on:click={() => pickCore(node.platform.id, c.id)}>Fix, then enable…</button>{:else}<button class="primary pv-en" disabled={stateBusy} aria-label={`Enable ${c.shortname} on the Pocket`} on:click={() => showCores([c.id])}>Enable</button>{/if}{/if}
        </li>
      {/each}
    </ul>
  {:else}
    <header><div><p class="eyebrow">YOUR CARD</p><h1 id="cores-title">Cores</h1><p class="lede">Every core on this card, grouped like the openFPGA Library: category, manufacturer, platform. Pick a platform to see its cores, or a core to see its files, saves, screenshots and settings.</p></div></header>
    <div class="ov-bar"><label class="ov-search"><span class="sr">Search cores, platforms and manufacturers</span>
      <input type="search" placeholder="Search cores, platforms, manufacturers" bind:value={query} /></label>
      <button class="quiet" on:click={() => openManualAdd()}>Add a core…</button></div>
    {#if $disabledList.length && !q}
      <div class="dis-bar" role="status"><span><strong>{plural($disabledList.length, 'core')} {$disabledList.length === 1 ? 'is' : 'are'} disabled on the Pocket.</strong> {$disabledList.length === 1 ? 'It is' : 'They are'} hidden from its menu but still listed here, tagged “disabled”.</span>
        <span class="dis-bar-btns"><button class="quiet" on:click={() => { coresTab.set('browse'); tick().then(() => document.getElementById('ov-disabled')?.scrollIntoView({ behavior: 'smooth', block: 'start' })); }}>Show them</button><button class="primary" disabled={stateBusy} on:click={() => showCores($disabledList.map((d) => d.id))}>Enable all</button></span></div>
    {/if}
    {#if q}
      {#if hits.length}
        <ul class="core-list pv-list" aria-label="Search results">{#each hits as h (h.core.id)}
          <li><button class="pv-open" aria-label={`${h.core.shortname}, ${h.platform.name}. Open`} on:click={() => { query = ''; pickCore(h.platform.id, h.core.id); }}>
            <span class="core-icon" aria-hidden="true"><Icon name={categoryIcon(h.category)} size={20} /></span>
            <span class="pv-main"><strong>{h.core.shortname}</strong><small>{h.category} › {h.maker} › {h.platform.name} · {h.core.id}</small></span>
            <span class="pv-ver">{h.core.version}</span>{#if h.core.disabled}<span class="chip warn">Disabled on the Pocket</span>{/if}{#if h.core.missing}<span class="chip warn">Needs {plural(h.core.missing, 'file')}</span>{/if}
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
                  <span class="ov-n">{plural(n.cores.length, 'core')}{#if n.cores.some((c) => c.disabled)}{' '}· {n.cores.filter((c) => c.disabled).length} disabled{/if}{#if n.cores.some((c) => c.missing)}<span class="tab-dot" role="img" aria-label="Needs attention"></span>{/if}</span>
                </button></li>
              {/each}
            </ul>
          {/each}
        </section>
      {/each}
    {/if}
    {#if $disabledList.length}
      <section class="ov-cat dis" aria-labelledby="ov-disabled"><h2 id="ov-disabled"><span class="ov-ic"><Icon name="folder" size={20} /></span>Disabled <small>{plural($disabledList.length, 'core')} hidden from the Pocket</small></h2>
        <p class="dis-lede">Hidden from the Pocket, but still listed with their platform above, tagged “disabled”: open one to add what it needs. Nothing was deleted; saves and settings are where they were.</p>
        <ul class="core-list pv-list" aria-label="Disabled cores">
          {#each $disabledList as d (d.id)}
            <li class="dis-row"><span class="core-icon" aria-hidden="true">{(d.shortname || d.id).slice(0, 2).toUpperCase()}</span>
              <span class="pv-main"><strong>{d.shortname || d.id}</strong><small>{d.id}{d.version ? ` · ${d.version}` : ''} · {d.disabled_at ? `disabled ${when(d.disabled_at)}` : 'disabled by hand'}{d.reasons.length ? ` · ${d.reasons.join(' ')}` : ''}</small></span>
              <button class="quiet" aria-label={`Open ${d.shortname || d.id}`} on:click={() => { const h = findCore($coresTreeStore, d.id); if (h) pickCore(h.node.platform.id, d.id); }}>Open</button><button class="quiet" disabled={stateBusy} aria-label={`Enable ${d.shortname || d.id}`} on:click={() => showCores([d.id])}>Enable</button></li>
          {/each}
        </ul>
      </section>
    {/if}
    {#if stateNote}<p class="cores-note" role="status">{stateNote}</p>{/if}
    {/if}
    </div>
    {/if}
  {/if}
</section>
<style>
  .cores-note{font-size:13px;color:#95a5a4}
  .cp-actions{display:flex;align-items:center;gap:10px;flex:none}
  .vis{display:inline-flex;align-items:center;gap:9px;padding:7px 12px 7px 9px;font-size:12.5px;color:#dce6e4;background:#1a2325;border:1px solid #2c393a;border-radius:999px}
  .vis:hover{border-color:#3d5254}.vis:focus-visible{outline:2px solid #c1f0ad;outline-offset:2px}.vis:disabled{opacity:.55}
  .vis.off{color:#a6b3b2}.vis.off .vis-knob{background:#4b5a5b}.vis.off .vis-knob::after{left:2px}
  .dis-bar{display:flex;justify-content:space-between;align-items:center;gap:14px;flex-wrap:wrap;margin:0 0 16px;padding:12px 16px;border:1px solid #5c5030;background:#241f12;color:#e5d9a8;border-radius:10px;font-size:13px;line-height:1.5}.dis-bar-btns{display:flex;gap:8px}
  .dis-banner{display:flex;gap:14px;align-items:center}.dis-banner .dis-on{flex:none}
  .pv-list li.has-en{display:flex;align-items:center;gap:10px;padding-right:14px}.pv-list li.has-en .pv-open{flex:1;min-width:0}.pv-en{flex:none;white-space:nowrap}
  .dis-banner,.dis-warn{margin:0 0 12px;padding:10px 14px;border:1px solid #5c5030;background:#241f12;color:#e5d9a8;border-radius:10px;font-size:12.5px;line-height:1.55}.dis-warn button{margin-left:8px}
  .vis-knob{position:relative;width:28px;height:16px;border-radius:999px;background:#6fa35a;flex:none}.vis-knob::after{content:'';position:absolute;top:2px;left:14px;width:12px;height:12px;border-radius:50%;background:#101617}
  .dis .dis-lede{font-size:12.5px;color:#95a5a4;margin:0 0 10px}.dis-row{display:flex;align-items:center;gap:13px;padding:11px 16px}.dis-row .core-icon{opacity:.6}
  .dis :global(code){font-size:11.5px}
  /* A core's page is one screen tall: the header and tabs stay put, the content and the details column each scroll on their own. */
  .cores.has-core{flex:1;min-height:0;display:flex;flex-direction:column;overflow:hidden;padding-bottom:0}
  .cp-head{flex:none;display:flex;justify-content:space-between;align-items:flex-start;gap:16px;padding-right:52px}
  .cores-main{display:flex;flex-direction:column;min-height:0;min-width:0}.cores-main .tabs{flex:none}
  .cores-scroll{flex:1;min-height:0;overflow:auto;padding:2px 6px 32px 0}
  .cores-side{min-height:0;overflow:auto;padding:2px 2px 24px 0}.cores-side :global(.ci){min-height:100%}
  .crumbs{display:flex;align-items:center;gap:2px;font-size:12px;color:#8f9e9d;margin:0 0 8px -8px}.crumbs .quiet{padding:4px 8px;font-size:12px}
  .cores-grid{flex:1;min-height:0;display:grid;grid-template-columns:minmax(0,1fr) 330px;gap:22px;align-items:stretch}.cores-grid.no-inspector{grid-template-columns:minmax(0,1fr)}
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
  .ov-bar{display:flex;gap:10px;align-items:center;margin:0 0 16px}.ov-bar .quiet{flex:none;white-space:nowrap}.ov-search{display:block;flex:1}.ov-search input{width:100%;box-sizing:border-box;font:inherit;font-size:13.5px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:10px;padding:10px 14px}
  .sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
  .ov-tabs{margin-bottom:18px}.ov-n-chip{margin-left:7px;padding:1px 7px;font-size:11px}
  .ov-ic{display:inline-grid;place-items:center;width:30px;height:30px;border-radius:9px;background:#1d2a2b;color:#c1f0ad;margin-right:10px;vertical-align:middle}.ov-pi{color:#c1f0ad;margin-bottom:2px}
  .ov-cat{margin:0 0 26px}.ov-cat h2{font-size:15px;margin:0 0 10px}.ov-cat h2 small{font-size:12px;font-weight:400;color:#8f9e9d;margin-left:6px}.ov-maker{font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:#9fb3aa;margin:14px 0 8px}
  .ov-grid{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fill,minmax(210px,1fr));gap:12px}
  .ov-card{width:100%;display:grid;gap:4px;padding:14px 16px;background:#161f20;border:1px solid #2c393a;border-radius:12px;color:inherit;text-align:left}.ov-card:hover{border-color:#3d5254}.ov-card:focus-visible{outline:2px solid #c1f0ad;outline-offset:2px}
  .ov-card strong{font-size:14.5px}.ov-card small{font-size:12px;color:#95a5a4;min-height:16px}.ov-n{font-size:12px;color:#c7d1d0;margin-top:6px}
  @media(max-width:1100px){.cores.has-core{overflow:visible}.cores-grid{grid-template-columns:minmax(0,1fr)}.cores-scroll,.cores-side{overflow:visible}}
</style>
