<script lang="ts">
  // A finished sync is good news, not a decision: a toast with the result in one line and the two things
  // worth doing next (eject, see the details), instead of a dialog that has to be dismissed. It stays while
  // there is something to read (a warning, an eject in progress or that failed) and while the pointer or
  // keyboard focus is on it; otherwise it goes away by itself.
  import { onDestroy } from 'svelte';

  export let summary = '';
  /** The first thing worth knowing about from the plan (for example an unusable cover), or empty. */
  export let warning = '';
  export let ejectMsg = '';
  export let ejectOk = false;
  export let ejecting = false;
  /** Whether the files were read back from the card itself (shown as a tooltip, not as text). */
  export let verifiedOnDevice = true;
  export let onEject: () => void = () => {};
  export let onDetails: () => void = () => {};
  export let onDismiss: () => void = () => {};

  let timer: ReturnType<typeof setTimeout> | undefined;
  let hover = false;
  let focused = false;
  const stop = () => { if (timer) { clearTimeout(timer); timer = undefined; } };
  $: {
    stop();
    const keep = !!warning || ejecting || hover || focused || (!!ejectMsg && !ejectOk);
    if (!keep) timer = setTimeout(onDismiss, ejectOk ? 6000 : 15000);
  }
  onDestroy(stop);
</script>

<div
  class="toast" role="status" aria-live="polite" aria-label="Sync complete"
  title={verifiedOnDevice ? 'Every file was read back from the card itself and matched.' : 'Every file was checked after writing, but on this computer that check may have been answered from memory.'}
  on:mouseenter={() => (hover = true)} on:mouseleave={() => (hover = false)}
  on:focusin={() => (focused = true)} on:focusout={() => (focused = false)}
>
  <span class="tick" aria-hidden="true">✓</span>
  <div class="body">
    <span><b>Sync complete</b> · {summary}</span>
    {#if warning}<small class="warn">{warning}</small>{/if}
    {#if ejectMsg}<small class={ejectOk ? 'ok' : 'warn'}>{ejectMsg}</small>{/if}
  </div>
  <div class="actions">
    {#if !ejectOk}<button class="quiet" disabled={ejecting} title="Unmount the card so it is safe to unplug" on:click={onEject}>{ejecting ? 'Ejecting…' : 'Eject safely'}</button>{/if}
    <button class="quiet" on:click={onDetails}>View details</button>
    <button class="quiet" aria-label="Dismiss" on:click={onDismiss}>✕</button>
  </div>
</div>

<style>
  .toast{position:fixed;right:20px;bottom:20px;z-index:95;max-width:min(520px,calc(100vw - 40px));display:flex;align-items:center;gap:12px;background:#182320;border:1px solid #2f4a37;border-radius:12px;padding:10px 12px;color:#a6b3b2;font-size:13px;box-shadow:0 8px 24px rgba(0,0,0,.35);animation:toast-in .18s ease-out}
  @keyframes toast-in{from{transform:translateY(8px);opacity:0}}
  @media (prefers-reduced-motion:reduce){.toast{animation:none}}
  .tick{color:#c1f0ad;font-weight:700}
  .body{display:flex;flex-direction:column;gap:2px;min-width:0}
  .body b{color:#e8ecec;font-weight:600}
  .body small{font-size:12px;overflow-wrap:anywhere}
  .warn{color:#f0d59a}.ok{color:#c1f0ad}
  .actions{display:flex;align-items:center;gap:4px;margin-left:auto;flex-shrink:0}
  .actions button{font-size:12px;padding:4px 10px}
</style>
