<script lang="ts">
  // Send diagnostics: reads what a Check run left on the open card and packs it into one zip to attach to a bug
  // report. Read-only on the card; the zip is written to a folder you choose, outside the card. Nothing is uploaded.
  import { open } from '@tauri-apps/plugin-dialog';
  import { diagRead, diagZip, errorMessage } from './tau-api';
  import type { DiagView } from './types';

  /** The open card's root folder, or '' when no card is open. */
  export let cardPath = '';
  export let cardLabel = '';

  let view: DiagView | null = null;
  let loading = false;
  let error = '';
  let notice = '';
  let loadedFor = '';

  $: if (cardPath !== loadedFor) { loadedFor = cardPath; view = null; notice = ''; error = ''; if (cardPath) read(); }

  async function read() {
    loading = true; error = '';
    try { view = await diagRead(cardPath); }
    catch (e) { error = errorMessage(e); view = null; }
    finally { loading = false; }
  }
  async function copySummary() {
    if (!view) return;
    try { await navigator.clipboard.writeText(view.summary); notice = 'Summary copied. Paste it into your bug report.'; }
    catch { notice = 'The summary could not be copied here; use Create zip instead, it holds the same text as report.md.'; }
  }
  async function createZip() {
    notice = '';
    const dest = await open({ directory: true, multiple: false, title: 'Where should the zip be saved? (not on the card)' });
    if (typeof dest !== 'string') return;
    loading = true;
    try { const path = await diagZip(cardPath, dest); notice = `Saved ${path}. It stays on your computer until you choose to send it; nothing was uploaded.`; }
    catch (e) { notice = errorMessage(e); }
    finally { loading = false; }
  }
  const mb = (b: number) => `${(b / 1048576).toFixed(1)} MB`;
  $: reading = view?.reading;
  $: anyEvidence = !!reading && (reading.shots.length > 0 || reading.cores.some((c) => c.check));
</script>

<section class="page dg" aria-labelledby="dg-title">
  <header><div><p class="eyebrow">SUPPORT</p><h1 id="dg-title">Send diagnostics</h1><p class="lede">Gathers what a Tau Check left on {cardLabel || 'the card'} (its saved results and any screenshots of the Check’s QR code) into one zip for a bug report. Nothing is uploaded, nothing is written to the card, and it never includes track names or other cores’ settings.</p></div></header>

  {#if !cardPath}
    <p class="dg-note" role="status">Open a card in Cards first.</p>
  {:else}
    <div class="dg-actions">
      <button class="quiet" disabled={loading} on:click={read}>{loading ? 'Reading…' : 'Read card again'}</button>
      <button class="quiet" disabled={!view || loading} on:click={copySummary}>Copy summary</button>
      <button class="primary" disabled={!view || loading || !anyEvidence} title={!anyEvidence && view ? 'There is no Check result to send yet.' : ''} on:click={createZip}>Create zip…</button>
    </div>
    {#if notice}<p class="dg-note" role="status">{notice}</p>{/if}
    {#if error}<p class="dg-bad" role="alert">{error}</p>{/if}
    {#if loading && !view}<p class="muted" role="status">Reading the card…</p>{/if}

    {#if reading}
      {#each reading.notes as n}<p class="dg-note" role="note">{n}</p>{/each}
      {#each reading.cores as c (c.id)}
        <article class="dg-card">
          <h3>{c.id} <small>{c.version} · {c.date_release}</small></h3>
          {#if c.check}
            <p class:ok={c.check.failed.length === 0} class:bad={c.check.failed.length > 0}><b>{c.check.verdict}</b> · {c.check.profile} · run {c.check.run}</p>
            <ul class="dg-tests">
              {#each c.check.passed as t}<li class="p"><span>PASS</span>{t}</li>{/each}
              {#each c.check.failed as t}<li class="f"><span>FAIL</span>{t}</li>{/each}
            </ul>
            <p class="muted">Worst memory access {c.check.worst_access_cycles} cycles · {c.check.late_underruns} late underruns · draw stall {c.check.draw_stall_ms} ms</p>
          {:else if c.check_note}<p class="muted">{c.check_note}</p>{/if}
          <p class="muted">Library: {c.index ? `${c.index.tracks} tracks, ${c.index.albums} albums` : 'no valid index'} · media {c.media_files} files, {mb(c.media_bytes)}</p>
        </article>
      {/each}
      {#if reading.shots.length}
        <h3>Screenshots with a Check report</h3>
        {#each reading.shots as s (s.filename)}
          <article class="dg-card">
            <h3>{s.filename}</h3>
            {#if s.report}
              <p><b>{s.report.verdict}</b> · {s.report.profile}</p>
              <ul class="dg-tests">{#each s.report.tests as t}<li class={t.result === 'FAIL' ? 'f' : t.result === 'PASS' ? 'p' : ''}><span>{t.result}</span>{t.name}</li>{/each}</ul>
            {:else if s.note}<p class="dg-bad">{s.note}</p>{/if}
          </article>
        {/each}
      {/if}
      <p class="muted">Looked at the {reading.shots_examined} newest screenshot{reading.shots_examined === 1 ? '' : 's'}. Card: {mb(reading.free_bytes)} free of {mb(reading.total_bytes)}.</p>
    {/if}
  {/if}
</section>

<style>
  .dg{display:grid;gap:12px;align-content:start}
  .dg h3,.dg p:not(.eyebrow){margin:0}
  .dg-actions{display:flex;gap:8px;flex-wrap:wrap}
  .dg button{padding:8px 12px;font-size:13px;background:#202b2d;color:#e8ecec;border:1px solid #2c393a;border-radius:8px;cursor:pointer}
  .dg button.quiet{background:transparent}.dg button.primary{background:#c1f0ad;color:#142015;font-weight:700;border-color:#c1f0ad}
  .dg button:disabled{opacity:.5;cursor:not-allowed}
  .dg-note{padding:8px 12px;border:1px solid #2c393a;background:#161f20;border-radius:8px;font-size:13px}
  .dg-bad{color:#ffb4a8;font-size:13px}
  .dg-card{border:1px solid #2c393a;background:#161f20;border-radius:10px;padding:12px;display:grid;gap:6px}
  .dg-card h3{font-size:14px}.dg-card small{color:#9fb3aa;font-weight:400}
  .dg-card .ok{color:#c1f0ad}.dg-card .bad{color:#ffb4a8}
  .dg-tests{list-style:none;margin:0;padding:0;display:grid;gap:2px;font-size:12px}
  .dg-tests li{display:grid;grid-template-columns:48px 1fr;gap:8px;color:#c7d1d0}.dg-tests li span{font-weight:700}
  .dg-tests li.p span{color:#c1f0ad}.dg-tests li.f span{color:#ffb4a8}
</style>
