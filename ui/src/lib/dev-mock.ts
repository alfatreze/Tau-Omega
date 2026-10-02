/** Browser-only stand-in for the Tauri backend, so the UI can be run and
 * screenshotted with `npm run dev` without the desktop shell or a real card.
 * Installed by main.ts only in dev builds when Tauri is absent; never shipped.
 *
 * The library workbench commands are stateful: adding, removing and editing
 * albums through `execute_changes` changes what the next `list_library`
 * returns, and progress events are emitted like the real engine's. */
import { mockIPC } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';

const CARD = '/Volumes/Pocket';
const MEDIA = `${CARD}/Assets/tau/common`;
const LIBRARY = '/Users/me/Music';
const MB = 1e6;
const GB = 1e9;

const cores = [
  { id: 'tau.omega', author: 'tau', shortname: 'omega', version: '0.3.0', platform: 'tau', platform_category: 'Media Players', library_capable: true, index_status: 'ok', tracks: 7180 },
  { id: 'tau.player2', author: 'tau', shortname: 'player2', version: '0.2.1', platform: 'tau2', platform_category: 'Media Players', library_capable: true, index_status: 'missing', tracks: null },
  { id: 'agg23.SNES', author: 'agg23', shortname: 'SNES', version: '1.4.0', platform: 'snes', platform_category: 'Console', library_capable: false, index_status: 'n/a', tracks: null },
];

type Album = { id: string; dest_id: string; title: string; artist: string; year: string | null; tracks: number; bytes: number; has_cover: boolean };
const album = (artist: string, title: string, tracks: number, mb: number, year: number): Album => ({
  id: `${artist}/${title}`, dest_id: `${artist}/${title}`, title, artist, year: String(year), tracks, bytes: mb * MB, has_cover: true,
});
const sourceAlbums: Album[] = [
  album('Miles Davis', 'Kind of Blue', 5, 310, 1959), album('John Coltrane', 'A Love Supreme', 4, 268, 1965),
  album('John Coltrane', 'Blue Train', 5, 290, 1958), album('Charles Mingus', 'Mingus Ah Um', 9, 402, 1959),
  album('Dave Brubeck Quartet', 'Time Out', 7, 344, 1959), album('Herbie Hancock', 'Head Hunters', 4, 380, 1973),
  album('Art Blakey', 'Moanin', 6, 322, 1958), album('Sonny Rollins', 'Saxophone Colossus', 5, 296, 1956),
  album('Charles Mingus', 'The Black Saint', 4, 712, 1963), album('Miles Davis', 'Bitches Brew', 7, 1380, 1970),
  album('Herbie Hancock', 'Maiden Voyage', 5, 330, 1965),
];
// The card starts with four albums; Blue Train is short a track ("changed").
let cardAlbums: Album[] = ['Kind of Blue', 'Blue Train', 'Time Out', 'Maiden Voyage'].map((t) => {
  const a = { ...sourceAlbums.find((s) => s.title === t)! };
  if (t === 'Blue Train') { a.tracks = 4; a.bytes -= 60 * MB; }
  return a;
});
let mounted = true;
let connection: 'direct_usb' | 'card_reader' | 'unknown' = 'direct_usb';
let prefs = { history_keep_last: 100, history_keep_days: 365, remove_mode: 'backup', backup_dir: null as string | null, remove_explained: false, slow_alert_suppressed: false, speeds: {} as Record<string, number>, connections: {} as Record<string, string> };
const CAPACITY = 8 * GB;
const OTHER_USED = 3.1 * GB;

