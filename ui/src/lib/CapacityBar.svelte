<script lang="ts">
  // The card's space and the effect of what is staged, in one place (docs/CAPACITY_BAR_SPEC.md):
  // a stacked bar of each Tau core's media, other data, what is being added or removed, and what
  // stays free, with a text legend (nothing depends on colour) and a Details button.
  import type { CardBreakdown } from './types';
  import { size } from './format';

  export let hasSpace = false;
  /** The card is still being read. */
  export let busy = false;
  export let total = 0;
  export let used = 0;
  export let addBytes = 0;
  export let removeBytes = 0;
  export let freeNow = 0;
  export let freeAfter = 0;
  export let margin = 0;
  export let overCapacity = false;
  export let breakdown: CardBreakdown | null = null;
  /** The per-core measurement is running in the background. */
  export let measuring = false;
  /** The platform folder the staged change belongs to (the core the Library is working with). */
  export let activePlatform = '';
  export let limitText = '';
  export let limitTitle = '';
  export let overLimit = false;
  export let nearLimit = false;
  export let onDetails: () => void = () => {};
  /** Start sync lives here, at the top, next to the numbers it affects. It only opens the review; nothing is written until the review is confirmed. */
  export let canStart = false;
  export let startReason = '';
  /** One line naming what is staged, read out with the button so a keyboard or screen-reader user hears what Start sync will review. */
  export let pendingSummary = '';
  export let onStart: () => void = () => {};
  /** Clear all lives next to Start sync, since the staged list itself is in the Details panel. */
  export let hasPending = false;
  export let clearDisabled = false;
  export let onClear: () => void = () => {};
  /** Short status lines that used to sit beside Start sync (slow link, will not fit, over the limit, disconnected). */
  export let notes: { text: string; bad: boolean }[] = [];

  let startButton: HTMLButtonElement;
  let detailsButton: HTMLButtonElement;
  /** After staging something, keyboard focus goes here: Start sync when it is allowed, else Details. */
  export function focusPrimary() {
    (startButton && !startButton.disabled ? startButton : detailsButton)?.focus();
  }

  const RAMP = ['#5b7f8f', '#6f9f9a', '#7f8fbf', '#9a86b8'];
  const pct = (n: number) => `${Math.max(0, Math.min(100, total ? (n / total) * 100 : 0))}%`;

  $: staged = addBytes > 0 || removeBytes > 0;
  $: segs = (breakdown?.segments ?? []).map((s, i) => ({
    key: s.platform,
    label: s.shortnames.length ? s.shortnames.join(' and ') : s.platform,
    shared: s.core_ids.length > 1,
    bytes: s.bytes_on_disk,
    color: RAMP[i % RAMP.length],
    active: s.platform === activePlatform,
  }));
  $: activeFound = segs.some((s) => s.active);
  // A removal is drawn inside the segment it comes from; with no such segment it is drawn out of "other data".
  $: removeInside = (s: { active: boolean; bytes: number }) => (s.active ? Math.min(removeBytes, s.bytes) : 0);
  $: removeOutside = activeFound ? 0 : Math.min(removeBytes, breakdown?.other_bytes ?? 0);
  $: otherKept = Math.max(0, (breakdown?.other_bytes ?? 0) - removeOutside);
  $: label = (() => {
    const parts = [`${size(used)} used`];
    if (breakdown) parts.push(`${segs.map((s) => `${s.label} ${size(s.bytes)}`).concat(`other data ${size(breakdown.other_bytes)}`).join(', ')}`);
    if (addBytes) parts.push(`about ${size(addBytes)} will be added`);
    if (removeBytes) parts.push(`about ${size(removeBytes)} will be removed`);
    parts.push(staged ? `${size(Math.max(freeAfter, 0))} free after sync` : `${size(Math.max(freeNow, 0))} free`);
    return parts.join('. ');
  })();
</script>

