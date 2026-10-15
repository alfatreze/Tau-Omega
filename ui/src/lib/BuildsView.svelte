<script lang="ts">
  // Developer tools: the Builds page. A folder of test builds (zips and package folders), grouped into series by the developer's naming schemes, compared
  // with the open card. Installing opens the same reviewed plan as every other install (and the Real-card confirmation); removing older builds of a
  // series goes through the same reviewed removal, one core at a time, each backed up.
  import { onMount } from 'svelte';
  import { open as pick } from '@tauri-apps/plugin-dialog';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { buildsConfig, buildsList, buildZip, executeRemoveCore, planRemoveCore, setBuildsConfig, trySchemeOn } from './tau-api';
  import { plural, size } from './coresFormat';
  import UpdatePlanView from './UpdatePlanView.svelte';
  import CarryMedia from './CarryMedia.svelte';
  import { roleOf, refreshRole, cardLabel } from './devStore';
  import type { BuildList, BuildRow, BuildScheme, BuildsConfig, RemovePlan, SchemeField, SchemeTry } from './types';
  export let card = '';

  let cfg: BuildsConfig | null = null; let dirInput = ''; let list: BuildList | null = null; let error = ''; let note = ''; let busy = false;
  let custom: BuildScheme[] = []; let schemeNote = ''; let schemeOpen = false;
  const FIELDS: [SchemeField, string][] = [['shortname', 'Shortname'], ['version', 'Version'], ['core_id', 'Core id'], ['file', 'File name']];
  const fieldLabel = (f: SchemeField) => FIELDS.find(([v]) => v === f)?.[1] ?? f;

  onMount(async () => { try { cfg = await buildsConfig(); dirInput = cfg.dir ?? ''; custom = cfg.custom.map((s) => ({ ...s })); if (cfg.dir) await rescan(); } catch (e) { error = errorMessage(e); } });
  $: if (card) void refreshRole(card);
  $: isTest = $roleOf.card === card && $roleOf.role === 'test';

  async function chooseFolder() { const r = await pick({ directory: true, title: 'Choose the folder your builds are in' }); if (typeof r === 'string') { dirInput = r; await useFolder(); } }
  async function useFolder() {
    error = ''; note = '';
    try { cfg = await setBuildsConfig(dirInput.trim() || null, custom); await rescan(); } catch (e) { error = errorMessage(e); }
  }
  async function rescan() {
    busy = true; error = '';
    try { list = await buildsList(card); } catch (e) { list = null; error = errorMessage(e); } finally { busy = false; }
  }
  // series groups, in the order the engine sorted them
  $: groups = (() => {
    const out: { key: string; label: string; rows: BuildRow[] }[] = [];
    for (const r of list?.rows ?? []) {
      const key = r.entry.error ? '!' : r.entry.placement?.series ?? '';
      let g = out.find((x) => x.key === key);
      if (!g) { g = { key, label: key === '!' ? 'Could not be read' : key ? key : 'Not in a series', rows: [] }; out.push(g); }
      g.rows.push(r);
    }
    return out;
  })();

  // ---- naming schemes
  let tryText: Record<number, string> = {}; let tries: Record<number, SchemeTry | null> = {};
  async function retry(i: number) {
    const s = custom[i]; const t = tryText[i] ?? '';
    if (!s || !t.trim()) { tries[i] = null; tries = tries; return; }
    try { tries[i] = await trySchemeOn(s.field, s.template, `alfatreze.${t}`, t, t, t); } catch { tries[i] = null; }
    tries = tries;
  }
  function addScheme() { custom = [...custom, { name: 'My scheme', field: 'shortname', template: '{series} {n}' }]; }
  function dropScheme(i: number) { custom = custom.filter((_, j) => j !== i); tryText = {}; tries = {}; }
  async function saveSchemes() {
    schemeNote = '';
    try { cfg = await setBuildsConfig(cfg?.dir ?? null, custom); custom = cfg.custom.map((s) => ({ ...s })); schemeNote = 'Saved.'; if (cfg.dir) await rescan(); }
    catch (e) { schemeNote = errorMessage(e); }
  }
  const sample = (p: SchemeTry | null) => (p === null ? '' : p.error ? p.error : p.placement ? `Series “${p.placement.series}”, number ${p.placement.order[3]}${p.placement.order[0] || p.placement.order[1] || p.placement.order[2] ? `, version ${p.placement.order[0]}.${p.placement.order[1]}.${p.placement.order[2]}` : ''}${p.placement.pre ? `, pre-release ${p.placement.pre}` : ''}` : 'No match: this name is not part of a series.');

  // ---- install
  let installing: { row: BuildRow; zip: string } | null = null; let installError = ''; let installedNow = false;
  async function install(r: BuildRow) {
    installError = ''; installedNow = false;
    try { installing = { row: r, zip: await buildZip(r.entry.path) }; } catch (e) { installError = errorMessage(e); }
  }
  // ---- remove older builds
  let removing: { row: BuildRow; plans: RemovePlan[] } | null = null; let removeDone = ''; let removeError = ''; let removeBusy = false;
  async function review(r: BuildRow) {
    removeError = ''; removeDone = ''; removeBusy = true;
    try { removing = { row: r, plans: await Promise.all(r.older.map((c) => planRemoveCore(card, c.id, true))) }; } catch (e) { removeError = errorMessage(e); } finally { removeBusy = false; }
  }
  async function removeOlder() {
    if (!removing) return; removeBusy = true; removeError = '';
    let gone = 0;
    try { for (const p of removing.plans) { await executeRemoveCore(card, p.core_id, p.id, true); gone++; } removeDone = `${plural(gone, 'older build')} removed. Their music, library and settings were kept.`; removing = null; await rescan(); }
    catch (e) { removeError = `${gone ? `${plural(gone, 'build')} removed, then it stopped. ` : 'Nothing was removed. '}${errorMessage(e)}`; }
    finally { removeBusy = false; }
  }
  const REL: Record<string, string> = { not_installed: 'Not on the card', same: 'On the card', newer: 'Newer than the card', older: 'Older than the card', different: 'Different from the card' };
  const when = (t: number) => (t ? new Date(t * 1000).toLocaleDateString() : '');
