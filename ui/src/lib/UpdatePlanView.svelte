<script lang="ts">
  import SettingsMigration from './SettingsMigration.svelte';
  import { planCoreUpdate, executeCoreUpdate, rollbackCoreUpdate } from './tau-api';
  import { errorMessage } from './backend';
  import type { InstallPlan, InstallReport, RollbackReport, UpdateVerdict } from './types';
  /** The package zip and the card folder chosen on the page above. */
  export let zipPath = ''; export let cardPath = '';

  let migrateFrom: string[] = []; let migrateTo: string[] = []; let plan: InstallPlan | null = null; let report: InstallReport | null = null; let undone: RollbackReport | null = null;
  let allowDowngrade = false; let busy = false; let notice = ''; let confirmRollback = false;

  const size = (b: number) => b < 1024 ? `${b} B` : b < 1024 * 1024 * 1024 ? `${(b / 1024 / 1024).toFixed(1)} MB` : `${(b / 1024 / 1024 / 1024).toFixed(2)} GB`;
  const VERDICT: Record<UpdateVerdict, string> = {
    new_install: 'New install', same_build: 'Already up to date', update: 'Update', same_date_different_build: 'Different build, same day', older: 'Older build (downgrade)', mismatch: 'Cannot be installed',
  };
  function pairText(pair: unknown): string {
    if (typeof pair === 'object' && pair && 'verified' in pair) return 'Firmware and bitstream match.';
    if (typeof pair === 'object' && pair && 'missing_feature' in pair) return `The firmware needs a bitstream feature this build lacks: the Pocket would show "NO UNIT".`;
    if (typeof pair === 'object' && pair && 'mismatch' in pair) return 'Firmware and bitstream do NOT match: the Pocket would show a black screen.';
    return pair === 'no_marker' ? 'An older firmware: its match with the bitstream cannot be checked.' : 'This bitstream is not a known build: the match cannot be checked.';
  }

  async function review() {
    busy = true; notice = ''; report = null; undone = null; confirmRollback = false;
    try { plan = await planCoreUpdate(zipPath, cardPath, allowDowngrade); }
    catch (error) { plan = null; notice = `Could not plan this: ${errorMessage(error)}`; }
    finally { busy = false; }
  }
  async function install() {
    if (!plan || plan.refused) return;
    busy = true; notice = '';
    try { const replaced = plan.superseded_candidates.filter((id) => !/DEV|TAU_0_/.test(id)); const installed = plan.update.cores.map((c) => c.package.core_id); report = await executeCoreUpdate(zipPath, cardPath, allowDowngrade, plan.id); migrateFrom = replaced; migrateTo = installed; plan = null; }
    catch (error) { notice = `Nothing was left half-done. ${errorMessage(error)}`; }
    finally { busy = false; }
  }
  async function rollBack() {
    if (!report) return;
    busy = true; notice = '';
    try { undone = await rollbackCoreUpdate(cardPath, report.backup_dir); confirmRollback = false; }
    catch (error) { notice = `The card was not changed by this: ${errorMessage(error)}`; }
    finally { busy = false; }
  }
  $: reviewable = plan && !plan.refused && !plan.nothing_to_do;
</script>

<section class="settings-card">
  <div style="width:100%">
    <h2>Review the update</h2>
    <p class="safety">Shows exactly what would happen to this card before anything is written: what is backed up, replaced, cleaned and kept. Nothing changes until you confirm.</p>
    <button class="primary" disabled={busy || !zipPath.trim() || !cardPath.trim()} on:click={review}>{busy && !plan ? 'Reviewing…' : 'Review update'}</button>
    <p class="notice" role="status">{notice}</p>
  </div>
</section>