// Sync history, newest first (what `list_history` returns). Each mock sync adds an entry, like the real journal.
type Entry = Record<string, unknown> & { path: string; recorded_at_unix: number };
let history: Entry[] = [];
const nowSecs = () => Math.floor(Date.now() / 1000);
function addEntry(fields: Record<string, unknown>, at = nowSecs()) {
  const entry: Entry = { path: `/mock/reports/${at}-${history.length}-library-changes.json`, kind: 'library_changes', state: 'completed', recorded_at_unix: at, plan_id: `plan-${at}-${history.length}`, destination: MEDIA, files: 0, copied: 0, deleted: 0, error: null, error_code: null, started_at_unix: at - 5, duration_secs: 5, edited: 0, bytes_written: 0, bytes_per_sec: null, phase: 'done', context: null, ...fields };
  history = [entry, ...history].sort((a, b) => b.recorded_at_unix - a.recorded_at_unix);
  return entry;
}
function seedHistory() {
  const day = 86400, t = nowSecs();
  addEntry({ state: 'completed', copied: 9, bytes_written: 402e6, bytes_per_sec: 2.1e6, duration_secs: 190, files: 9, context: { card: 'Pocket', core: 'omega', connection: 'direct_usb', items: [{ kind: 'add', title: 'Mingus Ah Um', artist: 'Charles Mingus', tracks: 9, bytes: 402e6 }] } }, t - 2 * day);
  addEntry({ state: 'failed', error: 'No such file or directory (os error 2)', error_code: 42, phase: 'copy', copied: 2, bytes_written: 80e6, context: { card: 'Pocket', core: 'omega', connection: 'direct_usb', items: [{ kind: 'add', title: 'Head Hunters', artist: 'Herbie Hancock', tracks: 4, bytes: 380e6 }] } }, t - 5 * day);
  addEntry({ state: 'completed', copied: 0, deleted: 7, edited: 5, files: 12, phase: 'done', context: { card: 'Pocket', core: 'omega', connection: 'card_reader', items: [{ kind: 'remove', title: 'Time Out', artist: 'Dave Brubeck Quartet', tracks: 7, bytes: 344e6 }, { kind: 'edit', title: 'Blue Train', label: 'Blue Train → Blue Train (Remaster) · cover', note: 'title, cover' }] } }, t - 9 * day);
}
function prune() {
  const t = nowSecs();
  let list = history;
  if (prefs.history_keep_last > 0) list = list.slice(0, prefs.history_keep_last);
  if (prefs.history_keep_days > 0) list = list.filter((e) => t - e.recorded_at_unix <= prefs.history_keep_days * 86400);
  const removed = history.length - list.length; history = list; return removed;
}
let maxTracks = 16384;
let failNext: null | { code: number; message: string; phase: string } = null;
const listing = (albums: Album[]) => ({
  limits: { max_tracks: maxTracks, max_albums: 2048, max_artists: 1024 },
  albums,
  tracks: albums.flatMap((a) => Array.from({ length: a.tracks }, (_, i) => ({
    rel: `${a.dest_id}/${String(i + 1).padStart(2, '0')} Track ${i + 1}.mp3`, album_id: a.id, title: `Track ${i + 1}`, artist: a.artist, album: a.title,
    secs: 180 + i * 17, bytes: Math.round(a.bytes / a.tracks), format: i % 3 ? 'MP3' : 'FLAC',
  }))),
  playlists: [{ name: 'Favourites', file: 'Favourites.m3u', tracks: 12 }],
  warnings: [],
});

let planWarnings: { code: string; message: string }[] = [];
let planDelayMs = 0;
let mockAssetsOnCard = false;
let lastOptions: unknown = null;
const plan = (request: any) => {
  lastOptions = request.options;
  const trackCount = (list: Album[]) => list.reduce((n, a) => n + a.tracks, 0);
  const addList = (request.add_albums as string[]).map((id) => sourceAlbums.find((a) => a.id === id)!).filter(Boolean);
  const rmList = (request.remove_albums as string[]).map((id) => cardAlbums.find((a) => a.id === id)!).filter(Boolean);
  const after = trackCount(cardAlbums) + trackCount(addList.filter((a) => !cardAlbums.some((c) => c.id === a.id))) - trackCount(rmList);
  if (after > maxTracks) throw { code: 14, message: `This would put ${after} tracks on the Pocket, but its library holds at most ${maxTracks}. Remove some albums or add fewer.` };
  const adds = (request.add_albums as string[]).map((id) => sourceAlbums.find((a) => a.id === id)!).filter(Boolean);
  const removes = (request.remove_albums as string[]).map((id) => cardAlbums.find((a) => a.id === id)!).filter(Boolean);
  const files = (list: Album[]) => list.reduce((n, a) => n + a.tracks, 0);
  return {
    id: `mock-${adds.length}-${removes.length}-${request.edits.length}`,
    new_files: files(adds), updated_files: 0, unchanged_files: 0,
    bytes_to_write: adds.reduce((n, a) => n + a.bytes, 0),
    removed_files: files(removes), bytes_to_remove: removes.reduce((n, a) => n + a.bytes, 0),
    edited_files: request.edits.length * 5, playlists_updated: removes.length ? 1 : 0, warnings: planWarnings,
  };
};

