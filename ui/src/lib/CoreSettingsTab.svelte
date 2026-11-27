<script lang="ts">
  // Settings of one core: what it still needs (missing BIOS), the settings the Pocket saved (decoded with the core's own menu), the saved
  // controls and video files, the per-game presets, and what the core reads (its data slots). Read-only.
  import { onMount } from 'svelte';
  import { errorMessage } from './backend';
  import { coreSettings } from './tau-api';
  import { day, plural, size } from './coresFormat';
  import RemoveFilesSheet from './RemoveFilesSheet.svelte';
  import Icon from './Icon.svelte';
  import { GROUP_ICON, fileIcon } from './iconFor';
  import type { CoreDetail, CoreFiles, CoreSettings } from './types';
  export let card: string; export let detail: CoreDetail; export let files: CoreFiles;
  /** The missing-file rows can open the Files tab's picker. */
  export let addMissing: (folderRel: string) => void;
  /** Called after files were removed so the page reloads the listing. */
  export let changed: () => void = () => {};
  export let isDisabled = false;
  export let stateBusy = false;
  export let confirmEnable = false;
  export let setVisible: (on: boolean) => void = () => {};
  export let cancelEnable: () => void = () => {};

  // The necessary files of the core: BIOS and configuration files (what the Files tab leaves out). Each row is a card-relative path.
  $: support = files.folders.filter((f) => f.folder.role === 'AssetsShared' || f.folder.role === 'AssetsCore')
    .flatMap((f) => f.entries.filter((e) => e.support).map((e) => ({ rel: `${f.folder.rel_path}/${e.rel_path}`, name: e.rel_path, slot: e.slot, bytes: e.bytes, modified: e.modified, shared: f.folder.role === 'AssetsShared', group: e.group || 'Other files', purpose: e.purpose, source: e.purpose_source })));
  const ORDER = ['BIOS and system', 'Palettes', 'Disks', 'Memory and saves', 'Game setup', 'Game data', 'Other files'];
  $: supportGroups = ORDER.map((g) => ({ name: g, items: support.filter((s) => s.group === g) })).concat(support.filter((s) => !ORDER.includes(s.group)).length ? [{ name: 'Other files', items: support.filter((s) => !ORDER.includes(s.group)) }] : []).filter((g) => g.items.length);
  const SOURCE: Record<string, string> = { core: "from the core's file list", known: 'known file', guess: 'a guess' };
  let picked: string[] = []; let removing: string[] | null = null;
  const toggle = (rel: string) => (picked = picked.includes(rel) ? picked.filter((r) => r !== rel) : [...picked, rel]);
  $: picked = picked.filter((r) => support.some((s) => s.rel === r));

  let st: CoreSettings | null = null; let error = ''; let loading = true;
  onMount(async () => {
    try { st = await coreSettings(card, detail.id); } catch (e) { error = errorMessage(e); } finally { loading = false; }
  });
  const GROUP: Record<string, string> = { Interact: 'Core menu', Input: 'Controls', Video: 'Video' };
  const where = (s: { core_specific: boolean; platform: string }) => (s.core_specific ? `Only this core (Assets/${s.platform}/${detail.id})` : `Shared (Assets/${s.platform}/common)`);
