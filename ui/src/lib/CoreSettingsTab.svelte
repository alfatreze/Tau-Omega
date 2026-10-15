<script lang="ts">
  // Settings of one core: what it still needs (missing BIOS), the settings the Pocket saved (decoded with the core's own menu), the saved
  // controls and video files, the per-game presets, and what the core reads (its data slots). Read-only.
  import { onMount } from 'svelte';
  import { errorMessage } from './backend';
  import { coreSettings } from './tau-api';
  import { day, plural, size } from './coresFormat';
  import type { CoreDetail, CoreFiles, CoreSettings } from './types';
  export let card: string; export let detail: CoreDetail; export let files: CoreFiles;
  /** The missing-file rows can open the Files tab's picker. */
  export let addMissing: (folderRel: string) => void;

  let st: CoreSettings | null = null; let error = ''; let loading = true;
  onMount(async () => {
    try { st = await coreSettings(card, detail.id); } catch (e) { error = errorMessage(e); } finally { loading = false; }
  });
  const GROUP: Record<string, string> = { Interact: 'Core menu', Input: 'Controls', Video: 'Video' };
  const where = (s: { core_specific: boolean; platform: string }) => (s.core_specific ? `Only this core (Assets/${s.platform}/${detail.id})` : `Shared (Assets/${s.platform}/common)`);
</script>
<div class="cg">
  <section class="cg-sec" aria-labelledby="cg-need">
    <h3 id="cg-need">Needs attention</h3>
    {#if files.missing.length}
      <ul class="cg-list">{#each files.missing as m}
        <li class="warn"><span><strong>{m.filename}</strong> <small>{m.slot}{m.size_exact ? `, exactly ${size(m.size_exact)}` : ''}: put it in <code>{m.folder_rel}</code></small></span><button class="quiet" on:click={() => addMissing(m.folder_rel)}>Add {m.filename}…</button></li>
      {/each}</ul>
    {:else}<p class="cg-ok">Nothing is missing. Every required file with a fixed name is on the card.</p>{/if}
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
<style>
  .cg{display:grid;gap:16px}.cg-sec{border:1px solid #2c393a;border-radius:12px;background:#161f20;padding:14px 16px}.cg-sec h3{margin:0 0 8px;font-size:15px}.cg-sec h4{margin:14px 0 6px;font-size:12px;color:#9fb3aa;letter-spacing:.06em}
  .cg-list{list-style:none;margin:0;padding:0;border:1px solid #2c393a;border-radius:9px;background:#141c1d}
  .cg-list li{display:flex;align-items:center;gap:10px;padding:9px 12px;border-bottom:1px solid #222d2f;font-size:12.5px}.cg-list li:last-child{border:0}.cg-list li.warn{background:#241f12;color:#e5d9a8}
  .cg-k{width:190px;flex:none;color:#a6b3b2}.cg-v{flex:1;min-width:0;color:#e8ecec;overflow-wrap:anywhere}.cg-v small,.cg-list small{color:#8f9e9d;font-size:11.5px}.cg-d{color:#8f9e9d;font-size:11.5px}
  .cg-def{margin-left:6px}
  .cg-note{font-size:12px;color:#95a5a4;margin:8px 0 0;line-height:1.55}.cg-ok{font-size:13px;color:#c5f7ad;margin:0}.cg-err{color:#ff9b9b;font-size:12.5px}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
  @media(max-width:720px){.cg-k{width:110px}}
</style>
