<script lang="ts">
  // Files of one core: ROMs, BIOS, tapes, disks in the two asset folders, with List/Grid views,
  // category filtering, opt-in multiselect, click-to-inspect game details, and drag-and-drop.
  import { onDestroy } from 'svelte';
  import { open as pick } from '@tauri-apps/plugin-dialog';
  import AddFilesSheet from './AddFilesSheet.svelte';
  import GameArtworkPickerSheet from './GameArtworkPickerSheet.svelte';
  import Icon from './Icon.svelte';
  import RemoveFilesSheet from './RemoveFilesSheet.svelte';
  import VirtualList from './VirtualList.svelte';
  import { KIND_LABEL, day, plural, size } from './coresFormat';
  import { onFileDrop, zoneAt } from './dropTarget';
  import {
    coreFilesViewMode,
    displayGameTitle,
    gameArtCacheKey,
    gameArtMap,
    gameMetaMap,
    lazyGameArt,
    requestGameGenres,
  } from './gameArtStore';
  import { fileIcon } from './iconFor';
  import type { AddDestination, CoreDetail, CoreFiles, FileEntry, FolderListing } from './types';

  export let card: string;
  export let detail: CoreDetail;
  export let files: CoreFiles;
  /** Called after files were added so the page reloads the listing. */
  export let changed: () => void;
  export let selectedGameRel = '';
  export let onSelectGame: (item: FileEntry, folder: FolderListing) => void = () => {};

  let dragging = false;
  let hot = '';
  let sheet: { sources: string[]; destination: AddDestination; platform: string } | null = null;
  let removing: string[] | null = null;
  let selectMode = false;
  let picked: Record<string, string[]> = {};
  let artTarget: { platformId: string; platformName: string; romName: string } | null = null;

  const isGameEntry = (item: FileEntry) =>
    item.kind === 'Rom' || item.kind === 'Disk' || item.kind === 'Tape';
  const isPicked = (z: string, rel: string) => (picked[z] ?? []).includes(rel);
  const togglePick = (z: string, rel: string) => {
    const cur = picked[z] ?? [];
    picked = {
      ...picked,
      [z]: cur.includes(rel) ? cur.filter((r) => r !== rel) : [...cur, rel],
    };
  };
  function toggleSelectMode() {
    selectMode = !selectMode;
    if (!selectMode) picked = {};
  }
  function selectAllInFolder(f: FolderListing, list: FileEntry[]) {
    const z = zoneId(f);
    const allRels = list.map((e) => e.rel_path);
    const cur = picked[z] ?? [];
    picked = {
      ...picked,
      [z]: cur.length === allRels.length ? [] : allRels,
    };
  }

  let query: Record<string, string> = {};
  let sortBy: Record<string, 'name' | 'size' | 'date'> = {};
  let catFilter: Record<string, string> = {};

  // Games only: a core's BIOS and configuration files are listed under Settings.
  $: games = files.folders.map((f) => {
    const entries = f.entries.filter((e) => !e.support);
    return f.truncated
      ? { ...f, entries }
      : {
          ...f,
          entries,
          total_files: entries.length,
          total_bytes: entries.reduce((n, e) => n + e.bytes, 0),
        };
  });
  $: assets = games.filter(
    (f) =>
      f.folder.role === 'AssetsShared' ||
      (f.folder.role === 'AssetsCore' && (f.entries.length || f.truncated)),
  );
  $: for (const f of assets) {
    const romNames = f.entries.filter(isGameEntry).map((e) => e.name);
    if (romNames.length) void requestGameGenres(f.folder.platform, romNames);
  }

  const zoneId = (f: FolderListing) => `${f.folder.role}:${f.folder.platform}`;
  const dest = (f: FolderListing): AddDestination =>
    f.folder.role === 'AssetsShared' ? 'Shared' : 'CoreOnly';
  const platformName = (id: string) => detail.platforms.find((p) => p.id === id)?.name ?? id;

  function genreOf(
    platformId: string,
    item: FileEntry,
    metaMap: Record<string, { genre: string | null }>,
  ): string | null {
    if (!isGameEntry(item)) return null;
    return metaMap[gameArtCacheKey(platformId, item.name)]?.genre ?? null;
  }

  function folderGenres(
    f: FolderListing,
    metaMap: Record<string, { genre: string | null }>,
  ): { name: string; count: number }[] {
    const counts = new Map<string, number>();
    let missing = 0;
    for (const item of f.entries) {
      if (!isGameEntry(item)) continue;
      const g = genreOf(f.folder.platform, item, metaMap);
      if (g) counts.set(g, (counts.get(g) ?? 0) + 1);
      else missing++;
    }
    if (!counts.size) return [];
    const list = [...counts.entries()]
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([name, count]) => ({ name, count }));
    if (missing > 0) list.push({ name: '__other__', count: missing });
    return list;
  }

  function entries(
    f: FolderListing,
    q0: string | undefined,
    cat0: string | undefined,
    by: 'name' | 'size' | 'date' | undefined,
    metaMap: Record<string, { genre: string | null }>,
  ): FileEntry[] {
    const q = (q0 ?? '').trim().toLowerCase();
    const cat = cat0 ?? '';
    const list = f.entries.filter((e) => {
      if (cat) {
        const g = genreOf(f.folder.platform, e, metaMap);
        if (cat === '__other__' ? g !== null : g !== cat) return false;
      }
      if (!q) return true;
      const g = genreOf(f.folder.platform, e, metaMap) ?? '';
      return `${e.rel_path} ${e.slot ?? ''} ${KIND_LABEL[e.kind]} ${g}`
        .toLowerCase()
        .includes(q);
    });
    return by === 'size'
      ? [...list].sort((a, b) => b.bytes - a.bytes)
      : by === 'date'
        ? [...list].sort((a, b) => b.modified - a.modified)
        : list;
  }

  function begin(paths: string[], zone: string) {
    const f =
      assets.find((a) => zoneId(a) === zone) ??
      assets.find((a) => a.folder.role === 'AssetsShared') ??
      assets[0];
    if (!f || !paths.length) return;
    sheet = { sources: paths, destination: dest(f), platform: f.folder.platform };
  }

  const off = onFileDrop((e) => {
    if (sheet) return;
    if (e.type === 'leave') {
      dragging = false;
      hot = '';
      return;
    }
    dragging = true;
    hot = zoneAt(e.x, e.y);
    if (e.type === 'drop') {
      dragging = false;
      const zone = hot;
      hot = '';
      begin(e.paths, zone);
    }
  });
  onDestroy(off);

  async function choose(f: FolderListing, folder: boolean) {
    const r = await pick({
      multiple: !folder,
      directory: folder,
      title: folder ? 'Add a folder' : 'Add files',
    });
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
      <ul>
        {#each files.missing as m}
          <li>
            <span>
              <strong>{m.filename}</strong>
              <span class="cf-sub">
                ({m.slot}{m.size_exact ? `, exactly ${size(m.size_exact)}` : ''}) is missing. Put it in <code>{m.folder_rel}</code>.
              </span>
            </span>
            <button type="button" class="quiet" on:click={() => chooseFor(m.folder_rel)}>
              Add {m.filename}…
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#each assets as f (zoneId(f))}
    {@const zid = zoneId(f)}
    {@const genres = folderGenres(f, $gameMetaMap)}
    {@const list = entries(f, query[zid], catFilter[zid], sortBy[zid], $gameMetaMap)}
    {@const zonePicked = picked[zid] ?? []}
    <section class="cf-folder" class:hot={hot === zid} data-drop-zone={zid} aria-label={f.folder.label}>
      <header>
        <div>
          <h3>{f.folder.label}</h3>
          <p class="cf-path">
            <code>{f.folder.rel_path}</code> · {plural(f.total_files, 'file')}{f.total_bytes ? `, ${size(f.total_bytes)}` : ''}{f.exists ? '' : ' · not created yet'}
          </p>
        </div>
        <span class="cf-actions">
          {#if f.entries.length}
            <button
              type="button"
              class="quiet"
              class:active-btn={selectMode}
              aria-pressed={selectMode}
              on:click={toggleSelectMode}
            >
              {selectMode ? 'Done' : 'Select'}
            </button>
          {/if}
          <button type="button" class="quiet" on:click={() => choose(f, false)}>Add files…</button>
          <button type="button" class="quiet" on:click={() => choose(f, true)}>Add folder…</button>
        </span>
      </header>

      {#if f.entries.length}
        <div class="cf-tools">
          <label>
            <span class="sr">Search {f.folder.label}</span>
            <input type="search" placeholder="Search these files" bind:value={query[zid]} />
          </label>
          {#if genres.length}
            <label>
              <span class="sr">Filter by category</span>
              <select aria-label="Filter by category" bind:value={catFilter[zid]}>
                <option value="">All categories</option>
                {#each genres as g (g.name)}
                  <option value={g.name}>
                    {g.name === '__other__' ? 'Other' : g.name} ({g.count})
                  </option>
                {/each}
              </select>
            </label>
          {/if}
          <label>
            <span class="sr">Sort</span>
            <select bind:value={sortBy[zid]}>
              <option value="name">Name</option>
              <option value="size">Largest</option>
              <option value="date">Newest</option>
            </select>
          </label>
          <div class="cf-view-toggle" role="group" aria-label="View mode">
            <button
              type="button"
              class:active={$coreFilesViewMode === 'list'}
              aria-pressed={$coreFilesViewMode === 'list'}
              on:click={() => coreFilesViewMode.set('list')}
            >
              List
            </button>
            <button
              type="button"
              class:active={$coreFilesViewMode === 'grid'}
              aria-pressed={$coreFilesViewMode === 'grid'}
              on:click={() => coreFilesViewMode.set('grid')}
            >
              Grid
            </button>
          </div>
        </div>

        {#if selectMode}
          <div class="cf-selbar" role="region" aria-label="Selection actions">
            <span>{zonePicked.length} selected</span>
            <div class="cf-sel-btns">
              {#if list.length}
                <button type="button" class="quiet" on:click={() => selectAllInFolder(f, list)}>
                  {zonePicked.length === list.length ? 'Clear selection' : `Select all (${list.length})`}
                </button>
              {/if}
              {#if zonePicked.length}
                <button
                  type="button"
                  class="danger-quiet"
                  on:click={() => (removing = zonePicked.map((r) => `${f.folder.rel_path}/${r}`))}
                >
                  Remove {zonePicked.length} selected…
                </button>
              {/if}
            </div>
          </div>
        {/if}

        {#if list.length}
          {#if $coreFilesViewMode === 'grid'}
            <div class="cf-grid" role="list" aria-label={f.folder.label}>
              {#each list as item (item.rel_path)}
                {@const artPlat = isGameEntry(item) ? f.folder.platform : ''}
                {@const art = artPlat ? $gameArtMap[gameArtCacheKey(artPlat, item.name)] : null}
                {@const g = genreOf(f.folder.platform, item, $gameMetaMap)}
                {@const fullRel = `${f.folder.rel_path}/${item.rel_path}`}
                <article
                  class="cf-card"
                  class:selected={isPicked(zid, item.rel_path)}
                  class:active-game={!selectMode && selectedGameRel === fullRel}
                  role="listitem"
                  use:lazyGameArt={{ platformId: artPlat, romName: item.name }}
                >
                  <div class="cf-cover">
                    <button
                      type="button"
                      class="cf-cover-hit"
                      aria-label={selectMode ? `Select ${item.rel_path}` : `Open details for ${item.name}`}
                      on:click={() => (selectMode ? togglePick(zid, item.rel_path) : onSelectGame(item, f))}
                    >
                      {#if art?.data_url}
                        <img src={art.data_url} alt={displayGameTitle(item.name)} />
                      {:else}
                        <span class="cf-cover-fallback" aria-hidden="true">
                          <Icon name={fileIcon(item.name, item.group, item.kind)} size={32} />
                        </span>
                      {/if}
                    </button>
                    {#if selectMode}
                      <input
                        type="checkbox"
                        class="cf-card-pick"
                        aria-label={`Select ${item.rel_path}`}
                        checked={isPicked(zid, item.rel_path)}
                        on:change={() => togglePick(zid, item.rel_path)}
                      />
                    {/if}
                    {#if isGameEntry(item)}
                      <button
                        type="button"
                        class="cf-card-art"
                        aria-label={`Change artwork for ${item.name}`}
                        title="Change artwork"
                        on:click|stopPropagation={() =>
                          (artTarget = {
                            platformId: f.folder.platform,
                            platformName: platformName(f.folder.platform),
                            romName: item.name,
                          })}
                      >
                        Art
                      </button>
                    {/if}
                  </div>
                  <button
                    type="button"
                    class="cf-card-meta"
                    on:click={() => (selectMode ? togglePick(zid, item.rel_path) : onSelectGame(item, f))}
                  >
                    <strong title={item.rel_path}>{displayGameTitle(item.name)}</strong>
                    <small>{size(item.bytes)}{g ? ` · ${g}` : ''}</small>
                  </button>
                </article>
              {/each}
            </div>
          {:else}
            <div class="cf-list" style={`height:${Math.min(list.length, 800) * 40}px`}>
              <VirtualList items={list} rowHeight={40} key={(e) => e.rel_path} label={f.folder.label} let:item>
                {@const artPlat = isGameEntry(item) ? f.folder.platform : ''}
                {@const art = artPlat ? $gameArtMap[gameArtCacheKey(artPlat, item.name)] : null}
                {@const g = genreOf(f.folder.platform, item, $gameMetaMap)}
                {@const fullRel = `${f.folder.rel_path}/${item.rel_path}`}
                <div
                  class="cf-row"
                  class:active-game={!selectMode && selectedGameRel === fullRel}
                  role="listitem"
                  use:lazyGameArt={{ platformId: artPlat, romName: item.name }}
                >
                  {#if selectMode}
                    <input
                      type="checkbox"
                      class="cf-pick"
                      aria-label={`Select ${item.rel_path}`}
                      checked={isPicked(zid, item.rel_path)}
                      on:change={() => togglePick(zid, item.rel_path)}
                    />
                  {/if}
                  <button
                    type="button"
                    class="cf-row-main"
                    on:click={() => (selectMode ? togglePick(zid, item.rel_path) : onSelectGame(item, f))}
                  >
                    <span class="cf-fi" aria-hidden="true">
                      {#if art?.data_url}
                        <img class="cf-thumb" src={art.data_url} alt="" />
                      {:else}
                        <Icon name={fileIcon(item.name, item.group, item.kind)} size={17} />
                      {/if}
                    </span>
                    <span class="cf-name" title={item.purpose ? `${item.rel_path}: ${item.purpose}` : item.rel_path}>
                      {item.rel_path}
                    </span>
                  </button>
                  {#if isGameEntry(item)}
                    <button
                      type="button"
                      class="cf-art-btn"
                      aria-label={`Change artwork for ${item.name}`}
                      title="Change artwork"
                      on:click|stopPropagation={() =>
                        (artTarget = {
                          platformId: f.folder.platform,
                          platformName: platformName(f.folder.platform),
                          romName: item.name,
                        })}
                    >
                      Art
                    </button>
                  {/if}
                  {#if g}<span class="chip cf-genre">{g}</span>{/if}
                  <span class="chip" class:capable={item.kind === 'Rom' || item.kind === 'Bios'}>
                    {KIND_LABEL[item.kind]}
                  </span>
                  <span class="cf-size">{size(item.bytes)}</span>
                  <span class="cf-date">{day(item.modified)}</span>
                </div>
              </VirtualList>
            </div>
          {/if}
          {#if f.truncated}
            <p class="cf-sub">Showing the first {f.entries.length} of {f.total_files} files. Search to narrow them.</p>
          {/if}
        {:else}
          <p class="cf-empty">Nothing matches the current search or category filter.</p>
        {/if}
      {:else}
        <div class="cf-drop">
          <p>
            <strong>{f.folder.role === 'AssetsShared' ? 'No files yet.' : 'Nothing only for this core.'}</strong>
            Drop ROMs, disks or tapes here{f.folder.role === 'AssetsShared' ? ` and every ${platformName(f.folder.platform)} core can use them` : ' and only this core will see them'}.
          </p>
        </div>
      {/if}
      {#if dragging}<div class="cf-hint" aria-hidden="true">Drop to add to {f.folder.rel_path}</div>{/if}
    </section>
  {/each}

  {#if assets.some((f) => f.folder.role === 'AssetsShared' && f.entries.length)}
    <p class="cf-sub">Tip: drop files anywhere on this page to add them to the shared folder.</p>
  {/if}
</div>

{#if removing}
  <RemoveFilesSheet
    {card}
    coreId={detail.id}
    coreName={detail.shortname}
    files={removing}
    on:close={() => (removing = null)}
    on:done={() => {
      picked = {};
      selectMode = false;
      changed();
    }}
  />
{/if}

{#if sheet}
  <AddFilesSheet
    {card}
    coreId={detail.id}
    platform={sheet.platform}
    platformName={platformName(sheet.platform)}
    coreName={detail.shortname}
    sources={sheet.sources}
    destination={sheet.destination}
    on:close={() => (sheet = null)}
    on:done={changed}
  />
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
  .cf-path{margin:0;font-size:12px;color:#95a5a4}.cf-sub{font-size:12px;color:#8f9e9d;margin:6px 0}.cf-actions{display:flex;gap:4px;flex-wrap:wrap}
  .quiet.active-btn{background:#233528;color:#c1f0ad;border-color:#446649}
  .cf-tools{display:flex;gap:8px;margin:6px 0 8px;align-items:center;flex-wrap:wrap}.cf-tools input,.cf-tools select{font:inherit;font-size:12.5px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:7px 10px}.cf-tools label:first-child{flex:1;min-width:160px}.cf-tools input{width:100%;box-sizing:border-box}
  .cf-view-toggle{display:inline-flex;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:2px;gap:2px;flex:none}
  .cf-view-toggle button{background:transparent;color:#95a5a4;border-radius:6px;padding:4px 10px;font-size:12px}
  .cf-view-toggle button.active{background:#243335;color:#c1f0ad}
  .cf-selbar{display:flex;justify-content:space-between;align-items:center;gap:10px;flex-wrap:wrap;margin:0 0 8px;padding:8px 12px;border-radius:9px;background:#11191a;border:1px solid #2d3e3f;font-size:12.5px;color:#dce6e4}
  .cf-sel-btns{display:flex;gap:6px;align-items:center}
  .cf-list{display:flex;flex-direction:column;border:1px solid #2c393a;border-radius:9px;overflow:hidden;background:#141c1d}
  .cf-row{height:40px;box-sizing:border-box;display:flex;align-items:center;gap:8px;padding:0 12px;border-bottom:1px solid #222d2f;font-size:12.5px}
  .cf-row.active-game{background:#182421}
  .cf-row-main{flex:1;min-width:0;display:flex;align-items:center;gap:8px;background:transparent;padding:0;text-align:left;color:inherit;font:inherit;cursor:pointer}
  .cf-row-main:hover .cf-name{color:#c1f0ad}
  .cf-pick{flex:none;accent-color:#c1f0ad;width:15px;height:15px}.danger-quiet{background:none;border:1px solid #6b3a3a;color:#ff9b9b;border-radius:8px;padding:6px 10px;font:inherit;font-size:12.5px;cursor:pointer}
  .cf-fi{flex:none;display:inline-grid;place-items:center;width:28px;height:28px;border-radius:6px;background:#1d2a2b;color:#c1f0ad;overflow:hidden}
  .cf-thumb{width:100%;height:100%;object-fit:cover;display:block}
  .cf-name{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:#e8ecec}.cf-size{width:70px;text-align:right;color:#a6b3b2;flex:none}.cf-date{width:78px;text-align:right;color:#8f9e9d;font-size:11.5px;flex:none}
  .cf-genre{font-size:11px}
  .cf-art-btn{flex:none;padding:2px 8px;font-size:11px;background:#1a2526;color:#9fb3aa;border:1px solid #304142;border-radius:6px;opacity:0;transition:opacity .12s ease}
  .cf-row:hover .cf-art-btn,.cf-art-btn:focus-visible{opacity:1;color:#c1f0ad;border-color:#c1f0ad}
  .cf-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(148px,1fr));gap:12px;max-height:620px;overflow-y:auto;padding:2px}
  .cf-card{background:#111819;border:1px solid #2a3738;border-radius:10px;overflow:hidden;display:flex;flex-direction:column;transition:border-color .14s ease}
  .cf-card.selected,.cf-card.active-game{border-color:#c1f0ad;background:#17251d}
  .cf-cover{position:relative;width:100%;aspect-ratio:1/1;background:#0a0f10;overflow:hidden}
  .cf-cover-hit{width:100%;height:100%;padding:0;background:transparent;display:grid;place-items:center;cursor:pointer}
  .cf-cover img{width:100%;height:100%;object-fit:cover;display:block}
  .cf-cover-fallback{color:#6d807f}
  .cf-card-pick{position:absolute;top:8px;left:8px;width:16px;height:16px;accent-color:#c1f0ad;cursor:pointer;z-index:2}
  .cf-card-art{position:absolute;top:8px;right:8px;padding:2px 8px;font-size:11px;background:rgba(16,22,23,.88);color:#eff5f4;border:1px solid #3c4d4e;border-radius:6px;opacity:0;transition:opacity .12s ease;z-index:2}
  .cf-card:hover .cf-card-art,.cf-card:focus-within .cf-card-art{opacity:1}
  .cf-card-art:hover{border-color:#c1f0ad;color:#c1f0ad}
  .cf-card-meta{display:grid;gap:2px;min-width:0;padding:8px 10px 10px;background:transparent;text-align:left;font:inherit;cursor:pointer}
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
