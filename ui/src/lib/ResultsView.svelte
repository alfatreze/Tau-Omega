<script lang="ts">
  // Developer tools: the Results page. What the Pocket's own screenshots say about the build you just installed: every Check report found in
  // Memories/Screenshots is decoded here, grouped by profile, and compared with the previous report of the same profile. Nothing is written to the card.
  import { onMount } from 'svelte';
  import { errorMessage } from './backend';
  import { readImageDataUrl, resultsScan, setLastInstall } from './tau-api';
  import { cardLabel, localStamp } from './devStore';
  import { plural } from './coresFormat';
  import type { ResultRun, ResultShot, ResultsView } from './types';
  export let card = '';

  let view: ResultsView | null = null; let error = ''; let busy = false; let note = '';
  let showAll = false; let pics: Record<string, string> = {}; let openPic = '';
  let askedFor = '';
  $: if (card && card !== askedFor) { askedFor = card; void scan(); }
  onMount(() => { if (card && !askedFor) { askedFor = card; void scan(); } });
  async function scan() {
    busy = true; error = '';
    try { view = await resultsScan(card); } catch (e) { view = null; error = errorMessage(e); } finally { busy = false; }
  }
  async function baseline(stamp: string | null, said: string) {
    note = '';
    try { await setLastInstall(card, stamp); note = said; await scan(); } catch (e) { error = errorMessage(e); }
  }
  const byName = (n: string | null): ResultShot | undefined => (n ? view?.shots.find((s) => s.filename === n) : undefined);
  $: shownRuns = (view?.runs ?? []).filter((r) => showAll || r.since_install);
  $: hiddenRuns = (view?.runs.length ?? 0) - shownRuns.length;
  $: others = (view?.shots ?? []).filter((s) => !s.report && !s.problem && (showAll || s.since_install));
  $: problems = (view?.shots ?? []).filter((s) => s.problem && (showAll || s.since_install));
  const nice = (t: string | null) => (t ? t.replace('T', ' ') : 'unknown time');
  const CH: Record<string, string> = { better: 'Better', worse: 'Worse', changed: 'Changed' };
  async function toggle(name: string) {
    if (openPic === name) { openPic = ''; return; }
    openPic = name; const s = byName(name);
    if (s && !pics[name]) { try { pics[name] = await readImageDataUrl(s.path); pics = pics; } catch { /* the picture is optional */ } }
  }
  const failing = (r: ResultRun) => byName(r.latest)?.report?.tests.filter((t) => t.result === 'FAIL').map((t) => t.name) ?? [];