<div class="wb-cap" role="group" aria-label="Storage on the Pocket">
  {#if hasSpace}
    <div class="wb-cap-text">
      <span><b>{size(used)}</b> on Pocket</span>
      {#if limitText}<span class="wb-limit" class:bad={overLimit} class:warn={nearLimit} title={limitTitle}>{limitText}</span>{/if}
      {#if overCapacity}
        <span class="wb-cap-free bad">{size(Math.max(0, margin - freeAfter))} too much for this card</span>
      {:else if staged}
        <span class="wb-cap-free">{size(freeNow)} free, <b>{size(Math.max(freeAfter, 0))} after sync</b></span>
      {:else}
        <span class="wb-cap-free">{size(freeNow)} free of {size(total)}</span>
      {/if}
    </div>
    <div class="wb-bar" role="img" aria-label={label}>
      {#if breakdown}
        {#each segs as s (s.key)}
          <i class="seg" style="width:{pct(s.bytes - removeInside(s))};background:{s.color}"></i>{#if removeInside(s)}<i class="seg k-rm" style="width:{pct(removeInside(s))}"></i>{/if}
        {/each}
        <i class="seg k-other" style="width:{pct(otherKept)}"></i>{#if removeOutside}<i class="seg k-rm" style="width:{pct(removeOutside)}"></i>{/if}
      {:else}
        <i class="seg k-used" class:measuring style="width:{pct(Math.max(0, used - removeBytes))}"></i>{#if removeBytes}<i class="seg k-rm" style="width:{pct(removeBytes)}"></i>{/if}
      {/if}
      {#if addBytes}<i class="seg k-add" class:over={overCapacity} style="width:{pct(addBytes)}"></i>{/if}
    </div>
    <div class="wb-legend">
      {#if breakdown}
        {#each segs as s (s.key)}
          <span><i class="sw" style="background:{s.color}"></i>{s.label}{s.shared ? ' (shared media)' : ''} {size(s.bytes)}</span>
        {/each}
        <span title="Other cores, saves, screenshots and system files"><i class="sw k-other"></i>Other data {size(breakdown.other_bytes)}</span>
      {:else}
        <span><i class="sw k-used"></i>Used {size(used)}</span>
        {#if measuring}<span class="muted" role="status">Measuring what is on the card…</span>{/if}
      {/if}
      {#if removeBytes}<span><i class="sw k-rm"></i>Removing about {size(removeBytes)}</span>{/if}
      {#if addBytes}<span><i class="sw k-add" class:over={overCapacity}></i>Adding about {size(addBytes)}</span>{/if}
      <span class="wb-actions">
        <button class="quiet" aria-haspopup="dialog" bind:this={detailsButton} on:click={onDetails}>Details</button>
        <button class="quiet" disabled={clearDisabled || !hasPending} on:click={onClear}>Clear all</button>
        <button class="primary" disabled={!canStart} title={canStart ? '' : startReason} aria-describedby="cap-start-desc" bind:this={startButton} on:click={onStart}>Start sync</button>
        <span id="cap-start-desc" class="sr-only">{canStart ? `Opens the review of: ${pendingSummary}. Nothing is written until you confirm.` : startReason}</span>
      </span>
    </div>
    {#if notes.length}
      <div class="wb-notes">{#each notes as n}<span class:bad={n.bad} role={n.bad ? 'alert' : 'note'}>{n.text}</span>{/each}</div>
    {/if}
  {:else}
    <div class="wb-cap-text"><span>{busy ? 'Reading the card…' : 'Storage information is not available for this card.'}</span></div>
  {/if}
</div>

<style>
  .wb-cap{flex-shrink:0;background:#161f20;border:1px solid #2c393a;border-radius:12px;padding:14px 16px}
  .wb-cap-text{display:flex;flex-wrap:wrap;gap:6px 18px;font-size:13px;color:#a6b3b2;margin-bottom:10px}
  .wb-cap-text b{color:#e8ecec}
  .wb-cap-free{margin-left:auto}.wb-cap-free.bad{color:#ff9d8a;font-weight:700}
  .wb-limit.bad{color:#ff9d8a;font-weight:700}.wb-limit.warn{color:#f0d59a}
  .wb-bar{display:flex;height:14px;border-radius:99px;background:#232e30;overflow:hidden}
  .seg{display:block;height:100%;min-width:0;transition:width .25s;border-right:1px solid #161f20}
  .seg[style*="width:0%"]{display:none}
  .k-used{background:#5b7f8f}.k-other{background:#3d4b4d}.k-add{background:#c1f0ad}.k-add.over{background:#ff9d8a}
  .k-rm{background:repeating-linear-gradient(45deg,#7a4a3d,#7a4a3d 4px,#5b3a30 4px,#5b3a30 8px)}
  .measuring{animation:cap-pulse 1.4s ease-in-out infinite}
  @keyframes cap-pulse{50%{opacity:.55}}
  @media (prefers-reduced-motion:reduce){.measuring{animation:none}.seg{transition:none}}
  .wb-legend{display:flex;flex-wrap:wrap;align-items:center;gap:6px 16px;margin-top:10px;font-size:12px;color:#a6b3b2}
  .wb-legend .sw{display:inline-block;width:10px;height:10px;border-radius:2px;margin-right:6px;vertical-align:-1px}
  .wb-legend .muted{color:#8c9c9b}
  .wb-notes{display:flex;flex-wrap:wrap;gap:4px 16px;margin-top:8px;font-size:12px;color:#f0d59a}
  .wb-notes .bad{color:#ff9d8a;font-weight:650}
  .wb-actions{margin-left:auto;display:flex;align-items:center;gap:8px}
  .wb-actions button{font-size:12px;padding:5px 14px}
  .sr-only{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
</style>
