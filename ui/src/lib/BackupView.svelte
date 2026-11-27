<script lang="ts">
  import type { BackupPlan, CapacityCheck } from './types';
  export let sourcePath = ''; export let destinationPath = '';
  export let chooseSource: () => void; export let chooseDestination: () => void;
  export let review: () => void;
  export let backupPlan: BackupPlan | null = null; export let backupNotice = '';
  export let capacity: CapacityCheck | null = null;

  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : bytes < 1024 * 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  const STATE_LABEL: Record<string, string> = { only_left: 'New', different: 'Would update', identical: 'Unchanged', only_right: 'Destination only' };
</script>
<section class="page" aria-labelledby="backup-title">
  <header><div><p class="eyebrow">STORAGE</p><h1 id="backup-title">Backup</h1><p class="lede">Preview what backing up a folder onto another would do, and whether it fits. This is a dry run: nothing is copied yet.</p></div></header>

  <section class="settings-card">
    <div style="width:100%">
      <h2>Plan a backup</h2>
      <label for="backup-source">Source folder</label>
      <div class="picker-row"><input id="backup-source" bind:value={sourcePath} placeholder="/Users/me/Music"/><button class="picker" on:click={chooseSource}>Choose</button></div>
      <label for="backup-destination">Backup destination</label>
      <div class="picker-row"><input id="backup-destination" bind:value={destinationPath} placeholder="/Volumes/Backup Drive/Music"/><button class="picker" on:click={chooseDestination}>Choose</button></div>
      <button class="primary" disabled={!sourcePath.trim() || !destinationPath.trim()} on:click={review}>Plan backup</button>
      <p class="notice" role="status">{backupNotice}</p>
    </div>
  </section>

  {#if backupPlan}
    <section class="comparison-counts backup-counts">
      <span><b>{backupPlan.new_files}</b> new</span>
      <span><b>{backupPlan.updated_files}</b> would update</span>
      <span><b>{backupPlan.unchanged_files}</b> unchanged</span>
      <span><b>{backupPlan.destination_only_files}</b> destination only</span>
    </section>

    <section class="settings-card">
      <div style="width:100%">
        <h2>{size(backupPlan.bytes_to_write)} to write</h2>
        {#if capacity}
          <p class:capacity-ok={capacity.fits} class:capacity-bad={!capacity.fits}>
            {capacity.fits ? 'Fits' : 'Does not fit'} on the destination volume — {size(capacity.space.available_bytes)} free of {size(capacity.space.total_bytes)}, {size(capacity.margin_bytes)} reserved as a safety margin.
          </p>
        {/if}
        <p class="safety">Backups only ever add or update files here; nothing is deleted. Copying isn't enabled yet — this is a read-only preview.</p>
      </div>
    </section>

    {#if backupPlan.items.length}
      <section class="difference-section">
        <ul class="difference-list">
          {#each backupPlan.items as item}
            <li><span class={`difference ${item.state}`}>{STATE_LABEL[item.state] ?? item.state}</span><span>{item.relative}</span><span>{size(item.bytes)}</span></li>
          {/each}
        </ul>
      </section>
    {/if}
  {/if}
</section>
<style>
  .backup-counts { max-width: 720px; }
  .capacity-ok { color: var(--accent); }
  .capacity-bad { color: var(--danger-text); font-weight: 600; }
  .difference-section { max-width: 720px; }
  .difference-list { padding: 0; margin: 0; border: 1px solid var(--border-strong); border-radius: 9px; overflow: auto; max-height: 380px; }
  .difference-list li { display: grid; grid-template-columns: 130px 1fr 90px; gap: 12px; align-items: center; padding: 10px 12px; border-bottom: 1px solid var(--border); font-size: 12px; color: var(--text-secondary); }
  .difference-list li:last-child { border-bottom: 0; }
  .difference-list li > span:last-child { color: var(--text-muted); text-align: right; }
  .difference { font-size: 11px; font-weight: 700; }
  .difference.only_left { color: var(--accent); }
  .difference.different { color: var(--warn-text); }
  .difference.identical { color: var(--text-muted); }
  .difference.only_right { color: var(--danger-text); }
</style>
