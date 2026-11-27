<script lang="ts">
  // App-wide action confirmation toasts (replaces inline banner alerts across Palettes, Artwork, Firmware, Cores, Game Bub).
  import { onDestroy } from 'svelte';
  import { dismissToast, toasts } from './toastStore';
  import type { ToastItem } from './toastStore';

  const timers = new Map<number, ReturnType<typeof setTimeout>>();
  const paused = new Set<number>();

  function schedule(t: ToastItem) {
    if (paused.has(t.id) || timers.has(t.id) || t.durationMs <= 0) return;
    timers.set(
      t.id,
      setTimeout(() => {
        timers.delete(t.id);
        dismissToast(t.id);
      }, t.durationMs),
    );
  }

  function pause(id: number) {
    paused.add(id);
    const timer = timers.get(id);
    if (timer) {
      clearTimeout(timer);
      timers.delete(id);
    }
  }

  function resume(t: ToastItem) {
    paused.delete(t.id);
    schedule(t);
  }

  $: {
    const activeIds = new Set($toasts.map((t) => t.id));
    for (const [id, timer] of timers.entries()) {
      if (!activeIds.has(id)) {
        clearTimeout(timer);
        timers.delete(id);
        paused.delete(id);
      }
    }
    for (const t of $toasts) {
      schedule(t);
    }
  }

  onDestroy(() => {
    for (const timer of timers.values()) clearTimeout(timer);
    timers.clear();
  });
</script>

{#if $toasts.length}
  <div class="app-toasts" aria-live="polite" aria-relevant="additions">
    {#each $toasts as t (t.id)}
      <div
        class="app-toast"
        class:info={t.tone === 'info'}
        class:error={t.tone === 'error'}
        role={t.tone === 'error' ? 'alert' : 'status'}
        on:mouseenter={() => pause(t.id)}
        on:mouseleave={() => resume(t)}
        on:focusin={() => pause(t.id)}
        on:focusout={() => resume(t)}
      >
        <span class="app-toast-ic" aria-hidden="true">
          {#if t.tone === 'error'}!{:else if t.tone === 'info'}i{:else}✓{/if}
        </span>
        <span class="app-toast-msg">{t.message}</span>
        <button
          type="button"
          class="app-toast-close"
          aria-label="Dismiss notification"
          on:click={() => dismissToast(t.id)}
        >
          ✕
        </button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .app-toasts {
    position: fixed;
    right: 20px;
    bottom: 20px;
    z-index: 96;
    display: grid;
    gap: 8px;
    justify-items: end;
    pointer-events: none;
  }
  .app-toast {
    pointer-events: auto;
    max-width: min(460px, calc(100vw - 40px));
    display: flex;
    align-items: center;
    gap: 10px;
    background: #182320;
    border: 1px solid #2f4a37;
    border-radius: 12px;
    padding: 10px 14px;
    color: #c5f7ad;
    font-size: 13px;
    line-height: 1.4;
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.42);
    animation: app-toast-in 0.18s ease-out;
  }
  .app-toast.info {
    background: #141f24;
    border-color: #2f4c58;
    color: #bfe4f2;
  }
  .app-toast.error {
    background: #261616;
    border-color: #6b3535;
    color: #f2b8b5;
  }
  .app-toast-ic {
    flex: none;
    font-weight: 700;
  }
  .app-toast-msg {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .app-toast-close {
    flex: none;
    background: transparent;
    color: inherit;
    opacity: 0.72;
    padding: 2px 6px;
    border-radius: 6px;
    font-size: 12px;
  }
  .app-toast-close:hover {
    opacity: 1;
    background: rgba(255, 255, 255, 0.08);
  }
  @keyframes app-toast-in {
    from {
      transform: translateY(8px);
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .app-toast {
      animation: none;
    }
  }
</style>
