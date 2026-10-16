<script lang="ts">
  // Developer tools: the Builder. A raw Quartus bitstream plus a template core becomes an installable package zip, without any script. It never writes
  // to a card: the zip (and a record with every hash) goes to your build folder, where the Builds page lists it.
  import { onMount } from 'svelte';
  import { open as pick } from '@tauri-apps/plugin-dialog';
  import { errorMessage } from './backend';
  import { builderBuild, builderNextName, builderPlan, builderTemplateView, buildsConfig } from './tau-api';
  import TemplateEditor from './TemplateEditor.svelte';
  import { coresTreeStore } from './coresStore';
  import { disabledList } from './coreStateStore';
  import { plural, size } from './coresFormat';
  import LintFindings from './LintFindings.svelte';
  import type { BuildPlan, BuildResult, BuildSpec, ExtraFile, SchemeField, TemplateEdits, TemplateSource, TemplateViewData } from './types';
  export let card = ''; export let openBuilds: () => void = () => {};

  let rbf = ''; let kind: 'card' | 'zip' | 'folder' = 'card'; let coreId = ''; let templatePath = '';
  let shortname = ''; let version = ''; let description = ''; let date = new Date().toISOString().slice(0, 10);
  let icon = ''; let extra: ExtraFile[] = []; let outDir = ''; let buildsDir = '';
  const noEdits: TemplateEdits = { icon_fit: 'contain', author: null, url: null, firmware_required: null, dock_supported: null, link_port: null, sleep_supported: null, platform_ids: null, platform: null, banner: null };
  let view: TemplateViewData | null = null; let edits: TemplateEdits = { ...noEdits }; let editOpen = false; let plan: BuildPlan | null = null; let result: BuildResult | null = null; let error = ''; let busy = false; let note = '';

  onMount(async () => { try { const c = await buildsConfig(); buildsDir = c.dir ?? ''; outDir = buildsDir; } catch { /* the folder can be typed */ } });
  $: cores = [...$coresTreeStore.flatMap((n) => n.cores.map((c) => ({ id: c.id, label: `${c.shortname} (${c.id})` }))), ...$disabledList.map((d) => ({ id: d.id, label: `${d.shortname || d.id} (${d.id}, disabled)` }))];
  $: source = (kind === 'card' ? (coreId && card ? { kind: 'card', card, id: coreId } : null) : templatePath.trim() ? { kind, path: templatePath.trim() } : null) as TemplateSource | null;
  let lastSource = '';
  $: { const key = JSON.stringify(source); if (key !== lastSource) { lastSource = key; plan = null; result = null; void loadInfo(); } }
  async function loadInfo() {
    view = null; edits = { ...noEdits }; icon = ''; if (!source) return;
    try { view = await builderTemplateView(source); shortname = view.shortname; version = view.version; description = view.description; error = ''; }
    catch (e) { error = errorMessage(e); }
  }
  async function choose(what: 'rbf' | 'zip' | 'folder' | 'icon' | 'out' | { extra: number }) {
    const opts = what === 'rbf' ? { multiple: false, title: 'Choose the raw .rbf from Quartus', filters: [{ name: 'Bitstream', extensions: ['rbf', 'sof', 'rbf_r'] }] }
      : what === 'zip' ? { multiple: false, title: 'Choose a core package zip', filters: [{ name: 'Core zip', extensions: ['zip'] }] }
      : what === 'icon' ? { multiple: false, title: 'Choose a 36x36 PNG', filters: [{ name: 'PNG', extensions: ['png'] }] }
      : typeof what === 'object' ? { multiple: false, title: 'Choose a file to add' }
      : { directory: true, title: what === 'out' ? 'Choose the folder to build into' : 'Choose a package or core folder' };
    const r = await pick(opts); if (typeof r !== 'string') return;
    if (what === 'rbf') rbf = r; else if (what === 'zip' || what === 'folder') templatePath = r; else if (what === 'icon') icon = r; else if (what === 'out') outDir = r; else { extra[what.extra].src = r; extra = extra; }
    plan = null;
  }
  $: spec = { rbf: rbf.trim(), template: source ?? { kind: 'zip', path: '' }, shortname, version, date_release: date, description: description.trim() ? description.trim() : null, icon_png: icon.trim() || null, extra: extra.filter((e) => e.src.trim() && e.dest.trim()), edits } as BuildSpec;
  $: ready = !!rbf.trim() && !!source && !!shortname.trim() && !!version.trim() && /^\d{4}-\d{2}-\d{2}$/.test(date);
  async function check() { busy = true; error = ''; note = ''; result = null; try { plan = await builderPlan(spec); } catch (e) { plan = null; error = errorMessage(e); } finally { busy = false; } }
  async function build() { busy = true; error = ''; try { result = await builderBuild(spec, outDir.trim() || null); } catch (e) { error = errorMessage(e); } finally { busy = false; } }
  async function next(field: SchemeField) {
    note = '';
    try { const n = await builderNextName(field, field === 'version' ? version : shortname); if (!n) { note = `No naming scheme reads “${field === 'version' ? version : shortname}” as a counted name. Add one on the Builds page.`; return; } if (field === 'version') version = n.next; else shortname = n.next; plan = null; note = `Next name by “${n.scheme}”.`; }
    catch (e) { error = errorMessage(e); }
  }
  const ORIGIN: Record<string, string> = { bitstream: 'bitstream (converted)', template: 'from the template', generated: 'updated', icon: 'new icon', banner: 'new banner', extra: 'extra file' };
  $: id = plan?.core_id ?? '';