/** Emits copy progress over a few seconds, then applies the change set. */
async function runChanges(request: any, jobId: string) {
  const p = plan(request);
  const context = (request as any).__context ?? null;
  if (failNext) {
    const e = failNext; failNext = null; await new Promise((r) => setTimeout(r, 300));
    addEntry({ state: e.code === 44 ? 'failed' : 'failed', error: e.message, error_code: e.code, phase: e.phase, files: p.new_files + p.removed_files, context });
    throw { code: e.code, message: e.message };
  }
  const total = p.bytes_to_write;
  const steps = 24;
  const adding = (request.add_albums as string[]).map((id) => sourceAlbums.find((a) => a.id === id)!).filter(Boolean);
  const folderAt = (offset: number) => {
    let start = 0;
    for (const a of adding) { if (offset < start + a.bytes) return a.dest_id; start += a.bytes; }
    return adding[adding.length - 1]?.dest_id ?? 'misc';
  };
  for (let i = 0; i <= steps && total > 0; i++) {
    const offset = Math.min(total - 1, Math.round((total * i) / steps));
    await emit('tau://progress', { job_id: jobId, stage: 'copying', done: Math.round((total * i) / steps), total, path: `/Volumes/Mock/Assets/tau/common/${folderAt(offset)}/${String(i).padStart(2, '0')}.mp3` });
    await new Promise((r) => setTimeout(r, 120));
  }
  await emit('tau://progress', { job_id: jobId, stage: 'building_index', done: 1, total: 1, path: null });
  await new Promise((r) => setTimeout(r, 300));
  const removing = new Set<string>(request.remove_albums);
  cardAlbums = cardAlbums.filter((a) => !removing.has(a.id));
  for (const id of request.add_albums as string[]) {
    const src = sourceAlbums.find((a) => a.id === id);
    if (src) cardAlbums = [...cardAlbums.filter((a) => a.id !== id), { ...src }];
  }
  for (const e of request.edits as any[]) {
    cardAlbums = cardAlbums.map((a) => (a.id === e.album_id ? { ...a, title: e.fields.album ?? a.title, artist: e.fields.artist ?? a.artist, year: e.fields.year ?? a.year, has_cover: e.cover ? true : a.has_cover } : a));
  }
  const done = { plan_id: p.id, copied: p.new_files, unchanged: 0, bytes_written: total, edited: p.edited_files, reapplied: 0, deleted: p.removed_files, playlists_updated: p.playlists_updated, backup_dir: null, index_path: `${MEDIA}/tau-library.tdb`, phase: 'done' };
  const entry = addEntry({ state: 'completed', copied: done.copied, deleted: done.deleted, edited: done.edited, files: done.copied + done.deleted, bytes_written: total, bytes_per_sec: 2.4e6, duration_secs: 6, context });
  prune();
  return { ...done, journal: entry.path };
}

