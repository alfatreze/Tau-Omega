<script lang="ts">
  // Meter Lab: one workspace. Pick a meter, see it drawn by Tau Alpha's real meter code (a pinned lab drop), and edit the presets it offers
  // on your Pocket; every change shows in the preview as you make it. The meter list and the controls are Omega's own; the lab only draws.
  import MeterPresets from './MeterPresets.svelte';

  export let mediaRoot = '';
  export let cardLabel = '';
</script>
<section class="page" aria-labelledby="meters-title">
  <header><div><p class="eyebrow">METER LAB</p><h1 id="meters-title">Meter Lab</h1><p class="lede">See each meter drawn by the real Pocket code and shape the presets it offers on your Pocket.</p></div></header>

  <MeterPresets {mediaRoot} {cardLabel} />
</section>
<style>
  /* A workspace (meter list, preview, controls side by side), so it may use more of the window than a text page. */
  .page{max-width:1480px;padding-right:clamp(64px,7vw,96px)}   /* keeps the content clear of the floating help button */
</style>