</script>
<section class="page builder" aria-labelledby="builder-title">
  <header><div><p class="eyebrow">DEVELOPER</p><h1 id="builder-title">Builder</h1><p class="lede">Turn a raw Quartus bitstream into an installable package: the bits are reversed once, the template’s files come along, the identity is set, and the core check must pass. Nothing is written to a card.</p></div></header>

  <h2 class="bd-h">1 · Bitstream</h2>
  <div class="bd-row"><label class="bd-in"><span>The raw .rbf Quartus wrote to output_files</span><input bind:value={rbf} placeholder="/path/to/output_files/core.rbf" on:input={() => (plan = null)} /></label><button class="quiet" on:click={() => choose('rbf')}>Choose…</button></div>

  <h2 class="bd-h">2 · Template core</h2>
  <p class="bd-help">The core whose files (core.json, data.json, interact.json and the rest) the new build starts from. Its own bitstream is not copied.</p>
  <div class="bd-kinds" role="radiogroup" aria-label="Template source">
    <label><input type="radio" bind:group={kind} value="card" /> An installed core</label><label><input type="radio" bind:group={kind} value="zip" /> A package zip</label><label><input type="radio" bind:group={kind} value="folder" /> A folder</label>
  </div>
  {#if kind === 'card'}
    <label class="bd-in"><span>Core on the open card</span><select bind:value={coreId} disabled={!card || !cores.length}><option value="">{card ? 'Choose a core…' : 'Open a card first'}</option>{#each cores as c}<option value={c.id}>{c.label}</option>{/each}</select></label>
  {:else}
    <div class="bd-row"><label class="bd-in"><span>{kind === 'zip' ? 'Package zip' : 'Package or core folder'}</span><input bind:value={templatePath} placeholder={kind === 'zip' ? '/path/to/core.zip' : '/path/to/folder'} /></label><button class="quiet" on:click={() => choose(kind === 'zip' ? 'zip' : 'folder')}>Choose…</button></div>
  {/if}
  {#if view}<p class="bd-info" role="status">Template: <strong>{view.shortname || view.core_id}</strong> · {view.core_id} · version {view.version || 'not stated'}{view.platform_ids.length ? ` · ${view.platform_ids.join(', ')}` : ''}</p>
    <button class="quiet" aria-expanded={editOpen} aria-controls="template-editor" on:click={() => (editOpen = !editOpen)}>{editOpen ? 'Hide the template editor' : 'View and edit this template…'}</button>
    {#if editOpen}<div id="template-editor"><TemplateEditor {view} {shortname} {version} bind:edits bind:icon changed={() => (plan = null)} /></div>{/if}{/if}

  <h2 class="bd-h">3 · This build</h2>
  <div class="bd-grid">
    <label class="bd-in"><span>Shortname <small>(also the core’s folder name)</small></span><div class="bd-with"><input aria-label="Shortname" bind:value={shortname} on:input={() => (plan = null)} /><button class="quiet" on:click={() => next('shortname')} aria-label="Use the next name">Next name</button></div></label>
    <label class="bd-in"><span>Version</span><div class="bd-with"><input aria-label="Version" bind:value={version} on:input={() => (plan = null)} /><button class="quiet" on:click={() => next('version')} aria-label="Use the next version">Next</button></div></label>
    <label class="bd-in"><span>Release date</span><input bind:value={date} placeholder="YYYY-MM-DD" on:input={() => (plan = null)} /></label>
    <label class="bd-in"><span>Description <small class:over={description.length > 63}>{description.length}/63</small></span><input bind:value={description} on:input={() => (plan = null)} /></label>
  </div>
  {#if note}<p class="bd-note" role="status">{note}</p>{/if}

  <h2 class="bd-h">4 · Optional</h2>
  <div class="bd-extra">
    <span class="bd-sub">Extra files <small>(a firmware ROM, data): <code>{'{id}'}</code> stands for the new core id</small></span>
    {#each extra as e, i}
      <div class="bd-erow" role="group" aria-label={`Extra file ${i + 1}`}>
        <input bind:value={e.src} placeholder="/path/to/file" aria-label={`Source of extra file ${i + 1}`} on:input={() => (plan = null)} /><button class="quiet" on:click={() => choose({ extra: i })} aria-label={`Choose extra file ${i + 1}`}>Choose…</button>
        <input bind:value={e.dest} placeholder={`Cores/{id}/firmware.bin or Assets/<platform>/{id}/boot.rom`} aria-label={`Destination of extra file ${i + 1}`} on:input={() => (plan = null)} />
        <button class="quiet" on:click={() => { extra = extra.filter((_, j) => j !== i); plan = null; }} aria-label={`Remove extra file ${i + 1}`}>Remove</button>
      </div>
    {/each}
    <button class="quiet" on:click={() => (extra = [...extra, { src: '', dest: 'Cores/{id}/' }])}>Add a file</button>
  </div>
  <div class="bd-row"><label class="bd-in"><span>Build into <small>{buildsDir && outDir === buildsDir ? '(your build folder)' : ''}</small></span><input bind:value={outDir} placeholder="the folder the Builds page reads" /></label><button class="quiet" on:click={() => choose('out')}>Choose…</button></div>

  <div class="bd-actions"><button class="primary" disabled={!ready || busy} on:click={check}>{busy && !plan ? 'Checking…' : 'Check the build'}</button>
    {#if plan && !plan.blocked}<button class="primary" disabled={busy || !!result} on:click={build}>{busy ? 'Building…' : 'Build the package'}</button>{/if}</div>
  {#if error}<p class="bd-err" role="alert">{error}</p>{/if}

  {#if plan}
    <article class="bd-plan" aria-label="Build plan">
      <h2>{id}</h2>
      <p class="bd-info">{plural(plan.files.length, 'file')} · bitstream {size(plan.bitstream_bytes)} · raw <code title={plan.raw_sha256}>{plan.raw_sha256.slice(0, 12)}…</code> → converted <code title={plan.converted_sha256}>{plan.converted_sha256.slice(0, 12)}…</code></p>
      <ul class="bd-files" aria-label="Files in the package">{#each plan.files as f}<li><span class="bd-fn">{f.rel}</span><span class="chip" class:capable={f.origin === 'bitstream'}>{ORIGIN[f.origin]}</span><small>{size(f.bytes)}</small></li>{/each}</ul>
      <h3>Core check</h3>
      {#if plan.lint.findings.length}<LintFindings lint={plan.lint} />{:else}<p class="bd-ok">Passes every check.</p>{/if}
      {#if plan.blocked}<p class="bd-err" role="alert">{plan.blocked} Fix it in the template or the fields above, then check again.</p>{/if}
    </article>
  {/if}
  {#if result}
    <article class="bd-done" role="status" aria-label="Build finished">
      <h2>Built {result.plan.core_id}</h2>
      <p>Package <code>{result.zip}</code> ({size(result.zip_bytes)})<br />SHA-256 <code title={result.zip_sha256}>{result.zip_sha256.slice(0, 16)}…</code><br />Build record <code>{result.record}</code> (every hash and input)</p>
      <button class="primary" on:click={openBuilds}>Show it in Builds</button>
    </article>
  {/if}
</section>
<style>
  .bd-h{font-size:14px;margin:22px 0 8px}.bd-help{font-size:12.5px;color:#95a5a4;margin:0 0 8px}.bd-note,.bd-info{font-size:12.5px;color:#a6b3b2;margin:8px 0}.bd-ok{color:#c5f7ad;font-size:13px}.bd-err{color:#ff9b9b;font-size:12.5px}
  .bd-row{display:flex;gap:10px;align-items:flex-end;flex-wrap:wrap;margin:0 0 10px}.bd-in{display:grid;gap:5px;flex:1;min-width:240px;font-size:12px;color:#a6b3b2}.bd-in small{color:#7f8e8d}.bd-in small.over{color:#ffb4b4}
  .bd-in input,.bd-in select,.bd-erow input{font:inherit;font-size:13px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:9px;padding:9px 12px;width:100%;box-sizing:border-box}
  .bd-kinds{display:flex;gap:18px;margin:0 0 10px;font-size:13px;color:#dce6e4}.bd-kinds label{display:inline-flex;gap:6px;align-items:center}
  .bd-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px}.bd-with{display:flex;gap:8px}.bd-with input{flex:1}
  .bd-extra{display:grid;gap:8px;margin:10px 0}.bd-sub{font-size:12px;color:#a6b3b2}.bd-sub small{color:#7f8e8d}.bd-erow{display:grid;grid-template-columns:1.2fr auto 1.4fr auto;gap:8px;align-items:center}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px;overflow-wrap:anywhere}
  .bd-actions{display:flex;gap:10px;margin:18px 0 8px}
  .bd-plan,.bd-done{border:1px solid #2c393a;border-radius:12px;background:#161f20;padding:16px 18px;margin:14px 0}.bd-plan h2,.bd-done h2{margin:0 0 6px;font-size:16px}.bd-plan h3{margin:14px 0 6px;font-size:11px;letter-spacing:.09em;color:#9fb3aa;text-transform:uppercase}
  .bd-files{list-style:none;margin:8px 0;padding:0;border:1px solid #2c393a;border-radius:9px}.bd-files li{display:grid;grid-template-columns:minmax(0,1fr) auto auto;gap:10px;align-items:center;padding:7px 12px;border-bottom:1px solid #2c393a;font-size:12.5px}.bd-files li:last-child{border:0}.bd-fn{overflow-wrap:anywhere}.bd-files small{color:#8f9e9d}
  .bd-done p{font-size:12.5px;line-height:1.7;color:#c7d1d0}
  @media(max-width:900px){.bd-grid{grid-template-columns:1fr}.bd-erow{grid-template-columns:1fr}}
</style>
