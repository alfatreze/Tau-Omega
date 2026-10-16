<script lang="ts">
  // Skip silence: the tracks with what would be skipped, and a per-track editor: the waveform with two cut marks (drag them, or type seconds),
  // "never skip", and reset to what was detected. docs/SKIP_SILENCE.md. A change is written to the card's tau-skip.bin for that folder.
  import { onMount } from 'svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { getPrefs, skipDetail, skipSetEdit, skipTracks } from './tau-api';
  import type { SkipDetail, SkipTrack } from './types';
  export let card = ''; export let mediaRoot = ''; export let close: () => void;

  let tracks: SkipTrack[] = []; let enabled = true; let loading = true; let error = ''; let notice = '';
  let sel: SkipTrack | null = null; let detail: SkipDetail | null = null; let busy = false;
  let leadS = 0; let tailS = 0; let never = false;
  const sec = (frames: number, rate: number) => (rate ? frames / rate : 0);
  const fmt = (s: number) => (s >= 60 ? `${Math.floor(s / 60)} min ${(s % 60).toFixed(1)} s` : `${s.toFixed(1)} s`);

  async function load() {
    loading = true; error = '';
    try { enabled = (await getPrefs()).trim.enabled; tracks = enabled ? await skipTracks(mediaRoot) : []; }
    catch (e) { error = errorMessage(e); }
    loading = false;
  }
  onMount(load);

  $: groups = (() => { const m = new Map<string, SkipTrack[]>(); for (const t of tracks) (m.get(t.dir) ?? m.set(t.dir, []).get(t.dir)!).push(t); return [...m.entries()]; })();
  $: withSkip = tracks.filter((t) => t.skip && (t.skip.lead_skip || t.skip.tail_skip)).length;

  async function choose(t: SkipTrack) {
    sel = t; detail = null; notice = ''; busy = true;
    try {
      const d = await skipDetail(mediaRoot, t.rel); detail = d;
      leadS = sec(d.skip.lead_skip, d.rate); tailS = sec(d.skip.tail_skip, d.rate); never = !!d.edit?.never;
    } catch (e) { error = errorMessage(e); }
    busy = false;
  }
  const total = () => (detail ? sec(detail.frames, detail.rate) : 0);
  const clampS = (v: number) => Math.max(0, Math.min(total() - 1, Number.isFinite(v) ? v : 0));
  $: if (detail && !never) { /* keep the two cuts from crossing */ if (leadS + tailS > total() - 1) tailS = Math.max(0, total() - 1 - leadS); }

  let svgEl: SVGSVGElement; let drag: 'lead' | 'tail' | null = null;
  function at(e: PointerEvent) { const r = svgEl.getBoundingClientRect(); return Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)) * total(); }
  function move(e: PointerEvent) { if (!drag || never) return; const t = at(e); if (drag === 'lead') leadS = clampS(Math.min(t, total() - 1 - tailS)); else tailS = clampS(Math.min(total() - t, total() - 1 - leadS)); }

  async function write(edit: Parameters<typeof skipSetEdit>[3], message: string) {
    if (!sel || !detail) return; busy = true; error = '';
    try { await skipSetEdit(card, mediaRoot, sel.rel, edit); notice = message; await load(); const again = tracks.find((t) => t.rel === sel!.rel); if (again) await choose(again); notice = message; }
    catch (e) { error = errorMessage(e); }
    busy = false;
  }
  function save() {
    if (!detail) return;
    if (never) return write({ never: true, lead: null, tail: null }, 'This track will always play whole.');
    const lead = Math.round(leadS * detail.rate); const tail = Math.round(tailS * detail.rate);
    const same = lead === detail.auto.lead_skip && tail === detail.auto.tail_skip;
    return same ? write(null, 'Back to what was detected.') : write({ never: false, lead, tail }, 'Saved. The numbers are written as typed.');
  }
  const reset = () => write(null, 'Back to what was detected.');
  const secs = (t: SkipTrack) => (t.skip && t.rate ? `${fmt(sec(t.skip.lead_skip, t.rate))} · ${fmt(sec(t.skip.tail_skip, t.rate))}` : 'not measured');
