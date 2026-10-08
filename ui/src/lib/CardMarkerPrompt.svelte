<script lang="ts">
  import { cardMarkerStatus, setCardMarker } from './tau-api';
  import { errorMessage } from './backend';
  import type { MarkerStatus } from './types';
  /** The card folder currently open. */
  export let card = '';

  let status: MarkerStatus | null = null; let busy = false; let notice = '';
  let askedFor = '';
  $: if (card && card !== askedFor) { askedFor = card; void load(); }
  async function load() { try { status = await cardMarkerStatus(card); } catch { status = null; } }
  async function answer(setting: 'on' | 'off') {
    busy = true; notice = '';
    try { status = await setCardMarker(setting, card); }
    catch (error) { notice = `Could not save that: ${errorMessage(error)}`; }
    finally { busy = false; }
  }
</script>
{#if status && status.is_card && status.setting === 'ask'}
  <div class="marker-bar" role="region" aria-label="Spotlight on this card">
    <span>macOS Spotlight scans Pocket cards in the background, which slows writes and can make Eject or Empty Trash report “in use”. Tau Omega can add an empty file named <code>.metadata_never_index</code> at the card’s root to stop that. It changes nothing else, the Pocket ignores it, and you can turn it off any time in Settings. Add it now, and to cards you write to later?</span>
    <span class="marker-actions"><button class="primary" disabled={busy} on:click={() => answer('on')}>Yes, keep Spotlight off cards</button><button class="quiet" disabled={busy} on:click={() => answer('off')}>No thanks</button></span>
    {#if notice}<span class="marker-error" role="alert">{notice}</span>{/if}
  </div>
{/if}
<style>
  .marker-bar{display:flex;flex-wrap:wrap;gap:10px 16px;align-items:center;justify-content:space-between;margin:12px 16px 14px;padding:12px 56px 12px 16px;border:1px solid #3b5a58;border-radius:10px;background:#16292a;color:#dce6e4;font-size:13px;line-height:1.5}
  .marker-actions{display:flex;gap:8px}.marker-error{flex-basis:100%;color:#ff9b9b}
  code{font-size:12px}
</style>
