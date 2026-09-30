<script lang="ts">
  // P0 PROTOTYPE of the card-centred library workbench (docs/LIBRARY_WORKBENCH_PLAN.md).
  // All data here is fixture data and every action is simulated: nothing in this
  // component calls the backend or touches a card. P1/P2 replace the fixtures
  // with the engine's scan and plan results; the layout and states stay.
  import { onDestroy } from 'svelte';

  export let cardLabel = 'Pocket';
  export let coreLabel = 'tau.omega';
  /** Force a re-read of the card (App re-inspects it). */
  export let refresh: () => void | Promise<void> = () => {};
  /** ISO-ish label of the last refresh, shown in the refresh tooltip. */
  export let refreshedAt = 'just now';

  type Album = { id: string; title: string; artist: string; tracks: number; bytes: number; year?: number; cover?: string };
  type Pending =
    | { key: string; kind: 'add'; album: Album }
    | { key: string; kind: 'remove'; album: Album }
    | { key: string; kind: 'edit'; album: Album; changes: Partial<Album>; note: string };

  const MB = 1e6, GB = 1e9;
  const CAPACITY = 8 * GB;
  const OTHER_USED = 3.1 * GB; // cores, saves, screenshots: not ours to touch
  const SLOW_LIMIT = 10 * MB;
  const SLOW_KEY = 'tau.workbench.slowAlertSuppressed';
  const REMOVE_KEY = 'tau.workbench.removeExplained';

  const mk = (id: string, title: string, artist: string, tracks: number, mb: number, year: number): Album => ({ id, title, artist, tracks, bytes: mb * MB, year });
  const source: Album[] = [
    mk('kob', 'Kind of Blue', 'Miles Davis', 5, 310, 1959), mk('alsu', 'A Love Supreme', 'John Coltrane', 4, 268, 1965),
    mk('bt', 'Blue Train', 'John Coltrane', 5, 290, 1958), mk('mam', 'Mingus Ah Um', 'Charles Mingus', 9, 402, 1959),
    mk('tof', 'Time Out', 'Dave Brubeck Quartet', 7, 344, 1959), mk('hh', 'Head Hunters', 'Herbie Hancock', 4, 380, 1973),
    mk('mpt', 'Moanin', 'Art Blakey', 6, 322, 1958), mk('sr', 'Saxophone Colossus', 'Sonny Rollins', 5, 296, 1956),
    mk('tbb', 'The Black Saint', 'Charles Mingus', 4, 712, 1963), mk('bwh', 'Bitches Brew', 'Miles Davis', 7, 1380, 1970),
    mk('mps', 'Maiden Voyage', 'Herbie Hancock', 5, 330, 1965), mk('ago', 'Ascenseur pour l Echafaud', 'Miles Davis', 8, 288, 1958),
  ];
  const byId = (id: string) => source.find((a) => a.id === id)!;
  let pocket: Album[] = ['kob', 'bt', 'tof', 'mps'].map(byId);
  const changedOnDisk = new Set(['bt']); // a source album that differs from the card copy
  const playlists = [{ name: 'Favourites', tracks: 24 }, { name: 'Night drive', tracks: 61 }];

  // ---- state ------------------------------------------------------------
  let sourcePath = '~/Music/Jazz';
  let sourceSearch = '', pocketSearch = '';
  let picked = new Set<string>(); // ticked albums in "This computer"
  let pocketPicked = new Set<string>(); // ticked albums "On Pocket"
  let view: 'albums' | 'artists' | 'playlists' = 'albums';
  let pending: Pending[] = [];
  let connection: 'direct' | 'reader' = 'direct'; // auto-detected in P2
  let dropActive = false;
  let toast = '';
  let toastTimer: ReturnType<typeof setTimeout>;
  let spinning = false;

  let dialog: null | 'review' | 'slow' | 'first-remove' | 'edit' = null;
  let slowSuppressed = read(SLOW_KEY), removeExplained = read(REMOVE_KEY);
  let slowDontAsk = false;
  function read(key: string) { try { return localStorage.getItem(key) === '1'; } catch { return false; } }
  function write(key: string) { try { localStorage.setItem(key, '1'); } catch { /* per-viewer convenience only */ } }

  let run: null | { phase: string; done: number; total: number; speed: number; finished: boolean; summary: string } = null;
  let timer: ReturnType<typeof setInterval>;
  onDestroy(() => { clearInterval(timer); clearTimeout(toastTimer); });

  // edit drawer
  let editTarget: Album | null = null;
  let editForm = { title: '', artist: '', year: 0, cover: '' };
  const covers = ['#7a5a3a', '#3a5a7a', '#5a7a3a', '#7a3a5a', '#4a4a4a'];

  // ---- derived numbers (the same maths the real planner will supply) ------
  $: pocketBytes = pocket.reduce((n, a) => n + a.bytes, 0);
  $: adds = pending.filter((p) => p.kind === 'add');
  $: removes = pending.filter((p) => p.kind === 'remove');
  $: edits = pending.filter((p) => p.kind === 'edit');
  $: addBytes = adds.reduce((n, p) => n + p.album.bytes, 0);
  $: removeBytes = removes.reduce((n, p) => n + p.album.bytes, 0);
  $: used = OTHER_USED + pocketBytes;
  $: projected = used + addBytes - removeBytes;
  $: free = CAPACITY - projected;
  $: overCapacity = free < 0;
  $: pct = (n: number) => `${Math.max(0, Math.min(100, (n / CAPACITY) * 100))}%`;
  $: queuedIds = new Set(adds.map((p) => p.album.id));
  $: removingIds = new Set(removes.map((p) => p.album.id));
  $: pocketIds = new Set(pocket.map((a) => a.id));
  $: slowApplies = connection === 'direct' && addBytes > SLOW_LIMIT;
  $: trackTotal = adds.reduce((n, p) => n + p.album.tracks, 0);
  $: pendingCount = pending.length;
  $: canStart = pendingCount > 0 && !overCapacity && !run;

  const size = (n: number) => (n >= GB ? `${(n / GB).toFixed(1)} GB` : `${Math.round(n / MB)} MB`);
  const matches = (a: Album, q: string) => !q || `${a.title} ${a.artist}`.toLowerCase().includes(q.toLowerCase());
  $: sourceRows = source.filter((a) => matches(a, sourceSearch));
  $: pocketRows = pocket.filter((a) => matches(a, pocketSearch));
  $: artists = [...pocketRows.reduce((m, a) => m.set(a.artist, [...(m.get(a.artist) ?? []), a]), new Map<string, Album[]>())];
  function eta(bytes: number, speed: number) { const s = Math.ceil(bytes / speed); return s < 90 ? `${s} sec` : `${Math.round(s / 60)} min`; }
  const speedFor = () => (connection === 'direct' ? 2.4 * MB : 38 * MB);

  function say(message: string) { toast = message; clearTimeout(toastTimer); toastTimer = setTimeout(() => (toast = ''), 3200); }
  function toggle(set: Set<string>, id: string) { const next = new Set(set); next.has(id) ? next.delete(id) : next.add(id); return next; }
  function stateOf(a: Album): 'queued' | 'on' | 'changed' | 'new' {
    if (queuedIds.has(a.id)) return 'queued';
    if (pocketIds.has(a.id)) return changedOnDisk.has(a.id) ? 'changed' : 'on';
    return 'new';
  }

  // ---- staging ------------------------------------------------------------
  function addAlbums(ids: string[]) {
    const fresh = ids.map(byId).filter((a) => !queuedIds.has(a.id) && (!pocketIds.has(a.id) || changedOnDisk.has(a.id)));
    if (!fresh.length) { say('Those albums are already on the Pocket.'); return; }
    pending = [...pending, ...fresh.map((album) => ({ key: `add-${album.id}`, kind: 'add' as const, album }))];
    picked = new Set();
    say(`${fresh.length} album${fresh.length === 1 ? '' : 's'} added to Pending changes.`);
  }
  const addPicked = () => addAlbums([...picked]);
  function onDragStart(e: DragEvent, a: Album) { const ids = picked.has(a.id) ? [...picked] : [a.id]; e.dataTransfer?.setData('text/tau-albums', ids.join(',')); if (e.dataTransfer) e.dataTransfer.effectAllowed = 'copy'; }
  function onDrop(e: DragEvent) { e.preventDefault(); dropActive = false; const raw = e.dataTransfer?.getData('text/tau-albums'); if (raw) addAlbums(raw.split(',')); }

  function requestRemove() { if (!pocketPicked.size) return; if (!removeExplained) dialog = 'first-remove'; else stageRemoval(); }
  function confirmFirstRemove() { removeExplained = true; write(REMOVE_KEY); dialog = null; stageRemoval(); }
  function stageRemoval() {
    const items = pocket.filter((a) => pocketPicked.has(a.id) && !removingIds.has(a.id));
    pending = [...pending, ...items.map((album) => ({ key: `rm-${album.id}`, kind: 'remove' as const, album }))];
    pocketPicked = new Set();
    say(`${items.length} album${items.length === 1 ? '' : 's'} marked for removal. Nothing is deleted until you sync.`);
  }
  function unstage(key: string) { pending = pending.filter((p) => p.key !== key); }
  function clearAll() { const n = pending.length; pending = []; picked = new Set(); pocketPicked = new Set(); say(`Cleared ${n} pending change${n === 1 ? '' : 's'}.`); }

  function openEdit(focus: 'title' | 'cover' = 'title') {
    const a = pocket.find((x) => pocketPicked.has(x.id)); if (!a) return;
    editTarget = a; editForm = { title: a.title, artist: a.artist, year: a.year ?? 0, cover: a.cover ?? '' }; dialog = 'edit';
    setTimeout(() => document.getElementById(focus === 'cover' ? 'cover-first' : 'edit-title')?.focus(), 30);
  }
  function stageEdit() {
    if (!editTarget) return;
    const changes: Partial<Album> = {}; const notes: string[] = [];
    if (editForm.title.trim() && editForm.title !== editTarget.title) { changes.title = editForm.title.trim(); notes.push('title'); }
    if (editForm.artist.trim() && editForm.artist !== editTarget.artist) { changes.artist = editForm.artist.trim(); notes.push('artist'); }
    if (editForm.year && editForm.year !== editTarget.year) { changes.year = editForm.year; notes.push('year'); }
    if (editForm.cover !== (editTarget.cover ?? '')) { changes.cover = editForm.cover; notes.push('cover'); }
    dialog = null;
    if (!notes.length) return;
    const album = editTarget;
    pending = [...pending.filter((p) => p.key !== `ed-${album.id}`), { key: `ed-${album.id}`, kind: 'edit', album, changes, note: notes.join(', ') }];
    pocketPicked = new Set(); say('Edit added to Pending changes.');
  }

  // ---- sync (simulated) ---------------------------------------------------
  function start() {
    if (!canStart) return;
    if (slowApplies && !slowSuppressed) { dialog = 'slow'; return; }
    dialog = 'review';
  }
  function slowContinue() { if (slowDontAsk) { slowSuppressed = true; write(SLOW_KEY); } dialog = 'review'; }
  function confirmSync() {
    dialog = null;
    const total = Math.max(addBytes, 1), speed = speedFor();
    run = { phase: 'Copying', done: 0, total, speed, finished: false, summary: '' };
    clearInterval(timer);
    // Time-lapse: the bar moves fast so the states are reviewable; speed and
    // ETA shown are the real-world figures for the chosen connection.
    timer = setInterval(() => {
      if (!run) return;
      const step = total / 60;
      const done = Math.min(total, run.done + step);
      run = { ...run, done, phase: done >= total ? 'Writing index' : done > total * 0.85 ? 'Verifying' : 'Copying' };
      if (done >= total) { clearInterval(timer); setTimeout(finishSync, 500); }
    }, 90);
  }
  function cancelSync() { clearInterval(timer); run = null; say('Sync cancelled. Files already copied are kept and the index was not changed.'); }
  function finishSync() {
    let next = pocket.filter((a) => !removingIds.has(a.id));
    for (const p of pending) {
      if (p.kind === 'add' && !next.some((a) => a.id === p.album.id)) next = [...next, p.album];
      if (p.kind === 'edit') next = next.map((a) => (a.id === p.album.id ? { ...a, ...p.changes } : a));
    }
    pocket = next;
    const parts = [adds.length && `${trackTotal} tracks added`, removes.length && `${removes.length} album${removes.length === 1 ? '' : 's'} removed`, edits.length && `${edits.length} edit${edits.length === 1 ? '' : 's'} applied`].filter(Boolean);
    run = { phase: 'Done', done: 1, total: 1, speed: 0, finished: true, summary: `${parts.join(', ')}. Index verified.` };
    pending = [];
  }
  function closeRun() { run = null; }

  async function doRefresh() { spinning = true; try { await refresh(); } finally { setTimeout(() => (spinning = false), 700); } }
  function key(e: KeyboardEvent) { if (e.key === 'Escape' && dialog && dialog !== 'edit') dialog = null; if (e.key === 'Escape' && dialog === 'edit') dialog = null; }
