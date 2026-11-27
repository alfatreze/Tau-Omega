<script lang="ts">
  // Dedicated core view for Game Bub cards (`/cores/<id>/core.json`, `files.json`, `settings.json`).
  // Supports extension-filtered ROM scanning, stem-matched companion saves/save states,
  // optional per-core pinned folder, reviewed Add/Remove, and Plex-style banner artwork.
  import { open } from '@tauri-apps/plugin-dialog';
  import ArtworkPickerSheet from './ArtworkPickerSheet.svelte';
  import GameArtworkPickerSheet from './GameArtworkPickerSheet.svelte';
  import GameDetailsInspector from './GameDetailsInspector.svelte';
  import { errorMessage } from './backend';
  import { day, plural, size } from './coresFormat';
  import { coresSelection, inspectorOpen, refreshCores } from './coresStore';
  import {
    coreFilesViewMode,
    displayGameTitle,
    gameArtCacheKey,
    gameArtMap,
    gameMetaMap,
    lazyGameArt,
    requestGameGenres,
  } from './gameArtStore';
  import Icon from './Icon.svelte';
  import { platformFallbackArt } from './platformArt';
  import { showToast } from './toastStore';
  import {
    artworkReadOverride,
    gamebubCoreFiles,
    gamebubExecuteAddFiles,
    gamebubExecuteDeleteFiles,
    gamebubExecuteRemove,
    gamebubInspectCard,
    gamebubPlanAddFiles,
    gamebubPlanDeleteFiles,
    gamebubPlanRemove,
    gamebubSetRomFolder,
    systemCoreProfile,
  } from './tau-api';
  import type {
    GameBubAddPlan,
    GameBubCoreFilesView,
    GameBubDeletePlan,
    GameBubInstalledCore,
    GameBubRemovePlan,
    GameBubRomEntry,
  } from './types';

  export let card = '';
  export let coreId = '';
  export let reloadCard: () => void = () => {};

  let tab: 'files' | 'saves' | 'settings' = 'files';
  let info: GameBubInstalledCore | null = null;
  let filesView: GameBubCoreFilesView | null = null;
  let loading = true;
  let error = '';
  let loadedFor = '';
  let overrideBanner: string | null = null;
  let inferredPlatform = '';
  let showArtPicker = false;
  let gameArtTarget: { platformId: string; platformName: string; romName: string } | null = null;

  // ROM search, category filter, multiselect, details sidebar, and reviewed add/remove state
  let romQuery = '';
  let romCategory = '';
  let selectMode = false;
  let selectedRoms: string[] = [];
  let selectedGame: GameBubRomEntry | null = null;
  let sideMode: 'core' | 'game' = 'core';
  let showUnmatched = false;
  let fileError = '';
  let fileBusy = false;

  let addSources: string[] = [];
  let addDestFolder = '';
  let addReplace = false;
  let addPlan: GameBubAddPlan | null = null;
  let deletePlan: GameBubDeletePlan | null = null;

  let removePlan: GameBubRemovePlan | null = null;
  let removeBusy = false;
  let removeError = '';

  async function refreshFiles(c: string, id: string) {
    try {
      filesView = await gamebubCoreFiles(c, id);
    } catch {
      filesView = null;
    }
  }

  async function load(c: string, id: string) {
    loadedFor = `${c}|${id}`;
    loading = true;
    error = '';
    info = null;
    filesView = null;
    overrideBanner = null;
    inferredPlatform = '';
    removePlan = null;
    removeError = '';
    selectedRoms = [];
    selectMode = false;
    romCategory = '';
    selectedGame = null;
    sideMode = 'core';
    addPlan = null;
    deletePlan = null;
    fileError = '';
    showUnmatched = false;
    tab = 'files';
    try {
      const [view, ov, profile, fv] = await Promise.all([
        gamebubInspectCard(c),
        artworkReadOverride(`game_bub:core:${id}`).catch(() => null),
        systemCoreProfile(c, 'game_bub', id).catch(() => null),
        gamebubCoreFiles(c, id).catch(() => null),
      ]);
      if (loadedFor !== `${c}|${id}`) return;
      info = view.cores.find((x) => x.core.id.toLowerCase() === id.toLowerCase()) ?? null;
      overrideBanner = ov;
      inferredPlatform = profile?.platform_ids?.[0] ?? '';
      filesView = fv;
      if (!info) error = `Core “${id}” was not found under /cores/ on this card.`;
    } catch (e) {
      if (loadedFor === `${c}|${id}`) error = errorMessage(e);
    } finally {
      if (loadedFor === `${c}|${id}`) loading = false;
    }
  }

  $: if (card && coreId && `${card}|${coreId}` !== loadedFor) void load(card, coreId);

  $: missingCount = (info?.missing_bitstreams.length ?? 0) + (info?.missing_required_files.length ?? 0);
  $: fixedFiles = info?.files.filter((f) => !!f.filename) ?? [];
  $: romSlots = info?.files.filter((f) => f.user_selected) ?? [];
  $: dependentFiles = info?.files.filter((f) => f.dependent_on_0 && !f.filename && !f.user_selected) ?? [];

  $: if (inferredPlatform && filesView?.roms.length) {
    void requestGameGenres(
      inferredPlatform,
      filesView.roms.map((r) => r.name),
    );
  }

  function romGenre(
    rom: GameBubRomEntry,
    plat: string,
    metaMap: Record<string, { genre: string | null }>,
  ): string | null {
    if (!plat) return null;
    return metaMap[gameArtCacheKey(plat, rom.name)]?.genre ?? null;
  }

  $: romGenres = (() => {
    if (!inferredPlatform || !filesView?.roms.length) return [];
    const counts = new Map<string, number>();
    let missing = 0;
    for (const r of filesView.roms) {
      const g = romGenre(r, inferredPlatform, $gameMetaMap);
      if (g) counts.set(g, (counts.get(g) ?? 0) + 1);
      else missing++;
    }
    if (!counts.size) return [];
    const list = [...counts.entries()]
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([name, count]) => ({ name, count }));
    if (missing > 0) list.push({ name: '__other__', count: missing });
    return list;
  })();

  $: q = romQuery.trim().toLowerCase();
  $: filteredRoms = (filesView?.roms ?? []).filter((r) => {
    const g = romGenre(r, inferredPlatform, $gameMetaMap);
    if (romCategory) {
      if (romCategory === '__other__' ? g !== null : g !== romCategory) return false;
    }
    if (!q) return true;
    return `${r.name} ${r.folder_rel} ${g ?? ''}`.toLowerCase().includes(q);
  });
  $: romFolders = (() => {
    const map = new Map<string, GameBubRomEntry[]>();
    for (const r of filteredRoms) {
      const key = r.folder_rel || '';
      const arr = map.get(key) ?? [];
      arr.push(r);
      map.set(key, arr);
    }
    return [...map.entries()];
  })();

  $: realSaves = (filesView?.saves ?? []).filter((s) => s.kind === 'save');
  $: saveStates = (filesView?.saves ?? []).filter((s) => s.kind !== 'save');
  $: unmatched = filesView?.unmatched_saves ?? [];

  $: selectedGameSaves = (() => {
    if (!selectedGame || !filesView) return [];
    const stem = selectedGame.stem.toLowerCase();
    return filesView.saves
      .filter((s) => s.matched_rom === selectedGame?.rel_path || s.game_stem.toLowerCase() === stem)
      .map((s) => ({
        name: s.name,
        label: s.slot_label || (s.kind === 'save' ? 'Save' : 'Save state'),
        kind: (s.kind === 'save' ? 'save' : 'state') as 'save' | 'state',
        bytes: s.bytes,
        modified: s.modified,
      }));
  })();

  function toggleSelectMode() {
    selectMode = !selectMode;
    if (!selectMode) {
      selectedRoms = [];
      deletePlan = null;
    }
  }

  function openRomDetails(rom: GameBubRomEntry) {
    selectedGame = rom;
    sideMode = 'game';
    inspectorOpen.set(true);
  }

  async function removeSingleRom(relPath: string) {
    if (!card || !info) return;
    selectedRoms = [relPath];
    selectMode = true;
    tab = 'files';
    await startDeleteRoms();
  }

  $: defaultAddFolder = (() => {
    if (filesView?.pinned_folder) return filesView.pinned_folder;
    const first = filesView?.roms[0]?.folder_rel;
    if (first) return first;
    const label = (info?.core.name || coreId).replace(/[/\\:]/g, ' ').trim();
    return label ? `ROMs/${label}` : 'ROMs';
  })();

  function toggleRom(rel: string) {
    deletePlan = null;
    selectedRoms = selectedRoms.includes(rel)
      ? selectedRoms.filter((p) => p !== rel)
      : [...selectedRoms, rel];
  }

  async function pinFolder() {
    if (!card || !info) return;
    fileError = '';
    const picked = await open({ directory: true, defaultPath: card });
    if (!picked || Array.isArray(picked)) return;
    const normCard = card.replace(/\/+$/, '');
    const normPicked = picked.replace(/\/+$/, '');
    if (normPicked !== normCard && !normPicked.startsWith(`${normCard}/`)) {
      fileError = 'Choose a folder inside the open Game Bub card.';
      return;
    }
    const rel = normPicked === normCard ? null : normPicked.slice(normCard.length + 1);
    fileBusy = true;
    try {
      await gamebubSetRomFolder(card, info.core.id, rel);
      await refreshFiles(card, info.core.id);
      showToast(rel ? `Pinned ROM scan to /${rel}.` : 'Scanning entire card.', 'info');
    } catch (e) {
      fileError = errorMessage(e);
    } finally {
      fileBusy = false;
    }
  }

  async function clearPin() {
    if (!card || !info) return;
    fileError = '';
    fileBusy = true;
    try {
      await gamebubSetRomFolder(card, info.core.id, null);
      await refreshFiles(card, info.core.id);
      showToast('Scanning entire card by file extension.', 'info');
    } catch (e) {
      fileError = errorMessage(e);
    } finally {
      fileBusy = false;
    }
  }

  async function chooseAddRoms() {
    if (!card || !info) return;
    fileError = '';
    deletePlan = null;
    const exts = filesView?.rom_extensions ?? [];
    const picked = await open({
      multiple: true,
      filters: exts.length ? [{ name: 'Compatible ROMs', extensions: exts }] : undefined,
    });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    if (!paths.length) return;
    addSources = paths;
    addDestFolder = defaultAddFolder;
    addReplace = false;
    await planAdd();
  }

  async function planAdd() {
    if (!card || !info || !addSources.length) return;
    fileBusy = true;
    fileError = '';
    try {
      addPlan = await gamebubPlanAddFiles(card, info.core.id, addDestFolder, addSources, addReplace);
    } catch (e) {
      fileError = errorMessage(e);
      addPlan = null;
    } finally {
      fileBusy = false;
    }
  }

  async function confirmAdd() {
    if (!card || !info || !addPlan) return;
    fileBusy = true;
    fileError = '';
    try {
      const rep = await gamebubExecuteAddFiles(
        card,
        info.core.id,
        addDestFolder,
        addSources,
        addReplace,
        addPlan.id,
      );
      showToast(`Added ${plural(rep.written, 'ROM')} (${size(rep.bytes_written)}) to /${addPlan.dest_folder || ''}.`);
      addPlan = null;
      addSources = [];
      await refreshFiles(card, info.core.id);
    } catch (e) {
      fileError = errorMessage(e);
    } finally {
      fileBusy = false;
    }
  }

  async function startDeleteRoms() {
    if (!card || !info || !selectedRoms.length) return;
    fileBusy = true;
    fileError = '';
    addPlan = null;
    try {
      deletePlan = await gamebubPlanDeleteFiles(card, info.core.id, selectedRoms);
    } catch (e) {
      fileError = errorMessage(e);
      deletePlan = null;
    } finally {
      fileBusy = false;
    }
  }

  async function confirmDeleteRoms() {
    if (!card || !info || !deletePlan) return;
    fileBusy = true;
    fileError = '';
    try {
      const rep = await gamebubExecuteDeleteFiles(
        card,
        info.core.id,
        deletePlan.rel_paths,
        deletePlan.id,
      );
      showToast(`Removed ${plural(rep.removed, 'file')}. Backed up outside the card.`);
      if (selectedGame && deletePlan.rel_paths.includes(selectedGame.rel_path)) {
        selectedGame = null;
        sideMode = 'core';
      }
      deletePlan = null;
      selectedRoms = [];
      selectMode = false;
      await refreshFiles(card, info.core.id);
    } catch (e) {
      fileError = errorMessage(e);
    } finally {
      fileBusy = false;
    }
  }

  async function startRemove() {
    if (!card || !info) return;
    removeBusy = true;
    removeError = '';
    try {
      removePlan = await gamebubPlanRemove(card, info.core.id);
    } catch (e) {
      removeError = errorMessage(e);
    } finally {
      removeBusy = false;
    }
  }

  async function confirmRemove() {
    if (!card || !info || !removePlan) return;
    removeBusy = true;
    removeError = '';
    const name = info.core.name || info.core.id;
    try {
      await gamebubExecuteRemove(card, info.core.id, removePlan.id);
      removePlan = null;
      await refreshCores(card);
      reloadCard();
      showToast(`Removed ${name} from card.`);
      coresSelection.set({ platform: '', core: '' });
    } catch (e) {
      removeError = errorMessage(e);
    } finally {
      removeBusy = false;
    }
  }
