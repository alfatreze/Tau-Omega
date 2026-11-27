<script lang="ts">
  /** Accessible pill toggle switch (`role="switch"`), matching Tau Omega's dark theme. */
  export let checked = false;
  export let label = '';
  export let disabled = false;
  export let title = '';
</script>

<button
  type="button"
  class="ts"
  class:on={checked}
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  {title}
  on:click
>
  <span class="ts-knob" aria-hidden="true"></span>
</button>

<style>
  .ts{position:relative;flex:none;width:40px;height:22px;padding:0;border-radius:999px;border:1px solid var(--border-strong);background:var(--bg-elevated);cursor:pointer;transition:background .16s ease,border-color .16s ease}
  .ts:hover:not(:disabled){border-color:var(--border-strong)}
  .ts:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
  .ts:disabled{opacity:.5;cursor:not-allowed}
  .ts.on{background:var(--accent-solid);border-color:var(--accent-soft-border)}
  .ts-knob{position:absolute;top:2px;left:2px;width:16px;height:16px;border-radius:50%;background:var(--text-secondary);transition:transform .16s ease,background .16s ease}
  .ts.on .ts-knob{transform:translateX(18px);background:var(--accent-on-solid)}
</style>