</script>
<svelte:window on:keydown={(e) => e.key === 'Escape' && close()} />
<div class="sk-backdrop" role="presentation" on:click={close}></div>
<div class="sk" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="sk-title" use:modal>
  <header class="sk-head"><div><p class="eyebrow">SKIP SILENCE</p><h2 id="sk-title">Tracks and what is skipped</h2></div><button class="quiet" data-autofocus on:click={close}>Done</button></header>
  {#if loading}<p class="sk-note">Reading the tracks…</p>
  {:else if !enabled}<p class="sk-note you">Skip silence is off. Turn it on in Settings > Syncing music, then sync once so the tracks are measured.</p>
  {:else}
    <p class="sk-note">{withSkip} of {tracks.length} tracks have silence to skip. Lead then tail shown for each track. Changes are written to the card straight away.</p>
    <div class="sk-body">
      <ul class="sk-list" aria-label="Tracks">
        {#each groups as [dir, list]}
          <li class="sk-dir">{dir || 'Music'}</li>
          {#each list as t}<li><button class="sk-track" class:on={sel?.rel === t.rel} on:click={() => choose(t)} aria-label={`${t.file}: ${secs(t)}`}><span>{t.file}{#if t.edit}<em>{t.edit.never ? 'never skip' : 'edited'}</em>{/if}</span><small>{secs(t)}</small></button></li>{/each}
        {/each}
      </ul>
      <section class="sk-edit" aria-label="Edit the selected track">
        {#if !sel}<p class="sk-note">Choose a track to see its waveform.</p>
        {:else if busy && !detail}<p class="sk-note">Reading {sel.file}…</p>
        {:else if detail}
          <h3>{sel.file}</h3>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <svg bind:this={svgEl} class="sk-wave" viewBox="0 0 400 80" preserveAspectRatio="none" role="img" aria-label="Waveform with the skipped start and end shaded" on:pointermove={move} on:pointerup={() => (drag = null)} on:pointerleave={() => (drag = null)}>
            {#each detail.envelope as v, i}<rect x={i} y={40 - (v / 255) * 38} width="0.8" height={(v / 255) * 76 + 0.5} class="sk-bar" />{/each}
            {#if !never}
              <rect x="0" y="0" width={(leadS / total()) * 400} height="80" class="sk-cut" />
              <rect x={400 - (tailS / total()) * 400} y="0" width={(tailS / total()) * 400} height="80" class="sk-cut" />
              <rect x={(leadS / total()) * 400 - 2} y="0" width="4" height="80" class="sk-handle" role="slider" tabindex="0" aria-label="Start of the track" aria-valuenow={leadS} on:pointerdown={() => (drag = 'lead')} />
              <rect x={400 - (tailS / total()) * 400 - 2} y="0" width="4" height="80" class="sk-handle" role="slider" tabindex="0" aria-label="End of the track" aria-valuenow={tailS} on:pointerdown={() => (drag = 'tail')} />
            {/if}
          </svg>
          <p class="sk-note">Track length {fmt(total())}. {never ? 'Plays whole.' : `Plays from ${fmt(leadS)} to ${fmt(total() - tailS)}, ${fmt(total() - leadS - tailS)} in all.`}</p>
          <div class="sk-fields">
            <label>Skip at the start (s)<input type="number" min="0" step="0.1" disabled={never} value={leadS.toFixed(2)} on:change={(e) => (leadS = clampS(Number(e.currentTarget.value)))} aria-label="Seconds skipped at the start" /></label>
            <label>Skip at the end (s)<input type="number" min="0" step="0.1" disabled={never} value={tailS.toFixed(2)} on:change={(e) => (tailS = clampS(Number(e.currentTarget.value)))} aria-label="Seconds skipped at the end" /></label>
          </div>
          <label class="sk-check"><input type="checkbox" bind:checked={never} /> Never skip silence on this track</label>
          <div class="sk-actions"><button class="quiet" disabled={busy || !detail.edit} on:click={reset}>Back to detected</button><button class="picker" disabled={busy} on:click={save}>Save</button></div>
        {/if}
        {#if notice}<p class="sk-note ok" role="status">{notice}</p>{/if}
      </section>
    </div>
  {/if}
  {#if error}<p class="sk-note bad" role="alert">{error}</p>{/if}
</div>
<style>
  .sk-backdrop{position:fixed;inset:0;background:rgba(0,0,0,.6);z-index:100}
  .sk{position:fixed;z-index:101;top:50%;left:50%;transform:translate(-50%,-50%);width:min(980px,calc(100vw - 32px));max-height:calc(100vh - 40px);overflow:auto;background:#1a2325;border:1px solid #2c393a;border-radius:14px;padding:20px 24px;color:#e8ecec}
  .sk-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px;margin-bottom:8px}.sk-head h2{margin:0}
  .sk-body{display:grid;grid-template-columns:minmax(260px,1fr) minmax(320px,1.3fr);gap:18px}
  .sk-list{list-style:none;margin:0;padding:0;max-height:52vh;overflow:auto;border:1px solid #2c393a;border-radius:10px}
  .sk-dir{padding:6px 12px;font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:#8f9e9d;background:#141c1d;position:sticky;top:0}
  .sk-track{display:flex;justify-content:space-between;gap:10px;width:100%;text-align:left;background:none;border:0;border-bottom:1px solid #222e2f;color:inherit;padding:8px 12px;font:inherit;font-size:13px}
  .sk-track:hover,.sk-track.on{background:#223033}.sk-track small{color:#95a5a4;white-space:nowrap}.sk-track em{margin-left:8px;font-style:normal;font-size:11px;color:#e5d9a8}
  .sk-edit h3{margin:0 0 8px;font-size:14px;overflow-wrap:anywhere}
  .sk-wave{display:block;width:100%;height:110px;background:#101617;border:1px solid #2c393a;border-radius:8px;touch-action:none}
  .sk-bar{fill:#7fb7a8}.sk-cut{fill:rgba(255,155,155,.28)}.sk-handle{fill:#ffb86b;cursor:ew-resize}
  .sk-fields{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin:10px 0}.sk-fields label{display:grid;gap:4px;font-size:12.5px;color:#a6b3b2}
  .sk-fields input{font:inherit;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:8px 10px}
  .sk-check{display:block;font-size:12.5px;margin:8px 0}.sk-actions{display:flex;justify-content:flex-end;gap:8px;margin-top:10px}
  .sk-note{font-size:12.5px;color:#95a5a4;line-height:1.55;margin:6px 0}.sk-note.bad{color:#ff9b9b}.sk-note.ok{color:#c5f7ad}.sk-note.you{color:#e5d9a8}
  @media (max-width:760px){.sk-body{grid-template-columns:1fr}}
</style>
