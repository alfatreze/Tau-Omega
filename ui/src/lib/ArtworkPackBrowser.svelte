<script lang="ts">
  // Full-UI categorized Pack Browser modal: search seamlessly across an entire community set,
  // switch creator variants, preview every platform grouped by Category -> Manufacturer -> Platform,
  // and review/apply either the whole pack ("Apply all") or any individual hierarchy segment.
  import { onMount } from 'svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { cardDevice } from './systemStore';
  import { showToast } from './toastStore';
  import {
    getCachedPackSamples,
    invalidatePackSamples,
    setCachedPackSamples,
  } from './artworkCache';
  import {
    artworkExecutePackApply,
    artworkPackSamples,
    artworkPlanPackApply,
  } from './tau-api';
  import type {
    ArtworkSample,
    ImagePackEntry,
    PackApplyPlan,
  } from './types';

  export let card = '';
  export let title = 'Image Pack';
  export let owner = '';
  export let variants: ImagePackEntry[] = [];
  export let initialPackId = '';
  export let onPackCached: (packId: string, count: number) => void = () => {};
  export let close: () => void = () => {};

  const CATEGORY_ORDER = [
    'Handheld',
    'Console',
    'Arcade',
    'Computer',
    'Fantasy Console',
    'Media Players',
    'Other',
  ];

  type MakerGroup = {
    manufacturer: string;
    items: ArtworkSample[];
    installedIds: string[];
  };

  type CategoryGroup = {
    category: string;
    total: number;
    installedIds: string[];
    makers: MakerGroup[];
  };

  let activePackId = initialPackId || variants[0]?.id || '';
  let samples: ArtworkSample[] = [];
  let loading = true;
  let error = '';

  let query = '';
  let scope: 'all' | 'card' = 'all';

  let pendingLabel = '';
  let pendingOnlyPlatforms: string[] | null = null;
  let plan: PackApplyPlan | null = null;
  let planBusy = false;
  let applyBusy = false;
  let statusNotice = '';
  let bodyEl: HTMLElement | null = null;

  $: activePack = variants.find((v) => v.id === activePackId) ?? variants[0];
  $: canApplyToCard = !!card && $cardDevice?.device === 'analogue_pocket';
  $: allInstalledIds = samples
    .map((s) => s.installed_platform_id)
    .filter((id): id is string => !!id);

  $: q = query.trim().toLowerCase();
  $: filteredSamples = samples.filter((s) => {
    if (scope === 'card' && !s.installed_platform_id) return false;
    if (!q) return true;
    return `${s.name} ${s.platform_id} ${s.category} ${s.manufacturer}`
      .toLowerCase()
      .includes(q);
  });

  $: grouped = (() => {
    const byCat = new Map<string, ArtworkSample[]>();
    for (const s of filteredSamples) {
      const cat = s.category || 'Other';
      const arr = byCat.get(cat) ?? [];
      arr.push(s);
      byCat.set(cat, arr);
    }
    const cats = [...byCat.keys()].sort((a, b) => {
      const ia = CATEGORY_ORDER.indexOf(a);
      const ib = CATEGORY_ORDER.indexOf(b);
      if (ia >= 0 && ib >= 0) return ia - ib;
      if (ia >= 0) return -1;
      if (ib >= 0) return 1;
      return a.localeCompare(b);
    });

    const result: CategoryGroup[] = [];
    for (const cat of cats) {
      const list = byCat.get(cat) ?? [];
      const byMaker = new Map<string, ArtworkSample[]>();
      for (const s of list) {
        const m = s.manufacturer?.trim() || 'Other';
        const arr = byMaker.get(m) ?? [];
        arr.push(s);
        byMaker.set(m, arr);
      }
      const makerNames = [...byMaker.keys()].sort((a, b) => {
        if (a === 'Other') return 1;
        if (b === 'Other') return -1;
        return a.localeCompare(b);
      });
      const makers: MakerGroup[] = makerNames.map((m) => {
        const items = (byMaker.get(m) ?? []).sort((x, y) => x.name.localeCompare(y.name));
        return {
          manufacturer: m,
          items,
          installedIds: items
            .map((i) => i.installed_platform_id)
            .filter((id): id is string => !!id),
        };
      });
      result.push({
        category: cat,
        total: list.length,
        installedIds: list
          .map((i) => i.installed_platform_id)
          .filter((id): id is string => !!id),
        makers,
      });
    }
    return result;
  })();

  const variantLabel = (p: ImagePackEntry) =>
    p.variant ? p.variant.replace(/[_-]+/g, ' ') : 'Default';

  async function loadPack(packId: string, bypassCache = false) {
    activePackId = packId;
    error = '';
    plan = null;
    pendingLabel = '';
    pendingOnlyPlatforms = null;
    const cardKey = canApplyToCard ? card : '';
    if (!bypassCache) {
      const hit = getCachedPackSamples(packId, cardKey);
      if (hit) {
        samples = hit;
        loading = false;
        onPackCached(packId, hit.length);
        return;
      }
    }
    loading = true;
    try {
      const list = await artworkPackSamples(packId, 500, cardKey || null);
      if (activePackId !== packId) return;
      samples = list;
      setCachedPackSamples(packId, cardKey, list);
      onPackCached(packId, list.length);
    } catch (e) {
      if (activePackId === packId) {
        error = errorMessage(e);
        samples = [];
      }
    } finally {
      if (activePackId === packId) loading = false;
    }
  }

  onMount(() => {
    if (activePackId) void loadPack(activePackId);
  });

  async function startApply(label: string, onlyIds: string[] | null) {
    if (!canApplyToCard || !activePack) return;
    planBusy = true;
    statusNotice = '';
    pendingLabel = label;
    pendingOnlyPlatforms = onlyIds;
    try {
      plan = await artworkPlanPackApply(card, activePack.id, onlyIds);
    } catch (e) {
      statusNotice = `Could not plan apply: ${errorMessage(e)}`;
      plan = null;
    } finally {
      planBusy = false;
    }
  }

  async function confirmApply() {
    if (!plan || !canApplyToCard || !activePack) return;
    applyBusy = true;
    statusNotice = '';
    try {
      const report = await artworkExecutePackApply(
        card,
        activePack.id,
        plan.id,
        pendingOnlyPlatforms,
      );
      const msg = `Applied ${report.written} ${report.written === 1 ? 'banner' : 'banners'}${report.backed_up ? ` (${report.backed_up} backed up)` : ''}.`;
      showToast(msg, 'success');
      statusNotice = '';
      plan = null;
      pendingLabel = '';
      window.dispatchEvent(new CustomEvent('tau-artwork-changed', { detail: { batch: true } }));
      invalidatePackSamples(activePack.id);
      await loadPack(activePack.id, true);
    } catch (e) {
      statusNotice = `Could not apply: ${errorMessage(e)}`;
      showToast(statusNotice, 'error');
    } finally {
      applyBusy = false;
    }
  }

  function jumpToCategory(cat: string) {
    const slug = `pb-cat-${cat.replace(/\W+/g, '-')}`;
    const el = bodyEl?.querySelector(`#${slug}`);
    if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      close();
    }
  }
