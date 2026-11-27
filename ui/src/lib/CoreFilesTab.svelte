<script lang="ts">
  // Files of one core: ROMs, BIOS, tapes, disks in the two asset folders, with a drop target per folder. Dropping or choosing files opens
  // the review sheet; nothing is written before it. Entries carry `kind` and `slot` now; a parser can add to them later.
  import { onDestroy } from 'svelte';
  import { open as pick } from '@tauri-apps/plugin-dialog';
  import AddFilesSheet from './AddFilesSheet.svelte';
  import RemoveFilesSheet from './RemoveFilesSheet.svelte';
  import GameArtworkPickerSheet from './GameArtworkPickerSheet.svelte';
  import Icon from './Icon.svelte';
  import { fileIcon } from './iconFor';
  import VirtualList from './VirtualList.svelte';
  import { onFileDrop, zoneAt } from './dropTarget';
  import { KIND_LABEL, day, plural, size } from './coresFormat';
  import { coreFilesViewMode, displayGameTitle, gameArtCacheKey, gameArtMap, lazyGameArt } from './gameArtStore';
  import type { AddDestination, CoreDetail, CoreFiles, FileEntry, FolderListing } from './types';
  export let card: string; export let detail: CoreDetail; export let files: CoreFiles;
  /** Called after files were added so the page reloads the listing. */
  export let changed: () => void;

  let dragging = false; let hot = ''; let sheet: { sources: string[]; destination: AddDestination; platform: string } | null = null;
  let removing: string[] | null = null; let picked: Record<string, string[]> = {};
  let artTarget: { platformId: string; platformName: string; romName: string } | null = null;
  const isGameEntry = (item: FileEntry) => item.kind === 'Rom' || item.kind === 'Disk' || item.kind === 'Tape';
  const isPicked = (z: string, rel: string) => (picked[z] ?? []).includes(rel);
  const toggle = (z: string, rel: string) => { const cur = picked[z] ?? []; picked = { ...picked, [z]: cur.includes(rel) ? cur.filter((r) => r !== rel) : [...cur, rel] }; };
  const relOf = (f: FolderListing, e: FileEntry) => `${f.folder.rel_path}/${e.rel_path}`;
  let query: Record<string, string> = {}; let sortBy: Record<string, 'name' | 'size' | 'date'> = {};

  // Games only: a core's BIOS and configuration files are listed under Settings. A core-only folder that holds nothing but those is not shown here.
  $: games = files.folders.map((f) => {
    const entries = f.entries.filter((e) => !e.support);
    return f.truncated ? { ...f, entries } : { ...f, entries, total_files: entries.length, total_bytes: entries.reduce((n, e) => n + e.bytes, 0) };
  });
  $: assets = games.filter((f) => f.folder.role === 'AssetsShared' || (f.folder.role === 'AssetsCore' && (f.entries.length || f.truncated)));
  const zoneId = (f: FolderListing) => `${f.folder.role}:${f.folder.platform}`;
  const dest = (f: FolderListing): AddDestination => (f.folder.role === 'AssetsShared' ? 'Shared' : 'CoreOnly');
  const platformName = (id: string) => detail.platforms.find((p) => p.id === id)?.name ?? id;

  function entries(f: FolderListing, q0: string | undefined, by: 'name' | 'size' | 'date' | undefined): FileEntry[] {
    const q = (q0 ?? '').trim().toLowerCase();
    const list = q ? f.entries.filter((e) => `${e.rel_path} ${e.slot ?? ''} ${KIND_LABEL[e.kind]}`.toLowerCase().includes(q)) : [...f.entries];
    return by === 'size' ? list.sort((a, b) => b.bytes - a.bytes) : by === 'date' ? list.sort((a, b) => b.modified - a.modified) : list;
  }
  function begin(paths: string[], zone: string) {
    const f = assets.find((a) => zoneId(a) === zone) ?? assets.find((a) => a.folder.role === 'AssetsShared') ?? assets[0];
    if (!f || !paths.length) return;
    sheet = { sources: paths, destination: dest(f), platform: f.folder.platform };
  }
  const off = onFileDrop((e) => {
    if (sheet) return;
    if (e.type === 'leave') { dragging = false; hot = ''; return; }
    dragging = true; hot = zoneAt(e.x, e.y);
    if (e.type === 'drop') { dragging = false; const zone = hot; hot = ''; begin(e.paths, zone); }
  });
  onDestroy(off);

  async function choose(f: FolderListing, folder: boolean) {
    const r = await pick({ multiple: !folder, directory: folder, title: folder ? 'Add a folder' : 'Add files' });
    const paths = r == null ? [] : Array.isArray(r) ? r : [r as string];
    begin(paths, zoneId(f));
  }
  /** The missing-file strip opens the picker aimed at the folder a file must be in. */
  export async function chooseFor(folderRel: string) {
    const f = assets.find((a) => a.folder.rel_path === folderRel) ?? assets[0];
    if (f) await choose(f, false);
  }
