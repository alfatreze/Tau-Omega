<script lang="ts">
  import type { ConnectionKind } from './types';
  // Three generic glyphs, not Analogue's real logo (no logo asset available to
  // embed). `kind` comes from the backend's `detect_connection`: the Pocket
  // itself (direct USB) gets the handheld, a card reader gets an SD card, and
  // anything unchecked or unknown falls back to the handheld, never a guess
  // at "probably a reader".
  export let kind: ConnectionKind | null | undefined = null;
</script>
{#if kind === 'card_reader'}
  <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <path d="M7 2h7l4 4v14a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1Z" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>
    <path d="M14 2v4h4" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/>
    <path d="M9 21v-5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v5" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/>
  </svg>
{:else}
  <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><rect x="4" y="2" width="16" height="20" rx="3" stroke="currentColor" stroke-width="1.6"/><rect x="7" y="5" width="10" height="7" rx="1" stroke="currentColor" stroke-width="1.4"/><circle cx="9" cy="16.5" r="1.1" fill="currentColor"/><circle cx="15" cy="16.5" r="1.1" fill="currentColor"/><circle cx="12" cy="19.2" r="1.1" fill="currentColor"/></svg>
{/if}
