<script lang="ts">
  // The card-centred library workbench (docs/LIBRARY_WORKBENCH_PLAN.md).
  // The card on the right and the local library on the left are both listed by
  // the engine (`list_library`); everything you do is staged in "Pending
  // changes" and only written when you press Start sync, as one reviewed plan
  // (`plan_changes` -> `execute_changes`). Pending changes are remembered per
  // card and core so switching cards switches the list.
  import { onDestroy, onMount, tick } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { cancelJob, cardBreakdown, checkStorageCapacity, detectConnection, ejectCard, readbackStatus, explainError, executeChanges, getPrefs, listHistory, listLibrary, newJobId, onProgress, planChanges, albumThumbnails, imageThumbnail, setPrefs } from './tau-api';
  import { modal } from './a11y';
  import VirtualList from './VirtualList.svelte';
  import CapacityBar from './CapacityBar.svelte';
  import DetailsPanel from './DetailsPanel.svelte';
  import SyncToast from './SyncToast.svelte';
  import RingProgress from './RingProgress.svelte';
  import type { AlbumInfo, CardBreakdown, CoreRef, StagedItem, SyncView, ChangePlanView, ChangeResult, ChangeRequest, HistoryContext, CapacityCheck, ConnectionInfo, EditRequest, FieldEdits, LibraryListing, PrefsView, TrackInfo } from './types';

  /** Root folder of the open card (used to detect how it is connected). */
  export let cardPath = '';
  /** The cores on this card (id, name, platform, library-capable): which folders count as Tau media in the capacity bar. */
  export let cores: CoreRef[] = [];
  export let cardLabel = 'Pocket';
  export let coreLabel = '';
  /** The selected core's media root on the card (`.../Assets/<platform>/common`). */
  export let mediaRoot = '';
  /** Bumped by the app when the card was re-read (auto-detect or the refresh button). */
  export let revision = 0;
  export let refresh: () => void | Promise<void> = () => {};
  export let refreshedAt = '';
  export let openSettings: () => void = () => {};
  /** Whether the app still sees the card as mounted (`false` = unplugged, `null` = not a removable volume). */
  export let connected: boolean | null = null;
  /** True when this card was just ejected on purpose (Eject safely), so "not connected" is expected and calm, not an alarm. */
  export let ejectedSafely = false;
  /** Opens the sync history page, optionally on one entry (`'latest'` = the newest). */
  export let openHistory: (id?: string) => void = () => {};

  type Pending =
    | { key: string; kind: 'add'; id: string; destId: string; title: string; artist: string; tracks: number; bytes: number }
    | { key: string; kind: 'remove'; id: string; title: string; artist: string; tracks: number; bytes: number }
    | { key: string; kind: 'edit'; id: string; track: string | null; title: string; fields: FieldEdits; cover: string | null; coverPreview: string | null; note: string };

  const MB = 1e6, GB = 1e9;
  const SLOW_LIMIT = 10 * MB;
  /** Canonical fallback speeds (bytes per second) for the time estimate until a real transfer on this card has been
   * timed. These are placeholders to be replaced with real figures collected during development; the review sheet
   * always says when an estimate rests on them rather than on a measured transfer. */
  const DEFAULT_SPEED: Record<string, number> = { direct_usb: 1.5 * MB, card_reader: 20 * MB, unknown: 5 * MB };
  const STAGES: Record<string, string> = { scanning: 'Reading files', hashing: 'Checking files', copying: 'Copying', building_index: 'Updating the library list', verifying: 'Verifying', deleting: 'Removing', editing: 'Updating tags' };

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
  // Where the card's space goes (Tau media per core versus other data), measured in the background so the
  // bar shows total and free at once and fills in the split when the walk finishes.
  let breakdown: CardBreakdown | null = null;
  let breakdownBusy = false;
  let breakdownJob = '';
  let detailsOpen = false;
  let prefs: PrefsView | null = null;
  let connection: ConnectionInfo = { kind: 'unknown', detail: '' };
  // Covers are always put inside the copied songs and always get the small fast-loading cover file: they
  // are what the Pocket shows, so there is nothing to ask. An album whose cover cannot be used is copied
  // without it and the finished sync says so.
  const EMBED_COVERS = true;
  const ART_SIDECAR = true;

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

  let dialog: null | 'review' | 'first-remove' | 'edit' = null;
  let capBar: CapacityBar;
  let lastFailed = false;
  let review: ChangePlanView | null = null;
  let reviewError = '';
  let slowDontAsk = false;
  let askBackup = true;

  type Run = { journal: string; phase: string; done: number; total: number; path: string; startedAt: number; speed: number; samples: { t: number; done: number }[]; finished: boolean; error: string; report: ChangeResult | null; cancelled: boolean; reassurance: string; warnings: { code: string; message: string }[] };
  let run: Run | null = null;
  // Per-album progress while a sync runs, keyed by the staged item's source album id. The engine reports the file
  // it is about to copy and the bytes done before it; the album a file belongs to is its folder.
  type AlbumRing = { state: 'queued' | 'syncing' | 'done'; pct: number };
  let albumSync: Record<string, AlbumRing> = {};
  let ringFinishedBytes = 0;
  let ringCurrent = '';
  // A finished sync is a toast, not a dialog; `run` is released as soon as it finishes so the UI is never blocked.
  let syncResult: { report: ChangeResult; warnings: { code: string; message: string }[]; journal: string } | null = null;
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
    lastFailed = mediaRoot ? load<boolean>('lastfail', false) : false;
    picked = new Set(); pocketPicked = new Set(); source = null; card = null; space = null; resetThumbs('s'); resetThumbs('c'); stateFilter = 'all';
    if (sourcePath) scanSource();
    await reloadCard();
    if (cardPath) detectConnection(cardPath).then((c) => (connection = c)).catch(() => (connection = { kind: 'unknown', detail: '' }));
  }
  async function reloadCard() {
    if (!mediaRoot) return;
    cardBusy = true; cardError = ''; cardJob = newJobId(); cardProgress = '';
    try {
      card = await listLibrary(mediaRoot, cardJob); resetThumbs('c');
      space = await checkStorageCapacity(mediaRoot, 0).catch(() => null);
      loadBreakdown();
      // Drop staged removals/edits for albums that are no longer on the card.
      const here = new Set(card.albums.map((a) => a.id));
      const kept = pending.filter((p) => p.kind === 'add' || here.has(p.id));
      if (kept.length !== pending.length) pending = kept;
    } catch (error) { card = null; cardError = explainError(error); }
    finally { cardBusy = false; }
  }
  /** Starts (or restarts) the background measurement; a stale answer for a card that has since changed is dropped. */
  function loadBreakdown() {
    if (breakdownJob) cancelJob(breakdownJob).catch(() => {});
    if (!cardPath || !cores.length) { breakdown = null; breakdownBusy = false; return; }
    const job = (breakdownJob = newJobId());
    breakdownBusy = true;
    cardBreakdown(cardPath, cores, job)
      .then((result) => { if (job === breakdownJob) breakdown = result; })
      .catch(() => { if (job === breakdownJob) breakdown = null; })
      .finally(() => { if (job === breakdownJob) { breakdownBusy = false; breakdownJob = ''; } });
  }
  async function scanSource() {
    if (!sourcePath) return;
    sourceBusy = true; sourceError = ''; sourceJob = newJobId(); sourceProgress = '';
    try { source = await listLibrary(sourcePath, sourceJob); }
    catch (error) { source = null; sourceError = explainError(error); }
    finally { sourceBusy = false; }
  }
  async function chooseSource() {
    const selected = await open({ directory: true });
    if (!selected || Array.isArray(selected)) return;
    sourcePath = selected; save('source', sourcePath); picked = new Set(); resetThumbs('s');
    await scanSource();
  }
  async function doRefresh() { spinning = true; try { await refresh(); } finally { setTimeout(() => (spinning = false), 700); } }
  $: save_pending(pending);
  function save_pending(p: Pending[]) {
    if (loadedFor !== mediaRoot || !mediaRoot) return;
    save('pending', p);
    // lets the sidebar show a "N pending" badge for this card
    window.dispatchEvent(new CustomEvent('tau-pending'));
  }

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
  $: queuedIds = new Set(adds.map((p) => p.id));
  $: removingIds = new Set(removes.map((p) => p.id));
  $: editingIds = new Set(edits.map((p) => p.id));
  $: cardByDest = new Map((card?.albums ?? []).map((a) => [a.id, a]));
  $: connKind = (prefs?.connections?.[cardLabel] && prefs.connections[cardLabel] !== 'auto' ? prefs.connections[cardLabel] : connection.kind) as ConnectionInfo['kind'];
  $: lastSpeed = prefs?.speeds?.[cardLabel] ?? 0;
  $: slowApplies = connKind === 'direct_usb' && addBytes > SLOW_LIMIT;
  $: estSpeed = lastSpeed || DEFAULT_SPEED[connKind] || DEFAULT_SPEED.unknown;
  $: estIsDefault = !lastSpeed;
  $: slowWarn = !!review && connKind === 'direct_usb' && review.bytes_to_write > SLOW_LIMIT && !prefs?.slow_alert_suppressed;
  $: pendingCount = pending.length;
  $: disconnected = connected === false;
  $: freeNow = space?.space.available_bytes ?? 0;
  /** The platform folder this Library works with (`.../Assets/<platform>/common`): the capacity bar's active segment. */
  $: activePlatform = (mediaRoot.match(/\/Assets\/([^/]+)\/common\/?$/) ?? [])[1] ?? '';
  $: activeSegment = breakdown?.segments.find((s) => s.platform === activePlatform) ?? null;
  // The Pocket's index has hard limits; show how full it is and stop before an over-limit sync.
  $: limits = card?.limits ?? null;
  $: tracksNow = card?.tracks.length ?? 0;
  $: albumsNow = card?.albums.length ?? 0;
  $: tracksAfter = tracksNow + adds.reduce((n, p) => n + p.tracks - (cardByDest.get(p.destId)?.tracks ?? 0), 0) - removes.reduce((n, p) => n + p.tracks, 0);
  $: albumsAfter = albumsNow + adds.filter((p) => !cardByDest.has(p.destId)).length - removes.length;
  $: overLimit = !!limits && (tracksAfter > limits.max_tracks || albumsAfter > limits.max_albums);
  $: nearLimit = !!limits && !overLimit && tracksAfter > limits.max_tracks * 0.9;
  // Fit check, both for what is ticked (before adding) and for what is queued (after).
  $: pickedAlbums = (source?.albums ?? []).filter((a) => picked.has(a.id));
  $: pickedBytes = pickedAlbums.reduce((n, a) => n + a.bytes, 0);
  $: pickedNetTracks = pickedAlbums.reduce((n, a) => n + a.tracks - (cardByDest.get(a.dest_id)?.tracks ?? 0), 0);
  $: margin = space?.margin_bytes ?? 0;
  $: afterPick = space ? free - pickedBytes : null;
  $: pickFits = afterPick !== null && afterPick >= margin && (!limits || tracksAfter + pickedNetTracks <= limits.max_tracks);
  $: pickOver = afterPick !== null ? Math.max(0, margin - afterPick) : 0;
  $: pickTooMany = !!limits && tracksAfter + pickedNetTracks > limits.max_tracks;
  $: canStart = pendingCount > 0 && !overCapacity && !overLimit && !disconnected && !run && !!card;
  $: startReason = !pendingCount ? 'Nothing is staged yet.' : overCapacity ? "This won't fit on the card." : overLimit ? "This is over the Pocket's library limit." : disconnected ? 'The card is not connected.' : run ? 'A sync is already running.' : !card ? 'The card is still being read.' : '';
  $: pendingLine = [adds.length && `${plural(adds.length, 'album')} to add (${plural(addTracks, 'track')}, ${size(addBytes)})`, removes.length && `${removes.length} to remove`, edits.length && `${edits.length} to edit`].filter(Boolean).join(' · ');
  $: stagedItems = pending.map((p): StagedItem => {
    const label = p.kind === 'edit' ? editLabel(p) : p.title;
    return {
      key: p.key, kind: p.kind, label,
      artist: p.kind === 'edit' ? undefined : p.artist, tracks: p.kind === 'edit' ? undefined : p.tracks, bytes: p.kind === 'edit' ? undefined : p.bytes,
      action: p.kind === 'remove' ? `Undo removing ${p.title}` : `Remove ${label} from pending changes`,
      verb: p.kind === 'remove' ? 'Undo' : p.kind === 'edit' ? 'Discard' : 'Remove',
    };
  });
  $: notes = [
    slowApplies && prefs?.slow_alert_suppressed ? { text: `Direct connection: slow · about ${eta(addBytes, estSpeed)}${estIsDefault ? ' (estimate)' : ''}`, bad: false } : null,
    overCapacity ? { text: "Won't fit on this card", bad: true } : null,
    overLimit && limits ? { text: `Over the Pocket's library limit (${limits.max_tracks.toLocaleString()} tracks)`, bad: true } : null,
    disconnected ? { text: ejectedSafely ? 'Card ejected' : 'Card disconnected', bad: true } : null,
  ].filter((n): n is { text: string; bad: boolean } => n !== null);
  $: pendingSummary = [adds.length && `${plural(adds.length, 'album')} to add`, removes.length && `${removes.length} to remove`, edits.length && `${plural(edits.length, 'edit')}`].filter(Boolean).join(', ');
  $: detailsWarnings = [
    overCapacity ? "This won't fit on the card: remove something or add less." : '',
    overLimit && limits ? `Over the Pocket's library limit (${limits.max_tracks.toLocaleString()} tracks, ${limits.max_albums.toLocaleString()} albums).` : '',
    nearLimit && limits ? `Close to the Pocket's library limit (${tracksAfter.toLocaleString()} of ${limits.max_tracks.toLocaleString()} tracks).` : '',
    slowApplies ? "Connected directly to the Pocket: transfers over 10 MB are slow, and the Pocket's own screen says not to use this for large transfers. A card reader is much faster." : '',
    !overCapacity && total && free < total * 0.05 ? 'The card will be nearly full.' : '',
  ].filter(Boolean);
  $: detailsBackup = removes.length ? (prefs?.remove_mode === 'ask' ? 'You will be asked whether to back these up first.' : prefs?.remove_mode === 'none' ? 'These will be removed without a backup (your setting).' : 'These are copied to your backup folder first, then removed.') : '';
  // ---- what each list shows: search, state filter and sort ----------------------------
  type SortKey = 'artist' | 'title' | 'largest' | 'smallest';
  let sort: SortKey = (() => { try { return (localStorage.getItem('tau.wb.sort') as SortKey) || 'artist'; } catch { return 'artist'; } })();
  $: { try { localStorage.setItem('tau.wb.sort', sort); } catch { /* a per-viewer convenience only */ } }
  let stateFilter: 'all' | AlbumState = 'all';
  const byArtist = (x: AlbumInfo, y: AlbumInfo) => (x.artist || '~').localeCompare(y.artist || '~') || x.title.localeCompare(y.title);
  const sorter = (k: SortKey) => (x: AlbumInfo, y: AlbumInfo) => k === 'largest' ? y.bytes - x.bytes : k === 'smallest' ? x.bytes - y.bytes : k === 'title' ? x.title.localeCompare(y.title) : byArtist(x, y);
  $: srcStates = (source?.albums ?? []).map((a) => [a, stateOf(a, queuedIds, cardByDest)] as const);
  $: stateCounts = { all: srcStates.length, new: srcStates.filter(([, st]) => st === 'new').length, changed: srcStates.filter(([, st]) => st === 'changed').length, on: srcStates.filter(([, st]) => st === 'on').length };
  $: sourceAlbums = srcStates.filter(([a, st]) => match(`${a.title} ${a.artist}`, sourceSearch) && (stateFilter === 'all' || st === stateFilter)).map(([a]) => a).sort(sorter(sort));
  $: pocketAlbums = (card?.albums ?? []).filter((a) => match(`${a.title} ${a.artist}`, pocketSearch)).sort(sorter(sort));
  type PocketItem = { key: string; kind: 'card'; a: AlbumInfo } | { key: string; kind: 'add'; p: Extract<Pending, { kind: 'add' }> };
  $: pocketItems = [...pocketAlbums.map((a): PocketItem => ({ key: a.id, kind: 'card', a })), ...adds.map((p): PocketItem => ({ key: p.key, kind: 'add', p }))];
  $: artists = [...pocketAlbums.reduce((m, a) => m.set(a.artist || 'Unknown artist', [...(m.get(a.artist || 'Unknown artist') ?? []), a]), new Map<string, AlbumInfo[]>())];
  $: allTracks = (card?.tracks ?? []).filter((t) => match(`${t.title} ${t.artist} ${t.album}`, pocketSearch));
  $: busy = !!run && !run.finished;

  const size = (n: number) => (n >= GB ? `${(n / GB).toFixed(1)} GB` : n >= MB ? `${Math.round(n / MB)} MB` : `${Math.max(1, Math.round(n / 1000))} KB`);
  const match = (text: string, q: string) => !q.trim() || text.toLowerCase().includes(q.trim().toLowerCase());
  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
  const duration = (secs: number) => `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, '0')}`;
  function eta(bytes: number, speed: number): string { if (!speed) return ''; const s = Math.ceil(bytes / speed); return s < 45 ? 'under a minute' : s < 5400 ? `${Math.max(1, Math.round(s / 60))} min` : `${(s / 3600).toFixed(1)} h`; }
  $: lastEta = lastSpeed && addBytes ? eta(addBytes, lastSpeed) : '';
  $: reviewEta = review && review.bytes_to_write ? eta(review.bytes_to_write, estSpeed) : '';

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

  function say(message: string) { toast = message; clearTimeout(toastTimer); toastTimer = setTimeout(() => (toast = ''), 7000); }
  /** After a change to the queue, keyboard focus goes to the bar (Start sync, or Details when it is not allowed yet). */
  async function focusTray() { await tick(); capBar?.focusPrimary(); }
  function toggle(set: Set<string>, id: string) { const next = new Set(set); next.has(id) ? next.delete(id) : next.add(id); return next; }

  // ---- selection: whole-row click, Shift-click ranges, select all ------------------------
  const selectableSrc = (a: AlbumInfo) => { const st = stateOf(a, queuedIds, cardByDest); return st !== 'on' && st !== 'queued'; };
  const selectablePk = (a: AlbumInfo) => !removingIds.has(a.id);
  let anchorSrc: string | null = null, anchorPk: string | null = null;
  /** Ticks or unticks `a`; with Shift held, selects the whole range from the last one clicked. */
  function pickFrom(set: Set<string>, a: { id: string }, e: MouseEvent | null, rows: { id: string }[], ok: (r: any) => boolean, anchor: string | null): { set: Set<string>; anchor: string } {
    if (e?.shiftKey && anchor) {
      const ids = rows.map((r) => r.id), i = ids.indexOf(anchor), j = ids.indexOf(a.id);
      if (i >= 0 && j >= 0) { const next = new Set(set); for (const r of rows.slice(Math.min(i, j), Math.max(i, j) + 1)) if (ok(r)) next.add(r.id); return { set: next, anchor }; }
    }
    return { set: toggle(set, a.id), anchor: a.id };
  }
  function pickSrc(a: AlbumInfo, e: MouseEvent | null) { if (busy || !selectableSrc(a)) return; const r = pickFrom(picked, a, e, sourceAlbums, selectableSrc, anchorSrc); picked = r.set; anchorSrc = r.anchor; }
  function pickPk(a: AlbumInfo, e: MouseEvent | null) { if (busy || !selectablePk(a)) return; const r = pickFrom(pocketPicked, a, e, pocketAlbums, selectablePk, anchorPk); pocketPicked = r.set; anchorPk = r.anchor; }
  function pickTrack(t: TrackInfo, e: MouseEvent | null) { const r = pickFrom(pocketPicked, { id: t.rel }, e, allTracks.map((x) => ({ id: x.rel })), () => true, anchorPk); pocketPicked = r.set; anchorPk = r.anchor; }
  /** A click anywhere on a row (not on its own controls) selects it. */
  const onRow = (e: MouseEvent, fn: () => void) => { if (!(e.target as HTMLElement).closest('input,button,a,label,select')) fn(); };
  $: shownSrc = sourceAlbums.filter(selectableSrc);
  $: allSrcSelected = shownSrc.length > 0 && shownSrc.every((a) => picked.has(a.id));
  $: someSrcSelected = shownSrc.some((a) => picked.has(a.id));
  function toggleAllSrc() { if (busy) return; const next = new Set(picked); if (allSrcSelected) shownSrc.forEach((a) => next.delete(a.id)); else shownSrc.forEach((a) => next.add(a.id)); picked = next; }
  $: shownPk = view === 'albums' ? pocketAlbums.filter(selectablePk).map((a) => a.id) : view === 'tracks' ? allTracks.map((t) => t.rel) : [];
  $: allPkSelected = shownPk.length > 0 && shownPk.every((id) => pocketPicked.has(id));
  $: somePkSelected = shownPk.some((id) => pocketPicked.has(id));
  function toggleAllPk() { if (busy) return; const next = new Set(pocketPicked); if (allPkSelected) shownPk.forEach((id) => next.delete(id)); else shownPk.forEach((id) => next.add(id)); pocketPicked = next; }
  let selAllSrcEl: HTMLInputElement, selAllPkEl: HTMLInputElement;
  $: if (selAllSrcEl) selAllSrcEl.indeterminate = someSrcSelected && !allSrcSelected;
  $: if (selAllPkEl) selAllPkEl.indeterminate = somePkSelected && !allPkSelected;

  // ---- cover thumbnails: loaded lazily for the rows on screen ------------------------------
  let thumbs: Record<string, string> = {};
  const asked = new Set<string>();
  function resetThumbs(side: 's' | 'c') { thumbs = Object.fromEntries(Object.entries(thumbs).filter(([k]) => !k.startsWith(side + ':'))); for (const k of [...asked]) if (k.startsWith(side + ':')) asked.delete(k); }
  function wantThumbs(side: 's' | 'c', ids: string[]) {
    const root = side === 's' ? sourcePath : mediaRoot;
    const need = ids.filter((id) => !asked.has(`${side}:${id}`));
    if (!root || !need.length) return;
    need.forEach((id) => asked.add(`${side}:${id}`));
    albumThumbnails(root, need).then((list) => {
      const next = { ...thumbs };
      for (const t of list) next[`${side}:${t.id}`] = t.png_base64 ? `data:${t.mime ?? 'image/png'};base64,${t.png_base64}` : '';
      thumbs = next;
    }).catch(() => need.forEach((id) => asked.delete(`${side}:${id}`)));
  }
  const onSrcRange = (e: CustomEvent<{ start: number; end: number }>) => wantThumbs('s', sourceAlbums.slice(e.detail.start, e.detail.end).map((a) => a.id));
  const onPkRange = (e: CustomEvent<{ start: number; end: number }>) => {
    const rows = pocketItems.slice(e.detail.start, e.detail.end);
    wantThumbs('c', rows.filter((r): r is Extract<PocketItem, { kind: 'card' }> => r.kind === 'card').map((r) => r.a.id));
    wantThumbs('s', rows.filter((r): r is Extract<PocketItem, { kind: 'add' }> => r.kind === 'add').map((r) => r.p.id));
  };

  // ---- staging -------------------------------------------------------------
  function addAlbums(ids: string[]) {
    if (busy) return;
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
    focusTray();
  }
  const addPicked = () => addAlbums([...picked]);
  function onDragStart(e: DragEvent, a: AlbumInfo) { const ids = picked.has(a.id) ? [...picked] : [a.id]; e.dataTransfer?.setData('text/tau-albums', JSON.stringify(ids)); if (e.dataTransfer) e.dataTransfer.effectAllowed = 'copy'; }
  function onDrop(e: DragEvent) { e.preventDefault(); dropActive = false; const raw = e.dataTransfer?.getData('text/tau-albums'); if (raw) { try { addAlbums(JSON.parse(raw)); } catch { /* not ours */ } } }

  function requestRemove() { if (busy || !pocketPicked.size) return; if (prefs && !prefs.remove_explained) dialog = 'first-remove'; else stageRemoval(); }
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
    if (ok.length) { say(`${plural(ok.length, 'album')} marked for removal. Nothing is deleted until you sync.`); focusTray(); }
  }
  function unstage(key: string) { if (busy) return; pending = pending.filter((p) => p.key !== key); focusTray(); }
  function clearAll() { if (busy) return; const n = pending.length; pending = []; picked = new Set(); pocketPicked = new Set(); say(`Cleared ${plural(n, 'pending change')}.`); focusTray(); }

  // ---- editing ---------------------------------------------------------------
  let editAlbum: AlbumInfo | null = null;
  let editTrack: TrackInfo | null = null;
  let editForm = { title: '', artist: '', albumArtist: '', year: '' };
  let editCover: string | null = null, editCoverPreview: string | null = null, editCoverError = '', editCurrent = '';
  function openEdit() {
    if (busy) return;
    editCover = null; editCoverPreview = null; editCoverError = ''; editCurrent = '';
    if (view === 'tracks') {
      const t = (card?.tracks ?? []).find((x) => pocketPicked.has(x.rel)); if (!t) return;
      editTrack = t; editAlbum = null; editForm = { title: t.title, artist: '', albumArtist: '', year: '' };
    } else {
      const a = (card?.albums ?? []).find((x) => pocketPicked.has(x.id)); if (!a) return;
      if (queuedIds.size && [...queuedIds].some((id) => source?.albums.find((s) => s.id === id)?.dest_id === a.id)) { say('This album is queued to be added. Sync first, then edit it.'); return; }
      editAlbum = a; editTrack = null; editForm = { title: a.title, artist: a.artist, albumArtist: '', year: a.year ?? '' }; editCurrent = thumbs[`c:${a.id}`] ?? '';
    }
    dialog = 'edit';
  }
  async function chooseCover() {
    editCoverError = '';
    const selected = await open({ multiple: false, filters: [{ name: 'JPEG image', extensions: ['jpg', 'jpeg'] }] });
    if (!selected || Array.isArray(selected)) return;
    editCover = selected;
    try { editCoverPreview = await imageThumbnail(selected, 160); } catch (error) { editCoverPreview = null; editCover = null; editCoverError = `That picture can\u2019t be used. ${explainError(error)}`; }
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
    pocketPicked = new Set(); say('Edit saved. It will be applied to the Pocket when you sync.'); focusTray();
  }

  /** What an album will look like once its pending edit is applied (shown straight away, before the sync). */
  function effective(a: AlbumInfo, list: typeof edits) {
    const e = list.find((p) => p.id === a.id && p.track === null);
    return { title: e?.fields.album ?? a.title, artist: e?.fields.artist ?? a.artist, year: e?.fields.year ?? a.year, edited: !!e };
  }
  /** Chip and review text for an edit: "Blue Train → Blue Train (Remaster) · cover". */
  function editLabel(p: Extract<Pending, { kind: 'edit' }>): string {
    const rename = p.fields.album ?? p.fields.title;
    const others = p.note.split(', ').filter((n) => n !== 'title');
    return `${rename ? `${p.title} → ${rename}` : p.title}${others.length ? ` · ${others.join(', ')}` : ''}`;
  }
  const historyContext = (): HistoryContext => ({
    card: cardLabel, core: coreLabel, connection: connKind,
    items: pending.map((p) => p.kind === 'edit'
      ? { kind: 'edit' as const, title: p.title, note: p.note, label: editLabel(p) }
      : { kind: p.kind, title: p.title, artist: p.artist, tracks: p.tracks, bytes: p.bytes }),
  });

  // ---- sync ------------------------------------------------------------------
  function buildRequest(): ChangeRequest {
    const editRequests: EditRequest[] = edits.map((p) => ({ album_id: p.id, track: p.track, fields: p.fields, cover: p.cover }));
    return { library_root: sourcePath || null, add_albums: adds.map((p) => p.id), remove_albums: removes.map((p) => p.id), edits: editRequests, options: { mirror: false, embed_covers: EMBED_COVERS, art_sidecar_pal256: ART_SIDECAR } };
  }
  async function start() {
    if (!canStart) return;
    reviewError = ''; review = null;
    try { review = await planChanges(buildRequest(), mediaRoot); }
    catch (error) { reviewError = explainError(error); dialog = 'review'; return; }
    slowDontAsk = false;
    // Nothing to decide (no removals, no slow direct-USB warning to acknowledge): just do it. The staged
    // list and the card space bar were the review; the engine still checks every file as it writes.
    const slowNow = connKind === 'direct_usb' && review.bytes_to_write > SLOW_LIMIT && !prefs?.slow_alert_suppressed;
    if (review.removed_files > 0 || slowNow) { dialog = 'review'; return; }
    await confirmSync();
  }
  async function confirmSync() {
    if (!review) return;
    if (slowWarn && slowDontAsk && prefs) { try { prefs = await setPrefs({ ...prefs, slow_alert_suppressed: true }); } catch { /* the warning simply shows again */ } }
    dialog = null;
    const mode = prefs?.remove_mode ?? 'backup';
    const backupOn = mode === 'backup' || (mode === 'ask' && askBackup);
    const backup = review.removed_files && backupOn ? (prefs?.backup_dir || prefs?.default_backup_dir || null) : null;
    const planWarnings = review.warnings;
    albumSync = Object.fromEntries(adds.map((p) => [p.id, { state: 'queued', pct: 0 } as AlbumRing]));
    ringFinishedBytes = 0; ringCurrent = '';
    runId = newJobId();
    run = { journal: '', reassurance: '', warnings: planWarnings, phase: 'Starting', done: 0, total: review.bytes_to_write, path: '', startedAt: Date.now(), speed: 0, samples: [], finished: false, error: '', report: null, cancelled: false };
    const startedAt = Date.now();
    try {
      const report = await executeChanges(buildRequest(), mediaRoot, review.id, backup, historyContext(), runId);
      const seconds = (Date.now() - startedAt) / 1000;
      if (prefs && report.bytes_written >= 5 * MB && seconds >= 2) {
        try { prefs = await setPrefs({ ...prefs, speeds: { ...prefs.speeds, [cardLabel]: report.bytes_written / seconds } }); } catch { /* speed memory is best-effort */ }
      }
      syncResult = { report, warnings: planWarnings, journal: report.journal };
      ejectMsg = ''; ejectOk = false;
      run = null;
      lastFailed = false; save('lastfail', false);
      pending = []; picked = new Set(); pocketPicked = new Set();
      await reloadCard();
    } catch (error) {
      const message = explainError(error);
      const cancelled = (error as { code?: number })?.code === 44;
      run = { ...run!, finished: true, cancelled, error: message };
      if (!cancelled) { lastFailed = true; save('lastfail', true); }
      // Ask the journal how far the run got, so the reassurance is accurate rather than generic.
      try { const latest = (await listHistory())[0]; run = { ...run!, journal: latest?.path ?? '', reassurance: reassure(latest?.phase ?? 'copy', cancelled) }; }
      catch { run = { ...run!, reassurance: reassure('copy', cancelled) }; }
      await reloadCard();
    }
  }
  /** What is and isn't safe after a run stopped, by how far it got (see `ChangeReport.phase`). */
  function reassure(phase: string, cancelled: boolean): string {
    if (phase === 'remove') return 'Your additions and edits were applied. Some removals may not have finished; anything removed is in your backup folder. Open the details to see exactly what was done.';
    if (phase === 'edit') return 'Your albums were copied, but the tag edits did not finish. Nothing was removed.';
    return `${cancelled ? '' : 'Your existing music on the Pocket is safe: '}the Pocket's library list is only updated after every file is copied and checked, so it still plays what it played before. Files copied so far were kept.`;
  }
  function seeDetails(id: string) { closeRun(); openHistory(id || 'latest'); }
  /** Moves each album's ring: the album being copied fills, the ones before it are done, the ones after wait. */
  function updateRings(doneBytes: number, path: string) {
    const parent = path.replace(/\\/g, '/').replace(/\/[^/]*$/, '');
    // Longest matching folder wins, so an album folder nested inside another is not mistaken for its parent.
    const album = adds.filter((p) => parent === p.destId || parent.endsWith(`/${p.destId}`)).sort((a, b) => b.destId.length - a.destId.length)[0];
    if (!album) return;
    const next = { ...albumSync };
    if (ringCurrent && ringCurrent !== album.id && next[ringCurrent]) {
      next[ringCurrent] = { state: 'done', pct: 100 };
      ringFinishedBytes += adds.find((p) => p.id === ringCurrent)?.bytes ?? 0;
    }
    ringCurrent = album.id;
    const share = album.bytes > 0 ? (doneBytes - ringFinishedBytes) / album.bytes : 0;
    next[album.id] = { state: 'syncing', pct: Math.max(2, Math.min(98, share * 100)) };
    albumSync = next;
  }
  function handleRunProgress(stage: string, done: number, totalUnits: number, path: string) {
    if (!run) return;
    const now = Date.now();
    let { samples, speed } = run;
    if (stage === 'copying') {
      samples = [...samples.filter((s) => now - s.t < 4000), { t: now, done }];
      if (samples.length > 1) { const a = samples[0], b = samples[samples.length - 1]; if (b.t > a.t) speed = ((b.done - a.done) / (b.t - a.t)) * 1000; }
    }
    if (stage === 'copying' && path) updateRings(done, path);
    run = { ...run, phase: STAGES[stage] ?? stage, done: stage === 'copying' ? done : run.done, total: stage === 'copying' ? totalUnits : run.total, path, samples, speed };
  }
  async function cancelSync() { if (runId) await cancelJob(runId); }
  function closeRun() { run = null; syncResult = null; ejectMsg = ''; ejectOk = false; }
  // Safe eject: unmount so the OS writes out anything it still holds. "You can unplug it" is only
  // ever said after the OS confirms the volume is unmounted.
  let ejecting = false; let ejectMsg = ''; let ejectOk = false;
  let verifiedOnDevice = true;
  readbackStatus().then((r) => (verifiedOnDevice = r.checks_the_device && r.failed_evictions === 0)).catch(() => {});
  async function eject() {
    if (!cardPath || busy || ejecting) return;
    ejecting = true; ejectMsg = ''; ejectOk = false;
    try {
      const result = await ejectCard(cardPath);
      ejectOk = result.ok; ejectMsg = result.message;
      if (result.ok) window.dispatchEvent(new CustomEvent('tau-ejected', { detail: cardPath }));
    } catch (error) { ejectMsg = explainError(error); }
    finally { ejecting = false; }
  }
  function dismissFailure() { lastFailed = false; save('lastfail', false); }
  $: syncView = run && !run.finished ? {
    title: `Syncing to ${cardLabel}`, phase: run.phase, done: run.done, total: run.total,
    line: [run.total ? `${size(run.done)} of ${size(run.total)}` : '', run.speed > 0 ? `${(run.speed / MB).toFixed(1)} MB/s · ${eta(run.total - run.done, run.speed) === 'under a minute' ? 'under a minute' : `about ${eta(run.total - run.done, run.speed)}`} left` : ''].filter(Boolean).join(' · '),
    note: 'Keep the Pocket connected until this finishes. You can keep looking around while it runs.',
    slow: runSlowNote ? "This is slower than a card reader usually is. If you're plugged into the Pocket directly, that's expected." : '',
  } : null;
  $: runSlowNote = run && !run.finished && connKind !== 'direct_usb' && run.speed > 0 && run.speed < 3 * MB && run.total > SLOW_LIMIT;

  function key(e: KeyboardEvent) {
    const t0 = e.target as HTMLElement | null;
    const typingNow = !!t0 && (t0.tagName === 'TEXTAREA' || t0.tagName === 'SELECT' || (t0.tagName === 'INPUT' && (t0 as HTMLInputElement).type !== 'checkbox'));
    if (e.key === 'Escape') { if (dialog && !run) dialog = null; else if (!dialog && !typingNow && (picked.size || pocketPicked.size)) { picked = new Set(); pocketPicked = new Set(); } connMenu = false; return; }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a' && !dialog && !run && !typingNow) { e.preventDefault(); if (t0?.closest?.('[data-pane=pocket]')) toggleAllPk(); else toggleAllSrc(); return; }
    if ((e.key === 'Delete' || e.key === 'Backspace') && !dialog && !run && view === 'albums' && pocketPicked.size) {
      const t = e.target as HTMLElement | null;
      const typing = !!t && (t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || (t.tagName === 'INPUT' && (t as HTMLInputElement).type !== 'checkbox'));
      if (!typing) { e.preventDefault(); requestRemove(); }
    }
  }
  async function setOverride(value: string) {
    connMenu = false;
    if (!prefs) return;
    try { prefs = await setPrefs({ ...prefs, connections: { ...prefs.connections, [cardLabel]: value } }); } catch { /* keeps the detected value */ }
  }
  $: connLabel = connKind === 'direct_usb' ? 'Connected directly · slow' : connKind === 'card_reader' ? 'Card reader · fast' : 'Connection unknown';
  $: noCard = !mediaRoot;
