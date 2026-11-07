<script lang="ts">
  // Meter Lab: Tau Alpha's own lab (pinned drop, real meters) above the preset editor. The editor works from the real meters
  // registry that comes with the lab drop (`/tau-lab/lab-data.json`), never from invented meters.
  import RealMeterLab from './RealMeterLab.svelte';
  import MeterPresets from './MeterPresets.svelte';

  export let mediaRoot = '';
  export let cardLabel = '';
</script>
<section class="page" aria-labelledby="meters-title">
  <header><div><p class="eyebrow">METER LAB</p><h1 id="meters-title">Meter Lab</h1><p class="lede">Tau Alpha’s own meter lab (pinned, real meters) and the presets each meter offers on your Pocket.</p></div></header>

  <RealMeterLab />
  <MeterPresets {mediaRoot} {cardLabel} />
</section>
