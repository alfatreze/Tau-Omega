<script lang="ts">
  // Developer tools: run notes. One note per install of a test build, kept on this computer: what went on (hashes), when, the backup, a line of your own,
  // and a snapshot of what the Pocket reported afterwards. A note exports as a plain Markdown report.
  import { open as pick } from '@tauri-apps/plugin-dialog';
  import { errorMessage } from './backend';
  import { runNoteAttach, runNoteDelete, runNoteExport, runNoteReport, runNotesList, runNoteText } from './tau-api';
  import { localStamp, cardLabel } from './devStore';
  import { plural } from './coresFormat';
  import type { RunNote } from './types';
  export let card = '';

  let notes: RunNote[] = []; let error = ''; let msg: Record<string, string> = {}; let reports: Record<string, string> = {}; let open: Record<string, boolean> = {}; let confirmDel = '';
  let askedFor = '';
  $: if (card && card !== askedFor) { askedFor = card; void load(); }
  async function load() { try { notes = await runNotesList(card); error = ''; } catch (e) { notes = []; error = errorMessage(e); } }
  const say = (id: string, m: string) => { msg[id] = m; msg = msg; };
  const nice = (t: string) => t.replace('T', ' ');
  const swap = (n: RunNote) => { notes = notes.map((x) => (x.id === n.id ? n : x)); };

  async function saveText(n: RunNote, text: string) {
    if (text === n.text) return;
    try { swap(await runNoteText(n.id, text)); say(n.id, 'Saved.'); } catch (e) { say(n.id, `Not saved. ${errorMessage(e)}`); }
  }
  async function attach(n: RunNote) {
    say(n.id, 'Reading the screenshots…');
    try { const u = await runNoteAttach(card, n.id, localStamp()); swap(u); say(n.id, u.results.length ? `Attached ${plural(u.results.length, 'run')}.` : 'No Check reports since this install yet: run Check on the Pocket and take a screenshot, then attach again.'); delete reports[n.id]; }
    catch (e) { say(n.id, errorMessage(e)); }
  }
  async function report(n: RunNote) { if (!reports[n.id]) reports[n.id] = await runNoteReport(n.id); reports = reports; return reports[n.id]; }
  async function toggleReport(n: RunNote) { try { await report(n); open[n.id] = !open[n.id]; open = open; } catch (e) { say(n.id, errorMessage(e)); } }
  async function copy(n: RunNote) {
    try { const t = await report(n); await navigator.clipboard.writeText(t); say(n.id, 'Report copied.'); } catch (e) { say(n.id, `Could not copy. ${errorMessage(e)}`); }
  }
  async function save(n: RunNote) {
    const dir = await pick({ directory: true, title: 'Choose a folder for the report' });
    if (typeof dir !== 'string') return;
    try { say(n.id, `Saved ${await runNoteExport(n.id, dir)}`); } catch (e) { say(n.id, errorMessage(e)); }
  }
  async function remove(n: RunNote) { try { await runNoteDelete(n.id); confirmDel = ''; await load(); } catch (e) { say(n.id, errorMessage(e)); } }
