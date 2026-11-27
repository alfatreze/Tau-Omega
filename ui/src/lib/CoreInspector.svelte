<script lang="ts">
  // Marketplace-style right-hand Core Details panel:
  // full-bleed banner, core icon/author/description, clickable GitHub link, version switcher
  // (update / downgrade / reinstall via reviewed plan), compact hardware support, card totals,
  // and safe core removal (prompting whether to also remove ROMs when removing the last core for a platform).
  import { onDestroy, onMount } from 'svelte';
  import ArtworkPickerSheet from './ArtworkPickerSheet.svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import CoreInstall from './CoreInstall.svelte';
  import { plural, size } from './coresFormat';
  import { refreshCores } from './coresStore';
  import { platformFallbackArt } from './platformArt';
  import RefreshLibrary from './RefreshLibrary.svelte';
  import {
    executeRemoveCore,
    marketplaceCatalog,
    marketplaceOpenRepo,
    planRemoveCore,
    readCoreIcon,
    readPlatformImage,
  } from './tau-api';
  import { showToast } from './toastStore';
  import type { CatalogCore, Core, CoreDetail, CoreFiles, RemovePlan } from './types';

  export let card: string;
  export let detail: CoreDetail;
  export let files: CoreFiles | null = null;
  export let lib: Core | null = null;
  export let openLibrary: (c: Core) => void = () => {};
  export let libraryChanged: () => void = () => {};
  export let onCoreRemoved: () => void = () => {};
  export let onCoreUpdated: () => void = () => {};

  let icon: string | null = null;
  $: void loadIcon(detail.id);
  async function loadIcon(id: string) {
    icon = null;
    try {
      icon = await readCoreIcon(card, id);
    } catch {
      icon = null;
    }
  }
  onMount(() => {
    void loadIcon(detail.id);
    void loadCatalog();
  });

  let banner: string | null = null;
  let showArtPicker = false;
  $: primaryPlatform = detail.platforms[0];
  $: bannerPlatform = detail.platforms.find((p) => p.has_image)?.id ?? primaryPlatform?.id ?? '';
  $: void loadBanner(bannerPlatform);
  async function loadBanner(id: string) {
    banner = null;
    if (!id) return;
    try {
      banner = await readPlatformImage(card, id);
    } catch {
      banner = null;
    }
  }
  function onArtChanged(e: Event) {
    const pid = (e as CustomEvent<{ platformId?: string; batch?: boolean }>).detail?.platformId;
    if (bannerPlatform && (!pid || pid === bannerPlatform)) void loadBanner(bannerPlatform);
  }
  onMount(() => {
    window.addEventListener('tau-artwork-changed', onArtChanged);
  });
  onDestroy(() => {
    window.removeEventListener('tau-artwork-changed', onArtChanged);
  });

  // Marketplace catalog match for Update / Downgrade controls + rich GitHub link.
  let catalogMatch: CatalogCore | null = null;
  let chosenVersion = '';
  let showVersionSheet = false;

  async function loadCatalog() {
    try {
      const cat = await marketplaceCatalog(false);
      findMatch(cat.catalog.cores, detail.id);
    } catch {
      catalogMatch = null;
    }
  }
  function findMatch(list: CatalogCore[], id: string) {
    const hit = list.find((c) => c.id.toLowerCase() === id.toLowerCase()) ?? null;
    catalogMatch = hit;
    chosenVersion = hit?.version ?? '';
  }
  $: if (detail.id) {
    removePlan = null;
    removeMedia = false;
    removeError = '';
    showVersionSheet = false;
    void marketplaceCatalog(false)
      .then((c) => findMatch(c.catalog.cores, detail.id))
      .catch(() => (catalogMatch = null));
  }

  $: versions = (() => {
    if (!catalogMatch) return [];
    const seen = new Set<string>();
    const list: { version: string; date: string; isNewest: boolean }[] = [];
    if (catalogMatch.version) {
      seen.add(catalogMatch.version);
      list.push({ version: catalogMatch.version, date: catalogMatch.release_date, isNewest: true });
    }
    for (const h of catalogMatch.history) {
      if (h.version && !seen.has(h.version)) {
        seen.add(h.version);
        list.push({ version: h.version, date: h.release_date, isNewest: false });
      }
    }
    return list;
  })();

  function cmpSemver(a: string, b: string): number {
    const pa = a.replace(/^v/i, '').split('.').map((x) => parseInt(x, 10) || 0);
    const pb = b.replace(/^v/i, '').split('.').map((x) => parseInt(x, 10) || 0);
    for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
      const diff = (pa[i] ?? 0) - (pb[i] ?? 0);
      if (diff !== 0) return diff;
    }
    return 0;
  }

  $: installedVer = detail.version || '';
  $: newestVer = catalogMatch?.version || '';
  $: hasUpdate = !!newestVer && !!installedVer && cmpSemver(newestVer, installedVer) > 0;
  $: selectedDiff = chosenVersion && installedVer ? cmpSemver(chosenVersion, installedVer) : 0;
  $: versionActionLabel = selectedDiff > 0
    ? `Update to ${chosenVersion}`
    : selectedDiff < 0
      ? `Downgrade to ${chosenVersion}`
      : `Reinstall ${chosenVersion || newestVer}`;

  function githubOwnerRepo(raw: string): { owner: string; name: string } | null {
    const m = /^https?:\/\/github\.com\/([^/]+)\/([^/#?]+)/i.exec(raw.trim());
    if (!m) return null;
    return { owner: m[1], name: m[2].replace(/\.git$/i, '') };
  }
  $: repoSlug = detail.url ? githubOwnerRepo(detail.url) : null;
  let linkError = '';
  let copied = false;
  async function openSourceRepo(e: MouseEvent) {
    if (!catalogMatch) return;
    e.preventDefault();
    linkError = '';
    try {
      await marketplaceOpenRepo(catalogMatch.id);
    } catch (err) {
      linkError = errorMessage(err);
    }
  }
  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      setTimeout(() => (copied = false), 1400);
    } catch {
      /* stays selectable */
    }
  }

  $: roms = files
    ? files.folders
        .filter((f) => f.folder.role === 'AssetsShared' || f.folder.role === 'AssetsCore')
        .reduce(
          (n, f) =>
            n +
            f.entries.filter((e) => e.kind === 'Rom' || e.kind === 'Disk' || e.kind === 'Tape')
              .length,
          0,
        )
    : 0;
  $: saves = files
    ? files.folders
        .filter((f) => f.folder.role === 'SavesShared' || f.folder.role === 'SavesCore')
        .reduce((n, f) => n + f.total_files, 0)
    : 0;
  $: states = files
    ? (files.folders.find((f) => f.folder.role === 'SaveStates')?.total_files ?? 0)
    : 0;
  $: bytes = files ? files.folders.reduce((n, f) => n + f.total_bytes, 0) : 0;

  // Core removal flow (with shared ROMs/files opt-in when this is the last core for the platform).
  let removePlan: RemovePlan | null = null;
  let removeMedia = false;
  let removeBusy = false;
  let removeError = '';

  async function startRemove(alsoRemoveMedia = false) {
    removeBusy = true;
    removeError = '';
    try {
      removePlan = await planRemoveCore(card, detail.id, !alsoRemoveMedia);
    } catch (e) {
      removeError = errorMessage(e);
      removePlan = null;
    } finally {
      removeBusy = false;
    }
  }

  async function toggleRemoveMedia() {
    removeMedia = !removeMedia;
    await startRemove(removeMedia);
  }

  async function confirmRemove() {
    if (!removePlan) return;
    removeBusy = true;
    removeError = '';
    const keepMedia = !removeMedia;
    const removedName = detail.shortname || detail.id;
    try {
      const rep = await executeRemoveCore(card, detail.id, removePlan.id, keepMedia);
      removePlan = null;
      await refreshCores(card);
      showToast(
        `Removed ${removedName} (${plural(rep.removed_files, 'file')}, backed up outside card).`,
      );
      onCoreRemoved();
    } catch (e) {
      removeError = errorMessage(e);
      showToast(removeError, 'error');
    } finally {
      removeBusy = false;
    }
  }
