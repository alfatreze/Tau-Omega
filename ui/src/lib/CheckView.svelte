<script lang="ts">
  // Developer tools: the Check page. The core checker on a package zip, a package or core folder, a core installed on the open card (with the checks
  // that explain "installed but not in the Pocket's menu"), or every core on the card. Nothing is written anywhere.
  import { open as pick } from '@tauri-apps/plugin-dialog';
  import { errorMessage } from './backend';
  import { checkCard, checkInstalled, checkSource } from './tau-api';
  import { coresTreeStore } from './coresStore';
  import { disabledList } from './coreStateStore';
  import { plural } from './coresFormat';
  import LintFindings from './LintFindings.svelte';
  import type { CardCheckRow, CheckResult } from './types';
  export let card = '';

  let source = ''; let installed = ''; let result: CheckResult | null = null; let rows: CardCheckRow[] | null = null; let error = ''; let busy = false; let copied = false;
  let isInstalled = false;
  $: cores = [...$coresTreeStore.flatMap((n) => n.cores.map((c) => ({ id: c.id, label: `${c.shortname} (${c.id})` }))), ...$disabledList.map((d) => ({ id: d.id, label: `${d.shortname || d.id} (${d.id}, disabled)` }))];

  async function run<T>(f: () => Promise<T>): Promise<T | null> { busy = true; error = ''; try { return await f(); } catch (e) { error = errorMessage(e); return null; } finally { busy = false; } }
  async function checkPath() { const r = await run(() => checkSource(source.trim())); if (r) { result = r; rows = null; isInstalled = false; } }
  async function choose(folder: boolean) {
    const r = await pick(folder ? { directory: true, title: 'Choose a core or package folder' } : { multiple: false, title: 'Choose a core zip', filters: [{ name: 'Core zip', extensions: ['zip'] }] });
    if (typeof r === 'string') { source = r; await checkPath(); }
  }
  async function checkOne(id: string) { installed = id; const r = await run(() => checkInstalled(card, id)); if (r) { result = r; rows = null; isInstalled = true; } }
  async function checkAll() { const r = await run(() => checkCard(card)); if (r) { rows = r; result = null; } }
  $: verdict = !result ? '' : result.lint.errors ? `${plural(result.lint.errors, 'error')} found` : result.lint.findings.length ? 'No errors' : 'Passes every check';
  async function copy() {
    if (!result) return;
    const lines = [`Core check: ${result.title}`, verdict, ...result.lint.findings.map((f) => `- [${f.severity}] ${f.id}: ${f.message} Fix: ${f.fix} (${f.rule})`), ...(result.symptoms.length ? ['', 'Why it may not be in the menu:', ...result.symptoms.map((f) => `- [${f.severity}] ${f.id}: ${f.message} Fix: ${f.fix}`)] : [])];
    try { await navigator.clipboard.writeText(lines.join('\n')); copied = true; setTimeout(() => (copied = false), 1500); } catch { /* the text stays on screen */ }
  }
