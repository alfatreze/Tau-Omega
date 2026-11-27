<script lang="ts">
  // Right-hand inspector panel for a selected game ROM: edge-to-edge hero cover, metadata from
  // Libretro Database, multi-image gallery (Boxart, Title screen, Screenshot) with Lightbox,
  // matching saves/save states on the card, and Change artwork / Remove file actions.
  import { onDestroy, onMount } from 'svelte';
  import { errorMessage } from './backend';
  import { day, plural, size } from './coresFormat';
  import { displayGameTitle, gameMetaMap } from './gameArtStore';
  import Lightbox from './Lightbox.svelte';
  import { gameDetailsRead } from './tau-api';
  import type { GameDetailsView } from './types';

  export let platformId = '';
  export let platformName = '';
  export let romName = '';
  export let relPath = '';
  export let bytes = 0;
  export let modified = 0;
  export let saves: {
    name: string;
    label: string;
    kind: 'save' | 'state';
    bytes: number;
    modified: number;
  }[] = [];
  export let canRemove = true;
  export let onOpenArtwork: () => void = () => {};
  export let onRemove: () => void = () => {};

  let details: GameDetailsView | null = null;
  let loading = true;
  let error = '';
  let loadedFor = '';
  let lightboxIndex: number | null = null;

  async function load(plat: string, rom: string) {
    loadedFor = `${plat}|${rom}`;
    loading = true;
    error = '';
    details = null;
    lightboxIndex = null;
    if (!plat || !rom) {
      loading = false;
      return;
    }
    try {
      const res = await gameDetailsRead(plat, rom);
      if (loadedFor === `${plat}|${rom}`) details = res;
    } catch (e) {
      if (loadedFor === `${plat}|${rom}`) error = errorMessage(e);
    } finally {
      if (loadedFor === `${plat}|${rom}`) loading = false;
    }
  }

  $: if (`${platformId}|${romName}` !== loadedFor) void load(platformId, romName);

  function onArtChanged(e: Event) {
    const d = (e as CustomEvent<{ platformId?: string; romName?: string }>).detail;
    if (d?.romName === romName) void load(platformId, romName);
  }
  onMount(() => {
    window.addEventListener('tau-game-art-changed', onArtChanged);
  });
  onDestroy(() => {
    window.removeEventListener('tau-game-art-changed', onArtChanged);
  });

  $: cachedMeta = platformId
    ? $gameMetaMap[`${platformId.trim().toLowerCase()}::${romName.trim()}`]
    : undefined;
  $: genre = details?.meta.genre || cachedMeta?.genre || null;
  $: region = details?.meta.region || cachedMeta?.region || null;
  $: hero = details?.images[0]?.data_url ?? null;
  $: galleryItems = (details?.images ?? []).map((img) => ({
    src: img.data_url,
    alt: `${kindLabel(img.kind)} · ${img.name}`,
  }));

  function kindLabel(k: string): string {
    if (k === 'title') return 'Title screen';
    if (k === 'snap') return 'Screenshot';
    return 'Boxart';
  }
</script>

