<script lang="ts">
  // Settings -> Artwork & Image Packs: grouped creator collections with variant pills and a full-UI Pack Browser.
  import { onMount } from 'svelte';
  import { errorMessage } from './backend';
  import ArtworkPackBrowser from './ArtworkPackBrowser.svelte';
  import { invalidateAllArtworkCache } from './artworkCache';
  import { cardDevice } from './systemStore';
  import { artworkListPacks } from './tau-api';
  import type { ImagePackEntry } from './types';

  export let card = '';

  type CollectionGroup = {
    key: string;
    owner: string;
    repository: string;
    title: string;
    packs: ImagePackEntry[];
  };

  let packs: ImagePackEntry[] = [];
  let filter = '';
  let loading = true;
  let error = '';

  let activePackByGroup: Record<string, string> = {};
  let browserGroup: CollectionGroup | null = null;
  let browserPackId = '';

  const variantLabel = (p: ImagePackEntry) =>
    p.variant ? p.variant.replace(/[_-]+/g, ' ') : 'Default';

  const groupTitle = (repo: string) =>
    `${repo.replace(/[-_]+/g, ' ')}`.replace(/\banalogue pocket\b/gi, '').trim() || repo;

  $: collections = (() => {
    const map = new Map<string, CollectionGroup>();
    for (const p of packs) {
      const key = `${p.owner}/${p.repository}`;
      let g = map.get(key);
      if (!g) {
        g = {
          key,
          owner: p.owner,
          repository: p.repository,
          title: groupTitle(p.repository),
          packs: [],
        };
        map.set(key, g);
      }
      g.packs.push(p);
    }
    return [...map.values()];
  })();

  $: q = filter.trim().toLowerCase();
  $: visibleCollections = q
    ? collections.filter((g) =>
        `${g.owner} ${g.repository} ${g.title} ${g.packs.map((p) => p.variant ?? '').join(' ')}`
          .toLowerCase()
          .includes(q),
      )
    : collections;

  $: canApplyToCard = !!card && $cardDevice?.device === 'analogue_pocket';

  function activePackFor(g: CollectionGroup): ImagePackEntry {
    const picked = activePackByGroup[g.key];
    return g.packs.find((p) => p.id === picked) ?? g.packs[0];
  }

  function selectVariant(groupKey: string, packId: string) {
    activePackByGroup = { ...activePackByGroup, [groupKey]: packId };
  }

  function openBrowser(g: CollectionGroup, packId: string) {
    browserGroup = g;
    browserPackId = packId;
  }

  function handlePackCached(packId: string, count: number) {
    packs = packs.map((p) =>
      p.id === packId ? { ...p, cached: true, cached_platforms: count } : p,
    );
  }

  async function load(force = false) {
    loading = true;
    error = '';
    if (force) invalidateAllArtworkCache();
    try {
      const res = await artworkListPacks(force);
      packs = res.packs;
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    void load(false);
  });
</script>