</script>
<svelte:window on:keydown={key} />

<section class="wb" aria-labelledby="wb-title">
  {#if !noCard}<a class="wb-skip" href="#cap-start-desc" on:click|preventDefault={focusTray}>Skip to Start sync</a>{/if}
  <header class="wb-top">
    <div class="wb-ctx">
      <p class="eyebrow">LIBRARY</p>
      <h1 id="wb-title">{cardLabel}</h1>
      <div class="wb-sub"><span class="wb-dot" aria-hidden="true"></span>{coreLabel ? `Player: ${coreLabel}` : 'No player core'}
        {#if cardPath}
          <span class="wb-connwrap">
            <button class="wb-conn" class:slow={connKind === 'direct_usb'} aria-haspopup="menu" aria-expanded={connMenu} title={connection.detail ? `Detected: ${connection.detail}` : 'How this card is connected'} on:click={() => (connMenu = !connMenu)}>{connLabel}</button>
            {#if connMenu}
              <div class="wb-menu" role="menu">
                <button role="menuitem" on:click={() => setOverride('auto')}>Detect automatically{connection.detail ? ` (${connection.detail})` : ''}</button>
                <button role="menuitem" on:click={() => setOverride('direct_usb')}>The Pocket itself (plugged in with its USB cable)</button>
                <button role="menuitem" on:click={() => setOverride('card_reader')}>A card reader</button>
              </div>
            {/if}
          </span>
          {#if connected !== false}<button class="wb-eject quiet" disabled={busy || ejecting} title="Unmount the card so it is safe to unplug" on:click={eject}>{ejecting ? 'Ejecting…' : 'Eject safely'}</button>{/if}
        {/if}
      </div>
    </div>
    <button class="wb-refresh" class:spin={spinning} aria-label="Refresh this card" title={refreshedAt ? `Refresh · updated ${refreshedAt}` : 'Refresh'} on:click={doRefresh}>
      <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 12a8 8 0 1 1-2.6-5.9"/><path d="M20 4v5h-5"/></svg>
    </button>
  </header>

  {#if disconnected}
    {#if ejectedSafely}
      <div class="wb-banner" role="status"><span>Safely ejected. You can unplug it now. Your pending changes are kept; plug it back in to continue.</span><button class="quiet" on:click={doRefresh}>Check again</button></div>
    {:else}
      <div class="wb-banner" role="alert"><span>This card is disconnected. Your pending changes are kept; reconnect it to continue.</span><button class="quiet" on:click={doRefresh}>Check again</button></div>
    {/if}
  {/if}
  {#if lastFailed && !run}
    <div class="wb-banner wb-banner-fail" role="alert"><span>Your last sync didn't finish. Nothing was lost; open the details to see what happened.</span><span class="wb-banner-actions"><button class="quiet" on:click={() => openHistory('latest')}>See what happened</button><button class="quiet" on:click={dismissFailure}>Dismiss</button></span></div>
  {/if}
  {#if noCard}
    <section class="empty"><div class="empty-art">◒</div><h2>No card selected</h2><p>Connect your Pocket or a card reader, or choose a card on the Cards screen. It will appear here automatically.</p></section>
  {:else}
  <CapacityBar bind:this={capBar} hasSpace={!!space} busy={cardBusy} {total} {used} {addBytes} {removeBytes} {freeNow} freeAfter={free} {margin} {overCapacity}
    {breakdown} measuring={breakdownBusy} {activePlatform}
    limitText={limits ? `${tracksAfter.toLocaleString()} of ${limits.max_tracks.toLocaleString()} tracks (Pocket limit)` : ''}
    limitTitle={limits ? `The Pocket's library can hold at most ${limits.max_tracks.toLocaleString()} tracks and ${limits.max_albums.toLocaleString()} albums` : ''}
    {overLimit} {nearLimit} sync={syncView} onCancel={cancelSync} onDetails={() => (detailsOpen = true)} {canStart} {startReason} pendingSummary={pendingSummary} onStart={start}
    hasPending={pendingCount > 0} clearDisabled={busy} onClear={clearAll} {notes} />

  <div class="wb-panes">
    <section class="wb-pane" data-pane="source" aria-labelledby="src-title">
      <div class="wb-pane-head"><h2 id="src-title">This computer</h2>{#if sourcePath}<button class="wb-link" title={sourcePath} on:click={chooseSource}>{sourcePath.split(/[\\/]/).filter(Boolean).pop()} · change</button>{/if}</div>
      {#if !sourcePath}
        <div class="wb-empty wb-cta"><b>Choose your music folder</b><p>Pick a folder on this computer to see its albums. Your files are never changed.</p><button class="primary" on:click={chooseSource}>Choose folder…</button></div>
      {:else if sourceBusy}
        <div class="wb-empty" role="status"><b>Reading your library…</b><p>{sourceProgress}</p></div>
      {:else if sourceError}
        <div class="wb-empty" role="alert"><b>Couldn't read this folder</b><p>{sourceError}</p><button class="quiet" on:click={scanSource}>Try again</button></div>
      {:else if source}
        <div class="wb-tools">
          <input class="wb-search" bind:value={sourceSearch} placeholder="Search albums or artists" aria-label="Search this computer" />
          <select class="wb-sort" aria-label="Sort albums" bind:value={sort}>
            <option value="artist">Artist A–Z</option><option value="title">Title A–Z</option><option value="largest">Largest first</option><option value="smallest">Smallest first</option>
          </select>
        </div>
        <div class="wb-filters" role="group" aria-label="Show albums">
          {#each [['all', 'All'], ['new', 'New'], ['changed', 'Changed'], ['on', 'On Pocket']] as [k, label]}
            <button class:on={stateFilter === k} aria-pressed={stateFilter === k} on:click={() => (stateFilter = k as typeof stateFilter)}>{label} ({stateCounts[k as keyof typeof stateCounts]})</button>
          {/each}
        </div>
        <label class="wb-selectall"><input type="checkbox" bind:this={selAllSrcEl} checked={allSrcSelected} disabled={busy || !shownSrc.length} on:change={toggleAllSrc} /> Select all {shownSrc.length} shown</label>
        <div class="wb-listwrap" data-pane="source">
          <VirtualList items={sourceAlbums} rowHeight={58} key={(a) => a.id} label="Albums on this computer" resetKey={`${sourceSearch}|${stateFilter}|${sort}|${sourcePath}`} on:range={onSrcRange} let:item={a} let:index let:count>
            {@const st = stateOf(a, queuedIds, cardByDest)}
            {@const onPk = cardByDest.get(a.dest_id)}
            {@const art = thumbs[`s:${a.id}`]}
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions (row click is a mouse convenience; keyboard users select with the row's checkbox) -->
            <div class="wb-row" class:sel={picked.has(a.id)} class:dim={st === 'on'} role="listitem" aria-posinset={index + 1} aria-setsize={count} draggable={!busy && st !== 'on' && st !== 'queued'} on:dragstart={(e) => onDragStart(e, a)} on:click={(e) => onRow(e, () => pickSrc(a, e))}>
              <input type="checkbox" aria-label={`Select ${a.title}`} disabled={busy || st === 'on' || st === 'queued'} checked={picked.has(a.id)} on:click|stopPropagation={(e) => pickSrc(a, e)} />
              <div class="wb-art" aria-hidden="true">{#if art}<img src={art} alt="" />{:else}♪{/if}</div>
              <div class="wb-meta"><strong>{a.title}</strong><small>{a.artist || 'Unknown artist'} · {plural(a.tracks, 'track')} · {size(a.bytes)}{#if st === 'changed' && onPk}{' · '}Pocket has {plural(onPk.tracks, 'track')}{/if}</small></div>
              {#if busy && albumSync[a.id]}<RingProgress state={albumSync[a.id].state} pct={albumSync[a.id].pct} name={a.title} />{:else}<span class="wb-badge {st}" title={st === 'changed' ? `The Pocket's copy has a different number of tracks (${onPk?.tracks}) than this folder (${a.tracks}).` : undefined}>{stateLabel[st]}</span>{/if}
            </div>
          </VirtualList>
          {#if !sourceAlbums.length}<div class="wb-empty"><b>{source.albums.length ? 'No albums match' : 'No music found'}</b><p>{source.albums.length ? 'Try a different search or filter.' : 'This folder has no MP3 or FLAC files.'}</p></div>{/if}
        </div>
        <div class="wb-pane-foot">
          <span class="wb-fit-line" aria-live="polite">{#if picked.size}{picked.size} selected · {size(pickedBytes)}{#if afterPick !== null}<span class="wb-fit" class:bad={!pickFits}>{' '}{#if pickFits}· Fits, {size(afterPick)} free afterwards{:else if pickTooMany}· Too many tracks for the Pocket's library{:else}· Won't fit: {size(pickOver)} too big. You could remove something else in the same sync.{/if}</span>{/if}{:else}{plural(source.albums.length, 'album')} · click albums to select them, or drag them across →{/if}</span>
          <button class="primary" disabled={busy || !picked.size} on:click={addPicked}>Add {picked.size || ''} to Analogue Pocket →</button>
        </div>
      {/if}
    </section>

    <section class="wb-pane wb-drop" data-pane="pocket" class:active={dropActive} aria-labelledby="pk-title"
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
          {#if view === 'albums'}<select class="wb-sort" aria-label="Sort albums on the Pocket" bind:value={sort}><option value="artist">Artist A–Z</option><option value="title">Title A–Z</option><option value="largest">Largest first</option><option value="smallest">Smallest first</option></select>{/if}
        </div>
        {#if view === 'albums' || view === 'tracks'}
          <label class="wb-selectall"><input type="checkbox" bind:this={selAllPkEl} checked={allPkSelected} disabled={busy || !shownPk.length} on:change={toggleAllPk} /> Select all {shownPk.length} shown</label>
        {/if}
        {#if pocketPicked.size && (view === 'albums' || view === 'tracks')}
          <div class="wb-actions" role="toolbar" aria-label="Actions for the selection">
            <span>{pocketPicked.size} selected</span>
            {#if view === 'albums'}
              <button on:click={openEdit} disabled={busy || pocketPicked.size !== 1} title="Change the title, artist, year or cover">Edit…</button>
              <button class="danger" disabled={busy} on:click={requestRemove}>Remove</button>
            {:else}
              <button on:click={openEdit} disabled={busy || pocketPicked.size !== 1}>Rename…</button>
            {/if}
          </div>
        {/if}
        {#if view === 'albums'}
          <div class="wb-listwrap" data-pane="pocket">
            <VirtualList items={pocketItems} rowHeight={58} key={(x) => x.key} label="Albums on the Pocket" resetKey={`${pocketSearch}|${sort}`} on:range={onPkRange} let:item={x} let:index let:count>
              {#if x.kind === 'card'}
                {@const a = x.a}
                {@const eff = effective(a, edits)}
                {@const art = thumbs[`c:${a.id}`]}
                <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions (row click is a mouse convenience; keyboard users select with the row's checkbox) -->
                <div class="wb-row" class:sel={pocketPicked.has(a.id)} class:removing={removingIds.has(a.id)} role="listitem" aria-posinset={index + 1} aria-setsize={count} on:click={(e) => onRow(e, () => pickPk(a, e))}>
                  <input type="checkbox" aria-label={`Select ${a.title}`} disabled={busy || removingIds.has(a.id)} checked={pocketPicked.has(a.id)} on:click|stopPropagation={(e) => pickPk(a, e)} />
                  <div class="wb-art" aria-hidden="true">{#if art}<img src={art} alt="" />{:else}♪{/if}</div>
                  <div class="wb-meta"><strong>{eff.title}{#if eff.title !== a.title} <span class="wb-was">(was {a.title})</span>{/if}</strong><small>{eff.artist || 'Unknown artist'} · {plural(a.tracks, 'track')} · {size(a.bytes)}</small></div>
                  {#if removingIds.has(a.id)}<span class="wb-badge removing">Will be removed</span>{:else if editingIds.has(a.id)}<span class="wb-badge edit">Edit pending</span>{/if}
                </div>
              {:else}
                {@const art = thumbs[`s:${x.p.id}`]}
                <div class="wb-row incoming" role="listitem" aria-posinset={index + 1} aria-setsize={count}><span class="wb-plus" aria-hidden="true">+</span><div class="wb-art" aria-hidden="true">{#if art}<img src={art} alt="" />{:else}♪{/if}</div><div class="wb-meta"><strong>{x.p.title}</strong><small>{x.p.artist || 'Unknown artist'} · {plural(x.p.tracks, 'track')} · {size(x.p.bytes)}</small></div>{#if busy && albumSync[x.p.id]}<RingProgress state={albumSync[x.p.id].state} pct={albumSync[x.p.id].pct} name={x.p.title} />{:else}<span class="wb-badge queued">Will be added</span>{/if}</div>
              {/if}
            </VirtualList>
            {#if !pocketItems.length}<div class="wb-empty"><b>{card.albums.length ? 'No albums match' : 'Nothing here yet'}</b><p>{card.albums.length ? 'Try a different search.' : 'Add albums from This computer to get started.'}</p></div>{/if}
          </div>
        {:else if view === 'tracks'}
          <div class="wb-listwrap" data-pane="pocket">
            <VirtualList items={allTracks} rowHeight={58} key={(t) => t.rel} label="Tracks on the Pocket" resetKey={pocketSearch} let:item={t} let:index let:count>
              <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions (row click is a mouse convenience; keyboard users select with the row's checkbox) -->
              <div class="wb-row" class:sel={pocketPicked.has(t.rel)} role="listitem" aria-posinset={index + 1} aria-setsize={count} on:click={(e) => onRow(e, () => pickTrack(t, e))}>
                <input type="checkbox" aria-label={`Select ${t.title}`} disabled={busy} checked={pocketPicked.has(t.rel)} on:click|stopPropagation={(e) => pickTrack(t, e)} />
                <div class="wb-meta"><strong>{t.title}</strong><small>{t.artist || 'Unknown artist'} · {t.album || 'Unknown album'}</small></div>
                <span class="wb-time">{duration(t.secs)}</span><span class="wb-badge">{t.format}</span>
              </div>
            </VirtualList>
            {#if !allTracks.length}<div class="wb-empty"><b>No tracks match</b><p>Try a different search.</p></div>{/if}
          </div>
        {:else}
          <div class="wb-list" role="list">
            {#if view === 'artists'}
              {#each artists as [name, list] (name)}<div class="wb-row" role="listitem"><div class="wb-art" aria-hidden="true">♪</div><div class="wb-meta"><strong>{name}</strong><small>{plural(list.length, 'album')} · {plural(list.reduce((n, a) => n + a.tracks, 0), 'track')}</small></div></div>{/each}
            {:else}
              {#each card.playlists as pl}<div class="wb-row" role="listitem"><div class="wb-art" aria-hidden="true">≡</div><div class="wb-meta"><strong>{pl.name}</strong><small>{plural(pl.tracks, 'track')} · managed on the Playlists page</small></div></div>
              {:else}<div class="wb-empty"><b>No playlists</b><p>Create them on the Playlists page.</p></div>{/each}
            {/if}
          </div>
        {/if}
      {/if}
      {#if dropActive}<div class="wb-dropveil">Drop to add to Analogue Pocket</div>{/if}
    </section>
  </div>

  {/if}

  {#if toast}<div class="wb-toast" role="status">{toast}</div>{/if}
</section>

{#if dialog === 'review'}
  <div class="wb-veil" role="presentation" on:click={() => (dialog = null)}></div>
  <div class="wb-modal" role={slowWarn ? 'alertdialog' : 'dialog'} aria-labelledby="rev-t" aria-describedby={slowWarn ? 'slow-d' : undefined} use:modal>
    {#if reviewError}
      <h2 id="rev-t">This can't be synced yet</h2>
      <p role="alert">{reviewError}</p>
      <div class="wb-modal-actions"><button class="primary" data-autofocus on:click={() => (dialog = null)}>Back</button></div>
    {:else if review}
      <h2 id="rev-t">Ready to sync to {cardLabel}?</h2>
      <dl class="wb-review">
        {#if adds.length}<div><dt>Add</dt><dd>{plural(review.new_files + review.updated_files, 'track')} · {size(review.bytes_to_write)}{review.unchanged_files ? ` (${review.unchanged_files} already up to date)` : ''}</dd></div>{/if}
        {#if review.removed_files}<div><dt>Remove</dt><dd>{plural(review.removed_files, 'track')} · {size(review.bytes_to_remove)}{review.playlists_updated ? ` · ${plural(review.playlists_updated, 'playlist')} updated` : ''}</dd></div>{/if}
        {#if review.edited_files}<div><dt>Edit</dt><dd>{plural(review.edited_files, 'track')} (only the copies on the Pocket)</dd></div>{/if}
        {#if reviewEta}<div><dt>Time</dt><dd>about {reviewEta}{#if estIsDefault} <small class="wb-was">(rough estimate; we haven't timed this card yet)</small>{:else} <small class="wb-was">(from your last transfer)</small>{/if}</dd></div>{/if}
        <div><dt>Free space afterwards</dt><dd>{size(free)} of {size(total)}</dd></div>
      </dl>
      {#if slowWarn}
        <div class="wb-warn">
          <b>You're connected to the Pocket directly</b>
          <p id="slow-d">Transfers over this connection are slow and this may take a long time. The Pocket's USB mode is meant for transfers under 10 MB; a card reader is much faster for larger syncs. Are you sure?</p>
          <label class="wb-check"><input type="checkbox" bind:checked={slowDontAsk} /> Don't ask again (you'll still see a small note next to Start sync)</label>
        </div>
      {/if}
      <details class="wb-detail" open={pendingCount <= 5}>
        <summary>What's changing ({pendingCount})</summary>
        <ul>
          {#each pending as p (p.key)}
            <li><span class="k {p.kind}">{p.kind === 'add' ? '+' : p.kind === 'remove' ? '−' : '✎'}</span><span class="t"><b>{p.title}</b>{#if p.kind === 'edit'} <small>({p.note})</small>{:else} <small>{p.artist || 'Unknown artist'} · {plural(p.tracks, 'track')} · {size(p.bytes)}</small>{/if}</span></li>
          {/each}
        </ul>
      </details>
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
      <div class="wb-modal-actions"><button class="quiet" data-autofocus={slowWarn ? '' : undefined} on:click={() => (dialog = null)}>Back</button><button class="primary" data-autofocus={slowWarn ? undefined : ''} on:click={confirmSync}>{slowWarn ? 'Sync anyway' : 'Confirm and start'}</button></div>
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
          <figure class="wb-cov"><div class="wb-art big" aria-hidden="true">{#if editCurrent}<img src={editCurrent} alt="" />{:else}♪{/if}</div><figcaption>Now</figcaption></figure>
          {#if editCoverPreview}<span aria-hidden="true">→</span><figure class="wb-cov"><img class="wb-newcov" src={editCoverPreview} alt="New cover" /><figcaption>New</figcaption></figure>{/if}
          <div><button id="cover-choose" class="wb-covers-file" on:click={chooseCover}>{editCover ? 'Choose a different image…' : 'Choose image…'}</button>{#if editCover}<small>{editCover.split(/[\\/]/).pop()}</small>{/if}</div>
        </div>
        {#if editCoverError}<small role="alert">{editCoverError}</small>{/if}
        <small>Use a JPEG picture under 2 MB. If the Pocket can't show a picture, you'll be told before the sync starts.</small>
      </fieldset>
    {/if}
    <div class="wb-modal-actions"><button class="quiet" on:click={() => (dialog = null)}>Cancel</button><button class="primary" on:click={stageEdit}>Save (applies when you sync)</button></div>
  </div>
{/if}

{#if run && run.error}
  <div class="wb-veil" role="presentation"></div>
  <div class="wb-modal wb-run" role="alertdialog" aria-labelledby="run-t" aria-describedby="run-d" use:modal>
    <h2 id="run-t">{run.cancelled ? 'Sync cancelled' : 'Sync didn\u2019t finish'}</h2>
    <p id="run-d" role="alert">{run.error}</p>
    {#if run.reassurance}<p class="wb-fine wb-safe">{run.reassurance}</p>{/if}
    <div class="wb-modal-actions"><button class="quiet" on:click={closeRun}>Close</button><button class="primary" data-autofocus on:click={() => seeDetails(run?.journal ?? '')}>See what happened</button></div>
  </div>
{/if}
{#if syncResult}
  <SyncToast
    summary={[syncResult.report.copied && `${plural(syncResult.report.copied, 'track')} copied`, syncResult.report.deleted && `${plural(syncResult.report.deleted, 'track')} removed`, syncResult.report.edited && `${plural(syncResult.report.edited, 'track')} edited`].filter(Boolean).join(', ') || 'nothing needed copying'}
    warning={syncResult.warnings.length ? `${syncResult.warnings[0].message}${syncResult.warnings.length > 1 ? ` (and ${syncResult.warnings.length - 1} more in the details)` : ''}` : ''}
    {ejectMsg} {ejectOk} {ejecting} {verifiedOnDevice}
    onEject={eject} onDetails={() => seeDetails(syncResult?.journal ?? '')} onDismiss={closeRun} />
{/if}
<DetailsPanel open={detailsOpen} items={stagedItems} {pendingLine} {addBytes} {removeBytes} {freeNow} freeAfter={free} {tracksAfter} maxTracks={limits?.max_tracks ?? 0}
  coreLabel={coreLabel} coreBytes={activeSegment ? activeSegment.bytes_on_disk : null} warnings={detailsWarnings} backupNote={detailsBackup} locked={busy}
  fits={!!space && !overCapacity && !overLimit} {canStart} {startReason} onClose={() => (detailsOpen = false)} onRemove={unstage} onClear={clearAll}
  onStart={() => { detailsOpen = false; start(); }} />

<style>
  .wb{padding:24px 40px 20px;display:flex;flex-direction:column;gap:14px;height:100vh;position:relative}
  .wb-top{display:flex;justify-content:space-between;align-items:flex-start;margin:0;padding:0;flex-shrink:0}
  .wb-top h1{margin:0 0 4px;font-size:26px}.wb-ctx .eyebrow{margin:0 0 2px}
  .wb-sub{margin:0;display:flex;align-items:center;gap:10px;color:#9cacab;font-size:13px}
  .wb-dot{width:7px;height:7px;border-radius:50%;background:#8fd6f2}
  .wb-conn{background:#273032;color:#b7c3c2;border-radius:99px;padding:4px 10px;font-size:11px;font-weight:650}
  .wb-conn.slow{background:#3d3220;color:#f0d59a}
  .wb-eject{font-size:11px;padding:3px 10px;margin-left:6px}
  .wb-refresh{margin-right:52px;background:transparent;color:#6f807f;padding:8px;border-radius:9px;border:1px solid transparent}
  .wb-refresh:hover{color:#e8ecec;border-color:#2c393a}
  .wb-refresh.spin svg{animation:wb-spin .7s linear}
  @keyframes wb-spin{to{transform:rotate(360deg)}}
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
  .wb-list{display:flex;flex-direction:column;overflow:auto;flex:1;min-height:0}
  .wb-row{height:56px;box-sizing:border-box;margin-bottom:2px;display:flex;align-items:center;gap:10px;padding:8px 10px;border-radius:9px;border:1px solid transparent;cursor:default}
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
  .wb-slownote{font-size:12px;color:#f0d59a}.wb-slownote.bad{color:#ff9d8a;font-weight:650}
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
  .wb-banner{display:flex;justify-content:space-between;align-items:center;gap:16px;padding:10px 16px;background:#3a2420;border:1px solid #6b3a30;border-radius:10px;color:#f4cfc4;font-size:13px;flex-shrink:0}
  .wb-detail{margin:0 0 10px;font-size:13px}.wb-detail summary{cursor:pointer;color:#b7c3c2;padding:4px 0}
  .wb-detail ul{list-style:none;margin:6px 0 0;padding:0;display:grid;gap:4px;max-height:160px;overflow:auto}
  .wb-detail li{display:flex;gap:8px}.wb-detail small{color:#8c9c9b;margin-left:8px}.wb-detail .k{font-weight:700;width:14px;text-align:center}.wb-detail .k.add{color:#c1f0ad}.wb-detail .k.remove{color:#e8b59f}
  .wb-warnlist{margin:6px 0 0;padding-left:18px;font-size:12px}
  .wb-keep{color:#f0d59a!important}
  .wb-path{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .wb-skip{position:absolute;left:12px;top:-40px;z-index:120;background:#c1f0ad;color:#142015;padding:8px 12px;border-radius:8px;font-weight:700;font-size:13px;text-decoration:none}.wb-skip:focus{top:8px}
  .wb-fit{color:#c1f0ad}.wb-fit.bad{color:#ff9d8a;font-weight:650}.wb-fit-line{line-height:1.4}
  .wb-was{color:#8c9c9b;font-weight:400;font-size:12px;margin-left:6px}
  .wb-warn{margin:0 0 12px;padding:12px 14px;background:#3d3220;border:1px solid #6b5630;border-radius:10px;color:#f0d59a;font-size:13px}.wb-warn p{margin:6px 0 8px;color:#e5d2a6}.wb-warn .wb-check{margin:0}
  .wb-banner-fail{background:#3d3220;border-color:#6b5630;color:#f0d59a}.wb-banner-actions{display:flex;gap:6px;flex-shrink:0}
  .wb-safe{color:#c1f0ad!important}
  .wb-listwrap{display:flex;flex-direction:column;flex:1;min-height:0}
  .wb-art img{width:100%;height:100%;object-fit:cover;border-radius:7px;display:block}
  .wb-art{overflow:hidden}
  .wb-tools{display:flex;gap:8px}.wb-tools .wb-search{flex:1;min-width:0}
  .wb-sort{background:#111617;color:#e8ecec;border:1px solid #2c393a;border-radius:8px;padding:6px 8px;font-size:12px}
  .wb-filters{display:flex;gap:6px;flex-wrap:wrap}
  .wb-filters button{background:#1a2325;color:#a6b3b2;border:1px solid #2c393a;border-radius:99px;padding:4px 10px;font-size:12px}
  .wb-filters button.on{background:#1d2c22;color:#c1f0ad;border-color:#2f4a37}
  .wb-selectall{display:flex;gap:8px;align-items:center;font-size:12px;color:#8c9c9b;padding:2px 4px;cursor:pointer}
  .wb-sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
  .wb-cov{margin:0;display:flex;flex-direction:column;align-items:center;gap:4px;font-size:11px;color:#8c9c9b}.wb-newcov{width:72px;height:72px;object-fit:cover;border-radius:8px}
</style>
