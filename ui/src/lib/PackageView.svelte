<script lang="ts">
  import type { PackageManifest, PackagePlan, PackageReport } from './types';
  export let zipPath = ''; export let cardPath = '';
  export let chooseZip: () => void; export let chooseCard: () => void;
  export let inspect: () => void; export let manifest: PackageManifest | null = null; export let inspectNotice = '';
  export let review: () => void; export let plan: PackagePlan | null = null; export let planNotice = '';
  export let confirm: () => void; export let report: PackageReport | null = null; export let confirmNotice = '';

  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : bytes < 1024 * 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  const STATE_LABEL: Record<string, string> = { only_left: 'New', different: 'Update', identical: 'Unchanged', only_right: 'Card only' };
</script>
<section class="jobs-panel page" aria-labelledby="package-title">
  <header><div><p class="eyebrow">CORE PACKAGES</p><h1 id="package-title">Install a core package</h1><p class="lede">Inspect a release zip, plan what installing or updating it would change, then confirm. Every write is verified against the zip immediately before and after.</p></div></header>

  <section class="settings-card">
    <div style="width:100%">
      <h2>Choose a package and a card</h2>
      <label for="package-zip">Release package (.zip)</label>
      <div class="picker-row"><input id="package-zip" bind:value={zipPath} placeholder="/Users/me/Downloads/alfatreze.TAU_0.4.0.zip"/><button class="picker" on:click={chooseZip}>Choose</button></div>
      <label for="package-card">Staging card folder</label>
      <div class="picker-row"><input id="package-card" bind:value={cardPath} placeholder="/Volumes/My Pocket"/><button class="picker" on:click={chooseCard}>Choose</button></div>
      <button class="primary" disabled={!zipPath.trim()} on:click={inspect}>Inspect package</button>
      <p class="notice" role="status">{inspectNotice}</p>
    </div>
  </section>

  {#if manifest}
    <section class="settings-card">
      <div style="width:100%">
        <h2>{manifest.core_ids.join(', ') || 'No core found'}</h2>
        <p>{manifest.entries.length} files in this package.</p>
        <button class="primary" disabled={!cardPath.trim()} on:click={review}>Plan install</button>
        <p class="notice" role="status">{planNotice}</p>
      </div>
    </section>
  {/if}

  {#if plan}
    <section class="comparison-counts package-counts">
      <span><b>{plan.new_files}</b> new</span>
      <span><b>{plan.updated_files}</b> updated</span>
      <span><b>{plan.unchanged_files}</b> unchanged</span>
    </section>
    <section class="settings-card">
      <div style="width:100%">
        <h2>{size(plan.bytes_to_write)} to write</h2>
        <p class="safety">Only files this package declares are touched. Every entry is re-hashed against the zip immediately before writing and the written file is read back and verified after.</p>
        <button class="danger" on:click={confirm}>Confirm and install</button>
        <p class="notice" role="status">{confirmNotice}</p>
      </div>
    </section>
    <section class="difference-section">
      <ul class="difference-list">
        {#each plan.items as item}
          <li><span class={`difference ${item.state}`}>{STATE_LABEL[item.state] ?? item.state}</span><span>{item.path}</span><span>{size(item.bytes)}</span></li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if report}
    <section class="settings-card"><div><h2>Installed</h2><p>{report.written} files written · {report.unchanged} already up to date · {size(report.bytes_written)} written</p></div><span class="chip capable">Verified</span></section>
  {/if}
</section>
<style>
  .package-counts { max-width: 540px; }
  .difference-section { max-width: 720px; }
  .difference-list { padding: 0; margin: 0; border: 1px solid #344244; border-radius: 9px; overflow: auto; max-height: 380px; }
  .difference-list li { display: grid; grid-template-columns: 90px 1fr 90px; gap: 12px; align-items: center; padding: 10px 12px; border-bottom: 1px solid #2c393a; font-size: 12px; color: #c7d1d0; }
  .difference-list li:last-child { border-bottom: 0; }
  .difference-list li > span:last-child { color: #8f9e9d; text-align: right; }
  .difference { font-size: 11px; font-weight: 700; }
  .difference.only_left { color: #b9e9a5; }
  .difference.different { color: #d9c47e; }
  .difference.identical { color: #8f9e9d; }
  .difference.only_right { color: #f29b83; }
</style>
