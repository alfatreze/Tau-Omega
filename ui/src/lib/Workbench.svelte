<script lang="ts">
  // The card-centred library workbench (docs/LIBRARY_WORKBENCH_PLAN.md).
  // The card on the right and the local library on the left are both listed by
  // the engine (`list_library`); everything you do is staged in "Pending
  // changes" and only written when you press Start sync, as one reviewed plan
  // (`plan_changes` -> `execute_changes`). Pending changes are remembered per
  // card and core so switching cards switches the list.
  import { onDestroy, onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { cancelJob, checkStorageCapacity, detectConnection, errorMessage, executeChanges, getPrefs, listLibrary, newJobId, onProgress, planChanges, readImageDataUrl, setPrefs } from './tau-api';
  import type { AlbumInfo, ChangePlanView, ChangeReport, ChangeRequest, CapacityCheck, ConnectionInfo, EditRequest, FieldEdits, LibraryListing, PrefsView, TrackInfo } from './types';

  /** Root folder of the open card (used to detect how it is connected). */
  export let cardPath = '';
  export let cardLabel = 'Pocket';
  export let coreLabel = '';
  /** The selected core's media root on the card (`.../Assets/<platform>/common`). */
  export let mediaRoot = '';
  /** Bumped by the app when the card was re-read (auto-detect or the refresh button). */
  export let revision = 0;
  export let refresh: () => void | Promise<void> = () => {};
  export let refreshedAt = '';
  export let openSettings: () => void = () => {};

  type Pending =
    | { key: string; kind: 'add'; id: string; destId: string; title: string; artist: string; tracks: number; bytes: number }
    | { key: string; kind: 'remove'; id: string; title: string; artist: string; tracks: number; bytes: number }
    | { key: string; kind: 'edit'; id: string; track: string | null; title: string; fields: FieldEdits; cover: string | null; coverPreview: string | null; note: string };

  const MB = 1e6, GB = 1e9;
  const SLOW_LIMIT = 10 * MB;
  const TRACK_LIMIT = 300;
  const STAGES: Record<string, string> = { scanning: 'Reading files', hashing: 'Checking files', copying: 'Copying', building_index: 'Writing index', verifying: 'Verifying', deleting: 'Removing', editing: 'Updating tags' };

  // ---- persistence (per card + core) ---------------------------------------
  const storeKey = (name: string) => `tau.wb.${name}.${mediaRoot}`;
  function load<T>(name: string, fallback: T): T { try { const raw = localStorage.getItem(storeKey(name)); return raw ? (JSON.parse(raw) as T) : fallback; } catch { return fallback; } }
  function save(name: string, value: unknown) { try { localStorage.setItem(storeKey(name), JSON.stringify(value)); } catch { /* a per-viewer convenience only */ } }

  // ---- state ---------------------------------------------------------------
  let loadedFor = '\u0000';
  let source: LibraryListing | null = null;
  let sourcePath = '';
  let sourceBusy = false, sourceError = '', sourceProgress = '';
  let card: LibraryListing | null = null;
  let cardBusy = false, cardError = '', cardProgress = '';
  let space: CapacityCheck | null = null;
  let prefs: PrefsView | null = null;
  let connection: ConnectionInfo = { kind: 'unknown', detail: '' };
  let embedCovers = true;

  let sourceSearch = '', pocketSearch = '';
  let picked = new Set<string>();       // ticked albums in "This computer"
  let pocketPicked = new Set<string>(); // ticked albums / tracks "On Pocket"
  let view: 'albums' | 'artists' | 'tracks' | 'playlists' = 'albums';
  let pending: Pending[] = [];
  let dropActive = false;
  let toast = '';
  let toastTimer: ReturnType<typeof setTimeout>;
  let spinning = false;
  let connMenu = false;

  let dialog: null | 'review' | 'slow' | 'first-remove' | 'edit' = null;
  let review: ChangePlanView | null = null;
  let reviewError = '';
  let slowDontAsk = false;
  let askBackup = true;

  type Run = { phase: string; done: number; total: number; path: string; startedAt: number; speed: number; samples: { t: number; done: number }[]; finished: boolean; error: string; report: ChangeReport | null; cancelled: boolean };
  let run: Run | null = null;
  let runId = '';
  let unlisten: (() => void) | undefined;

  onMount(async () => {
    unlisten = await onProgress((event) => {
      if (event.job_id === runId && run && !run.finished) handleRunProgress(event.stage, event.done, event.total, event.path ?? '');
      else if (event.job_id === cardJob) cardProgress = `${event.done} / ${event.total || '…'}`;
      else if (event.job_id === sourceJob) sourceProgress = `${event.done} / ${event.total || '…'}`;
    });
    try { prefs = await getPrefs(); } catch { /* defaults are applied below */ }
  });
  onDestroy(() => { unlisten?.(); clearTimeout(toastTimer); });

  // ---- loading -------------------------------------------------------------
  let cardJob = '', sourceJob = '';
  $: if (mediaRoot !== loadedFor) init();
  $: if (revision) reloadCard();

  async function init() {
    loadedFor = mediaRoot;
    pending = mediaRoot ? load<Pending[]>('pending', []) : [];
    sourcePath = mediaRoot ? load<string>('source', '') : '';
    embedCovers = mediaRoot ? load<boolean>('embed', true) : true;
    picked = new Set(); pocketPicked = new Set(); source = null; card = null; space = null;
    if (sourcePath) scanSource();
    await reloadCard();
    if (cardPath) detectConnection(cardPath).then((c) => (connection = c)).catch(() => (connection = { kind: 'unknown', detail: '' }));
  }
  async function reloadCard() {
    if (!mediaRoot) return;
    cardBusy = true; cardError = ''; cardJob = newJobId(); cardProgress = '';
    try {
      card = await listLibrary(mediaRoot, cardJob);
      space = await checkStorageCapacity(mediaRoot, 0).catch(() => null);
      // Drop staged removals/edits for albums that are no longer on the card.
      const here = new Set(card.albums.map((a) => a.id));
      const kept = pending.filter((p) => p.kind === 'add' || here.has(p.id));
      if (kept.length !== pending.length) pending = kept;
    } catch (error) { card = null; cardError = errorMessage(error); }
    finally { cardBusy = false; }
  }
  async function scanSource() {
    if (!sourcePath) return;
    sourceBusy = true; sourceError = ''; sourceJob = newJobId(); sourceProgress = '';
    try { source = await listLibrary(sourcePath, sourceJob); }
    catch (error) { source = null; sourceError = errorMessage(error); }
    finally { sourceBusy = false; }
  }
  async function chooseSource() {
    const selected = await open({ directory: true });
    if (!selected || Array.isArray(selected)) return;
    sourcePath = selected; save('source', sourcePath); picked = new Set();
    await scanSource();
  }
  async function doRefresh() { spinning = true; try { await refresh(); } finally { setTimeout(() => (spinning = false), 700); } }
  $: save_pending(pending);
  function save_pending(p: Pending[]) { if (loadedFor === mediaRoot && mediaRoot) save('pending', p); }
  $: if (mediaRoot && loadedFor === mediaRoot) save('embed', embedCovers);

  // ---- derived numbers -------------------------------------------------------
  $: adds = pending.filter((p): p is Extract<Pending, { kind: 'add' }> => p.kind === 'add');
  $: removes = pending.filter((p): p is Extract<Pending, { kind: 'remove' }> => p.kind === 'remove');
  $: edits = pending.filter((p): p is Extract<Pending, { kind: 'edit' }> => p.kind === 'edit');
  $: addBytes = adds.reduce((n, p) => n + p.bytes, 0);
  $: removeBytes = removes.reduce((n, p) => n + p.bytes, 0);
  $: addTracks = adds.reduce((n, p) => n + p.tracks, 0);
  $: total = space?.space.total_bytes ?? 0;
  $: used = space ? space.space.total_bytes - space.space.available_bytes : 0;
  $: free = (space?.space.available_bytes ?? 0) - addBytes + removeBytes;
  $: overCapacity = !!space && free < (space.margin_bytes ?? 0);
  $: pct = (n: number) => `${Math.max(0, Math.min(100, total ? (n / total) * 100 : 0))}%`;
  $: queuedIds = new Set(adds.map((p) => p.id));
  $: removingIds = new Set(removes.map((p) => p.id));
  $: editingIds = new Set(edits.map((p) => p.id));
  $: cardByDest = new Map((card?.albums ?? []).map((a) => [a.id, a]));
  $: connKind = (prefs?.connections?.[cardLabel] && prefs.connections[cardLabel] !== 'auto' ? prefs.connections[cardLabel] : connection.kind) as ConnectionInfo['kind'];
  $: lastSpeed = prefs?.speeds?.[cardLabel] ?? 0;
  $: slowApplies = connKind === 'direct_usb' && addBytes > SLOW_LIMIT;
  $: pendingCount = pending.length;
  $: canStart = pendingCount > 0 && !overCapacity && !run && !!card;
  $: pocketAlbums = (card?.albums ?? []).filter((a) => match(`${a.title} ${a.artist}`, pocketSearch));
  $: sourceAlbums = (source?.albums ?? []).filter((a) => match(`${a.title} ${a.artist}`, sourceSearch));
  $: artists = [...pocketAlbums.reduce((m, a) => m.set(a.artist || 'Unknown artist', [...(m.get(a.artist || 'Unknown artist') ?? []), a]), new Map<string, AlbumInfo[]>())];
  $: allTracks = (card?.tracks ?? []).filter((t) => match(`${t.title} ${t.artist} ${t.album}`, pocketSearch));
  $: tracksShown = allTracks.slice(0, TRACK_LIMIT);

  const size = (n: number) => (n >= GB ? `${(n / GB).toFixed(1)} GB` : n >= MB ? `${Math.round(n / MB)} MB` : `${Math.max(1, Math.round(n / 1000))} KB`);
  const match = (text: string, q: string) => !q.trim() || text.toLowerCase().includes(q.trim().toLowerCase());
  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
  const duration = (secs: number) => `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, '0')}`;
  function eta(bytes: number, speed: number): string { if (!speed) return ''; const s = Math.ceil(bytes / speed); return s < 90 ? `${s} sec` : s < 5400 ? `${Math.round(s / 60)} min` : `${(s / 3600).toFixed(1)} h`; }
  $: lastEta = lastSpeed && addBytes ? eta(addBytes, lastSpeed) : '';

  type AlbumState = 'queued' | 'on' | 'changed' | 'new';
  // Takes its inputs as parameters (not closure variables) so the template
  // re-renders the badges when the queue or the card listing changes.
  function stateOf(a: AlbumInfo, queued: Set<string>, onCardBy: Map<string, AlbumInfo>): AlbumState {
    if (queued.has(a.id)) return 'queued';
    const onCard = onCardBy.get(a.dest_id);
    if (!onCard) return 'new';
    return onCard.tracks === a.tracks ? 'on' : 'changed';
  }
  const stateLabel: Record<AlbumState, string> = { on: 'On Pocket', queued: 'Queued', changed: 'Changed', new: 'New' };

  function say(message: string) { toast = message; clearTimeout(toastTimer); toastTimer = setTimeout(() => (toast = ''), 3600); }
  function toggle(set: Set<string>, id: string) { const next = new Set(set); next.has(id) ? next.delete(id) : next.add(id); return next; }

  // ---- staging -------------------------------------------------------------
  function addAlbums(ids: string[]) {
    const wanted = ids.map((id) => source?.albums.find((a) => a.id === id)).filter((a): a is AlbumInfo => !!a);
    const fresh: AlbumInfo[] = []; let blocked = 0;
    for (const a of wanted) {
      const st = stateOf(a, queuedIds, cardByDest);
      if (st === 'on' || st === 'queued') continue;
      if (removingIds.has(a.dest_id) || editingIds.has(a.dest_id)) { blocked++; continue; }
      fresh.push(a);
    }
    if (blocked) say(`${plural(blocked, 'album')} already ha${blocked === 1 ? 's' : 've'} a pending removal or edit. Sync that first, or undo it.`);
    if (!fresh.length) { if (!blocked) say('Those albums are already on the Pocket.'); return; }
    pending = [...pending, ...fresh.map((a) => ({ key: `add:${a.id}`, kind: 'add' as const, id: a.id, destId: a.dest_id, title: a.title, artist: a.artist, tracks: a.tracks, bytes: a.bytes }))];
    picked = new Set();
    say(`${plural(fresh.length, 'album')} added to Pending changes.`);
  }
  const addPicked = () => addAlbums([...picked]);
  function onDragStart(e: DragEvent, a: AlbumInfo) { const ids = picked.has(a.id) ? [...picked] : [a.id]; e.dataTransfer?.setData('text/tau-albums', JSON.stringify(ids)); if (e.dataTransfer) e.dataTransfer.effectAllowed = 'copy'; }
  function onDrop(e: DragEvent) { e.preventDefault(); dropActive = false; const raw = e.dataTransfer?.getData('text/tau-albums'); if (raw) { try { addAlbums(JSON.parse(raw)); } catch { /* not ours */ } } }

  function requestRemove() { if (!pocketPicked.size) return; if (prefs && !prefs.remove_explained) dialog = 'first-remove'; else stageRemoval(); }
  async function confirmFirstRemove() {
    dialog = null;
    if (prefs) { try { prefs = await setPrefs({ ...prefs, remove_explained: true }); } catch { /* the note simply shows again */ } }
    stageRemoval();
  }
  function stageRemoval() {
    const items = (card?.albums ?? []).filter((a) => pocketPicked.has(a.id) && !removingIds.has(a.id));
    const clash = items.filter((a) => [...queuedIds].some((id) => source?.albums.find((s) => s.id === id)?.dest_id === a.id));
    const ok = items.filter((a) => !clash.includes(a));
    if (clash.length) say('An album that is queued to be added can’t also be removed. Undo the add first.');
    pending = [...pending.filter((p) => !(p.kind === 'edit' && ok.some((a) => a.id === p.id))), ...ok.map((a) => ({ key: `rm:${a.id}`, kind: 'remove' as const, id: a.id, title: a.title, artist: a.artist, tracks: a.tracks, bytes: a.bytes }))];
    pocketPicked = new Set();
    if (ok.length) say(`${plural(ok.length, 'album')} marked for removal. Nothing is deleted until you sync.`);
  }
  function unstage(key: string) { pending = pending.filter((p) => p.key !== key); }
  function clearAll() { const n = pending.length; pending = []; picked = new Set(); pocketPicked = new Set(); say(`Cleared ${plural(n, 'pending change')}.`); }

  // ---- editing ---------------------------------------------------------------
  let editAlbum: AlbumInfo | null = null;
  let editTrack: TrackInfo | null = null;
  let editForm = { title: '', artist: '', albumArtist: '', year: '' };
  let editCover: string | null = null, editCoverPreview: string | null = null, editCoverError = '';
  function openEdit(focus: 'title' | 'cover' = 'title') {
    editCover = null; editCoverPreview = null; editCoverError = '';
    if (view === 'tracks') {
      const t = (card?.tracks ?? []).find((x) => pocketPicked.has(x.rel)); if (!t) return;
      editTrack = t; editAlbum = null; editForm = { title: t.title, artist: '', albumArtist: '', year: '' };
    } else {
      const a = (card?.albums ?? []).find((x) => pocketPicked.has(x.id)); if (!a) return;
      if (queuedIds.size && [...queuedIds].some((id) => source?.albums.find((s) => s.id === id)?.dest_id === a.id)) { say('This album is queued to be added. Sync first, then edit it.'); return; }
      editAlbum = a; editTrack = null; editForm = { title: a.title, artist: a.artist, albumArtist: '', year: a.year ?? '' };
    }
    dialog = 'edit';
    setTimeout(() => document.getElementById(focus === 'cover' ? 'cover-choose' : 'edit-title')?.focus(), 30);
  }
  async function chooseCover() {
    editCoverError = '';
    const selected = await open({ multiple: false, filters: [{ name: 'JPEG image', extensions: ['jpg', 'jpeg'] }] });
    if (!selected || Array.isArray(selected)) return;
    editCover = selected;
    try { editCoverPreview = await readImageDataUrl(selected); } catch { editCoverPreview = null; editCoverError = 'Could not preview this image. It will be checked when you review the sync.'; }
  }
  function stageEdit() {
    const fields: FieldEdits = { title: null, artist: null, album: null, album_artist: null, year: null };
    const notes: string[] = [];
    if (editTrack) {
      if (editForm.title.trim() && editForm.title !== editTrack.title) { fields.title = editForm.title.trim(); notes.push('title'); }
      dialog = null;
      if (!notes.length) return;
      const t = editTrack;
      pending = [...pending.filter((p) => p.key !== `ed:t:${t.rel}`), { key: `ed:t:${t.rel}`, kind: 'edit', id: t.album_id, track: t.rel, title: t.title, fields, cover: null, coverPreview: null, note: notes.join(', ') }];
    } else if (editAlbum) {
      const a = editAlbum;
      if (editForm.title.trim() && editForm.title !== a.title) { fields.album = editForm.title.trim(); notes.push('title'); }
      if (editForm.artist.trim() && editForm.artist !== a.artist) { fields.artist = editForm.artist.trim(); notes.push('artist'); }
      if (editForm.albumArtist.trim()) { fields.album_artist = editForm.albumArtist.trim(); notes.push('album artist'); }
      if (editForm.year.trim() && editForm.year !== (a.year ?? '')) { fields.year = editForm.year.trim(); notes.push('year'); }
      if (editCover) notes.push('cover');
      dialog = null;
      if (!notes.length) return;
      pending = [...pending.filter((p) => p.key !== `ed:${a.id}`), { key: `ed:${a.id}`, kind: 'edit', id: a.id, track: null, title: a.title, fields, cover: editCover, coverPreview: editCoverPreview, note: notes.join(', ') }];
    }
    pocketPicked = new Set(); say('Edit added to Pending changes.');
  }

  // ---- sync ------------------------------------------------------------------
  function buildRequest(): ChangeRequest {
    const editRequests: EditRequest[] = edits.map((p) => ({ album_id: p.id, track: p.track, fields: p.fields, cover: p.cover }));
    return { library_root: sourcePath || null, add_albums: adds.map((p) => p.id), remove_albums: removes.map((p) => p.id), edits: editRequests, options: { mirror: false, embed_covers: embedCovers, art_sidecar_pal256: false } };
  }
  async function start() {
    if (!canStart) return;
    reviewError = ''; review = null;
    try { review = await planChanges(buildRequest(), mediaRoot); }
    catch (error) { reviewError = errorMessage(error); dialog = 'review'; return; }
    const needsAlert = connKind === 'direct_usb' && review.bytes_to_write > SLOW_LIMIT && !(prefs?.slow_alert_suppressed);
    dialog = needsAlert ? 'slow' : 'review';
  }
  /** The plan (and its confirmation token) depends on the options, so changing one in the review sheet re-plans. */
  async function replan() {
    try { review = await planChanges(buildRequest(), mediaRoot); reviewError = ''; }
    catch (error) { review = null; reviewError = errorMessage(error); }
  }
  async function slowContinue() {
    if (slowDontAsk && prefs) { try { prefs = await setPrefs({ ...prefs, slow_alert_suppressed: true }); } catch { /* the alert simply shows again */ } }
    dialog = 'review';
  }
  async function confirmSync() {
    if (!review) return;
    dialog = null;
    const mode = prefs?.remove_mode ?? 'backup';
    const backupOn = mode === 'backup' || (mode === 'ask' && askBackup);
    const backup = review.removed_files && backupOn ? (prefs?.backup_dir || prefs?.default_backup_dir || null) : null;
    runId = newJobId();
    run = { phase: 'Starting', done: 0, total: review.bytes_to_write, path: '', startedAt: Date.now(), speed: 0, samples: [], finished: false, error: '', report: null, cancelled: false };
    const startedAt = Date.now();
    try {
      const report = await executeChanges(buildRequest(), mediaRoot, review.id, backup, runId);
      const seconds = (Date.now() - startedAt) / 1000;
      if (prefs && report.bytes_written >= 5 * MB && seconds >= 2) {
        try { prefs = await setPrefs({ ...prefs, speeds: { ...prefs.speeds, [cardLabel]: report.bytes_written / seconds } }); } catch { /* speed memory is best-effort */ }
      }
      run = { ...run!, phase: 'Done', finished: true, report };
      pending = []; picked = new Set(); pocketPicked = new Set();
      await reloadCard();
    } catch (error) {
      const message = errorMessage(error);
      run = { ...run!, finished: true, cancelled: /cancel/i.test(message), error: message };
      await reloadCard();
    }
  }
  function handleRunProgress(stage: string, done: number, totalUnits: number, path: string) {
    if (!run) return;
    const now = Date.now();
    let { samples, speed } = run;
    if (stage === 'copying') {
      samples = [...samples.filter((s) => now - s.t < 4000), { t: now, done }];
      if (samples.length > 1) { const a = samples[0], b = samples[samples.length - 1]; if (b.t > a.t) speed = ((b.done - a.done) / (b.t - a.t)) * 1000; }
    }
    run = { ...run, phase: STAGES[stage] ?? stage, done: stage === 'copying' ? done : run.done, total: stage === 'copying' ? totalUnits : run.total, path, samples, speed };
  }
  async function cancelSync() { if (runId) await cancelJob(runId); }
  function closeRun() { run = null; }
  $: runSlowNote = run && !run.finished && connKind !== 'direct_usb' && run.speed > 0 && run.speed < 3 * MB && run.total > SLOW_LIMIT;

  /** Dialog behaviour: focus moves in when it opens, Tab stays inside, and focus returns to where it was when it closes. */
  function modal(node: HTMLElement) {
    const previous = document.activeElement as HTMLElement | null;
    const focusable = () => [...node.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), [href], select, textarea, [tabindex]:not([tabindex="-1"])')];
    (node.querySelector<HTMLElement>('input:not([type=checkbox])') ?? node.querySelector<HTMLElement>('button.primary') ?? focusable()[0])?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return;
      const items = focusable(); if (!items.length) return;
      const first = items[0], last = items[items.length - 1];
      if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
    };
    node.addEventListener('keydown', onKey);
    return { destroy() { node.removeEventListener('keydown', onKey); previous?.isConnected && previous.focus(); } };
  }
  function key(e: KeyboardEvent) { if (e.key === 'Escape') { if (dialog) dialog = null; connMenu = false; } }
  async function setOverride(value: string) {
    connMenu = false;
    if (!prefs) return;
    try { prefs = await setPrefs({ ...prefs, connections: { ...prefs.connections, [cardLabel]: value } }); } catch { /* keeps the detected value */ }
  }
  $: connLabel = connKind === 'direct_usb' ? 'Direct USB · slow' : connKind === 'card_reader' ? 'Card reader · fast' : 'Connection unknown';
  $: noCard = !mediaRoot;
