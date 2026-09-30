<script lang="ts" generics="T">
  // A windowed list: only the rows in (and just around) the visible area are in
  // the page, so thousands of albums or tracks stay fast. Every row must be
  // exactly `rowHeight` px tall. The parent is told which range is showing so it
  // can lazily load things for visible rows (cover thumbnails).
  import { createEventDispatcher } from 'svelte';

  export let items: T[] = [];
  export let rowHeight = 58;
  export let overscan = 6;
  export let key: (item: T) => string = (item) => String(item);
  export let label = 'List';
  /** Change this to scroll back to the top (a new search, filter or sort). */
  export let resetKey = '';

  const dispatch = createEventDispatcher<{ range: { start: number; end: number } }>();
  let viewport: HTMLDivElement;
  let height = 400;
  let top = 0;
  let lastReset = resetKey;
  $: if (resetKey !== lastReset) { lastReset = resetKey; top = 0; if (viewport) viewport.scrollTop = 0; }
  $: start = Math.max(0, Math.floor(top / rowHeight) - overscan);
  $: end = Math.min(items.length, Math.ceil((top + height) / rowHeight) + overscan);
  $: visible = items.slice(start, end);
  $: dispatch('range', { start, end });
</script>
<div class="vl" role="list" aria-label={label} bind:this={viewport} bind:clientHeight={height} on:scroll={() => (top = viewport.scrollTop)}>
  <div class="vl-space" style="height:{items.length * rowHeight}px">
    <div class="vl-window" style="transform:translateY({start * rowHeight}px)">
      {#each visible as item, i (key(item))}
        <slot {item} index={start + i} count={items.length} />
      {/each}
    </div>
  </div>
</div>
<style>
  .vl{overflow:auto;flex:1;min-height:0}
  .vl-space{position:relative}
  .vl-window{position:absolute;top:0;left:0;right:0}
</style>
