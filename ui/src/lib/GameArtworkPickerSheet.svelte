<script lang="ts">
  // Plex-style per-game artwork picker: search Libretro Thumbnails (Boxart, Title screen, Screenshot)
  // or choose a custom PNG/JPEG/WebP/GIF from the computer. Saved locally in Tau Omega; SD card is never touched.
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { showToast } from './toastStore';
  import {
    clearGameArtEntry,
    displayGameTitle,
    requestGameArt,
    setGameArtEntry,
  } from './gameArtStore';
  import {
    gameArtClearOverride,
    gameArtPreviewLocal,
    gameArtSearch,
    gameArtSetOverride,
  } from './tau-api';
  import type { GameArtCandidate, GameArtKind, GameArtResult } from './types';

  export let platformId = '';
  export let platformName = '';
  export let romName = '';
  export let current: GameArtResult | null = null;
  export let close: () => void = () => {};
  export let onChanged: (res: GameArtResult | null) => void = () => {};

  function cleanInitialQuery(name: string): string {
    return displayGameTitle(name)
      .replace(/^\d{1,5}\s*[-.]\s*/, '')
      .replace(/\s*(\([^)]*\)|\[[^\]]*\])/g, '')
      .replace(/\s+/g, ' ')
      .trim();
  }

  let tab: 'libretro' | 'local' = 'libretro';
  let artKind: GameArtKind = current?.kind ?? 'boxart';
  let lookupPlatform = platformId;
  let query = cleanInitialQuery(romName);
  let candidates: GameArtCandidate[] = [];
  let loading = false;
  let searchError = '';
  let selectedCandidate: GameArtCandidate | null = null;

  let localPath = '';
  let localPreviewUrl: string | null = null;
  let localLoading = false;
  let localError = '';

  let busy = false;
  let actionError = '';

  $: hasSelection = tab === 'libretro' ? !!selectedCandidate : !!localPath;
  $: savePlatform = lookupPlatform.trim() || platformId.trim();

  async function runSearch() {
    const plat = savePlatform;
    const q = query.trim();
    selectedCandidate = null;
    searchError = '';
    if (!plat || !q) {
      candidates = [];
      return;
    }
    loading = true;
    try {
      candidates = await gameArtSearch(plat, q, artKind, 18);
    } catch (e) {
      searchError = errorMessage(e);
      candidates = [];
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    if (savePlatform && query) void runSearch();
  });

  function switchKind(next: GameArtKind) {
    if (artKind === next) return;
    artKind = next;
    void runSearch();
  }

  async function pickLocalFile() {
    localError = '';
    actionError = '';
    const picked = await open({
      directory: false,
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
    });
    if (!picked || Array.isArray(picked)) return;
    localPath = picked;
    localLoading = true;
    try {
      localPreviewUrl = await gameArtPreviewLocal(picked);
    } catch (e) {
      localError = errorMessage(e);
      localPreviewUrl = null;
    } finally {
      localLoading = false;
    }
  }

  async function applySelection() {
    if (!hasSelection) return;
    const plat = platformId.trim() || savePlatform;
    if (!plat) {
      actionError = 'Enter a platform ID first (for example gba, snes, gb, nes).';
      return;
    }
    busy = true;
    actionError = '';
    try {
      const saved = await gameArtSetOverride({
        platformId: plat,
        romName,
        libretroSystem: tab === 'libretro' ? selectedCandidate?.system ?? null : null,
        libretroKind: tab === 'libretro' ? selectedCandidate?.kind ?? artKind : null,
        libretroName: tab === 'libretro' ? selectedCandidate?.name ?? null : null,
        sourcePath: tab === 'local' ? localPath : null,
      });
      setGameArtEntry(plat, romName, saved);
      showToast(`Saved artwork for ${displayGameTitle(romName) || romName}.`);
      onChanged(saved);
      close();
    } catch (e) {
      actionError = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function resetToAuto() {
    const plat = platformId.trim() || savePlatform;
    if (!plat) return;
    busy = true;
    actionError = '';
    try {
      await gameArtClearOverride(plat, romName);
      clearGameArtEntry(plat, romName);
      requestGameArt(plat, romName, true);
      showToast(`Reset artwork for ${displayGameTitle(romName) || romName} to automatic.`, 'info');
      onChanged(null);
      close();
    } catch (e) {
      actionError = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      close();
    }
  }
</script>

<svelte:window on:keydown={onKey} />
<div class="gart-backdrop" role="presentation" on:click={close}></div>
<div
  class="gart-sheet"
  role="dialog"
  aria-modal="true"
  aria-labelledby="gart-title"
  use:modal
>
  <header class="gart-head">
    <div>
      <p class="eyebrow">GAME ARTWORK</p>
      <h2 id="gart-title">{displayGameTitle(romName) || romName}</h2>
      <p class="gart-sub">
        <code>{romName}</code> · Saved locally in Tau Omega — your SD card is untouched.
      </p>
    </div>
    <button class="quiet" type="button" aria-label="Close artwork picker" on:click={close}>✕</button>
  </header>

  <div class="gart-tabs" role="tablist" aria-label="Game artwork source">
    <button
      role="tab"
      type="button"
      class:current={tab === 'libretro'}
      aria-selected={tab === 'libretro'}
      on:click={() => (tab = 'libretro')}
    >
      Libretro Thumbnails
    </button>
    <button
      role="tab"
      type="button"
      class:current={tab === 'local'}
      aria-selected={tab === 'local'}
      on:click={() => (tab = 'local')}
    >
      From computer
    </button>
  </div>

  <div class="gart-body">
    {#if tab === 'libretro'}
      <div class="gart-controls">
        <form class="gart-search" on:submit|preventDefault={() => void runSearch()}>
          <label class="sr" for="gart-query">Game title to search</label>
          <input
            id="gart-query"
            type="text"
            bind:value={query}
            placeholder="Game title (e.g. Super Metroid, Golden Sun)"
          />
          {#if !platformId}
            <label class="sr" for="gart-plat">Platform ID</label>
            <input
              id="gart-plat"
              class="gart-plat-input"
              type="text"
              bind:value={lookupPlatform}
              placeholder="Platform (gba, snes…)"
            />
          {/if}
          <button class="quiet" type="submit" disabled={loading}>Search</button>
        </form>

        <div class="gart-kind-bar">
          <div class="gart-kinds" role="group" aria-label="Artwork type">
            {#each [['boxart', 'Boxart'], ['title', 'Title screen'], ['snap', 'Screenshot']] as [k, label]}
              <button
                type="button"
                aria-pressed={artKind === k}
                class:active={artKind === k}
                on:click={() => switchKind(k as GameArtKind)}
              >
                {label}
              </button>
            {/each}
          </div>
          {#if platformName || savePlatform}
            <span class="gart-plat-tag">{platformName || savePlatform}</span>
          {/if}
        </div>
      </div>

      {#if !savePlatform}
        <div class="gart-state">
          <strong>Platform unknown</strong>
          <p>Enter a platform ID above (such as <code>gba</code>, <code>snes</code>, or <code>gb</code>) to search Libretro Thumbnails, or pick an image from your computer.</p>
        </div>
      {:else if loading}
        <div class="gart-state">
          <span class="gart-spinner" aria-hidden="true"></span>
          <strong>Searching Libretro Thumbnails…</strong>
          <p>First-time platform indexes are cached on disk so future lookups are fast.</p>
        </div>
      {:else if searchError}
        <div class="gart-state err">
          <strong>Could not reach Libretro Thumbnails</strong>
          <p>{searchError}</p>
        </div>
      {:else if !candidates.length}
        <div class="gart-state">
          <strong>No matches for “{query || romName}”</strong>
          <p>Try a shorter title above, switch between Boxart / Title screen / Screenshot, or upload your own file under <b>From computer</b>.</p>
        </div>
      {:else}
        <p class="gart-count">{candidates.length} {candidates.length === 1 ? 'image' : 'images'} found</p>
        <div class="gart-grid" role="listbox" aria-label="Game artwork results">
          {#each candidates as c (`${c.system}|${c.kind}|${c.name}`)}
            <button
              type="button"
              class="gart-opt"
              class:selected={selectedCandidate?.name === c.name && selectedCandidate?.system === c.system && selectedCandidate?.kind === c.kind}
              on:click={() => (selectedCandidate = c)}
            >
              <span class="gart-frame">
                <img src={c.data_url} alt={c.name} />
              </span>
              <span class="gart-meta">
                <strong title={c.name}>{c.name}</strong>
                <small>{c.system}</small>
              </span>
            </button>
          {/each}
        </div>
      {/if}
    {:else}
      <div class="gart-local">
        <div class="gart-local-bar">
          <button class="picker" type="button" on:click={pickLocalFile}>Choose image…</button>
          {#if localPath}<code class="gart-path">{localPath}</code>{/if}
        </div>

        {#if localLoading}
          <div class="gart-state">
            <span class="gart-spinner" aria-hidden="true"></span>
            <p>Reading image…</p>
          </div>
        {:else if localError}
          <div class="gart-state err">
            <strong>Could not open this image</strong>
            <p>{localError}</p>
          </div>
        {:else if localPreviewUrl}
          <div class="gart-preview-box">
            <span class="gart-frame large">
              <img src={localPreviewUrl} alt="Selected game artwork preview" />
            </span>
            <p>Saved locally inside Tau Omega — original proportions are preserved.</p>
          </div>
        {:else if current?.data_url}
          <div class="gart-preview-box muted">
            <span class="gart-frame large">
              <img src={current.data_url} alt="Current game artwork" />
            </span>
            <p>Current artwork. Choose a PNG, JPEG, GIF, or WebP file above to replace it.</p>
          </div>
        {:else}
          <div class="gart-state">
            <strong>Choose a cover image from your computer</strong>
            <p>PNG, JPEG, GIF, or WebP up to 16 MB. Nothing is written to your SD card.</p>
          </div>
        {/if}
      </div>
    {/if}

    {#if actionError}
      <p class="gart-err" role="alert">{actionError}</p>
    {/if}
  </div>

  <footer class="gart-foot">
    <div class="gart-foot-left">
      {#if current?.has_override}
        <button class="quiet" type="button" disabled={busy} on:click={resetToAuto}>
          Reset to automatic
        </button>
      {/if}
    </div>
    <div class="gart-foot-right">
      <button class="quiet" type="button" disabled={busy} on:click={close}>Cancel</button>
      <button
        class="primary"
        type="button"
        disabled={busy || !hasSelection}
        on:click={applySelection}
      >
        {busy ? 'Saving…' : 'Use selected artwork'}
      </button>
    </div>
  </footer>
</div>

<style>
  .gart-backdrop {
    position: fixed;
    inset: 0;
    background: var(--bg-backdrop);
    backdrop-filter: blur(4px);
    z-index: 60;
  }
  .gart-sheet {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(820px, calc(100vw - 32px));
    max-height: min(88vh, 780px);
    background: var(--bg-subtle);
    border: 1px solid var(--border-strong);
    border-radius: 16px;
    box-shadow: 0 24px 60px var(--shadow);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    z-index: 61;
  }
  .gart-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
    padding: 20px 24px 12px;
  }
  .gart-head h2 {
    margin: 2px 0 4px;
    font-size: 20px;
  }
  .gart-sub {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .gart-tabs {
    display: flex;
    gap: 6px;
    padding: 0 24px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .gart-tabs [role='tab'] {
    padding: 9px 14px;
    background: transparent;
    color: var(--text-muted);
    font-size: 13px;
    border-radius: 8px 8px 0 0;
  }
  .gart-tabs [role='tab']:hover {
    color: var(--text);
    background: var(--bg-card);
  }
  .gart-tabs [role='tab'].current {
    color: var(--accent);
    box-shadow: inset 0 -2px 0 var(--accent-solid);
  }
  .gart-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 18px 24px;
    display: grid;
    gap: 14px;
    align-content: start;
  }
  .gart-controls {
    display: grid;
    gap: 10px;
  }
  .gart-search {
    display: flex;
    gap: 8px;
    flex-wrap:wrap;
  }
  .gart-search input {
    flex: 1;
    min-width: 180px;
    font: inherit;
    font-size: 13px;
    color: var(--text);
    background: var(--bg-input);
    border: 1px solid var(--border-strong);
    border-radius: 9px;
    padding: 8px 12px;
  }
  .gart-plat-input {
    flex: 0 0 150px !important;
    min-width: 120px !important;
  }
  .gart-kind-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    flex-wrap:wrap;
  }
  .gart-kinds {
    display: inline-flex;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 2px;
    gap: 2px;
  }
  .gart-kinds button {
    background: transparent;
    color: var(--text-muted);
    border-radius: 7px;
    padding: 5px 11px;
    font-size: 12px;
  }
  .gart-kinds button.active {
    background: var(--bg-hover);
    color: var(--accent);
  }
  .gart-plat-tag {
    font-size: 11.5px;
    color: var(--text-muted);
    padding: 3px 9px;
    border-radius: 999px;
    background: var(--bg-input);
    border: 1px solid var(--border-subtle);
  }
  .gart-count {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }
  .gart-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
    gap: 12px;
  }
  .gart-opt {
    display: grid;
    gap: 8px;
    padding: 9px;
    text-align: left;
    background: var(--bg-input);
    border: 1.5px solid var(--border-subtle);
    border-radius: 11px;
    color: var(--text);
    cursor: pointer;
    transition: border-color 0.14s ease, transform 0.14s ease;
  }
  .gart-opt:hover {
    border-color: var(--border-strong);
    transform: translateY(-1px);
  }
  .gart-opt.selected {
    border-color: var(--accent);
    background: var(--accent-soft-bg);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 35%, transparent);
  }
  .gart-frame {
    display: grid;
    place-items: center;
    width: 100%;
    aspect-ratio: 1 / 1;
    border-radius: 7px;
    overflow: hidden;
    background: var(--bg-sidebar);
    padding: 4px;
    box-sizing: border-box;
  }
  .gart-frame.large {
    width: min(320px, 100%);
    aspect-ratio: 4 / 3;
  }
  .gart-frame img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
  }
  .gart-meta {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .gart-meta strong {
    font-size: 12.5px;
    color: var(--text);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .gart-meta small {
    font-size: 11px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .gart-local {
    display: grid;
    gap: 14px;
  }
  .gart-local-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap:wrap;
  }
  .gart-path {
    font-size: 12px;
    color: var(--text-secondary);
    overflow-wrap: anywhere;
  }
  .gart-preview-box {
    display: grid;
    justify-items: center;
    gap: 8px;
    padding: 18px;
    background: var(--bg-input);
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    text-align: center;
  }
  .gart-preview-box p {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }
  .gart-state {
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 6px;
    padding: 28px 18px;
    background: var(--bg-input);
    border: 1px dashed var(--border);
    border-radius: 12px;
  }
  .gart-state.err {
    border-color: var(--danger-border);
    background: var(--danger-bg);
  }
  .gart-state p {
    margin: 0;
    max-width: 460px;
    font-size: 12.5px;
    color: var(--text-muted);
    line-height: 1.5;
  }
  .gart-spinner {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    animation: gart-spin 0.8s linear infinite;
  }
  @keyframes gart-spin {
    to {
      transform: rotate(360deg);
    }
  }
  .gart-err {
    margin: 0;
    padding: 10px 12px;
    border-radius: 9px;
    border: 1px solid var(--danger-border);
    background: var(--danger-bg);
    color: var(--danger-text);
    font-size: 12.5px;
  }
  .gart-foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 14px 24px 18px;
    border-top: 1px solid var(--border-subtle);
    background: var(--bg-app);
  }
  .gart-foot-left,
  .gart-foot-right {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }
  code {
    font-size: 11.5px;
    background: var(--bg-input);
    border-radius: 5px;
    padding: 1px 5px;
  }
</style>