const handlers: Record<string, (args: any) => unknown> = {
  get_reports_dir: () => '~/Library/Application Support/Tau Omega/reports',
  get_recent_cards: () => [CARD],
  list_mounted_cards: () => (mounted ? [CARD] : []),
  get_manual_players: () => [],
  inspect_card: () => cores,
  read_persisted_settings: () => [],
  read_check_summary: () => null,
  read_qr_report: () => null,
  list_screenshots: () => [],
  read_core_icon: () => null,
  read_platform_image: () => null,
  // A 1x1 grey PNG, so cover previews have something to show.
  read_image_data_url: () => 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==',
  list_journals: () => [],
  list_library: (a) => listing(String(a.path).startsWith(LIBRARY) ? sourceAlbums : cardAlbums),
  scan_media: () => ({ playlists: [{ name: 'Favourites', file: 'Favourites.m3u8', tracks: ['Music/a.mp3'] }], warnings: [] }),
  find_problems: () => [{ kind: 'missing_tag', files: ['Music/x.mp3'], message: 'No artist tag' }],
  check_storage_capacity: (a) => {
    const used = OTHER_USED + cardAlbums.reduce((n, x) => n + x.bytes, 0);
    return { space: { total_bytes: CAPACITY, available_bytes: CAPACITY - used }, bytes_needed: a.bytesNeeded, margin_bytes: 16 * MB, fits: CAPACITY - used >= a.bytesNeeded };
  },
  card_breakdown: () => {
    const albums = cardAlbums.reduce((n, x) => n + x.bytes, 0);
    const player2 = 0.25 * GB;
    const used = OTHER_USED + albums;
    return {
      total_bytes: CAPACITY, available_bytes: CAPACITY - used, unit: 32768,
      segments: [
        { platform: 'tau', core_ids: ['tau.omega'], shortnames: ['omega'], bytes_on_disk: albums, files: 400 },
        { platform: 'tau2', core_ids: ['tau.player2'], shortnames: ['player2'], bytes_on_disk: player2, files: 60 },
      ],
      tau_bytes: albums + player2, other_bytes: used - albums - player2,
    };
  },
  plan_changes: async (a) => { if (planDelayMs) await new Promise((r) => setTimeout(r, planDelayMs)); return plan(a.request); },
  execute_changes: (a) => runChanges({ ...a.request, __context: a.context }, a.jobId),
  // A colourful placeholder "cover" per album (stable per id) so lists and the edit drawer have artwork to show.
  album_thumbnails: (a) => (a.ids as string[]).map((id) => {
    if (id.includes('Time Out')) return { id, png_base64: null }; // one album with no picture, to test the placeholder
    let h = 0; for (const c of id) h = (h * 31 + c.charCodeAt(0)) % 360;
    const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='96' height='96'><rect width='96' height='96' fill='hsl(${h},45%,38%)'/><circle cx='48' cy='48' r='26' fill='hsl(${(h + 40) % 360},50%,62%)'/></svg>`;
    return { id, png_base64: btoa(svg), mime: 'image/svg+xml' };
  }),
  image_thumbnail: () => "data:image/svg+xml;base64," + btoa("<svg xmlns='http://www.w3.org/2000/svg' width='160' height='160'><rect width='160' height='160' fill='#b5651d'/><circle cx='80' cy='80' r='46' fill='#f2c078'/></svg>"),
  list_history: () => history,
  prune_history: () => prune(),
  clear_history: () => { const n = history.length; history = []; return n; },
  detect_connection: () => ({ kind: connection, detail: connection === 'direct_usb' ? 'Analogue Pocket (USB)' : 'Generic card reader (USB)' }),
  ledger_forget: () => true,
  // The real checks live in the engine (tau_core::assets, tested against the firmware's own tool); the mock only needs the same shape.
  appearance_check: (a) => appearanceCheckMock(a.theme),
  appearance_open: () => [],
  appearance_export: () => 144,
  appearance_plan_install: (a) => ({
    id: 'mock-plan', destination: `${a.mediaRoot}/tau-assets.bin`, bytes: 144, sha256: 'x', themes: (a.themes as { name: string }[]).map((t) => t.name),
    existing: mockAssetsOnCard ? { bytes: 144, sha256: 'y', themes: ['OLDER'], other_sections: ['METR'], readable: true } : null,
    readers: [{ core_id: 'alfatreze.TAU', version: '0.6.0', declares_slot: true }], interrupted_install: false,
    warnings: mockAssetsOnCard ? ['The file already there also holds METR, which is not carried over: it will be gone after this install (the backup keeps it).'] : [],
  }),
  appearance_install: (a) => { mockAssetsOnCard = true; return { destination: `${a.mediaRoot}/tau-assets.bin`, bytes_written: 144, replaced: false, backup: a.backup ?? null }; },
  eject_card: () => ({ ok: true, message: 'Safe to remove. The card is unmounted; you can unplug the reader or leave USB mode on the Pocket.' }),
  readback_status: () => ({ checks_the_device: true, failed_evictions: 0 }),
  get_prefs: () => ({ ...prefs, default_backup_dir: '~/Library/Application Support/Tau Omega/removed-backups', reports_dir: '~/Library/Application Support/Tau Omega/reports' }),
  set_prefs: (a) => { prefs = { ...a.prefs }; return { ...prefs, default_backup_dir: '~/Library/Application Support/Tau Omega/removed-backups', reports_dir: '~/Library/Application Support/Tau Omega/reports' }; },
  plan_sync: () => ({ id: 'mock-plan-1', new_files: 42, updates: 5, unchanged: 7133, bytes_to_write: 412_000_000, art_sidecars: 3, art_sidecar_previews: [], warnings: [] }),
  compare_media: () => ({ left: '/mock/a', right: '/mock/b', only_left: 2, only_right: 1, different: 1, identical: 10, differences: [{ relative: 'Music/x.mp3', state: 'only_left', left_bytes: 4e6, right_bytes: null }] }),
  cancel_job: () => null,
  set_manual_player: () => null,
  record_recent_card: () => null,
  set_reports_dir: () => null,
};

export function installDevMock() {
  // Test hooks (dev only): simulate plugging the card in/out, changing its
  // contents behind the app's back, and switching how it is connected.
  (window as any).__tauMock = {
    setPlanDelay: (ms: number) => { planDelayMs = ms; },
    setMounted: (value: boolean) => { mounted = value; },
    addCardAlbum: (title: string) => { cardAlbums = [...cardAlbums, { ...album('Test Artist', title, 3, 100, 2001) }]; },
    setConnection: (value: typeof connection) => { connection = value; },
    setMaxTracks: (value: number) => { maxTracks = value; },
    setPlanWarnings: (list: { code: string; message: string }[]) => { planWarnings = list; },
    lastOptions: () => lastOptions,
    failNext: (code: number, message: string, phase = 'copy') => { failNext = { code, message, phase }; },
    seedHistory,
    addOldEntries: (n: number, daysAgo: number) => { for (let i = 0; i < n; i++) addEntry({ state: 'completed', context: { card: 'Pocket', items: [] } }, nowSecs() - daysAgo * 86400 - i); },
    historyCount: () => history.length,
  };
  // Test hook: `?connection=card_reader` or `?connection=unknown` in the URL.
  const query = new URLSearchParams(location.search);
  if (query.get('history') === 'seed') seedHistory();
  // ?big=2000 adds that many generated albums to the local library (to test long lists).
  for (let i = 0; i < Number(query.get('big') ?? 0); i++) sourceAlbums.push(album(`Artist ${String(i % 97).padStart(2, '0')}`, `Album ${String(i).padStart(4, '0')}`, 8, 100 + (i % 50), 2000));
  const wanted = query.get('connection');
  if (wanted === 'card_reader' || wanted === 'unknown') connection = wanted;
  mockIPC((cmd, args) => {
    if (cmd.startsWith('plugin:dialog|')) return (args as any)?.options?.directory ? LIBRARY : '/mock/cover.jpg';
    const h = handlers[cmd];
    if (!h) { console.warn(`[dev-mock] unmocked command: ${cmd}`); throw { code: 'E_MOCK', message: `No mock for ${cmd}` }; }
    return h(args);
  }, { shouldMockEvents: true });
}


function lumOf(hex: string) {
  const f = (x: number) => { const v = x / 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
  const n = parseInt(hex.slice(1), 16);
  return 0.2126 * f((n >> 16) & 255) + 0.7152 * f((n >> 8) & 255) + 0.0722 * f(n & 255);
}
function appearanceCheckMock(t: { name: string; dark: any; light: any }) {
  const problems: string[] = []; const checks: any[] = [];
  if (!/^[A-Z0-9 _-]{1,15}$/.test(t.name)) problems.push('The name must be 1 to 15 characters: capital letters, digits, space, _ or - (the Pocket\'s font is capitals only).');
  if (['TAU', 'OCEAN'].includes(t.name)) problems.push(`“${t.name}” is already a built-in theme; pick another name.`);
  for (const pol of ['dark', 'light'] as const) {
    const c = t[pol].colors;
    for (const [text, back, need] of [['text_primary', 'surface', 4.5], ['text_secondary', 'surface', 3.0]] as const) {
      const a = lumOf(c[text]), b = lumOf(c[back]);
      const worst = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
      checks.push({ polarity: pol, text, against: back, worst, needed: need, ok: worst >= need });
      if (worst < need) problems.push(`${pol === 'dark' ? 'Dark' : 'Light'}: ${text.replace('_', ' ')} on ${back} is too faint (${worst.toFixed(2)}, needs ${need}).`);
    }
  }
  return { problems, checks, dark_snapped: { ...t.dark.colors }, light_snapped: { ...t.light.colors } };
}
