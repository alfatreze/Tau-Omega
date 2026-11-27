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
  .ts{position:relative;flex:none;width:40px;height:22px;padding:0;border-radius:999px;border:1px solid #354547;background:#1e292b;cursor:pointer;transition:background .16s ease,border-color .16s ease}
  .ts:hover:not(:disabled){border-color:#4b6264}
  .ts:focus-visible{outline:2px solid #c1f0ad;outline-offset:2px}
  .ts:disabled{opacity:.5;cursor:not-allowed}
  .ts.on{background:#6fa35a;border-color:#84ba6e}
  .ts-knob{position:absolute;top:2px;left:2px;width:16px;height:16px;border-radius:50%;background:#dce6e4;transition:transform .16s ease,background .16s ease}
  .ts.on .ts-knob{transform:translateX(18px);background:#101617}
</style>
