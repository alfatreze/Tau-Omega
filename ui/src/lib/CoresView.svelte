<script lang="ts">
  // The Cores page. Three views, chosen by the side-menu tree: all platforms (overview), one platform (its cores), one core (tabs + details).
  // A core opens on Files; the right-hand details panel can be hidden and the choice is remembered.
  import { onDestroy, tick } from 'svelte';
  import CoreFilesTab from './CoreFilesTab.svelte';
  import CoreInspector from './CoreInspector.svelte';
  import CoreSavesTab from './CoreSavesTab.svelte';
  import CoreScreenshotsTab from './CoreScreenshotsTab.svelte';
  import CoreSettingsTab from './CoreSettingsTab.svelte';
  import GameArtworkPickerSheet from './GameArtworkPickerSheet.svelte';
  import GameBubCoreView from './GameBubCoreView.svelte';
  import GameDetailsInspector from './GameDetailsInspector.svelte';
  import ArtworkPickerSheet from './ArtworkPickerSheet.svelte';
  import RemoveFilesSheet from './RemoveFilesSheet.svelte';
  import { extensionsForCore, extensionsForPlatform } from './coreExtensions';
  import { errorMessage } from './backend';
  import { displayGameTitle, gameArtCacheKey, gameArtMap } from './gameArtStore';
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
  import { platformFallbackArt } from './platformArt';
  import { showToast } from './toastStore';
  import type { Core, CoreDetail, CoreFiles, FileEntry, FolderListing, PlatformNode } from './types';
  export let card = '';
  /** The card-level core records (library facts) and what to do with them. */
  export let cores: Core[] = []; export let openLibrary: (c: Core) => void = () => {}; export let reloadCard: () => void = () => {};

  type BaseTab = 'files' | 'saves' | 'screens' | 'settings';
  const BASE_TABS: [string, string][] = [['files', 'Files'], ['saves', 'Saves'], ['screens', 'Screenshots'], ['settings', 'Settings']];
  let tab = 'files';
  let platTab = 'cores';
  let platTabFor = '';
  let detail: CoreDetail | null = null; let files: CoreFiles | null = null; let error = ''; let loadedFor = '';
  let filesTab: CoreFilesTab | undefined; let banner: string | null = null; let bannerFor = ''; let showPlatArtPicker = false;
  type SelectedGame = { platformId: string; platformName: string; romName: string; relPath: string; bytes: number; modified: number };
  let selectedGame: SelectedGame | null = null;
  let sideMode: 'core' | 'game' = 'core';
  let gameArtTarget: { platformId: string; platformName: string; romName: string } | null = null;
  let removeSingleGame: string[] | null = null;

  function openGameDetails(item: FileEntry, folder: FolderListing) {
    const platName = detail?.platforms.find((p) => p.id === folder.folder.platform)?.name ?? folder.folder.platform;
    selectedGame = {
      platformId: folder.folder.platform,
      platformName: platName,
      romName: item.name,
      relPath: `${folder.folder.rel_path}/${item.rel_path}`,
      bytes: item.bytes,
      modified: item.modified,
    };
    sideMode = 'game';
    inspectorOpen.set(true);
  }

  $: selectedGameSaves = (() => {
    if (!selectedGame || !files) return [];
    const targetTitle = displayGameTitle(selectedGame.romName).toLowerCase();
    const out: { name: string; label: string; kind: 'save' | 'state'; bytes: number; modified: number }[] = [];
    for (const f of files.folders) {
      if (f.folder.role !== 'SavesShared' && f.folder.role !== 'SavesCore' && f.folder.role !== 'SaveStates') continue;
      for (const e of f.entries) {
        const gStem = (e.game || displayGameTitle(e.name)).trim().toLowerCase();
        if (gStem === targetTitle || (targetTitle && gStem.includes(targetTitle))) {
          out.push({
            name: e.name,
            label: f.folder.role === 'SaveStates' ? (e.taken ? `Save state · ${e.taken}` : 'Save state') : (e.slot || 'Save RAM'),
            kind: f.folder.role === 'SaveStates' ? 'state' : 'save',
            bytes: e.bytes,
            modified: e.modified,
          });
        }
      }
    }
    return out;
  })();

  $: node = $coresTreeStore.find((n) => n.platform.id === $coresSelection.platform);
  $: platExts = node ? extensionsForPlatform('analogue_pocket', node.platform.id) : [];
  $: coreExts = detail ? extensionsForCore({ device: 'analogue_pocket', coreId: detail.id, platformIds: detail.platforms.map((p) => p.id) }) : [];
  $: visibleTabs = [...BASE_TABS, ...coreExts.map((ext): [string, string] => [ext.id, ext.label])];
  $: if (node && platTabFor !== node.platform.id) { platTabFor = node.platform.id; platTab = 'cores'; }
  $: row = $coresSelection.core ? findCore($coresTreeStore, $coresSelection.core)?.core : undefined;
  $: isGameBubCore = $coresSelection.platform === 'gamebub';
  $: if (card && $coresSelection.core && !isGameBubCore && `${card}|${$coresSelection.core}` !== loadedFor) void load($coresSelection.core);
  $: if (!$coresSelection.core || isGameBubCore) { detail = null; files = null; loadedFor = ''; }
  $: if (card && node && bannerFor !== `${card}|${node.platform.id}`) {
    bannerFor = `${card}|${node.platform.id}`;
    banner = null;
    readPlatformImage(card, node.platform.id).then((u) => (banner = u)).catch(() => (banner = null));
  } else if (!node && bannerFor) { bannerFor = ''; banner = null; }
  function onArtworkChanged(e: Event) {
    const pid = (e as CustomEvent<{ platformId?: string }>).detail?.platformId;
    if (card && node && (!pid || pid === node.platform.id)) {
      readPlatformImage(card, node.platform.id).then((u) => (banner = u)).catch(() => (banner = null));
    }
    if (pid) {
      if (ovBannerRequested.has(pid)) loadOvBanner(pid, true);
    } else {
      for (const id of ovBannerRequested) loadOvBanner(id, true);
    }
  }
  if (typeof window !== 'undefined') window.addEventListener('tau-artwork-changed', onArtworkChanged);

  async function load(id: string) {
    loadedFor = `${card}|${id}`; error = ''; detail = null; files = null; tab = 'files'; selectedGame = null; sideMode = 'core';
    try { [detail, files] = await Promise.all([coreDetail(card, id), coreFiles(card, id)]); }
    catch (e) { error = errorMessage(e); }
  }
  // Hiding from the Pocket: the switch in Settings, and the Disabled section on the overview.
  $: if (card) void refreshDisabled(card);
  let stateBusy = false;
  /** The open core is in Disabled Cores: still here, still fixable, just not listed by the Pocket. */
  $: isDisabled = !!row?.disabled;
  let confirmEnable = false;
  async function setVisible(id: string, on: boolean) {
    if (on && files?.missing.length && !confirmEnable) { confirmEnable = true; return; }
    confirmEnable = false; stateBusy = true;
    try {
      const why: [string, string[]][] = on ? [] : [[id, (files?.missing ?? []).map((m) => `It needs ${m.filename} (${m.slot}).`)]];
      await setCoresEnabled(card, [id], on, why);
      if (on) await coreKeep(card, [id], true);
      await reload(); reloadCard();
      showToast(on ? 'This core is visible on the Pocket again.' : 'This core is now hidden from the Pocket. It stays here so you can fix what it needs.', on ? 'success' : 'info');
    } catch (e) { showToast(`Nothing was changed. ${errorMessage(e)}`, 'error'); }
    finally { stateBusy = false; }
  }
  async function showCores(ids: string[]) {
    stateBusy = true;
    try { await setCoresEnabled(card, ids, true); await coreKeep(card, ids, true); reloadCard(); showToast(`${plural(ids.length, 'core')} enabled again. ${ids.length === 1 ? 'It' : 'They'} will not be suggested for hiding again.`); }
    catch (e) { showToast(`Nothing was changed. ${errorMessage(e)}`, 'error'); }
    finally { stateBusy = false; }
  }
  const when = (t: number) => (t ? new Date(t * 1000).toLocaleDateString() : 'unknown date');
  async function reload() { if (!$coresSelection.core) return; try { files = await coreFiles(card, $coresSelection.core); void refreshCores(card); } catch (e) { error = errorMessage(e); } }
  async function addMissing(folderRel: string) { tab = 'files'; await tick(); await filesTab?.chooseFor(folderRel); }

  function tabKey(e: KeyboardEvent) {
    const i = visibleTabs.findIndex(([t]) => t === tab);
    const next = e.key === 'ArrowRight' ? (i + 1) % visibleTabs.length : e.key === 'ArrowLeft' ? (i + visibleTabs.length - 1) % visibleTabs.length : e.key === 'Home' ? 0 : e.key === 'End' ? visibleTabs.length - 1 : -1;
    if (next < 0) return;
    e.preventDefault(); tab = visibleTabs[next][0];
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
  onDestroy(() => {
    offDrop();
    if (typeof window !== 'undefined') window.removeEventListener('tau-artwork-changed', onArtworkChanged);
  });
  // Search and overview filters live here, on the Cores page: cores, platforms, manufacturers and categories.
  let query = '';
  let catFilter = '';
  let makerFilter = '';
  let statusFilter: 'all' | 'enabled' | 'disabled' | 'needs' = 'all';
  let filtersForCard = '';
  $: if (card !== filtersForCard) {
    filtersForCard = card;
    catFilter = '';
    makerFilter = '';
    statusFilter = 'all';
  }
  function clearFilters() {
    catFilter = '';
    makerFilter = '';
    statusFilter = 'all';
    query = '';
  }

  $: allCoresCount = $coresGroups.reduce((n, g) => n + g.cores, 0);
  $: totalDisabled = $coresTreeStore.reduce((n, p) => n + p.cores.filter((c) => c.disabled).length, 0);
  $: totalNeeds = $coresTreeStore.reduce((n, p) => n + p.cores.filter((c) => c.missing > 0).length, 0);
  $: availableMakers = [...new Set($coresGroups.flatMap((g) => g.makers.map((m) => m.name).filter(Boolean)))].sort((a, b) => a.localeCompare(b));
  $: hasActiveFilters = !!(catFilter || makerFilter || statusFilter !== 'all');

  function matchesStatus(n: PlatformNode): boolean {
    if (statusFilter === 'enabled') return n.cores.some((c) => !c.disabled);
    if (statusFilter === 'disabled') return n.cores.some((c) => c.disabled);
    if (statusFilter === 'needs') return n.cores.some((c) => c.missing > 0);
    return true;
  }
  function coreMatchesStatus(c: { disabled?: boolean; missing: number }): boolean {
    if (statusFilter === 'enabled') return !c.disabled;
    if (statusFilter === 'disabled') return !!c.disabled;
    if (statusFilter === 'needs') return c.missing > 0;
    return true;
  }
  function cardMeta(n: PlatformNode) {
    const disabled = n.cores.filter((c) => c.disabled).length;
    const missing = n.cores.filter((c) => c.missing > 0).length;
    const allDisabled = n.cores.length > 0 && disabled === n.cores.length;
    return { disabled, missing, allDisabled };
  }

  $: filteredGroups = !hasActiveFilters
    ? $coresGroups
    : $coresGroups
        .filter((g) => !catFilter || g.name === catFilter)
        .map((g) => {
          const makers = g.makers
            .filter((m) => !makerFilter || m.name === makerFilter)
            .map((m) => {
              const platforms = m.platforms.filter(matchesStatus);
              return { ...m, platforms, cores: platforms.reduce((n, p) => n + p.cores.length, 0) };
            })
            .filter((m) => m.platforms.length > 0);
          return {
            ...g,
            makers,
            platforms: makers.reduce((n, m) => n + m.platforms.length, 0),
            cores: makers.reduce((n, m) => n + m.cores, 0),
          };
        })
        .filter((g) => g.makers.length > 0);

  $: filteredTotals = {
    platforms: filteredGroups.reduce((n, g) => n + g.platforms, 0),
    cores: filteredGroups.reduce((n, g) => n + g.cores, 0),
  };

  let ovBanners: Record<string, string | null> = {};
  let ovBannersCard = '';
  const ovBannerRequested = new Set<string>();
  $: if (card !== ovBannersCard) {
    ovBannersCard = card;
    ovBanners = {};
    ovBannerRequested.clear();
  }
  function loadOvBanner(platformId: string, force = false) {
    if (!card || !platformId || platformId === '_none') return;
    if (!force && ovBannerRequested.has(platformId)) return;
    ovBannerRequested.add(platformId);
    const forCard = card;
    readPlatformImage(forCard, platformId)
      .then((u) => { if (card === forCard) ovBanners = { ...ovBanners, [platformId]: u }; })
      .catch(() => { if (card === forCard) ovBanners = { ...ovBanners, [platformId]: null }; });
  }
  function observeBanner(el: HTMLElement, platformId: string) {
    let pid = platformId;
    let observer: IntersectionObserver | null = null;
    const attach = () => {
      observer?.disconnect();
      observer = null;
      if (!pid || pid === '_none') return;
      if (typeof IntersectionObserver === 'undefined') {
        loadOvBanner(pid);
        return;
      }
      observer = new IntersectionObserver(
        (entries) => {
          if (entries.some((entry) => entry.isIntersecting)) {
            loadOvBanner(pid);
            observer?.disconnect();
            observer = null;
          }
        },
        { rootMargin: '200px' },
      );
      observer.observe(el);
    };
    attach();
    return {
      update(nextPid: string) {
        if (nextPid !== pid) {
          pid = nextPid;
          attach();
        }
      },
      destroy() {
        observer?.disconnect();
        observer = null;
      },
    };
  }

  $: q = query.trim().toLowerCase();
  $: hits = q ? $coresGroups
    .filter((g) => !catFilter || g.name === catFilter)
    .flatMap((g) => g.makers
      .filter((m) => !makerFilter || m.name === makerFilter)
      .flatMap((m) => m.platforms.flatMap((n) => n.cores
        .filter((c) => coreMatchesStatus(c) && (`${c.id} ${c.shortname} ${c.author} ${n.platform.name} ${m.name} ${g.name}`.toLowerCase().includes(q) || n.platform.name.toLowerCase().includes(q)))
        .map((c) => ({ core: c, platform: n.platform, category: g.name, maker: m.name }))))) : [];
  const pickPlatform = (id: string) => {
    const n = $coresTreeStore.find((x) => x.platform.id === id);
    if (n && n.cores.length === 1) coresSelection.set({ platform: id, core: n.cores[0].id });
    else coresSelection.set({ platform: id, core: '' });
  };
  const pickCore = (platform: string, core: string) => coresSelection.set({ platform, core });
  onDestroy(() => {});
</script>
<svelte:window on:keydown={onKey} />
<section class="page cores" class:has-core={!!$coresSelection.core && !!card} aria-labelledby="cores-title">
  {#if !card}
    <header><div><p class="eyebrow">CORES</p><h1 id="cores-title">Cores</h1><p class="lede">Open a card to see the cores on it, organised by platform.</p></div></header>
  {:else if $coresSelection.core && isGameBubCore}
    <GameBubCoreView {card} coreId={$coresSelection.core} {reloadCard} />
  {:else if $coresSelection.core}
    <header class="cp-head">
      <div>
        <nav class="crumbs" aria-label="Where you are"><button class="quiet" on:click={() => coresSelection.set({ platform: '', core: '' })}>Cores</button>{#if node && node.cores.length > 1}<span aria-hidden="true">›</span><button class="quiet" on:click={() => pickPlatform($coresSelection.platform)}>{node.platform.name}</button>{/if}</nav>
        <h1 id="cores-title">{row?.shortname ?? detail?.shortname ?? $coresSelection.core}</h1>
        <p class="lede">{$coresSelection.core}{detail?.version ? ` · version ${detail.version}` : ''}</p>
      </div>
      <div class="cp-actions">
      {#if detail}
        <button
          type="button"
          class="cp-side-btn"
          class:active={$inspectorOpen}
          aria-label={$inspectorOpen ? 'Hide details' : 'Show details'}
          aria-expanded={$inspectorOpen}
          aria-controls="core-details"
          on:click={() => inspectorOpen.update((v) => !v)}
          title={$inspectorOpen ? 'Hide details (I)' : 'Show details (I)'}
        >
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <rect x="3" y="4" width="18" height="16" rx="2" />
            <line x1="15" y1="4" x2="15" y2="20" />
          </svg>
        </button>
      {/if}
      </div>
    </header>
    {#if isDisabled && detail}<p class="dis-banner" role="status"><button class="primary dis-on" disabled={stateBusy} on:click={() => setVisible(detail?.id ?? '', true)}>Enable on the Pocket</button><span><strong>Disabled on the Pocket.</strong> The Pocket does not list this core. {files && files.missing.length ? `It still needs ${files.missing.map((m) => m.filename).join(', ')}: add ${files.missing.length === 1 ? 'it' : 'them'} in Files, then open Settings and switch “Visible on the Pocket” on.` : 'Open Settings and switch “Visible on the Pocket” on to bring it back.'}</span></p>{/if}
    {#if confirmEnable && files && tab !== 'settings'}<p class="dis-warn" role="alert">It still lacks {files.missing.map((m) => m.filename).join(', ')}, so it may not start. Enable it anyway? <button class="quiet" on:click={() => setVisible(detail?.id ?? '', true)}>Enable anyway</button><button class="quiet" on:click={() => (confirmEnable = false)}>Cancel</button></p>{/if}
    {#if error}<div class="empty"><h2>This core could not be read</h2><p>{error}</p><button class="primary" on:click={() => load($coresSelection.core)}>Try again</button></div>
    {:else if !detail || !files}<p class="cores-note" role="status">Reading the card…</p>
    {:else}
      <div class="cores-grid" class:no-inspector={!$inspectorOpen}>
        <div class="cores-main">
          <div class="tabs" role="tablist" aria-label="Core sections" tabindex="-1" on:keydown={tabKey}>
            {#each visibleTabs as [id, label]}
              <button id={`cores-tab-${id}`} role="tab" class:current={tab === id} aria-selected={tab === id} aria-controls={`cores-panel-${id}`} tabindex={tab === id ? 0 : -1} on:click={() => (tab = id)}>{label}{#if id === 'files' && files.missing.length}<span class="tab-dot" role="img" aria-label="Needs attention"></span>{/if}</button>
            {/each}
          </div>
          <div class="cores-scroll" id={`cores-panel-${tab}`} role="tabpanel" aria-labelledby={`cores-tab-${tab}`} tabindex="-1">
            {#if tab === 'files'}<CoreFilesTab bind:this={filesTab} {card} {detail} {files} selectedGameRel={selectedGame?.relPath ?? ''} onSelectGame={openGameDetails} changed={reload} />
            {:else if tab === 'saves'}<CoreSavesTab {files} />
            {:else if tab === 'screens'}{#key detail.id}<CoreScreenshotsTab {card} coreId={detail.id} />{/key}
            {:else if tab === 'settings'}{#key detail.id}<CoreSettingsTab {card} {detail} {files} {isDisabled} {stateBusy} {confirmEnable} setVisible={(on) => setVisible(detail?.id ?? '', on)} cancelEnable={() => (confirmEnable = false)} {addMissing} changed={reload} />{/key}
            {:else}
              {#each coreExts as ext (ext.id)}
                {#if tab === ext.id}<svelte:component this={ext.component} {card} />{/if}
              {/each}
            {/if}
          </div>
        </div>
        {#if $inspectorOpen}
          <div id="core-details" class="cores-side">
            {#if selectedGame}
              <div class="side-mode-bar" role="group" aria-label="Details view">
                <button type="button" class:active={sideMode === 'core'} aria-pressed={sideMode === 'core'} on:click={() => (sideMode = 'core')}>Core details</button>
                <button type="button" class:active={sideMode === 'game'} aria-pressed={sideMode === 'game'} on:click={() => (sideMode = 'game')}>Game details</button>
                <button type="button" class="side-mode-clear" aria-label="Clear selected game" title="Back to core details" on:click={() => { selectedGame = null; sideMode = 'core'; }}>✕</button>
              </div>
            {/if}
            {#if sideMode === 'game' && selectedGame}
              <GameDetailsInspector
                platformId={selectedGame.platformId}
                platformName={selectedGame.platformName}
                romName={selectedGame.romName}
                relPath={selectedGame.relPath}
                bytes={selectedGame.bytes}
                modified={selectedGame.modified}
                saves={selectedGameSaves}
                onOpenArtwork={() => {
                  if (selectedGame) gameArtTarget = { platformId: selectedGame.platformId, platformName: selectedGame.platformName, romName: selectedGame.romName };
                }}
                onRemove={() => {
                  if (selectedGame) removeSingleGame = [selectedGame.relPath];
                }}
              />
            {:else}
              <CoreInspector
                {card}
                {detail}
                {files}
                lib={cores.find((c) => c.id === detail?.id) ?? null}
                {openLibrary}
                libraryChanged={reloadCard}
                onCoreRemoved={() => {
                  reloadCard();
                  coresSelection.set({ platform: '', core: '' });
                }}
                onCoreUpdated={() => {
                  reloadCard();
                  void load(detail?.id ?? $coresSelection.core);
                }}
              />
            {/if}
          </div>
        {/if}
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
          {@const st = cardMeta(n)}
          <li use:observeBanner={n.platform.id}><button class="ov-card" class:has-disabled={st.disabled > 0} class:all-disabled={st.allDisabled} on:click={() => pickPlatform(n.platform.id)} aria-label={`${n.platform.name}, ${plural(n.cores.length, 'core')}${st.disabled ? `, ${st.disabled} disabled` : ''}. Open`}>
            <span class="ov-art" aria-hidden="true">
              <img src={ovBanners[n.platform.id] || platformFallbackArt({ id: n.platform.id, name: n.platform.name, category: grp.cat, manufacturer: n.platform.manufacturer })} alt="" />
              {#if st.disabled > 0}
                <span class="ov-badge" class:ov-badge-all={st.allDisabled}>
                  <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true"><circle cx="8" cy="8" r="6" /><path d="M3.8 12.2L12.2 3.8" /></svg>
                  {st.allDisabled ? 'Disabled' : `${st.disabled}/${n.cores.length} disabled`}
                </span>
              {/if}
            </span>
            <span class="ov-body">
              <span class="ov-title-row"><strong>{n.platform.name}</strong>{#if n.platform.year}<small class="ov-year">{n.platform.year}</small>{/if}</span>
              <span class="ov-n">{plural(n.cores.length, 'core')}{#if st.disabled}{' '}· <span class="ov-dis-txt">{st.disabled} disabled</span>{/if}{#if st.missing}<span class="tab-dot" role="img" aria-label="Needs attention"></span>{/if}</span>
            </span>
          </button></li>
        {/each}
      </ul>
    {/each}
  {:else if node}
    <header><div>
      <nav class="crumbs" aria-label="Where you are"><button class="quiet" on:click={() => coresSelection.set({ platform: '', core: '' })}>Cores</button></nav>
      <p class="eyebrow">{node.platform.category ? node.platform.category.toUpperCase() : 'PLATFORM'}</p><h1 id="cores-title">{node.platform.name}</h1>
      <p class="lede">{[node.platform.manufacturer, node.platform.year || ''].filter(Boolean).join(' · ')}{node.platform.manufacturer || node.platform.year ? ' · ' : ''}{plural(node.cores.length, 'core')} on this card</p></div></header>
    <div class="plat-art-wrap">
      <img class="plat-banner" src={banner || platformFallbackArt({ id: node.platform.id, name: node.platform.name, category: node.platform.category, manufacturer: node.platform.manufacturer })} alt="" />
      <button type="button" class="plat-art-edit" on:click={() => (showPlatArtPicker = true)}>Change image</button>
    </div>
    {#if platExts.length}
      <div class="tabs" role="tablist" aria-label="Platform sections">
        <button id="plat-tab-cores" role="tab" class:current={platTab === 'cores'} aria-selected={platTab === 'cores'} on:click={() => (platTab = 'cores')}>Cores</button>
        {#each platExts as ext (ext.id)}
          <button id={`plat-tab-${ext.id}`} role="tab" class:current={platTab === ext.id} aria-selected={platTab === ext.id} on:click={() => (platTab = ext.id)}>{ext.label}</button>
        {/each}
      </div>
    {/if}
    {#if platTab !== 'cores' && platExts.some((e) => e.id === platTab)}
      {#each platExts as ext (ext.id)}
        {#if platTab === ext.id}<svelte:component this={ext.component} {card} />{/if}
      {/each}
    {:else}
    <ul class="core-list pv-list" aria-label={`Cores for ${node.platform.name}`}>
      {#each node.cores as c (c.id)}
        <li class:has-en={c.disabled} class:pv-disabled={c.disabled}>
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
    {/if}
  {:else}
    <header><div><p class="eyebrow">YOUR CARD</p><h1 id="cores-title">Cores</h1><p class="lede">Every core on this card, grouped like the openFPGA Library: category, manufacturer, platform. Pick a platform to see its cores, or a core to see its files, saves, screenshots and settings.</p></div></header>
    <div class="ov-bar">
      <label class="ov-search"><span class="sr">Search cores, platforms and manufacturers</span>
        <input type="search" placeholder="Search cores, platforms, manufacturers" bind:value={query} /></label>
      <label class="ov-select"><span class="sr">Status</span>
        <select aria-label="Status" bind:value={statusFilter} on:change={() => coresTab.set('browse')}>
          <option value="all">All statuses</option>
          <option value="enabled">Enabled only</option>
          <option value="disabled">{totalDisabled ? `Disabled only (${totalDisabled})` : 'Disabled only'}</option>
          <option value="needs">{totalNeeds ? `Needs files (${totalNeeds})` : 'Needs files'}</option>
        </select>
      </label>
      {#if availableMakers.length > 1}
        <label class="ov-select"><span class="sr">Manufacturer</span>
          <select aria-label="Manufacturer" bind:value={makerFilter} on:change={() => coresTab.set('browse')}>
            <option value="">All manufacturers</option>
            {#each availableMakers as mk}<option value={mk}>{mk}</option>{/each}
          </select>
        </label>
      {/if}
      <button class="quiet" on:click={() => openManualAdd()}>Add a core…</button>
    </div>
    {#if $disabledList.length && !q}
      <div class="dis-bar" role="status"><span><strong>{plural($disabledList.length, 'core')} {$disabledList.length === 1 ? 'is' : 'are'} disabled on the Pocket.</strong> {$disabledList.length === 1 ? 'It is' : 'They are'} hidden from its menu but still listed here, tagged “disabled”.</span>
        <span class="dis-bar-btns"><button class="quiet" on:click={() => { coresTab.set('browse'); tick().then(() => document.getElementById('ov-disabled')?.scrollIntoView({ behavior: 'smooth', block: 'start' })); }}>Show them</button><button class="primary" disabled={stateBusy} on:click={() => showCores($disabledList.map((d) => d.id))}>Enable all</button></span></div>
    {/if}
    {#if q}
      {#if hits.length}
        <ul class="core-list pv-list" aria-label="Search results">{#each hits as h (h.core.id)}
          <li class:pv-disabled={h.core.disabled}><button class="pv-open" aria-label={`${h.core.shortname}, ${h.platform.name}. Open`} on:click={() => { query = ''; pickCore(h.platform.id, h.core.id); }}>
            <span class="core-icon" aria-hidden="true"><Icon name={categoryIcon(h.category)} size={20} /></span>
            <span class="pv-main"><strong>{h.core.shortname}</strong><small>{h.category} › {h.maker} › {h.platform.name} · {h.core.id}</small></span>
            <span class="pv-ver">{h.core.version}</span>{#if h.core.disabled}<span class="chip warn">Disabled on the Pocket</span>{/if}{#if h.core.missing}<span class="chip warn">Needs {plural(h.core.missing, 'file')}</span>{/if}
          </button></li>{/each}</ul>
      {:else}<div class="empty"><h2>Nothing matches “{query}”</h2><p>Try a core name, a platform or a manufacturer.</p>{#if hasActiveFilters}<button class="quiet" on:click={clearFilters}>Clear filters</button>{/if}</div>{/if}
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
      {#if $coresGroups.length}
        <div class="ov-tags" role="group" aria-label="Category">
          <button class="chip toggle" aria-pressed={!catFilter} on:click={() => (catFilter = '')}>All <span class="ov-count">{allCoresCount}</span></button>
          {#each $coresGroups as g (g.name)}
            <button class="chip toggle" aria-pressed={catFilter === g.name} on:click={() => (catFilter = catFilter === g.name ? '' : g.name)}>{g.name} <span class="ov-count">{g.cores}</span></button>
          {/each}
        </div>
      {/if}
      {#if hasActiveFilters}
        <div class="ov-filter-note" role="status" aria-live="polite">
          <span>Showing {plural(filteredTotals.platforms, 'platform')}, {plural(filteredTotals.cores, 'core')}</span>
          <button class="quiet ov-clear" on:click={clearFilters}>Clear filters</button>
        </div>
      {/if}
      {#if !filteredGroups.length}
        <div class="empty"><h2>No platforms match</h2><p>Nothing matches the current filters.</p><button class="quiet" on:click={clearFilters}>Clear filters</button></div>
      {:else}
        {#each filteredGroups as g (g.name)}
          <section class="ov-cat" aria-labelledby={`ov-cat-${g.name.replace(/\W+/g, '-')}`}>
            <h2 id={`ov-cat-${g.name.replace(/\W+/g, '-')}`}><span class="ov-ic"><Icon name={categoryIcon(g.name)} size={20} /></span>{g.name} <small>{plural(g.cores, 'core')}</small></h2>
            {#each g.makers as m (m.name)}
              {#if m.name}<h3 class="ov-maker">{m.name}</h3>{/if}
              <ul class="ov-grid" aria-label={`${g.name}${m.name ? ', ' + m.name : ''}`}>
                {#each m.platforms as n (n.platform.id)}
                  {@const st = cardMeta(n)}
                  <li use:observeBanner={n.platform.id}><button class="ov-card" class:has-disabled={st.disabled > 0} class:all-disabled={st.allDisabled} on:click={() => pickPlatform(n.platform.id)} aria-label={`${n.platform.name}, ${plural(n.cores.length, 'core')}${st.disabled ? `, ${st.disabled} disabled` : ''}. Open`}>
                    <span class="ov-art" aria-hidden="true">
                      <img src={ovBanners[n.platform.id] || platformFallbackArt({ id: n.platform.id, name: n.platform.name, category: g.name, manufacturer: n.platform.manufacturer })} alt="" />
                      {#if st.disabled > 0}
                        <span class="ov-badge" class:ov-badge-all={st.allDisabled}>
                          <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true"><circle cx="8" cy="8" r="6" /><path d="M3.8 12.2L12.2 3.8" /></svg>
                          {st.allDisabled ? 'Disabled' : `${st.disabled}/${n.cores.length} disabled`}
                        </span>
                      {/if}
                    </span>
                    <span class="ov-body">
                      <span class="ov-title-row"><strong>{n.platform.name}</strong>{#if n.platform.year}<small class="ov-year">{n.platform.year}</small>{/if}</span>
                      <span class="ov-n">{plural(n.cores.length, 'core')}{#if st.disabled}{' '}· <span class="ov-dis-txt">{st.disabled} disabled</span>{/if}{#if st.missing}<span class="tab-dot" role="img" aria-label="Needs attention"></span>{/if}</span>
                    </span>
                  </button></li>
                {/each}
              </ul>
            {/each}
          </section>
        {/each}
      {/if}
    {/if}
    {#if $disabledList.length && (!hasActiveFilters || statusFilter === 'disabled')}
      <section class="ov-cat dis" aria-labelledby="ov-disabled"><h2 id="ov-disabled"><span class="ov-ic"><Icon name="folder" size={20} /></span>Disabled <small>{plural($disabledList.length, 'core')} hidden from the Pocket</small></h2>
        <p class="dis-lede">Hidden from the Pocket, but still listed with their platform above, tagged “disabled”: open one to add what it needs. Nothing was deleted; saves and settings are where they were.</p>
        <ul class="core-list pv-list" aria-label="Disabled cores">
          {#each $disabledList as d (d.id)}
            <li class="dis-row pv-disabled"><span class="core-icon" aria-hidden="true">{(d.shortname || d.id).slice(0, 2).toUpperCase()}</span>
              <span class="pv-main"><strong>{d.shortname || d.id}</strong><small>{d.id}{d.version ? ` · ${d.version}` : ''} · {d.disabled_at ? `disabled ${when(d.disabled_at)}` : 'disabled by hand'}{d.reasons.length ? ` · ${d.reasons.join(' ')}` : ''}</small></span>
              <button class="quiet" aria-label={`Open ${d.shortname || d.id}`} on:click={() => { const h = findCore($coresTreeStore, d.id); if (h) pickCore(h.node.platform.id, d.id); }}>Open</button><button class="quiet" disabled={stateBusy} aria-label={`Enable ${d.shortname || d.id}`} on:click={() => showCores([d.id])}>Enable</button></li>
          {/each}
        </ul>
      </section>
    {/if}
    {/if}
    </div>
    {/if}
  {/if}
</section>
{#if removeSingleGame && detail}
  <RemoveFilesSheet
    {card}
    coreId={detail.id}
    coreName={detail.shortname}
    files={removeSingleGame}
    on:close={() => (removeSingleGame = null)}
    on:done={() => {
      removeSingleGame = null;
      selectedGame = null;
      sideMode = 'core';
      void reload();
    }}
  />
{/if}
{#if gameArtTarget}
  <GameArtworkPickerSheet
    platformId={gameArtTarget.platformId}
    platformName={gameArtTarget.platformName}
    romName={gameArtTarget.romName}
    current={$gameArtMap[gameArtCacheKey(gameArtTarget.platformId, gameArtTarget.romName)] ?? null}
    close={() => (gameArtTarget = null)}
  />
{/if}
{#if showPlatArtPicker && node}
  <ArtworkPickerSheet
    {card}
    device="analogue_pocket"
    title={`Artwork — ${node.platform.name}`}
    platformId={node.platform.id}
    targetId={node.platform.id}
    currentUrl={banner}
    onApplied={(u) => { if (u) banner = u; }}
    close={() => (showPlatArtPicker = false)}
  />
{/if}
<style>
  .cores-note{font-size:13px;color:var(--text-muted)}
  .cp-actions{display:flex;align-items:center;gap:10px;flex:none}
  .cp-side-btn{display:inline-grid;place-items:center;width:34px;height:34px;padding:0;border-radius:9px;background:var(--bg-surface);border:1px solid var(--border);color:var(--text-muted);cursor:pointer}
  .cp-side-btn:hover{color:var(--text);border-color:var(--border-strong)}
  .cp-side-btn.active{color:var(--accent);border-color:var(--accent-soft-border);background:var(--accent-soft-bg)}
  .side-mode-bar{display:flex;align-items:center;gap:4px;margin:0 0 8px;padding:3px;background:var(--bg-input);border:1px solid var(--border);border-radius:9px}
  .side-mode-bar button:not(.side-mode-clear){flex:1;padding:5px 8px;font-size:12px;border-radius:7px;background:transparent;color:var(--text-muted)}
  .side-mode-bar button:not(.side-mode-clear).active{background:var(--bg-card);color:var(--accent)}
  .side-mode-clear{padding:4px 8px;font-size:11.5px;background:transparent;color:var(--text-muted);border-radius:6px}
  .side-mode-clear:hover{color:var(--text);background:var(--bg-card)}
  .dis-bar{display:flex;justify-content:space-between;align-items:center;gap:14px;flex-wrap:wrap;margin:0 0 16px;padding:12px 16px;border:1px solid var(--warn-border);background:var(--warn-bg);color:var(--warn-text);border-radius:10px;font-size:13px;line-height:1.5}.dis-bar-btns{display:flex;gap:8px}
  .dis-banner{display:flex;gap:14px;align-items:center}.dis-banner .dis-on{flex:none}
  .pv-list li.has-en{display:flex;align-items:center;gap:10px;padding-right:14px}.pv-list li.has-en .pv-open{flex:1;min-width:0}.pv-en{flex:none;white-space:nowrap}
  .dis-banner,.dis-warn{margin:0 0 12px;padding:10px 14px;border:1px solid var(--warn-border);background:var(--warn-bg);color:var(--warn-text);border-radius:10px;font-size:12.5px;line-height:1.55}.dis-warn button{margin-left:8px}
  .dis .dis-lede{font-size:12.5px;color:var(--text-muted);margin:0 0 10px}.dis-row{display:flex;align-items:center;gap:13px;padding:11px 16px}.dis-row .core-icon{opacity:.6}
  .dis::global(code){font-size:11.5px}
  /* A core's page is one screen tall: the header and tabs stay put, the content and the details column each scroll on their own. */
  .cores.has-core{flex:1;min-height:0;display:flex;flex-direction:column;overflow:hidden;max-width:none;padding:20px 28px 0}
  .cp-head{flex:none;display:flex;justify-content:space-between;align-items:flex-start;gap:16px}
  .cores-main{display:flex;flex-direction:column;min-height:0;min-width:0}.cores-main .tabs{flex:none}
  .cores-scroll{flex:1;min-height:0;overflow:auto;padding:2px 8px 36px 0}
  .cores-side{min-height:0;overflow:auto;padding:2px 2px 24px 0}
  .crumbs{display:flex;align-items:center;gap:2px;font-size:12px;color:var(--text-muted);margin:0 0 8px -8px}.crumbs .quiet{padding:4px 8px;font-size:12px}
  .cores-grid{flex:1;min-height:0;display:grid;grid-template-columns:minmax(0,1fr) 316px;gap:24px;align-items:stretch}.cores-grid.no-inspector{grid-template-columns:minmax(0,1fr)}
  .tabs{display:flex;gap:4px;border-bottom:1px solid var(--border);margin:0 0 16px}
  .tabs [role=tab]{position:relative;padding:10px 14px;font-size:13.5px;color:var(--text-secondary);background:transparent;border-radius:8px 8px 0 0}
  .tabs [role=tab]:hover{color:var(--text);background:var(--bg-card)}.tabs [role=tab].current{color:var(--accent);box-shadow:inset 0 -2px 0 var(--accent-solid)}
  .tabs [role=tab]:focus-visible{outline:2px solid var(--accent);outline-offset:-2px}
  .tab-dot{display:inline-block;width:7px;height:7px;border-radius:50%;background:var(--warn-text);margin-left:7px;vertical-align:middle}
  .plat-art-wrap{position:relative;width:min(521px,100%);margin:0 0 18px}
  .plat-banner{display:block;width:100%;border-radius:10px;background:#000}
  .plat-art-edit{position:absolute;right:10px;bottom:10px;padding:5px 11px;font-size:12px;background:color-mix(in srgb,var(--bg-surface) 92%,transparent);color:var(--text-strong);border:1px solid var(--border-strong);border-radius:8px;opacity:0;transition:opacity .15s ease}
  .plat-art-wrap:hover .plat-art-edit,.plat-art-wrap:focus-within .plat-art-edit{opacity:1}
  .plat-art-edit:hover{border-color:var(--accent);color:var(--accent)}
  .pv-list{list-style:none;margin:0;padding:0}.pv-list li{display:block;padding:0;border-bottom:1px solid var(--border)}.pv-list li:last-child{border:0}
  .pv-open{width:100%;display:flex;align-items:center;gap:13px;padding:13px 16px;background:transparent;color:inherit;text-align:left}.pv-open:hover{background:var(--bg-card)}.pv-open:focus-visible{outline:2px solid var(--accent);outline-offset:-2px}
  .pv-main{flex:1;min-width:0;display:grid;gap:3px}.pv-main strong{font-size:14px}.pv-main small{font-size:12px;color:var(--text-muted);overflow:hidden;text-overflow:ellipsis}.pv-ver{font-size:12.5px;color:var(--text-secondary)}
  .chip.warn{background:var(--warn-bg);color:var(--warn-text);border:1px solid var(--warn-border);font-weight:600}
  .pv-list li.pv-disabled{background:color-mix(in srgb,var(--warn-bg) 40%,transparent);box-shadow:inset 3px 0 0 var(--warn-border)}
  .pv-list li.pv-disabled .core-icon{opacity:.55;filter:grayscale(.6)}
  .pv-list li.pv-disabled .pv-main strong{color:var(--text-secondary)}
  .ov-bar{display:flex;flex-wrap:wrap;gap:10px;align-items:center;margin:0 0 12px}.ov-bar .quiet{flex:none;white-space:nowrap}.ov-search{display:block;flex:1 1 260px;min-width:0}.ov-search input{width:100%;box-sizing:border-box;font:inherit;font-size:13.5px;color:var(--text);background:var(--bg-input);border:1px solid var(--border-strong);border-radius:10px;padding:10px 14px}
  .ov-select{display:block;flex:0 1 auto;max-width:100%}.ov-select select{font:inherit;font-size:13px;color:var(--text);background:var(--bg-input);border:1px solid var(--border-strong);border-radius:10px;padding:10px 12px;max-width:100%}
  .sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
  .ov-tabs{margin-bottom:14px}.ov-n-chip{margin-left:7px;padding:1px 7px;font-size:11px}
  .ov-tags{display:flex;flex-wrap:wrap;gap:8px;margin:0 0 14px}
  .chip.toggle{cursor:pointer;border:1px solid transparent;padding:7px 12px;font-size:12px}
  .chip.toggle[aria-pressed="true"]{border-color:var(--accent);color:var(--accent-soft-text);background:var(--info-bg)}
  .ov-count{opacity:.65;font-weight:500;margin-left:4px}
  .ov-filter-note{display:flex;align-items:center;justify-content:space-between;gap:10px;flex-wrap:wrap;font-size:12px;color:var(--text-muted);margin:-4px 0 14px}
  .ov-clear{padding:4px 10px;font-size:12px}
  .ov-ic{display:inline-grid;place-items:center;width:30px;height:30px;border-radius:9px;background:var(--bg-card);color:var(--accent);margin-right:10px;vertical-align:middle}
  .ov-cat{margin:0 0 26px}.ov-cat h2{font-size:15px;margin:0 0 10px}.ov-cat h2 small{font-size:12px;font-weight:400;color:var(--text-muted);margin-left:6px}.ov-maker{font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:var(--text-secondary);margin:14px 0 8px}
  .ov-grid{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fill,minmax(min(236px,100%),1fr));gap:14px}
  .ov-card{width:100%;display:flex;flex-direction:column;padding:0;overflow:hidden;background:var(--bg-card);border:1px solid var(--border-subtle);border-radius:12px;color:inherit;text-align:left;transition:border-color .14s ease}.ov-card:hover{border-color:var(--border-strong)}.ov-card:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
  .ov-art{position:relative;display:block;width:100%;aspect-ratio:521/165;background:#000;overflow:hidden;border-bottom:1px solid var(--border-subtle)}
  .ov-art img{display:block;width:100%;height:100%;object-fit:cover;transition:opacity .15s ease,filter .15s ease}
  .ov-body{display:grid;gap:4px;padding:12px 14px 13px}
  .ov-title-row{display:flex;align-items:baseline;justify-content:space-between;gap:8px;min-width:0}
  .ov-title-row strong{font-size:14.5px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .ov-year{font-size:12px;color:var(--text-muted);flex:none}
  .ov-n{font-size:12px;color:var(--text-secondary);margin-top:2px}
  .ov-badge{position:absolute;top:8px;right:8px;display:inline-flex;align-items:center;gap:5px;padding:3px 8px;border-radius:999px;font-size:11px;font-weight:700;line-height:1.2;background:color-mix(in srgb,var(--warn-bg) 94%,#000);color:var(--warn-text);border:1px solid var(--warn-border);box-shadow:0 2px 6px rgba(0,0,0,.35)}
  .ov-card.has-disabled{border-color:color-mix(in srgb,var(--warn-border) 75%,var(--border))}
  .ov-card.all-disabled{border-style:dashed;border-color:var(--warn-border);background:color-mix(in srgb,var(--bg-card) 88%,var(--warn-bg))}
  .ov-card.all-disabled .ov-art img{opacity:.45;filter:grayscale(.8)}
  .ov-card.all-disabled .ov-title-row strong{color:var(--text-secondary)}
  .ov-dis-txt{color:var(--warn-text);font-weight:600}
  @media(max-width:1100px){.cores.has-core{overflow:visible}.cores-grid{grid-template-columns:minmax(0,1fr)}.cores-scroll,.cores-side{overflow:visible}}
</style>