</script>

<header class="gb-head">
  <div>
    <nav class="crumbs" aria-label="Where you are">
      <button class="quiet" on:click={() => coresSelection.set({ platform: '', core: '' })}>Cores</button>
    </nav>
    <h1 id="cores-title">{info?.core.name || coreId}</h1>
    <p class="lede">{coreId}{info?.core.version ? ` · version ${info.core.version}` : ''} · Game Bub</p>
  </div>
  {#if info}
    <div class="gb-actions">
      <button
        type="button"
        class="gb-side-btn"
        class:active={$inspectorOpen}
        aria-label={$inspectorOpen ? 'Hide details' : 'Show details'}
        aria-expanded={$inspectorOpen}
        aria-controls="gb-details-side"
        on:click={() => inspectorOpen.update((v) => !v)}
        title={$inspectorOpen ? 'Hide details (I)' : 'Show details (I)'}
      >
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <line x1="15" y1="4" x2="15" y2="20" />
        </svg>
      </button>
    </div>
  {/if}
</header>

{#if removeError}<p class="gb-err" role="alert">{removeError}</p>{/if}

{#if removePlan}
  <div class="gb-rm-plan" role="region" aria-label="Remove core review">
    <strong>Remove {removePlan.name || removePlan.core_id} from this card?</strong>
    <p>
      Deletes {removePlan.files_to_remove.length} {removePlan.files_to_remove.length === 1 ? 'file' : 'files'} under <code>/cores/{info?.folder}</code> after saving a verified backup outside the card. Your ROMs and saves elsewhere on the card are untouched.
    </p>
    <div class="gb-rm-btns">
      <button class="primary danger" disabled={removeBusy} on:click={confirmRemove}>
        {removeBusy ? 'Removing…' : 'Remove core'}
      </button>
      <button class="quiet" disabled={removeBusy} on:click={() => (removePlan = null)}>Cancel</button>
    </div>
  </div>
{/if}

{#if error}
  <div class="gb-empty">
    <div class="gb-empty-ic" aria-hidden="true">!</div>
    <h2>This core could not be read</h2>
    <p>{error}</p>
    <button class="primary" on:click={() => load(card, coreId)}>Try again</button>
  </div>
{:else if loading || !info}
  <p class="gb-note" role="status">Reading core metadata…</p>
{:else}
  <div class="gb-grid" class:no-inspector={!$inspectorOpen}>
    <div class="gb-main">
      <div class="tabs" role="tablist" aria-label="Core sections">
        <button id="gb-tab-files" role="tab" class:current={tab === 'files'} aria-selected={tab === 'files'} on:click={() => (tab = 'files')}>
          Files{#if filesView?.roms.length}<span class="gb-tab-n">{filesView.roms.length}</span>{/if}
          {#if missingCount}<span class="tab-dot" role="img" aria-label="Needs attention"></span>{/if}
        </button>
        <button id="gb-tab-saves" role="tab" class:current={tab === 'saves'} aria-selected={tab === 'saves'} on:click={() => (tab = 'saves')}>
          Saves{#if filesView?.saves.length}<span class="gb-tab-n">{filesView.saves.length}</span>{/if}
        </button>
        <button id="gb-tab-settings" role="tab" class:current={tab === 'settings'} aria-selected={tab === 'settings'} on:click={() => (tab = 'settings')}>
          Settings{#if info.settings.length}<span class="gb-tab-n">{info.settings.length}</span>{/if}
        </button>
      </div>

      <div class="gb-scroll" role="tabpanel">
        {#if tab === 'files'}
          {#if missingCount > 0}
            <section class="gb-card warn">
              <h3>Needs attention</h3>
              {#if info.missing_bitstreams.length}
                <p class="gb-warn-text">Missing bitstream{info.missing_bitstreams.length === 1 ? '' : 's'}: <code>{info.missing_bitstreams.join(', ')}</code></p>
              {/if}
              {#if info.missing_required_files.length}
                <p class="gb-warn-text">Missing required file{info.missing_required_files.length === 1 ? '' : 's'}: <code>{info.missing_required_files.join(', ')}</code> (place in <code>/cores/{info.folder}/</code>)</p>
              {/if}
            </section>
          {/if}

          <div class="gb-pin-bar">
            <div class="gb-pin-info">
              <strong>
                {filesView?.pinned_folder ? `Pinned to /${filesView.pinned_folder}` : 'Scanning entire card'}
              </strong>
              <small>
                {filesView?.rom_extensions.length
                  ? `Showing ${filesView.rom_extensions.map((e) => `.${e}`).join(' ')}`
                  : 'Pin a folder to list files for this core'}
              </small>
            </div>
            <div class="gb-pin-btns">
              <button class="quiet" type="button" disabled={fileBusy} on:click={pinFolder}>Pin folder…</button>
              {#if filesView?.pinned_folder}
                <button class="quiet" type="button" disabled={fileBusy} on:click={clearPin}>Clear pin</button>
              {/if}
            </div>
          </div>

          <div class="gb-file-bar">
            <label class="sr" for="gb-rom-search">Search ROMs</label>
            <input
              id="gb-rom-search"
              type="search"
              bind:value={romQuery}
              placeholder="Search ROMs or folders…"
            />
            {#if romGenres.length}
              <label class="sr" for="gb-cat-filter">Filter by category</label>
              <select id="gb-cat-filter" aria-label="Filter by category" bind:value={romCategory}>
                <option value="">All categories</option>
                {#each romGenres as g (g.name)}
                  <option value={g.name}>{g.name === '__other__' ? 'Other' : g.name} ({g.count})</option>
                {/each}
              </select>
            {/if}
            {#if filesView?.roms.length}
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
            <div class="gb-view-toggle" role="group" aria-label="View mode">
              <button type="button" class:active={$coreFilesViewMode === 'list'} aria-pressed={$coreFilesViewMode === 'list'} on:click={() => coreFilesViewMode.set('list')}>List</button>
              <button type="button" class:active={$coreFilesViewMode === 'grid'} aria-pressed={$coreFilesViewMode === 'grid'} on:click={() => coreFilesViewMode.set('grid')}>Grid</button>
            </div>
            {#if selectMode && selectedRoms.length}
              <button class="quiet danger" type="button" disabled={fileBusy} on:click={startDeleteRoms}>
                Remove ({selectedRoms.length})
              </button>
            {/if}
            <button class="picker" type="button" disabled={fileBusy} on:click={chooseAddRoms}>
              Add ROMs…
            </button>
          </div>

          {#if fileError}<p class="gb-err" role="alert">{fileError}</p>{/if}

          {#if addPlan}
            <div class="gb-plan" role="region" aria-label="Add ROMs review">
              <strong>Add {plural(addPlan.items.length, 'ROM')} to <code>/{addPlan.dest_folder || ''}</code></strong>
              <div class="gb-plan-dest">
                <label>
                  <span>Destination folder (relative to card)</span>
                  <input type="text" bind:value={addDestFolder} on:change={planAdd} />
                </label>
                <label class="gb-check">
                  <input type="checkbox" bind:checked={addReplace} on:change={planAdd} />
                  <span>Overwrite existing files</span>
                </label>
              </div>
              {#if addPlan.items.length}
                <p>
                  {addPlan.new_files} new{addPlan.replaced_files ? `, ${addPlan.replaced_files} replacing existing (backed up first)` : ''} · {size(addPlan.bytes_to_write)}
                </p>
              {:else}
                <p>No eligible files to add.</p>
              {/if}
              {#if addPlan.skipped.length}
                <p class="gb-warn-text">Skipped: {addPlan.skipped.join('; ')}</p>
              {/if}
              <div class="gb-rm-btns">
                {#if addPlan.items.length}
                  <button class="primary" type="button" disabled={fileBusy} on:click={confirmAdd}>
                    {fileBusy ? 'Copying…' : `Copy ${plural(addPlan.items.length, 'file')}`}
                  </button>
                {/if}
                <button class="quiet" type="button" disabled={fileBusy} on:click={() => { addPlan = null; addSources = []; }}>
                  Cancel
                </button>
              </div>
            </div>
          {/if}

          {#if deletePlan}
            <div class="gb-rm-plan" role="region" aria-label="Remove ROMs review">
              <strong>Remove {plural(deletePlan.rel_paths.length, 'selected file')}?</strong>
              <p>
                Frees {size(deletePlan.total_bytes)}. Each file is backed up outside the card and verified before removal.
              </p>
              <div class="gb-rm-btns">
                <button class="primary danger" type="button" disabled={fileBusy} on:click={confirmDeleteRoms}>
                  {fileBusy ? 'Removing…' : 'Remove files'}
                </button>
                <button class="quiet" type="button" disabled={fileBusy} on:click={() => (deletePlan = null)}>
                  Cancel
                </button>
              </div>
            </div>
          {/if}

          {#if !filesView?.roms.length}
            <div class="gb-empty">
              <div class="gb-empty-ic" aria-hidden="true">
                <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
              </div>
              <strong>No matching ROMs on this card</strong>
              <p>
                {filesView?.rom_extensions.length
                  ? `No files matching ${filesView.rom_extensions.map((e) => `.${e}`).join(', ')} were found${filesView?.pinned_folder ? ` under /${filesView.pinned_folder}` : ' anywhere on this card'}.`
                  : 'Pin a folder containing ROMs for this core, or add ROM files directly.'}
              </p>
              <button class="primary" type="button" on:click={chooseAddRoms}>Add ROMs…</button>
            </div>
          {:else if !romFolders.length}
            <div class="gb-empty compact">
              <strong>Nothing matches “{romQuery}”</strong>
              <p>Try a different filename or folder name.</p>
            </div>
          {:else}
            {#each romFolders as [folder, items] (folder)}
              <section class="gb-card">
                <div class="gb-folder-head">
                  <h3><code>/{folder || '(card root)'}</code></h3>
                  <small>{plural(items.length, 'file')} · {size(items.reduce((a, b) => a + b.bytes, 0))}</small>
                </div>
                {#if $coreFilesViewMode === 'grid'}
                  <div class="gb-rom-grid" role="list" aria-label={`ROMs in /${folder || 'root'}`}>
                    {#each items as rom (rom.rel_path)}
                      {@const art = inferredPlatform ? $gameArtMap[gameArtCacheKey(inferredPlatform, rom.name)] : null}
                      {@const g = romGenre(rom, inferredPlatform, $gameMetaMap)}
                      <article
                        class="gb-rom-card"
                        class:selected={selectedRoms.includes(rom.rel_path)}
                        class:active-game={!selectMode && selectedGame?.rel_path === rom.rel_path}
                        role="listitem"
                        use:lazyGameArt={{ platformId: inferredPlatform, romName: rom.name }}
                      >
                        <div class="gb-rom-cover">
                          <button
                            type="button"
                            class="gb-cover-hit"
                            aria-label={selectMode ? `Select ${rom.rel_path}` : `Open details for ${rom.name}`}
                            on:click={() => (selectMode ? toggleRom(rom.rel_path) : openRomDetails(rom))}
                          >
                            {#if art?.data_url}
                              <img src={art.data_url} alt={displayGameTitle(rom.name)} />
                            {:else}
                              <span class="gb-cover-fallback" aria-hidden="true"><Icon name="game-controller" size={32} /></span>
                            {/if}
                          </button>
                          {#if selectMode}
                            <input
                              type="checkbox"
                              class="gb-card-pick"
                              aria-label={`Select ${rom.rel_path}`}
                              checked={selectedRoms.includes(rom.rel_path)}
                              on:change={() => toggleRom(rom.rel_path)}
                            />
                          {/if}
                          <button
                            type="button"
                            class="gb-card-art"
                            aria-label={`Change artwork for ${rom.name}`}
                            title="Change artwork"
                            on:click|stopPropagation={() => (gameArtTarget = { platformId: inferredPlatform, platformName: info?.core.name || '', romName: rom.name })}
                          >
                            Art
                          </button>
                        </div>
                        <button
                          type="button"
                          class="gb-rom-card-meta"
                          on:click={() => (selectMode ? toggleRom(rom.rel_path) : openRomDetails(rom))}
                        >
                          <strong title={rom.rel_path}>{displayGameTitle(rom.name)}</strong>
                          <small>{size(rom.bytes)}{g ? ` · ${g}` : ''}</small>
                        </button>
                      </article>
                    {/each}
                  </div>
                {:else}
                  <ul class="gb-rom-list">
                    {#each items as rom (rom.rel_path)}
                      {@const art = inferredPlatform ? $gameArtMap[gameArtCacheKey(inferredPlatform, rom.name)] : null}
                      {@const g = romGenre(rom, inferredPlatform, $gameMetaMap)}
                      <li
                        class="gb-rom-row"
                        class:active-game={!selectMode && selectedGame?.rel_path === rom.rel_path}
                        use:lazyGameArt={{ platformId: inferredPlatform, romName: rom.name }}
                      >
                        {#if selectMode}
                          <input
                            type="checkbox"
                            aria-label={`Select ${rom.rel_path}`}
                            checked={selectedRoms.includes(rom.rel_path)}
                            on:change={() => toggleRom(rom.rel_path)}
                          />
                        {/if}
                        <button
                          type="button"
                          class="gb-rom-main"
                          on:click={() => (selectMode ? toggleRom(rom.rel_path) : openRomDetails(rom))}
                        >
                          <span class="gb-fi" aria-hidden="true">{#if art?.data_url}<img class="gb-thumb" src={art.data_url} alt="" />{:else}<Icon name="game-controller" size={16} />{/if}</span>
                          <span class="gb-rom-name" title={rom.rel_path}>{rom.name}</span>
                        </button>
                        {#if g}<span class="chip gb-genre">{g}</span>{/if}
                        <button
                          type="button"
                          class="gb-art-btn"
                          aria-label={`Change artwork for ${rom.name}`}
                          title="Change artwork"
                          on:click|stopPropagation={() => (gameArtTarget = { platformId: inferredPlatform, platformName: info?.core.name || '', romName: rom.name })}
                        >
                          Art
                        </button>
                        <span class="gb-rom-size">{size(rom.bytes)}</span>
                        <span class="gb-rom-date">{day(rom.modified)}</span>
                      </li>
                    {/each}
                  </ul>
                {/if}
              </section>
            {/each}
          {/if}
        {:else if tab === 'saves'}
          {#if !filesView?.saves.length}
            <div class="gb-empty">
              <div class="gb-empty-ic" aria-hidden="true">
                <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>
              </div>
              <strong>No saves for this core yet</strong>
              <p>
                {#if filesView?.save_extensions.length}
                  Saves and save states ({filesView.save_extensions.map((e) => `.${e}`).join(', ')}) placed beside a matching ROM in the same folder appear here automatically.
                {:else}
                  This core does not declare companion save extensions in <code>files.json</code>.
                {/if}
              </p>
            </div>
          {:else}
            {#if realSaves.length}
              <section class="gb-card">
                <div class="gb-folder-head">
                  <h3>Saves</h3>
                  <small>{plural(realSaves.length, 'save')} · {size(realSaves.reduce((a, b) => a + b.bytes, 0))}</small>
                </div>
                <ul class="gb-save-list">
                  {#each realSaves as s (s.rel_path)}
                    <li>
                      <span class="gb-fi" aria-hidden="true"><Icon name="memory" size={16} /></span>
                      <div class="gb-save-main">
                        <strong>{s.game_stem}</strong>
                        <small><code>/{s.rel_path}</code></small>
                      </div>
                      <span class="chip">{s.slot_label}</span>
                      <span class="gb-rom-size">{size(s.bytes)}</span>
                      <span class="gb-rom-date">{day(s.modified)}</span>
                    </li>
                  {/each}
                </ul>
              </section>
            {/if}
            {#if saveStates.length}
              <section class="gb-card">
                <div class="gb-folder-head">
                  <h3>Save states</h3>
                  <small>{plural(saveStates.length, 'save state')} · {size(saveStates.reduce((a, b) => a + b.bytes, 0))}</small>
                </div>
                <ul class="gb-save-list">
                  {#each saveStates as s (s.rel_path)}
                    <li>
                      <span class="gb-fi" aria-hidden="true"><Icon name="clock-counter-clockwise" size={16} /></span>
                      <div class="gb-save-main">
                        <strong>{s.game_stem}</strong>
                        <small><code>/{s.rel_path}</code></small>
                      </div>
                      <span class="chip">{s.slot_label}</span>
                      <span class="gb-rom-size">{size(s.bytes)}</span>
                      <span class="gb-rom-date">{day(s.modified)}</span>
                    </li>
                  {/each}
                </ul>
              </section>
            {/if}
          {/if}

          {#if unmatched.length}
            <div class="gb-unmatched-bar">
              <button class="quiet" type="button" on:click={() => (showUnmatched = !showUnmatched)}>
                {showUnmatched ? 'Hide' : 'Show'} {plural(unmatched.length, 'unmatched save file')} without a matching ROM beside {unmatched.length === 1 ? 'it' : 'them'}
              </button>
              {#if showUnmatched}
                <section class="gb-card">
                  <p class="gb-sub">
                    These files match {info.core.name || info.core.id}’s save extensions, so they could belong to this core or another core using the same folder.
                  </p>
                  <ul class="gb-save-list">
                    {#each unmatched as s (s.rel_path)}
                      <li>
                        <span class="gb-fi" aria-hidden="true"><Icon name="memory" size={16} /></span>
                        <div class="gb-save-main">
                          <strong>{s.name}</strong>
                          <small><code>/{s.rel_path}</code></small>
                        </div>
                        <span class="gb-rom-size">{size(s.bytes)}</span>
                        <span class="gb-rom-date">{day(s.modified)}</span>
                      </li>
                    {/each}
                  </ul>
                </section>
              {/if}
            </div>
          {/if}
        {:else}
          <section class="gb-card">
            <h3>Core settings</h3>
            {#if !info.settings.length}
              <div class="gb-empty compact">
                <div class="gb-empty-ic" aria-hidden="true">
                  <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9c.26.604.852.997 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
                </div>
                <strong>No configurable settings</strong>
                <p>This core doesn’t declare a <code>settings.json</code> file, so it runs with its built-in defaults on Game Bub.</p>
              </div>
            {:else}
              <p class="gb-sub">Declared in <code>/cores/{info.folder}/settings.json</code>. Change these in the core’s menu on your Game Bub.</p>
              <ul class="gb-list">
                {#each info.settings as s}
                  <li>
                    <strong>{s.label}</strong>
                    <span class="chip">{s.kind || 'option'}</span>
                  </li>
                {/each}
              </ul>
            {/if}
          </section>

          <section class="gb-card">
            <h3>Declared files</h3>
            {#if !info.files.length}
              <p class="gb-sub">This core does not include a <code>files.json</code> manifest.</p>
            {:else}
              {#if fixedFiles.length}
                <h4 class="gb-subh">Fixed files in <code>/cores/{info.folder}/</code></h4>
                <ul class="gb-list">
                  {#each fixedFiles as f}
                    {@const isMissing = !!f.filename && info.missing_required_files.includes(f.filename)}
                    <li>
                      <div>
                        <strong>{f.label || f.filename}</strong>
                        <small><code>{f.filename}</code>{f.optional ? ' · optional' : ' · required'}</small>
                      </div>
                      {#if isMissing}<span class="chip warn">Missing</span>{:else}<span class="chip capable">Present</span>{/if}
                    </li>
                  {/each}
                </ul>
              {/if}
              {#if romSlots.length}
                <h4 class="gb-subh">User-selected files (loaded from SD card)</h4>
                <ul class="gb-list">
                  {#each romSlots as f}
                    <li>
                      <div>
                        <strong>{f.label || 'Game ROM'}</strong>
                        <small>{f.extensions.length ? f.extensions.map((e) => (e.startsWith('.') ? e : `.${e}`)).join('  ') : 'Any file'}{f.optional ? ' · optional' : ''}</small>
                      </div>
                      <span class="chip">Browser</span>
                    </li>
                  {/each}
                </ul>
              {/if}
              {#if dependentFiles.length}
                <h4 class="gb-subh">Companion files (matched to loaded ROM)</h4>
                <ul class="gb-list">
                  {#each dependentFiles as f}
                    <li>
                      <div>
                        <strong>{f.label}</strong>
                        <small>{f.extensions.length ? f.extensions.map((e) => (e.startsWith('.') ? e : `.${e}`)).join('  ') : 'Matched by name'}{f.read_only ? ' · read-only' : ''}</small>
                      </div>
                    </li>
                  {/each}
                </ul>
              {/if}
            {/if}
          </section>
        {/if}
      </div>
    </div>

    {#if $inspectorOpen}
      <div id="gb-details-side" class="gb-side-shell">
        {#if selectedGame}
          <div class="gb-mode-bar" role="group" aria-label="Details view">
            <button type="button" class:active={sideMode === 'core'} aria-pressed={sideMode === 'core'} on:click={() => (sideMode = 'core')}>Core details</button>
            <button type="button" class:active={sideMode === 'game'} aria-pressed={sideMode === 'game'} on:click={() => (sideMode = 'game')}>Game details</button>
            <button type="button" class="gb-mode-clear" aria-label="Clear selected game" title="Back to core details" on:click={() => { selectedGame = null; sideMode = 'core'; }}>✕</button>
          </div>
        {/if}

        {#if sideMode === 'game' && selectedGame}
          <GameDetailsInspector
            platformId={inferredPlatform}
            platformName={info.core.name || info.core.id}
            romName={selectedGame.name}
            relPath={selectedGame.rel_path}
            bytes={selectedGame.bytes}
            modified={selectedGame.modified}
            saves={selectedGameSaves}
            onOpenArtwork={() => {
              if (selectedGame) gameArtTarget = { platformId: inferredPlatform, platformName: info?.core.name || '', romName: selectedGame.name };
            }}
            onRemove={() => {
              if (selectedGame) void removeSingleRom(selectedGame.rel_path);
            }}
          />
        {:else}
          <aside class="gb-side" aria-label={info.core.name || info.core.id}>
            <div class="gb-art-wrap">
              <img class="gb-banner" src={overrideBanner || platformFallbackArt({ id: inferredPlatform || info.core.id, name: info.core.name || info.core.id, category: 'Handheld', manufacturer: info.core.author || 'Game Bub' })} alt="" />
              <button type="button" class="gb-art-edit" on:click={() => (showArtPicker = true)}>Change image</button>
            </div>
            <div class="gb-side-body">
              <div class="gb-side-title">
                <strong>{info.core.name || info.core.id}</strong>
                <small>{info.core.author || info.core.id}{info.core.release_date ? ` · ${info.core.release_date}` : ''}</small>
              </div>
              {#if info.core.description}<p class="gb-desc">{info.core.description}</p>{/if}
              <dl class="gb-dl">
                <div><dt>Version</dt><dd>{info.core.version || 'Not stated'}</dd></div>
                {#if info.core.license}<div><dt>Licence</dt><dd>{info.core.license}</dd></div>{/if}
                <div><dt>Min firmware</dt><dd>{info.core.min_firmware || 'Not stated'}</dd></div>
              </dl>

              <h4>On this card</h4>
              {#if filesView}
                <dl class="gb-dl">
                  <div><dt>Games</dt><dd>{plural(filesView.roms.length, 'file')}</dd></div>
                  <div><dt>Saves</dt><dd>{plural(realSaves.length, 'save')} · {plural(saveStates.length, 'save state')}</dd></div>
                  <div><dt>Disk used</dt><dd>{size(filesView.total_rom_bytes + filesView.total_save_bytes)}</dd></div>
                </dl>
              {:else}
                <p class="gb-sub">Reading files…</p>
              {/if}

              <h4>Hardware targets</h4>
              {#if info.core.bitstreams.length}
                <ul class="gb-targets">
                  {#each info.core.bitstreams as bs}
                    {@const missing = info.missing_bitstreams.includes(bs.filename)}
                    <li>
                      <span><strong>{bs.target}</strong><small><code>{bs.filename}</code></small></span>
                      {#if missing}<span class="chip warn">Missing</span>{:else}<span class="chip capable">Ready</span>{/if}
                    </li>
                  {/each}
                </ul>
              {:else}
                <p class="gb-sub">No bitstreams listed in <code>core.json</code>.</p>
              {/if}

              {#if info.core.hardware.cartridge_enable || info.core.hardware.cartridge_selectable}
                <h4>Cartridge slot</h4>
                <p class="gb-sub">
                  {info.core.hardware.cartridge_enable ? `Mode: ${info.core.hardware.cartridge_enable}` : 'Supported'}
                  {info.core.hardware.cartridge_selectable ? ' · selectable' : ''}
                </p>
              {/if}

              <div class="gb-side-remove">
                <button class="danger-quiet" disabled={removeBusy || !!removePlan} on:click={startRemove}>
                  {removeBusy && !removePlan ? 'Planning…' : 'Remove core…'}
                </button>
              </div>
            </div>
          </aside>
        {/if}
      </div>
    {/if}
  </div>
  {#if showArtPicker}
    <ArtworkPickerSheet
      {card}
      device="game_bub"
      title={`Artwork — ${info.core.name || info.core.id}`}
      platformId={inferredPlatform || info.core.name || info.core.id}
      overrideKey={`game_bub:core:${info.core.id}`}
      hasOverride={!!overrideBanner}
      currentUrl={overrideBanner}
      onApplied={(u) => (overrideBanner = u)}
      close={() => (showArtPicker = false)}
    />
  {/if}
  {#if gameArtTarget}
    <GameArtworkPickerSheet
      platformId={gameArtTarget.platformId}
      platformName={gameArtTarget.platformName}
      romName={gameArtTarget.romName}
      current={gameArtTarget.platformId ? ($gameArtMap[gameArtCacheKey(gameArtTarget.platformId, gameArtTarget.romName)] ?? null) : null}
      close={() => (gameArtTarget = null)}
    />
  {/if}
{/if}

<style>
  .gb-head{flex:none;display:flex;justify-content:space-between;align-items:flex-start;gap:16px;padding-right:52px}
  .crumbs{display:flex;align-items:center;gap:2px;font-size:12px;color:#8f9e9d;margin:0 0 8px -8px}.crumbs .quiet{padding:4px 8px;font-size:12px}
  .gb-actions{display:flex;gap:8px;flex:none}
  .gb-err{margin:0 0 12px;padding:10px 14px;border-radius:10px;border:1px solid #6b3535;background:#261616;color:#f2b8b5;font-size:13px}
  .gb-note{font-size:13px;color:#95a5a4}
  .gb-rm-plan,.gb-plan{margin:0 0 14px;padding:13px 16px;border-radius:12px;border:1px solid #6b3535;background:#261616;display:grid;gap:8px}
  .gb-plan{border-color:#3c5d41;background:#152118}
  .gb-rm-plan p,.gb-plan p{margin:0;font-size:12.5px;color:#d6e0d8;line-height:1.5}
  .gb-plan-dest{display:flex;flex-wrap:wrap;gap:12px;align-items:flex-end}
  .gb-plan-dest label{display:grid;gap:4px;font-size:12px;color:#9fb3aa}
  .gb-plan-dest input[type=text]{font:inherit;font-size:12.5px;color:#e8ecec;background:#0e1415;border:1px solid #314436;border-radius:7px;padding:6px 10px;min-width:220px}
  .gb-check{display:flex !important;flex-direction:row !important;align-items:center;gap:6px;cursor:pointer}
  .gb-rm-btns{display:flex;gap:8px;margin-top:4px}
  .gb-grid{flex:1;min-height:0;display:grid;grid-template-columns:minmax(0,1fr) 330px;gap:22px;align-items:start}
  .gb-grid.no-inspector{grid-template-columns:minmax(0,1fr)}
  .gb-side-btn{display:inline-grid;place-items:center;width:34px;height:34px;padding:0;border-radius:9px;background:#161f20;border:1px solid #2c393a;color:#95a5a4;cursor:pointer}
  .gb-side-btn:hover{color:#eff5f4;border-color:#3d5254}
  .gb-side-btn.active{color:#c1f0ad;border-color:#446649;background:#1a2820}
  .gb-side-shell{display:grid;gap:8px;align-content:start}
  .gb-mode-bar{display:flex;align-items:center;gap:4px;padding:3px;background:#101617;border:1px solid #2c393a;border-radius:9px}
  .gb-mode-bar button:not(.gb-mode-clear){flex:1;padding:5px 8px;font-size:12px;border-radius:7px;background:transparent;color:#95a5a4}
  .gb-mode-bar button:not(.gb-mode-clear).active{background:#1f2e30;color:#c1f0ad}
  .gb-mode-clear{padding:4px 8px;font-size:11.5px;background:transparent;color:#8f9e9d;border-radius:6px}
  .gb-mode-clear:hover{color:#eff5f4;background:#1c2627}
  .gb-main{display:flex;flex-direction:column;min-height:0;min-width:0}
  .tabs{display:flex;gap:4px;border-bottom:1px solid #2c393a;margin:0 0 16px}
  .tabs [role=tab]{position:relative;padding:10px 14px;font-size:13.5px;color:#a4b0b0;background:transparent;border-radius:8px 8px 0 0;display:inline-flex;align-items:center;gap:7px}
  .tabs [role=tab]:hover{color:#eff5f4;background:#1a2325}.tabs [role=tab].current{color:#c1f0ad;box-shadow:inset 0 -2px 0 #c1f0ad}
  .gb-tab-n{font-size:11px;padding:1px 6px;border-radius:999px;background:#1f2c2e;color:#a6b3b2}
  .tab-dot{display:inline-block;width:7px;height:7px;border-radius:50%;background:#e5c88a}
  .gb-scroll{display:grid;gap:14px;padding-bottom:28px}
  .gb-pin-bar{display:flex;justify-content:space-between;align-items:center;gap:12px;flex-wrap:wrap;padding:10px 14px;border-radius:10px;background:#131b1c;border:1px solid #283536}
  .gb-pin-info{display:grid;gap:2px}
  .gb-pin-info strong{font-size:13px;color:#eff5f4}
  .gb-pin-info small{font-size:11.5px;color:#8f9e9d}
  .gb-pin-btns{display:flex;gap:6px}
  .gb-file-bar{display:flex;gap:8px;align-items:center;flex-wrap:wrap}
  .gb-file-bar input[type=search]{flex:1;min-width:160px;font:inherit;font-size:13px;color:#e8ecec;background:#101617;border:1px solid #314041;border-radius:9px;padding:8px 12px}
  .gb-file-bar select{font:inherit;font-size:12.5px;color:#e8ecec;background:#101617;border:1px solid #314041;border-radius:9px;padding:7px 10px}
  .quiet.active-btn{background:#233528;color:#c1f0ad;border-color:#446649}
  .sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0)}
  .gb-card{background:#161f20;border:1px solid #2c393a;border-radius:12px;padding:16px 18px;display:grid;gap:10px}
  .gb-card.warn{border-color:#5c5030;background:#1f1c13}
  .gb-card h3{margin:0;font-size:14.5px}
  .gb-folder-head{display:flex;justify-content:space-between;align-items:baseline;gap:12px}
  .gb-folder-head small{font-size:12px;color:#8f9e9d}
  .gb-subh{margin:6px 0 0;font-size:11.5px;letter-spacing:.05em;text-transform:uppercase;color:#9fb3aa}
  .gb-warn-text{margin:0;font-size:13px;color:#e5d9a8;line-height:1.5}
  .gb-sub{margin:0;font-size:12.5px;color:#95a5a4;line-height:1.5}
  .gb-list,.gb-rom-list,.gb-save-list{list-style:none;margin:0;padding:0;display:grid;gap:6px}
  .gb-rom-list,.gb-save-list{border:1px solid #2c393a;border-radius:9px;background:#141c1d;gap:0}
  .gb-list li{display:flex;justify-content:space-between;align-items:center;gap:12px;padding:9px 12px;border-radius:9px;background:#121a1b;border:1px solid #232f31;font-size:13px}
  .gb-list small{display:block;font-size:11.5px;color:#8f9e9d;margin-top:2px}
  .gb-rom-list li,.gb-save-list li{border-bottom:1px solid #222d2f}
  .gb-rom-list li:last-child,.gb-save-list li:last-child{border:0}
  .gb-rom-row{display:flex;align-items:center;gap:9px;padding:8px 12px;font-size:12.5px}
  .gb-rom-row.active-game{background:#182421}
  .gb-rom-main{flex:1;min-width:0;display:flex;align-items:center;gap:9px;background:transparent;padding:0;text-align:left;color:inherit;font:inherit;cursor:pointer}
  .gb-rom-main:hover .gb-rom-name{color:#c1f0ad}
  .gb-rom-row input[type=checkbox]{flex:none;accent-color:#c1f0ad;width:15px;height:15px}
  .gb-save-list li{display:flex;align-items:center;gap:9px;padding:9px 12px;font-size:12.5px;cursor:default}
  .gb-view-toggle{display:inline-flex;background:#101617;border:1px solid #314041;border-radius:8px;padding:2px;gap:2px;flex:none}
  .gb-view-toggle button{background:transparent;color:#95a5a4;border-radius:6px;padding:5px 10px;font-size:12px}
  .gb-view-toggle button.active{background:#243335;color:#c1f0ad}
  .gb-fi{flex:none;display:inline-grid;place-items:center;width:28px;height:28px;border-radius:6px;background:#1d2a2b;color:#c1f0ad;overflow:hidden}
  .gb-thumb{width:100%;height:100%;object-fit:cover;display:block}
  .gb-genre{font-size:11px}
  .gb-art-btn{flex:none;padding:2px 8px;font-size:11px;background:#1a2526;color:#9fb3aa;border:1px solid #304142;border-radius:6px;opacity:0;transition:opacity .12s ease}
  .gb-rom-row:hover .gb-art-btn,.gb-art-btn:focus-visible{opacity:1;color:#c1f0ad;border-color:#c1f0ad}
  .gb-rom-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(148px,1fr));gap:12px}
  .gb-rom-card{background:#111819;border:1px solid #2a3738;border-radius:10px;overflow:hidden;display:flex;flex-direction:column;transition:border-color .14s ease}
  .gb-rom-card.selected,.gb-rom-card.active-game{border-color:#c1f0ad;background:#17251d}
  .gb-rom-cover{position:relative;width:100%;aspect-ratio:1/1;background:#0a0f10;overflow:hidden}
  .gb-cover-hit{width:100%;height:100%;padding:0;background:transparent;display:grid;place-items:center;cursor:pointer}
  .gb-rom-cover img{width:100%;height:100%;object-fit:cover;display:block}
  .gb-cover-fallback{color:#6d807f}
  .gb-card-pick{position:absolute;top:8px;left:8px;width:16px;height:16px;accent-color:#c1f0ad;cursor:pointer;z-index:2}
  .gb-card-art{position:absolute;top:8px;right:8px;padding:2px 8px;font-size:11px;background:rgba(16,22,23,.88);color:#eff5f4;border:1px solid #3c4d4e;border-radius:6px;opacity:0;transition:opacity .12s ease;z-index:2}
  .gb-rom-card:hover .gb-card-art,.gb-rom-card:focus-within .gb-card-art{opacity:1}
  .gb-card-art:hover{border-color:#c1f0ad;color:#c1f0ad}
  .gb-rom-card-meta{display:grid;gap:2px;min-width:0;padding:8px 10px 10px;background:transparent;text-align:left;font:inherit;cursor:pointer}
  .gb-rom-card-meta strong{font-size:12.5px;color:#eff5f4;display:-webkit-box;-webkit-line-clamp:2;line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;line-height:1.3}
  .gb-rom-card-meta small{font-size:11px;color:#8f9e9d;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .gb-rom-name{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:#e8ecec}
  .gb-save-main{flex:1;min-width:0;display:grid;gap:2px}
  .gb-save-main strong{font-size:13px;color:#eff5f4;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .gb-save-main small{font-size:11px;color:#8f9e9d;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .gb-rom-size{width:68px;text-align:right;color:#a6b3b2;flex:none}
  .gb-rom-date{width:92px;text-align:right;color:#8f9e9d;font-size:11.5px;flex:none}
  .gb-unmatched-bar{display:grid;gap:8px;justify-items:start}
  .gb-empty{display:grid;justify-items:center;text-align:center;gap:8px;padding:28px 20px;border:1px dashed #334345;border-radius:12px;background:#121a1b}
  .gb-empty.compact{padding:22px 18px}
  .gb-empty-ic{width:40px;height:40px;border-radius:12px;background:#1b2729;border:1px solid #2c3b3d;color:#c1f0ad;display:grid;place-items:center;font-weight:700}
  .gb-empty strong{font-size:14px;color:#e8ecec}
  .gb-empty p{margin:0;max-width:420px;font-size:12.5px;color:#95a5a4;line-height:1.55}
  .gb-side{background:#161f20;border:1px solid #2c393a;border-radius:12px;overflow:hidden;display:grid;align-content:start}
  .gb-side-body{padding:16px;display:grid;gap:12px}
  .gb-side-remove{padding-top:10px;border-top:1px solid #263334}
  .danger-quiet{background:none;border:1px solid #6b3a3a;color:#ff9b9b;border-radius:8px;padding:7px 12px;font:inherit;font-size:12.5px;cursor:pointer}
  .danger-quiet:hover{background:#2b1717}
  .gb-art-wrap{position:relative;background:#0b1011;border-bottom:1px solid #2c393a}
  .gb-banner{display:block;width:100%;aspect-ratio:16/7;object-fit:cover;background:#0b1011}
  .gb-art-edit{position:absolute;right:8px;bottom:8px;padding:4px 10px;font-size:11.5px;background:rgba(16,22,23,.86);color:#eff5f4;border:1px solid #3c4d4e;border-radius:7px;opacity:0;transition:opacity .15s ease}
  .gb-art-wrap:hover .gb-art-edit,.gb-art-wrap:focus-within .gb-art-edit{opacity:1}
  .gb-art-edit:hover{border-color:#c1f0ad;color:#c1f0ad}
  .gb-side-title strong{display:block;font-size:15px}.gb-side-title small{font-size:12px;color:#8f9e9d}
  .gb-desc{margin:0;font-size:12.5px;color:#c7d1d0;line-height:1.55}
  .gb-dl{display:grid;gap:8px;margin:0}.gb-dl>div{display:flex;justify-content:space-between;gap:12px;font-size:12.5px}
  .gb-dl dt{color:#8f9e9d}.gb-dl dd{margin:0;color:#e8ecec;text-align:right;overflow-wrap:anywhere}
  .gb-side h4{margin:4px 0 0;font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:#9fb3aa}
  .gb-targets{list-style:none;margin:0;padding:0;display:grid;gap:6px}
  .gb-targets li{display:flex;justify-content:space-between;align-items:center;gap:8px;padding:7px 10px;border-radius:8px;background:#121a1b;border:1px solid #232f31;font-size:12.5px}
  .gb-targets small{display:block;font-size:11px;color:#8f9e9d}
  @media(max-width:1100px){.gb-grid{grid-template-columns:minmax(0,1fr)}}
  @media(max-width:720px){.gb-rom-date{display:none}}
</style>
