<script lang="ts">
  import type { MediaScan, PlaylistPlan } from './types';
  export let path = ''; export let result: MediaScan | null = null; export let notice = ''; export let output = ''; export let selected = ''; export let choose: () => void; export let scan: () => void; export let exportList: () => void;

  export let selectedPlaylistDetail: { name: string; file: string; tracks: string[] } | null = null;
  export let reorderTracks: string[] = [];
  export let moveTrack: (index: number, delta: -1 | 1) => void;
  export let reorderPlan: PlaylistPlan | null = null; export let reorderNotice = '';
  export let reviewReorder: () => void; export let confirmReorder: () => void;

  export let renameNewFile = ''; export let renamePlan: PlaylistPlan | null = null; export let renameNotice = '';
  export let reviewRename: () => void; export let confirmRename: () => void;

  export let createFile = ''; export let createTracksText = ''; export let createPlan: PlaylistPlan | null = null; export let createNotice = '';
  export let reviewCreate: () => void; export let confirmCreate: () => void;

  export let importSource = ''; export let importDestFile = ''; export let importPlan: PlaylistPlan | null = null; export let importNotice = '';
  export let chooseImportSource: () => void; export let reviewImport: () => void; export let confirmImport: () => void;
</script>
<section class="jobs-panel page" aria-labelledby="playlists-title">
  <header><div><p class="eyebrow">PLAYLISTS</p><h1 id="playlists-title">Playlists</h1><p class="lede">Read, create, rename, reorder, and import ordinary `.m3u` files. Every change is planned and reviewed before anything is written.</p></div></header>
  <section class="settings-card"><div class="picker-row"><input bind:value={path} placeholder="/Users/me/Music"/><button class="picker" on:click={choose}>Choose</button><button class="primary" on:click={scan}>Scan</button></div><p class="notice" role="status">{notice}</p></section>

  {#if result}
    <section class="core-list">{#each result.playlists as playlist}<article><div class="core-icon">♪</div><div><h3>{playlist.name}</h3><p>{playlist.tracks.length} tracks · {playlist.file}</p></div><span class="chip" class:capable={selected === playlist.name}>{selected === playlist.name ? 'Selected' : 'Imported'}</span><button class="quiet" on:click={() => selected = playlist.name}>Select</button></article>{/each}</section>

    <section class="settings-card"><h2>Export playlist</h2><div class="picker-row"><input bind:value={output} placeholder="/Users/me/Desktop/playlist.m3u"/><button class="primary" disabled={!selected || !output} on:click={exportList}>Export</button></div><p class="notice">{selected ? `Selected: ${selected}` : 'Select a playlist first.'}</p></section>

    {#if selectedPlaylistDetail}
      <section class="settings-card">
        <div style="width:100%">
          <h2>Reorder "{selectedPlaylistDetail.name}"</h2>
          <p>Rewrites {selectedPlaylistDetail.file} with this track order. The source library is never changed.</p>
          <ol class="track-order">
            {#each reorderTracks as track, i}
              <li><span>{track}</span><span class="track-order-buttons"><button class="quiet" disabled={i === 0} on:click={() => moveTrack(i, -1)}>↑</button><button class="quiet" disabled={i === reorderTracks.length - 1} on:click={() => moveTrack(i, 1)}>↓</button></span></li>
            {/each}
          </ol>
          <button class="primary" on:click={reviewReorder}>Review order</button>
          {#if reorderPlan}<div class="plan-inline"><p>{reorderPlan.tracks.length} tracks{reorderPlan.overwrites_existing ? ' · replaces the existing file' : ''}</p><button class="danger" on:click={confirmReorder}>Confirm and save</button></div>{/if}
          <p class="notice" role="status">{reorderNotice}</p>
        </div>
      </section>

      <section class="settings-card">
        <div style="width:100%">
          <h2>Rename "{selectedPlaylistDetail.name}"</h2>
          <div class="picker-row"><input bind:value={renameNewFile} placeholder="NewName.m3u"/><button class="primary" disabled={!renameNewFile.trim()} on:click={reviewRename}>Review rename</button></div>
          {#if renamePlan}<div class="plan-inline"><p>{selectedPlaylistDetail.file} → {renamePlan.file}</p><button class="danger" on:click={confirmRename}>Confirm and rename</button></div>{/if}
          <p class="notice" role="status">{renameNotice}</p>
        </div>
      </section>
    {/if}

    <section class="settings-card">
      <div style="width:100%">
        <h2>Create a playlist</h2>
        <p>One track's media-relative path per line (as shown in the Library screen).</p>
        <div class="picker-row"><input bind:value={createFile} placeholder="My Mix.m3u"/></div>
        <textarea bind:value={createTracksText} placeholder="Artist/Album/01 - Track.mp3"></textarea>
        <button class="primary" disabled={!createFile.trim() || !createTracksText.trim()} on:click={reviewCreate}>Review</button>
        {#if createPlan}<div class="plan-inline"><p>{createPlan.tracks.length} tracks{createPlan.overwrites_existing ? ' · replaces an existing file' : ''}</p><button class="danger" on:click={confirmCreate}>Confirm and create</button></div>{/if}
        <p class="notice" role="status">{createNotice}</p>
      </div>
    </section>

    <section class="settings-card">
      <div style="width:100%">
        <h2>Import a playlist</h2>
        <p>Matches each line of an external `.m3u` against tracks already in this media root by path or filename; anything unmatched is listed, not silently kept.</p>
        <div class="picker-row"><input bind:value={importSource} placeholder="/Users/me/Music/exported.m3u"/><button class="picker" on:click={chooseImportSource}>Choose</button></div>
        <div class="picker-row"><input bind:value={importDestFile} placeholder="Imported.m3u"/><button class="primary" disabled={!importSource.trim() || !importDestFile.trim()} on:click={reviewImport}>Review</button></div>
        {#if importPlan}
          <div class="plan-inline"><p>{importPlan.tracks.length} matched · {importPlan.dropped.length} dropped{importPlan.overwrites_existing ? ' · replaces an existing file' : ''}</p><button class="danger" on:click={confirmImport}>Confirm and import</button></div>
          {#if importPlan.dropped.length}<ul class="dropped-list">{#each importPlan.dropped as line}<li>{line}</li>{/each}</ul>{/if}
        {/if}
        <p class="notice" role="status">{importNotice}</p>
      </div>
    </section>

    {#if result.warnings.length}<section class="settings-card"><h2>Import warnings</h2><ul>{#each result.warnings as warning}<li>{warning.message}</li>{/each}</ul></section>{/if}
  {:else}
    <section class="empty"><div class="empty-art">♫</div><h2>No playlists scanned</h2><p>Choose a media folder to inspect its playlists.</p></section>
  {/if}
</section>
<style>
  .track-order { list-style: none; margin: 10px 0; padding: 0; border: 1px solid #2c393a; border-radius: 9px; overflow: hidden; max-height: 320px; overflow-y: auto; }
  .track-order li { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 9px 12px; border-bottom: 1px solid #202b2c; font-size: 12px; color: #c7d1d0; background: #101617; }
  .track-order li:last-child { border-bottom: 0; }
  .track-order-buttons button { padding: 4px 9px; font-size: 12px; }
  .plan-inline { display: flex; align-items: center; gap: 14px; margin-top: 12px; padding: 12px 14px; background: #101617; border: 1px solid #344244; border-radius: 9px; }
  .plan-inline p { margin: 0; color: #aab8b7; font-size: 12px; flex: 1; }
  .dropped-list { margin: 10px 0 0; padding-left: 20px; color: #e5c88a; font-size: 12px; }
  textarea { width: 100%; min-height: 88px; margin: 10px 0; color: #e8ecec; background: #101617; border: 1px solid #3b4a4b; border-radius: 8px; padding: 11px 12px; font: inherit; font-size: 13px; resize: vertical; }
</style>
