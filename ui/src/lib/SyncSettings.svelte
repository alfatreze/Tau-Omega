<script lang="ts">
  // Settings -> Syncing: what happens to files the Pocket cannot play, and ReplayGain. Saved straight to
  // the preferences every sync reads.
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { checkLame, errorMessage, getPrefs, installLame, lameStatus, setPrefs } from './tau-api';
  import type { ConvertSettings, LameStatus, PrefsView, TrimSettings } from './types';

  let prefs: PrefsView | null = null; let lame: LameStatus | null = null; let notice = ''; let busy = false;
  onMount(async () => {
    try { prefs = await getPrefs(); } catch (error) { notice = `Could not read preferences: ${errorMessage(error)}`; }
    try { lame = await lameStatus(); } catch { lame = null; }
  });
  async function change(part: Partial<ConvertSettings>, message = 'Saved.') {
    if (!prefs) return;
    try { prefs = await setPrefs({ ...prefs, convert: { ...prefs.convert, ...part } }); lame = await lameStatus(); notice = message; }
    catch (error) { notice = `Could not save: ${errorMessage(error)}`; }
  }
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
  async function chooseLame() {
    const selected = await open({ directory: false });
    if (!selected || Array.isArray(selected)) return;
    try { await checkLame(selected); await change({ lame_path: selected }, 'LAME found and saved.'); }
    catch (error) { notice = `That is not a working LAME: ${errorMessage(error)}`; }
  }
  async function install(method: string) {
    busy = true; notice = 'Installing LAME… this can take a minute.';
    try { lame = await installLame(method); notice = lame.path ? 'LAME is installed.' : 'The install finished but LAME was not found.'; }
    catch (error) { notice = errorMessage(error); }
    finally { busy = false; }
  }
  const mp3Value = (q: ConvertSettings['mp3']) => ('vbr' in q ? `v${q.vbr}` : `b${q.cbr}`);
  const mp3Parse = (v: string): ConvertSettings['mp3'] => (v.startsWith('v') ? { vbr: Number(v.slice(1)) } : { cbr: Number(v.slice(1)) });
  const MP3_CHOICES = [['v0', 'Best (V0, about 245 kbit/s)'], ['v2', 'High (V2, about 190 kbit/s)'], ['v4', 'Good (V4, about 165 kbit/s)'], ['b320', '320 kbit/s'], ['b256', '256 kbit/s'], ['b192', '192 kbit/s'], ['b128', '128 kbit/s']];
