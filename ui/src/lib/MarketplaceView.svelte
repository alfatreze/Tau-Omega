<script lang="ts">
  import { onDestroy } from 'svelte';
  import { onFileDrop } from './dropTarget';
  import { openManualAdd } from './manualAddStore';
  import { onMount } from 'svelte';
  import CoreInstall from './CoreInstall.svelte';
  import Lightbox from './Lightbox.svelte';
  import MultiInstall from './MultiInstall.svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { platformFallbackArt } from './platformArt';
  import { gamebubDownloadCore, gamebubExecuteInstall, gamebubPlanInstall, getPrefs, marketplaceArt, marketplaceCatalog, marketplaceCoreUpdates, marketplaceGallery, marketplaceOpenRepo, marketplaceRepoDetails, setPrefs } from './tau-api';
  import { activeSystem, setActiveSystem, syncSystemForCard, systemsStore } from './systemStore';
  import { categoryCounts, filterCores, listedBy, size, sourceLabel, sourceNote, sourcesOf, statusLabel, statusMap, type Filters } from './marketplace';
  import { coreUpdateCount } from './marketStore';
  import type { CatalogCore, CatalogResult, CoreStatus, DeviceKind, GalleryItem, GameBubDownloaded, GameBubInstallPlan, GameBubInstallReport, MarketplaceDownload, RepoDetails } from './types';
  /** The card folder currently open (empty when none): used for the installed/update chips and for installing. */
  export let card = '';
  /** Opens an installed core's page in Cores. */
  export let openCore: (id: string) => void = () => {};

  let gbDownload: GameBubDownloaded | null = null;
  let gbPlan: GameBubInstallPlan | null = null;
  let gbReport: GameBubInstallReport | null = null;
  let gbError = '';
  let gbBusy = false;
  let gbPhase: '' | 'downloading' | 'installing' = '';
  $: if (card) void syncSystemForCard(card);
  const coreDevice = (c: CatalogCore): DeviceKind => c.device ?? 'analogue_pocket';
  const cardShortName = (p: string) => p.split('/').filter(Boolean).pop() ?? p;
  $: isGameBubCore = !!selected && coreDevice(selected) === 'game_bub';
  $: { void selected?.id; gbDownload = null; gbPlan = null; gbReport = null; gbError = ''; gbBusy = false; gbPhase = ''; }

  async function startGameBubInstall(installAll = false) {
    if (!selected || !card) return;
    gbBusy = true; gbPhase = 'downloading'; gbError = ''; gbReport = null; gbPlan = null;
    try {
      const dl = await gamebubDownloadCore(selected.id);
      gbDownload = dl;
      const only = installAll ? null : selected.id;
      gbPlan = await gamebubPlanInstall(card, dl.path, only, selected.asset_sha256 || null);
    } catch (e) {
      gbError = errorMessage(e);
    } finally {
      gbBusy = false; gbPhase = '';
    }
  }

  async function confirmGameBubInstall(installAll = false) {
    if (!selected || !card || !gbDownload || !gbPlan) return;
    gbBusy = true; gbPhase = 'installing'; gbError = '';
    try {
      const only = installAll ? null : selected.id;
      const rep = await gamebubExecuteInstall(card, gbDownload.path, only, selected.asset_sha256 || null, gbPlan.id);
      gbReport = rep;
      gbPlan = null;
      gbDownload = null;
      void loadStatuses(card);
      window.dispatchEvent(new CustomEvent('tau-cores-changed'));
    } catch (e) {
      gbError = errorMessage(e);
    } finally {
      gbBusy = false; gbPhase = '';
    }
  }

  let result: CatalogResult | null = null; let loading = false; let error = '';
  let filters: Filters = { query: '', category: '', installedOnly: false, sort: 'newest', source: '' };
  let statuses = new Map<string, CoreStatus>();
  let searchEl: HTMLInputElement;
  let selected: CatalogCore | null = null;
  /** The version chosen in the history ('' = the newest). */
  let chosen = '';
  /** For a card with install choices (Tau): the channel, build and version picked in the dropdowns. */
  let optChannel = ''; let optDiag = false; let optVersion = '';
  $: opts = selected?.options ?? [];
  $: channels = [...new Set(opts.map((o) => o.channel))];
  $: builds = opts.filter((o) => o.channel === optChannel);
  $: option = opts.find((o) => o.channel === optChannel && o.diagnostics === optDiag) ?? builds[0] ?? null;
  /** The Install button sits in the header; the review it leads to is drawn by CoreInstall below it. */
  let installer: CoreInstall | undefined; let installBusy = false; let review: MarketplaceDownload | null = null;
  $: isDev = opts.length ? option?.channel === 'Dev' : selected?.channel === 'Dev';
  $: target = isDev ? null : opts.length ? (option ? { id: option.core_id, version: optVersion } : null) : selected?.download_url ? { id: selected.id, version: chosen } : null;
  $: canInstall = !!card && !!target;
  /** A different core, channel or version starts the review over. */
  $: { void target?.id; void target?.version; review = null; installBusy = false; }
  $: installLabel = target?.version ? `Install version ${target.version}` : 'Install';
  /** This core is on the open card: no Install button, just its version (or Update when a newer one is listed) and a link to it. */
  $: have = selected && card ? statusOf(selected) : undefined;
  $: haveInstalled = !!have && typeof have.status === 'string' && ['UpToDate', 'UpdateAvailable', 'NewerThanCatalog', 'Unknown'].includes(have.status);
  $: haveUpdate = haveInstalled && have?.status === 'UpdateAvailable';
  $: haveCurrent = haveInstalled && !haveUpdate;
  $: haveVersion = have?.installed_version ? (/^v/i.test(have.installed_version) ? have.installed_version : `v${have.installed_version}`) : 'installed';
  $: updateLabel = target?.version ? installLabel : have?.latest_version ? `Update to ${have.latest_version}` : 'Update';
  function goToCore() { if (!have) return; const id = have.id; close(); openCore(id); }
  $: installNote = isDev ? 'Dev builds are made locally and never published: install one from its zip or folder.' : !card ? 'Open a card first (Cards page).' : !target ? 'The author has not published a download yet.' : '';
  $: metaLine = selected ? [selected.category, selected.manufacturer, selected.year ? String(selected.year) : ''].filter(Boolean).join(' · ') : '';
  const pickChannel = (c: string) => { optChannel = c; optDiag = false; optVersion = ''; };
  /** Pictures by `kind:id`: undefined = not asked yet, null = the library has none. Fetched only for rows that are shown. */
  let art: Record<string, string | null | undefined> = {}; const asking = new Set<string>(); let queue: string[] = [];
  let announce = '';
  /** Cores ticked for one installation together (only cores with a download, and only with a card open). */
  let picked = new Set<string>(); let multi = false;
  const toggle = (id: string) => { picked.has(id) ? picked.delete(id) : picked.add(id); picked = new Set(picked); };
  $: if (!card) picked = new Set();
  $: pickedCores = cores.filter((c) => picked.has(c.id)).map((c) => ({ id: c.id, name: c.name }));

  $: allCores = result?.catalog.cores ?? [];
  $: hasGameBub = allCores.some((c) => coreDevice(c) === 'game_bub');
  $: cores = allCores.filter((c) => coreDevice(c) === $activeSystem);
  $: installed = new Set([...statuses.keys()]);
  $: sources = sourcesOf(cores);
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
  /** Which marketplace is shown (and checked for updates) when a core has several; a strictly newer listing elsewhere still wins. */
  let defaultSource = 'openfpga-library';
  async function setDefaultSource(value: string) {
    defaultSource = value;
    try { const p = await getPrefs(); await setPrefs({ ...p, marketplace_default_source: value }); } catch { /* the choice stays for this session */ }
    await load(false); void loadStatuses(card);
  }
  onMount(() => { void load(false); getPrefs().then((p) => { defaultSource = p.marketplace_default_source || 'openfpga-library'; }, () => {}); });

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

  /** The details sheet: `About` says what the core is, `Details` holds the technical facts and requirements. */
  let tab: 'about' | 'details' = 'about';
  /** What GitHub says about the open core (null until it answers or when it cannot be reached). */
  let repo: RepoDetails | null = null; let repoState: 'loading' | 'done' | 'failed' = 'loading'; let linkError = '';
  let repoFor = '';
  /** The pictures the author's README shows on GitHub (empty when there are none or GitHub cannot be reached) and the one open in the lightbox. */
  let gallery: GalleryItem[] = []; let galleryLoading = false; let lightbox: number | null = null;
  async function loadGallery(core: CatalogCore) {
    gallery = []; lightbox = null; galleryLoading = true;
    try { const g = await marketplaceGallery(core.id); if (repoFor === core.id) gallery = g; }
    catch { /* no pictures is fine */ }
    finally { if (repoFor === core.id) galleryLoading = false; }
  }
  async function loadRepo(core: CatalogCore) {
    repo = null; repoState = 'loading'; repoFor = core.id;
    try { const d = await marketplaceRepoDetails(core.id); if (repoFor === core.id) { repo = d; repoState = 'done'; } }
    catch { if (repoFor === core.id) repoState = 'failed'; }
  }
  function open(core: CatalogCore) { selected = core; chosen = ''; optVersion = ''; optDiag = false; tab = 'about'; linkError = ''; review = null; installBusy = false; optChannel = core.options?.[0]?.channel ?? ''; wantArt('Banner', core.id); wantArt('Icon', core.id); void loadRepo(core); void loadGallery(core); }
  function close() { selected = null; repoFor = ''; gallery = []; galleryLoading = false; lightbox = null; }
  const repoUrl = (c: CatalogCore) => `https://github.com/${c.repository.owner}/${c.repository.name}`;
  /** The address is opened by the app from the catalog entry (a plain click on the link; nothing typed or pasted is ever opened). */
  async function openRepo(e: Event) { e.preventDefault(); if (!selected) return; linkError = ''; try { await marketplaceOpenRepo(selected.id); } catch (err) { linkError = errorMessage(err); } }
  const count = (n: number) => n.toLocaleString('en-US');
  function tabKey(e: KeyboardEvent) {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
    e.preventDefault(); tab = tab === 'about' ? 'details' : 'about';
    queueMicrotask(() => document.getElementById(`mk-tab-${tab}`)?.focus());
  }
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
  // Lock background page scroll while the details sheet is open so only the sheet's own scrollbar shows.
  $: if (typeof document !== 'undefined') {
    const lock = selected ? 'hidden' : '';
    document.documentElement.style.overflow = lock;
    document.body.style.overflow = lock;
  }
  onDestroy(() => {
    offDrop();
    if (typeof document !== 'undefined') {
      document.documentElement.style.overflow = '';
      document.body.style.overflow = '';
    }
  });
