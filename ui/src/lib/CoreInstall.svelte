<script lang="ts">
  // Download a catalog core, vet it, then hand the zip to the same reviewed install plan every package uses.
  // Nothing is written to the card here: UpdatePlanView shows the plan and asks for the confirmation.
  import { createEventDispatcher } from 'svelte';
  import UpdatePlanView from './UpdatePlanView.svelte';
  import { errorMessage } from './backend';
  import { marketplaceDownload } from './tau-api';
  import { size } from './marketplace';
  import type { MarketplaceDownload } from './types';
  export let coreId: string; export let card: string;
  /** A specific version from the catalog's history; empty means the newest. */
  export let version = '';
  /** The button label before anything is downloaded. */
  export let label = 'Download and review';
  const dispatch = createEventDispatcher<{ installed: void }>();
  let download: MarketplaceDownload | null = null; let busy = false; let error = '';
  async function fetchZip() {
    busy = true; error = ''; download = null;
    try { download = await marketplaceDownload(coreId, version || null); }
    catch (e) { error = errorMessage(e); }
    finally { busy = false; }
  }
</script>
{#if !download}
  <p class="ci-small">Omega downloads the zip, checks it, and shows exactly what would change on <strong>{card.split('/').pop()}</strong>. Nothing is written yet.</p>
  <button class="primary" disabled={busy} on:click={fetchZip}>{busy ? 'Downloading and checking…' : label}</button>
  {#if error}<p class="ci-err" role="alert">{error}</p>{/if}
{:else}
  <p class="ci-small" role="status">Downloaded {size(download.download.bytes)} · {download.download.checksum === 'Verified' ? 'checksum verified against GitHub’s' : 'no checksum was published for this release'} · {download.vetted.files} files{download.vetted.asset_files ? ` (${download.vetted.asset_files} in Assets)` : ''}.
    {download.vetted.firmware_required ? `Needs Pocket firmware ${download.vetted.firmware_required} or newer.` : 'The core does not state a minimum Pocket firmware.'}
    {download.vetted.dock_supported === false ? ' Not supported with the Dock.' : ''}</p>
  <UpdatePlanView zipPath={download.download.path} cardPath={card} on:installed={() => dispatch('installed')} />
{/if}
<style>
  .ci-small{font-size:12px;color:#a6b3b2;line-height:1.55;margin:6px 0}
  .ci-err{color:#ff9b9b;font-size:12px}
</style>
