<script lang="ts">
  // The Pocket does not skip a per-meter settings file of the wrong size: it stops at start with "Error in framework file id [11] size bad" and
  // the core does not run (Tau Alpha 032, measured). This tells the person, and offers the one thing that fixes it: delete that file, after a
  // reviewed plan and a verified copy outside the card. A settings file of the right size is never touched.
  import { meterSettingsDelete, meterSettingsPlanDelete, meterSettingsScan } from './tau-api';
  import { errorMessage } from './backend';
  import type { SettingsDeletePlan, SettingsFile } from './types';
  /** The card folder currently open (empty when none). */
  export let card = '';

  let files: SettingsFile[] = []; let askedFor = ''; let plan: SettingsDeletePlan | null = null; let busy = false; let notice = ''; let done = '';
  $: if (card && card !== askedFor) { askedFor = card; plan = null; done = ''; notice = ''; void scan(); }
  $: if (!card) { files = []; plan = null; askedFor = ''; }
  async function scan() { try { files = (await meterSettingsScan(card)).filter((f) => f.state === 'wrong_size'); } catch { files = []; } }
  async function review(f: SettingsFile) {
    busy = true; notice = '';
    try { plan = await meterSettingsPlanDelete(card, f.rel_path); }
    catch (e) { notice = errorMessage(e); }
    finally { busy = false; }
  }
  async function remove() {
    if (!plan || plan.blocked) return;
    busy = true; notice = '';
    try { const r = await meterSettingsDelete(card, plan.rel_path, plan.id); done = `Deleted. A copy is kept in ${r.backup_dir}. Start the core again; the Pocket makes a new file the next time you change a meter.`; plan = null; await scan(); }
    catch (e) { notice = `Nothing was deleted: ${errorMessage(e)}`; }
    finally { busy = false; }
  }
  const bytes = (n: number) => n.toLocaleString('en-US');
</script>
{#if files.length || done}
  <div class="sf-bar" role="region" aria-label="Meter settings file problem">
    {#if done}
      <span role="status">{done}</span>
      <span class="sf-actions"><button class="quiet" on:click={() => (done = '')}>Close</button></span>
    {:else if plan}
      <div class="sf-plan">
        <p><strong>Delete <code>{plan.rel_path}</code> ({bytes(plan.bytes)} bytes)?</strong></p>
        {#if plan.blocked}<p>{plan.blocked}</p>{:else}
          <p>Omega first copies it to a backup folder outside your card and checks the copy, then deletes the file. Your meters go back to their default settings, and the Pocket makes a new file the next time you change a meter. Nothing else on the card is touched.</p>{/if}
      </div>
      <span class="sf-actions">{#if !plan.blocked}<button class="primary" disabled={busy} on:click={remove}>{busy ? 'Deleting…' : 'Delete the file'}</button>{/if}<button class="quiet" disabled={busy} on:click={() => (plan = null)}>Cancel</button></span>
    {:else}
      <div class="sf-text">
        {#each files as f}
          <p><strong>{f.core} will not start on the Pocket.</strong> Its saved meter settings file is {bytes(f.bytes)} bytes, and the Pocket only accepts exactly 8,192, so it stops with “Error in framework file id [11] size bad”.</p>
        {/each}
        <p class="sf-small">Deleting that file fixes it. Your meters then start from their defaults.</p>
      </div>
      <span class="sf-actions">{#each files as f}<button class="primary" disabled={busy} on:click={() => review(f)}>Review{files.length > 1 ? ` ${f.core}` : ''}…</button>{/each}</span>
    {/if}
    {#if notice}<span class="sf-error" role="alert">{notice}</span>{/if}
  </div>
{/if}
<style>
  .sf-bar{display:flex;flex-wrap:wrap;gap:10px 16px;align-items:center;justify-content:space-between;margin:12px 16px 14px;padding:12px 56px 12px 16px;border:1px solid #6b4f27;border-left:3px solid #e5c88a;border-radius:10px;background:#2a2417;color:#f1e6cf;font-size:13px;line-height:1.55}
  .sf-bar p{margin:0 0 4px}.sf-small{color:#cdbf9f;font-size:12px}.sf-plan{flex:1 1 420px}.sf-text{flex:1 1 420px}
  .sf-actions{display:flex;gap:8px;flex-wrap:wrap}.sf-error{flex-basis:100%;color:#ff9b9b}
  code{font-size:12px;overflow-wrap:anywhere}
</style>