</script>
<svelte:window on:keydown={onKey} />
<section class="page" aria-labelledby="market-title">
  <header>
    <div>
      <p class="eyebrow">COMMUNITY CORES</p>
      <h1 id="market-title">Marketplace</h1>
      <p class="lede">
        {#if $activeSystem === 'game_bub'}
          Community cores for Game Bub (Beta · community-tested, FAT32 microSD). Nothing is installed until you review it.
        {:else}
          Cores made by the community for the Analogue Pocket. Nothing is installed until you review it.
        {/if}
      </p>
    </div>
    <span class="mk-head-actions"><button class="quiet" on:click={() => openManualAdd()}>Add a core manually…</button><button class="quiet" disabled={loading} on:click={() => load(true)}>{loading ? 'Checking…' : 'Refresh'}</button></span>
  </header>

  <div class="mk-tags" role="group" aria-label="Device" data-testid="mk-device-switch">
    {#each $systemsStore as sys (sys.id)}
      <button
        class="chip toggle"
        aria-pressed={$activeSystem === sys.id}
        data-testid={sys.id === 'game_bub' ? 'mk-device-gamebub' : `mk-device-${sys.id}`}
        on:click={() => { setActiveSystem(sys.id); filters = { ...filters, category: '', source: '' }; }}
      >{sys.name}{#if sys.id === 'game_bub' && !hasGameBub && result} <span class="mk-count">0</span>{/if}</button>
    {/each}
  </div>

  <div class="mk-filters" role="search" aria-label="Filter cores">
    <label class="mk-search"><span class="sr">Search cores</span>
      <input bind:this={searchEl} type="search" placeholder="Search by name, author or system   ( / )" bind:value={filters.query} />
    </label>
    <label><span class="sr">Sort</span>
      <select bind:value={filters.sort}><option value="newest">Newest release</option><option value="name">Name</option></select>
    </label>
    {#if sources.length > 1}<label class="mk-default"><span class="sr">Marketplace to check for updates</span><span aria-hidden="true">Updates from</span>
      <select value={defaultSource} on:change={(e) => setDefaultSource(e.currentTarget.value)} title="The marketplace used when a core is listed by several. A newer version in another one is still offered.">{#each sources as src}<option value={src}>{sourceLabel(src)}</option>{/each}</select></label>{/if}
    <button class="chip toggle" aria-pressed={filters.installedOnly} disabled={!card} title={card ? '' : 'Open a card to filter by what it has'} on:click={() => (filters.installedOnly = !filters.installedOnly)}>On this card</button>
  </div>
  {#if sources.length > 1}
    <div class="mk-tags" role="group" aria-label="Source">
      <button class="chip toggle" aria-pressed={!filters.source} on:click={() => (filters.source = '')}>All sources</button>
      {#each sources as src}<button class="chip toggle" aria-pressed={filters.source === src} on:click={() => (filters.source = filters.source === src ? '' : src)}>{sourceLabel(src)}</button>{/each}
    </div>
  {/if}
  {#if result}
  <div class="mk-tags" role="group" aria-label="Category">
    <button class="chip toggle" aria-pressed={!filters.category} on:click={() => (filters.category = '')}>All <span class="mk-count">{counts.all}</span></button>
    {#each counts.byCategory as [name, n]}<button class="chip toggle" aria-pressed={filters.category === name} on:click={() => toggleCategory(name)}>{name} <span class="mk-count">{n}</span></button>{/each}
  </div>
  {/if}
  <p class="mk-note" role="status" aria-live="polite">{#if error && result}<span class="err">{error}</span>{:else if error}{:else if loading && !result}Loading the list…{:else}{announce}{#if note}{' · '}{note}{/if}{#each result?.notes ?? [] as n}{' · '}{n}{/each}{/if}</p>

  {#if picked.size}<div class="mk-bar" role="region" aria-label="Selected cores"><span>{picked.size} {picked.size === 1 ? 'core' : 'cores'} selected</span><span><button class="quiet" on:click={() => (picked = new Set())}>Clear</button> <button class="primary" on:click={() => (multi = true)}>Install {picked.size} together…</button></span></div>{/if}
  {#if error && !result}
    <div class="empty"><h2>The list could not be loaded</h2><p>{error}</p><button class="primary" on:click={() => load(true)}>Try again</button></div>
  {:else if loading && !result}
    <!-- Skeleton cards: the same grid and card proportions as the real list, so nothing jumps when it arrives. Hidden from
         screen readers; the "Loading the list…" status above is what they hear. -->
    <ul class="mk-grid" aria-hidden="true" data-testid="mk-skeleton">
      {#each Array(9) as _}
        <li class="mk-card mk-skel">
          <span class="mk-art sk"></span>
          <span class="mk-body"><span class="sk sk-title"></span><span class="sk sk-sub"></span>
            <span class="mk-author"><span class="sk sk-avatar"></span><span class="sk sk-name"></span></span></span>
          <div class="mk-foot"><span class="sk sk-chip"></span><span class="sk sk-ver"></span></div>
        </li>
      {/each}
    </ul>
  {:else if result && !shown.length}
    <div class="empty"><h2>No cores match</h2><p>{filters.query ? `Nothing matches “${filters.query}”` : 'Nothing matches these filters'}{filters.category ? ` in ${filters.category}` : ''}{filters.installedOnly ? ' on this card' : ''}.</p>
      <button class="quiet" on:click={() => (filters = { ...filters, query: '', category: '', installedOnly: false, source: '' })}>Clear filters</button></div>
  {:else if result}
    <ul class="mk-grid" aria-busy={loading} aria-label="Cores">
      {#each shown as item (item.id)}
        <li class="mk-card" use:visible={item.id}>
          <button class="mk-open" on:keydown={cardKey} aria-label={`${item.name}, ${item.category}, version ${item.version || 'unknown'}. Show details`} on:click={() => open(item)}>
            <span class="mk-art" aria-hidden="true">
              <img src={art[`Banner:${item.id}`] || platformFallbackArt(item)} alt="" class:mk-art-gen={!art[`Banner:${item.id}`]} />
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
            {#if card && item.download_url && !item.options?.length}<label class="mk-pick"><input type="checkbox" checked={picked.has(item.id)} on:change={() => toggle(item.id)} aria-label={`Select ${item.name} to install together`} /> Select</label>{/if}
            {#if item.category}<button class="chip toggle mk-tag" aria-pressed={filters.category === item.category} aria-label={`Show only ${item.category}`} on:click={() => toggleCategory(item.category)}>{item.category}</button>{/if}
            {#if item.options?.length}{#each [...new Set(item.options.map((o) => o.channel))] as ch}<span class="chip">{ch}</span>{/each}{:else if item.channel}<span class="chip">{item.channel}</span>{/if}
            {#if chip(item)}<span class="chip" class:capable={statusOf(item)?.status === 'UpdateAvailable'}>{chip(item)}</span>{/if}
            <span class="mk-ver">{item.version || '—'}<small>{item.release_date}</small></span>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

{#if multi}<MultiInstall {card} device={$activeSystem} cores={pickedCores} close={() => { multi = false; }} installed={() => { picked = new Set(); void loadStatuses(card); }} />{/if}
{#if selected}
  <div class="mk-backdrop" role="presentation" on:click={close}></div>
  <div class="mk-sheet" role="dialog" aria-modal="true" aria-labelledby="mk-title" use:modal>
    <div class="mk-x-wrap"><button class="mk-x" data-autofocus on:click={close} aria-label="Close details"><svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M3 3l10 10M13 3L3 13" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" fill="none" /></svg></button></div>
    <img class="mk-banner" src={art[`Banner:${selected.id}`] || platformFallbackArt(selected)} alt={`${selected.name} platform artwork`} />
    {#if gallery.length}
      <ul class="mk-strip" aria-label={`Pictures of ${selected.name} from GitHub`}>
        {#each gallery as g, i}<li><button class="mk-thumb" aria-label={`View picture ${i + 1} of ${gallery.length}${g.alt ? `: ${g.alt}` : ''}`} on:click={() => (lightbox = i)}><img src={g.src} alt="" loading="lazy" /></button></li>{/each}
      </ul>
    {:else if galleryLoading}
      <div class="mk-loading-row" role="status" aria-busy="true">
        <span class="mk-spin" aria-hidden="true"></span>
        <span>Checking GitHub for screenshots…</span>
      </div>
    {/if}
    <header class="mk-sheet-head">
      <div class="mk-head-main">
        <h2 id="mk-title">{selected.name}</h2>
        <p class="mk-dev"><span class="mk-avatar" aria-hidden="true">{#if art[`Icon:${selected.id}`]}<img class="mk-icon" src={art[`Icon:${selected.id}`]} alt="" width="28" height="28" />{:else}{selected.author.slice(0, 1).toUpperCase()}{/if}</span><span class="mk-author-name">{selected.author}</span></p>
        {#if metaLine}<p class="mk-by">{metaLine}</p>{/if}
      </div>
      {#if isDev}
        <button class="primary mk-go" on:click={() => { close(); openManualAdd(); }}>Install a local build…</button>
      {:else if haveCurrent}
        <div class="mk-have" data-testid="mk-have">
          <span class="chip capable">Installed · {haveVersion}</span>
          <button class="mk-have-link" on:click={goToCore}>View on card</button>
        </div>
      {:else if isGameBubCore}
        <button class="primary mk-go" disabled={!card || gbBusy || !!gbPlan} on:click={() => startGameBubInstall(false)} data-testid="gb-install-btn">{gbBusy ? 'Working…' : haveUpdate ? updateLabel : installLabel}</button>
      {:else}
        <button class="primary mk-go" disabled={!canInstall || installBusy || !!review} on:click={() => installer?.start()}>{installBusy ? 'Working…' : haveUpdate ? updateLabel : installLabel}</button>
      {/if}
    </header>

    <div class="mk-install">
      {#if isGameBubCore}
        <p class="mk-callout soft" role="note">Game Bub support is community-tested beta. Requires a FAT32 (MBR) microSD card{#if selected.firmware_required}{' '}and firmware {selected.firmware_required}+{/if}.</p>
        {#if !card}<p class="mk-small">Open a Game Bub card first (Cards page).</p>{/if}
        {#if gbError}<p class="err" role="alert">{gbError}</p>{/if}
        {#if gbBusy && gbPhase === 'downloading'}
          <div class="mk-loading-row" role="status" aria-busy="true"><span class="mk-spin" aria-hidden="true"></span><span>Downloading and verifying archive…</span></div>
        {/if}
        {#if gbPlan}
          {@const pc = gbPlan.cores[0]}
          <div class="gb-plan-card" data-testid="gb-plan">
            <div class="gb-plan-head">
              <div>
                <strong>Install {selected.name} on {cardShortName(card)}</strong>
                <p class="mk-small">
                  {#if gbDownload}Downloaded {size(gbDownload.bytes)} · SHA-256 verified · {/if}
                  {gbPlan.files_to_write.length} {gbPlan.files_to_write.length === 1 ? 'file' : 'files'} to write under <code>/cores/</code>
                  {#if gbPlan.backup_files.length} · {gbPlan.backup_files.length} existing backed up first{/if}
                </p>
              </div>
              {#if pc}
                <span class="chip" class:capable={pc.action === 'new' || pc.action === 'update'}>
                  {pc.previous_version ? `Update from ${pc.previous_version}` : pc.action === 'new' ? 'New' : pc.action}
                </span>
              {/if}
            </div>
            {#if gbDownload && gbDownload.bundled_core_ids.length > 1}
              <p class="mk-small">This archive bundles {gbDownload.bundled_core_ids.length} cores: {gbDownload.bundled_core_ids.join(', ')}.</p>
            {/if}
            {#each gbPlan.warnings as w}<p class="gb-warn">{w}</p>{/each}
            <details class="gb-details">
              <summary>Details ({gbPlan.files_to_write.length} files)</summary>
              <ul>
                {#each gbPlan.files_to_write as f}<li><code>{f}</code></li>{/each}
              </ul>
            </details>
            <div class="gb-plan-btns">
              <button class="primary" disabled={gbBusy} on:click={() => confirmGameBubInstall(false)} data-testid="gb-confirm-btn">{gbBusy ? 'Installing…' : 'Install'}</button>
              <button class="quiet" disabled={gbBusy} on:click={() => { gbPlan = null; gbDownload = null; }}>Cancel</button>
            </div>
          </div>
        {/if}
        {#if gbReport}
          <div class="gb-done-card" role="status" data-testid="gb-done">
            <div class="gb-plan-head">
              <div>
                <strong>Installed {selected.name}</strong>
                <p class="mk-small">
                  {gbReport.files_written} {gbReport.files_written === 1 ? 'file' : 'files'} written and verified ({gbReport.cores_installed.join(', ')})
                  {#if gbReport.backup_dir} · previous files backed up{/if}
                </p>
              </div>
              <span class="chip capable">Verified</span>
            </div>
          </div>
        {/if}
      {:else}
        {#if opts.length}
          <div class="mk-opts">
            <label>Channel<select value={optChannel} on:change={(e) => pickChannel(e.currentTarget.value)}>{#each channels as c}<option value={c}>{c}</option>{/each}</select></label>
            {#if builds.length > 1}<label>Build<select value={optDiag ? 'diag' : 'std'} on:change={(e) => { optDiag = e.currentTarget.value === 'diag'; optVersion = ''; }}><option value="std">Standard</option><option value="diag">Diagnostics</option></select></label>{/if}
            {#if option && option.history.length}<label>Version<select bind:value={optVersion}>{#each option.history as h, i}<option value={i === 0 ? '' : h.version}>{h.version} · {h.release_date}{i === 0 ? ' (newest)' : ''}</option>{/each}</select></label>{/if}
          </div>
        {/if}
        {#if installNote}<p class="mk-small">{installNote}</p>{/if}
        {#if canInstall}{#key `${target?.id}|${target?.version}`}<CoreInstall bind:this={installer} bind:busy={installBusy} bind:download={review} coreId={target?.id ?? ''} {card} version={target?.version ?? ''} buttonless on:installed={() => loadStatuses(card)} />{/key}{/if}
        {#if selected.requires_license}<p class="mk-callout soft" role="note">This core needs a key file from its author, or it will not run. See Details.</p>{/if}
      {/if}
    </div>

    <div class="mk-tabs" role="tablist" aria-label="About this core" tabindex="-1" on:keydown={tabKey}>
      <button id="mk-tab-about" role="tab" aria-selected={tab === 'about'} aria-controls="mk-panel" tabindex={tab === 'about' ? 0 : -1} on:click={() => (tab = 'about')}>About</button>
      <button id="mk-tab-details" role="tab" aria-selected={tab === 'details'} aria-controls="mk-panel" tabindex={tab === 'details' ? 0 : -1} on:click={() => (tab = 'details')}>Details</button>
    </div>

    <div id="mk-panel" role="tabpanel" aria-labelledby={`mk-tab-${tab}`}>
    {#if tab === 'about'}
      {#if repoState === 'loading'}
        {#if selected.description}<p class="mk-desc">{selected.description}</p>{/if}
        <div class="mk-loading-block" role="status" aria-busy="true">
          <div class="mk-loading-row"><span class="mk-spin" aria-hidden="true"></span><span>Loading details from GitHub…</span></div>
          {#if !selected.description}<span class="sk mk-sk-line"></span><span class="sk mk-sk-line short"></span>{/if}
        </div>
      {:else if repo?.info?.description}<p class="mk-desc">{repo.info.description}</p>
      {:else if selected.description}<p class="mk-desc">{selected.description}</p>
      {:else if repoState === 'failed' || !repo?.info}<p class="mk-small">The description could not be loaded from GitHub. Check your connection.</p>
      {:else}<p class="mk-small">The author has not written a description on GitHub.</p>{/if}

      <dl class="mk-facts">
        <div><dt>Newest version</dt><dd>{selected.version || 'none published'}{selected.release_date ? ` · ${selected.release_date}` : ''}</dd></div>
        <div><dt>Downloads</dt><dd>{#if repoState === 'loading'}<span class="sk mk-sk-inline" aria-label="Loading downloads"></span>{:else if repo?.downloads}{repo.downloads.complete ? '' : 'At least '}{count(repo.downloads.total)} in total{#if repo.downloads.newest_tag}{' · '}{count(repo.downloads.newest)} for {repo.downloads.newest_tag}{/if}{:else}not available{/if}</dd></div>
        {#if repo?.info?.license || selected.license}<div><dt>Licence</dt><dd>{repo?.info?.license || selected.license}</dd></div>{/if}
        <div><dt>Source</dt><dd><a class="mk-link" href={repoUrl(selected)} on:click={openRepo}>{selected.repository.owner}/{selected.repository.name} on GitHub</a>{#if linkError}<span class="err"> {linkError}</span>{/if}</dd></div>
        {#if statusOf(selected) && chip(selected)}<div><dt>On your card</dt><dd>{statusOf(selected)?.installed_version} · {chip(selected)}</dd></div>{/if}
      </dl>

      {#if repoState === 'loading'}
        <div class="mk-loading-block" aria-hidden="true">
          <span class="sk mk-sk-head"></span>
          <span class="sk mk-sk-line"></span>
          <span class="sk mk-sk-line short"></span>
        </div>
      {:else if repo?.downloads?.notes}<h3>What’s new in {repo.downloads.newest_tag}</h3><p class="mk-notes">{repo.downloads.notes}</p>{/if}
      {#if selected.sponsor.links.length}
        <p class="mk-small">Support the author: {#each selected.sponsor.links as l, i}<code>{l.replace('https://', '')}</code>{i < selected.sponsor.links.length - 1 ? ', ' : ''}{/each}</p>
      {/if}
    {:else}
      <h3>Requirements</h3>
      <dl class="mk-reqs">
        <div><dt>Pocket firmware</dt><dd>{selected.firmware_required ? `${selected.firmware_required} or newer` : 'not stated'}</dd></div>
        <div><dt>Dock</dt><dd>{yesNo(selected.dock_supported)}</dd></div>
        <div><dt>Link port</dt><dd>{yesNo(selected.link_port)}</dd></div>
        <div><dt>Sleep</dt><dd>{yesNo(selected.sleep_supported)}</dd></div>
        {#if selected.requires_license}
          <div class="mk-req-key"><dt>Key file</dt><dd><code>{selected.license_filename || 'a file the author names'}</code> is not in the download: get it from the author (see the link under About) and put it in <code>Cores/{selected.id}/</code> on the card. The core will not run without it.</dd></div>
        {/if}
        {#if selected.assets.length}
          <div><dt>Your own files</dt><dd>{#each selected.assets as a, i}<code>Assets/{a.platform}/{a.core_specific ? selected.id : 'common'}</code>{#if a.extensions.length} ({a.extensions.map((x) => '.' + x).join(' ')}){/if}{i < selected.assets.length - 1 ? ', ' : ''}{/each}</dd></div>
        {/if}
      </dl>

      <h3>About the core</h3>
      <dl class="mk-reqs">
        <div><dt>Core id</dt><dd><code>{selected.id}</code></dd></div>
        {#if selected.category}<div><dt>Category</dt><dd>{selected.category}</dd></div>{/if}
        {#if selected.manufacturer}<div><dt>Original maker</dt><dd>{selected.manufacturer}{selected.year ? ` · ${selected.year}` : ''}</dd></div>{/if}
        <div><dt>{listedBy(selected).length > 1 ? 'Distributed by' : 'Listed by'}</dt><dd class="mk-listed">{#if selected.listings?.length}{#each selected.listings as l}<span class="chip">{sourceLabel(l.source)}{l.version ? ` · ${l.version}` : ''}</span>{/each}{:else}<span class="chip">{sourceLabel(selected.source)}</span>{/if}</dd></div>
        {#if repoState === 'loading'}<div><dt>GitHub</dt><dd><span class="mk-loading-row"><span class="mk-spin" aria-hidden="true"></span><span>Loading topics and activity…</span></span></dd></div>{/if}
        {#if repo?.info?.topics.length}<div><dt>Topics</dt><dd class="mk-listed">{#each repo.info.topics as t}<span class="chip">{t}</span>{/each}</dd></div>{/if}
        {#if repo?.info?.pushed}<div><dt>Repository updated</dt><dd>{repo.info.pushed}</dd></div>{/if}
      </dl>

      {#if selected.history.length > 1 && !opts.length}
        <details class="mk-history"><summary>Earlier versions ({selected.history.length - 1})</summary>
          <ul>{#each selected.history.slice(1) as h}<li><span>{h.version} · {h.release_date}</span>{#if card && selected.download_url}<button class="quiet" aria-pressed={chosen === h.version} on:click={() => (chosen = chosen === h.version ? '' : h.version)}>{chosen === h.version ? 'Use the newest instead' : 'Choose this version'}</button>{/if}</li>{/each}</ul>
          <p class="mk-small">An older version replaces a newer one on the card only if you confirm the downgrade in the plan. Your files are kept, as with any update.</p>
        </details>
      {/if}
    {/if}
    </div>
  </div>
  {#if lightbox !== null}<Lightbox items={gallery} index={lightbox} label={`Pictures of ${selected.name}`} on:close={() => (lightbox = null)} />{/if}
{/if}
<style>
  .sk{display:block;background:var(--bg-hover);border-radius:6px;position:relative;overflow:hidden}
  .sk::after{content:"";position:absolute;inset:0;transform:translateX(-100%);background:linear-gradient(90deg,transparent,color-mix(in srgb,var(--accent) 10%,transparent),transparent);animation:sk-sweep 1.4s ease-in-out infinite}
  .mk-art.sk{border-radius:0;background:var(--bg-card)}
  .sk-title{height:14px;width:62%}.sk-sub{height:11px;width:40%;margin-top:6px}
  .sk-avatar{flex:none;width:28px;height:28px;border-radius:7px}.sk-name{height:11px;width:34%}
  .sk-chip{height:22px;width:74px;border-radius:999px}.sk-ver{height:12px;width:52px;margin-left:auto}
  .mk-skel{pointer-events:none}
  @keyframes sk-sweep{to{transform:translateX(100%)}}
  @media (prefers-reduced-motion:reduce){.sk::after{animation:none;display:none}}
  .mk-opts{display:grid;gap:10px;margin:6px 0 12px}.mk-opts label{display:grid;gap:4px;font-size:12px;color:var(--text-muted)}.mk-opts select{font:inherit;font-size:13px;color:var(--text);background:var(--bg-input);border:1px solid var(--border-strong);border-radius:8px;padding:10px 12px}
  .mk-default{display:inline-flex;align-items:center;gap:8px;font-size:12px;color:var(--text-muted)}.mk-default select{font:inherit;font-size:13px;color:var(--text);background:var(--bg-input);border:1px solid var(--border-strong);border-radius:8px;padding:10px 12px}
  .mk-listed{display:flex;flex-wrap:wrap;gap:6px}
  .mk-pick{font-size:12px;color:var(--text-secondary);display:inline-flex;gap:5px;align-items:center;margin-right:auto}
  .mk-bar{display:flex;justify-content:space-between;align-items:center;gap:12px;position:sticky;top:0;z-index:5;background:var(--info-bg);border:1px solid var(--border-strong);border-radius:10px;padding:8px 14px;margin:0 0 10px;font-size:13px}
  .sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
  .mk-filters{display:flex;flex-wrap:wrap;gap:10px;align-items:center;margin:0 0 8px}
  .mk-filters input,.mk-filters select{font:inherit;font-size:13px;color:var(--text);background:var(--bg-input);border:1px solid var(--border-strong);border-radius:8px;padding:10px 12px}
  .mk-search{flex:1 1 280px}.mk-search input{width:100%}
  .chip.toggle{cursor:pointer;border:1px solid transparent;padding:8px 12px;font-size:12px}
  .chip.toggle[aria-pressed="true"]{border-color:var(--accent);color:var(--accent-soft-text);background:var(--info-bg)}
  .chip.toggle:disabled{opacity:.45;cursor:not-allowed}
  .mk-note{font-size:12px;color:var(--text-muted);min-height:18px;margin:0 0 10px}
  .err{color:var(--danger-text)}
  .mk-tags{display:flex;flex-wrap:wrap;gap:8px;margin:0 0 10px}
  .mk-count{opacity:.65;font-weight:500;margin-left:4px}
  .mk-grid{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fill,minmax(270px,1fr));gap:16px}
  .mk-card{display:flex;flex-direction:column;background:var(--bg-surface);border:1px solid var(--border);border-radius:12px;overflow:hidden}
  .mk-card:hover{border-color:var(--border-strong)}
  .mk-open{display:flex;flex-direction:column;flex:1;background:transparent;color:inherit;text-align:left;padding:0;border-radius:0}
  .mk-open:focus-visible{outline:2px solid var(--accent);outline-offset:-2px}
  .mk-art{display:block;width:100%;aspect-ratio:521/165;background:#000;overflow:hidden}
  .mk-art img{display:block;width:100%;height:100%;object-fit:cover}
  .mk-art img.mk-art-gen{object-fit:cover}
  .mk-loading-row{display:inline-flex;align-items:center;gap:8px;font-size:12px;color:var(--text-muted);margin:4px 0}
  .mk-loading-block{display:grid;gap:7px;margin:8px 0 12px}
  .mk-spin{width:13px;height:13px;border-radius:50%;border:2px solid var(--border);border-top-color:var(--accent);animation:mk-spin .75s linear infinite;flex:none}
  .mk-sk-head{display:block;height:13px;width:36%;border-radius:6px}
  .mk-sk-line{display:block;height:11px;width:92%;border-radius:5px}
  .mk-sk-line.short{width:64%}
  .mk-sk-inline{display:inline-block;height:12px;width:120px;border-radius:5px;vertical-align:middle}
  @keyframes mk-spin{to{transform:rotate(360deg)}}
  .mk-body{display:flex;flex-direction:column;gap:4px;padding:12px 14px 10px}
  .mk-title{font-size:15px;line-height:1.3;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .mk-sub{font-size:12px;color:var(--text-muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .mk-author{display:flex;align-items:center;gap:8px;margin-top:6px;min-width:0}
  .mk-avatar{flex:none;width:28px;height:28px;display:grid;place-items:center;border-radius:7px;background:var(--bg-elevated);color:var(--accent);font-weight:700;font-size:12px;overflow:hidden}
  .mk-author-name{font-size:12.5px;color:var(--text-secondary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .mk-foot{display:flex;flex-wrap:wrap;align-items:center;gap:6px;padding:9px 14px 12px;border-top:1px solid var(--border);background:var(--bg-subtle)}
  .mk-tag{padding:4px 10px;font-size:11px}
  .mk-ver{margin-left:auto;font-size:12px;color:var(--text-secondary);display:flex;flex-direction:column;align-items:flex-end;line-height:1.3}.mk-ver small{font-size:11px;color:var(--text-muted)}
  .mk-icon{display:block;width:28px;height:28px;border-radius:7px;image-rendering:pixelated}
  .mk-banner{display:block;width:100%;height:auto;border-radius:10px;margin:0 0 16px;background:#000}
  .mk-reqs{display:grid;gap:8px;margin:6px 0 4px}.mk-reqs>div{display:grid;grid-template-columns:130px 1fr;gap:10px;align-items:baseline}.mk-reqs dt{font-size:12px;color:var(--text-muted)}.mk-reqs dd{margin:0;font-size:12.5px;color:var(--text);overflow-wrap:anywhere;line-height:1.5}
  .mk-req-key{border:1px solid var(--warn-border);background:var(--warn-bg);border-radius:9px;padding:9px 11px}.mk-req-key dt{color:var(--warn-text);font-weight:700}
  .mk-history{margin:10px 0;font-size:12px;color:var(--text-secondary)}.mk-history summary{cursor:pointer}.mk-history ul{margin:8px 0;padding:0;list-style:none}.mk-history li{display:flex;justify-content:space-between;align-items:center;gap:10px;padding:4px 0}
  .mk-backdrop{position:fixed;inset:0;background:var(--shadow);z-index:100}
  .mk-sheet{position:fixed;top:0;right:0;bottom:0;width:min(560px,100%);overflow-y:auto;overflow-x:hidden;overscroll-behavior:contain;z-index:101;background:var(--bg-card);border-left:1px solid var(--border);padding:26px 28px 40px;box-sizing:border-box}
  .mk-sheet h3{font-size:14px;margin:22px 0 8px}
  .mk-x-wrap{position:sticky;top:0;height:0;z-index:4;display:flex;justify-content:flex-end;overflow:visible}
  .mk-x{width:34px;height:34px;margin:10px 10px 0 0;border-radius:999px;border:1px solid var(--border-strong);background:color-mix(in srgb,var(--bg-surface) 90%,transparent);color:var(--text-strong);display:grid;place-items:center;padding:0;cursor:pointer;backdrop-filter:blur(4px)}
  .mk-x:hover,.mk-x:focus-visible{border-color:var(--accent);color:var(--text)}
  .mk-sheet-head{display:flex;flex-wrap:nowrap;justify-content:space-between;gap:16px;align-items:flex-start;margin-bottom:14px}.mk-head-main{flex:1 1 auto;min-width:0}.mk-sheet-head h2{margin:0;line-height:1.2}
  .mk-go{flex:none;margin-top:-2px;white-space:nowrap}
  .mk-have{flex:none;display:flex;flex-direction:column;align-items:flex-end;gap:6px;margin-top:2px}
  .mk-have-link{background:none;border:0;padding:0;font:inherit;font-size:12px;color:var(--accent-soft-text);text-decoration:underline;text-underline-offset:2px;cursor:pointer}.mk-have-link:hover{color:var(--accent-soft-text)}
  .mk-dev{display:flex;align-items:center;gap:8px;margin:8px 0 0;min-width:0}
  .mk-by{color:var(--text-muted);font-size:12px;margin:8px 0 0}
  .mk-facts{display:grid;gap:10px;margin:0 0 14px}.mk-facts div{display:grid;gap:2px}.mk-facts dt{font-size:11px;letter-spacing:.08em;color:var(--text-secondary);font-weight:700}.mk-facts dd{margin:0;font-size:13px;color:var(--text);overflow-wrap:anywhere}
  .mk-strip{list-style:none;margin:10px 0 14px;padding:0 0 6px;display:flex;gap:8px;overflow-x:auto}
  .mk-strip li{flex:none}
  .mk-thumb{display:block;padding:0;border:1px solid var(--border);border-radius:8px;overflow:hidden;background:#000;cursor:zoom-in;width:104px;height:68px}
  .mk-thumb:hover,.mk-thumb:focus-visible{border-color:var(--accent)}
  .mk-thumb img{display:block;width:100%;height:100%;object-fit:cover}
  .mk-install{margin:2px 0 16px}
  .mk-tabs{display:flex;gap:4px;border-bottom:1px solid var(--border);margin:0 0 14px}
  .mk-tabs button{background:transparent;color:var(--text-secondary);border:0;border-bottom:2px solid transparent;border-radius:0;padding:9px 14px;font:inherit;font-size:13px;font-weight:600;cursor:pointer;margin-bottom:-1px}
  .mk-tabs button[aria-selected="true"]{color:var(--text);border-bottom-color:var(--accent)}
  .mk-desc{font-size:14px;line-height:1.6;color:var(--text);margin:0 0 14px;overflow-wrap:anywhere}
  .mk-notes{font-size:12.5px;line-height:1.6;color:var(--text);white-space:pre-wrap;overflow-wrap:anywhere;margin:4px 0 12px}
  .mk-link{color:var(--accent-soft-text);text-decoration:underline;text-underline-offset:2px;cursor:pointer}.mk-link:hover{color:var(--accent-soft-text)}
  .mk-small{font-size:12px;color:var(--text-secondary);line-height:1.55;margin:6px 0}
  .mk-callout{border:1px solid var(--border-strong);background:var(--info-bg);border-radius:9px;padding:10px 12px;font-size:12px;line-height:1.5;color:var(--text);margin:10px 0}.mk-callout.soft{border-color:var(--warn-border);background:var(--warn-bg);color:var(--warn-text)}
  .gb-plan-card,.gb-done-card{margin:10px 0 6px;padding:14px 16px;border-radius:12px;border:1px solid var(--border-strong);background:var(--bg-surface);display:grid;gap:8px}
  .gb-done-card{border-color:var(--accent-soft-border);background:var(--accent-soft-bg)}
  .gb-plan-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px}
  .gb-plan-head strong{font-size:14px}
  .gb-warn{margin:0;font-size:12px;color:var(--warn-text)}
  .gb-details{font-size:12px;color:var(--text-secondary)}
  .gb-details summary{cursor:pointer;color:var(--text-secondary)}
  .gb-details ul{margin:6px 0 0;padding-left:18px;max-height:140px;overflow:auto;display:grid;gap:2px}
  .gb-plan-btns{display:flex;gap:8px;margin-top:2px}
  code{font-size:11.5px;background:var(--bg-input);border-radius:5px;padding:1px 5px}
  @media(max-width:720px){.mk-reqs>div{grid-template-columns:1fr;gap:2px}.mk-grid{grid-template-columns:1fr}.mk-sheet{padding:20px 16px 32px}}
</style>
