<script lang="ts">
  // Plex-style per-game artwork picker: search Libretro Thumbnails (Boxart, Title screen, Screenshot)
  // or choose a custom PNG/JPEG/WebP/GIF from the computer. Saved locally in Tau Omega; SD card is never touched.
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
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
    background: rgba(7, 11, 12, 0.72);
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
    background: #141c1d;
    border: 1px solid #304142;
    border-radius: 16px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
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
    color: #93a3a2;
  }
  .gart-tabs {
    display: flex;
    gap: 6px;
    padding: 0 24px;
    border-bottom: 1px solid #283536;
  }
  .gart-tabs [role='tab'] {
    padding: 9px 14px;
    background: transparent;
    color: #9cacab;
    font-size: 13px;
    border-radius: 8px 8px 0 0;
  }
  .gart-tabs [role='tab']:hover {
    color: #eff5f4;
    background: #1a2426;
  }
  .gart-tabs [role='tab'].current {
    color: #c1f0ad;
    box-shadow: inset 0 -2px 0 #c1f0ad;
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
    flex-wrap: wrap;
  }
  .gart-search input {
    flex: 1;
    min-width: 180px;
    font: inherit;
    font-size: 13px;
    color: #e8ecec;
    background: #0e1415;
    border: 1px solid #304142;
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
    flex-wrap: wrap;
  }
  .gart-kinds {
    display: inline-flex;
    background: #0e1415;
    border: 1px solid #2b3839;
    border-radius: 9px;
    padding: 2px;
    gap: 2px;
  }
  .gart-kinds button {
    background: transparent;
    color: #9cacab;
    border-radius: 7px;
    padding: 5px 11px;
    font-size: 12px;
  }
  .gart-kinds button.active {
    background: #233335;
    color: #c1f0ad;
  }
  .gart-plat-tag {
    font-size: 11.5px;
    color: #8f9e9d;
    padding: 3px 9px;
    border-radius: 999px;
    background: #101718;
    border: 1px solid #273435;
  }
  .gart-count {
    margin: 0;
    font-size: 12px;
    color: #8f9e9d;
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
    background: #101718;
    border: 1.5px solid #283536;
    border-radius: 11px;
    color: #e8ecec;
    cursor: pointer;
    transition: border-color 0.14s ease, transform 0.14s ease;
  }
  .gart-opt:hover {
    border-color: #4b6163;
    transform: translateY(-1px);
  }
  .gart-opt.selected {
    border-color: #c1f0ad;
    background: #17251d;
    box-shadow: 0 0 0 1px rgba(193, 240, 173, 0.35);
  }
  .gart-frame {
    display: grid;
    place-items: center;
    width: 100%;
    aspect-ratio: 1 / 1;
    border-radius: 7px;
    overflow: hidden;
    background: #0a0f10;
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
    color: #eff5f4;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .gart-meta small {
    font-size: 11px;
    color: #8f9e9d;
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
    flex-wrap: wrap;
  }
  .gart-path {
    font-size: 12px;
    color: #9fb3aa;
    overflow-wrap: anywhere;
  }
  .gart-preview-box {
    display: grid;
    justify-items: center;
    gap: 8px;
    padding: 18px;
    background: #101718;
    border: 1px solid #283536;
    border-radius: 12px;
    text-align: center;
  }
  .gart-preview-box p {
    margin: 0;
    font-size: 12px;
    color: #93a3a2;
  }
  .gart-state {
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 6px;
    padding: 28px 18px;
    background: #101718;
    border: 1px dashed #2e3e3f;
    border-radius: 12px;
  }
  .gart-state.err {
    border-color: #6b3535;
    background: #231414;
  }
  .gart-state p {
    margin: 0;
    max-width: 460px;
    font-size: 12.5px;
    color: #95a5a4;
    line-height: 1.5;
  }
  .gart-spinner {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2px solid #2e4042;
    border-top-color: #c1f0ad;
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
    border: 1px solid #6b3535;
    background: #261616;
    color: #f2b8b5;
    font-size: 12.5px;
  }
  .gart-foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 14px 24px 18px;
    border-top: 1px solid #283536;
    background: #111819;
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
    background: #101617;
    border-radius: 5px;
    padding: 1px 5px;
  }
</style>
