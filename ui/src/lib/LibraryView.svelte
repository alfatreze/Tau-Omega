<script lang="ts">
  import type { LibraryScan, TrackRow } from './types';
  export let path = '';
  export let scan: LibraryScan | null = null;
  export let rows: TrackRow[] = [];
  export let notice = '';
  export let loading = false;
  export let progress = '';
  export let search = '';
  export let format: 'all' | 'mp3' | 'flac' = 'all';
  export let choose: () => void;
  export let inspect: () => void;
  export let cancel: () => void;

  const ROW_HEIGHT = 34;
  const OVERSCAN = 8;
  let viewport: HTMLDivElement;
  let viewportHeight = 480;
  let scrollTop = 0;
  function onScroll() { scrollTop = viewport.scrollTop; }
  // A fresh scan or a changed filter gives `rows` a new array reference;
  // snap the scroll position back so a virtualised window from the
  // previous (longer) list can't leave the view scrolled past the end.
  $: { rows; if (viewport) viewport.scrollTop = 0; scrollTop = 0; }

  $: totalHeight = rows.length * ROW_HEIGHT;
  $: startIndex = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  $: visibleCount = Math.ceil(viewportHeight / ROW_HEIGHT) + OVERSCAN * 2;
  $: visibleRows = rows.slice(startIndex, startIndex + visibleCount);
  $: offsetY = startIndex * ROW_HEIGHT;

  function duration(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s.toString().padStart(2, '0')}`;
  }
</script>
<section class="jobs-panel page" aria-labelledby="library-title">
  <header><div><p class="eyebrow">LIBRARY</p><h1 id="library-title">Library overview</h1><p class="lede">A read-only, searchable view of the media Tau Omega can index. Nothing here is written to a card.</p></div></header>
  <section class="settings-card"><div class="picker-row"><input bind:value={path} placeholder="/Users/me/Music"/><button class="picker" on:click={choose}>Choose</button><button class="primary" disabled={loading} on:click={inspect}>{loading ? 'Scanning…' : 'Inspect'}</button></div>{#if loading}<p class="notice" role="status">{progress || 'starting…'} <button class="quiet" on:click={cancel}>Cancel</button></p>{:else}<p class="notice" role="status">{notice}</p>{/if}</section>
  {#if scan}
    <section class="comparison-counts library-counts"><span><b>{scan.tracks.length}</b> tracks</span><span><b>{scan.playlists.length}</b> playlists</span><span><b>{rows.length}</b> shown</span></section>
    <section class="settings-card library-filters"><div class="picker-row"><input bind:value={search} placeholder="Search title, artist, album, or path…" aria-label="Search tracks"/><select bind:value={format} aria-label="Filter by format"><option value="all">All formats</option><option value="mp3">MP3</option><option value="flac">FLAC</option></select></div></section>
    {#if rows.length}
      <section class="track-table" aria-label="Tracks">
        <div class="track-head"><span>Title</span><span>Artist</span><span>Album</span><span>Time</span><span>Format</span></div>
        <div class="track-viewport" bind:this={viewport} bind:clientHeight={viewportHeight} on:scroll={onScroll}>
          <div class="track-spacer" style="height:{totalHeight}px">
            <div class="track-window" style="transform:translateY({offsetY}px)">
              {#each visibleRows as row (row.rel)}
                <div class="track-row" title={row.rel}><span>{row.title}</span><span>{row.artist}</span><span>{row.album}</span><span>{duration(row.secs)}</span><span>{row.format}</span></div>
              {/each}
            </div>
          </div>
        </div>
      </section>
    {:else}
      <section class="empty"><div class="empty-art">∅</div><h2>No tracks match</h2><p>Try a different search term or format filter.</p></section>
    {/if}
    {#if scan.playlists.length}
      <section class="settings-card"><h2>Playlists</h2><div class="settings-values">{#each scan.playlists as playlist}<div><span>{playlist.name}</span><strong>{playlist.tracks} tracks</strong></div>{/each}</div></section>
    {/if}
    {#if scan.warnings.length}
      <section class="settings-card"><h2>Library warnings</h2><ul>{#each scan.warnings as warning}<li>{warning.message}</li>{/each}</ul></section>
    {/if}
  {:else}
    <section class="empty"><div class="empty-art">τ</div><h2>No library selected</h2><p>Choose a local media root to inspect it.</p></section>
  {/if}
</section>
<style>
  .library-counts { grid-template-columns: repeat(3, 1fr); }
  .library-filters .picker-row select { flex: 0 0 auto; background: #101617; color: #e8ecec; border: 1px solid #344244; border-radius: 8px; padding: 0 10px; font: inherit; }
  .track-table { border: 1px solid #2c393a; border-radius: 12px; overflow: hidden; margin-bottom: 14px; background: #1a2325; }
  .track-head, .track-row { display: grid; grid-template-columns: 2fr 1.4fr 1.4fr 70px 70px; gap: 12px; align-items: center; padding: 0 16px; }
  .track-head { height: 36px; font-size: 11px; letter-spacing: 0.04em; text-transform: uppercase; color: #8f9e9d; border-bottom: 1px solid #2c393a; background: #151d1e; }
  .track-viewport { height: 480px; overflow-y: auto; position: relative; }
  .track-spacer { position: relative; }
  .track-window { position: absolute; top: 0; left: 0; right: 0; }
  .track-row { height: 34px; border-bottom: 1px solid #202b2c; font-size: 13px; color: #c7d1d0; }
  .track-row span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .track-row span:nth-child(4), .track-row span:nth-child(5) { color: #8f9e9d; font-size: 12px; }
</style>
