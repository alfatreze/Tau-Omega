<script lang="ts">
  // The processing budget of the meter on show, at a glance: the worst frame's drawing commands against what the meter's cost class allows.
  // Under the allowance the bar fills part of it (green) and says how much is left; over it, the stretch beyond the allowance is red and the
  // text says by how much. The tick is the allowance; the thin white mark is the average frame. All numbers are the lab's own estimate.
  import { budgetView, type Budget } from './meters/costInfo';

  /** null while the lab is measuring (a new meter, or a moment after a change): the bar keeps its place and shows nothing wrong. */
  export let budget: Budget | null = null;
  $: v = budget ? budgetView(budget) : null;
  $: label = budget && v ? `Processing budget over the last second: worst frame ${budget.worst} drawing commands, ${budget.allowed} allowed. ${v.text}.` : 'Processing budget: measuring.';
</script>

<div class="bb" data-testid="rml-budget" data-kind={v?.kind ?? 'wait'} role="img" aria-label={label}>
  <div class="bb-head"><span class="bb-title">Processing budget <small>last second</small></span><span class="bb-verdict" data-testid="rml-budget-text">{v ? v.text : 'Measuring…'}</span></div>
  <div class="bb-track" aria-hidden="true">
    {#if v}
      <i class="bb-zone" style="width:{v.allowedAt}%"></i>
      <i class="bb-fill" style="width:{v.withinTo}%"></i>
      <i class="bb-over" style="left:{v.overFrom ?? v.allowedAt}%;width:{v.overFrom !== null && v.overTo !== null ? v.overTo - v.overFrom : 0}%"></i>
      <i class="bb-mean" style="left:{v.meanAt}%"></i>
      <i class="bb-allowed" style="left:{v.allowedAt}%"></i>
    {/if}
  </div>
  <div class="bb-nums" aria-hidden="true">
    <span>average <b>{budget ? budget.mean : '–'}</b></span><span>worst <b>{budget ? budget.worst : '–'}</b></span><span>allowed <b>{budget ? budget.allowed : '–'}</b></span>
  </div>
</div>

<style>
  .bb{display:grid;gap:6px;border:1px solid #2c393a;border-radius:10px;background:#161f20;padding:9px 12px;font-size:12.5px;color:#aab8b7}
  .bb-head{display:flex;justify-content:space-between;align-items:baseline;gap:10px}
  .bb-title{color:#c7d1d0;font-weight:600}.bb-title small{color:#7d8c8b;font-weight:400;margin-left:6px}
  .bb-verdict{font-variant-numeric:tabular-nums;font-weight:650;white-space:nowrap}
  .bb[data-kind=under] .bb-verdict,.bb[data-kind=at] .bb-verdict{color:#8fcf9f}.bb[data-kind=over] .bb-verdict{color:#ffb4a8}
  .bb-track{position:relative;height:12px;border-radius:6px;background:#0f1516;overflow:visible}
  .bb-track i{position:absolute;top:0;bottom:0;display:block;font-style:normal}
  /* Everything that moves uses the same short glide, so the fill, the red stretch and the marks stay together as they change. */
  .bb-fill,.bb-over,.bb-mean,.bb-zone,.bb-allowed{transition:width .07s linear,left .07s linear}
  .bb[data-kind=wait]{opacity:.7}
  .bb-zone{left:0;border-radius:6px 0 0 6px;background:#1d2a2b}
  .bb-fill{left:0;border-radius:6px 0 0 6px;background:#5f9e6c}
  .bb-over{background:#d9604f;border-radius:0 6px 6px 0}
  .bb-mean{width:2px;margin-left:-1px;background:#f3f6f6;top:2px!important;bottom:2px!important;border-radius:1px}
  .bb-allowed{width:2px;margin-left:-1px;top:-3px!important;bottom:-3px!important;background:#e8ecec;border-radius:1px}
  .bb-nums{display:flex;justify-content:space-between;gap:8px;font-size:11.5px;color:#8f9e9d;font-variant-numeric:tabular-nums}
  .bb-nums b{color:#e8ecec;font-weight:650}
  .bb-nums span:last-child{text-align:right}
</style>