</script>
<svelte:window on:keydown={key} />

<section class="wb" aria-labelledby="wb-title">
  <header class="wb-top">
    <div class="wb-ctx">
      <p class="eyebrow">LIBRARY</p>
      <h1 id="wb-title">{cardLabel}</h1>
      <p class="wb-sub"><span class="wb-dot" aria-hidden="true"></span>{coreLabel}
        <button class="wb-conn" class:slow={connection === 'direct'} title="How this card is connected. Detected automatically; click to change while prototyping." on:click={() => (connection = connection === 'direct' ? 'reader' : 'direct')}>
          {connection === 'direct' ? 'Direct USB · slow' : 'Card reader · fast'}
        </button>
      </p>
    </div>
    <button class="wb-refresh" class:spin={spinning} aria-label="Refresh this card" title={`Refresh · updated ${refreshedAt}`} on:click={doRefresh}>
      <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 12a8 8 0 1 1-2.6-5.9"/><path d="M20 4v5h-5"/></svg>
    </button>
  </header>

  <div class="wb-cap" role="group" aria-label="Storage on the Pocket">
    <div class="wb-cap-text">
      <span><b>{size(used)}</b> on Pocket</span>
      {#if addBytes}<span class="add">+ {size(addBytes)} queued</span>{/if}
      {#if removeBytes}<span class="rm">− {size(removeBytes)} removing</span>{/if}
      <span class="wb-cap-free" class:bad={overCapacity}>{overCapacity ? `${size(-free)} too much for this card` : `${size(free)} free of ${size(CAPACITY)}`}</span>
    </div>
    <div class="wb-bar" role="img" aria-label={`${size(used)} used, ${size(addBytes)} queued, ${size(Math.max(free, 0))} free`}>
      <i class="k-used" style="width:{pct(used - removeBytes)}"></i><i class="k-rm" style="width:{pct(removeBytes)}"></i><i class="k-add" class:over={overCapacity} style="width:{pct(addBytes)}"></i>
    </div>
  </div>

  <div class="wb-panes">
    <section class="wb-pane" aria-labelledby="src-title">
      <div class="wb-pane-head"><h2 id="src-title">This computer</h2><button class="wb-link" on:click={() => say('Folder picker opens here (simulated).')}>{sourcePath} · change</button></div>
      <input class="wb-search" bind:value={sourceSearch} placeholder="Search albums or artists" aria-label="Search this computer" />
      <div class="wb-list" role="list">
        {#each sourceRows as a (a.id)}
          {@const st = stateOf(a)}
          <div class="wb-row" class:sel={picked.has(a.id)} class:dim={st === 'on'} role="listitem" draggable={st !== 'on' && st !== 'queued'} on:dragstart={(e) => onDragStart(e, a)}>
            <input type="checkbox" aria-label={`Select ${a.title}`} disabled={st === 'on' || st === 'queued'} checked={picked.has(a.id)} on:change={() => (picked = toggle(picked, a.id))} />
            <div class="wb-art" aria-hidden="true">♪</div>
            <div class="wb-meta"><strong>{a.title}</strong><small>{a.artist} · {a.tracks} tracks · {size(a.bytes)}</small></div>
            <span class="wb-badge {st}">{st === 'on' ? 'On Pocket' : st === 'queued' ? 'Queued' : st === 'changed' ? 'Changed' : 'New'}</span>
          </div>
        {/each}
      </div>
      <div class="wb-pane-foot">
        <span>{picked.size ? `${picked.size} selected` : 'Tick albums, or drag them across →'}</span>
        <button class="primary" disabled={!picked.size} on:click={addPicked}>Add {picked.size || ''} to Pocket →</button>
      </div>
    </section>

    <section class="wb-pane wb-drop" class:active={dropActive} aria-labelledby="pk-title"
      on:dragover|preventDefault={() => (dropActive = true)} on:dragleave={() => (dropActive = false)} on:drop={onDrop}>
      <div class="wb-pane-head"><h2 id="pk-title">On Pocket</h2><span class="wb-count">{pocket.length} albums · {pocket.reduce((n, a) => n + a.tracks, 0)} tracks</span></div>
      <div class="wb-tabs" role="tablist">
        {#each ['albums', 'artists', 'playlists'] as t}<button role="tab" aria-selected={view === t} class:on={view === t} on:click={() => (view = t as typeof view)}>{t[0].toUpperCase() + t.slice(1)}</button>{/each}
        <input class="wb-search inline" bind:value={pocketSearch} placeholder="Search" aria-label="Search the Pocket" />
      </div>
      {#if pocketPicked.size}
        <div class="wb-actions" role="toolbar" aria-label="Actions for selected albums">
          <span>{pocketPicked.size} selected</span>
          <button on:click={() => openEdit('title')} disabled={pocketPicked.size !== 1}>Rename</button>
          <button on:click={() => openEdit('title')} disabled={pocketPicked.size !== 1}>Edit info</button>
          <button on:click={() => openEdit('cover')} disabled={pocketPicked.size !== 1}>Change cover</button>
          <button class="danger" on:click={requestRemove}>Remove</button>
        </div>
      {/if}
      <div class="wb-list" role="list">
        {#if view === 'albums'}
          {#each pocketRows as a (a.id)}
            <div class="wb-row" class:sel={pocketPicked.has(a.id)} class:removing={removingIds.has(a.id)} role="listitem">
              <input type="checkbox" aria-label={`Select ${a.title}`} disabled={removingIds.has(a.id)} checked={pocketPicked.has(a.id)} on:change={() => (pocketPicked = toggle(pocketPicked, a.id))} />
              <div class="wb-art" style={a.cover ? `background:${a.cover}` : ''} aria-hidden="true">{a.cover ? '' : '♪'}</div>
              <div class="wb-meta"><strong>{a.title}</strong><small>{a.artist} · {a.tracks} tracks · {size(a.bytes)}</small></div>
              {#if removingIds.has(a.id)}<span class="wb-badge removing">Will be removed</span>{/if}
            </div>
          {/each}
          {#each adds as p (p.key)}
            <div class="wb-row incoming" role="listitem"><span class="wb-plus" aria-hidden="true">+</span><div class="wb-art" aria-hidden="true">♪</div><div class="wb-meta"><strong>{p.album.title}</strong><small>{p.album.artist} · {p.album.tracks} tracks · {size(p.album.bytes)}</small></div><span class="wb-badge queued">Will be added</span></div>
          {/each}
          {#if !pocketRows.length && !adds.length}<div class="wb-empty"><b>Nothing here yet</b><p>Add albums from This computer to get started.</p></div>{/if}
        {:else if view === 'artists'}
          {#each artists as [name, list] (name)}<div class="wb-row" role="listitem"><div class="wb-art" aria-hidden="true">♪</div><div class="wb-meta"><strong>{name}</strong><small>{list.length} album{list.length === 1 ? '' : 's'} · {list.reduce((n, a) => n + a.tracks, 0)} tracks</small></div></div>{/each}
        {:else}
          {#each playlists as pl}<div class="wb-row" role="listitem"><div class="wb-art" aria-hidden="true">≡</div><div class="wb-meta"><strong>{pl.name}</strong><small>{pl.tracks} tracks · managed on the Playlists page</small></div></div>{/each}
        {/if}
      </div>
      {#if dropActive}<div class="wb-dropveil">Drop to add to Pocket</div>{/if}
    </section>
  </div>

  <section class="wb-tray" aria-labelledby="tray-title">
    <div class="wb-tray-head">
      <div>
        <h2 id="tray-title">Pending changes</h2>
        <p>{#if pendingCount}{[adds.length && `${adds.length} to add (${trackTotal} tracks, ${size(addBytes)})`, removes.length && `${removes.length} to remove`, edits.length && `${edits.length} to edit`].filter(Boolean).join(' · ')}{:else}Nothing yet. Changes are only made when you press Start sync.{/if}</p>
      </div>
      <div class="wb-tray-actions">
        {#if slowApplies && slowSuppressed}<span class="wb-slownote" role="note">Direct connection: about {eta(addBytes, speedFor())}</span>{/if}
        {#if overCapacity}<span class="wb-slownote bad" role="alert">Won't fit: remove {size(-free)}</span>{/if}
        <button class="quiet" disabled={!pendingCount} on:click={clearAll}>Clear all</button>
        <button class="primary" disabled={!canStart} on:click={start}>Start sync</button>
      </div>
    </div>
    {#if pendingCount}
      <ul class="wb-chips">
        {#each pending as p (p.key)}
          <li class={p.kind}><span class="k">{p.kind === 'add' ? '+' : p.kind === 'remove' ? '−' : '✎'}</span><span class="t">{p.album.title}{p.kind === 'edit' ? ` (${p.note})` : ''}</span>{#if p.kind !== 'edit'}<small>{size(p.album.bytes)}</small>{/if}<button aria-label={`Remove ${p.album.title} from pending changes`} on:click={() => unstage(p.key)}>{p.kind === 'remove' ? 'Undo' : '×'}</button></li>
        {/each}
      </ul>
    {/if}
  </section>

  {#if toast}<div class="wb-toast" role="status">{toast}</div>{/if}
</section>

{#if dialog === 'slow'}
  <div class="wb-veil" role="presentation" on:click={() => (dialog = null)}></div>
  <div class="wb-modal" role="alertdialog" aria-labelledby="slow-t" aria-describedby="slow-d">
    <h2 id="slow-t">You're connected to the Pocket directly</h2>
    <p id="slow-d">Transfers over this connection are slow, and this may take a long time: about <b>{eta(addBytes, speedFor())}</b> for {size(addBytes)}. A card reader is much faster for big syncs. Are you sure?</p>
    <label class="wb-check"><input type="checkbox" bind:checked={slowDontAsk} /> Don't ask again (you'll still see a small note next to Start sync)</label>
    <div class="wb-modal-actions"><button class="quiet" on:click={() => (dialog = null)}>Cancel</button><button class="primary" on:click={slowContinue}>Sync anyway</button></div>
  </div>
{/if}

{#if dialog === 'review'}
  <div class="wb-veil" role="presentation" on:click={() => (dialog = null)}></div>
  <div class="wb-modal" role="dialog" aria-labelledby="rev-t">
    <h2 id="rev-t">Ready to sync to {cardLabel}?</h2>
    <dl class="wb-review">
      {#if adds.length}<div><dt>Add</dt><dd>{adds.length} albums · {trackTotal} tracks · {size(addBytes)}</dd></div>{/if}
      {#if removes.length}<div><dt>Remove</dt><dd>{removes.length} albums · {size(removeBytes)} (backed up first)</dd></div>{/if}
      {#if edits.length}<div><dt>Edit</dt><dd>{edits.length} album{edits.length === 1 ? '' : 's'} (Pocket copies only)</dd></div>{/if}
      <div><dt>Free after</dt><dd>{size(free)} of {size(CAPACITY)}</dd></div>
    </dl>
    <p class="wb-fine">Your music on this computer is never changed. Every file is verified, then the index is written last.</p>
    <div class="wb-modal-actions"><button class="quiet" on:click={() => (dialog = null)}>Back</button><button class="primary" on:click={confirmSync}>Confirm and start</button></div>
  </div>
{/if}

{#if dialog === 'first-remove'}
  <div class="wb-veil" role="presentation" on:click={() => (dialog = null)}></div>
  <div class="wb-modal" role="alertdialog" aria-labelledby="fr-t">
    <h2 id="fr-t">Remove from the Pocket?</h2>
    <p>Removals are only <b>marked</b> now; nothing is deleted until you press Start sync, and you can undo until then. Before deleting, the files are copied to your backup folder on this computer.</p>
    <p class="wb-fine">You can change this any time in <b>Settings → Library → Removing from the Pocket</b> (ask every time, back up then remove, or just remove).</p>
    <div class="wb-modal-actions"><button class="quiet" on:click={() => (dialog = null)}>Cancel</button><button class="primary danger-btn" on:click={confirmFirstRemove}>Mark for removal</button></div>
  </div>
{/if}

{#if dialog === 'edit' && editTarget}
  <div class="wb-veil" role="presentation" on:click={() => (dialog = null)}></div>
  <aside class="wb-drawer" aria-labelledby="ed-t">
    <div class="wb-drawer-head"><h2 id="ed-t">Edit album</h2><button class="quiet" aria-label="Close" on:click={() => (dialog = null)}>✕</button></div>
    <p class="wb-fine">Changes apply to the copy on the Pocket only. Your files on this computer stay as they are. They are re-applied on future syncs.</p>
    <label>Album title<input id="edit-title" bind:value={editForm.title} /></label>
    <label>Artist<input bind:value={editForm.artist} /></label>
    <label>Year<input type="number" bind:value={editForm.year} /></label>
    <fieldset><legend>Cover</legend>
      <div class="wb-covers">{#each covers as c, i}<button id={i === 0 ? 'cover-first' : undefined} class:on={editForm.cover === c} style="background:{c}" aria-label={`Use cover ${i + 1}`} on:click={() => (editForm.cover = c)}></button>{/each}<button class="wb-covers-file" on:click={() => say('Image picker opens here (simulated).')}>Choose image…</button></div>
      <small>Covers must be baseline JPEG under 2 MiB; anything else is flagged before you can add it.</small>
    </fieldset>
    <div class="wb-modal-actions"><button class="quiet" on:click={() => (dialog = null)}>Cancel</button><button class="primary" on:click={stageEdit}>Add to Pending changes</button></div>
  </aside>
{/if}

{#if run}
  <div class="wb-veil" role="presentation"></div>
  <div class="wb-modal wb-run" role="dialog" aria-labelledby="run-t" aria-live="polite">
    <h2 id="run-t">{run.finished ? 'Sync complete' : `Syncing to ${cardLabel}`}</h2>
    {#if !run.finished}
      <div class="wb-progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round((run.done / run.total) * 100)}><i style="width:{(run.done / run.total) * 100}%"></i></div>
      <dl class="wb-review">
        <div><dt>Step</dt><dd>{run.phase}</dd></div>
        <div><dt>Copied</dt><dd>{size(run.done)} of {size(run.total)}</dd></div>
        <div><dt>Speed</dt><dd>{(run.speed / MB).toFixed(1)} MB/s</dd></div>
        <div><dt>Time left</dt><dd>about {eta(run.total - run.done, run.speed)}</dd></div>
      </dl>
      <div class="wb-modal-actions"><button class="quiet" on:click={cancelSync}>Cancel sync</button></div>
    {:else}
      <p class="wb-ok">✓ {run.summary}</p>
      <div class="wb-modal-actions"><button class="quiet" on:click={() => say('Journal opens here (simulated).')}>Show details</button><button class="primary" on:click={closeRun}>Done</button></div>
    {/if}
  </div>
{/if}

<style>
  .wb{padding:24px 40px 20px;display:flex;flex-direction:column;gap:14px;height:100vh;position:relative}
  .wb-top{display:flex;justify-content:space-between;align-items:flex-start;margin:0;padding:0;flex-shrink:0}
  .wb-top h1{margin:0 0 4px;font-size:26px}.wb-ctx .eyebrow{margin:0 0 2px}
  .wb-sub{margin:0;display:flex;align-items:center;gap:10px;color:#9cacab;font-size:13px}
  .wb-dot{width:7px;height:7px;border-radius:50%;background:#8fd6f2}
  .wb-conn{background:#273032;color:#b7c3c2;border-radius:99px;padding:4px 10px;font-size:11px;font-weight:650}
  .wb-conn.slow{background:#3d3220;color:#f0d59a}
  .wb-refresh{margin-right:52px;background:transparent;color:#6f807f;padding:8px;border-radius:9px;border:1px solid transparent}
  .wb-refresh:hover{color:#e8ecec;border-color:#2c393a}
  .wb-refresh.spin svg{animation:wb-spin .7s linear}
  @keyframes wb-spin{to{transform:rotate(360deg)}}
  .wb-cap{flex-shrink:0;background:#161f20;border:1px solid #2c393a;border-radius:12px;padding:14px 16px}
  .wb-cap-text{display:flex;flex-wrap:wrap;gap:6px 18px;font-size:13px;color:#a6b3b2;margin-bottom:10px}
  .wb-cap-text b{color:#e8ecec}.wb-cap-text .add{color:#c1f0ad}.wb-cap-text .rm{color:#e8b59f}
  .wb-cap-free{margin-left:auto}.wb-cap-free.bad{color:#ff9d8a;font-weight:700}
  .wb-bar{display:flex;height:12px;border-radius:99px;background:#232e30;overflow:hidden}
  .wb-bar i{display:block;height:100%;transition:width .25s}
  .k-used{background:#5b7f8f}.k-rm{background:repeating-linear-gradient(45deg,#7a4a3d,#7a4a3d 4px,#5b3a30 4px,#5b3a30 8px)}.k-add{background:#c1f0ad}.k-add.over{background:#ff9d8a}
  .wb-panes{display:grid;grid-template-columns:1fr 1fr;gap:16px;flex:1;min-height:0}
  @media(max-width:1100px){.wb-panes{grid-template-columns:1fr}.wb{height:auto;min-height:100vh}.wb-list{max-height:320px}}
  .wb-pane{position:relative;min-height:0;background:#161f20;border:1px solid #2c393a;border-radius:14px;padding:16px;display:flex;flex-direction:column;gap:10px;min-width:0}
  .wb-drop.active{border-color:#c1f0ad;background:#1a2a20}
  .wb-dropveil{position:absolute;inset:8px;border:2px dashed #c1f0ad;border-radius:12px;display:grid;place-items:center;background:rgba(20,32,21,.85);color:#c1f0ad;font-weight:700;pointer-events:none}
  .wb-pane-head{display:flex;justify-content:space-between;align-items:baseline;gap:10px}
  .wb-pane-head h2{margin:0;font-size:15px}.wb-count{font-size:12px;color:#8c9c9b}
  .wb-link{background:transparent;color:#8fd6f2;font-size:12px;padding:0}
  .wb-search{width:100%;padding:8px 10px;border-radius:8px;border:1px solid #2c393a;background:#111617;color:#e8ecec;font-size:13px}
  .wb-search.inline{width:auto;flex:1;margin-left:auto;max-width:180px}
  .wb-tabs{display:flex;gap:4px;align-items:center}
  .wb-tabs button{background:transparent;color:#95a5a4;padding:6px 10px;border-radius:7px;font-size:13px}
  .wb-tabs button.on{background:#202b2d;color:#eff5f4}
  .wb-actions{display:flex;flex-wrap:wrap;gap:6px;align-items:center;padding:8px 10px;background:#1d2c22;border-radius:9px;font-size:12px;color:#c1f0ad}
  .wb-actions button{background:#273638;color:#e8ecec;border-radius:7px;padding:5px 10px;font-size:12px}
  .wb-actions button:disabled{opacity:.4}.wb-actions .danger{background:#5a352c;color:#f4cfc4;margin-left:auto}
  .wb-list{display:flex;flex-direction:column;gap:2px;overflow:auto;flex:1;min-height:0}
  .wb-row{display:flex;align-items:center;gap:10px;padding:8px 10px;border-radius:9px;border:1px solid transparent;cursor:default}
  .wb-row:hover{background:#1c2729}.wb-row.sel{background:#1d2c22;border-color:#2f4a37}.wb-row.dim{opacity:.55}
  .wb-row[draggable=true]{cursor:grab}
  .wb-row.removing{opacity:.6}.wb-row.removing .wb-meta strong{text-decoration:line-through}
  .wb-row.incoming{background:#182619;border:1px dashed #3a5a41}
  .wb-plus{width:16px;text-align:center;color:#c1f0ad;font-weight:700}
  .wb-art{flex-shrink:0;width:38px;height:38px;border-radius:7px;background:#273638;display:grid;place-items:center;color:#5c716f}
  .wb-meta{display:flex;flex-direction:column;min-width:0;flex:1}
  .wb-meta strong{font-size:14px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .wb-meta small{font-size:12px;color:#8c9c9b;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .wb-badge{font-size:11px;font-weight:650;padding:3px 8px;border-radius:99px;background:#273032;color:#b7c3c2;flex-shrink:0}
  .wb-badge.new{background:#1f3550;color:#a9d0f5}.wb-badge.changed{background:#3d3220;color:#f0d59a}.wb-badge.queued{background:#234029;color:#c5f7ad}.wb-badge.removing{background:#3a2420;color:#f0c7ba}
  .wb-pane-foot{display:flex;justify-content:space-between;align-items:center;gap:10px;font-size:12px;color:#8c9c9b;margin-top:auto;padding-top:6px}
  .wb-empty{text-align:center;padding:40px 10px;color:#8c9c9b}.wb-empty b{color:#e8ecec}
  .wb-tray{flex-shrink:0;background:#161f20;border:1px solid #2c393a;border-radius:14px;padding:14px 16px}
  .wb-tray-head{display:flex;justify-content:space-between;align-items:center;gap:16px;flex-wrap:wrap}
  .wb-tray-head h2{margin:0;font-size:15px}.wb-tray-head p{margin:2px 0 0;font-size:12px;color:#8c9c9b}
  .wb-tray-actions{display:flex;align-items:center;gap:10px}
  .wb-slownote{font-size:12px;color:#f0d59a}.wb-slownote.bad{color:#ff9d8a;font-weight:650}
  .wb-chips{list-style:none;margin:10px 0 0;padding:0;display:flex;flex-wrap:wrap;gap:8px;max-height:72px;overflow:auto}
  .wb-chips li{display:flex;align-items:center;gap:8px;padding:5px 6px 5px 10px;border-radius:99px;background:#1d2c22;border:1px solid #2f4a37;font-size:12px}
  .wb-chips li.remove{background:#2a1d1a;border-color:#5a352c}.wb-chips li.edit{background:#1f2836;border-color:#33465f}
  .wb-chips .k{font-weight:700}.wb-chips small{color:#8c9c9b}
  .wb-chips button{background:#273638;color:#c3d0cf;border-radius:99px;min-width:22px;height:22px;font-size:12px;padding:0 7px}
  .wb-toast{position:fixed;top:18px;left:50%;transform:translateX(-50%);background:#243033;border:1px solid #3b4a4b;padding:10px 16px;border-radius:10px;font-size:13px;box-shadow:0 10px 30px rgba(0,0,0,.4);z-index:80}
  .wb-veil{position:fixed;inset:0;background:rgba(5,9,10,.6);z-index:60}
  .wb-modal{position:fixed;z-index:70;top:50%;left:50%;transform:translate(-50%,-50%);width:min(460px,92vw);background:#1a2325;border:1px solid #344244;border-radius:16px;padding:22px;box-shadow:0 20px 60px rgba(0,0,0,.5)}
  .wb-modal h2{margin:0 0 10px;font-size:18px}.wb-modal p{color:#b7c3c2;font-size:14px;line-height:1.5;margin:0 0 12px}
  .wb-fine{font-size:12px!important;color:#8c9c9b!important}
  .wb-check{display:flex;gap:8px;align-items:flex-start;font-size:13px;color:#b7c3c2;margin:6px 0 14px}
  .wb-modal-actions{display:flex;justify-content:flex-end;gap:8px;margin-top:14px}
  .danger-btn{background:#e8a58f!important;color:#2a120b!important}
  .wb-review{margin:0 0 10px;display:grid;gap:6px}.wb-review div{display:flex;justify-content:space-between;gap:12px;font-size:14px}
  .wb-review dt{color:#8c9c9b}.wb-review dd{margin:0;text-align:right}
  .wb-progress{height:12px;background:#232e30;border-radius:99px;overflow:hidden;margin:6px 0 16px}.wb-progress i{display:block;height:100%;background:#c1f0ad;transition:width .1s}
  .wb-ok{color:#c1f0ad!important;font-size:15px!important}
  .wb-drawer{position:fixed;z-index:70;top:0;right:0;bottom:0;width:min(400px,94vw);background:#1a2325;border-left:1px solid #344244;padding:22px;display:flex;flex-direction:column;gap:12px;overflow:auto}
  .wb-drawer-head{display:flex;justify-content:space-between;align-items:center}.wb-drawer-head h2{margin:0;font-size:18px}
  .wb-drawer label{display:flex;flex-direction:column;gap:5px;font-size:12px;color:#8c9c9b}
  .wb-drawer input{padding:9px 10px;border-radius:8px;border:1px solid #2c393a;background:#111617;color:#e8ecec;font-size:14px}
  .wb-drawer fieldset{border:1px solid #2c393a;border-radius:10px;padding:10px;margin:0}.wb-drawer legend{font-size:12px;color:#8c9c9b;padding:0 6px}
  .wb-drawer small{color:#8c9c9b;font-size:11px;display:block;margin-top:8px}
  .wb-covers{display:flex;gap:8px;flex-wrap:wrap;align-items:center}
  .wb-covers button{width:44px;height:44px;border-radius:8px;border:2px solid transparent;padding:0}.wb-covers button.on{border-color:#c1f0ad}
  .wb-covers .wb-covers-file{width:auto;height:auto;padding:8px 10px;background:#273638;color:#e8ecec;font-size:12px}
  .wb-drawer .wb-modal-actions{margin-top:auto}
</style>
