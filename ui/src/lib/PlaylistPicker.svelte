<script lang="ts">
  // Choose the playlist the selected albums go into: one that exists, or a new one (a name you type). Nothing is written here; the choice is staged with the
  // other changes and written by the sync.
  import { modal } from './a11y';
  export let albums = 0;
  /** Playlists on the card (and any already staged), each with its file name and how many tracks it holds. */
  export let playlists: { file: string; name: string; tracks: number }[] = [];
  /** The playlist the player opens by default, listed first even before it exists. */
  export let defaultFile = '';
  /** How many tracks one playlist holds, when the player limits it (HarpMudd: 256). */
  export let maxTracks: number | null = null;
  /** The longest playlist file name (with .m3u) the player remembers; null when there is no limit. */
  export let maxFileChars: number | null = null;
  export let close: () => void;
  export let chosen: (file: string) => void;

  let mode: 'existing' | 'new' = 'existing'; let pick = ''; let name = '';
  $: all = (defaultFile && !playlists.some((p) => p.file.toLowerCase() === defaultFile.toLowerCase()) ? [{ file: defaultFile, name: defaultFile.replace(/\.m3u$/i, ''), tracks: 0 }] : []).concat(playlists);
  $: if (!pick && all.length) pick = all[0].file;
  $: if (!all.length) mode = 'new';
  const clean = (n: string) => n.trim().replace(/\.m3u$/i, '');
  $: file = mode === 'new' ? (clean(name) ? `${clean(name)}.m3u` : '') : pick;
  $: problem = mode !== 'new' ? '' : !clean(name) ? '' : /[\/\\:\0]/.test(name) ? 'A playlist name cannot contain / \\ or :.' : name.trim().startsWith('.') ? 'A playlist name cannot start with a dot.' : all.some((p) => p.file.toLowerCase() === file.toLowerCase()) ? 'A playlist with that name exists: choose it above.' : maxFileChars && file.length > maxFileChars ? `This player remembers names of at most ${maxFileChars} characters including .m3u.` : file.length > 100 ? 'That name is too long.' : '';
  $: ok = !!file && !problem;
</script>
<svelte:window on:keydown={(e) => e.key === 'Escape' && close()} />
<div class="pp-backdrop" role="presentation" on:click={close}></div>
<div class="pp" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="pp-title" use:modal>
  <header class="pp-head"><div><p class="eyebrow">ADD TO A PLAYLIST</p><h2 id="pp-title">{albums === 1 ? 'Put this album in a playlist' : `Put ${albums} albums in a playlist`}</h2></div><button class="quiet" data-autofocus on:click={close}>Cancel</button></header>
  {#if all.length}
    <div class="pp-list" role="radiogroup" aria-label="Playlists">
      {#each all as p}
        <label class="pp-row"><input type="radio" name="pp" checked={mode === 'existing' && pick === p.file} on:change={() => { mode = 'existing'; pick = p.file; }} /><span><b>{p.name}</b><small>{p.tracks} track{p.tracks === 1 ? '' : 's'}{maxTracks ? ` of ${maxTracks}` : ''}{p.file === defaultFile ? ' · opens by default' : ''}</small></span></label>
      {/each}
      <label class="pp-row"><input type="radio" name="pp" checked={mode === 'new'} on:change={() => (mode = 'new')} /><span><b>A new playlist…</b></span></label>
    </div>
  {/if}
  {#if mode === 'new'}
    <label class="pp-name">Name of the new playlist<input bind:value={name} autocomplete="off" placeholder="Road trip" aria-label="New playlist name" /></label>
  {/if}
  {#if problem}<p class="pp-bad" role="alert">{problem}</p>{/if}
  {#if maxTracks}<p class="pp-note">This player plays at most {maxTracks} tracks from one playlist. Omega checks that before anything is copied.</p>{/if}
  <div class="pp-actions"><button class="primary" disabled={!ok} on:click={() => chosen(file)}>Add to {file ? file.replace(/\.m3u$/i, '') : 'playlist'}</button></div>
</div>
<style>
  .pp-backdrop{position:fixed;inset:0;background:rgba(0,0,0,.6);z-index:100}
  .pp{position:fixed;z-index:101;top:50%;left:50%;transform:translate(-50%,-50%);width:min(520px,calc(100vw - 32px));max-height:calc(100vh - 40px);overflow:auto;background:#1a2325;border:1px solid #2c393a;border-radius:14px;padding:20px 24px;color:#e8ecec}
  .pp-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px;margin-bottom:10px}.pp-head h2{margin:0;font-size:18px}
  .pp-list{border:1px solid #2c393a;border-radius:10px;max-height:40vh;overflow:auto}
  .pp-row{display:flex;gap:10px;align-items:center;padding:9px 12px;border-bottom:1px solid #222e2f;cursor:pointer}.pp-row:last-child{border:0}
  .pp-row span{display:flex;flex-direction:column;gap:1px}.pp-row small{color:#95a5a4;font-size:12px}
  .pp-name{display:grid;gap:5px;font-size:12.5px;color:#a6b3b2;margin:12px 0 4px}.pp-name input{font:inherit;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:9px 11px}
  .pp-bad{color:#ff9b9b;font-size:12.5px;margin:6px 0}.pp-note{font-size:12px;color:#95a5a4;margin:8px 0}
  .pp-actions{display:flex;justify-content:flex-end;margin-top:12px}
</style>
