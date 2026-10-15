<script lang="ts">
  // Updates for the community cores on the open card: only the cores that actually need one (a newer release, or a replacement the author
  // published). Checking is automatic only when the app's update check is on (Settings); installing never is: each update is its own reviewed
  // plan, one at a time.
  import CoreInstall from './CoreInstall.svelte';
  import { openManualAdd } from './manualAddStore';
  import { sourceNote } from './marketplace';
  import { checkUpdates, updatesAutomatic, updatesBusy, updatesError, updatesResult } from './updatesStore';
  import type { CoreStatus } from './types';
  export let card = '';

  /** The core whose plan is open, and the ids still waiting when "Update all" is running. */
  let openId = ''; let queue: string[] = []; let queueTotal = 0; let finishedNote = '';
  $: result = $updatesResult; $: busy = $updatesBusy; $: error = $updatesError; $: automatic = $updatesAutomatic;
  $: updatable = (result?.rows ?? []).filter((r) => r.status === 'UpdateAvailable');
  $: rows = (result?.rows ?? []).filter((r) => r.status === 'UpdateAvailable' || typeof r.status === 'object');
  $: hasTau = (result?.rows ?? []).some((r) => r.status === 'Excluded');
  const check = (force: boolean) => checkUpdates(card, force);

  const LABEL: Record<string, string> = { UpToDate: 'Up to date', UpdateAvailable: 'Update available', NewerThanCatalog: 'Newer than the list', Unknown: 'Cannot compare versions', NotInCatalog: 'Not in the library' };
  const label = (r: CoreStatus) => (typeof r.status === 'object' ? `Replaced by ${r.status.ReplacedBy}` : LABEL[r.status] ?? '');
  const target = (r: CoreStatus) => (typeof r.status === 'object' ? r.status.ReplacedBy : r.id);
  const actionable = (r: CoreStatus) => r.status === 'UpdateAvailable' || typeof r.status === 'object';

  function review(r: CoreStatus) { openId = openId === r.id ? '' : r.id; queue = []; finishedNote = ''; }
  // A manually added core is reviewed in the manual-add sheet (it asks for the trust tick again); the queue below is for catalog cores.
  $: queueable = updatable.filter((r) => !r.manual_url);
  function updateAll() { const ids = updatable.filter((r) => !r.manual_url).map((r) => r.id); queueTotal = ids.length; openId = ids[0] ?? ''; queue = ids.slice(1); finishedNote = ''; }
  async function installed() {
    await check(false);
    if (queue.length) { openId = queue[0]; queue = queue.slice(1); }
    else if (queueTotal) { finishedNote = `All ${queueTotal} updates were reviewed.`; queueTotal = 0; openId = ''; }
  }
  function skip() { if (queue.length) { openId = queue[0]; queue = queue.slice(1); } else { openId = ''; queueTotal = 0; } }
</script>
<section aria-label="Core updates" class="cu">
    <div class="cu-actions">
      {#if queueable.length > 1 && !queueTotal}<button class="quiet" on:click={updateAll}>Review all {queueable.length} updates, one at a time</button>{/if}
      <button class="quiet" disabled={busy || !card} on:click={() => check(true)}>{busy ? 'Checking…' : result ? 'Check again' : 'Check for updates'}</button>
    </div>
  {#if !card}<p class="cu-note">Open a card to see which of its cores have updates.</p>
  {:else if error}<p class="cu-err" role="alert">{error}</p>
  {:else if !result}<p class="cu-note" role="status">{busy ? 'Checking the openFPGA Library…' : automatic ? '' : 'Checking is off in Settings. Press Check for updates to look once.'}</p>
  {:else}
    <p class="cu-note" role="status" aria-live="polite">{#if rows.length}{rows.length} {rows.length === 1 ? 'core needs' : 'cores need'} updating{:else}Everything on this card is up to date{/if} · {sourceNote(result.source, result.fetched)}{#if hasTau}{' · '}Tau cores update through the Tau update bar{/if}</p>
    {#if finishedNote}<p class="cu-note" role="status">{finishedNote}</p>{/if}
    {#if rows.length}
      <div class="core-list">
        {#each rows as r (r.id)}
          <article class="cu-row" class:open={openId === r.id}>
            <div class="cu-line">
              <span class="core-icon" aria-hidden="true">{r.id.split('.').pop()?.slice(0, 2).toUpperCase()}</span>
              <div><h3>{r.id}</h3><p>{r.installed_version || 'unknown version'}{r.latest_version && r.status === 'UpdateAvailable' ? ` → ${r.latest_version}` : ''}</p></div>
              <span class="chip" class:capable={r.status === 'UpdateAvailable'}>{label(r)}</span>{#if r.manual_url}<span class="chip" title="Added manually from GitHub: not in the Library">added manually</span>{/if}
              {#if actionable(r) && r.manual_url}<button class="quiet" on:click={() => openManualAdd(r.manual_url ?? '')}>Review update</button>{:else if actionable(r)}<button class="quiet" aria-expanded={openId === r.id} on:click={() => review(r)}>{openId === r.id ? 'Close' : typeof r.status === 'object' ? 'Review replacement' : 'Review update'}</button>{/if}
            </div>
            {#if openId === r.id}
              <div class="cu-detail">
                {#if queueTotal}<p class="cu-note" role="status">Update {queueTotal - queue.length} of {queueTotal}. <button class="quiet" on:click={skip}>Skip this one</button></p>{/if}
                {#if typeof r.status === 'object'}<p class="cu-note">The author has released this core under a new name. Installing it leaves <code>{r.id}</code> on the card until you remove it.</p>{/if}
                <CoreInstall coreId={target(r)} {card} label="Download and review update" on:installed={installed} />
              </div>
            {/if}
          </article>
        {/each}
      </div>
    {:else}<div class="cu-empty"><span aria-hidden="true">✓</span><h3>Nothing to update</h3><p>Every community core on this card is on its latest release.</p></div>{/if}
  {/if}
</section>
<style>
  .cu{margin-top:4px}
  .cu-empty{display:grid;justify-items:center;gap:4px;text-align:center;padding:34px 16px;border:1px dashed #2c393a;border-radius:14px;background:#141c1d}.cu-empty span{font-size:28px;color:#c1f0ad}.cu-empty h3{margin:4px 0 0;font-size:16px}.cu-empty p{margin:0;font-size:12.5px;color:#95a5a4}
  .cu-actions{display:flex;gap:6px;flex-wrap:wrap;margin:2px 0 4px -8px}
  .cu-note{font-size:12px;color:#95a5a4;margin:6px 0}.cu-err{color:#ff9b9b;font-size:12px}
  .cu-row{display:block;padding:0;min-height:0}
  .cu-line{display:flex;align-items:center;gap:13px;padding:13px 16px;min-height:76px;box-sizing:border-box}
  .cu-line>div{flex:1;min-width:0}.cu-line h3{font-size:14px;margin:0 0 3px;overflow:hidden;text-overflow:ellipsis}.cu-line p{margin:0;font-size:12px;color:#95a5a4}
  .cu-detail{padding:4px 16px 18px 67px;border-top:1px solid #2c393a;background:#141c1d}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
  @media(max-width:720px){.cu-detail{padding-left:16px}}
</style>
