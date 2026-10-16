<script lang="ts">
  // Advanced sync option: Skip silence. Whether silence at the start and end of tracks is measured at sync (and the limits that decide what counts), and
  // the way into adjusting the tracks already on the card. Saved straight to the preferences every sync reads. docs/SKIP_SILENCE.md.
  import { onMount } from 'svelte';
  import SkipSilenceSheet from './SkipSilenceSheet.svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { getPrefs, setPrefs } from './tau-api';
  import type { PrefsView, TrimSettings } from './types';
  export let card = ''; export let mediaRoot = ''; export let close: () => void;
  let prefs: PrefsView | null = null; let notice = ''; let adjusting = false;
  onMount(async () => { try { prefs = await getPrefs(); } catch (e) { notice = `Could not read preferences: ${errorMessage(e)}`; } });
  async function changeTrim(part: Partial<TrimSettings>, message = 'Saved.') {
    if (!prefs) return;
    try { prefs = await setPrefs({ ...prefs, trim: { ...prefs.trim, ...part } }); notice = message; }
    catch (error) { notice = `Could not save: ${errorMessage(error)}`; }
  }
  const DB_CHOICES = [-80, -75, -70, -65, -60, -55, -50, -45, -40];
  const LEAD_CHOICES = [0, 100, 200, 500, 1000];
  const TAIL_CHOICES = [0, 250, 500, 1000, 2000];
  const MIN_CHOICES = [1000, 2000, 3000, 5000, 10000];
  const secs = (ms: number) => (ms < 1000 ? `${ms} ms` : `${ms / 1000} s`);
</script>
<svelte:window on:keydown={(e) => e.key === 'Escape' && !adjusting && close()} />
<div class="so-backdrop" role="presentation" on:click={close}></div>
<div class="so" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="so-title" use:modal>
  <header class="so-head"><div><p class="eyebrow">ADVANCED SYNC OPTION</p><h2 id="so-title">Skip silence</h2></div><button class="quiet" data-autofocus on:click={close}>Done</button></header>
  {#if prefs}
    {@const t = prefs.trim}
          <div class="ss-row"><div><b>Skip silence</b><small>Measured when you sync and saved as a small <code>tau-skip.bin</code> file in each album folder; your music files are never changed. The player skips the silence when its own Skip silence switch is on (a player version that supports it, MP3 first).</small></div>
        <div><button class="quiet" on:click={() => changeTrim({ enabled: !t.enabled }, t.enabled ? 'Silence will not be measured.' : 'Silence will be measured at the next sync.')}>{t.enabled ? 'Turn off' : 'Turn on'}</button></div></div>
      {#if t.enabled}
        <div class="ss-row"><div><b>Counts as silence below</b><small>-60 dB is quiet enough to be inaudible on headphones in a normal room. Lower catches only digital silence; higher also trims soft room noise and fade tails.</small></div>
          <select aria-label="Silence level" value={t.threshold_db10} on:change={(e) => changeTrim({ threshold_db10: Number(e.currentTarget.value) })}>{#each DB_CHOICES as d}<option value={d * 10}>{d} dB{d === -60 ? ' (default)' : ''}</option>{/each}</select></div>
        <div class="ss-row"><div><b>Silence left before the first sound</b><small>A natural attack is not silence, so the track starts a little before it.</small></div>
          <select aria-label="Silence kept at the start" value={t.lead_margin_ms} on:change={(e) => changeTrim({ lead_margin_ms: Number(e.currentTarget.value) })}>{#each LEAD_CHOICES as k}<option value={k}>{secs(k)}{k === 200 ? ' (default)' : ''}</option>{/each}</select></div>
        <div class="ss-row"><div><b>Silence left after the last sound</b><small>A natural decay is not silence, so the track ends a little after it.</small></div>
          <select aria-label="Silence kept at the end" value={t.tail_margin_ms} on:change={(e) => changeTrim({ tail_margin_ms: Number(e.currentTarget.value) })}>{#each TAIL_CHOICES as k}<option value={k}>{secs(k)}{k === 500 ? ' (default)' : ''}</option>{/each}</select></div>
        <div class="ss-row"><div><b>Only skip silences of at least</b><small>Shorter ones are left alone, so normal gaps stay. Silence in the middle of a track is never skipped.</small></div>
          <select aria-label="Smallest silence" value={t.min_ms} on:change={(e) => changeTrim({ min_ms: Number(e.currentTarget.value) })}>{#each MIN_CHOICES as k}<option value={k}>{secs(k)}{k === 2000 ? ' (default)' : ''}</option>{/each}</select></div>
      {/if}
    <div class="ss-row"><div><b>Adjust tracks already on the card</b><small>See what is skipped on each track, change it by hand, or mark a track as never skipped. Works on what the card holds now, without syncing.{mediaRoot ? '' : ' Open a card with a music player core first.'}</small></div>
      <div><button class="quiet" disabled={!mediaRoot} on:click={() => (adjusting = true)}>Adjust tracks…</button></div></div>
    {#if adjusting}<SkipSilenceSheet {card} {mediaRoot} close={() => (adjusting = false)} />{/if}
    <p class="ss-notice" role="status">{notice}</p>
  {:else}<p class="ss-notice">{notice || 'Loading…'}</p>{/if}
</div>
<style>
  .so-backdrop{position:fixed;inset:0;background:rgba(0,0,0,.6);z-index:100}
  .so{position:fixed;z-index:101;top:50%;left:50%;transform:translate(-50%,-50%);width:min(720px,calc(100vw - 32px));max-height:calc(100vh - 40px);overflow:auto;background:#1a2325;border:1px solid #2c393a;border-radius:14px;padding:20px 24px;color:#e8ecec}
  .so-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px;margin-bottom:8px}.so-head h2{margin:0}
  .ss-row{display:flex;justify-content:space-between;align-items:center;gap:16px;padding:10px 0;border-top:1px solid #2c393a}
  .ss-row > div:first-child{display:flex;flex-direction:column;gap:2px;min-width:0}.ss-row small{color:#8c9c9b;font-size:12px}
  .ss-row select{background:#111617;color:#e8ecec;border:1px solid #2c393a;border-radius:8px;padding:8px 10px;font-size:13px}
  .ss-notice{min-height:1.2em;font-size:12px;color:#9cacab;margin:8px 0 0}
</style>