</script>

<svelte:window on:keydown={onKey} />
<div class="pb-backdrop" role="presentation" on:click={close}></div>
<div
  class="pb-shell"
  role="dialog"
  aria-modal="true"
  aria-labelledby="pb-title"
  use:modal
>
  <header class="pb-head">
    <div class="pb-title-wrap">
      <p class="eyebrow">IMAGE PACK BROWSER</p>
      <div class="pb-title-row">
        <h2 id="pb-title">{title}</h2>
        {#if owner}<span class="pb-author">by <b>{owner}</b></span>{/if}
      </div>
      {#if variants.length > 1}
        <div class="pb-variants" role="group" aria-label="Pack variants">
          {#each variants as v (v.id)}
            <button
              type="button"
              class="pb-var"
              class:selected={activePackId === v.id}
              disabled={loading}
              on:click={() => void loadPack(v.id)}
            >
              <span>{variantLabel(v)}</span>
              {#if v.cached}<small>{v.cached_platforms}</small>{/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="pb-head-actions">
      {#if canApplyToCard && allInstalledIds.length > 0}
        <button
          type="button"
          class="primary"
          disabled={loading || planBusy || applyBusy}
          on:click={() => void startApply(`All installed platforms (${allInstalledIds.length})`, null)}
        >
          {planBusy && pendingOnlyPlatforms === null ? 'Planning…' : `Apply all (${allInstalledIds.length})`}
        </button>
      {/if}
      <button type="button" class="quiet pb-close" aria-label="Close pack browser" on:click={close}>
        ✕
      </button>
    </div>
  </header>

  <div class="pb-nav">
    <div class="pb-search-bar">
      <label class="sr" for="pb-search">Search platforms, manufacturers, or categories</label>
      <input
        id="pb-search"
        type="search"
        bind:value={query}
        placeholder="Search platforms, manufacturers, categories…"
      />
      {#if canApplyToCard}
        <div class="pb-scope" role="group" aria-label="Filter platforms">
          <button
            type="button"
            class:active={scope === 'all'}
            on:click={() => (scope = 'all')}
          >
            All in set ({samples.length})
          </button>
          <button
            type="button"
            class:active={scope === 'card'}
            on:click={() => (scope = 'card')}
          >
            On my card ({allInstalledIds.length})
          </button>
        </div>
      {/if}
    </div>

    {#if grouped.length > 1}
      <nav class="pb-jumps" aria-label="Jump to category">
        {#each grouped as g (g.category)}
          <button type="button" on:click={() => jumpToCategory(g.category)}>
            {g.category} <span>{g.total}</span>
          </button>
        {/each}
      </nav>
    {/if}
  </div>

  <div class="pb-body" bind:this={bodyEl}>
    {#if loading}
      <div class="pb-state">
        <span class="pb-spinner" aria-hidden="true"></span>
        <strong>{activePack?.cached ? 'Loading cached pack…' : 'Downloading & indexing pack…'}</strong>
        <p>
          {activePack?.cached
            ? 'Preparing categorized banner previews from your local cache.'
            : 'Downloading this pack once and saving decoded previews to your cache.'}
        </p>
      </div>
    {:else if error}
      <div class="pb-state err">
        <strong>Could not open this image pack</strong>
        <p>{error}</p>
        <button type="button" class="primary" on:click={() => void loadPack(activePackId)}>
          Try again
        </button>
      </div>
    {:else if !grouped.length}
      <div class="pb-state">
        <strong>Nothing matches your filter</strong>
        <p>
          {#if scope === 'card' && !allInstalledIds.length}
            None of the platforms in this pack are currently installed on the open card. Switch to
            <b>All in set</b> above to browse every banner anyway.
          {:else}
            Try a different platform, manufacturer, or category search.
          {/if}
        </p>
      </div>
    {:else}
      {#each grouped as cat (cat.category)}
        <section
          class="pb-cat"
          id={`pb-cat-${cat.category.replace(/\W+/g, '-')}`}
          aria-labelledby={`pb-cat-h-${cat.category.replace(/\W+/g, '-')}`}
        >
          <div class="pb-cat-head">
            <div class="pb-cat-title">
              <h3 id={`pb-cat-h-${cat.category.replace(/\W+/g, '-')}`}>{cat.category}</h3>
              <span>{cat.total} {cat.total === 1 ? 'platform' : 'platforms'}</span>
              {#if canApplyToCard && cat.installedIds.length}
                <span class="pb-card-pill">{cat.installedIds.length} on card</span>
              {/if}
            </div>
            {#if canApplyToCard && cat.installedIds.length > 0}
              <button
                type="button"
                class="seg-apply"
                disabled={planBusy || applyBusy}
                on:click={() =>
                  void startApply(
                    `${cat.category} (${cat.installedIds.length})`,
                    cat.installedIds,
                  )}
              >
                Apply {cat.category} ({cat.installedIds.length})
              </button>
            {/if}
          </div>

          {#each cat.makers as mk (mk.manufacturer)}
            <div class="pb-maker">
              <div class="pb-maker-head">
                <h4>
                  {mk.manufacturer}
                  <small>{mk.items.length}</small>
                </h4>
                {#if canApplyToCard && mk.installedIds.length > 0 && cat.makers.length > 1}
                  <button
                    type="button"
                    class="seg-apply subtle"
                    disabled={planBusy || applyBusy}
                    on:click={() =>
                      void startApply(
                        `${mk.manufacturer} · ${cat.category} (${mk.installedIds.length})`,
                        mk.installedIds,
                      )}
                  >
                    Apply {mk.manufacturer} ({mk.installedIds.length})
                  </button>
                {/if}
              </div>

              <div class="pb-grid">
                {#each mk.items as item (item.platform_id)}
                  <article class="pb-card" class:installed={!!item.installed_platform_id}>
                    <div class="pb-frame">
                      <img src={item.data_url} alt={item.name} loading="lazy" />
                      {#if item.installed_platform_id}
                        <span class="pb-badge-card" class:replaces={item.replaces_existing}>
                          {item.replaces_existing ? 'On card' : 'New for card'}
                        </span>
                      {/if}
                    </div>
                    <div class="pb-card-foot">
                      <div class="pb-card-names">
                        <strong>{item.name}</strong>
                        <small>{item.platform_id}.bin</small>
                      </div>
                      {#if canApplyToCard && item.installed_platform_id}
                        <button
                          type="button"
                          class="seg-apply mini"
                          disabled={planBusy || applyBusy}
                          on:click={() =>
                            void startApply(item.name, [item.installed_platform_id ?? item.platform_id])}
                        >
                          Apply
                        </button>
                      {/if}
                    </div>
                  </article>
                {/each}
              </div>
            </div>
          {/each}
        </section>
      {/each}
    {/if}
  </div>

  {#if plan || statusNotice}
    <footer class="pb-foot">
      {#if plan}
        <div class="pb-review">
          <div>
            <strong>{pendingLabel}</strong>
            <p>
              {#if plan.items.length === 0}
                No installed platforms matched. Nothing will be written.
              {:else}
                Replaces {plan.will_replace} existing and adds {plan.will_add} new
                {plan.will_add === 1 ? 'banner' : 'banners'}. Replaced images are backed up outside
                your card first.
              {/if}
            </p>
          </div>
          <div class="pb-review-btns">
            <button
              type="button"
              class="quiet"
              disabled={applyBusy}
              on:click={() => {
                plan = null;
                pendingLabel = '';
              }}
            >
              Cancel
            </button>
            {#if plan.items.length > 0}
              <button
                type="button"
                class="primary"
                disabled={applyBusy}
                on:click={confirmApply}
              >
                {applyBusy
                  ? 'Applying…'
                  : `Confirm & apply ${plan.items.length}`}
              </button>
            {/if}
          </div>
        </div>
      {:else if statusNotice}
        <p class="pb-status" role="alert">{statusNotice}</p>
      {/if}
    </footer>
  {/if}
</div>

<style>
  .pb-backdrop {
    position: fixed;
    inset: 0;
    background: var(--bg-backdrop);
    backdrop-filter: blur(5px);
    z-index: 65;
  }
  .pb-shell {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(1160px, calc(100vw - 36px));
    height: min(90vh, 860px);
    background: var(--bg-app);
    border: 1px solid var(--border-strong);
    border-radius: 16px;
    box-shadow: 0 28px 72px var(--shadow);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    z-index: 66;
  }
  .pb-head {
    flex: none;
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
    padding: 18px 24px 14px;
    border-bottom: 1px solid var(--bg-hover);
    background: var(--bg-subtle);
  }
  .pb-title-wrap {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .pb-title-row {
    display: flex;
    align-items: baseline;
    flex-wrap:wrap;
    gap: 10px;
  }
  .pb-title-row h2 {
    margin: 0;
    font-size: 21px;
    text-transform: capitalize;
    color: var(--text);
  }
  .pb-author {
    font-size: 13px;
    color: var(--text-muted);
  }
  .pb-author b {
    color: var(--text-secondary);
  }
  .pb-variants {
    display: flex;
    flex-wrap:wrap;
    gap: 6px;
    margin-top: 2px;
  }
  .pb-var {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 11px;
    font-size: 12px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-secondary);
  }
  .pb-var:hover {
    border-color: var(--border-strong);
  }
  .pb-var.selected {
    border-color: var(--accent);
    background: var(--accent-soft-bg);
    color: var(--text);
  }
  .pb-var small {
    font-size: 10.5px;
    color: var(--text-muted);
  }
  .pb-head-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: none;
  }
  .pb-close {
    font-size: 16px;
    padding: 6px 10px;
  }
  .pb-nav {
    flex: none;
    padding: 12px 24px;
    border-bottom: 1px solid var(--bg-hover);
    background: var(--bg-subtle);
    display: grid;
    gap: 10px;
  }
  .pb-search-bar {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap:wrap;
  }
  .pb-search-bar input {
    flex: 1;
    min-width: 240px;
    font: inherit;
    font-size: 13.5px;
    color: var(--text);
    background: var(--bg-sidebar);
    border: 1px solid var(--border-strong);
    border-radius: 9px;
    padding: 8px 13px;
  }
  .pb-scope {
    display: inline-flex;
    background: var(--bg-sidebar);
    border: 1px solid var(--border-strong);
    border-radius: 9px;
    padding: 2px;
    gap: 2px;
  }
  .pb-scope button {
    padding: 6px 12px;
    font-size: 12px;
    background: transparent;
    color: var(--text-muted);
    border-radius: 7px;
  }
  .pb-scope button.active {
    background: var(--accent-soft-bg);
    color: var(--accent);
  }
  .pb-jumps {
    display: flex;
    gap: 6px;
    flex-wrap:wrap;
  }
  .pb-jumps button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    font-size: 12px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-secondary);
  }
  .pb-jumps button:hover {
    border-color: var(--accent);
    color: var(--text);
  }
  .pb-jumps button span {
    font-size: 11px;
    color: var(--text-muted);
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
  .pb-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 18px 24px 28px;
    display: grid;
    gap: 28px;
    align-content: start;
  }
  .pb-state {
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 8px;
    padding: 48px 24px;
    border: 1px dashed var(--border-strong);
    border-radius: 12px;
    background: var(--bg-card);
  }
  .pb-state strong {
    font-size: 15px;
    color: var(--text);
  }
  .pb-state p {
    margin: 0;
    max-width: 460px;
    font-size: 13px;
    color: var(--text-muted);
    line-height: 1.5;
  }
  .pb-state.err {
    border-color: var(--danger-border);
    background: var(--danger-bg);
  }
  .pb-spinner {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2.5px solid var(--border-strong);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .pb-cat {
    display: grid;
    gap: 16px;
  }
  .pb-cat-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border-subtle);
    flex-wrap:wrap;
  }
  .pb-cat-title {
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-wrap:wrap;
  }
  .pb-cat-title h3 {
    margin: 0;
    font-size: 17px;
    color: var(--text);
  }
  .pb-cat-title span {
    font-size: 12px;
    color: var(--text-muted);
  }
  .pb-card-pill {
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--accent-soft-bg);
    color: var(--accent) !important;
    font-size: 11px !important;
  }
  .pb-maker {
    display: grid;
    gap: 10px;
  }
  .pb-maker-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
  }
  .pb-maker-head h4 {
    margin: 0;
    font-size: 12px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-secondary);
    display: flex;
    align-items: baseline;
    gap: 7px;
  }
  .pb-maker-head h4 small {
    font-weight: 400;
    color: var(--text-faint);
  }
  .pb-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(225px, 1fr));
    gap: 12px;
  }
  .pb-card {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 8px;
    display: grid;
    gap: 7px;
  }
  .pb-card.installed {
    border-color: var(--accent-soft-border);
  }
  .pb-frame {
    position: relative;
    background: var(--pocket-art-bg);
    border: 1px solid var(--bg-hover);
    border-radius: 7px;
    overflow: hidden;
  }
  .pb-frame img {
    display: block;
    width: 100%;
    height: auto;
    aspect-ratio: 521 / 165;
    object-fit: contain;
    filter: var(--pocket-art-filter);
  }
  .pb-badge-card {
    position: absolute;
    top: 5px;
    right: 5px;
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent-soft-bg) 92%, transparent);
    border: 1px solid var(--accent-soft-border);
    color: var(--accent);
  }
  .pb-badge-card.replaces {
    background: color-mix(in srgb, var(--warn-bg) 92%, transparent);
    border-color: var(--warn-border);
    color: var(--warn-text);
  }
  .pb-card-foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .pb-card-names {
    display: grid;
    min-width: 0;
  }
  .pb-card-names strong {
    font-size: 12.5px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pb-card-names small {
    font-size: 11px;
    color: var(--text-muted);
  }
  .seg-apply {
    padding: 5px 11px;
    font-size: 12px;
    border-radius: 8px;
    background: var(--accent-soft-bg);
    border: 1px solid var(--accent-soft-border);
    color: var(--accent);
    white-space: nowrap;
    flex: none;
  }
  .seg-apply:hover:not(:disabled) {
    background: var(--accent-soft-bg);
    border-color: var(--accent);
  }
  .seg-apply.subtle {
    padding: 3px 9px;
    font-size: 11.5px;
    background: var(--accent-soft-bg);
    border-color: var(--accent-soft-border);
    color: var(--accent);
  }
  .seg-apply.mini {
    padding: 3px 8px;
    font-size: 11px;
  }
  .pb-foot {
    flex: none;
    padding: 12px 24px;
    border-top: 1px solid var(--border);
    background: var(--accent-soft-bg);
  }
  .pb-review {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 14px;
    flex-wrap:wrap;
  }
  .pb-review strong {
    display: block;
    font-size: 13.5px;
    color: var(--text);
  }
  .pb-review p {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .pb-review-btns {
    display: flex;
    gap: 8px;
  }
  .pb-status {
    margin: 0;
    font-size: 13px;
    color: var(--accent);
  }
</style>