</script>
<section class="sync-settings" aria-labelledby="sync-settings-title">
  <h2 id="sync-settings-title">Syncing music</h2>
  {#if prefs}
    {@const c = prefs.convert}
    <p class="ss-lede">The Pocket plays MP3 and FLAC (FLAC up to 48 kHz, stereo). Files it can play are copied exactly as they are. Your originals are never changed.</p>
    <div class="ss-row"><div><b>Convert files the Pocket cannot play</b><small>Lossless files (WAV, AIFF, ALAC, hi-res FLAC) become FLAC; lossy ones (AAC, Ogg) become MP3. Off leaves them out and lists them.</small></div>
      <div><button class="quiet" on:click={() => change({ enabled: !c.enabled }, c.enabled ? 'Unplayable files will be left out.' : 'Unplayable files will be converted.')}>{c.enabled ? 'Turn off' : 'Turn on'}</button></div></div>
    {#if c.enabled}
      <fieldset>
        <legend>Quality</legend>
        <label class="radio"><input type="radio" name="conv-mode" checked={c.mode === 'auto'} on:change={() => change({ mode: 'auto' })} /><span><b>Auto</b><small>Match the best quality the Pocket accepts: hi-res is brought to 44.1 or 48 kHz keeping the bit depth, surround is folded to stereo, lossy files use LAME’s best setting.</small></span></label>
        <label class="radio"><input type="radio" name="conv-mode" checked={c.mode === 'manual'} on:change={() => change({ mode: 'manual' })} /><span><b>Manual</b><small>Choose the limits yourself, up to what the Pocket accepts.</small></span></label>
      </fieldset>
      {#if c.mode === 'manual'}
        <div class="ss-row"><div><b>Highest sample rate</b><small>Files above this are brought down to it.</small></div>
          <select aria-label="Highest sample rate" value={c.max_rate} on:change={(e) => change({ max_rate: Number(e.currentTarget.value) })}><option value={44100}>44.1 kHz</option><option value={48000}>48 kHz</option></select></div>
        <div class="ss-row"><div><b>FLAC bit depth</b><small>24-bit sources can be written as 16-bit to save space (with dither).</small></div>
          <select aria-label="FLAC bit depth" value={c.flac_bits} on:change={(e) => change({ flac_bits: Number(e.currentTarget.value) })}><option value={16}>16-bit</option><option value={24}>24-bit</option></select></div>
        <div class="ss-row"><div><b>MP3 quality</b><small>For lossy files that need converting.</small></div>
          <select aria-label="MP3 quality" value={mp3Value(c.mp3)} on:change={(e) => change({ mp3: mp3Parse(e.currentTarget.value) })}>{#each MP3_CHOICES as [v, l]}<option value={v}>{l}</option>{/each}</select></div>
        <div class="ss-row"><div><b>Also shrink playable FLAC files above these limits</b><small>Off by default: files the Pocket can already play are copied untouched.</small></div>
          <div><button class="quiet" on:click={() => change({ reduce_playable: !c.reduce_playable })}>{c.reduce_playable ? 'Turn off' : 'Turn on'}</button></div></div>
      {/if}
      <div class="ss-row"><div><b>LAME (makes the MP3 files)</b><small>{#if lame?.path}{lame.version ?? 'Found'} · {lame.path}{:else}Not installed. It is a separate free program; this app runs it but never bundles it.{/if}</small></div>
        <div class="ss-btns">
          {#if !lame?.path}{#each lame?.options ?? [] as o}{#if o.runnable}<button class="picker" disabled={busy} on:click={() => install(o.method)}>{o.label}</button>{:else}<span class="ss-hint">{o.label}: <code>{o.command}</code></span>{/if}{/each}{/if}
          <button class="quiet" on:click={chooseLame}>{lame?.path ? 'Choose another…' : 'I have it — choose…'}</button>
        </div></div>
    {/if}

    <fieldset>
      <legend>ReplayGain (volume levelling, in the copies only)</legend>
      <label class="radio"><input type="radio" name="rg" checked={c.replaygain === 'add_missing'} on:change={() => change({ replaygain: 'add_missing' })} /><span><b>Keep existing tags and add what is missing</b><small>Albums that already have ReplayGain are left alone. Measuring takes a little time on the first sync.</small></span></label>
      <label class="radio"><input type="radio" name="rg" checked={c.replaygain === 'rebuild'} on:change={() => change({ replaygain: 'rebuild' })} /><span><b>Always measure again and replace the tags</b><small>Every album gets fresh values in the copies.</small></span></label>
      <label class="radio"><input type="radio" name="rg" checked={c.replaygain === 'off'} on:change={() => change({ replaygain: 'off' })} /><span><b>Do not touch ReplayGain</b><small>Tags are copied as they are.</small></span></label>
    </fieldset>
    {@const t = prefs.trim}
    <fieldset>
      <legend>Skip silence at the start and end of tracks</legend>
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
    </fieldset>
    <p class="ss-notice" role="status">{notice}</p>
  {:else}<p>{notice || 'Loading…'}</p>{/if}
</section>
<style>
  .sync-settings{max-width:720px;margin-bottom:14px;padding:22px 24px;border:1px solid #2c393a;border-radius:12px;background:#1a2325}
  h2{margin:0 0 12px}.ss-lede{margin:0 0 8px;font-size:13px;color:#9cacab}
  fieldset{border:0;padding:0;margin:0 0 14px}legend{font-weight:700;margin:12px 0 8px}
  .radio{display:flex;gap:10px;align-items:flex-start;padding:8px 0;cursor:pointer}
  .radio span{display:flex;flex-direction:column;gap:2px}.radio small,.ss-row small{color:#8c9c9b;font-size:12px}
  .ss-row{display:flex;justify-content:space-between;align-items:center;gap:16px;padding:10px 0;border-top:1px solid #2c393a}
  .ss-row > div:first-child{display:flex;flex-direction:column;gap:2px;min-width:0}
  .ss-row select{background:#111617;color:#e8ecec;border:1px solid #2c393a;border-radius:8px;padding:8px 10px;font-size:13px}
  .ss-btns{display:flex;gap:8px;align-items:center;flex-wrap:wrap;justify-content:flex-end}.ss-hint{font-size:12px;color:#9cacab;max-width:300px}
  .ss-notice{min-height:1.2em;font-size:12px;color:#9cacab;margin:8px 0 0}
</style>
