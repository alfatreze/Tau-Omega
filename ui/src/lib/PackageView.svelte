<script lang="ts">
  import type { Core, PackageManifest, PackagePlan, PackageReport, RemovePlan, RemoveReport } from './types';
  export let zipPath = ''; export let cardPath = '';
  export let chooseZip: () => void; export let chooseCard: () => void;
  export let inspect: () => void; export let manifest: PackageManifest | null = null; export let inspectNotice = '';
  export let review: () => void; export let plan: PackagePlan | null = null; export let planNotice = '';
  export let confirm: () => void; export let report: PackageReport | null = null; export let confirmNotice = '';

  export let loadCoresToRemove: () => void; export let removeCores: Core[] = []; export let removeCoresNotice = '';
  export let removeCoreId = ''; export let reviewRemove: () => void; export let removePlan: RemovePlan | null = null; export let removePlanNotice = '';
  export let confirmRemove: () => void; export let removeReport: RemoveReport | null = null; export let removeConfirmNotice = '';

  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : bytes < 1024 * 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  const STATE_LABEL: Record<string, string> = { only_left: 'New', different: 'Update', identical: 'Unchanged', only_right: 'Card only' };
</script>
<section class="page" aria-labelledby="package-title">
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

  <header><div><p class="eyebrow">CORE PACKAGES</p><h1>Remove an installed core</h1><p class="lede">Checks every other installed core on this card first. Files shared with another core (the platform's <code>Assets/&lt;platform&gt;/common</code> media root and its <code>Platforms</code> entry) are only removed when no sibling core still needs them.</p></div></header>

  <section class="settings-card">
    <div style="width:100%">
      <h2>Choose a card</h2>
      <p>Uses the staging card folder above.</p>
      <button class="primary" disabled={!cardPath.trim()} on:click={loadCoresToRemove}>List installed cores</button>
      <p class="notice" role="status">{removeCoresNotice}</p>
    </div>
  </section>

  {#if removeCores.length}
    <section class="settings-card">
      <div style="width:100%">
        <h2>Pick a core to remove</h2>
        <label for="remove-core-select">Installed core</label>
        <select id="remove-core-select" bind:value={removeCoreId}>
          <option value="" disabled>Choose a core…</option>
          {#each removeCores as core}<option value={core.id}>{core.id} ({core.platform}, v{core.version})</option>{/each}
        </select>
        <button class="primary" disabled={!removeCoreId} on:click={reviewRemove}>Plan removal</button>
        <p class="notice" role="status">{removePlanNotice}</p>
      </div>
    </section>
  {/if}

  {#if removePlan}
    <section class="settings-card">
      <div style="width:100%">
        <h2>{removePlan.files_to_remove} files · {size(removePlan.bytes_to_remove)} to delete</h2>
        {#if removePlan.platform_shared}
          <p class="safety">Another installed core still uses platform <code>{removePlan.platform}</code>, so its shared <code>Assets/{removePlan.platform}/common</code> and <code>Platforms</code> files are kept.</p>
        {:else if removePlan.platform}
          <p class="safety">No other installed core uses platform <code>{removePlan.platform}</code>, so its shared platform files are removed too.</p>
        {/if}
        <ul class="difference-list remove-list">
          {#each removePlan.paths as removePath}<li><span>{removePath}</span></li>{/each}
        </ul>
        <button class="danger" on:click={confirmRemove}>Confirm and remove</button>
        <p class="notice" role="status">{removeConfirmNotice}</p>
      </div>
    </section>
  {/if}

  {#if removeReport}
    <section class="settings-card"><div><h2>Removed</h2><p>{removeReport.removed_files} files deleted · {size(removeReport.bytes_removed)} freed</p></div><span class="chip capable">Verified</span></section>
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
  .remove-list li { display: block; }
  .remove-list li > span { color: #c7d1d0; }
</style>
