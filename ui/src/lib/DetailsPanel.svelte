<script lang="ts">
  // "What will change": the staged list and its effect on the card, opened from the capacity bar
  // (docs/CAPACITY_BAR_SPEC.md). This is where the Pending changes live now: a plain list, one row per
  // staged item, each with its own Remove / Undo / Discard. Sizes are estimates until the real plan is
  // made at Start sync.
  import { tick } from 'svelte';
  import { modal } from './a11y';
  import { size } from './format';
  import type { StagedItem } from './types';


  export let open = false;
  export let items: StagedItem[] = [];
  export let pendingLine = '';
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
  /** A sync or another change is running, so the list cannot be edited. */
  export let locked = false;
  export let canStart = false;
  export let startReason = '';
  export let fits = true;
  export let onClose: () => void = () => {};
  export let onStart: () => void = () => {};
  export let onRemove: (key: string) => void = () => {};
  export let onClear: () => void = () => {};

  let panel: HTMLElement;
  $: coreAfter = coreBytes === null ? null : Math.max(0, coreBytes + addBytes - removeBytes);
  $: adding = items.filter((i) => i.kind === 'add');
  $: removing = items.filter((i) => i.kind === 'remove');
  $: editing = items.filter((i) => i.kind === 'edit');
  $: groups = [
    { title: 'Adding', rows: adding, sign: '+' },
    { title: 'Removing', rows: removing, sign: '−' },
    { title: 'Editing', rows: editing, sign: '✎' },
  ];
  $: nothing = items.length === 0;
  const detail = (i: StagedItem) =>
    [i.artist, i.tracks !== undefined ? `${i.tracks} track${i.tracks === 1 ? '' : 's'}` : '', i.bytes !== undefined ? size(i.bytes) : ''].filter(Boolean).join(' · ');

  /** Removes a row and keeps keyboard focus on the row that took its place (or Close when the list is empty). */
  async function remove(event: MouseEvent, key: string) {
    const buttons = [...panel.querySelectorAll<HTMLElement>('.dp-rm')];
    const index = buttons.indexOf(event.currentTarget as HTMLElement);
    onRemove(key);
    await tick();
    const rest = [...panel.querySelectorAll<HTMLElement>('.dp-rm')];
    (rest[Math.min(index, rest.length - 1)] ?? panel.querySelector<HTMLElement>('.dp-close-btn'))?.focus();
  }
</script>

<svelte:window on:keydown={(e) => { if (open && e.key === 'Escape') { e.preventDefault(); onClose(); } }} />

