<script lang="ts">
  // The core's details (the right-hand panel). Facts come straight from the core's own files on the card; nothing here is a guess.
  import { createEventDispatcher, onMount } from 'svelte';
  import { readCoreIcon, readPlatformImage } from './tau-api';
  import { plural, size } from './coresFormat';
  import type { CoreDetail, CoreFiles } from './types';
  export let card: string; export let detail: CoreDetail; export let files: CoreFiles | null = null;
  const dispatch = createEventDispatcher<{ close: void }>();

  let icon: string | null = null;
  $: void loadIcon(detail.id);
  async function loadIcon(id: string) { icon = null; try { icon = await readCoreIcon(card, id); } catch { icon = null; } }
  onMount(() => { void loadIcon(detail.id); });
  // The core's picture: its platform's banner (Platforms/_images), shown above the facts.
  let banner: string | null = null;
  $: bannerPlatform = detail.platforms.find((p) => p.has_image)?.id ?? '';
  $: void loadBanner(bannerPlatform);
  async function loadBanner(id: string) { banner = null; if (!id) return; try { banner = await readPlatformImage(card, id); } catch { banner = null; } }

  const yn = (v: boolean | null) => (v === true ? 'Yes' : v === false ? 'No' : 'Not stated');
  let copied = false;
  async function copy(text: string) { try { await navigator.clipboard.writeText(text); copied = true; setTimeout(() => (copied = false), 1400); } catch { /* the text stays visible to select */ } }
  $: roms = files ? files.folders.filter((f) => f.folder.role === 'AssetsShared' || f.folder.role === 'AssetsCore').reduce((n, f) => n + f.entries.filter((e) => e.kind === 'Rom' || e.kind === 'Disk' || e.kind === 'Tape').length, 0) : 0;
  $: saves = files ? files.folders.filter((f) => f.folder.role === 'SavesShared' || f.folder.role === 'SavesCore').reduce((n, f) => n + f.total_files, 0) : 0;
  $: states = files ? (files.folders.find((f) => f.folder.role === 'SaveStates')?.total_files ?? 0) : 0;
  $: bytes = files ? files.folders.reduce((n, f) => n + f.total_bytes, 0) : 0;
</script>
<aside class="ci" aria-labelledby="ci-title">
  <header>
    <span class="ci-icon" aria-hidden="true">{#if icon}<img src={icon} alt="" width="40" height="40" />{:else}{detail.shortname.slice(0, 2).toUpperCase()}{/if}</span>
    <div class="ci-head"><h2 id="ci-title">{detail.shortname || detail.id}</h2><p>{detail.id}</p></div>
    <button class="quiet" on:click={() => dispatch('close')} aria-label="Hide core details" title="Hide details (I)">Hide</button>
  </header>
  {#if banner}<img class="ci-banner" src={banner} alt={`${detail.platforms.find((p) => p.has_image)?.name ?? 'Platform'} artwork`} />{/if}
  {#if detail.description}<p class="ci-desc">{detail.description}</p>{/if}

  <dl class="ci-facts">
    <div><dt>Version</dt><dd>{detail.version || 'not stated'}{detail.date_release ? ` · ${detail.date_release}` : ''}</dd></div>
    <div><dt>Author</dt><dd>{detail.author || 'not stated'}</dd></div>
    {#if detail.url}<div><dt>Source</dt><dd><code>{detail.url}</code> <button class="quiet" on:click={() => copy(detail.url)}>{copied ? 'Copied' : 'Copy'}</button></dd></div>{/if}
    <div><dt>Platform</dt><dd>{detail.platforms.length ? detail.platforms.map((p) => p.name + (p.year ? ` (${p.manufacturer ? p.manufacturer + ', ' : ''}${p.year})` : '')).join(', ') : 'None declared'}</dd></div>
  </dl>

  <h3>Requirements</h3>
  <dl class="ci-facts">
    <div><dt>Pocket firmware</dt><dd>{detail.firmware_required ? `${detail.firmware_required} or newer` : 'Not stated'}</dd></div>
    <div><dt>Dock</dt><dd>{yn(detail.dock_supported)}</dd></div>
    <div><dt>Link port</dt><dd>{yn(detail.link_port)}</dd></div>
    <div><dt>Sleep</dt><dd>{yn(detail.sleep_supported)}</dd></div>
  </dl>

  <h3>On this card</h3>
  {#if files}
    <dl class="ci-facts">
      <div><dt>Games</dt><dd>{plural(roms, 'file')}</dd></div>
      <div><dt>Saves</dt><dd>{plural(saves, 'save')} · {plural(states, 'save state')}</dd></div>
      <div><dt>Disk used</dt><dd>{size(bytes)}</dd></div>
      <div><dt>Needs</dt><dd>{files.missing.length ? files.missing.map((m) => m.filename).join(', ') : 'Nothing'}</dd></div>
    </dl>
    <ul class="ci-folders" aria-label="Folders on the card">{#each files.folders as f}<li><code>{f.folder.rel_path}</code><span>{f.exists ? plural(f.total_files, 'file') : 'not created'}</span></li>{/each}</ul>
  {:else}<p class="ci-note" role="status">Reading the card…</p>{/if}
</aside>
<style>
  .ci{background:#161f20;border:1px solid #2c393a;border-radius:12px;padding:16px;display:grid;gap:10px;align-content:start}
  .ci header{display:flex;align-items:center;gap:12px}.ci-head{flex:1;min-width:0}.ci-head h2{margin:0;font-size:16px;overflow:hidden;text-overflow:ellipsis}.ci-head p{margin:2px 0 0;font-size:11.5px;color:#8f9e9d;overflow:hidden;text-overflow:ellipsis}
  .ci-icon{flex:none;width:44px;height:44px;display:grid;place-items:center;background:#273638;border-radius:10px;color:#c1f0ad;font-weight:700}.ci-icon img{width:36px;height:36px;image-rendering:pixelated}
  .ci-banner{display:block;width:100%;height:auto;border-radius:9px;border:1px solid #2c393a;background:#000}
  .ci-desc{margin:0;font-size:12.5px;color:#c7d1d0;line-height:1.55}
  .ci h3{margin:10px 0 0;font-size:11px;letter-spacing:.09em;color:#9fb3aa;text-transform:uppercase}
  .ci-facts{display:grid;gap:7px;margin:0}.ci-facts>div{display:grid;grid-template-columns:104px 1fr;gap:10px;align-items:baseline}.ci-facts dt{font-size:12px;color:#8f9e9d}.ci-facts dd{margin:0;font-size:12.5px;color:#e8ecec;overflow-wrap:anywhere;line-height:1.45}
  .ci-folders{list-style:none;margin:6px 0 0;padding:0;display:grid;gap:4px}.ci-folders li{display:flex;justify-content:space-between;gap:8px;font-size:11.5px;color:#8f9e9d}
  .ci-note{font-size:12px;color:#95a5a4}code{font-size:11px;background:#101617;border-radius:5px;padding:1px 5px;overflow-wrap:anywhere}
</style>
