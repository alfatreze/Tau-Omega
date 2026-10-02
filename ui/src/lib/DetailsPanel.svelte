<script lang="ts">
  // "What will change": a read-only look at the staged list and its effect on the card, opened from the
  // capacity bar (docs/CAPACITY_BAR_SPEC.md). It writes nothing; Start sync here is the same action as the
  // one in the Pending changes area. Sizes are estimates until the real plan is made at Start sync.
  import { modal } from './a11y';
  import { size, plural } from './format';

  type Line = { title: string; artist?: string; tracks?: number; bytes?: number; note?: string };
  export let open = false;
  export let adds: Line[] = [];
  export let removes: Line[] = [];
  export let edits: Line[] = [];
  export let addBytes = 0;
  export let removeBytes = 0;
  export let freeNow = 0;
  export let freeAfter = 0;
  export let tracksAfter = 0;
  export let maxTracks = 0;
  /** The core the staged change belongs to, with its media size on the card now (null if not measured). */
  export let coreLabel = '';
  export let coreBytes: number | null = null;
  export let warnings: string[] = [];
  export let backupNote = '';
  export let canStart = false;
  export let startReason = '';
  export let onClose: () => void = () => {};
  export let onStart: () => void = () => {};

  const SHOWN = 8;
  let showAllAdds = false;
  let showAllRemoves = false;
  $: if (!open) { showAllAdds = false; showAllRemoves = false; }
  $: coreAfter = coreBytes === null ? null : Math.max(0, coreBytes + addBytes - removeBytes);
  $: nothing = !adds.length && !removes.length && !edits.length;
  const line = (l: Line) => `${l.tracks !== undefined ? `${plural(l.tracks, 'track')}, ` : ''}${l.bytes !== undefined ? size(l.bytes) : ''}`.replace(/, $/, '');
</script>

<svelte:window on:keydown={(e) => { if (open && e.key === 'Escape') { e.preventDefault(); onClose(); } }} />

{#if open}
  <div class="dp-backdrop" role="presentation" on:click={onClose}></div>
  <div class="dp" role="dialog" aria-modal="true" aria-labelledby="dp-title" use:modal>
    <header>
      <h2 id="dp-title">What will change</h2>
      <button class="quiet dp-x" aria-label="Close" on:click={onClose}>✕</button>
    </header>

    {#if nothing}
      <p class="dp-empty">Nothing is staged yet. Tick albums on the left and press Add, or mark albums on the Pocket for removal, and the effect shows here before anything is written.</p>
    {:else}
      <section>
        <p>
          {#if addBytes}About <b class="add">{size(addBytes)} will be added</b>{/if}{#if addBytes && removeBytes}, {/if}{#if removeBytes}<b class="rm">{size(removeBytes)} removed</b>{/if}.
        </p>
        <p>Free space after: <b>{size(Math.max(freeAfter, 0))}</b> (now {size(freeNow)}).</p>
        {#if maxTracks}<p>Tracks after: {tracksAfter.toLocaleString()} of {maxTracks.toLocaleString()}.</p>{/if}
      </section>

      {#if coreLabel && coreBytes !== null && coreAfter !== null}
        <section>
          <h3>By core</h3>
          <p>{coreLabel}: {size(coreBytes)}, then about {size(coreAfter)}</p>
          <p class="muted">Other cores and other data are unchanged.</p>
        </section>
      {/if}

      {#if adds.length}
        <section>
          <h3>Adding ({plural(adds.length, 'album')})</h3>
          <ul>{#each showAllAdds ? adds : adds.slice(0, SHOWN) as a}<li><span>{a.title}</span><small>{line(a)}</small></li>{/each}</ul>
          {#if adds.length > SHOWN && !showAllAdds}<button class="quiet" on:click={() => (showAllAdds = true)}>Show all {adds.length}</button>{/if}
        </section>
      {/if}

      {#if removes.length}
        <section>
          <h3>Removing ({plural(removes.length, 'album')})</h3>
          <ul>{#each showAllRemoves ? removes : removes.slice(0, SHOWN) as r}<li><span>{r.title}</span><small>{line(r)}</small></li>{/each}</ul>
          {#if removes.length > SHOWN && !showAllRemoves}<button class="quiet" on:click={() => (showAllRemoves = true)}>Show all {removes.length}</button>{/if}
          {#if backupNote}<p class="muted">{backupNote}</p>{/if}
        </section>
      {/if}

      {#if edits.length}
        <section>
          <h3>Editing ({plural(edits.length, 'item')})</h3>
          <ul>{#each edits as e}<li><span>{e.title}</span><small>{e.note ?? ''}</small></li>{/each}</ul>
        </section>
      {/if}

      {#if warnings.length}
        <section class="dp-warn">
          <h3>Heads-up</h3>
          {#each warnings as w}<p>{w}</p>{/each}
        </section>
      {/if}
    {/if}

    <footer>
      <button class="quiet" on:click={onClose}>Close</button>
      <button class="primary" disabled={!canStart} title={canStart ? '' : startReason} on:click={onStart}>Start sync</button>
    </footer>
    {#if !nothing}<p class="muted dp-foot">These sizes are estimates. Press Start sync to check every file exactly before anything is written.</p>{/if}
    {#if !canStart && startReason}<p class="muted dp-foot" role="status">{startReason}</p>{/if}
  </div>
{/if}

<style>
  .dp-backdrop{position:fixed;inset:0;background:rgba(5,10,10,.55);z-index:90}
  .dp{position:fixed;top:0;right:0;bottom:0;width:min(420px,100vw);z-index:91;background:#161f20;border-left:1px solid #2c393a;padding:18px 18px 14px;display:flex;flex-direction:column;gap:2px;overflow:auto;color:#a6b3b2;font-size:13px;animation:dp-in .18s ease-out}
  @keyframes dp-in{from{transform:translateX(24px);opacity:0}}
  @media (prefers-reduced-motion:reduce){.dp{animation:none}}
  header{display:flex;justify-content:space-between;align-items:center;margin-bottom:6px}
  h2{margin:0;font-size:17px;color:#e8ecec}
  h3{margin:0 0 4px;font-size:13px;color:#e8ecec}
  .dp-x{font-size:14px;padding:2px 8px}
  section{border-top:1px solid #2c393a;padding:10px 0}
  section p{margin:0 0 4px}
  section b{color:#e8ecec;font-weight:600}.add{color:#c1f0ad!important}.rm{color:#e8b59f!important}
  ul{list-style:none;margin:0;padding:0}
  li{display:flex;justify-content:space-between;gap:12px;padding:3px 0}
  li span{color:#e8ecec;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  li small{color:#8c9c9b;white-space:nowrap}
  .muted{color:#8c9c9b;font-size:12px}
  .dp-empty{padding:14px 0;color:#a6b3b2}
  .dp-warn h3{color:#f0d59a}
  footer{margin-top:auto;display:flex;justify-content:flex-end;gap:8px;padding-top:12px;border-top:1px solid #2c393a}
  .dp-foot{margin:8px 0 0}
</style>
