<script lang="ts">
  // Compare cores: two Tau media roots side by side, then a reviewed full-library copy. Moved out of App.svelte unchanged; the state and
  // the handlers stay in App.svelte (like BackupView and PackageView) and arrive as props.
  import type { CapacityCheck, Comparison, Plan } from './types';
  export let leftCore = ''; export let rightCore = ''; export let copyReportPath = '';
  export let comparison: Comparison | null = null; export let compareNotice = '';
  export let coreCopyPlan: Plan | null = null; export let coreCopyCapacity: CapacityCheck | null = null;
  export let size: (bytes: number) => string;
  export let chooseFolder: (which: 'left' | 'right' | 'copyReport') => unknown;
  export let compareCores: () => void; export let reviewCoreCopy: () => void; export let runCoreCopy: () => void;
</script>

<section class="page compare-page"><header><div><p class="eyebrow">CORE TRANSFER</p><h1>Compare cores</h1><p class="lede">Compare two Tau media roots before copying the complete library to the second core.</p></div></header><section class="wizard" aria-labelledby="compare-title"><h2 id="compare-title">Compare two cores</h2><label for="left-core">Source media root</label><div class="picker-row"><input id="left-core" bind:value={leftCore} placeholder="/Volumes/Pocket/Assets/tau/common"/><button class="picker" on:click={() => chooseFolder('left')}>Choose</button></div><label for="right-core">Destination media root</label><div class="picker-row"><input id="right-core" bind:value={rightCore} placeholder="/Volumes/Pocket/Assets/tau-test/common"/><button class="picker" on:click={() => chooseFolder('right')}>Choose</button></div><label for="copy-report">Copy report location on this computer</label><div class="picker-row"><input id="copy-report" bind:value={copyReportPath} placeholder="/Users/me/Documents/tau-core-copy.json"/><button class="picker" on:click={() => chooseFolder('copyReport')}>Choose</button></div><button class="primary" on:click={compareCores}>Compare safely</button></section><p class="notice" role="status">{compareNotice}</p>{#if comparison}<section class="comparison" aria-labelledby="comparison-title"><div class="section-title"><div><p class="eyebrow">RESULT</p><h2 id="comparison-title">{comparison.differences.length} files compared</h2></div><span>Read-only</span></div><div class="comparison-counts"><span><b>{comparison.only_left}</b> only in source</span><span><b>{comparison.only_right}</b> only in destination</span><span><b>{comparison.different}</b> different</span><span><b>{comparison.identical}</b> matching</span></div><ul class="difference-list">{#each comparison.differences as item}<li><span class={`difference ${item.state}`}>{item.state === 'only_left' ? 'Source only' : item.state === 'only_right' ? 'Destination only' : item.state === 'different' ? 'Different' : 'Matching'}</span><span>{item.relative}</span><span>{item.left_bytes === null ? '—' : size(item.left_bytes)} / {item.right_bytes === null ? '—' : size(item.right_bytes)}</span></li>{/each}</ul><button class="primary" on:click={reviewCoreCopy}>Review full-library copy</button>{#if coreCopyPlan}<div class="copy-plan"><p class="eyebrow">READY FOR CONFIRMATION</p><h3>{coreCopyPlan.id}</h3><p>{coreCopyPlan.new_files} new · {coreCopyPlan.updates} updated · {coreCopyPlan.unchanged} unchanged · {size(coreCopyPlan.bytes_to_write)} to write</p>{#if coreCopyCapacity}<p class:capacity-ok={coreCopyCapacity.fits} class:capacity-bad={!coreCopyCapacity.fits}>{coreCopyCapacity.fits ? 'Fits' : 'Does not fit'} on the destination volume — {size(coreCopyCapacity.space.available_bytes)} free.</p>{/if}<button class="danger" on:click={runCoreCopy}>Confirm and copy</button></div>{/if}<p class="safety">The source is never changed. The destination files are verified and its index is rebuilt last. Move remains unavailable until its separate backup and deletion review is ready.</p></section>{/if}</section>

<style>
  .comparison { margin-top: 20px; border: 1px solid #2c393a; border-radius: 15px; padding: 29px 31px; background: #1a2325; }
  .difference-list { padding: 0; margin: 0; border: 1px solid #344244; border-radius: 9px; overflow: auto; max-height: 380px; }
  .difference-list li { min-width: 500px; display: grid; grid-template-columns: 100px 1fr 110px; gap: 12px; align-items: center; padding: 10px 12px; border-bottom: 1px solid #2c393a; font-size: 12px; color: #c7d1d0; }
  .difference-list li:last-child { border-bottom: 0; }
  .difference-list li > span:last-child { color: #8f9e9d; text-align: right; }
  .difference { font-size: 11px; font-weight: 700; }
  .difference.only_left, .difference.only_right { color: #d9c47e; }
  .capacity-ok { color: #b9e9a5; font-size: 12px; margin: 6px 0 0; }
  .capacity-bad { color: #f29b83; font-weight: 600; font-size: 12px; margin: 6px 0 0; }
  .difference.different { color: #f29b83; }
  .difference.identical { color: #b9e9a5; }
  .comparison > .primary { margin-top: 18px; }
  .copy-plan { margin-top: 18px; padding: 18px; background: #101617; border: 1px solid #344244; border-radius: 9px; }
  .copy-plan h3 { margin: 0 0 6px; }
  .copy-plan p:not(.eyebrow) { color: #aab8b7; font-size: 13px; }
  .copy-plan .danger { padding: 11px 14px; background: #c1f0ad; color: #142015; font-weight: 700; }
  @media (max-width: 720px) { .comparison { padding: 22px 18px; } .comparison-counts { grid-template-columns: repeat(2, 1fr); } }
</style>