</script>
<div class="cg">
  <section class="cg-sec cg-vis-sec" aria-labelledby="cg-vis">
    <div class="cg-vis-row">
      <div>
        <h3 id="cg-vis">Pocket visibility</h3>
        <p class="cg-note">Turn off to hide this core from the Pocket’s menu. Its files stay on the card so you can fix or enable it again anytime.</p>
      </div>
      <button
        type="button"
        class="cg-vis"
        class:off={isDisabled}
        role="switch"
        aria-checked={!isDisabled}
        disabled={stateBusy}
        on:click={() => setVisible(isDisabled)}
      >
        <span class="cg-vis-knob" aria-hidden="true"></span>
        <span>Visible on the Pocket</span>
      </button>
    </div>
    {#if confirmEnable && files.missing.length}
      <p class="cg-warn" role="alert">
        It still lacks {files.missing.map((m) => m.filename).join(', ')}, so it may not start. Enable it anyway?
        <button type="button" class="quiet" on:click={() => setVisible(true)}>Enable anyway</button>
        <button type="button" class="quiet" on:click={cancelEnable}>Cancel</button>
      </p>
    {/if}
  </section>

  <section class="cg-sec" aria-labelledby="cg-need">
    <h3 id="cg-need">Needs attention</h3>
    {#if files.missing.length}
      <ul class="cg-list">{#each files.missing as m}
        <li class="warn"><span><strong>{m.filename}</strong> <small>{m.slot}{m.size_exact ? `, exactly ${size(m.size_exact)}` : ''}: put it in <code>{m.folder_rel}</code></small></span><button class="quiet" on:click={() => addMissing(m.folder_rel)}>Add {m.filename}…</button></li>
      {/each}</ul>
    {:else}<p class="cg-ok">Nothing is missing. Every required file with a fixed name is on the card.</p>{/if}
  </section>

  <section class="cg-sec" aria-labelledby="cg-support">
    <div class="cg-head"><h3 id="cg-support">Required and configuration files</h3>
      {#if picked.length}<button class="danger-quiet" on:click={() => (removing = picked)}>Remove {picked.length} selected…</button>{/if}</div>
    {#if support.length}
      {#each supportGroups as g (g.name)}
        <h4 class="cg-group"><span class="cg-gi"><Icon name={GROUP_ICON[g.name] ?? 'package'} size={16} /></span>{g.name} <small>{g.items.length}</small></h4>
        <ul class="cg-list cg-files" aria-label={g.name}>{#each g.items as s (s.rel)}
          <li><input type="checkbox" class="cg-pick" aria-label={`Select ${s.name}`} checked={picked.includes(s.rel)} on:change={() => toggle(s.rel)} />
            <span class="cg-fi" aria-hidden="true"><Icon name={fileIcon(s.name, s.group)} size={17} /></span>
            <span class="cg-fmain"><strong title={s.rel}>{s.name}</strong>{#if s.purpose}<small>{s.purpose}</small>{/if}
              <span class="cg-meta">{#if s.slot}<span class="chip">{s.slot}</span>{/if}<span class="chip" title="Where the explanation comes from">{SOURCE[s.source] ?? s.source}</span><span class="cg-where">{s.shared ? 'shared by every core of the platform' : 'only this core'}</span></span></span>
            <span class="cg-d">{size(s.bytes)} · {day(s.modified)}</span></li>
        {/each}</ul>
      {/each}
    {:else}<p class="cg-note">None on the card. Files a core needs in order to start (a BIOS, palettes, configuration) are listed here once they are added.</p>{/if}
  </section>

  <section class="cg-sec" aria-labelledby="cg-saved">
    <h3 id="cg-saved">Saved settings</h3>
    {#if loading}<p class="cg-note" role="status">Reading the saved settings…</p>
    {:else if error}<p class="cg-err" role="alert">{error}</p>
    {:else if st}
      {#if st.unreadable}<p class="cg-err" role="alert">{st.unreadable}.</p>{/if}
      {#if st.values.length}
        <ul class="cg-list" aria-label="Saved settings">{#each st.values as v (v.id)}
          <li><span class="cg-k">{v.name}</span><span class="cg-v">{v.display}{#if v.display !== v.default_display && v.default_display}<small class="cg-def">(default {v.default_display})</small>{/if}</span>{#if !v.known}<span class="chip" title="This core's menu no longer has this setting">unknown</span>{/if}</li>
        {/each}</ul>
      {:else}<p class="cg-note">Nothing saved yet. Change a setting in the core's menu on the Pocket and it appears here.</p>{/if}
      {#if st.files.length}
        <h4>Saved files</h4>
        <ul class="cg-list" aria-label="Saved settings files">{#each st.files as f (f.rel_path)}
          <li><span class="cg-k">{GROUP[f.group] ?? f.group}</span><span class="cg-v"><code>{f.rel_path.split('/').slice(-1)[0]}</code> · {size(f.bytes)}</span><span class="cg-d">{day(f.modified)}</span></li>
        {/each}</ul>
      {/if}
      <p class="cg-note">{st.presets ? `${plural(st.presets, 'per-game preset')} in ` : 'No per-game presets in '}<code>{st.presets_folder}</code>.</p>
    {/if}
  </section>

  <section class="cg-sec" aria-labelledby="cg-slots">
    <h3 id="cg-slots">What this core reads</h3>
    {#if detail.slots.length}
      <ul class="cg-list" aria-label="Data slots">{#each detail.slots as s}
        <li><span class="cg-k">{s.name}</span>
          <span class="cg-v">{s.filename ? s.filename : s.extensions.length ? s.extensions.map((e) => '.' + e).join(' ') : 'any file'} <small>{where(s)}{s.nonvolatile ? ' · saves' : ''}</small></span>
          {#if s.required}<span class="chip">required</span>{/if}</li>
      {/each}</ul>
    {:else}<p class="cg-note">This core does not declare any files.</p>{/if}
  </section>
</div>
{#if removing}
  <RemoveFilesSheet {card} coreId={detail.id} coreName={detail.shortname} files={removing} on:close={() => (removing = null)} on:done={() => { picked = []; changed(); }} />
{/if}
<style>
  .cg{display:grid;gap:16px}.cg-sec{background:transparent;border:0;border-radius:0;padding:0}.cg-sec + .cg-sec{padding-top:18px;border-top:1px solid var(--border-subtle)}.cg-sec h3{margin:0 0 8px;font-size:15px}.cg-sec h4{margin:14px 0 6px;font-size:12px;color:var(--text-secondary);letter-spacing:.06em}
  .cg-list{list-style:none;margin:0;padding:0;border:1px solid var(--border-subtle);border-radius:10px;background:var(--bg-card)}
  .cg-list li{display:flex;align-items:center;gap:10px;padding:9px 12px;border-bottom:1px solid var(--bg-hover);font-size:12.5px}.cg-list li:last-child{border:0}.cg-list li.warn{background:var(--warn-bg);color:var(--warn-text)}
  .cg-k{width:190px;flex:none;color:var(--text-secondary)}.cg-v{flex:1;min-width:0;color:var(--text);overflow-wrap:anywhere}.cg-v small,.cg-list small{color:var(--text-muted);font-size:11.5px}.cg-d{color:var(--text-muted);font-size:11.5px}
  .cg-head{display:flex;justify-content:space-between;align-items:center;gap:10px}.cg-head h3{margin:0 0 8px}.cg-pick{flex:none;accent-color:var(--accent);width:15px;height:15px}.danger-quiet{background:none;border:1px solid var(--danger-border);color:var(--danger-text);border-radius:8px;padding:6px 10px;font:inherit;font-size:12.5px;cursor:pointer}
  .cg-group{display:flex;align-items:center;gap:8px;margin:14px 0 6px;font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:var(--text-secondary)}.cg-group small{font-size:11px;color:var(--text-muted)}.cg-gi,.cg-fi{flex:none;display:inline-grid;place-items:center;border-radius:8px;background:var(--bg-card);color:var(--accent)}.cg-gi{width:26px;height:26px}.cg-fi{width:30px;height:30px}
  .cg-files li{align-items:flex-start}.cg-fmain{flex:1;min-width:0;display:grid;gap:3px}.cg-fmain strong{font-size:13px;font-weight:600;overflow-wrap:anywhere}.cg-fmain>small{font-size:12px;color:var(--text-secondary);line-height:1.5}.cg-meta{display:flex;gap:6px;flex-wrap:wrap;align-items:center;margin-top:2px}.cg-where{font-size:11.5px;color:var(--text-muted)}
  .cg-def{margin-left:6px}
  .cg-note{font-size:12px;color:var(--text-muted);margin:8px 0 0;line-height:1.55}.cg-ok{font-size:13px;color:var(--text-muted);margin:0}.cg-err{color:var(--danger-text);font-size:12.5px}
  .cg-vis-row{display:flex;justify-content:space-between;align-items:center;gap:14px;flex-wrap:wrap;padding:14px 16px;border:1px solid var(--border-subtle);border-radius:10px;background:var(--bg-card)}.cg-vis-row h3{margin:0}.cg-vis-row .cg-note{margin:4px 0 0}
  .cg-vis{display:inline-flex;align-items:center;gap:9px;padding:7px 12px 7px 9px;font-size:12.5px;color:var(--text);background:var(--bg-card);border:1px solid var(--border);border-radius:999px;flex:none}
  .cg-vis:hover{border-color:var(--border-strong)}.cg-vis:disabled{opacity:.55}
  .cg-vis.off{color:var(--text-secondary)}.cg-vis.off .cg-vis-knob{background:var(--bg-btn)}.cg-vis.off .cg-vis-knob::after{left:2px;background:var(--text-strong)}
  .cg-vis-knob{position:relative;width:28px;height:16px;border-radius:999px;background:var(--accent-solid);flex:none}.cg-vis-knob::after{content:'';position:absolute;top:2px;left:14px;width:12px;height:12px;border-radius:50%;background:var(--accent-on-solid)}
  .cg-warn{margin:10px 0 0;padding:10px 12px;border:1px solid var(--warn-border);background:var(--warn-bg);color:var(--warn-text);border-radius:9px;font-size:12.5px}.cg-warn button{margin-left:8px}
  code{font-size:11.5px;background:var(--bg-input);border-radius:5px;padding:1px 5px}
  @media(max-width:720px){.cg-k{width:110px}}
</style>
