<script lang="ts">
  // Saves and save states of one core, read from the card. Read-only: Pocket Sync (or a copy of the card) is the right tool to back them up;
  // its backup zips sit in Saves/ and are counted here, never listed as saves.
  import Icon from './Icon.svelte';
  import { day, plural, size } from './coresFormat';
  import type { CoreFiles, FileEntry, FolderListing } from './types';
  export let files: CoreFiles;

  $: saves = files.folders.filter((f) => f.folder.role === 'SavesShared' || f.folder.role === 'SavesCore');
  $: states = files.folders.find((f) => f.folder.role === 'SaveStates');
  const sorted = (l: FolderListing, byTaken: boolean): FileEntry[] => [...l.entries].sort((a, b) => (byTaken ? (b.taken ?? '').localeCompare(a.taken ?? '') : b.modified - a.modified));
  $: nothing = !saves.some((f) => f.entries.length) && !(states?.entries.length);
</script>
<div class="cs">
  {#if nothing}
    <div class="empty"><h2>No saves yet</h2><p>Play this core and save in the game (or make a save state from the Pocket's menu) and they will show up here.</p></div>
  {/if}
  {#each saves.filter((f) => f.entries.length) as f (f.folder.rel_path)}
    <section class="cs-sec" aria-labelledby={`cs-${f.folder.role}`}>
      <h3 id={`cs-${f.folder.role}`}>{f.folder.label}</h3>
      <p class="cs-sub"><code>{f.folder.rel_path}</code> · {plural(f.total_files, 'save')}, {size(f.total_bytes)}</p>
      <ul class="cs-list" aria-label={f.folder.label}>
        {#each sorted(f, false) as e (e.rel_path)}
          <li><span class="cs-fi" aria-hidden="true"><Icon name="memory" size={16} /></span><span class="cs-name" title={e.rel_path}>{e.game ?? e.name}</span>{#if e.slot}<span class="chip">{e.slot}</span>{/if}<span class="cs-size">{size(e.bytes)}</span><span class="cs-date">{day(e.modified)}</span></li>
        {/each}
      </ul>
    </section>
  {/each}
  {#if states?.entries.length}
    <section class="cs-sec" aria-labelledby="cs-states">
      <h3 id="cs-states">Save states</h3>
      <p class="cs-sub"><code>{states.folder.rel_path}</code> · {plural(states.total_files, 'save state')}, {size(states.total_bytes)}</p>
      <ul class="cs-list" aria-label="Save states">
        {#each sorted(states, true) as e (e.rel_path)}
          <li><span class="cs-fi" aria-hidden="true"><Icon name="clock-counter-clockwise" size={16} /></span><span class="cs-name" title={e.name}>{e.game ?? e.name}</span><span class="chip">Save state</span><span class="cs-size">{size(e.bytes)}</span><span class="cs-date">{e.taken ? e.taken.slice(0, 16) : day(e.modified)}</span></li>
        {/each}
      </ul>
    </section>
  {/if}
  {#if files.save_backups.length}
    <p class="cs-note" role="note">{plural(files.save_backups.length, 'Pocket Sync save backup')} ({size(files.save_backups.reduce((a, b) => a + b.bytes, 0))}) {files.save_backups.length === 1 ? 'is' : 'are'} in <code>Saves/</code>. They are backups of every core's saves together, so they are not listed here.</p>
  {/if}
</div>
<style>
  .cs{display:grid;gap:18px}
  .cs-sec{border:1px solid #2c393a;border-radius:12px;background:#161f20;padding:14px 16px}.cs-sec h3{margin:0 0 2px;font-size:15px}
  .cs-sub{margin:0 0 8px;font-size:12px;color:#95a5a4}
  .cs-list{list-style:none;margin:0;padding:0;border:1px solid #2c393a;border-radius:9px;background:#141c1d}
  .cs-list li{display:flex;align-items:center;gap:8px;padding:9px 12px;border-bottom:1px solid #222d2f;font-size:12.5px}.cs-list li:last-child{border:0}
  .cs-fi{flex:none;display:inline-grid;place-items:center;width:26px;height:26px;border-radius:8px;background:#1d2a2b;color:#c1f0ad}
  .cs-name{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:#e8ecec}.cs-size{width:70px;text-align:right;color:#a6b3b2}.cs-date{width:112px;text-align:right;color:#8f9e9d;font-size:11.5px}
  .cs-note{font-size:12px;color:#95a5a4;line-height:1.55}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
  @media(max-width:720px){.cs-date{display:none}}
</style>
