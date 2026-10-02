<script lang="ts">
  // A small circular progress for one album while a sync runs, like the copy progress in Finder: an empty
  // ring while queued, a ring that fills clockwise while it is copying, a check when it is done.
  export let state: 'queued' | 'syncing' | 'done' = 'queued';
  /** 0 to 100, used while syncing. */
  export let pct = 0;
  /** The album's name, for the accessible label. */
  export let name = '';

  const R = 7;
  const C = 2 * Math.PI * R;
  $: clamped = Math.max(0, Math.min(100, pct));
  $: label = state === 'done' ? `Copied ${name}` : state === 'syncing' ? `Copying ${name}, ${Math.round(clamped)} percent` : `Waiting to copy ${name}`;
</script>

{#if state === 'syncing'}
  <span class="ring syncing" role="progressbar" aria-label={label} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(clamped)}>
    <svg viewBox="0 0 18 18" width="20" height="20" aria-hidden="true">
      <circle class="track" cx="9" cy="9" r={R} />
      <circle class="arc" cx="9" cy="9" r={R} stroke-dasharray={`${(C * clamped) / 100} ${C}`} transform="rotate(-90 9 9)" />
    </svg>
  </span>
{:else}
  <span class="ring {state}" role="img" aria-label={label}>
    <svg viewBox="0 0 18 18" width="20" height="20" aria-hidden="true">
      {#if state === 'done'}
        <circle class="fill" cx="9" cy="9" r={R + 1} />
        <path class="tick" d="M5.6 9.3l2.3 2.3 4.5-4.8" />
      {:else}
        <circle class="track" cx="9" cy="9" r={R} />
      {/if}
    </svg>
  </span>
{/if}

<style>
  .ring{display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;width:20px;height:20px}
  svg{display:block}
  .track{fill:none;stroke:#3a4a4c;stroke-width:2}
  .queued .track{stroke-dasharray:2.4 2.6}
  .arc{fill:none;stroke:#c1f0ad;stroke-width:2.4;stroke-linecap:round;transition:stroke-dasharray .25s linear}
  .fill{fill:#234029;stroke:#c1f0ad;stroke-width:1}
  .tick{fill:none;stroke:#c5f7ad;stroke-width:1.8;stroke-linecap:round;stroke-linejoin:round}
  @media (prefers-reduced-motion:reduce){.arc{transition:none}}
</style>