<aside class="gi" aria-labelledby="gi-title">
  <div class="gi-hero" class:clickable={!!hero}>
    {#if hero}
      <button
        type="button"
        class="gi-hero-img"
        aria-label="Open cover image larger"
        on:click={() => (lightboxIndex = 0)}
      >
        <img src={hero} alt={displayGameTitle(romName)} />
      </button>
    {:else}
      <div class="gi-hero-empty" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="38" height="38" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
          <rect x="2" y="6" width="20" height="12" rx="4" />
          <path d="M6 12h4M8 10v4" />
          <circle cx="15" cy="11" r="1" />
          <circle cx="18" cy="13" r="1" />
        </svg>
      </div>
    {/if}
    <button type="button" class="gi-art-edit" on:click={onOpenArtwork}>Change image</button>
  </div>

  <div class="gi-body">
    <div class="gi-head">
      <h2 id="gi-title">{details?.display_title || displayGameTitle(romName) || romName}</h2>
      <div class="gi-badges">
        {#if platformName}<span class="chip">{platformName}</span>{/if}
        {#if genre}<span class="chip capable">{genre}</span>{/if}
        {#if region}<span class="chip">{region}</span>{/if}
      </div>
    </div>

    {#if loading && !details}
      <p class="gi-note" role="status">Loading game details…</p>
    {:else if error}
      <p class="gi-note">{error}</p>
    {/if}

    <h3>About this game</h3>
    <dl class="gi-facts">
      {#if genre}<div><dt>Category</dt><dd>{genre}</dd></div>{/if}
      {#if details?.meta.release_year}<div><dt>Released</dt><dd>{details.meta.release_year}</dd></div>{/if}
      {#if details?.meta.developer}<div><dt>Developer</dt><dd>{details.meta.developer}</dd></div>{/if}
      {#if details?.meta.publisher}<div><dt>Publisher</dt><dd>{details.meta.publisher}</dd></div>{/if}
      {#if region}<div><dt>Region</dt><dd>{region}</dd></div>{/if}
      <div><dt>File</dt><dd><code>{relPath || romName}</code></dd></div>
      {#if bytes > 0}<div><dt>Size</dt><dd>{size(bytes)}</dd></div>{/if}
      {#if modified > 0}<div><dt>Modified</dt><dd>{day(modified)}</dd></div>{/if}
    </dl>

    {#if galleryItems.length > 0}
      <h3>Images ({galleryItems.length})</h3>
      <div class="gi-gallery" role="list" aria-label="Game images">
        {#each details?.images ?? [] as img, idx (`${img.kind}|${img.name}`)}
          <button
            type="button"
            class="gi-thumb"
            on:click={() => (lightboxIndex = idx)}
            aria-label={`View ${kindLabel(img.kind)}: ${img.name}`}
          >
            <img src={img.data_url} alt={kindLabel(img.kind)} />
            <span>{kindLabel(img.kind)}</span>
          </button>
        {/each}
      </div>
    {/if}

    <h3>Saves on this card</h3>
    {#if saves.length}
      <ul class="gi-saves" aria-label="Matching saves on this card">
        {#each saves as s (`${s.kind}|${s.name}`)}
          <li>
            <div>
              <strong>{s.label}</strong>
              <small><code>{s.name}</code></small>
            </div>
            <span>{size(s.bytes)}</span>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="gi-note">No matching saves or {plural(0, 'save state')} found on this card.</p>
    {/if}

    <div class="gi-actions">
      <button type="button" class="quiet" on:click={onOpenArtwork}>Change artwork…</button>
      {#if canRemove}
        <button type="button" class="danger-quiet" on:click={onRemove}>Remove file…</button>
      {/if}
    </div>
  </div>
</aside>

{#if lightboxIndex !== null && galleryItems.length}
  <Lightbox
    items={galleryItems}
    index={lightboxIndex}
    label={`Images for ${displayGameTitle(romName)}`}
    on:close={() => (lightboxIndex = null)}
  />
{/if}

<style>
  .gi {
    background: #161f20;
    border: 1px solid #2c393a;
    border-radius: 12px;
    overflow: hidden;
    display: grid;
    align-content: start;
  }
  .gi-hero {
    position: relative;
    width: 100%;
    aspect-ratio: 1 / 1;
    background: #0a0f10;
    border-bottom: 1px solid #2c393a;
  }
  .gi-hero-img {
    display: block;
    width: 100%;
    height: 100%;
    padding: 0;
    background: transparent;
    cursor: zoom-in;
  }
  .gi-hero-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .gi-hero-empty {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    color: #556867;
  }
  .gi-art-edit {
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
    transition: opacity 0.14s ease;
  }
  .gi-hero:hover .gi-art-edit,
  .gi-hero:focus-within .gi-art-edit {
    opacity: 1;
  }
  .gi-art-edit:hover {
    border-color: #c1f0ad;
    color: #c1f0ad;
  }
  .gi-body {
    padding: 16px;
    display: grid;
    gap: 10px;
    align-content: start;
  }
  .gi-head {
    display: grid;
    gap: 7px;
  }
  .gi-head h2 {
    margin: 0;
    font-size: 16px;
    color: #eff5f4;
    line-height: 1.3;
  }
  .gi-badges {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .gi h3 {
    margin: 8px 0 0;
    font-size: 11px;
    letter-spacing: 0.09em;
    color: #9fb3aa;
    text-transform: uppercase;
  }
  .gi-facts {
    display: grid;
    gap: 7px;
    margin: 0;
  }
  .gi-facts > div {
    display: grid;
    grid-template-columns: 88px 1fr;
    gap: 10px;
    align-items: baseline;
  }
  .gi-facts dt {
    font-size: 12px;
    color: #8f9e9d;
  }
  .gi-facts dd {
    margin: 0;
    font-size: 12.5px;
    color: #e8ecec;
    overflow-wrap: anywhere;
    line-height: 1.45;
  }
  .gi-gallery {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }
  .gi-thumb {
    display: grid;
    gap: 4px;
    padding: 4px;
    background: #101718;
    border: 1px solid #2b3839;
    border-radius: 8px;
    color: #b7c4c3;
    font-size: 10.5px;
    text-align: center;
    cursor: pointer;
  }
  .gi-thumb:hover {
    border-color: #c1f0ad;
    color: #eff5f4;
  }
  .gi-thumb img {
    width: 100%;
    aspect-ratio: 1 / 1;
    object-fit: cover;
    border-radius: 5px;
    display: block;
    background: #0a0f10;
  }
  .gi-saves {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .gi-saves li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: 8px;
    background: #111819;
    border: 1px solid #253233;
    font-size: 12px;
  }
  .gi-saves strong {
    display: block;
    color: #e8ecec;
    font-size: 12px;
  }
  .gi-saves small {
    color: #8f9e9d;
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  .gi-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    padding-top: 8px;
    border-top: 1px solid #273435;
  }
  .danger-quiet {
    background: none;
    border: 1px solid #6b3a3a;
    color: #ff9b9b;
    border-radius: 8px;
    padding: 6px 12px;
    font: inherit;
    font-size: 12.5px;
    cursor: pointer;
  }
  .danger-quiet:hover {
    background: #2b1717;
  }
  .gi-note {
    margin: 0;
    font-size: 12px;
    color: #8f9e9d;
  }
  code {
    font-size: 11px;
    background: #101617;
    border-radius: 5px;
    padding: 1px 5px;
    overflow-wrap: anywhere;
  }
</style>