</script>
<div class="cf" class:dragging>
  {#if files.missing.length}
    <section class="cf-need" aria-labelledby="cf-need-t">
      <h3 id="cf-need-t">Needs attention</h3>
      <ul>{#each files.missing as m}
        <li><span><strong>{m.filename}</strong> <span class="cf-sub">({m.slot}{m.size_exact ? `, exactly ${size(m.size_exact)}` : ''}) is missing. Put it in <code>{m.folder_rel}</code>.</span></span>
          <button class="quiet" on:click={() => chooseFor(m.folder_rel)}>Add {m.filename}…</button></li>
      {/each}</ul>
    </section>
  {/if}

  {#each assets as f (zoneId(f))}
    {@const list = entries(f, query[zoneId(f)], sortBy[zoneId(f)])}
    <section class="cf-folder" class:hot={hot === zoneId(f)} data-drop-zone={zoneId(f)} aria-label={f.folder.label}>
      <header>
        <div><h3>{f.folder.label}</h3><p class="cf-path"><code>{f.folder.rel_path}</code> · {plural(f.total_files, 'file')}{f.total_bytes ? `, ${size(f.total_bytes)}` : ''}{f.exists ? '' : ' · not created yet'}</p></div>
        <span class="cf-actions">{#if (picked[zoneId(f)] ?? []).length}<button class="danger-quiet" on:click={() => (removing = (picked[zoneId(f)] ?? []).map((r) => `${f.folder.rel_path}/${r}`))}>Remove {(picked[zoneId(f)] ?? []).length} selected…</button>{/if}<button class="quiet" on:click={() => choose(f, false)}>Add files…</button><button class="quiet" on:click={() => choose(f, true)}>Add folder…</button></span>
      </header>
      {#if f.entries.length}
        <div class="cf-tools"><label><span class="sr">Search {f.folder.label}</span><input type="search" placeholder="Search these files" bind:value={query[zoneId(f)]} /></label>
          <label><span class="sr">Sort</span><select bind:value={sortBy[zoneId(f)]}><option value="name">Name</option><option value="size">Largest</option><option value="date">Newest</option></select></label>
          <div class="cf-view-toggle" role="group" aria-label="View mode">
            <button type="button" class:active={$coreFilesViewMode === 'list'} aria-pressed={$coreFilesViewMode === 'list'} on:click={() => coreFilesViewMode.set('list')}>List</button>
            <button type="button" class:active={$coreFilesViewMode === 'grid'} aria-pressed={$coreFilesViewMode === 'grid'} on:click={() => coreFilesViewMode.set('grid')}>Grid</button>
          </div></div>
        {#if list.length}
          {#if $coreFilesViewMode === 'grid'}
            <div class="cf-grid" role="list" aria-label={f.folder.label}>
              {#each list as item (item.rel_path)}
                {@const artPlat = isGameEntry(item) ? f.folder.platform : ''}
                {@const art = artPlat ? $gameArtMap[gameArtCacheKey(artPlat, item.name)] : null}
                <article class="cf-card" class:selected={isPicked(zoneId(f), item.rel_path)} role="listitem" use:lazyGameArt={{ platformId: artPlat, romName: item.name }}>
                  <div class="cf-cover">
                    {#if art?.data_url}
                      <img src={art.data_url} alt={displayGameTitle(item.name)} />
                    {:else}
                      <span class="cf-cover-fallback" aria-hidden="true"><Icon name={fileIcon(item.name, item.group, item.kind)} size={32} /></span>
                    {/if}
                    <input type="checkbox" class="cf-card-pick" aria-label={`Select ${item.rel_path}`} checked={isPicked(zoneId(f), item.rel_path)} on:change={() => toggle(zoneId(f), item.rel_path)} />
                    {#if isGameEntry(item)}
                      <button type="button" class="cf-card-art" aria-label={`Change artwork for ${item.name}`} title="Change artwork" on:click={() => (artTarget = { platformId: f.folder.platform, platformName: platformName(f.folder.platform), romName: item.name })}>Art</button>
                    {/if}
                  </div>
                  <div class="cf-card-meta">
                    <strong title={item.rel_path}>{displayGameTitle(item.name)}</strong>
                    <small>{size(item.bytes)} · {item.slot || KIND_LABEL[item.kind]}</small>
                  </div>
                </article>
              {/each}
            </div>
          {:else}
            <div class="cf-list" style={`height:${Math.min(list.length, 800) * 40}px`}>
              <VirtualList items={list} rowHeight={40} key={(e) => e.rel_path} label={f.folder.label} let:item>
                {@const artPlat = isGameEntry(item) ? f.folder.platform : ''}
                {@const art = artPlat ? $gameArtMap[gameArtCacheKey(artPlat, item.name)] : null}
                <div class="cf-row" role="listitem" use:lazyGameArt={{ platformId: artPlat, romName: item.name }}>
                  <input type="checkbox" class="cf-pick" aria-label={`Select ${item.rel_path}`} checked={isPicked(zoneId(f), item.rel_path)} on:change={() => toggle(zoneId(f), item.rel_path)} />
                  <span class="cf-fi" aria-hidden="true">{#if art?.data_url}<img class="cf-thumb" src={art.data_url} alt="" />{:else}<Icon name={fileIcon(item.name, item.group, item.kind)} size={17} />{/if}</span><span class="cf-name" title={item.purpose ? `${item.rel_path}: ${item.purpose}` : item.rel_path}>{item.rel_path}</span>
                  {#if isGameEntry(item)}
                    <button type="button" class="cf-art-btn" aria-label={`Change artwork for ${item.name}`} title="Change artwork" on:click={() => (artTarget = { platformId: f.folder.platform, platformName: platformName(f.folder.platform), romName: item.name })}>Art</button>
                  {/if}
                  {#if item.slot}<span class="chip">{item.slot}</span>{/if}
                  <span class="chip" class:capable={item.kind === 'Rom' || item.kind === 'Bios'}>{KIND_LABEL[item.kind]}</span>
                  <span class="cf-size">{size(item.bytes)}</span><span class="cf-date">{day(item.modified)}</span>
                </div>
              </VirtualList>
            </div>
          {/if}
          {#if f.truncated}<p class="cf-sub">Showing the first {f.entries.length} of {f.total_files} files. Search to narrow them.</p>{/if}
        {:else}<p class="cf-empty">Nothing matches “{query[zoneId(f)]}”.</p>{/if}
      {:else}
        <div class="cf-drop"><p><strong>{f.folder.role === 'AssetsShared' ? 'No files yet.' : 'Nothing only for this core.'}</strong> Drop ROMs, disks or tapes here{f.folder.role === 'AssetsShared' ? ` and every ${platformName(f.folder.platform)} core can use them` : ' and only this core will see them'}.</p></div>
      {/if}
      {#if dragging}<div class="cf-hint" aria-hidden="true">Drop to add to {f.folder.rel_path}</div>{/if}
    </section>
  {/each}
  {#if assets.some((f) => f.folder.role === 'AssetsShared' && f.entries.length)}<p class="cf-sub">Tip: drop files anywhere on this page to add them to the shared folder.</p>{/if}
</div>
{#if removing}
  <RemoveFilesSheet {card} coreId={detail.id} coreName={detail.shortname} files={removing} on:close={() => (removing = null)} on:done={() => { picked = {}; changed(); }} />
{/if}
{#if sheet}
  <AddFilesSheet {card} coreId={detail.id} platform={sheet.platform} platformName={platformName(sheet.platform)} coreName={detail.shortname} sources={sheet.sources} destination={sheet.destination}
    on:close={() => (sheet = null)} on:done={changed} />
{/if}
{#if artTarget}
  <GameArtworkPickerSheet
    platformId={artTarget.platformId}
    platformName={artTarget.platformName}
    romName={artTarget.romName}
    current={$gameArtMap[gameArtCacheKey(artTarget.platformId, artTarget.romName)] ?? null}
    close={() => (artTarget = null)}
  />
{/if}
<style>
  .cf{display:grid;gap:16px;position:relative}
  .cf-need{border:1px solid #5c5030;background:#241f12;border-radius:11px;padding:12px 14px}.cf-need h3{margin:0 0 6px;font-size:13px;color:#e5d9a8}
  .cf-need ul{list-style:none;margin:0;padding:0;display:grid;gap:6px}.cf-need li{display:flex;justify-content:space-between;gap:12px;align-items:center;font-size:12.5px;color:#e5d9a8}
  .cf-folder{position:relative;border:1px solid #2c393a;border-radius:12px;background:#161f20;padding:14px 16px}
  .cf-folder.hot{border-color:#c1f0ad;box-shadow:0 0 0 2px rgba(193,240,173,.25)}
  .cf-folder header{display:flex;justify-content:space-between;gap:12px;align-items:flex-start;margin-bottom:8px}.cf-folder h3{margin:0 0 2px;font-size:15px}
  .cf-path{margin:0;font-size:12px;color:#95a5a4}.cf-sub{font-size:12px;color:#8f9e9d;margin:6px 0}.cf-actions{display:flex;gap:2px;flex-wrap:wrap}
  .cf-tools{display:flex;gap:8px;margin:6px 0 8px;align-items:center}.cf-tools input,.cf-tools select{font:inherit;font-size:12.5px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:7px 10px}.cf-tools label:first-child{flex:1}.cf-tools input{width:100%;box-sizing:border-box}
  .cf-view-toggle{display:inline-flex;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:2px;gap:2px;flex:none}
  .cf-view-toggle button{background:transparent;color:#95a5a4;border-radius:6px;padding:4px 10px;font-size:12px}
  .cf-view-toggle button.active{background:#243335;color:#c1f0ad}
  .cf-list{display:flex;flex-direction:column;border:1px solid #2c393a;border-radius:9px;overflow:hidden;background:#141c1d}
  .cf-row{height:40px;box-sizing:border-box;display:flex;align-items:center;gap:8px;padding:0 12px;border-bottom:1px solid #222d2f;font-size:12.5px}
  .cf-pick{flex:none;accent-color:#c1f0ad;width:15px;height:15px}.danger-quiet{background:none;border:1px solid #6b3a3a;color:#ff9b9b;border-radius:8px;padding:6px 10px;font:inherit;font-size:12.5px;cursor:pointer}
  .cf-fi{flex:none;display:inline-grid;place-items:center;width:28px;height:28px;border-radius:8px;background:#1d2a2b;color:#c1f0ad;overflow:hidden}
  .cf-thumb{width:100%;height:100%;object-fit:cover;display:block}
  .cf-name{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:#e8ecec}.cf-size{width:70px;text-align:right;color:#a6b3b2;flex:none}.cf-date{width:78px;text-align:right;color:#8f9e9d;font-size:11.5px;flex:none}
  .cf-art-btn{flex:none;padding:2px 8px;font-size:11px;background:#1a2526;color:#9fb3aa;border:1px solid #304142;border-radius:6px;opacity:0;transition:opacity .12s ease}
  .cf-row:hover .cf-art-btn,.cf-art-btn:focus-visible{opacity:1;color:#c1f0ad;border-color:#c1f0ad}
  .cf-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(148px,1fr));gap:12px;max-height:620px;overflow-y:auto;padding:2px}
  .cf-card{background:#111819;border:1px solid #2a3738;border-radius:10px;padding:8px;display:grid;gap:7px;transition:border-color .14s ease}
  .cf-card.selected{border-color:#c1f0ad;background:#17251d}
  .cf-cover{position:relative;aspect-ratio:1/1;border-radius:7px;background:#0a0f10;overflow:hidden;display:grid;place-items:center;padding:4px;box-sizing:border-box}
  .cf-cover img{width:100%;height:100%;object-fit:contain;display:block}
  .cf-cover-fallback{color:#6d807f}
  .cf-card-pick{position:absolute;top:6px;left:6px;width:16px;height:16px;accent-color:#c1f0ad;cursor:pointer}
  .cf-card-art{position:absolute;top:6px;right:6px;padding:2px 8px;font-size:11px;background:rgba(16,22,23,.88);color:#eff5f4;border:1px solid #3c4d4e;border-radius:6px;opacity:0;transition:opacity .12s ease}
  .cf-card:hover .cf-card-art,.cf-card:focus-within .cf-card-art{opacity:1}
  .cf-card-art:hover{border-color:#c1f0ad;color:#c1f0ad}
  .cf-card-meta{display:grid;gap:2px;min-width:0}
  .cf-card-meta strong{font-size:12.5px;color:#eff5f4;display:-webkit-box;-webkit-line-clamp:2;line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;line-height:1.3}
  .cf-card-meta small{font-size:11px;color:#8f9e9d;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .cf-drop{border:1.5px dashed #3b4a4b;border-radius:10px;padding:22px 16px;text-align:center;color:#a6b3b2;font-size:13px}.cf-drop p{margin:0}
  .cf-empty{font-size:12.5px;color:#a6b3b2}
  .cf-hint{position:absolute;inset:0;display:grid;place-items:center;background:rgba(20,28,29,.82);border-radius:12px;font-size:14px;font-weight:700;color:#c1f0ad;pointer-events:none}
  .cf-folder.hot .cf-hint{background:rgba(35,64,41,.9)}
  .sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
  @media(max-width:720px){.cf-date{display:none}}
</style>