{#if plan}
  {#each plan.update.cores as core}
    <section class="settings-card">
      <div style="width:100%">
        <p class="eyebrow">{core.package.shortname}</p>
        <h2>{VERDICT[core.verdict]}{#if core.package_release} · {core.package_release}{/if}</h2>
        {#each core.reasons as reason}<p>{reason}</p>{/each}
        <p class="safety">{pairText(core.pair)}</p>
        {#if core.verdict === 'older'}
          <label class="inline"><input type="checkbox" bind:checked={allowDowngrade} on:change={review} /> Install this older build anyway (a downgrade)</label>
        {/if}
      </div>
    </section>
  {/each}

  {#if plan.refused}
    <section class="settings-card"><div style="width:100%"><h2>Not installed</h2><p class="notice" role="alert">{plan.refused}</p></div></section>
  {:else if plan.nothing_to_do}
    <section class="settings-card"><div><h2>Nothing to do</h2><p>This card already has exactly this build.</p></div><span class="chip capable">Up to date</span></section>
  {/if}

  {#each plan.cautions as caution}<p class="safety" role="note">⚠ {caution}</p>{/each}

  {#if reviewable}
    <section class="comparison-counts package-counts">
      <span><b>{plan.files.new_files}</b> new</span><span><b>{plan.files.updated_files}</b> replaced</span><span><b>{plan.files.unchanged_files}</b> unchanged</span>
    </section>
    <section class="settings-card">
      <div style="width:100%">
        <h2>Safety net</h2>
        <p><b>{plan.backup.length}</b> existing {plan.backup.length === 1 ? 'file is' : 'files are'} copied to a backup folder on this computer first ({size(plan.backup_bytes)}), and each copy is read back and checked. If anything goes wrong the card is put back by itself, and you can undo the update afterwards too.</p>
        {#if plan.caches_to_clear.length}<p><b>{plan.caches_to_clear.length}</b> stale Pocket catalog cache files are cleared (also backed up) so the Pocket rescans and shows the core.</p>{/if}
        {#if plan.stubs_to_sweep.length}<p><b>{plan.stubs_to_sweep.length}</b> leftover <code>._</code> files from Finder are removed.</p>{/if}
        {#if plan.obsolete_to_remove.length}<p><b>{plan.obsolete_to_remove.length}</b> files this release no longer uses are removed: {plan.obsolete_to_remove.join(', ')}.</p>{/if}
        {#if plan.user_files_kept.length}<p class="safety">Kept exactly as they are: {plan.user_files_kept.join(', ')}. Your music and other cores are never touched.</p>{/if}
        {#if plan.update.persist_changed?.length}<p class="safety">This release changes how {plan.update.persist_changed.length} saved {plan.update.persist_changed.length === 1 ? 'setting is' : 'settings are'} stored; the core may reset them.</p>{/if}
        {#if plan.superseded_candidates.length}<p class="safety">Older test builds on this card ({plan.superseded_candidates.join(', ')}) are left alone; remove them separately if you no longer need them.</p>{/if}
        {#if plan.capacity}<p class:capacity-ok={plan.capacity.fits} class:capacity-bad={!plan.capacity.fits}>{plan.capacity.fits ? 'Fits' : 'Does not fit'} on the card — {size(plan.capacity.space.available_bytes)} free, {size(plan.capacity.bytes_needed)} needed.</p>{/if}
        <button class="danger" disabled={busy} on:click={install}>{busy ? 'Installing…' : 'Confirm and install'}</button>
      </div>
    </section>
    <section class="difference-section">
      <ul class="difference-list">
        {#each plan.files.items as item}
          <li><span class={`difference ${item.state}`}>{item.state === 'only_left' ? 'New' : item.state === 'different' ? 'Replace' : 'Unchanged'}</span><span>{item.path}</span><span>{size(item.bytes)}</span></li>
        {/each}
      </ul>
    </section>
  {/if}
{/if}

{#if report && !report.nothing_to_do}<SettingsMigration {cardPath} fromCores={migrateFrom} toCores={migrateTo} />{/if}
{#if report}
  <section class="settings-card">
    <div style="width:100%">
      <h2>{report.nothing_to_do ? 'Nothing was changed' : report.checks.every((c) => c.verdict !== 'fail') ? 'Installed and checked' : 'Installed, but a check failed'}</h2>
      {#if !report.nothing_to_do}
        <p>{report.files_written} files written · {size(report.bytes_written)} · {report.files_backed_up} backed up · {report.caches_cleared} caches cleared · {report.stubs_swept} stray files removed.</p>
        <p class="safety">Backup and journal: <code>{report.backup_dir}</code></p>
      {/if}
      {#each report.checks as check}
        <h3>{check.summary}</h3>
        <ul class="difference-list">
          {#each check.items as item}<li><span class={`difference ${item.status === 'pass' ? 'identical' : item.status === 'warn' ? 'different' : 'only_right'}`}>{item.status === 'pass' ? 'Pass' : item.status === 'warn' ? 'Note' : 'Fail'}</span><span>{item.name}</span><span>{item.detail}</span></li>{/each}
        </ul>
      {/each}
      {#if !report.nothing_to_do && !undone}
        {#if !confirmRollback}
          <button class="quiet" disabled={busy} on:click={() => (confirmRollback = true)}>Undo this update…</button>
        {:else}
          <span role="alert">Put the card back as it was before this update? <button class="danger" disabled={busy} on:click={rollBack}>Yes, undo</button> <button class="quiet" on:click={() => (confirmRollback = false)}>Keep it</button></span>
        {/if}
      {/if}
      {#if undone}<p class="notice" role="status">Undone: {undone.restored} files restored, {undone.created_removed} new files removed.</p>{/if}
    </div>
  </section>
{/if}
<style>
  .package-counts { max-width: 540px; }
  .difference-section { max-width: 720px; }
  .difference-list { padding: 0; margin: 0; border: 1px solid #344244; border-radius: 9px; overflow: auto; max-height: 380px; }
  .difference-list li { display: grid; grid-template-columns: 90px 1fr 90px; gap: 12px; align-items: center; padding: 10px 12px; border-bottom: 1px solid #2c393a; font-size: 12px; color: #c7d1d0; list-style: none; }
  .difference-list li:last-child { border-bottom: 0; }
  .difference-list li > span:last-child { color: #8f9e9d; text-align: right; }
  .difference { font-size: 11px; font-weight: 700; }
  .difference.only_left { color: #b9e9a5; }
  .difference.different { color: #d9c47e; }
  .difference.identical { color: #8f9e9d; }
  .difference.only_right { color: #f29b83; }
  button.danger { margin-top: 10px; padding: 11px 14px; border-radius: 8px; background: #5a352c; color: #f4cfc4; font-weight: 700; }
  button.danger:disabled { opacity: .5; }
  label.inline { display: flex; gap: 8px; align-items: center; margin-top: 8px; }
  h3 { margin: 12px 0 6px; font-size: 14px; }
</style>