</script>
<section class="page check" aria-labelledby="check-title">
  <header><div><p class="eyebrow">DEVELOPER</p><h1 id="check-title">Check</h1><p class="lede">The core checker: every documented limit and rule, with the rule it comes from and how to fix it. Run it on a package before you install, or on a core that is already on the card. Nothing is written.</p></div></header>

  <div class="ck-src">
    <label class="ck-in"><span>A package zip or a core folder</span><input bind:value={source} placeholder="/path/to/core.zip or /path/to/folder" on:keydown={(e) => e.key === 'Enter' && source.trim() && checkPath()} /></label>
    <button class="quiet" on:click={() => choose(false)}>Choose a zip…</button><button class="quiet" on:click={() => choose(true)}>Choose a folder…</button>
    <button class="primary" disabled={!source.trim() || busy} on:click={checkPath}>Check it</button>
  </div>
  <div class="ck-src">
    <label class="ck-in"><span>A core on the open card</span>
      <select bind:value={installed} disabled={!card || !cores.length}><option value="">{card ? 'Choose a core…' : 'Open a card first'}</option>{#each cores as c}<option value={c.id}>{c.label}</option>{/each}</select></label>
    <button class="primary" disabled={!installed || busy} on:click={() => checkOne(installed)}>Check it</button>
    <button class="quiet" disabled={!card || busy} on:click={checkAll}>Check every core on the card</button>
  </div>
  {#if error}<p class="ck-err" role="alert">{error}</p>{/if}
  {#if busy}<p class="ck-note" role="status">Checking…</p>{/if}

  {#if rows}
    <h2 class="ck-h">Every core on the card</h2>
    <ul class="ck-rows" aria-label="Core check for every core">{#each rows as r (r.id)}
      <li><button class="ck-row" on:click={() => checkOne(r.id)} aria-label={`${r.shortname || r.id}: ${plural(r.errors, 'error')}, ${plural(r.warnings, 'warning')}. Open`}>
        <strong>{r.shortname || r.id}</strong><small>{r.id}</small>
        <span class="ck-chips">{#if r.errors}<span class="chip bad">{plural(r.errors, 'error')}</span>{/if}{#if r.warnings}<span class="chip warn">{plural(r.warnings, 'warning')}</span>{/if}{#if r.notes}<span class="chip">{plural(r.notes, 'note')}</span>{/if}{#if !r.errors && !r.warnings && !r.notes}<span class="chip capable">Passes</span>{/if}</span></button></li>
    {/each}</ul>
    <p class="ck-note">{rows.filter((r) => r.errors).length ? `${plural(rows.filter((r) => r.errors).length, 'core')} with errors.` : 'No core has errors.'} Pick one to see the details.</p>
  {/if}

  {#if result}
    <article class="ck-res" aria-label={`Check result for ${result.title}`}>
      <header class="ck-head"><div><h2>{result.title}</h2><p class="ck-verdict" class:bad={result.lint.errors > 0}>{verdict}</p></div><button class="quiet" on:click={copy}>{copied ? 'Copied' : 'Copy findings'}</button></header>
      {#if result.lint.findings.length}<LintFindings lint={result.lint} />{:else if !result.symptoms.length}<p class="ck-ok">Nothing to fix: this core passes every check.</p>{/if}
      {#if isInstalled}
        <h3>Why it might not be in the Pocket’s menu</h3>
        {#if result.symptoms.length}
          <ul class="ck-sym" aria-label="Menu symptoms">{#each result.symptoms as f}<li class={f.severity.toLowerCase()}><span class="chip {f.severity === 'Error' ? 'bad' : 'warn'}">{f.severity}</span><div><span>{f.message}</span><small>{f.fix}</small></div></li>{/each}</ul>
        {:else}<p class="ck-ok">None of the usual causes was found. If it is still missing, restart the Pocket so it rebuilds its menu.</p>{/if}
      {/if}
    </article>
  {/if}
</section>
<style>
  .ck-src{display:flex;gap:10px;align-items:flex-end;flex-wrap:wrap;margin:0 0 12px}.ck-in{display:grid;gap:5px;flex:1;min-width:260px;font-size:12px;color:#a6b3b2}
  .ck-in input,.ck-in select{font:inherit;font-size:13px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:9px;padding:9px 12px;width:100%;box-sizing:border-box}
  .ck-err{color:#ff9b9b;font-size:12.5px}.ck-note{font-size:12.5px;color:#95a5a4}.ck-ok{font-size:13px;color:#c5f7ad;margin:8px 0}.ck-h{font-size:15px;margin:18px 0 8px}
  .ck-rows{list-style:none;margin:0;padding:0;border:1px solid #2c393a;border-radius:12px;background:#161f20}.ck-rows li{border-bottom:1px solid #2c393a}.ck-rows li:last-child{border:0}
  .ck-row{width:100%;display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1.4fr) auto;gap:12px;align-items:center;padding:11px 16px;background:transparent;color:inherit;text-align:left}.ck-row:hover{background:#1c282a}.ck-row:focus-visible{outline:2px solid #c1f0ad;outline-offset:-2px}
  .ck-row small{color:#95a5a4;font-size:12px;overflow:hidden;text-overflow:ellipsis}.ck-chips{display:flex;gap:6px}
  .ck-res{border:1px solid #2c393a;border-radius:12px;background:#161f20;padding:16px 18px;margin:16px 0}.ck-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px}.ck-head h2{margin:0;font-size:16px}
  .ck-verdict{margin:4px 0 0;font-size:13px;color:#c5f7ad}.ck-verdict.bad{color:#ffb4b4}.ck-res h3{margin:16px 0 6px;font-size:11px;letter-spacing:.09em;color:#9fb3aa;text-transform:uppercase}
  .ck-sym{list-style:none;margin:6px 0;padding:0;border:1px solid #2c393a;border-radius:9px}.ck-sym li{display:flex;gap:10px;align-items:flex-start;padding:9px 12px;border-bottom:1px solid #2c393a;font-size:12.5px}.ck-sym li:last-child{border:0}.ck-sym small{display:block;color:#95a5a4;font-size:11.5px;margin-top:2px}
  .chip.bad{background:#3a1c1c;color:#ffb4b4}.chip.warn{background:#3a2e14;color:#e5d9a8}
</style>