</script>
<section aria-label="Run notes">
  <p class="rn-lede">One note for each install of a test build, kept on this computer only. Add a line of your own, attach what the Pocket reported, and export the whole thing as a plain report.</p>
  {#if error}<p class="rn-err" role="alert">{error}</p>{/if}
  {#if !card}<div class="empty"><h2>Open a card first</h2></div>
  {:else if !notes.length}<div class="empty"><h2>No run notes for {cardLabel(card)}</h2><p>A note is created each time you finish installing a build with developer tools on.</p></div>
  {:else}
    {#each notes as n (n.id)}
      <article class="rn" aria-label={`Run note for ${n.shortname || n.package}`}>
        <header class="rn-head"><div><h2>{n.shortname || n.package} <small>{n.version}</small></h2><p class="rn-sub">Installed {nice(n.stamp)} · {plural(n.files_written, 'file')} written</p></div>
          {#if confirmDel === n.id}<span class="rn-confirm" role="alert">Delete this note? <button class="quiet danger" on:click={() => remove(n)}>Delete</button><button class="quiet" on:click={() => (confirmDel = '')}>Keep</button></span>
          {:else}<button class="quiet" aria-label={`Delete the note for ${n.shortname || n.package}`} on:click={() => (confirmDel = n.id)}>Delete…</button>{/if}</header>
        <dl class="rn-facts">
          <div><dt>Package</dt><dd>{n.package}</dd></div>
          <div><dt>Package SHA-256</dt><dd><code title={n.package_sha256}>{n.package_sha256.slice(0, 16)}…</code></dd></div>
          {#each n.bitstreams as [name, h]}<div><dt>Bitstream</dt><dd><code title={h}>{h.slice(0, 16)}…</code> <small>{name.split('/').pop()}</small></dd></div>{/each}
          <div><dt>Backup</dt><dd><code>{n.backup_dir}</code></dd></div>
        </dl>
        <label class="rn-text">Your note<textarea rows="2" placeholder="What you changed, what you expect, what you noticed" value={n.text} on:blur={(e) => saveText(n, e.currentTarget.value)}></textarea></label>
        <h3>What the Pocket reported</h3>
        {#if !n.results.length}<p class="rn-none">Nothing attached yet.</p>
        {:else}
          <ul class="rn-res" aria-label="Attached results">{#each n.results as r}
            <li><strong>{r.profile}</strong><span class="chip" class:capable={r.verdict.includes('passed')} class:bad={r.verdict.includes('failed')}>{r.verdict}</span>
              <small>{nice(r.captured_at)}{r.failing.length ? ` · failing: ${r.failing.join(', ')}` : ''} · {r.changes.length ? plural(r.changes.length, 'change') + ' since the previous run' : 'no measurable change'}</small></li>
          {/each}</ul>
          {#if n.results_at}<p class="rn-none">Attached {nice(n.results_at)}.</p>{/if}
        {/if}
        <div class="rn-actions">
          <button class="primary" on:click={() => attach(n)}>{n.results.length ? 'Attach the results again' : 'Attach the results so far'}</button>
          <button class="quiet" on:click={() => copy(n)}>Copy report</button>
          <button class="quiet" on:click={() => save(n)}>Save report…</button>
          <button class="quiet" aria-expanded={!!open[n.id]} on:click={() => toggleReport(n)}>{open[n.id] ? 'Hide the report' : 'Show the report'}</button>
          {#if msg[n.id]}<span class="rn-msg" role="status">{msg[n.id]}</span>{/if}
        </div>
        {#if open[n.id] && reports[n.id]}<pre class="rn-report" aria-label="Report">{reports[n.id]}</pre>{/if}
      </article>
    {/each}
  {/if}
</section>
<style>
  .rn-lede{font-size:13px;color:#a6b3b2;margin:0 0 14px}.rn-err{color:#ff9b9b;font-size:12.5px}
  .rn{border:1px solid #2c393a;border-radius:12px;background:#161f20;padding:16px 18px;margin:0 0 16px}
  .rn-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px}.rn-head h2{margin:0;font-size:16px}.rn-head h2 small{font-size:12px;font-weight:400;color:#8f9e9d;margin-left:8px}.rn-sub{margin:4px 0 0;font-size:12px;color:#8f9e9d}
  .rn-confirm{display:flex;gap:6px;align-items:center;font-size:13px;color:#f4cfc4}.rn-confirm .danger{color:#ff9d8a}
  .rn-facts{display:grid;gap:6px;margin:12px 0}.rn-facts>div{display:grid;grid-template-columns:130px 1fr;gap:10px;align-items:baseline}.rn-facts dt{font-size:12px;color:#8f9e9d}.rn-facts dd{margin:0;font-size:12.5px;overflow-wrap:anywhere}.rn-facts small{color:#7f8e8d}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
  .rn-text{display:grid;gap:5px;font-size:12px;color:#a6b3b2}.rn-text textarea{font:inherit;font-size:13px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:9px;padding:9px 12px;resize:vertical;width:100%;box-sizing:border-box}
  .rn h3{margin:14px 0 6px;font-size:11px;letter-spacing:.09em;color:#9fb3aa;text-transform:uppercase}.rn-none{font-size:12.5px;color:#95a5a4;margin:4px 0}
  .rn-res{list-style:none;margin:0;padding:0;border:1px solid #2c393a;border-radius:9px}.rn-res li{display:flex;gap:10px;align-items:center;flex-wrap:wrap;padding:8px 12px;border-bottom:1px solid #2c393a;font-size:13px}.rn-res li:last-child{border:0}.rn-res small{color:#95a5a4;font-size:12px}
  .chip.bad{background:#3a1c1c;color:#ffb4b4}.rn-actions{display:flex;gap:8px;align-items:center;flex-wrap:wrap;margin-top:12px}.rn-msg{font-size:12.5px;color:#c7d1d0}
  .rn-report{margin:12px 0 0;padding:12px 14px;background:#101617;border:1px solid #2c393a;border-radius:9px;font-size:12px;line-height:1.55;white-space:pre-wrap;overflow-wrap:anywhere;max-height:360px;overflow:auto}
</style>