{#if open}
  <div class="dp-backdrop" role="presentation" on:click={onClose}></div>
  <div class="dp" role="dialog" aria-modal="true" aria-labelledby="dp-title" use:modal bind:this={panel}>
    <header>
      <h2 id="dp-title">What will change</h2>
      <button class="quiet dp-x" aria-label="Close" on:click={onClose}>✕</button>
    </header>

    <div class="dp-body">
      {#if nothing}
        <p class="dp-empty">Nothing is staged yet. Tick albums on the left and press Add, or mark albums on the Pocket for removal, and the effect shows here before anything is written.</p>
      {:else}
        <section>
          <p class="dp-line">{pendingLine}{#if fits}{' · '}<span class="dp-fit">Fits, {size(Math.max(freeAfter, 0))} free afterwards</span>{/if}</p>
          <p>
            {#if addBytes}About <b class="add">{size(addBytes)} will be added</b>{/if}{#if addBytes && removeBytes}, {/if}{#if removeBytes}<b class="rm">{size(removeBytes)} removed</b>{/if}.
            Free space after: <b>{size(Math.max(freeAfter, 0))}</b> (now {size(freeNow)}).
          </p>
          {#if maxTracks}<p>Tracks after: {tracksAfter.toLocaleString()} of {maxTracks.toLocaleString()}.</p>{/if}
          {#if coreLabel && coreBytes !== null && coreAfter !== null}<p class="muted">{coreLabel}: {size(coreBytes)}, then about {size(coreAfter)}. Other cores and other data are unchanged.</p>{/if}
        </section>

        {#if warnings.length}
          <section class="dp-warn">
            <h3>Heads-up</h3>
            {#each warnings as w}<p>{w}</p>{/each}
          </section>
        {/if}

        {#each groups as { title, rows, sign } (title)}
          {#if rows.length}
            <section>
              <h3>{title} ({rows.length})</h3>
              {#if title === 'Removing' && backupNote}<p class="muted">{backupNote}</p>{/if}
              <ul class="dp-list" aria-label={`${title}`}>
                {#each rows as i (i.key)}
                  <li>
                    <span class="dp-sign" aria-hidden="true">{sign}</span>
                    <span class="dp-name"><span class="dp-title" title={i.label}>{i.label}</span>{#if detail(i)}<small>{detail(i)}</small>{/if}</span>
                    <button class="quiet dp-rm" aria-label={i.action} disabled={locked} on:click={(e) => remove(e, i.key)}>{i.verb}</button>
                  </li>
                {/each}
              </ul>
            </section>
          {/if}
        {/each}
      {/if}
    </div>

    <footer>
      <button class="quiet dp-close-btn" on:click={onClose}>Close</button>
      <button class="quiet" disabled={locked || nothing} on:click={onClear}>Clear all</button>
      <button class="primary" disabled={!canStart} title={canStart ? '' : startReason} on:click={onStart}>Start sync</button>
    </footer>
    <p class="muted dp-foot">{#if !nothing}These sizes are estimates. Start sync checks every file exactly before anything is written.{/if}{#if !canStart && startReason}{' '}{startReason}{/if}</p>
  </div>
{/if}

<style>
  .dp-backdrop{position:fixed;inset:0;background:rgba(5,10,10,.55);z-index:90}
  .dp{position:fixed;top:0;right:0;bottom:0;width:min(480px,100vw);z-index:91;background:#161f20;border-left:1px solid #2c393a;padding:18px 18px 12px;display:flex;flex-direction:column;color:#a6b3b2;font-size:13px;animation:dp-in .18s ease-out}
  @keyframes dp-in{from{transform:translateX(24px);opacity:0}}
  @media (prefers-reduced-motion:reduce){.dp{animation:none}}
  header{display:flex;justify-content:space-between;align-items:center;margin-bottom:6px;flex-shrink:0}
  h2{margin:0;font-size:17px;color:#e8ecec}
  h3{margin:0 0 6px;font-size:13px;color:#e8ecec}
  .dp-x{font-size:14px;padding:2px 8px}
  .dp-body{flex:1;min-height:0;overflow:auto;padding-right:4px}
  section{border-top:1px solid #2c393a;padding:10px 0}
  section p{margin:0 0 4px}
  section b{color:#e8ecec;font-weight:600}.add{color:#c1f0ad!important}.rm{color:#e8b59f!important}
  .dp-line{color:#e8ecec}.dp-fit{color:#c1f0ad}
  .dp-list{list-style:none;margin:0;padding:0}
  .dp-list li{display:flex;align-items:center;gap:10px;padding:5px 0;border-bottom:1px solid #1f2b2c}
  .dp-list li:last-child{border-bottom:0}
  .dp-sign{width:14px;text-align:center;color:#8c9c9b;flex-shrink:0}
  .dp-name{flex:1;min-width:0;display:flex;flex-direction:column}
  .dp-title{color:#e8ecec;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .dp-name small{color:#8c9c9b;font-size:11px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .dp-rm{font-size:12px;padding:3px 10px;flex-shrink:0}
  .muted{color:#8c9c9b;font-size:12px}
  .dp-empty{padding:14px 0;color:#a6b3b2}
  .dp-warn h3{color:#f0d59a}
  footer{flex-shrink:0;display:flex;justify-content:flex-end;gap:8px;padding-top:12px;border-top:1px solid #2c393a}
  .dp-foot{margin:8px 0 0;flex-shrink:0}
</style>