</script>
<svelte:window on:keydown={key} />

<section class="wb" aria-labelledby="wb-title">
  <header class="wb-top">
    <div class="wb-ctx">
      <p class="eyebrow">LIBRARY</p>
      <h1 id="wb-title">{cardLabel}</h1>
      <div class="wb-sub"><span class="wb-dot" aria-hidden="true"></span>{coreLabel || 'No player core'}
        {#if cardPath}
          <span class="wb-connwrap">
            <button class="wb-conn" class:slow={connKind === 'direct_usb'} aria-haspopup="menu" aria-expanded={connMenu} title={connection.detail ? `Detected: ${connection.detail}` : 'How this card is connected'} on:click={() => (connMenu = !connMenu)}>{connLabel}</button>
            {#if connMenu}
              <div class="wb-menu" role="menu">
                <button role="menuitem" on:click={() => setOverride('auto')}>Detect automatically{connection.detail ? ` (${connection.detail})` : ''}</button>
                <button role="menuitem" on:click={() => setOverride('direct_usb')}>This is the Pocket (direct USB)</button>
                <button role="menuitem" on:click={() => setOverride('card_reader')}>This is a card reader</button>
              </div>
            {/if}
          </span>
        {/if}
      </div>
    </div>
    <button class="wb-refresh" class:spin={spinning} aria-label="Refresh this card" title={refreshedAt ? `Refresh · updated ${refreshedAt}` : 'Refresh'} on:click={doRefresh}>
      <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 12a8 8 0 1 1-2.6-5.9"/><path d="M20 4v5h-5"/></svg>
    </button>
  </header>

  {#if noCard}
    <section class="empty"><div class="empty-art">◒</div><h2>No card selected</h2><p>Connect your Pocket or a card reader, or choose a card on the Cards screen. It will appear here automatically.</p></section>
  {:else}
  <div class="wb-cap" role="group" aria-label="Storage on the Pocket">
    {#if space}
      <div class="wb-cap-text">
        <span><b>{size(used)}</b> on Pocket</span>
        {#if addBytes}<span class="add">+ {size(addBytes)} queued</span>{/if}
        {#if removeBytes}<span class="rm">− {size(removeBytes)} removing</span>{/if}
        <span class="wb-cap-free" class:bad={overCapacity}>{overCapacity ? `${size(Math.max(0, (space.margin_bytes ?? 0) - free))} too much for this card` : `${size(free)} free of ${size(total)}`}</span>
      </div>
      <div class="wb-bar" role="img" aria-label={`${size(used)} used, ${size(addBytes)} queued, ${size(Math.max(free, 0))} free`}>
        <i class="k-used" style="width:{pct(used - removeBytes)}"></i><i class="k-rm" style="width:{pct(removeBytes)}"></i><i class="k-add" class:over={overCapacity} style="width:{pct(addBytes)}"></i>
      </div>
    {:else}
      <div class="wb-cap-text"><span>{cardBusy ? 'Reading the card…' : 'Storage information is not available for this card.'}</span></div>
    {/if}
  </div>

  <div class="wb-panes">
    <section class="wb-pane" aria-labelledby="src-title">
      <div class="wb-pane-head"><h2 id="src-title">This computer</h2>{#if sourcePath}<button class="wb-link" title={sourcePath} on:click={chooseSource}>{sourcePath.split(/[\\/]/).filter(Boolean).pop()} · change</button>{/if}</div>
      {#if !sourcePath}
        <div class="wb-empty wb-cta"><b>Choose your music folder</b><p>Pick a folder on this computer to see its albums. Your files are never changed.</p><button class="primary" on:click={chooseSource}>Choose folder…</button></div>
      {:else if sourceBusy}
        <div class="wb-empty" role="status"><b>Reading your library…</b><p>{sourceProgress}</p></div>
      {:else if sourceError}
        <div class="wb-empty" role="alert"><b>Couldn't read this folder</b><p>{sourceError}</p><button class="quiet" on:click={scanSource}>Try again</button></div>
      {:else if source}
        <input class="wb-search" bind:value={sourceSearch} placeholder="Search albums or artists" aria-label="Search this computer" />
        <div class="wb-list" role="list">
          {#each sourceAlbums as a (a.id)}
            {@const st = stateOf(a, queuedIds, cardByDest)}
            <div class="wb-row" class:sel={picked.has(a.id)} class:dim={st === 'on'} role="listitem" draggable={st !== 'on' && st !== 'queued'} on:dragstart={(e) => onDragStart(e, a)}>
              <input type="checkbox" aria-label={`Select ${a.title}`} disabled={st === 'on' || st === 'queued'} checked={picked.has(a.id)} on:change={() => (picked = toggle(picked, a.id))} />
              <div class="wb-art" aria-hidden="true">♪</div>
              <div class="wb-meta"><strong>{a.title}</strong><small>{a.artist || 'Unknown artist'} · {plural(a.tracks, 'track')} · {size(a.bytes)}</small></div>
              <span class="wb-badge {st}">{stateLabel[st]}</span>
            </div>
          {:else}
            <div class="wb-empty"><b>{source.albums.length ? 'No albums match' : 'No music found'}</b><p>{source.albums.length ? 'Try a different search.' : 'This folder has no MP3 or FLAC files.'}</p></div>
          {/each}
        </div>
        <div class="wb-pane-foot">
          <span>{picked.size ? `${picked.size} selected` : `${plural(source.albums.length, 'album')} · tick albums or drag them across →`}</span>
          <button class="primary" disabled={!picked.size} on:click={addPicked}>Add {picked.size || ''} to Pocket →</button>
        </div>
      {/if}
    </section>

    <section class="wb-pane wb-drop" class:active={dropActive} aria-labelledby="pk-title"
      on:dragover|preventDefault={() => (dropActive = true)} on:dragleave={() => (dropActive = false)} on:drop={onDrop}>
      <div class="wb-pane-head"><h2 id="pk-title">On Pocket</h2>{#if card}<span class="wb-count">{plural(card.albums.length, 'album')} · {plural(card.tracks.length, 'track')}</span>{/if}</div>
      {#if cardBusy && !card}
        <div class="wb-empty" role="status"><b>Reading the card…</b><p>{cardProgress}</p></div>
      {:else if cardError}
        <div class="wb-empty" role="alert"><b>Couldn't read this core's music folder</b><p>{cardError}</p><button class="quiet" on:click={reloadCard}>Try again</button></div>
      {:else if card}
        <div class="wb-tabs">
          <div class="wb-tablist" role="tablist" aria-label="View">
            {#each ['albums', 'artists', 'tracks', 'playlists'] as t}<button role="tab" aria-selected={view === t} class:on={view === t} on:click={() => { view = t as typeof view; pocketPicked = new Set(); }}>{t[0].toUpperCase() + t.slice(1)}</button>{/each}
          </div>
          <input class="wb-search inline" bind:value={pocketSearch} placeholder="Search" aria-label="Search the Pocket" />
        </div>
        {#if pocketPicked.size && (view === 'albums' || view === 'tracks')}
          <div class="wb-actions" role="toolbar" aria-label="Actions for the selection">
            <span>{pocketPicked.size} selected</span>
            <button on:click={() => openEdit('title')} disabled={pocketPicked.size !== 1}>Rename</button>
            {#if view === 'albums'}
              <button on:click={() => openEdit('title')} disabled={pocketPicked.size !== 1}>Edit info</button>
              <button on:click={() => openEdit('cover')} disabled={pocketPicked.size !== 1}>Change cover</button>
              <button class="danger" on:click={requestRemove}>Remove</button>
            {/if}
          </div>
        {/if}
        <div class="wb-list" role="list">
          {#if view === 'albums'}
            {#each pocketAlbums as a (a.id)}
              <div class="wb-row" class:sel={pocketPicked.has(a.id)} class:removing={removingIds.has(a.id)} role="listitem">
                <input type="checkbox" aria-label={`Select ${a.title}`} disabled={removingIds.has(a.id)} checked={pocketPicked.has(a.id)} on:change={() => (pocketPicked = toggle(pocketPicked, a.id))} />
                <div class="wb-art" aria-hidden="true">♪</div>
                <div class="wb-meta"><strong>{a.title}</strong><small>{a.artist || 'Unknown artist'} · {plural(a.tracks, 'track')} · {size(a.bytes)}</small></div>
                {#if removingIds.has(a.id)}<span class="wb-badge removing">Will be removed</span>{:else if editingIds.has(a.id)}<span class="wb-badge edit">Edit pending</span>{/if}
              </div>
            {/each}
            {#each adds as p (p.key)}
              <div class="wb-row incoming" role="listitem"><span class="wb-plus" aria-hidden="true">+</span><div class="wb-art" aria-hidden="true">♪</div><div class="wb-meta"><strong>{p.title}</strong><small>{p.artist || 'Unknown artist'} · {plural(p.tracks, 'track')} · {size(p.bytes)}</small></div><span class="wb-badge queued">Will be added</span></div>
            {/each}
            {#if !pocketAlbums.length && !adds.length}<div class="wb-empty"><b>{card.albums.length ? 'No albums match' : 'Nothing here yet'}</b><p>{card.albums.length ? 'Try a different search.' : 'Add albums from This computer to get started.'}</p></div>{/if}
          {:else if view === 'artists'}
            {#each artists as [name, list] (name)}<div class="wb-row" role="listitem"><div class="wb-art" aria-hidden="true">♪</div><div class="wb-meta"><strong>{name}</strong><small>{plural(list.length, 'album')} · {plural(list.reduce((n, a) => n + a.tracks, 0), 'track')}</small></div></div>{/each}
          {:else if view === 'tracks'}
            {#each tracksShown as t (t.rel)}
              <div class="wb-row" class:sel={pocketPicked.has(t.rel)} role="listitem">
                <input type="checkbox" aria-label={`Select ${t.title}`} checked={pocketPicked.has(t.rel)} on:change={() => (pocketPicked = toggle(pocketPicked, t.rel))} />
                <div class="wb-meta"><strong>{t.title}</strong><small>{t.artist || 'Unknown artist'} · {t.album || 'Unknown album'}</small></div>
                <span class="wb-time">{duration(t.secs)}</span><span class="wb-badge">{t.format}</span>
              </div>
            {/each}
            {#if allTracks.length > TRACK_LIMIT}<div class="wb-more">Showing the first {TRACK_LIMIT} of {allTracks.length}. Search to narrow the list.</div>{/if}
          {:else}
            {#each card.playlists as pl}<div class="wb-row" role="listitem"><div class="wb-art" aria-hidden="true">≡</div><div class="wb-meta"><strong>{pl.name}</strong><small>{plural(pl.tracks, 'track')} · managed on the Playlists page</small></div></div>
            {:else}<div class="wb-empty"><b>No playlists</b><p>Create them on the Playlists page.</p></div>{/each}
          {/if}
        </div>
      {/if}
      {#if dropActive}<div class="wb-dropveil">Drop to add to Pocket</div>{/if}
    </section>
  </div>

  <section class="wb-tray" aria-labelledby="tray-title">
    <div class="wb-tray-head">
      <div>
        <h2 id="tray-title">Pending changes</h2>
        <p>{#if pendingCount}{[adds.length && `${plural(adds.length, 'album')} to add (${plural(addTracks, 'track')}, ${size(addBytes)})`, removes.length && `${removes.length} to remove`, edits.length && `${edits.length} to edit`].filter(Boolean).join(' · ')}{:else}Nothing yet. Changes are only made when you press Start sync.{/if}</p>
      </div>
      <div class="wb-tray-actions">
        {#if slowApplies && (prefs?.slow_alert_suppressed)}<span class="wb-slownote" role="note">Direct connection: slow{lastEta ? ` · about ${lastEta}` : ''}</span>{/if}
        {#if overCapacity}<span class="wb-slownote bad" role="alert">Won't fit on this card</span>{/if}
        <button class="quiet" disabled={!pendingCount} on:click={clearAll}>Clear all</button>
        <button class="primary" disabled={!canStart} on:click={start}>Start sync</button>
      </div>
    </div>
    {#if pendingCount}
      <ul class="wb-chips">
        {#each pending as p (p.key)}
          <li class={p.kind}><span class="k">{p.kind === 'add' ? '+' : p.kind === 'remove' ? '−' : '✎'}</span><span class="t">{p.title}{p.kind === 'edit' ? ` (${p.note})` : ''}</span>{#if p.kind !== 'edit'}<small>{size(p.bytes)}</small>{/if}<button aria-label={p.kind === 'remove' ? `Undo removing ${p.title}` : `Remove ${p.title} from pending changes`} on:click={() => unstage(p.key)}>{p.kind === 'remove' ? 'Undo' : '×'}</button></li>
        {/each}
      </ul>
    {/if}
  </section>
  {/if}

  {#if toast}<div class="wb-toast" role="status">{toast}</div>{/if}
</section>

{#if dialog === 'slow'}
  <div class="wb-veil" role="presentation" on:click={() => (dialog = null)}></div>
  <div class="wb-modal" role="alertdialog" aria-labelledby="slow-t" aria-describedby="slow-d" use:modal>
    <h2 id="slow-t">You're connected to the Pocket directly</h2>
    <p id="slow-d">Transfers over this connection are slow and this may take a long time{lastEta ? ` (about ${lastEta} at your last speed)` : ''}. The Pocket's USB mode is meant for transfers under 10 MB; a card reader is much faster for larger syncs. Are you sure?</p>
    <label class="wb-check"><input type="checkbox" bind:checked={slowDontAsk} /> Don't ask again (you'll still see a small note next to Start sync)</label>
    <div class="wb-modal-actions"><button class="quiet" on:click={() => (dialog = null)}>Cancel</button><button class="primary" on:click={slowContinue}>Sync anyway</button></div>
  </div>
{/if}

{#if dialog === 'review'}
  <div class="wb-veil" role="presentation" on:click={() => (dialog = null)}></div>
  <div class="wb-modal" role="dialog" aria-labelledby="rev-t" use:modal>
    {#if reviewError}
      <h2 id="rev-t">This can't be synced yet</h2>
      <p role="alert">{reviewError}</p>
      <div class="wb-modal-actions"><button class="primary" on:click={() => (dialog = null)}>Back</button></div>
    {:else if review}
      <h2 id="rev-t">Ready to sync to {cardLabel}?</h2>
      <dl class="wb-review">
        {#if adds.length}<div><dt>Add / update</dt><dd>{plural(review.new_files + review.updated_files, 'file')} · {size(review.bytes_to_write)}{review.unchanged_files ? ` (${review.unchanged_files} already up to date)` : ''}</dd></div>{/if}
        {#if review.removed_files}<div><dt>Remove</dt><dd>{plural(review.removed_files, 'file')} · {size(review.bytes_to_remove)}{review.playlists_updated ? ` · ${plural(review.playlists_updated, 'playlist')} updated` : ''}</dd></div>{/if}
        {#if review.edited_files}<div><dt>Edit</dt><dd>{plural(review.edited_files, 'file')} (Pocket copies only)</dd></div>{/if}
        <div><dt>Free after</dt><dd>{size(free)} of {size(total)}</dd></div>
      </dl>
      {#if adds.length}<label class="wb-check"><input type="checkbox" bind:checked={embedCovers} on:change={replan} /> Embed each folder's cover art into the copies</label>{/if}
      {#if review.removed_files}
        {#if prefs?.remove_mode === 'ask'}
          <label class="wb-check"><input type="checkbox" bind:checked={askBackup} /> Copy removed files to my backup folder first</label>
        {:else if prefs?.remove_mode === 'none'}
          <p class="wb-fine">Removed files are deleted without a backup (Settings → Library).</p>
        {:else}
          <p class="wb-fine">Removed files are copied to your backup folder first (change in Settings → Library).</p>
        {/if}
      {/if}
      {#each review.warnings as w}<p class="wb-fine">⚠ {w.message}</p>{/each}
      <p class="wb-fine">Your music on this computer is never changed. Every file is verified, then the index is written last.</p>
      <div class="wb-modal-actions"><button class="quiet" on:click={() => (dialog = null)}>Back</button><button class="primary" on:click={confirmSync}>Confirm and start</button></div>
    {/if}
  </div>
{/if}

{#if dialog === 'first-remove'}
  <div class="wb-veil" role="presentation" on:click={() => (dialog = null)}></div>
  <div class="wb-modal" role="alertdialog" aria-labelledby="fr-t" use:modal>
    <h2 id="fr-t">Remove from the Pocket?</h2>
    <p>Removals are only <b>marked</b> now. Nothing is deleted until you press Start sync, and you can undo until then. By default the files are copied to a backup folder on this computer before they're deleted.</p>
    <p class="wb-fine">You can change this any time in <b>Settings → Library → Removing from the Pocket</b> (ask every time, back up then remove, or just remove).</p>
    <div class="wb-modal-actions"><button class="quiet" on:click={openSettings}>Open settings</button><button class="quiet" on:click={() => (dialog = null)}>Cancel</button><button class="primary danger-btn" on:click={confirmFirstRemove}>Mark for removal</button></div>
  </div>
{/if}

{#if dialog === 'edit' && (editAlbum || editTrack)}
  <div class="wb-veil" role="presentation" on:click={() => (dialog = null)}></div>
  <div class="wb-drawer" aria-labelledby="ed-t" role="dialog" aria-modal="true" use:modal>
    <div class="wb-drawer-head"><h2 id="ed-t">{editTrack ? 'Rename track' : 'Edit album'}</h2><button class="quiet" aria-label="Close" on:click={() => (dialog = null)}>✕</button></div>
    <p class="wb-fine">Changes apply to the copy on the Pocket only. Your files on this computer stay as they are, and your edits are re-applied on future syncs.</p>
    <label>{editTrack ? 'Track title' : 'Album title'}<input id="edit-title" bind:value={editForm.title} /></label>
    {#if editAlbum}
      <label>Artist<input bind:value={editForm.artist} /></label>
      <label>Album artist (optional)<input bind:value={editForm.albumArtist} placeholder="e.g. Various Artists" /></label>
      <label>Year<input inputmode="numeric" maxlength="4" bind:value={editForm.year} /></label>
      <fieldset><legend>Cover</legend>
        <div class="wb-cover-pick">
          {#if editCoverPreview}<img src={editCoverPreview} alt="Chosen cover" />{:else}<div class="wb-art big" aria-hidden="true">♪</div>{/if}
          <div><button id="cover-choose" class="wb-covers-file" on:click={chooseCover}>{editCover ? 'Choose a different image…' : 'Choose image…'}</button>{#if editCover}<small>{editCover.split(/[\\/]/).pop()}</small>{/if}</div>
        </div>
        {#if editCoverError}<small role="alert">{editCoverError}</small>{/if}
        <small>Covers must be baseline JPEG under 2 MiB; anything else is flagged before the sync starts.</small>
      </fieldset>
    {/if}
    <div class="wb-modal-actions"><button class="quiet" on:click={() => (dialog = null)}>Cancel</button><button class="primary" on:click={stageEdit}>Add to Pending changes</button></div>
  </div>
{/if}

{#if run}
  <div class="wb-veil" role="presentation"></div>
  <div class="wb-modal wb-run" role="dialog" aria-labelledby="run-t" aria-live="polite" use:modal>
    <h2 id="run-t">{run.finished ? (run.error ? (run.cancelled ? 'Sync cancelled' : 'Sync didn’t finish') : 'Sync complete') : `Syncing to ${cardLabel}`}</h2>
    {#if !run.finished}
      <div class="wb-progress" class:indeterminate={!run.total} role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={run.total ? Math.round((run.done / run.total) * 100) : undefined}><i style="width:{run.total ? (run.done / run.total) * 100 : 40}%"></i></div>
      <dl class="wb-review">
        <div><dt>Step</dt><dd>{run.phase}</dd></div>
        {#if run.total}<div><dt>Copied</dt><dd>{size(run.done)} of {size(run.total)}</dd></div>{/if}
        {#if run.speed > 0}<div><dt>Speed</dt><dd>{(run.speed / MB).toFixed(1)} MB/s</dd></div><div><dt>Time left</dt><dd>about {eta(run.total - run.done, run.speed)}</dd></div>{/if}
      </dl>
      {#if run.path}<p class="wb-fine wb-path" title={run.path}>{run.path}</p>{/if}
      {#if runSlowNote}<p class="wb-fine" role="note">This is slower than a card reader usually is. If you're plugged into the Pocket directly, that's expected.</p>{/if}
      <div class="wb-modal-actions"><button class="quiet" on:click={cancelSync}>Cancel sync</button></div>
    {:else if run.error}
      <p role="alert">{run.cancelled ? 'Stopped. Files already copied were kept and verified; the index was not changed until the end of a run, so the card still works.' : run.error}</p>
      <div class="wb-modal-actions"><button class="primary" on:click={closeRun}>Close</button></div>
    {:else if run.report}
      <p class="wb-ok">✓ {[run.report.copied && `${plural(run.report.copied, 'file')} copied`, run.report.deleted && `${plural(run.report.deleted, 'file')} removed`, run.report.edited && `${plural(run.report.edited, 'file')} edited`].filter(Boolean).join(', ') || 'Everything was already up to date'}. Index verified.</p>
      {#if run.report.backup_dir}<p class="wb-fine">Removed files were backed up to {run.report.backup_dir}</p>{/if}
      <div class="wb-modal-actions"><button class="primary" on:click={closeRun}>Done</button></div>
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
  .wb-search.inline{width:auto;flex:1 1 80px;min-width:0;margin-left:auto;max-width:180px}
  .wb-tabs{display:flex;gap:4px;align-items:center}.wb-tablist{display:flex;gap:4px}
  .wb-tabs button{background:transparent;color:#95a5a4;padding:6px 10px;border-radius:7px;font-size:13px}
  .wb-tabs button.on{background:#202b2d;color:#eff5f4}
  .wb-actions{display:flex;flex-wrap:wrap;gap:6px;align-items:center;padding:8px 10px;background:#1d2c22;border-radius:9px;font-size:12px;color:#c1f0ad}
  .wb-actions button{background:#273638;color:#e8ecec;border-radius:7px;padding:5px 10px;font-size:12px}
  .wb-actions button:disabled{opacity:.4}.wb-actions .danger{background:#5a352c;color:#f4cfc4;margin-left:auto}
  .wb-list{display:flex;flex-direction:column;gap:2px;overflow:auto;flex:1;min-height:0}
  .wb-row{display:flex;align-items:center;gap:10px;padding:8px 10px;border-radius:9px;border:1px solid transparent;cursor:default}
  .wb-row:hover{background:#1c2729}.wb-row.sel{background:#1d2c22;border-color:#2f4a37}.wb-row.dim .wb-art{opacity:.45}.wb-row.dim .wb-meta strong{color:#a6b3b2}
  .wb-row[draggable=true]{cursor:grab}
  .wb-row.removing .wb-art{opacity:.45}.wb-row.removing .wb-meta strong{text-decoration:line-through;color:#a6b3b2}
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
  .wb-progress.indeterminate i{animation:wb-slide 1.2s ease-in-out infinite alternate}
  @keyframes wb-slide{from{margin-left:0}to{margin-left:60%}}
  .wb-ok{color:#c1f0ad!important;font-size:15px!important}
  .wb-drawer{position:fixed;z-index:70;top:0;right:0;bottom:0;width:min(400px,94vw);background:#1a2325;border-left:1px solid #344244;padding:22px;display:flex;flex-direction:column;gap:12px;overflow:auto}
  .wb-drawer-head{display:flex;justify-content:space-between;align-items:center}.wb-drawer-head h2{margin:0;font-size:18px}
  .wb-drawer label{display:flex;flex-direction:column;gap:5px;font-size:12px;color:#8c9c9b}
  .wb-drawer input{padding:9px 10px;border-radius:8px;border:1px solid #2c393a;background:#111617;color:#e8ecec;font-size:14px}
  .wb-drawer fieldset{border:1px solid #2c393a;border-radius:10px;padding:10px;margin:0}.wb-drawer legend{font-size:12px;color:#8c9c9b;padding:0 6px}
  .wb-drawer small{color:#8c9c9b;font-size:11px;display:block;margin-top:8px}
  .wb-covers-file{width:auto;height:auto;padding:8px 10px;background:#273638;color:#e8ecec;font-size:12px}
  .wb-drawer .wb-modal-actions{margin-top:auto}
  .wb-connwrap{position:relative;display:inline-block}
  .wb-menu{position:absolute;top:calc(100% + 6px);left:0;z-index:50;min-width:260px;background:#1a2325;border:1px solid #344244;border-radius:10px;padding:6px;box-shadow:0 12px 32px rgba(0,0,0,.4);display:flex;flex-direction:column}
  .wb-menu button{background:transparent;color:#c7d1d0;text-align:left;padding:8px 10px;border-radius:7px;font-size:12px}.wb-menu button:hover{background:#202b2d}
  .wb-cta{display:flex;flex-direction:column;align-items:center;gap:6px;padding:44px 14px}.wb-cta p{margin:0 0 8px}
  .wb-more{padding:10px;font-size:12px;color:#8c9c9b;text-align:center}
  .wb-time{font-size:12px;color:#8c9c9b;font-variant-numeric:tabular-nums}
  .wb-badge.edit{background:#1f2836;color:#a9c3ea}
  .wb-cover-pick{display:flex;gap:12px;align-items:center}.wb-cover-pick img{width:72px;height:72px;object-fit:cover;border-radius:8px}
  .wb-art.big{width:72px;height:72px}
  .wb-path{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
</style>
