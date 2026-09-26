<script lang="ts">
  import type { Plan, CapacityCheck } from './types';
  export let sources = '';
  export let destination = '';
  export let manifestPath = '';
  export let embedCovers = false;
  export let artSidecar = false;
  export let chooseSource: () => void;
  export let chooseDestination: () => void;
  export let chooseManifest: () => void;
  export let makePlan: () => void;
  export let plan: Plan | null = null;
  export let syncCapacity: CapacityCheck | null = null;
  export let artPreviews: Record<string, string> = {};
  export let artPreviewsLoading: Record<string, boolean> = {};
  export let loadArtPreview: (coverSource: string) => void;
  export let runSync: () => void;
  export let syncProgress = '';
  export let cancelSync: () => void;
  export let syncNotice = '';
  // Matches App.svelte's own formatting exactly (each extracted view keeps
  // its own copy, same precedent as BackupView.svelte's slightly different
  // one) rather than threading a prop through for one small pure function.
  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
</script>
<section class="page sync-page">
  <header><div><p class="eyebrow">SAFE SYNC</p><h1>Review before copying</h1><p class="lede">Sources are read-only. Every file is verified, then the index is published last.</p></div></header>
  <section class="wizard" aria-labelledby="sync-title">
    <div class="steps" aria-label="Sync steps"><span class="current">1 Sources</span><span class:current={plan}>2 Plan</span><span>3 Confirm</span></div>
    <h2 id="sync-title">Build a sync plan</h2>
    <label for="sources">Source folders or files — one per line</label>
    <div class="picker-row"><textarea id="sources" bind:value={sources} placeholder="/Users/me/Music/Album"></textarea><button class="picker" on:click={chooseSource}>Choose</button></div>
    <label for="destination">Destination media root</label>
    <div class="picker-row"><input id="destination" bind:value={destination} placeholder="/Volumes/Pocket/Assets/tau/common"/><button class="picker" on:click={chooseDestination}>Choose</button></div>
    <label for="manifest">Report location on this computer</label>
    <div class="picker-row"><input id="manifest" bind:value={manifestPath} placeholder="/Users/me/Documents/tau-sync-report.json"/><button class="picker" on:click={chooseManifest}>Choose</button></div>
    <label class="cover-option"><input type="checkbox" bind:checked={embedCovers}/> Add folder cover art to MP3 and FLAC copies <small>Baseline JPEG only. Your originals are never changed.</small></label>
    <label class="cover-option"><input type="checkbox" bind:checked={artSidecar}/> Write a tau-art cover thumbnail alongside each album <small>tau-alpha's decided format (palette-256, 128px); no firmware reader exists yet, so this has no effect on the Pocket today.</small></label>
    <button class="primary" on:click={makePlan}>Review plan</button>
  </section>
  {#if plan}
    <section class="plan-card" aria-labelledby="plan-title">
      <div>
        <p class="eyebrow">READY FOR CONFIRMATION</p>
        <h2 id="plan-title">{plan.id}</h2>
        <p class="lede">{plan.new_files} new · {plan.updates} updated · {plan.unchanged} unchanged · {size(plan.bytes_to_write)} to write{plan.art_sidecars ? ` · ${plan.art_sidecars} art sidecar${plan.art_sidecars === 1 ? '' : 's'}` : ''}</p>
        {#if syncCapacity}<p class:capacity-ok={syncCapacity.fits} class:capacity-bad={!syncCapacity.fits}>{syncCapacity.fits ? 'Fits' : 'Does not fit'} on the destination volume — {size(syncCapacity.space.available_bytes)} free.</p>{/if}
        {#if plan.art_sidecar_previews.length}
          <div class="art-preview-list">
            {#each plan.art_sidecar_previews as preview (preview.cover_source)}
              <div class="art-preview-item">
                <span class="art-preview-folder" title={preview.cover_source}>{preview.folder}</span>
                {#if artPreviews[preview.cover_source] === 'error'}<span class="art-preview-error">Could not preview</span>
                {:else if artPreviews[preview.cover_source]}<img class="art-preview-thumb" src={artPreviews[preview.cover_source]} alt="Palette-256 preview of the {preview.folder} cover"/>
                {:else}<button class="quiet" disabled={artPreviewsLoading[preview.cover_source]} on:click={() => loadArtPreview(preview.cover_source)}>{artPreviewsLoading[preview.cover_source] ? 'Loading…' : 'Preview'}</button>{/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
      <button class="danger" on:click={runSync}>Confirm and sync</button>
      {#if syncProgress}<p class="notice" role="status">{syncProgress} <button class="quiet" on:click={cancelSync}>Cancel</button></p>{/if}
      {#if plan.warnings.length}<ul>{#each plan.warnings as warning}<li>{warning.message}</li>{/each}</ul>{/if}
      <p class="safety">This action targets the exact destination above. It copies, hashes, verifies, and writes the index last.</p>
    </section>
  {/if}
  <p class="notice" role="status">{syncNotice}</p>
</section>