</script>
<section class="page results" aria-labelledby="results-title">
  <header><div><p class="eyebrow">DEVELOPER</p><h1 id="results-title">Results</h1><p class="lede">What the Pocket’s screenshots say about your test build: every Check report is decoded here and compared with the previous one. Nothing is written to the card.</p></div></header>
  {#if !card}<div class="empty"><h2>Open a card first</h2><p>Results are read from the card’s <code>Memories/Screenshots</code> folder.</p></div>
  {:else}
    <div class="r-bar">
      <p class="r-since" role="status">{view?.since ? `Showing what the Pocket saved since the last install on ${cardLabel(card)}, ${nice(view.since)}.` : `Showing every screenshot on ${cardLabel(card)}: no install has been recorded.`}</p>
      <div class="r-btns">
        <button class="quiet" disabled={busy} on:click={scan}>Rescan</button>
        <button class="quiet" on:click={() => baseline(localStamp(), 'Starting fresh from now: only screenshots taken from now on count as new.')}>Start fresh from now</button>
        {#if view?.since}<button class="quiet" on:click={() => baseline(null, 'Showing everything.')}>Show everything</button>{/if}
        <label class="r-all"><input type="checkbox" bind:checked={showAll} /> Include older runs</label>
      </div>
    </div>
    {#if note}<p class="r-note" role="status">{note}</p>{/if}
    {#if error}<p class="r-err" role="alert">{error}</p>{/if}
    {#if busy && !view}<p class="r-note" role="status">Reading the screenshots…</p>{/if}
    {#if view}
      {#if !shownRuns.length}
        <div class="empty"><h2>No Check reports {view.since && !showAll ? 'since the last install' : 'found'}</h2><p>Run Check on the Pocket and take a screenshot of the report page; it shows up here when the card is mounted.{hiddenRuns > 0 ? ` ${plural(hiddenRuns, 'older run')} hidden: tick “Include older runs”.` : ''}</p></div>
      {/if}
      {#each shownRuns as r (r.profile)}
        {@const cur = byName(r.latest)}
        {@const prev = byName(r.previous)}
        <article class="r-run" aria-label={`${r.profile} run`}>
          <header class="r-head">
            <div><h2>{r.profile} <small>{nice(cur?.captured_at ?? null)}</small></h2>
              <p class="r-sub">{cur?.report?.entries.build ? `Firmware ${cur.report.entries.build.firmware} · bitstream ${cur.report.entries.build.bitstream}` : 'No build information in this report.'}</p></div>
            <span class="chip" class:capable={cur?.report?.verdict.includes('passed')} class:bad={cur?.report?.verdict.includes('failed')}>{cur?.report?.verdict ?? ''}</span>
          </header>
          {#if failing(r).length}<p class="r-fail">Failing: {failing(r).join(', ')}</p>{/if}
          <h3>{prev ? `Compared with the previous ${r.profile} run, ${nice(prev.captured_at)}` : 'First run of this profile'}</h3>
          {#if !prev}<p class="r-note">Nothing to compare with yet.</p>
          {:else if !r.diff.length}<p class="r-ok">No measurable change.</p>
          {:else}
            <ul class="r-diff" aria-label={`Changes in the ${r.profile} run`}>{#each r.diff as d}
              <li class={d.change}><span class="chip {d.change === 'worse' ? 'bad' : d.change === 'better' ? 'capable' : ''}">{CH[d.change]}</span><strong>{d.label}</strong><span class="r-ba">{d.before} → {d.after}</span></li>
            {/each}</ul>
          {/if}
          <div class="r-foot"><button class="quiet" aria-expanded={openPic === r.latest} on:click={() => toggle(r.latest)}>{openPic === r.latest ? 'Hide the screenshot' : 'Show the screenshot'}</button><small>{r.latest}</small></div>
          {#if openPic === r.latest && pics[r.latest]}<img class="r-pic" src={pics[r.latest]} alt={`Screenshot ${r.latest}`} />{/if}
        </article>
      {/each}
      {#if problems.length}
        <h2 class="r-h">Report pages that could not be read</h2>
        <ul class="r-list" aria-label="Unreadable report pages">{#each problems as s}<li><strong>{s.filename}</strong><small>{s.problem}</small></li>{/each}</ul>
      {/if}
      {#if others.length}<p class="r-note">{plural(others.length, 'other screenshot')} {showAll ? 'on the card' : 'since the last install'} {others.length === 1 ? 'is not a Check report' : 'are not Check reports'}.</p>{/if}
    {/if}
  {/if}
</section>
<style>
  .r-bar{display:flex;justify-content:space-between;align-items:center;gap:14px;flex-wrap:wrap;margin:0 0 12px}.r-since{margin:0;font-size:13px;color:#c7d1d0}
  .r-btns{display:flex;gap:8px;align-items:center;flex-wrap:wrap}.r-all{font-size:12.5px;color:#a6b3b2;display:inline-flex;gap:6px;align-items:center}
  .r-note{font-size:12.5px;color:#95a5a4}.r-err{color:#ff9b9b;font-size:12.5px}.r-ok{font-size:13px;color:#c5f7ad;margin:6px 0}
  .r-run{border:1px solid #2c393a;border-radius:12px;background:#161f20;padding:16px 18px;margin:0 0 16px}
  .r-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px}.r-head h2{margin:0;font-size:16px}.r-head h2 small{font-size:12px;font-weight:400;color:#8f9e9d;margin-left:8px}.r-sub{margin:4px 0 0;font-size:12px;color:#8f9e9d}
  .r-fail{margin:8px 0 0;font-size:12.5px;color:#ffb4b4}.r-run h3{margin:14px 0 6px;font-size:11px;letter-spacing:.09em;color:#9fb3aa;text-transform:uppercase}
  .r-diff{list-style:none;margin:0;padding:0;border:1px solid #2c393a;border-radius:9px}.r-diff li{display:grid;grid-template-columns:84px minmax(0,1fr) auto;gap:10px;align-items:center;padding:8px 12px;border-bottom:1px solid #2c393a;font-size:13px}.r-diff li:last-child{border:0}.r-ba{color:#c7d1d0;font-size:12.5px;font-variant-numeric:tabular-nums}
  .chip.bad{background:#3a1c1c;color:#ffb4b4}.r-foot{display:flex;gap:12px;align-items:center;margin-top:12px}.r-foot small{color:#7f8e8d;font-size:11.5px}.r-pic{display:block;max-width:100%;border-radius:10px;margin-top:10px;border:1px solid #2c393a}
  .r-h{font-size:14px;margin:20px 0 8px}.r-list{list-style:none;margin:0;padding:0;border:1px solid #2c393a;border-radius:10px}.r-list li{display:grid;gap:3px;padding:9px 12px;border-bottom:1px solid #2c393a;font-size:13px}.r-list li:last-child{border:0}.r-list small{color:#e5d9a8;font-size:12px}
</style>