</script>

<aside class="ci" aria-labelledby="ci-title">
  <div class="art-wrap">
    <img
      class="ci-banner"
      src={banner ||
        platformFallbackArt({
          id: primaryPlatform?.id ?? detail.id,
          name: primaryPlatform?.name ?? detail.shortname,
          category: primaryPlatform?.category,
          manufacturer: primaryPlatform?.manufacturer,
        })}
      alt={`${primaryPlatform?.name ?? detail.shortname} artwork`}
    />
    {#if primaryPlatform?.id}
      <button type="button" class="art-edit" on:click={() => (showArtPicker = true)}>
        Change image
      </button>
    {/if}
  </div>

  <div class="ci-body">
    <header>
      <span class="ci-icon" aria-hidden="true">
        {#if icon}
          <img src={icon} alt="" width="40" height="40" />
        {:else}
          {detail.shortname.slice(0, 2).toUpperCase()}
        {/if}
      </span>
      <div class="ci-head">
        <h2 id="ci-title">{detail.shortname || detail.id}</h2>
        <p>
          {detail.author || detail.id}{#if primaryPlatform} · {primaryPlatform.name}{#if primaryPlatform.year} ({primaryPlatform.year}){/if}{/if}
        </p>
      </div>
    </header>

    {#if detail.description || catalogMatch?.description}
      <p class="ci-desc">{detail.description || catalogMatch?.description}</p>
    {/if}

    <dl class="ci-facts">
      <div>
        <dt>Version</dt>
        <dd>
          <span>{detail.version || 'not stated'}{detail.date_release ? ` · ${detail.date_release}` : ''}</span>
          {#if hasUpdate}
            <span class="chip capable">Update {newestVer}</span>
          {/if}
        </dd>
      </div>
      {#if detail.url}
        <div>
          <dt>Source</dt>
          <dd>
            {#if repoSlug}
              <a class="ci-link" href={detail.url} on:click={openSourceRepo}>
                {repoSlug.owner}/{repoSlug.name} on GitHub
              </a>
              {#if linkError}<span class="ci-err"> {linkError}</span>{/if}
            {:else}
              <code>{detail.url}</code>
              <button type="button" class="quiet ci-copy" on:click={() => copy(detail.url)}>
                {copied ? 'Copied' : 'Copy'}
              </button>
            {/if}
          </dd>
        </div>
      {/if}
      {#if detail.platforms.length > 1}
        <div>
          <dt>Platforms</dt>
          <dd>{detail.platforms.map((p) => p.name).join(', ')}</dd>
        </div>
      {/if}
    </dl>

    {#if versions.length > 0}
      <div class="ci-ver-box">
        <div class="ci-ver-head">
          <h3>Version &amp; updates</h3>
          {#if hasUpdate}
            <span class="chip capable">{newestVer} available</span>
          {:else if newestVer && installedVer === newestVer}
            <span class="chip">Up to date</span>
          {/if}
        </div>
        <div class="ci-ver-controls">
          <select aria-label="Choose core version" bind:value={chosenVersion}>
            {#each versions as v (v.version)}
              <option value={v.version}>
                {v.version}{v.date ? ` · ${v.date}` : ''}{v.isNewest ? ' (newest)' : ''}{v.version === installedVer ? ' · installed' : ''}
              </option>
            {/each}
          </select>
          <button
            type="button"
            class={selectedDiff > 0 ? 'primary' : 'quiet'}
            on:click={() => (showVersionSheet = true)}
          >
            {versionActionLabel}
          </button>
        </div>
      </div>
    {/if}

    {#if lib?.library_capable}
      <h3>Music library</h3>
      <dl class="ci-facts">
        <div><dt>Library</dt><dd>Library ready</dd></div>
        <div><dt>Index</dt><dd>{lib.index_status}</dd></div>
        <div><dt>Tracks</dt><dd>{lib.tracks ?? '—'}</dd></div>
      </dl>
      <div class="ci-lib">
        <button type="button" class="primary" on:click={() => openLibrary(lib)}>Open library</button>
      </div>
      {#if lib.media_platform}
        {#key lib.id}
          <RefreshLibrary
            mediaRoot={`${card.replace(/\/$/, '')}/Assets/${lib.media_platform}/common`}
            coreName={lib.shortname || lib.id}
            onChanged={libraryChanged}
          />
        {/key}
      {/if}
    {:else if lib && lib.audio_formats.length}
      <h3>Music</h3>
      <dl class="ci-facts">
        <div><dt>Plays</dt><dd>{lib.audio_formats.join(' + ').toUpperCase()}</dd></div>
      </dl>
      <div class="ci-lib">
        <button type="button" class="quiet" on:click={() => openLibrary(lib)}>Use as the Library target</button>
      </div>
    {/if}

    <h3>Requirements &amp; hardware</h3>
    <dl class="ci-facts">
      <div>
        <dt>Pocket firmware</dt>
        <dd>{detail.firmware_required ? `${detail.firmware_required} or newer` : 'Not stated'}</dd>
      </div>
    </dl>
    <div class="ci-caps" aria-label="Supported features">
      <span class="chip" class:capable={detail.dock_supported === true}>
        Dock: {detail.dock_supported === true ? 'Yes' : detail.dock_supported === false ? 'No' : '—'}
      </span>
      <span class="chip" class:capable={detail.link_port === true}>
        Link port: {detail.link_port === true ? 'Yes' : detail.link_port === false ? 'No' : '—'}
      </span>
      <span class="chip" class:capable={detail.sleep_supported === true}>
        Sleep: {detail.sleep_supported === true ? 'Yes' : detail.sleep_supported === false ? 'No' : '—'}
      </span>
    </div>

    <h3>On this card</h3>
    {#if files}
      <dl class="ci-facts">
        <div><dt>Games</dt><dd>{plural(roms, 'file')}</dd></div>
        <div><dt>Saves</dt><dd>{plural(saves, 'save')} · {plural(states, 'save state')}</dd></div>
        <div><dt>Disk used</dt><dd>{size(bytes)}</dd></div>
      </dl>
    {:else}
      <p class="ci-note" role="status">Reading the card…</p>
    {/if}

    <div class="ci-remove-sec">
      {#if removeError}<p class="ci-err" role="alert">{removeError}</p>{/if}
      {#if removePlan}
        <div class="ci-rm-plan" role="region" aria-label="Remove core review">
          <strong>Remove {detail.shortname || detail.id}?</strong>
          <p>
            Removes {plural(removePlan.files_to_remove, 'file')} ({size(removePlan.bytes_to_remove)}).
            Everything removed is backed up outside the card first.
          </p>
          {#if !removePlan.platform_shared && removePlan.platform}
            <label class="ci-rm-opt">
              <input
                type="checkbox"
                checked={removeMedia}
                disabled={removeBusy}
                on:change={toggleRemoveMedia}
              />
              <span>
                This is the last {primaryPlatform?.name || removePlan.platform} core on this card. Also remove ROMs and files in <code>Assets/{removePlan.platform}/common</code>.
              </span>
            </label>
          {/if}
          <div class="ci-rm-btns">
            <button type="button" class="primary danger" disabled={removeBusy} on:click={confirmRemove}>
              {removeBusy ? 'Removing…' : 'Remove core'}
            </button>
            <button
              type="button"
              class="quiet"
              disabled={removeBusy}
              on:click={() => {
                removePlan = null;
                removeMedia = false;
              }}
            >
              Cancel
            </button>
          </div>
        </div>
      {:else}
        <button type="button" class="danger-quiet" disabled={removeBusy} on:click={() => startRemove(false)}>
          {removeBusy ? 'Planning…' : 'Remove core…'}
        </button>
      {/if}
    </div>
  </div>
</aside>

{#if showArtPicker && primaryPlatform?.id}
  <ArtworkPickerSheet
    {card}
    device="analogue_pocket"
    title={`Artwork — ${primaryPlatform.name || primaryPlatform.id}`}
    platformId={primaryPlatform.id}
    targetId={primaryPlatform.id}
    currentUrl={banner}
    onApplied={(u) => {
      if (u) banner = u;
    }}
    close={() => (showArtPicker = false)}
  />
{/if}

{#if showVersionSheet && catalogMatch}
  <div class="ci-upd-backdrop" role="presentation" on:click={() => (showVersionSheet = false)}></div>
  <div
    class="ci-upd-sheet"
    role="dialog"
    aria-modal="true"
    aria-label={`Change version for ${detail.shortname || detail.id}`}
    use:modal
  >
    <div class="ci-upd-head">
      <div>
        <p class="eyebrow">CORE VERSION</p>
        <h3>{versionActionLabel}</h3>
      </div>
      <button type="button" class="quiet" aria-label="Close" on:click={() => (showVersionSheet = false)}>
        ✕
      </button>
    </div>
    <div class="ci-upd-body">
      {#key `${catalogMatch.id}|${chosenVersion}`}
        <CoreInstall
          coreId={catalogMatch.id}
          {card}
          version={chosenVersion === newestVer ? '' : chosenVersion}
          label={`Review ${versionActionLabel}`}
          on:installed={() => {
            showVersionSheet = false;
            showToast(`Updated ${detail.shortname || detail.id} to ${chosenVersion || newestVer}.`);
            onCoreUpdated();
          }}
        />
      {/key}
    </div>
  </div>
{/if}

<style>
  .ci {
    background: #161f20;
    border: 1px solid #2c393a;
    border-radius: 12px;
    overflow: hidden;
    display: grid;
    align-content: start;
  }
  .art-wrap {
    position: relative;
    background: #000;
    border-bottom: 1px solid #2c393a;
  }
  .ci-banner {
    display: block;
    width: 100%;
    height: auto;
  }
  .art-edit {
    position: absolute;
    right: 10px;
    bottom: 10px;
    padding: 4px 10px;
    font-size: 11.5px;
    background: rgba(16, 22, 23, 0.88);
    color: #eff5f4;
    border: 1px solid #3c4d4e;
    border-radius: 7px;
    opacity: 0;
    transition: opacity 0.15s ease;
  }
  .art-wrap:hover .art-edit,
  .art-wrap:focus-within .art-edit {
    opacity: 1;
  }
  .art-edit:hover {
    border-color: #c1f0ad;
    color: #c1f0ad;
  }
  .ci-body {
    padding: 16px;
    display: grid;
    gap: 10px;
    align-content: start;
  }
  .ci header {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .ci-head {
    flex: 1;
    min-width: 0;
  }
  .ci-head h2 {
    margin: 0;
    font-size: 16px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ci-head p {
    margin: 2px 0 0;
    font-size: 11.5px;
    color: #8f9e9d;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ci-icon {
    flex: none;
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    background: #273638;
    border-radius: 10px;
    color: #c1f0ad;
    font-weight: 700;
  }
  .ci-icon img {
    width: 36px;
    height: 36px;
    image-rendering: pixelated;
  }
  .ci-desc {
    margin: 0;
    font-size: 12.5px;
    color: #c7d1d0;
    line-height: 1.55;
  }
  .ci h3 {
    margin: 8px 0 0;
    font-size: 11px;
    letter-spacing: 0.09em;
    color: #9fb3aa;
    text-transform: uppercase;
  }
  .ci-facts {
    display: grid;
    gap: 7px;
    margin: 0;
  }
  .ci-facts > div {
    display: grid;
    grid-template-columns: 98px 1fr;
    gap: 10px;
    align-items: baseline;
  }
  .ci-facts dt {
    font-size: 12px;
    color: #8f9e9d;
  }
  .ci-facts dd {
    margin: 0;
    font-size: 12.5px;
    color: #e8ecec;
    overflow-wrap: anywhere;
    line-height: 1.45;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .ci-link {
    color: #c1f0ad;
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .ci-link:hover {
    color: #eff5f4;
  }
  .ci-copy {
    padding: 2px 8px;
    font-size: 11px;
  }
  .ci-ver-box {
    display: grid;
    gap: 8px;
    padding: 11px 12px;
    border-radius: 10px;
    background: #111819;
    border: 1px solid #283536;
  }
  .ci-ver-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }
  .ci-ver-head h3 {
    margin: 0;
  }
  .ci-ver-controls {
    display: grid;
    gap: 8px;
  }
  .ci-ver-controls select {
    width: 100%;
    font: inherit;
    font-size: 12.5px;
    color: #e8ecec;
    background: #0d1314;
    border: 1px solid #354546;
    border-radius: 8px;
    padding: 7px 10px;
  }
  .ci-caps {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .ci-lib {
    display: flex;
    gap: 8px;
    margin: 4px 0 2px;
  }
  .ci-note {
    font-size: 12px;
    color: #95a5a4;
    margin: 0;
  }
  .ci-remove-sec {
    padding-top: 10px;
    border-top: 1px solid #263334;
    display: grid;
    gap: 8px;
  }
  .ci-rm-plan {
    padding: 11px 12px;
    border-radius: 10px;
    border: 1px solid #6b3535;
    background: #241515;
    display: grid;
    gap: 8px;
  }
  .ci-rm-plan p {
    margin: 0;
    font-size: 12px;
    color: #e3d6d5;
    line-height: 1.5;
  }
  .ci-rm-opt {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: 12px;
    color: #f0d7a8;
    line-height: 1.45;
    cursor: pointer;
  }
  .ci-rm-opt input {
    margin-top: 2px;
    accent-color: #c1f0ad;
  }
  .ci-rm-btns {
    display: flex;
    gap: 8px;
  }
  .danger-quiet {
    background: none;
    border: 1px solid #6b3a3a;
    color: #ff9b9b;
    border-radius: 8px;
    padding: 7px 12px;
    font: inherit;
    font-size: 12.5px;
    cursor: pointer;
    justify-self: start;
  }
  .danger-quiet:hover {
    background: #2b1717;
  }
  .ci-err {
    margin: 0;
    font-size: 12px;
    color: #ff9b9b;
  }
  .ci-upd-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(6, 10, 11, 0.72);
    backdrop-filter: blur(4px);
    z-index: 65;
  }
  .ci-upd-sheet {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(640px, calc(100vw - 32px));
    max-height: min(86vh, 720px);
    display: flex;
    flex-direction: column;
    background: #141c1d;
    border: 1px solid #304142;
    border-radius: 14px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    z-index: 66;
    overflow: hidden;
  }
  .ci-upd-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    padding: 16px 20px;
    border-bottom: 1px solid #283536;
  }
  .ci-upd-head h3 {
    margin: 2px 0 0;
    font-size: 17px;
  }
  .ci-upd-body {
    padding: 16px 20px;
    overflow: auto;
  }
  code {
    font-size: 11px;
    background: #101617;
    border-radius: 5px;
    padding: 1px 5px;
    overflow-wrap: anywhere;
  }
</style>
