<script lang="ts">
  /** A full-size look at one picture of a few, over whatever is open: Left/Right (or the arrows) move between them, Escape or a click outside closes only this. */
  import { createEventDispatcher } from 'svelte';
  import { modal } from './a11y';
  export let items: { src: string; alt: string }[];
  export let index = 0;
  export let label = 'Pictures';
  const dispatch = createEventDispatcher<{ close: void }>();
  $: n = items.length;
  $: item = items[Math.min(index, n - 1)];
  const go = (d: number) => { if (n > 1) index = (index + d + n) % n; };
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') { e.stopPropagation(); dispatch('close'); }
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') { e.preventDefault(); e.stopPropagation(); go(e.key === 'ArrowLeft' ? -1 : 1); }
  }
</script>
<div class="lb-backdrop" role="presentation" on:click={() => dispatch('close')}></div>
<div class="lb" role="dialog" aria-modal="true" aria-label={label} tabindex="-1" use:modal on:keydown={key}
  on:click={(e) => { if (e.target === e.currentTarget) dispatch('close'); }}>
  <div class="lb-top">
    <span class="lb-count" aria-live="polite">{index + 1} of {n}{item?.alt ? ` · ${item.alt}` : ''}</span>
    <button class="quiet" data-autofocus on:click={() => dispatch('close')}>Close</button>
  </div>
  <div class="lb-stage" on:click={(e) => { if (e.target === e.currentTarget) dispatch('close'); }} role="presentation">
    {#if n > 1}<button class="lb-nav" aria-label="Previous picture" on:click={() => go(-1)}>‹</button>{/if}
    {#if item}<img src={item.src} alt={item.alt || `Picture ${index + 1}`} />{/if}
    {#if n > 1}<button class="lb-nav" aria-label="Next picture" on:click={() => go(1)}>›</button>{/if}
  </div>
</div>
<style>
  .lb-backdrop{position:fixed;inset:0;z-index:200;background:rgba(5,9,10,.86)}
  .lb{position:fixed;inset:0;z-index:201;display:flex;flex-direction:column;padding:16px 20px 24px;box-sizing:border-box}
  .lb-top{display:flex;justify-content:space-between;align-items:center;gap:16px;color:#dce6e4;font-size:13px;min-height:40px}
  .lb-count{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .lb-stage{flex:1;min-height:0;display:flex;align-items:center;justify-content:center;gap:14px}
  .lb-stage img{max-width:100%;max-height:100%;min-width:0;object-fit:contain;border-radius:8px;background:#000;box-shadow:0 8px 40px rgba(0,0,0,.6)}
  .lb-nav{flex:none;width:44px;height:44px;border-radius:999px;border:1px solid #3b4a4b;background:#16292a;color:#e8ecec;font-size:26px;line-height:1;cursor:pointer}
  .lb-nav:hover{border-color:#c1f0ad}
  @media(max-width:720px){.lb{padding:10px}.lb-nav{width:38px;height:38px}}
</style>
