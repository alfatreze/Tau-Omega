<script lang="ts">
  import { executeSettingsMigration, planSettingsMigration, rollbackSettingsMigration, errorMessage } from './tau-api';
  import type { MigrationPlan, MigrationReport } from './types';

  /** The card, the cores this install replaced, and the cores it installed. */
  export let cardPath = ''; export let fromCores: string[] = []; export let toCores: string[] = [];

  let plan: MigrationPlan | null = null; let report: MigrationReport | null = null; let notice = ''; let busy = false;
  const pairs = () => fromCores.flatMap((from) => toCores.map((to) => ({ from, to })));

  async function check(from: string, to: string) {
    busy = true; notice = ''; report = null;
    try { plan = await planSettingsMigration(cardPath, from, to); } catch (error) { plan = null; notice = errorMessage(error); }
    busy = false;
  }
  async function copy() {
    if (!plan) return; busy = true;
    try { report = await executeSettingsMigration(cardPath, plan.from_core, plan.to_core, plan.id); notice = `Copied the saved settings of ${plan.from_core} to ${plan.to_core}. The old file was not touched.`; plan = null; }
    catch (error) { notice = errorMessage(error); }
    busy = false;
  }
  async function undo() {
    if (!report) return; busy = true;
    try { await rollbackSettingsMigration(cardPath, report); report = null; notice = 'The copied settings were removed again.'; } catch (error) { notice = errorMessage(error); }
    busy = false;
  }
</script>

{#if fromCores.length && toCores.length}
  <section class="settings-card">
    <div style="width:100%">
      <h2>Carry over saved settings?</h2>
      <p>The new core has a different name, so it starts with factory settings. Omega can copy the old core's saved settings across, but only when the release shows that none of them changed meaning.</p>
      {#each pairs() as pair}
        <button class="quiet" disabled={busy} on:click={() => check(pair.from, pair.to)}>Check {pair.from} → {pair.to}</button>
      {/each}
      {#if plan}
        {#if plan.allowed}
          <p class="safety">{plan.reasons.join(' ')} {plan.bytes} bytes will be copied to <code>{plan.dest}</code>. An existing file is never overwritten.</p>
          <button class="primary" disabled={busy} on:click={copy}>Copy settings</button>
        {:else}
          <p class="safety" role="status">Not copied: {plan.reasons.join(' ')}</p>
        {/if}
      {/if}
      {#if report}<button class="quiet" disabled={busy} on:click={undo}>Undo the copy</button>{/if}
      {#if notice}<p class="notice" role="status">{notice}</p>{/if}
    </div>
  </section>
{/if}
