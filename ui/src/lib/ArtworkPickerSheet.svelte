<script lang="ts">
  // Plex-style alternative artwork picker: browse community image pack banners or choose a local image file.
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { getCachedAlternatives, setCachedAlternatives } from './artworkCache';
  import {
    artworkAlternatives,
    artworkClearOverride,
    artworkExecuteApply,
    artworkPlanApply,
    artworkPreviewLocal,
    artworkSetOverride,
  } from './tau-api';
  import type {
    ArtworkAlternative,
    ArtworkApplyPlan,
    ArtworkFit,
    ArtworkSlot,
    DeviceKind,
  } from './types';

  export let card = '';
  export let device: DeviceKind = 'analogue_pocket';
  export let title = 'Change artwork';
  export let platformId = '';
  export let targetId = '';
  export let slot: ArtworkSlot = 'banner';
  export let overrideKey = '';
  export let hasOverride = false;
  export let currentUrl: string | null = null;
  export let onApplied: (newUrl: string | null) => void = () => {};
  export let close: () => void = () => {};

  let tab: 'packs' | 'local' = 'packs';
  let lookupQuery = platformId;
  let alts: ArtworkAlternative[] = [];
  let loadingAlts = false;
  let altsError = '';

  let selectedAlt: ArtworkAlternative | null = null;
  let localPath = '';
  let localFit: ArtworkFit = 'cover';
  let localPreviewUrl: string | null = null;
  let localLoading = false;
  let localError = '';

  let plan: ArtworkApplyPlan | null = null;
  let busy = false;
  let actionError = '';

  $: effectiveTarget = targetId || platformId;
  $: canPocketWrite = device === 'analogue_pocket' && !!card && !!effectiveTarget;
  $: hasSelection = tab === 'packs' ? !!selectedAlt : !!localPath;

  async function loadAlternatives(pid: string) {
    const clean = pid.trim();
    plan = null;
    selectedAlt = null;
    altsError = '';
    if (!clean) {
      alts = [];
      return;
    }
    const hit = getCachedAlternatives(clean);
    if (hit) {
      alts = hit;
      loadingAlts = false;
      return;
    }
    loadingAlts = true;
    try {
      const list = await artworkAlternatives(clean, { includeAll: true });
      alts = list;
      setCachedAlternatives(clean, list);
    } catch (e) {
      altsError = errorMessage(e);
      alts = [];
    } finally {
      loadingAlts = false;
    }
  }

  onMount(() => {
    if (lookupQuery) void loadAlternatives(lookupQuery);
  });

  function choosePackAlt(alt: ArtworkAlternative) {
    selectedAlt = alt;
    plan = null;
    actionError = '';
  }

  async function pickLocalFile() {
    localError = '';
    plan = null;
    actionError = '';
    const picked = await open({
      directory: false,
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg'] }],
    });
    if (!picked || Array.isArray(picked)) return;
    localPath = picked;
    await refreshLocalPreview();
  }

  async function refreshLocalPreview() {
    if (!localPath) return;
    localLoading = true;
    localError = '';
    plan = null;
    try {
      localPreviewUrl = await artworkPreviewLocal(
        localPath,
        slot,
        localFit,
        device === 'analogue_pocket',
      );
    } catch (e) {
      localError = errorMessage(e);
      localPreviewUrl = null;
    } finally {
      localLoading = false;
    }
  }

  function changeFit(next: ArtworkFit) {
    localFit = next;
    void refreshLocalPreview();
  }

  async function reviewPocketChange() {
    if (!canPocketWrite || !hasSelection) return;
    busy = true;
    actionError = '';
    try {
      plan = await artworkPlanApply({
        card,
        slot,
        targetId: effectiveTarget,
        packId: tab === 'packs' ? selectedAlt?.pack_id ?? null : null,
        packPlatformId: tab === 'packs' ? selectedAlt?.matched_platform || lookupQuery.trim() : null,
        sourcePath: tab === 'local' ? localPath : null,
        fit: tab === 'local' ? localFit : null,
      });
    } catch (e) {
      actionError = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function confirmPocketChange() {
    if (!plan || !canPocketWrite) return;
    busy = true;
    actionError = '';
    try {
      const appliedPreview = plan.preview_data_url;
      await artworkExecuteApply({
        card,
        slot,
        targetId: effectiveTarget,
        packId: tab === 'packs' ? selectedAlt?.pack_id ?? null : null,
        packPlatformId: tab === 'packs' ? selectedAlt?.matched_platform || lookupQuery.trim() : null,
        sourcePath: tab === 'local' ? localPath : null,
        fit: tab === 'local' ? localFit : null,
        confirmation: plan.id,
      });
      onApplied(appliedPreview);
      window.dispatchEvent(
        new CustomEvent('tau-artwork-changed', {
          detail: { platformId: effectiveTarget, packId: selectedAlt?.pack_id ?? null },
        }),
      );
      close();
    } catch (e) {
      actionError = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function applyAppOverride() {
    if (!overrideKey || !hasSelection) return;
    busy = true;
    actionError = '';
    try {
      const url = await artworkSetOverride({
        overrideKey,
        packId: tab === 'packs' ? selectedAlt?.pack_id ?? null : null,
        platformId: tab === 'packs' ? selectedAlt?.matched_platform || lookupQuery.trim() : null,
        sourcePath: tab === 'local' ? localPath : null,
      });
      onApplied(url);
      window.dispatchEvent(
        new CustomEvent('tau-artwork-changed', {
          detail: { overrideKey, platformId: lookupQuery.trim() },
        }),
      );
      close();
    } catch (e) {
      actionError = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function resetOverride() {
    if (!overrideKey) return;
    busy = true;
    actionError = '';
    try {
      await artworkClearOverride(overrideKey);
      onApplied(null);
      window.dispatchEvent(
        new CustomEvent('tau-artwork-changed', { detail: { overrideKey, reset: true } }),
      );
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
<div class="art-backdrop" role="presentation" on:click={close}></div>
<div
  class="art-sheet"
  role="dialog"
  aria-modal="true"
  aria-labelledby="art-picker-title"
  use:modal
>
  <header class="art-head">
    <div>
      <p class="eyebrow">ALTERNATIVE ARTWORK</p>
      <h2 id="art-picker-title">{title}</h2>
      <p class="art-sub">
        {#if device === 'analogue_pocket'}
          Writes <code>Platforms/_images/{effectiveTarget}.bin</code> on this card after review.
        {:else}
          Saved locally in Tau Omega as a custom banner for this core.
        {/if}
      </p>
    </div>
    <button class="quiet" aria-label="Close artwork picker" on:click={close}>✕</button>
  </header>

  <div class="art-tabs" role="tablist" aria-label="Artwork source">
    <button
      role="tab"
      class:current={tab === 'packs'}
      aria-selected={tab === 'packs'}
      on:click={() => {
        tab = 'packs';
        plan = null;
      }}
    >
      Community packs
    </button>
    <button
      role="tab"
      class:current={tab === 'local'}
      aria-selected={tab === 'local'}
      on:click={() => {
        tab = 'local';
        plan = null;
      }}
    >
      From computer
    </button>
  </div>

  <div class="art-body">
    {#if tab === 'packs'}
      <form
        class="art-search"
        on:submit|preventDefault={() => void loadAlternatives(lookupQuery)}
      >
        <label class="sr" for="art-plat-search">Platform ID to search in packs</label>
        <input
          id="art-plat-search"
          type="text"
          bind:value={lookupQuery}
          placeholder="Platform id (e.g. gb, gba, snes, lynx)"
        />
        <button class="quiet" type="submit" disabled={loadingAlts}>Search</button>
      </form>

      {#if loadingAlts}
        <div class="art-state">
          <span class="art-spinner" aria-hidden="true"></span>
          <strong>Fetching alternative artwork…</strong>
          <p>First-time pack downloads are cached locally so switching images later is instant.</p>
        </div>
      {:else if altsError}
        <div class="art-state err">
          <strong>Could not load community packs</strong>
          <p>{altsError}</p>
        </div>
      {:else if !alts.length}
        <div class="art-state">
          <strong>No pack artwork found for “{lookupQuery || 'this platform'}”</strong>
          <p>
            None of the community packs include a matching banner for that ID. Try another
            platform ID above or upload your own image under <b>From computer</b>.
          </p>
        </div>
      {:else}
        <p class="art-count">{alts.length} {alts.length === 1 ? 'banner' : 'banners'} found</p>
        <div class="art-grid" role="listbox" aria-label="Available banners">
          {#each alts as alt (alt.pack_id)}
            <button
              type="button"
              class="art-opt"
              class:selected={selectedAlt?.pack_id === alt.pack_id}
              on:click={() => choosePackAlt(alt)}
            >
              <span class="art-frame">
                <img src={alt.data_url} alt={`${alt.pack_name} banner`} />
              </span>
              <span class="art-meta">
                <strong>{alt.owner}{alt.variant ? ` · ${alt.variant.replace(/_/g, ' ')}` : ''}</strong>
                <small>{alt.matched_platform}.bin</small>
              </span>
            </button>
          {/each}
        </div>
      {/if}
    {:else}
      <div class="art-local">
        <div class="art-local-bar">
          <button class="picker" type="button" on:click={pickLocalFile}>Choose PNG or JPEG…</button>
          {#if localPath}<code class="art-path">{localPath}</code>{/if}
        </div>

        {#if device === 'analogue_pocket'}
          <div class="art-fit" role="group" aria-label="Image fit">
            <span>Fit</span>
            {#each [['contain', 'Contain'], ['cover', 'Cover'], ['stretch', 'Stretch']] as [mode, label]}
              <button
                type="button"
                class:active={localFit === mode}
                on:click={() => changeFit(mode as ArtworkFit)}
              >
                {label}
              </button>
            {/each}
          </div>
        {/if}

        {#if localLoading}
          <div class="art-state">
            <span class="art-spinner" aria-hidden="true"></span>
            <p>Preparing preview…</p>
          </div>
        {:else if localError}
          <div class="art-state err">
            <strong>Could not preview this image</strong>
            <p>{localError}</p>
          </div>
        {:else if localPreviewUrl}
          <div class="art-preview-box">
            <span class="art-frame large">
              <img src={localPreviewUrl} alt="Local artwork preview" />
            </span>
            <p>
              {#if device === 'analogue_pocket'}
                Live 521×165 monochrome Pocket preview ({localFit}).
              {:else}
                Preview of your selected banner image.
              {/if}
            </p>
          </div>
        {:else if currentUrl}
          <div class="art-preview-box muted">
            <span class="art-frame large">
              <img src={currentUrl} alt="Current artwork" />
            </span>
            <p>Current artwork. Choose a PNG or JPEG above to replace it.</p>
          </div>
        {:else}
          <div class="art-state">
            <strong>Choose a PNG or JPEG from your computer</strong>
            <p>
              {#if device === 'analogue_pocket'}
                Tau Omega converts it to a 521×165 Pocket banner and lets you preview it before
                writing to your card.
              {:else}
                Your image will be stored locally and shown as this core’s banner.
              {/if}
            </p>
          </div>
        {/if}
      </div>
    {/if}

    {#if plan}
      <div class="art-plan" role="region" aria-label="Artwork change review">
        <div>
          <strong>Ready to write <code>{plan.target_rel}</code></strong>
          <p>
            {plan.target_exists
              ? 'Your existing banner will be backed up outside the card first.'
              : 'No banner exists yet — this adds a new image file.'}
          </p>
        </div>
        <span class="art-frame thumb">
          <img src={plan.preview_data_url} alt="Planned banner preview" />
        </span>
      </div>
    {/if}

    {#if actionError}
      <p class="art-err" role="alert">{actionError}</p>
    {/if}
  </div>

  <footer class="art-foot">
    <div class="art-foot-left">
      {#if device !== 'analogue_pocket' && hasOverride}
        <button class="quiet" type="button" disabled={busy} on:click={resetOverride}>
          Reset to default
        </button>
      {/if}
    </div>
    <div class="art-foot-right">
      <button class="quiet" type="button" disabled={busy} on:click={close}>Cancel</button>
      {#if device === 'analogue_pocket'}
        {#if plan}
          <button class="primary" type="button" disabled={busy} on:click={confirmPocketChange}>
            {busy ? 'Applying…' : 'Apply to card'}
          </button>
        {:else}
          <button
            class="primary"
            type="button"
            disabled={busy || !hasSelection || !canPocketWrite}
            on:click={reviewPocketChange}
          >
            {busy ? 'Planning…' : 'Review change'}
          </button>
        {/if}
      {:else}
        <button
          class="primary"
          type="button"
          disabled={busy || !hasSelection || !overrideKey}
          on:click={applyAppOverride}
        >
          {busy ? 'Saving…' : 'Use this image'}
        </button>
      {/if}
    </div>
  </footer>
</div>

<style>
  .art-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(7, 11, 12, 0.72);
    backdrop-filter: blur(4px);
    z-index: 60;
  }
  .art-sheet {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(760px, calc(100vw - 32px));
    max-height: min(86vh, 760px);
    background: #141c1d;
    border: 1px solid #304142;
    border-radius: 16px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    z-index: 61;
  }
  .art-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
    padding: 20px 24px 12px;
  }
  .art-head h2 {
    margin: 2px 0 4px;
    font-size: 20px;
  }
  .art-sub {
    margin: 0;
    font-size: 12.5px;
    color: #93a3a2;
  }
  .art-tabs {
    display: flex;
    gap: 6px;
    padding: 0 24px;
    border-bottom: 1px solid #283536;
  }
  .art-tabs [role='tab'] {
    padding: 9px 14px;
    background: transparent;
    color: #9cacab;
    font-size: 13px;
    border-radius: 8px 8px 0 0;
  }
  .art-tabs [role='tab'].current {
    color: #c1f0ad;
    box-shadow: inset 0 -2px 0 #c1f0ad;
  }
  .art-body {
    flex: 1;
    min-height: 220px;
    overflow: auto;
    padding: 18px 24px;
    display: grid;
    gap: 14px;
    align-content: start;
  }
  .art-search {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .art-count {
    margin: 0;
    font-size: 12px;
    color: #8f9e9d;
  }
  .art-search input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 13px;
    color: #e8ecec;
    background: #0e1415;
    border: 1px solid #334345;
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
  .art-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(290px, 1fr));
    gap: 12px;
  }
  .art-opt {
    display: grid;
    gap: 8px;
    padding: 10px;
    text-align: left;
    background: #182224;
    border: 1.5px solid #2c393a;
    border-radius: 11px;
    color: inherit;
    cursor: pointer;
  }
  .art-opt:hover {
    border-color: #486062;
  }
  .art-opt.selected {
    border-color: #c1f0ad;
    background: #1d2c22;
  }
  .art-frame {
    display: block;
    background: #000;
    border: 1px solid #263334;
    border-radius: 8px;
    overflow: hidden;
  }
  .art-frame img {
    display: block;
    width: 100%;
    height: auto;
    aspect-ratio: 521 / 165;
    object-fit: contain;
  }
  .art-frame.large img {
    max-height: 180px;
  }
  .art-frame.thumb {
    width: 148px;
    flex: none;
  }
  .art-meta {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  .art-meta strong {
    font-size: 12.5px;
    color: #eff5f4;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .art-meta small {
    font-size: 11px;
    color: #8f9e9d;
    flex: none;
  }
  .art-local {
    display: grid;
    gap: 12px;
  }
  .art-local-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .art-path {
    font-size: 11.5px;
    color: #93a3a2;
    overflow-wrap: anywhere;
  }
  .art-fit {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .art-fit span {
    font-size: 12px;
    color: #8f9e9d;
    margin-right: 4px;
  }
  .art-fit button {
    padding: 5px 11px;
    font-size: 12px;
    background: #1a2325;
    border: 1px solid #2c393a;
    border-radius: 7px;
    color: #c7d1d0;
  }
  .art-fit button.active {
    border-color: #c1f0ad;
    color: #c1f0ad;
    background: #1d2c22;
  }
  .art-preview-box {
    display: grid;
    gap: 8px;
  }
  .art-preview-box p {
    margin: 0;
    font-size: 12px;
    color: #8f9e9d;
  }
  .art-state {
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 6px;
    padding: 32px 20px;
    border: 1px dashed #304042;
    border-radius: 12px;
    background: #101718;
  }
  .art-state strong {
    font-size: 14px;
    color: #eff5f4;
  }
  .art-state p {
    margin: 0;
    max-width: 440px;
    font-size: 12.5px;
    color: #93a3a2;
    line-height: 1.5;
  }
  .art-state.err {
    border-color: #6b3535;
    background: #231515;
  }
  .art-spinner {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2px solid #344244;
    border-top-color: #c1f0ad;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .art-plan {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 12px 14px;
    border-radius: 11px;
    border: 1px solid #3f6043;
    background: #18261b;
  }
  .art-plan strong {
    display: block;
    font-size: 13px;
    color: #e8ecec;
    margin-bottom: 3px;
  }
  .art-plan p {
    margin: 0;
    font-size: 12px;
    color: #b7c9b9;
  }
  .art-err {
    margin: 0;
    padding: 9px 12px;
    border-radius: 9px;
    border: 1px solid #6b3535;
    background: #261616;
    color: #f2b8b5;
    font-size: 12.5px;
  }
  .art-foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 14px 24px 18px;
    border-top: 1px solid #283536;
  }
  .art-foot-right {
    display: flex;
    gap: 8px;
  }
</style>