<section class="art-settings" aria-labelledby="art-settings-title">
  <div class="as-head">
    <div>
      <h2 id="art-settings-title">Artwork &amp; Image Packs</h2>
      <p class="as-lede">
        Community banner sets grouped by creator. Open any collection to browse all platforms by
        category &amp; manufacturer and apply the whole pack or individual sections — or click
        <b>Change image</b> on any core to pick artwork per platform.
      </p>
    </div>
    <button class="quiet" type="button" disabled={loading} on:click={() => load(true)}>
      {loading ? 'Refreshing…' : 'Refresh list'}
    </button>
  </div>

  {#if !canApplyToCard}
    <p class="as-card-hint">
      Open an Analogue Pocket card to apply a pack across your whole card, by category, or per
      manufacturer. You can still browse every set below.
    </p>
  {/if}

  <div class="as-toolbar">
    <label class="sr" for="as-filter">Filter image packs</label>
    <input
      id="as-filter"
      type="search"
      bind:value={filter}
      placeholder="Search creators, sets, or regions…"
    />
  </div>

  {#if loading && !packs.length}
    <div class="as-empty">
      <p>Loading community image packs…</p>
    </div>
  {:else if error && !packs.length}
    <div class="as-empty err">
      <strong>Could not load community image packs</strong>
      <p>{error}</p>
      <button class="picker" type="button" on:click={() => load(true)}>Try again</button>
    </div>
  {:else if !visibleCollections.length}
    <div class="as-empty">
      <strong>No image pack collections match “{filter}”</strong>
      <p>Clear your search to see every creator’s sets.</p>
    </div>
  {:else}
    <ul class="as-groups">
      {#each visibleCollections as group (group.key)}
        {@const active = activePackFor(group)}
        <li class="as-group">
          <div class="as-group-top">
            <div class="as-group-id">
              <h3>{group.title}</h3>
              <span class="as-author">by <b>{group.owner}</b></span>
              {#if group.packs.length > 1}
                <span class="as-badge">{group.packs.length} sets</span>
              {/if}
            </div>
            <div class="as-controls">
              <button
                class={canApplyToCard ? 'picker' : 'quiet'}
                type="button"
                on:click={() => openBrowser(group, active.id)}
              >
                {canApplyToCard ? 'Browse & apply…' : 'Browse set…'}
              </button>
            </div>
          </div>

          {#if group.packs.length > 1}
            <div class="as-variants" role="group" aria-label={`Sets in ${group.title}`}>
              {#each group.packs as p (p.id)}
                {@const selected = active.id === p.id}
                <button
                  type="button"
                  class="as-var"
                  class:selected
                  on:click={() => selectVariant(group.key, p.id)}
                >
                  <span>{variantLabel(p)}</span>
                  {#if p.cached}<small>{p.cached_platforms}</small>{/if}
                </button>
              {/each}
            </div>
          {/if}

          <div class="as-meta-line">
            <code>{active.id}</code>
            {#if active.cached}
              <span>· Cached ({active.cached_platforms} platforms)</span>
            {:else}
              <span>· Downloads automatically when opened</span>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

{#if browserGroup}
  <ArtworkPackBrowser
    {card}
    title={browserGroup.title}
    owner={browserGroup.owner}
    variants={browserGroup.packs}
    initialPackId={browserPackId}
    onPackCached={handlePackCached}
    close={() => (browserGroup = null)}
  />
{/if}

<style>
  .art-settings {
    max-width: 1040px;
    margin-bottom: 14px;
    padding: 22px 24px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg-card);
  }
  .as-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 10px;
  }
  .as-head h2 {
    margin: 0 0 6px;
  }
  .as-lede {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
    line-height: 1.5;
  }
  .as-card-hint {
    margin: 0 0 12px;
    padding: 9px 12px;
    border-radius: 9px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .as-toolbar {
    display: flex;
    gap: 10px;
    align-items: center;
    margin: 12px 0 14px;
  }
  .as-toolbar input {
    flex: 1;
    box-sizing: border-box;
    font: inherit;
    font-size: 13px;
    color: var(--text);
    background: var(--bg-input);
    border: 1px solid var(--border-strong);
    border-radius: 9px;
    padding: 8px 12px;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
  .as-groups {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  .as-group {
    border: 1px solid var(--border);
    background: var(--bg-subtle);
    border-radius: 12px;
    padding: 13px 16px;
    display: grid;
    gap: 9px;
  }
  .as-group-top {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    flex-wrap:wrap;
  }
  .as-group-id {
    display: flex;
    align-items: baseline;
    flex-wrap:wrap;
    gap: 8px;
    min-width: 0;
  }
  .as-group-id h3 {
    margin: 0;
    font-size: 15px;
    color: var(--text);
    text-transform: capitalize;
  }
  .as-author {
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .as-author b {
    color: var(--text-secondary);
    font-weight: 600;
  }
  .as-badge {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--bg-card);
    color: var(--text-secondary);
  }
  .as-controls {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }
  .as-variants {
    display: flex;
    flex-wrap:wrap;
    gap: 6px;
  }
  .as-var {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    font-size: 12px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-secondary);
  }
  .as-var:hover {
    border-color: var(--border-strong);
  }
  .as-var.selected {
    border-color: var(--accent);
    background: var(--accent-soft-bg);
    color: var(--text);
  }
  .as-var small {
    font-size: 10.5px;
    color: var(--text-muted);
  }
  .as-meta-line {
    display: flex;
    flex-wrap:wrap;
    gap: 6px;
    font-size: 11.5px;
    color: var(--text-muted);
    align-items: center;
  }
  .as-empty {
    padding: 20px;
    border: 1px dashed var(--border-strong);
    border-radius: 10px;
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
  }
  .as-empty.err {
    border-color: var(--danger-border);
    background: var(--danger-bg);
  }
</style>