</script>
<section class="page builds" aria-labelledby="builds-title">
  <header><div><p class="eyebrow">DEVELOPER</p><h1 id="builds-title">Builds</h1><p class="lede">Your test builds, grouped into series by your own naming, compared with the open card. Nothing is written until you review and confirm an install.</p></div></header>

  <div class="b-folder">
    <label class="b-in"><span>Build folder</span><input bind:value={dirInput} placeholder="/path/to/your/builds" on:keydown={(e) => e.key === 'Enter' && useFolder()} /></label>
    <button class="quiet" on:click={chooseFolder}>Choose…</button><button class="primary" disabled={!dirInput.trim() || busy} on:click={useFolder}>Use this folder</button>
    {#if list}<button class="quiet" disabled={busy} on:click={rescan}>Rescan</button>{/if}
  </div>
  <p class="b-card" role="status">{card ? `Open card: ${cardLabel(card)} · ${isTest ? 'Test card' : 'Real card'}${isTest ? '' : ': installing will ask you to type its name'}` : 'Open a card to compare and install.'}</p>
  {#if error}<p class="b-err" role="alert">{error}</p>{/if}
  {#if installError}<p class="b-err" role="alert">{installError}</p>{/if}
  {#if removeDone}<p class="b-ok" role="status">{removeDone}</p>{/if}

  <details class="b-schemes" bind:open={schemeOpen}>
    <summary>Naming schemes <small>{custom.length ? `${plural(custom.length, 'of yours')} and ${cfg?.builtin.length ?? 0} built in` : `${cfg?.builtin.length ?? 0} built in`}</small></summary>
    <p class="b-help">A scheme reads one field of a build with a template. <code>{'{series}'}</code> names the series, <code>{'{n}'}</code> is a counter, <code>{'{major}'}</code> <code>{'{minor}'}</code> <code>{'{patch}'}</code> and <code>{'{pre}'}</code> are SemVer, <code>{'{any}'}</code> is ignored; everything else is literal text. Yours are tried first, then the built-in ones; the first that matches decides the series.</p>
    {#each custom as s, i}
      <div class="b-scheme" role="group" aria-label={`Scheme ${s.name}`}>
        <label>Name<input bind:value={s.name} /></label>
        <label>Reads<select bind:value={s.field} on:change={() => retry(i)}>{#each FIELDS as [v, l]}<option value={v}>{l}</option>{/each}</select></label>
        <label>Template<input bind:value={s.template} on:input={() => retry(i)} aria-label={`Template for ${s.name}`} /></label>
        <label>Try a {fieldLabel(s.field).toLowerCase()}<input bind:value={tryText[i]} on:input={() => retry(i)} placeholder="TAU DEV 12" /></label>
        <button class="quiet" on:click={() => dropScheme(i)} aria-label={`Remove scheme ${s.name}`}>Remove</button>
        {#if tries[i]}<p class="b-try" class:bad={!!tries[i]?.error} role="status">{sample(tries[i])}</p>{/if}
      </div>
    {/each}
    <div class="b-sbar"><button class="quiet" on:click={addScheme}>Add a scheme</button><button class="primary" on:click={saveSchemes}>Save schemes</button>{#if schemeNote}<span role="status" class="b-snote">{schemeNote}</span>{/if}</div>
    <h3>Built in</h3>
    <ul class="b-builtin" aria-label="Built-in schemes">{#each cfg?.builtin ?? [] as s}<li><strong>{s.name}</strong><small>reads {fieldLabel(s.field).toLowerCase()} · <code>{s.template}</code></small></li>{/each}</ul>
  </details>

  {#if !cfg?.dir && !list}<div class="empty"><h2>Choose your build folder</h2><p>Point Omega at the folder your builds land in: zips and package folders are listed, never changed.</p></div>
  {:else if list && !list.rows.length}<div class="empty"><h2>No builds in this folder</h2><p>Omega lists core zips and package folders one level down.</p></div>
  {:else}
    {#each groups as g (g.key)}
      <section class="b-group" aria-labelledby={`bg-${g.key.replace(/\W+/g, '-')}`}>
        <h2 id={`bg-${g.key.replace(/\W+/g, '-')}`}>{g.label} <small>{plural(g.rows.length, 'build')}</small></h2>
        <ul class="b-list">
          {#each g.rows as r (r.entry.path)}
            <li class="b-row">
              <div class="b-main"><strong>{r.entry.shortname || r.entry.file}</strong>
                <small>{r.entry.error ? r.entry.error : `${r.entry.core_id} · version ${r.entry.version || 'not stated'}${r.entry.date_release ? ' · ' + r.entry.date_release : ''} · ${size(r.entry.bytes)} · ${r.entry.file}`}</small></div>
              <div class="b-chips">
                {#if !r.entry.error}<span class="chip" class:capable={r.relation === 'same'} class:warn={r.relation === 'older' || r.relation === 'different'}>{REL[r.relation]}</span>{/if}
                {#if r.entry.lint_errors}<span class="chip bad" title="Core check errors">{plural(r.entry.lint_errors, 'error')}</span>{/if}
                {#if r.entry.lint_warnings}<span class="chip warn" title="Core check warnings">{plural(r.entry.lint_warnings, 'warning')}</span>{/if}
                {#if r.entry.placement}<span class="chip" title={`Matched by: ${r.entry.placement.scheme}`}>{r.entry.placement.pre ? `${r.entry.placement.order.slice(0, 3).join('.')}-${r.entry.placement.pre}` : r.entry.placement.order[3] ? `#${r.entry.placement.order[3]}` : r.entry.placement.order.slice(0, 3).join('.')}</span>{/if}
              </div>
              <div class="b-act">
                {#if !r.entry.error}<button class="primary" disabled={!card} aria-label={`Install ${r.entry.file} to the card`} on:click={() => install(r)}>{r.relation === 'same' ? 'Reinstall' : 'Install to the card'}</button>{/if}
                {#if r.older.length}<button class="quiet" disabled={!card || removeBusy} aria-label={`Remove ${plural(r.older.length, 'older build')} of ${r.entry.shortname}`} on:click={() => review(r)}>Remove {r.older.length} older…</button>{/if}
              </div>
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  {/if}
</section>

{#if installing}
  <div class="b-backdrop" role="presentation" on:click={() => (installing = null)}></div>
  <div class="b-sheet" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="bs-title" use:modal on:keydown={(e) => e.key === 'Escape' && (installing = null)}>
    <header class="b-shead"><div><p class="eyebrow">INSTALL A BUILD</p><h2 id="bs-title">{installing.row.entry.shortname || installing.row.entry.file}</h2></div><button class="quiet" data-autofocus on:click={() => { installing = null; void rescan(); }}>Close</button></header>
    <UpdatePlanView zipPath={installing.zip} cardPath={card} on:installed={() => { installedNow = true; void rescan(); }} />
    {#if installedNow && installing.row.older.length}{#key installing.row.entry.path}<CarryMedia {card} newId={installing.row.entry.core_id} newName={installing.row.entry.shortname} older={installing.row.older} />{/key}{/if}
  </div>
{/if}
{#if removing}
  <div class="b-backdrop" role="presentation" on:click={() => !removeBusy && (removing = null)}></div>
  <div class="b-sheet narrow" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="br-title" use:modal on:keydown={(e) => e.key === 'Escape' && !removeBusy && (removing = null)}>
    <header class="b-shead"><div><p class="eyebrow">REMOVE OLDER BUILDS</p><h2 id="br-title">{plural(removing.plans.length, 'older build')} of {removing.row.entry.shortname}</h2></div><button class="quiet" data-autofocus disabled={removeBusy} on:click={() => (removing = null)}>Cancel</button></header>
    <p class="b-help">These cores on the card belong to the same series and are older than {removing.row.entry.shortname}. Each one’s own files are removed; its music, library and settings are kept. Every file is copied to a backup folder first.</p>
    <ul class="b-rm" aria-label="Builds to remove">{#each removing.plans as p}<li><strong>{p.core_id}</strong><small>{plural(p.files_to_remove, 'file')} · {size(p.bytes_to_remove)}</small></li>{/each}</ul>
    {#if removeError}<p class="b-err" role="alert">{removeError}</p>{/if}
    <div class="b-actions"><button class="danger" disabled={removeBusy} on:click={removeOlder}>{removeBusy ? 'Removing…' : `Remove ${plural(removing.plans.length, 'build')}`}</button></div>
  </div>
{/if}
<style>
  .b-folder{display:flex;gap:10px;align-items:flex-end;flex-wrap:wrap;margin:0 0 8px}.b-in{display:grid;gap:5px;flex:1;min-width:260px;font-size:12px;color:#a6b3b2}
  .b-in input,.b-scheme input,.b-scheme select{font:inherit;font-size:13px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:9px;padding:9px 12px;width:100%;box-sizing:border-box}
  .b-card{font-size:12.5px;color:#a6b3b2;margin:4px 0 12px}.b-err{color:#ff9b9b;font-size:12.5px}.b-ok{color:#c5f7ad;font-size:13px}
  .b-schemes{border:1px solid #2c393a;border-radius:12px;background:#161f20;padding:12px 16px;margin:0 0 18px}.b-schemes summary{cursor:pointer;font-size:14px;font-weight:600}.b-schemes summary small{font-weight:400;color:#8f9e9d;margin-left:8px}
  .b-help{font-size:12.5px;color:#a6b3b2;line-height:1.6;margin:10px 0}.b-help code,.b-builtin code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
  .b-scheme{display:grid;grid-template-columns:1.1fr .8fr 1.2fr 1.2fr auto;gap:10px;align-items:end;border-top:1px solid #2c393a;padding:12px 0;font-size:12px;color:#a6b3b2}.b-scheme label{display:grid;gap:5px}
  .b-try{grid-column:1/-1;margin:0;font-size:12.5px;color:#c5f7ad}.b-try.bad{color:#ff9b9b}
  .b-sbar{display:flex;gap:10px;align-items:center;margin:8px 0}.b-snote{font-size:12.5px;color:#c7d1d0}
  .b-schemes h3{margin:14px 0 6px;font-size:11px;letter-spacing:.09em;color:#9fb3aa;text-transform:uppercase}.b-builtin{list-style:none;margin:0;padding:0;display:grid;gap:6px}.b-builtin li{display:flex;gap:12px;align-items:baseline;font-size:13px}.b-builtin small{color:#8f9e9d;font-size:12px}
  .b-group{margin:0 0 22px}.b-group h2{font-size:15px;margin:0 0 8px;text-transform:none}.b-group h2 small{font-size:12px;font-weight:400;color:#8f9e9d;margin-left:6px}
  .b-list{list-style:none;margin:0;padding:0;border:1px solid #2c393a;border-radius:12px;background:#161f20}
  .b-row{display:grid;grid-template-columns:minmax(0,1.6fr) auto auto;gap:14px;align-items:center;padding:12px 16px;border-bottom:1px solid #2c393a}.b-row:last-child{border:0}
  .b-main{min-width:0;display:grid;gap:3px}.b-main strong{font-size:14px}.b-main small{font-size:12px;color:#95a5a4;overflow:hidden;text-overflow:ellipsis}
  .b-chips{display:flex;gap:6px;flex-wrap:wrap;justify-content:flex-end}.b-act{display:flex;gap:8px;flex-wrap:wrap;justify-content:flex-end}
  .chip.warn{background:#3a2e14;color:#e5d9a8}.chip.bad{background:#3a1c1c;color:#ffb4b4}
  .b-backdrop{position:fixed;inset:0;background:rgba(0,0,0,.6);z-index:100}
  .b-sheet{position:fixed;z-index:101;top:50%;left:50%;transform:translate(-50%,-50%);width:min(820px,calc(100vw - 32px));max-height:calc(100vh - 40px);overflow:auto;background:#1a2325;border:1px solid #2c393a;border-radius:14px;padding:22px 24px;box-sizing:border-box}.b-sheet.narrow{width:min(620px,calc(100vw - 32px))}
  .b-shead{display:flex;justify-content:space-between;align-items:flex-start;gap:12px;margin-bottom:12px}.b-shead h2{margin:0}
  .b-rm{list-style:none;margin:10px 0;padding:0;border:1px solid #2c393a;border-radius:9px}.b-rm li{display:flex;justify-content:space-between;gap:10px;padding:9px 12px;border-bottom:1px solid #2c393a;font-size:13px}.b-rm li:last-child{border:0}.b-rm small{color:#8f9e9d}
  .b-actions{display:flex;justify-content:flex-end}.b-actions .danger{background:#ff9b9b;color:#2a1212;font-weight:700;border-radius:9px;padding:10px 14px}
  @media(max-width:900px){.b-scheme{grid-template-columns:1fr 1fr}.b-row{grid-template-columns:1fr}.b-chips,.b-act{justify-content:flex-start}}
</style>
