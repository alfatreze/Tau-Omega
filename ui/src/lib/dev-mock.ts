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

const mc = (id: string, name: string, owner: string, repo: string, category: string, manufacturer: string, year: number, version: string, date: string, license = false, extra: Record<string, unknown> = {}) => ({
  id, author: id.split('.')[0], name, repository: { owner, name: repo }, requires_license: license, license_filename: license ? 'beta.bin' : '',
  sponsor: { links: license ? [] : [`https://github.com/sponsors/${owner}`] }, replaces: [] as string[], declares_ai: [] as string[],
  platform_id: name.toLowerCase().replace(/\W/g, ''), version, release_date: date, category, manufacturer, year,
  download_url: version ? `https://github.com/${owner}/${repo}/releases/download/${version}/${id}.${version}.zip` : '',
  firmware_required: '1.1', dock_supported: true, link_port: false, sleep_supported: false,
  assets: [{ platform: name.toLowerCase().replace(/\W/g, ''), filename: '', extensions: ['sfc', 'smc'], core_specific: false }],
  history: version ? [{ version, release_date: date, download_url: '' }, { version: '0.9.0', release_date: '2025-05-01', download_url: '' }] : [],
  source: 'openfpga-library', channel: '',
  ...extra,
});
const tauCore = (id: string, name: string, channel: string, version: string, date: string) => mc(id, name, 'alfatreze', 'Tau-Alpha', 'Music player', 'Tau', 0, version, date, false, { source: 'tau-alpha', channel, platform_id: 'tau', sponsor: { links: [] }, assets: [], firmware_required: '', dock_supported: null, link_port: null, sleep_supported: null, history: version ? [{ version, release_date: date, download_url: '' }] : [] });
const marketCores = [
  tauCore('alfatreze.TAU', 'TAU', 'Stable', '0.5.0', '2026-09-27'),
  tauCore('alfatreze.TAU Preview', 'TAU Preview', 'Preview', '0.6.0-preview.1', '2026-10-09'),
  tauCore('alfatreze.TAU DEV', 'TAU DEV', 'Dev', '', ''),
  mc('agg23.SNES', 'SNES', 'agg23', 'openfpga-SNES', 'Console', 'Nintendo', 1990, '1.4.0', '2026-09-02', false, { firmware_required: '2.0' }),
  mc('agg23.Arduboy', 'Arduboy', 'agg23', 'openfpga-arduboy', 'Handheld', 'Arduboy', 2016, '0.9.0', '2022-09-03', false, { history: [{ version: '0.9.0', release_date: '2022-09-03', download_url: '' }] }),
  mc('Alius.ChisKit', 'ChisKit', 'gpzi994', 'ChisKit', 'Utility', 'Alius', 2026, '0.01b', '2026-10-11', false, { declares_ai: ['Claude'] }),
  mc('jotego.jtcps1', 'Capcom CPS1', 'jotego', 'jtcores', 'Arcade', 'Capcom', 1988, 'c68f02e', '2026-10-11', true),
  mc('coc.bublbobl', 'Bubble Bobble', 'coc', 'bublbobl', 'Arcade', 'Taito', 1986, '1.0.0', '2026-09-26', true),
  mc('spiritualized.GB', 'Game Boy', 'spiritualized', 'gb', 'Handheld', 'Nintendo', 1989, '1.2.1', '2026-08-14', false, { dock_supported: false }),
  mc('Mazamars312.PCE', 'PC Engine', 'Mazamars312', 'pce', 'Console', 'NEC', 1987, '2.0.3', '2025-12-01'),
  mc('zz.NoRelease', 'Unreleased', 'zz', 'none', 'Computer', 'Nobody', 2026, '', ''),
  ...['Amiga', 'C64', 'ZX Spectrum', 'NES', 'Genesis', 'Neo Geo', 'Atari 2600', 'Intellivision', 'Vectrex', 'MSX', 'CPC', 'Apple II'].map((n, i) =>
    mc(`pack.${n.replace(/\W/g, '')}`, n, 'pack', n.toLowerCase().replace(/\W/g, ''), i % 3 ? 'Computer' : 'Console', 'Various', 1977 + i * 2, `1.${i}.0`, `2025-0${1 + (i % 9)}-1${i % 9}`)),
];
// Placeholder pictures for the dev mock: a square "icon" and a wide "banner" drawn from the id, none for one core.
const svgUrl = (w: number, h: number, id: string) => {
  let hue = 0; for (const c of id) hue = (hue * 31 + c.charCodeAt(0)) % 360;
  return `data:image/svg+xml;utf8,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}"><rect width="100%" height="100%" fill="#000"/><circle cx="${w / 2}" cy="${h / 2}" r="${Math.min(w, h) / 3}" fill="hsl(${hue},0%,85%)"/></svg>`)}`;
};
// ---- Cores section mock: a small card with GBA (BIOS missing), GB, Amiga and a core with no platform ----------------------------
const plat = (id: string, name: string, category: string, manufacturer: string, year: number) => ({ id, name, category, manufacturer, year, has_image: true });
const PLAT: Record<string, ReturnType<typeof plat>> = {
  gba: plat('gba', 'Game Boy Advance', 'Handheld', 'Nintendo', 2001), gb: plat('gb', 'Game Boy', 'Handheld', 'Nintendo', 1989), snes: plat('snes', 'SNES', 'Console', 'Nintendo', 1990),
  tau: plat('tau', 'TAU', 'Media Players', 'alfatreze', 2026),
  amiga: plat('amiga', 'Amiga', 'Computer', 'Commodore', 1985), _none: { id: '_none', name: 'No platform', category: '', manufacturer: '', year: 0, has_image: false },
};
const crow = (id: string, platform: string, version: string, missing = 0) => ({ id, shortname: id.split('.')[1], author: id.split('.')[0], version, platform, platforms: [platform], missing });
let mockRole = 'real'; const mockNotes: { id: string; card: string; stamp: string; core_id: string; shortname: string; version: string; package: string; package_sha256: string; bitstreams: [string, string][]; backup_dir: string; files_written: number; text: string; lint_overrides: string[]; results: unknown[]; results_at: string | null }[] = [{ id: '20261014100500-0', card: 'Pocket', stamp: '2026-10-14T10:05:00', core_id: 'alfatreze.TAU DEV 112', shortname: 'TAU DEV 112', version: '0.6.0', package: 'TAU_DEV_112.zip', package_sha256: 'a'.repeat(64), bitstreams: [['Cores/alfatreze.TAU DEV 112/bitstream.rbf_r', 'b'.repeat(64)]], backup_dir: '/backups/omega/20261014-1005', files_written: 12, lint_overrides: [] as string[], text: '', results: [], results_at: null }]; let mockSince: string | null = '2026-10-14T19:30:00'; let mockBuildsDir: string | null = null; let mockSchemes: { name: string; field: string; template: string }[] = [];
const mockValidate = (t: string): string | null => { if (!t.trim()) return 'The template is empty.'; const toks = [...t.matchAll(/\{(\w+)\}/g)].map((m) => m[1]); if (toks.some((x) => !['series', 'n', 'major', 'minor', 'patch', 'pre', 'any'].includes(x))) return 'A token is not known.'; if (/\}\{/.test(t)) return 'Two tokens touch. Put some text between them so Omega knows where one ends.'; if (!toks.some((x) => ['n', 'major', 'minor', 'patch', 'pre'].includes(x))) return 'Nothing here orders the builds. Add {n}, a SemVer token or {pre}.'; return null; };
const mockDisabled: { id: string; shortname: string; version: string; disabled_at: number; reasons: string[] }[] = [];
const coresTreeMock = () => coresTreeAll().map((n) => ({ ...n, cores: n.cores.map((c) => ({ ...c, disabled: mockDisabled.some((d) => d.id === c.id) })) }));
let mockNewCores: ReturnType<typeof crow>[] = [];
const coresTreeAll = () => [
  { platform: PLAT.snes, cores: [crow('agg23.SNES', 'snes', '0.4.4'), ...mockNewCores] },
  { platform: PLAT.gb, cores: [crow('Spiritualized.GB', 'gb', '1.3.0')] },
  { platform: PLAT.gba, cores: [crow('Spiritualized.GBA', 'gba', '1.2.0', mockBios ? 0 : 1), crow('other.GBAlt', 'gba', '0.3')] },
  { platform: PLAT.amiga, cores: [crow('Mazamars312.Amiga', 'amiga', '0.2.3')] },
  { platform: PLAT.tau, cores: [crow('tau.omega', 'tau', '0.3.0'), { ...crow('tau.player2', 'tau_preview', '0.2.1'), platforms: ['tau_preview', 'tau'] }] },
  { platform: PLAT._none, cores: [crow('solo.Thing', '', '1.0')] },
];
const slotOf = (name: string, o: Record<string, unknown> = {}) => ({ id: '0x100', name, required: false, filename: '', extensions: [] as string[], nonvolatile: false, core_specific: false, platform: 'gba', size_exact: null, size_maximum: null, ...o });
const detailFor = (id: string) => {
  const row = coresTreeMock().flatMap((n) => n.cores).find((c) => c.id === id)!;
  const p = row.platform ? [PLAT[row.platform]] : [];
  const slots = row.platform === 'gba' && id === 'Spiritualized.GBA'
    ? [slotOf('Cartridge', { required: true, extensions: ['gba', 'rom'] }), slotOf('Save RAM', { nonvolatile: true, extensions: ['sav'] }), slotOf('BIOS', { required: true, filename: 'gba_bios.bin', extensions: ['bin'], size_exact: 16384 })]
    : [slotOf('ROM', { required: true, extensions: row.platform === 'amiga' ? ['adf', 'hdf'] : ['gb', 'gbc'], platform: row.platform })];
  return { id, shortname: row.shortname, author: row.author, description: `${row.shortname} for the Analogue Pocket.`, url: `https://example.org/${row.shortname}`, version: row.version, date_release: '2026-01-02', firmware_required: '1.1', dock_supported: true, link_port: false, sleep_supported: false, platforms: p, slots, has_icon: true, has_interact: true };
};
const fe = (rel: string, bytes: number, kind: string, slot: string | null, extra: Record<string, unknown> = {}) => ({ rel_path: rel, name: rel.split('/').pop()!, bytes, modified: 1767225600, kind, slot, game: null, taken: null, group: kind === 'Rom' ? 'Game data' : '', purpose: '', purpose_source: '', support: false, ...extra });
const mockRoms: string[] = ['Advance Wars (USA).gba', 'Boktai 2 (USA).gba', 'Golden Sun (USA).gba', 'Metroid Fusion (USA).gba', 'Mother 3.gba', 'Zelda - Minish Cap (USA).gba'];
let mockBios = false; const mockAdded: string[] = []; const mockGone: string[] = [];
const filesFor = (id: string) => {
  const d = detailFor(id); const gba = id === 'Spiritualized.GBA';
  const roms = gba ? [...mockRoms, ...mockAdded].filter((n) => !mockGone.includes(n)).map((n) => fe(n, 8 * MB, 'Rom', 'Cartridge')).concat(mockBios ? [fe('gba_bios.bin', 16384, 'Bios', 'BIOS', { group: 'BIOS and system', support: true, purpose_source: 'known', purpose: 'The Game Boy Advance boot ROM (16 KB). The core needs it to start any game. Slot "BIOS": required, exactly 16 KB.' })] : []).sort((a, b) => a.name.localeCompare(b.name)) : [];
  const folder = (role: string, label: string, rel: string, entries: unknown[], exists = true) => ({ folder: { role, platform: d.platforms[0]?.id ?? '', label, rel_path: rel }, exists, entries, total_files: entries.length, total_bytes: (entries as { bytes: number }[]).reduce((a, e) => a + e.bytes, 0), truncated: false });
  const pid = d.platforms[0]?.id ?? '_none';
  return {
    folders: [
      folder('AssetsShared', `Shared by every ${d.platforms[0]?.name ?? ''} core`, `Assets/${pid}/common`, roms),
      folder('AssetsCore', `Only ${d.shortname}`, `Assets/${pid}/${id}`, gba ? [fe('C2C03.PAL', 192, 'Bios', 'VS Pal', { group: 'Palettes', support: true, purpose_source: 'core', purpose: 'Colour palette for Nintendo Vs. System arcade games (the RP2C03 picture chip). Only used when playing Vs. System games. Slot "VS Pal": optional, exactly 192 bytes, only this core reads it.' }), fe('mpu.bin', 31616, 'Other', null, { group: 'Other files', support: true, purpose_source: 'guess', purpose: "Not named in the core's file list. It is in the core's own folder, so the core or its helper firmware probably reads it directly." })] : [], !!gba),
      folder('SavesShared', `Saves shared by every ${d.platforms[0]?.name ?? ''} core`, `Saves/${pid}/common`, gba ? [fe('Boktai 2 (USA).sav', 65536, 'Save', 'Save RAM', { game: 'Boktai 2 (USA)' }), fe('Mother 3.sav', 131072, 'Save', 'Save RAM', { game: 'Mother 3' })] : []),
      folder('SaveStates', 'Save states', `Memories/Save States/${id}`, gba ? [fe('20260101_120000_USR_00000000_Boktai 2 (USA).sta', 900000, 'State', null, { game: 'Boktai 2 (USA)', taken: '2026-01-01 12:00:00' })] : []),
    ],
    missing: gba && !mockBios ? [{ slot: 'BIOS', filename: 'gba_bios.bin', folder_rel: 'Assets/gba/common', size_exact: 16384 }] : [],
    save_backups: [{ name: 'pocket-sync-save-backup__1791411688.zip', bytes: 2.4 * MB, modified: 1767225600 }],
  };
};
const thumb = (name: string) => { let h = 0; for (const c of name) h = (h * 31 + c.charCodeAt(0)) % 360; return `data:image/svg+xml;utf8,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="240" height="160"><rect width="100%" height="100%" fill="hsl(${h},35%,22%)"/><rect x="20" y="20" width="200" height="120" fill="hsl(${h},30%,35%)"/></svg>`)}`; };
const SHOTS = Array.from({ length: 24 }, (_, i) => ({ file: `202601${String(28 - Math.floor(i / 3)).padStart(2, '0')}_1${i % 6}0${i % 10}00.png`, taken: `2026-01-${String(28 - Math.floor(i / 3)).padStart(2, '0')} 1${i % 6}:0${i % 10}:00`, bytes: 900000, meta: i < 5 ? { core: 'Spiritualized.GBA', version: '1.2.0', loaded_file: 'Boktai 2 (USA).gba', platform_id: 'gba', platform_name: 'Game Boy Advance' } : i < 20 ? { core: 'other.GBAlt', version: '0.3', loaded_file: 'Mine.gba', platform_id: 'gba', platform_name: 'Game Boy Advance' } : null, recorded: i < 5, likely: i >= 20 && i < 22 ? 'no core is recorded in this older file; taken within a minute of one of this core\'s save states' : null }));
const addPlanFor = (a: Record<string, unknown>) => {
  const sources = (a.sources as string[]) ?? [];
  const items = sources.map((s) => { const name = s.split('/').pop()!; const dup = mockRoms.includes(name) || mockAdded.includes(name); return { source: s, dest_rel: `Assets/gba/${a.destination === 'CoreOnly' ? a.coreId : 'common'}/${name}`, bytes: 8 * MB, sha256: 'x', state: name.includes(':') ? { Refused: 'the name has a character the card cannot hold' } : dup ? (a.replace ? 'Different' : 'Identical') : 'New' }; });
  const n = (f: (s: unknown) => boolean) => items.filter((i) => f(i.state)).length;
  const fresh = n((s) => s === 'New'); const exts = [...new Set(sources.map((s) => s.split('.').pop()!).filter((e) => !['gba', 'rom', 'bin', 'sav'].includes(e)))];
  return { id: `mockadd-${a.destination}-${a.replace}-${sources.length}`, core_id: a.coreId, platform: a.platform, destination: a.destination, folder_rel: `Assets/gba/${a.destination === 'CoreOnly' ? a.coreId : 'common'}`, replace: a.replace, items, new_files: fresh, identical: n((s) => s === 'Identical'), different: n((s) => s === 'Different'), refused: n((s) => typeof s === 'object'), bytes_to_write: (fresh + n((s) => s === 'Different' && !!a.replace)) * 8 * MB, warnings: exts.length ? [`No file slot of this core reads ${exts.map((e) => '.' + e).join(', ')}; the core may ignore ${exts.length === 1 ? 'it' : 'them'}.`] : [], blocked: fresh + (a.replace ? n((s) => s === 'Different') : 0) === 0 ? 'Nothing to add: everything is already on the card.' : null };
};

const mockSkip: { rel: string; auto: [number, number]; edit: { never: boolean; lead: number | null; tail: number | null } | null }[] = [
  { rel: 'Live At The Hall/01 Intro.mp3', auto: [441000 - 8820, 0], edit: null },
  { rel: 'Live At The Hall/02 Hidden Track.mp3', auto: [0, 2646000 - 22050], edit: null },
  { rel: 'Live At The Hall/03 Plain.mp3', auto: [0, 0], edit: null },
];
const cores = [
  { id: 'tau.omega', author: 'tau', shortname: 'omega', version: '0.3.0', platform: 'tau', platforms: ['tau'], media_platform: 'tau', platform_category: 'Media Players', library_capable: true, audio_formats: [], index_status: 'ok', tracks: 7180 },
  { id: 'tau.player2', author: 'tau', shortname: 'player2', version: '0.2.1', platform: 'tau2', platforms: ['tau2'], media_platform: 'tau2', platform_category: 'Media Players', library_capable: true, audio_formats: [], index_status: 'missing', tracks: null },
  { id: 'harpmudd.Mp3Player', author: 'harpmudd', shortname: 'Mp3Player', version: '1.7.0', platform: 'mp3player', platforms: ['mp3player'], media_platform: 'mp3player', platform_category: 'Others', library_capable: false, audio_formats: ['mp3', 'flac'], playlist: { file: 'playlist.m3u', fixed_name: true, max_tracks: 256, max_bytes: 12288, name_limit: 12, list_file: 'playlists.m3u', formats: ['mp3', 'flac'] }, index_status: 'n/a', tracks: null },
  { id: 'agg23.SNES', author: 'agg23', shortname: 'SNES', version: '1.4.0', platform: 'snes', platforms: ['snes'], media_platform: 'snes', platform_category: 'Console', library_capable: false, audio_formats: [], index_status: 'n/a', tracks: null },
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
let prefs = { history_keep_last: 100, history_keep_days: 365, remove_mode: 'backup', backup_dir: null as string | null, remove_explained: false, slow_alert_suppressed: false, check_updates: true, update_notice_shown: true, card_marker: 'ask', core_disable: (() => { try { return localStorage.getItem('tau.mock.coreDisable') ?? 'never'; } catch { return 'never'; } })(), core_keep: {} as Record<string, string[]>, developer_tools: (() => { try { return localStorage.getItem('tau.mock.devTools') === '1'; } catch { return false; } })(), speeds: {} as Record<string, number>, connections: {} as Record<string, string>, omega_channel: 'auto', trim: { enabled: true, threshold_db10: -600, lead_margin_ms: 200, tail_margin_ms: 500, min_ms: 2000 }, convert: { enabled: true, mode: 'auto', max_rate: 48000, flac_bits: 24, reduce_playable: false, mp3: { vbr: 0 }, lame_path: null as string | null, lame_dir: null as string | null, analysis_cache: null as string | null, replaygain: 'add_missing' } };
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
let lastRequest: any = null;
const plan = (request: any) => {
  lastOptions = request.options; lastRequest = request;
  const trackCount = (list: Album[]) => list.reduce((n, a) => n + a.tracks, 0);
  const addList = (request.add_albums as string[]).map((id) => sourceAlbums.find((a) => a.id === id)!).filter(Boolean);
  const rmList = (request.remove_albums as string[]).map((id) => cardAlbums.find((a) => a.id === id)!).filter(Boolean);
  const after = trackCount(cardAlbums) + trackCount(addList.filter((a) => !cardAlbums.some((c) => c.id === a.id))) - trackCount(rmList);
  if (after > maxTracks) throw { code: 14, message: `This would put ${after} tracks on the Pocket, but its library holds at most ${maxTracks}. Remove some albums or add fewer.` };
  const cap = request.playlist?.max_tracks; if (cap && after > cap) throw { code: 14, message: `This would put ${after} tracks on the Pocket, but ${request.playlist.core_name} plays at most ${cap} from its playlist. Remove ${after - cap} ${after - cap === 1 ? 'track' : 'tracks'} or add fewer.` };
  const adds = (request.add_albums as string[]).map((id) => sourceAlbums.find((a) => a.id === id)!).filter(Boolean);
  const removes = (request.remove_albums as string[]).map((id) => cardAlbums.find((a) => a.id === id)!).filter(Boolean);
  const files = (list: Album[]) => list.reduce((n, a) => n + a.tracks, 0);
  return {
    id: `mock-${adds.length}-${removes.length}-${request.edits.length}`,
    new_files: files(adds), updated_files: 0, unchanged_files: 0,
    bytes_to_write: adds.reduce((n, a) => n + a.bytes, 0),
    removed_files: files(removes), bytes_to_remove: removes.reduce((n, a) => n + a.bytes, 0),
    edited_files: request.edits.length * 5, playlists_updated: removes.length ? 1 : 0, warnings: planWarnings, converted_files: 0, replaygain_files: 0, skipped: [],
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
  read_core_icon: () => `data:image/svg+xml;utf8,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="36" height="36"><circle cx="18" cy="18" r="12" fill="white"/></svg>')}`,
  read_platform_image: () => `data:image/svg+xml;utf8,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="521" height="165"><rect width="100%" height="100%" fill="#000"/><rect x="190" y="50" width="140" height="70" rx="10" fill="#ddd"/></svg>')}`,
  // A 1x1 grey PNG, so cover previews have something to show.
  read_image_data_url: () => 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==',
  list_journals: () => [],
  list_library: async (a) => {
    const isSource = String(a.path).startsWith(LIBRARY);
    const delay = isSource ? Number((() => { try { return localStorage.getItem('tau.mock.listDelay'); } catch { return 0; } })() ?? 0) : 0;
    if (delay) await new Promise((r) => setTimeout(r, delay));
    const result = listing(isSource ? sourceAlbums : cardAlbums);
    if (isSource && a.includeConvertible !== undefined) { try { localStorage.setItem('tau.mock.saved', JSON.stringify({ listing: result, saved_at: Math.floor(Date.now() / 1000) })); } catch { /* none */ } }
    return result;
  },
  list_library_saved: () => { try { const s = localStorage.getItem('tau.mock.saved'); return s ? JSON.parse(s) : null; } catch { return null; } },
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
  diag_read: () => ({
    summary: '# Tau diagnostics\n\nmock summary',
    reading: {
      card: '/Volumes/Pocket', total_bytes: 64e9, free_bytes: 20e9, shots_examined: 12, notes: [],
      cores: [{ id: 'alfatreze.TAU_DEV_67', version: '0.6.0', date_release: '2026-09-30', platform: 'tau_dev_67', persist_path: 'Settings/x', check: { profile: 'USER CHECK', run: 5, verdict: 'some checks failed', passed: ['SDRAM window test'], failed: ['Playback counters'], worst_access_cycles: 373, cold_cycles_per_word: 31.6, late_underruns: 1, draw_stall_ms: 4, last_load_s: 1.2, library_error: 0, cold_error: 0, firmware_minor: 6 }, check_note: null, files: [], index: { tracks: 77, albums: 8, artists: 5, playlists: 2, build_id: 1 }, media_files: 120, media_bytes: 5e8 }],
      shots: [{ filename: '20261002_191502.png', captured_at: null, note: null, report: { format: 1, profile: 'USER CHECK', verdict: 'all checks passed', tests: [{ id: 0, name: 'SDRAM window test', result: 'PASS', value: 89, busy_permille: null, audio_full: null }], entries: {}, unknown: [] } }],
    },
  }),
  diag_zip: (a) => `${a.destDir}/tau-diagnostics-alfatreze.TAU_DEV_67-0.6.0-2026-10-03.zip`,
  appearance_plan_install: (a) => ({
    id: 'mock-plan', destination: `${a.mediaRoot}/tau-assets.bin`, bytes: 144, sha256: 'x', themes: ((a.themes ?? []) as { name: string }[]).map((t) => t.name), presets: [],
    existing: mockAssetsOnCard ? { bytes: 144, sha256: 'y', themes: ['OLDER'], other_sections: ['METR'], readable: true } : null,
    readers: [{ core_id: 'alfatreze.TAU', version: '0.6.0', declares_slot: true }], interrupted_install: false,
    warnings: mockAssetsOnCard ? ['The file already there also holds METR, which is not carried over: it will be gone after this install (the backup keeps it).'] : [],
  }),
  appearance_install: (a) => { mockAssetsOnCard = true; return { destination: `${a.mediaRoot}/tau-assets.bin`, bytes_written: 144, replaced: false, backup: a.backup ?? null }; },
  // the mock runs as a dev build (0.5.1-dev.1): Stable and Preview offer a switch, Dev and Automatic offer nothing
  omega_update_check: () => { if (prefs.omega_channel === 'stable') return { current: '0.5.1-dev.1', version: '0.5.0', notes: 'Release', date: null, channel: 'stable', switch: true }; if (prefs.omega_channel === 'preview') return { current: '0.5.1-dev.1', version: '0.5.1-preview.1', notes: 'Preview build', date: null, channel: 'preview', switch: true }; throw { code: 7, message: 'Development builds are updated by hand. Choose Preview or Stable above to follow releases.' }; },
  omega_update_install: () => undefined,
  halcyon_open: () => [],
  halcyon_import_apo: (a) => ({ preset: { kind: 'raw', name: a.name as string, preamp: 3000000, stages: [[4194304, 0, 0, 0, 0]] }, preamp_db: -2.9, peak_gain_db: 2.9, filters: 1 }),
  halcyon_curve: () => Array.from({ length: 200 }, (_, i) => [20 * 1000 ** (i / 199), 0]),
  halcyon_validate: () => 24,
  halcyon_export: () => 144,
  halcyon_plan_install: (a) => ({
    id: 'mock-plan', destination: `${a.mediaRoot}/tau-assets.bin`, bytes: 144, sha256: 'x', themes: [], presets: (a.presets as { name: string }[]).map((t) => t.name),
    existing: null, readers: [{ core_id: 'alfatreze.TAU', version: '0.6.0', declares_slot: true }], interrupted_install: false, warnings: [],
  }),
  halcyon_install: (a) => ({ destination: `${a.mediaRoot}/tau-assets.bin`, bytes_written: 144, replaced: false, backup: a.backup ?? null }),
  eject_card: () => ({ ok: true, message: 'Safe to remove. The card is unmounted; you can unplug the reader or leave USB mode on the Pocket.' }),
  readback_status: () => ({ checks_the_device: true, failed_evictions: 0 }),
  inspect_package: () => ({ source: '/x.zip', core_ids: ['alfatreze.TAU'], entries: [{ path: 'Cores/alfatreze.TAU/core.json', bytes: 891, sha256: 'x' }] }),
  plan_core_update: (a) => { const base = ((a.zip as string).split('/').pop() ?? 'core').replace(/\.zip$/i, ''); const upd = /update/i.test(base); return ({ id: 'mockplan-' + base, update: { cores: [{ verdict: upd ? 'update' : 'new_install', reasons: [], installed: null, package: { core_id: 'alfatreze.' + base, shortname: base, version: '0.6.0', date_release: '2026-10-07', platform: 'tau' }, pair: { verified: { core_version: '4D50331A' } }, installed_release: upd ? 'v0.6.0-alpha.3' : null, package_release: 'v0.6.0-alpha.4' }], files_replaced: [], user_files_kept: [], persist_changed: upd ? [16] : null },
    files: upd ? { id: 'f', source: '', destination: '', items: [{ path: 'Assets/tau/common/tau.rom', state: 'different', bytes: 107197 }, { path: 'Cores/alfatreze.TAU/bitstream.rbf_r', state: 'identical', bytes: 1910624 }], new_files: 0, updated_files: 1, unchanged_files: 1, bytes_to_write: 107197 } : { id: 'f', source: '', destination: '', items: [{ path: `Cores/alfatreze.${base}/core.json`, state: 'only_left', bytes: 965 }, { path: `Cores/alfatreze.${base}/bitstream.rbf_r`, state: 'only_left', bytes: 1900000 }], new_files: 2, updated_files: 0, unchanged_files: 0, bytes_to_write: 1900965 },
    backup: upd ? [{ path: 'Assets/tau/common/tau.rom', bytes: 100000 }] : [], backup_bytes: upd ? 100000 : 0, obsolete_to_remove: [], caches_to_clear: ['System/cores_cache.bin'], stubs_to_sweep: [], user_files_kept: upd ? ['Assets/tau/common/tau-library.tdb'] : [], superseded_candidates: [], capacity: { space: { available_bytes: 5e9 }, bytes_needed: upd ? 107197 : 1900965, margin_bytes: 1048576, fits: true }, nothing_to_do: false, refused: null, cautions: [], allow_downgrade: false }); },
  execute_core_update: (a) => { const base = ((a.zip as string).split('/').pop() ?? 'core').replace(/\.zip$/i, ''); mockNewCores = [...mockNewCores, crow('new.' + base, 'snes', '1.0.0')]; return ({ backup_dir: '/backups/mock-' + base, files_written: 2, bytes_written: 1900965, files_backed_up: 0, obsolete_removed: 0, caches_cleared: 1, stubs_swept: 0, nothing_to_do: false, checks: [{ core_id: 'alfatreze.' + base, items: [{ name: 'core.json', status: 'pass', detail: 'valid.' }], verdict: 'pass', summary: `alfatreze.${base}: all 1 checks passed.` }] }); },
  rollback_core_update: () => ({ restored: 2, created_removed: 0 }),
  library_health: () => ({ present: true, valid: true, tracks: 120, root: '/Assets/tau/common/', root_matches: true, missing_files: 0, albums_without_cover_file: 3 }),
  lame_status: () => ({ path: '/opt/homebrew/bin/lame', version: 'LAME 64bits version 4.0', options: [{ method: 'brew', label: 'Install with Homebrew', command: 'brew install lame', runnable: true }] }),
  check_lame: () => 'LAME 64bits version 4.0',
  install_lame: () => ({ path: '/opt/homebrew/bin/lame', version: 'LAME 64bits version 4.0', options: [] }),
  plan_cover_files: () => ({ id: 'cp', albums: 40, have_cover_file: 37, items: [], no_picture: [] }),
  execute_cover_files: () => 3,
  plan_library_refresh: () => ({ id: 'rp', media_root: '/x', root_prefix: '/Assets/tau/common/', before: { present: true, valid: true, tracks: 120, root: '/Assets/tau_dev_42/common/', root_matches: false, missing_files: 120 }, media_files: 124, would_index: 122,
    findings: [{ rel: 'Beyoncé/Lemonade/01 Formation.mp3', reason: 'non_ascii_name', message: 'The Pocket cannot open names with accents or other non-ASCII characters; this is renamed to its plain form.', fix: 'Beyonce/Lemonade/01 Formation.mp3', skipped: false }, { rel: 'Misc/voice memo.m4a', reason: 'unsupported_format', message: 'The Pocket plays MP3 and FLAC only, not .m4a; this file is not in the library.', fix: null, skipped: true }],
    renames: [{ from: 'Beyoncé', to: 'Beyonce' }], playlist_fixes: [{ file: 'All.m3u', lines: 2 }], stubs_to_remove: 3, refused: null, nothing_to_do: false }),
  execute_library_refresh: () => ({ backup_dir: '/backups/refresh-rp', renamed: 1, playlists_rewritten: 1, stubs_removed: 3, before: { present: true, valid: true, tracks: 120, root: '', root_matches: false, missing_files: 120 }, after: { present: true, valid: true, tracks: 122, root: '/Assets/tau/common/', root_matches: true, missing_files: 0 }, index_path: '/x/tau-library.tdb', nothing_to_do: false }),
  rollback_library_refresh: () => null,
  card_marker_status: () => ({ setting: prefs.card_marker, is_card: true, present: prefs.card_marker === 'on' }),
  set_card_marker: (a) => { prefs.card_marker = a.setting; return { setting: a.setting, is_card: true, present: a.setting === 'on' }; },
  remove_card_marker: () => ({ setting: prefs.card_marker, is_card: true, present: false }),
  marketplace_catalog: () => {
    // Failure modes for checking the error states by hand: set window.__mkt = 'fail' | 'offline' in the console.
    const mode = (globalThis as { __mkt?: string }).__mkt;
    if (mode === 'fail') throw { code: 'E_IO', message: 'Could not reach the server. Are you offline?' };
    // `slow` keeps the skeleton on screen for a moment (set window.__mkt = 'slow').
    if (mode === 'slow') return new Promise((resolve) => setTimeout(() => resolve({ notes: [], catalog: { cores: marketCores }, source: 'Network', fetched: Math.floor(Date.now() / 1000) }), 1500));
    return { notes: [], catalog: { cores: marketCores }, source: mode === 'offline' ? { Offline: 'Could not reach the server. Are you offline?' } : 'Network', fetched: Math.floor(Date.now() / 1000) - (mode === 'offline' ? 3 * 86400 : 120) };
  },
  marketplace_art: (a) => (a.coreId === 'zz.NoRelease' ? null : a.kind === 'Banner' ? svgUrl(521, 165, a.coreId as string) : svgUrl(36, 36, a.coreId as string)),
  marketplace_core_updates: () => ({ rows: [
    { id: 'agg23.SNES', installed_version: '1.2.0', latest_version: '1.4.0', release_date: '2026-09-02', status: 'UpdateAvailable' },
    { id: 'spiritualized.GB', installed_version: '1.0.0', latest_version: '1.2.1', release_date: '2026-08-14', status: 'UpdateAvailable' },
    { id: 'Mazamars312.PCE', installed_version: '1.9.0', latest_version: '2.0.3', release_date: '2025-12-01', status: 'UpdateAvailable' },
    { id: 'someone.Unlisted', installed_version: '1.0.0', latest_version: '1.1.0', release_date: '2026-10-01', status: 'UpdateAvailable', manual_url: 'https://github.com/someone/unlisted-core/releases/tag/v1.1.0' },
    { id: 'agg23.Arduboy', installed_version: '0.9.0', latest_version: '0.9.0', release_date: '2022-09-03', status: 'UpToDate' },
    { id: 'old.NES', installed_version: '0.3', latest_version: '1.1.0', release_date: '2025-04-01', status: { ReplacedBy: 'pack.NES' } },
    { id: 'me.Homebrew', installed_version: '0.1', latest_version: '', release_date: '', status: 'NotInCatalog' },
    { id: 'alfatreze.TAU', installed_version: '0.6.0', latest_version: '', release_date: '', status: 'Excluded' },
  ], source: 'Cache', fetched: Math.floor(Date.now() / 1000) - 3600 }),
  marketplace_download: (a) => ({ download: { path: `/cache/${a.coreId}.zip`, bytes: 3.2 * MB, sha256: 'b'.repeat(64), checksum: a.coreId === 'agg23.Arduboy' ? 'NotPublished' : 'Verified' }, vetted: { core_id: a.coreId as string, version: (a.version as string | null) ?? '1.4.0', date_release: '2026-09-02', platform_ids: ['snes'], firmware_required: '2.0', dock_supported: true, files: 14, uncompressed_bytes: 6 * MB, asset_files: 2 } }),
  cores_tree: () => coresTreeMock(),
  core_suggestions: () => {
    const kept = prefs.core_keep.Pocket ?? []; const flagged: unknown[] = [];
    if (prefs.core_disable === 'never') return { mode: 'never', cores: [] };
    const live = (id: string) => !mockDisabled.some((d) => d.id === id) && !kept.includes(id);
    if (!mockBios && live('Spiritualized.GBA')) flagged.push({ id: 'Spiritualized.GBA', shortname: 'GBA', verdict: 'Unusable', needs_no_files: false, reasons: [{ code: 'missing-file', message: 'It needs gba_bios.bin (BIOS), which is not on the card.', fix: 'Put gba_bios.bin in Assets/gba/common.' }] });
    if (live('solo.Thing')) flagged.push({ id: 'solo.Thing', shortname: 'Thing', verdict: 'Broken', needs_no_files: false, reasons: [{ code: 'lint:L01', message: 'The core has no bitstream (bitstream.rbf_r).', fix: 'Reinstall the core.' }] });
    return { mode: prefs.core_disable, cores: flagged };
  },
  card_role: () => (mockRole), set_card_role: (a) => { mockRole = a.test ? 'test' : 'real'; return mockRole; },
  builds_config: () => ({ dir: mockBuildsDir, builtin: [{ name: 'Name and number (TAU DEV 12)', field: 'shortname', template: '{series} {n}' }, { name: 'SemVer pre-release (0.6.0-alpha.3)', field: 'version', template: '{major}.{minor}.{patch}-{pre}' }, { name: 'SemVer (0.6.0)', field: 'version', template: '{major}.{minor}.{patch}' }], custom: mockSchemes }),
  set_builds_config: (a) => { for (const s of (a.custom as { name: string; template: string }[])) { if (!s.name.trim()) throw { code: 7, message: 'Every scheme needs a name.' }; const e = mockValidate(s.template); if (e) throw { code: 7, message: `${s.name}: ${e}` }; } mockBuildsDir = (a.dir as string | null) || null; mockSchemes = a.custom as typeof mockSchemes; return handlers.builds_config(a); },
  try_scheme: (a) => { const e = mockValidate(a.template as string); if (e) return { error: e, placement: null }; const text = (a.field === 'core_id' ? a.coreId : a.field === 'shortname' ? a.shortname : a.field === 'version' ? a.version : a.file) as string; const toks: string[] = []; const re = new RegExp('^' + (a.template as string).replace(/[.*+?^$()|[\]\\]/g, '\\$&').replace(/\{(\w+)\}/g, (_m, t) => { toks.push(t); return t === 'series' || t === 'any' ? '(.+?)' : t === 'pre' ? '([0-9A-Za-z.-]+)' : '(\\d+)'; }) + '$', 'i'); const m = re.exec(text.trim()); if (!m) return { error: null, placement: null }; const g = (t: string) => { const i = toks.indexOf(t); return i < 0 ? null : m[i + 1]; }; return { error: null, placement: { scheme: 'try', series: (g('series') ?? (a.coreId as string)).toLowerCase(), order: [+(g('major') ?? 0), +(g('minor') ?? 0), +(g('patch') ?? 0), +(g('n') ?? 0)], pre: g('pre') } }; },
  builds_list: () => { if (!mockBuildsDir) throw { code: 7, message: 'Choose the folder your builds are in first.' }; const e = (file: string, short: string, ver: string, n: number, extra: Record<string, unknown> = {}) => ({ path: `${mockBuildsDir}/${file}`, kind: file.endsWith('.zip') ? 'zip' : 'folder', file, core_id: `alfatreze.${short}`, shortname: short, version: ver, date_release: '2026-10-14', bytes: 4.8 * MB, modified: 1792000000 + n, placement: { scheme: 'Name and number (TAU DEV 12)', series: 'tau dev', order: [0, 0, 0, n], pre: null }, lint_errors: 0, lint_warnings: 0, error: null, ...extra }); return { dir: mockBuildsDir, rows: [
      { entry: e('TAU_DEV_113.zip', 'TAU DEV 113', '0.6.0', 113, { lint_warnings: 1 }), relation: 'newer', older: [{ id: 'alfatreze.TAU DEV 111', shortname: 'TAU DEV 111', version: '0.6.0' }, { id: 'alfatreze.TAU DEV 112', shortname: 'TAU DEV 112', version: '0.6.0' }] },
      { entry: e('TAU_DEV_112.zip', 'TAU DEV 112', '0.6.0', 112), relation: 'same', older: [{ id: 'alfatreze.TAU DEV 111', shortname: 'TAU DEV 111', version: '0.6.0' }] },
      { entry: e('TAU_DEV_110', 'TAU DEV 110', '0.6.0', 110), relation: 'not_installed', older: [{ id: 'alfatreze.TAU DEV 111', shortname: 'TAU DEV 111', version: '0.6.0' }, { id: 'alfatreze.TAU DEV 112', shortname: 'TAU DEV 112', version: '0.6.0' }, { id: 'alfatreze.TAU DEV 105', shortname: 'TAU DEV 105', version: '0.6.0' }] },
      { entry: { ...e('weird.zip', 'Weird', '1.0', 0), placement: null, lint_errors: 1 }, relation: 'not_installed', older: [] },
      { entry: { ...e('broken.zip', '', '', 0), core_id: '', placement: null, bytes: 0, error: 'This zip has no core.json.' }, relation: 'not_installed', older: [] },
    ] }; },
  build_zip: (a) => a.path as string,
  plan_remove_core: (a) => ({ id: `rmc-${a.coreId}`, card_root: '/Volumes/Pocket', core_id: a.coreId, platform: 'tau', platform_shared: true, paths: [`Cores/${a.coreId}`], files_to_remove: 6, bytes_to_remove: 4 * MB, media_kept: true }),
  execute_remove_core: () => ({ removed_files: 6, bytes_removed: 4 * MB }),
  results_scan: () => { const t = (id: number, name: string, result: string, value = 0) => ({ id, name, result, value, busy_permille: null, audio_full: null });
    const rep = (profile: string, tests: ReturnType<typeof t>[], fw: string, underruns: number) => ({ format: 1, profile, tests, unknown: [], verdict: tests.some((x) => x.result === 'FAIL') ? 'some checks failed' : 'all checks passed', entries: { build: { firmware: fw, bitstream: 'bs-5', flags: 0, heap_gap: 9000 }, audio: { late_underruns: underruns, audio_full: true, stall_ms: 0, window_s: 30 } } });
    const shot = (name: string, at: string, report: unknown, _since: boolean, problem: string | null = null) => ({ filename: name, path: `/Volumes/Pocket/Memories/Screenshots/${name}`, captured_at: at, bytes: 40000, since_install: !mockSince || at >= mockSince, report, problem });
    const shots = [
      shot('20261014_201500.png', '2026-10-14T20:15:00', rep('STANDARD', [t(1, 'SDRAM window', 'PASS', 89), t(2, 'Track changes', 'FAIL'), t(3, 'Blit storm', 'PASS', 150)], 'fw-0.6.0+b2', 2), true),
      shot('20261014_201000.png', '2026-10-14T20:10:00', null, true),
      shot('20261014_200500.png', '2026-10-14T20:05:00', null, true, 'The pixel grid is damaged: 3 cells could not be read.'),
      shot('20261014_190000.png', '2026-10-14T19:00:00', rep('STANDARD', [t(1, 'SDRAM window', 'PASS', 89), t(2, 'Track changes', 'FAIL'), t(3, 'Blit storm', 'PASS', 150)], 'fw-0.6.0+b1', 0), false),
      shot('20261013_100000.png', '2026-10-13T10:00:00', rep('FULL', [t(1, 'SDRAM window', 'PASS', 89)], 'fw-0.6.0+b1', 0), false),
    ];
    const runs = [{ profile: 'STANDARD', latest: shots[0].filename, previous: shots[3].filename, since_install: shots[0].since_install, diff: [{ kind: 'build', label: 'Firmware', before: 'fw-0.6.0+b1', after: 'fw-0.6.0+b2', change: 'changed' }, { kind: 'metric', label: 'Late underruns', before: '0', after: '2', change: 'worse' }] }, { profile: 'FULL', latest: shots[4].filename, previous: null, since_install: shots[4].since_install, diff: [] }];
    return { since: mockSince, shots, runs }; },
  set_last_install: (a) => { mockSince = (a.stamp as string | null) ?? null; return null; },
  run_notes_list: () => mockNotes.map((n) => ({ ...n })),
  run_note_record: (a) => { const n = { lint_overrides: a.lintOverrides ?? [], id: `2026${mockNotes.length}-0`, card: 'Pocket', stamp: a.stamp, core_id: 'someone.Unlisted', shortname: 'Unlisted', version: '1.0.0', package: (a.zip as string).split('/').pop(), package_sha256: 'e'.repeat(64), bitstreams: [['Cores/someone.Unlisted/bitstream.rbf_r', 'f'.repeat(64)]], backup_dir: a.backupDir, files_written: a.filesWritten, text: '', results: [], results_at: null }; mockNotes.unshift(n as never); return n; },
  run_note_text: (a) => { const n = mockNotes.find((x) => x.id === a.id)!; n.text = a.text as string; return { ...n }; },
  run_note_delete: (a) => { const i = mockNotes.findIndex((x) => x.id === a.id); if (i >= 0) mockNotes.splice(i, 1); return null; },
  run_note_attach: (a) => { const n = mockNotes.find((x) => x.id === a.id)!; n.results = [{ profile: 'STANDARD', captured_at: '2026-10-14T20:15:00', verdict: 'some checks failed', failing: ['Track changes'], firmware: 'fw-0.6.0+b2', bitstream: 'bs-5', changes: [{ kind: 'metric', label: 'Late underruns', before: '0', after: '2', change: 'worse' }] }] as never; n.results_at = a.now as string; return { ...n }; },
  run_note_report: (a) => { const n = mockNotes.find((x) => x.id === a.id)!; return `# ${n.shortname} ${n.version} on Pocket\n\n- Installed: ${n.stamp.replace('T', ' ')}\n- Package: \`${n.package}\`\n\n## Note\n\n${n.text || '(none)'}\n`; },
  run_note_export: () => '/Users/dev/reports/run-Unlisted-1.md',
  lint_package: (a) => { const z = a.zip as string; const bad = /weird/.test(z); const warn = /warny/.test(z); const f = bad ? [{ id: 'L09', severity: 'Error', message: 'The bitstream file bitstream.rbf_r is missing or empty.', rule: 'core.json cores[].filename names the bitstream', fix: 'Add the converted bitstream next to core.json.' }] : warn ? [{ id: 'L05', severity: 'Warning', message: 'The description is 71 characters; the documented limit is 63.', rule: 'Analogue core.json: description at most 63 characters', fix: 'Shorten the description.' }] : []; return { core_id: 'x.Y', findings: f, errors: bad ? 1 : 0, warnings: warn ? 1 : 0, notes: 0 }; },
  check_source: (a) => { const t = a.path as string; const bad = /weird/.test(t); const f = bad ? [{ id: 'L09', severity: 'Error', message: 'The bitstream file bitstream.rbf_r is missing or empty.', rule: 'core.json cores[].filename names the bitstream', fix: 'Add the converted bitstream next to core.json.' }, { id: 'L04', severity: 'Warning', message: 'description is 71 characters; the documented limit is 63.', rule: 'Analogue core.json: description at most 63 characters', fix: 'Shorten description to 63 characters or fewer.' }] : []; return { title: t.split('/').pop(), lint: { core_id: 'x.Y', findings: f, errors: bad ? 1 : 0, warnings: bad ? 1 : 0, notes: 0 }, symptoms: [] }; },
  check_installed: (a) => { const id = a.coreId as string; const sym = id === 'solo.Thing' ? [{ id: 'M4', severity: 'Error', message: 'The folder is named solo.Thing, but core.json says Solo.Thing.', rule: 'Analogue docs: the Pocket lists only valid cores in Cores/<author>.<shortname>, from its menu cache', fix: 'Rename the folder to Solo.Thing, or fix author and shortname in core.json.' }, { id: 'M7', severity: 'Warning', message: "The Pocket's menu cache (System/corelist_cache.bin) is older than this core's files, so the menu may not have been rebuilt since it was installed.", rule: 'Analogue docs: the Pocket lists only valid cores in Cores/<author>.<shortname>, from its menu cache', fix: 'Clear the five menu cache files in System (an Omega install does this for you); the Pocket rebuilds its menu on the next start.' }] : []; return { title: id, lint: { core_id: id, findings: [], errors: 0, warnings: 0, notes: 0 }, symptoms: sym }; },
  check_card: () => [{ id: 'agg23.SNES', shortname: 'SNES', errors: 0, warnings: 0, notes: 0 }, { id: 'Spiritualized.GBA', shortname: 'GBA', errors: 0, warnings: 1, notes: 0 }, { id: 'solo.Thing', shortname: 'Thing', errors: 1, warnings: 0, notes: 0 }],
  carry_candidates: (a) => { const old = a.older as [string, string][]; if (!old.length) return []; const out: unknown[] = [{ kind: 'folder', from_id: old[0][0], from_name: old[0][1], platform: 'tau', source_rel: `Assets/tau/${old[0][0]}`, dest_rel: `Assets/tau/${a.newId}`, files: 14, bytes: 52 * MB, note: `${old[0][1]} kept these files in its own folder; the new build reads Assets/tau/${a.newId}.` }]; if (old[1]) out.push({ kind: 'shared', from_id: old[1][0], from_name: old[1][1], platform: 'tau', source_rel: '', dest_rel: '', files: 0, bytes: 0, note: `${old[1][1]} and the new build read the same music library (Assets/tau/common): nothing to carry.` }); if (old[2]) out.push({ kind: 'library', from_id: old[2][0], from_name: old[2][1], platform: 'tau_old', source_rel: 'Assets/tau_old/common', dest_rel: 'Assets/tau/common', files: 120, bytes: 900 * MB, note: `${old[2][1]} reads another library. Copying it rebuilds the index with the right root path.` }); return out; },
  carry_plan: (a) => ({ id: `carry-${a.fromId}`, source_rel: `Assets/${a.platform}/${a.fromId}`, dest_rel: `Assets/${a.platform}/${a.toId}`, items: [{ rel: 'a.bin', bytes: 4 * MB, state: 'new' }, { rel: 'b.bin', bytes: 4 * MB, state: 'new' }, { rel: 'c.pal', bytes: 192, state: 'identical' }, { rel: 'd.cfg', bytes: 10, state: 'different' }], new_files: 2, identical: 1, different: 1, bytes_to_write: 8 * MB, blocked: null }),
  carry_execute: () => ({ copied: 2, identical: 1, left_different: 1, bytes_written: 8 * MB }),
  plan_core_copy: () => ({ id: 'cc1', new_files: 120, updates: 0, unchanged: 0, bytes_to_write: 900 * MB, art_sidecars: 0, art_sidecar_previews: [], warnings: [] }),
  execute_core_copy: () => ({ plan_id: 'cc1', copied: 120, unchanged: 0, bytes_written: 900 * MB, deleted: 0, art_sidecars_written: 0, converted: 0, replaygain_tagged: 0, index_path: '/Volumes/Pocket/Assets/tau/common/tau-library.tdb', index_sha256: 'a'.repeat(64), warnings: [] }),
  builder_template_info: (a) => { const t = a.source as { kind: string; id?: string; path?: string }; const id = t.kind === 'card' ? t.id! : 'alfatreze.TAU DEV 112'; const short = id.split('.').slice(1).join('.') || 'Core'; return { core_id: id, shortname: t.kind === 'card' && id === 'Spiritualized.GBA' ? 'GBA DEV 1' : short, author: id.split('.')[0], version: '0.6.0-alpha.3', description: 'Test build of the player', platforms: ['tau'] }; },
  builder_template_view: (a) => { const t = a.source as { kind: string; id?: string }; const id = t.kind === 'card' ? t.id! : 'alfatreze.TAU DEV 112'; const author = id.split('.')[0]; const short = t.kind === 'card' && id === 'Spiritualized.GBA' ? 'GBA DEV 1' : (id.split('.').slice(1).join('.') || 'Core'); const full = `${author}.${short}`;
    return { core_id: full, shortname: short, author, description: 'Test build of the player', url: `https://example.org/${short}`, version: '0.6.0-alpha.3', date_release: '2026-10-14', firmware_required: '1.1', dock_supported: true, link_port: false, sleep_supported: null, platform_ids: ['gba'], platform: { id: 'gba', name: 'Game Boy Advance', category: 'Handheld', manufacturer: 'Nintendo', year: 2001 },
      files: [[`Cores/${full}/audio.json`, 120], [`Cores/${full}/core.json`, 900], [`Cores/${full}/data.json`, 800], [`Cores/${full}/icon.bin`, 2592], [`Cores/${full}/interact.json`, 1500], ['Platforms/_images/gba.bin', 169620], ['Platforms/gba.json', 160]], icon: thumb('icon-' + short), banner: thumb('banner-gba') }; },
  builder_preview_image: (a) => thumb(`${a.kind}-${a.path}-${a.fit}`),
  builder_next_name: (a) => { const t = a.text as string; let m = /^(.*\D)(\d+)$/.exec(t); if (a.field === 'version') { m = /^(.*\D)(\d+)$/.exec(t); } return m ? { scheme: a.field === 'version' ? 'SemVer pre-release (0.6.0-alpha.3)' : 'Name and number (TAU DEV 12)', next: m[1] + String(+m[2] + 1).padStart(m[2].length, '0') } : null; },
  builder_plan: (a) => { const sp = a.spec as { rbf: string; shortname: string; version: string; icon_png: string | null; extra: { dest: string }[]; edits?: { author: string | null; banner: { path: string } | null; platform: { id: string } | null } }; if (/\.sof$/i.test(sp.rbf)) throw { code: 55, message: 'A .sof is the JTAG programming file. Choose the raw binary file Quartus writes to output_files (.rbf): turn on Raw Binary File in Programming Files.' }; if (/\.rbf_r$/i.test(sp.rbf)) throw { code: 55, message: 'This is already a converted bitstream (.rbf_r). Choose the raw .rbf from Quartus: converting twice makes a bitstream the Pocket cannot load.' }; if (!sp.shortname.trim()) throw { code: 55, message: 'Give the build a shortname.' };
    const id = `${sp.edits?.author?.trim() || 'alfatreze'}.${sp.shortname.trim()}`; const f = (rel: string, bytes: number, origin: string) => ({ rel, bytes, origin, sha256: rel.length.toString(16).padStart(2, '0').repeat(32) });
    const files = [f(`Cores/${id}/core.json`, 1200, 'generated'), f(`Cores/${id}/data.json`, 800, 'template'), f(`Cores/${id}/bitstream.rbf_r`, 4194304, 'bitstream'), ...(sp.icon_png ? [f(`Cores/${id}/icon.bin`, 2592, 'icon')] : []), ...(sp.edits?.platform ? [f(`Platforms/${sp.edits.platform.id}.json`, 170, 'generated')] : []), ...(sp.edits?.banner ? [f('Platforms/_images/gba.bin', 169620, 'banner')] : []), ...sp.extra.map((e) => f(e.dest.replace('{id}', id), 3, 'extra'))];
    const bad = /bad/i.test(sp.shortname); const lint = { core_id: id, errors: bad ? 1 : 0, warnings: 1, notes: 0, findings: [...(bad ? [{ id: 'L11', severity: 'Error', message: 'audio.json is missing from the package.', rule: 'Analogue docs: the core folder holds seven JSON files', fix: 'Add it.' }] : []), { id: 'L04', severity: 'Warning', message: 'description is 71 characters; the documented limit is 63.', rule: 'Analogue core.json: description at most 63 characters', fix: 'Shorten description to 63 characters or fewer.' }] };
    return { core_id: id, files, raw_sha256: 'a'.repeat(64), converted_sha256: 'b'.repeat(64), bitstream_bytes: 4194304, lint, blocked: bad ? 'The core check found an error: audio.json is missing from the package. Add it.' : null }; },
  builder_build: (a) => { const sp = a.spec as { shortname: string }; const plan = handlers.builder_plan(a) as { core_id: string }; const dir = (a.outDir as string | null) || mockBuildsDir; if (!dir) throw { code: 7, message: 'Choose the folder to build into (the build folder on the Builds page is the usual one).' }; if (/dup/i.test(sp.shortname)) throw { code: 55, message: `${plan.core_id}.zip already exists in that folder. Change the shortname or choose another folder: a build is never overwritten.` }; return { zip: `${dir}/${plan.core_id}.zip`, record: `${dir}/${plan.core_id}.zip.build.json`, zip_bytes: 3.1 * MB, zip_sha256: 'c'.repeat(64), plan }; },
  disabled_cores: () => [...mockDisabled],
  plan_core_state: (a) => ({ token: `st-${a.enable}-${(a.ids as string[]).join('+')}`, enable: a.enable, ids: a.ids, from: (a.ids as string[]).map((i) => `${a.enable ? 'Disabled Cores' : 'Cores'}/${i}`), to: (a.ids as string[]).map((i) => `${a.enable ? 'Cores' : 'Disabled Cores'}/${i}`), blocked: (a.ids as string[]).length ? null : 'No core is selected.', notes: [] }),
  execute_core_state: (a) => {
    for (const id of a.ids as string[]) {
      const at = mockDisabled.findIndex((d) => d.id === id);
      if (a.enable) { if (at >= 0) mockDisabled.splice(at, 1); }
      else if (at < 0) { const row = coresTreeAll().flatMap((n) => n.cores).find((c) => c.id === id); const why = ((a.reasons as [string, string[]][]).find((r) => r[0] === id) ?? [id, []])[1]; mockDisabled.push({ id, shortname: row?.shortname ?? id, version: row?.version ?? '', disabled_at: 1792091402, reasons: why }); }
    }
    return { moved: (a.ids as string[]).length, caches_cleared: 5 };
  },
  core_keep: (a) => { const l = (prefs.core_keep.Pocket ??= []); for (const id of a.ids as string[]) { const i = l.indexOf(id); if (i >= 0) l.splice(i, 1); if (a.keep) l.push(id); } return null; },
  core_detail: (a) => detailFor(a.coreId as string),
  core_files: (a) => filesFor(a.coreId as string),
  core_settings: (a) => (a.coreId === 'Spiritualized.GBA'
    ? { values: [{ id: 1, name: 'High Quality Audio', kind: 'check', value: 0, display: 'Off', default_display: 'On', known: true }, { id: 2, name: 'Rumble Strength', kind: 'slider', value: 6, display: '6', default_display: '5', known: true }, { id: 3, name: 'Edge Masking', kind: 'list', value: 3, display: 'Auto Masking', default_display: 'Auto Masking', known: true }], files: [{ group: 'Interact', rel_path: 'Settings/Spiritualized.GBA/Interact/_core/interact_persist.json', bytes: 210, modified: 1767225600 }, { group: 'Input', rel_path: 'Settings/Spiritualized.GBA/Input/_core/input_persist.json', bytes: 540, modified: 1767225600 }], presets: 12, presets_folder: 'Presets/Spiritualized.GBA', unreadable: null }
    : { values: [], files: [], presets: 0, presets_folder: `Presets/${a.coreId}`, unreadable: null }),
  core_screenshots: (a) => a.coreId !== 'Spiritualized.GBA' ? ({ shots: [], total: 489, recorded: 0, likely: 0, with_meta: 489, states: 0, window_secs: 600 }) : ({ shots: SHOTS, total: SHOTS.length + 465, recorded: 5, likely: 2, with_meta: 20, states: 6, window_secs: 600 }),
  screenshot_thumbnail: (a) => thumb(a.file as string),
  manual_classify: (a) => { const t = String(a.input ?? '').trim();
    if (t.startsWith('http://')) return { kind: '', ok: false, reason: 'Use the https:// address.' };
    if (t.startsWith('https://')) {
      if (/^https:\/\/github\.com\/[\w-]+\/[\w.-]+(\/releases(\/(latest|tag\/.+|download\/.+\.zip))?)?\/?$/.test(t)) return { kind: t.includes('/releases/download/') ? 'github-file' : t.includes('/releases/tag/') ? 'github-release' : 'github-repo', ok: true, reason: '' };
      return { kind: '', ok: false, reason: t.startsWith('https://github.com') ? 'That is a page inside the repository. Paste the repository address or a release address.' : 'Only github.com addresses are accepted. Download the zip yourself and add it as a file.' };
    }
    if (/bad/.test(t)) return { kind: '', ok: false, reason: 'That is not a file or folder on this computer, and not a web address.' };
    return /\.zip$/i.test(t) ? { kind: 'zip', ok: true, reason: '' } : { kind: 'folder', ok: true, reason: '' }; },
  manual_inspect: (a) => { const g = a.args as { input: string; tag?: string; asset?: string; readme: boolean; card?: string }; const t = g.input;
    if (/tau/i.test(t)) throw { code: 55, message: 'This is a Tau core. Tau cores are installed and updated from Packages and the Tau update bar, not from here.' };
    if (/refused/.test(t)) throw { code: 55, message: 'The zip carries user data (Settings, Saves or Memories). A core package must not.' };
    const github = t.startsWith('https://');
    if (/multi/.test(t) && !g.asset) return { status: 'choose', owner: 'someone', repo: 'multi-core', tag: g.tag ?? 'v2.0', releases: [{ tag: 'v2.0', name: 'Version 2.0', published: '2026-09-30', prerelease: false, zips: ['Some.Core_2.0.zip', 'Some.Core_2.0_dock.zip'] }, { tag: 'v1.9', name: 'Version 1.9', published: '2026-08-01', prerelease: true, zips: ['Some.Core_1.9.zip'] }] };
    return { status: 'ready', zip_path: `/cache/${t.replace(/[^a-z0-9]+/gi, '-').replace(/^-|-$/g, '').slice(-40) || 'unlisted'}.zip`, lint: /badlint/.test(t) ? { core_id: 'someone.Unlisted', errors: 1, warnings: 1, notes: 0, findings: [{ id: 'L09', severity: 'Error', message: 'The bitstream file bitstream.rbf_r is missing or empty.', rule: 'core.json cores[].filename names the bitstream', fix: 'Add the converted bitstream next to core.json.' }, { id: 'L05', severity: 'Warning', message: 'The description is 71 characters; the documented limit is 63.', rule: 'Analogue core.json: description at most 63 characters', fix: 'Shorten the description.' }] } : { core_id: 'someone.Unlisted', errors: 0, warnings: /warnlint/.test(t) ? 1 : 0, notes: 0, findings: /warnlint/.test(t) ? [{ id: 'L05', severity: 'Warning', message: 'The description is 71 characters; the documented limit is 63.', rule: 'Analogue core.json: description at most 63 characters', fix: 'Shorten the description.' }] : [] }, in_library: /library/.test(t), installed_version: /installed/.test(t) ? '0.9.0' : null, different_source: /other/.test(t),
      provenance: github ? { kind: 'github', owner: 'someone', repo: 'unlisted-core', tag: g.tag ?? 'v1.0', asset: g.asset ?? 'Some.Core_1.0.zip', bytes: 3.2 * MB, sha256: 'c'.repeat(64), checksum: /nosum/.test(t) ? 'NotPublished' : 'Verified', packed: null } : { kind: /\.zip$/i.test(t) ? 'zip' : 'folder', owner: '', repo: '', tag: '', asset: '', bytes: 0, sha256: '', checksum: 'Local', packed: /\.zip$/i.test(t) ? null : 'BareCore' },
      readme: github && g.readme ? { text: '# Unlisted core\nA homebrew system for the Pocket. Put your BIOS in the common folder.', images: [thumb('shot-1'), thumb('shot-2')] } : null,
      preview: { core_id: 'someone.Unlisted', shortname: 'Unlisted', author: 'someone', description: 'A homebrew system for the Pocket.', url: 'https://github.com/someone/unlisted-core', version: '1.0.0', date_release: '2026-09-30', firmware_required: '2.1', dock_supported: true, link_port: false, sleep_supported: true,
        platforms: [{ id: 'unlisted', name: 'Unlisted Computer', category: 'Computer', manufacturer: 'Somebody Ltd.', in_package: true }],
        needs: [{ slot: 'BIOS', filename: 'unlisted.rom', extensions: ['rom'], required: true, size_exact: 16384, size_maximum: null, core_specific: false, folder_rel: 'Assets/unlisted/common', in_package: false, you_supply: true, purpose: 'Slot "BIOS": required, exactly 16 KB.' }, { slot: 'Cartridge', filename: '', extensions: ['bin', 'rom'], required: true, size_exact: null, size_maximum: 4194304, core_specific: false, folder_rel: 'Assets/unlisted/common', in_package: false, you_supply: false, purpose: 'Read by the slot "Cartridge" (required, up to 4.0 MB).' }, { slot: 'Palette', filename: 'pal.bin', extensions: ['bin'], required: false, size_exact: 192, size_maximum: null, core_specific: true, folder_rel: 'Assets/unlisted/someone.Unlisted', in_package: true, you_supply: false, purpose: 'Colour palette. Slot "Palette": optional, exactly 192 bytes, only this core reads it.' }],
        settings: ['Scanlines', 'Region', 'Overscan'], info_text: 'Unlisted: a homebrew system.\nPorted by someone.', has_bitstream: true, sha256: 'd'.repeat(64), zip_bytes: 3.2 * MB, files: 12, uncompressed_bytes: 5 * MB, asset_files: 1, warnings: /nopic/.test(t) ? ['No valid icon.bin (36x36): the core will have no picture in the Pocket\'s menu.'] : [], icon: github || true ? thumb('icon') : null, banner: thumb('banner') } }; },
  manual_register: () => null,
  plan_core_add: (a) => addPlanFor(a),
  plan_core_remove: (a) => { const files = (a.files as string[]) ?? []; return { id: 'rm' + files.join('|'), core_id: a.coreId, items: files.map((f) => ({ rel_path: f, bytes: 8 * MB, shared_with: f.includes('/common/') ? ['other.GBAlt'] : [], needed_by: f.endsWith('gba_bios.bin') ? 'BIOS' : null })), total_bytes: files.length * 8 * MB, warnings: files.some((f) => f.endsWith('gba_bios.bin')) ? ['gba_bios.bin is a file this core needs to start.'] : [], blocked: files.length ? null : 'Nothing is selected.' }; },
  execute_core_remove: (a) => { const files = (a.files as string[]) ?? []; for (const f of files) { const nm = f.split('/').pop()!; if (nm === 'gba_bios.bin') mockBios = false; else mockGone.push(nm); } return { removed: files.length, bytes_removed: files.length * 8 * MB, backup_dir: '/Backups/omega/abc123' }; },
  execute_core_add: (a) => { const plan = addPlanFor(a); for (const i of plan.items) if (i.state === 'New') { const nm = i.source.split('/').pop()!; if (nm === 'gba_bios.bin') mockBios = true; else mockAdded.push(nm); } return { written: plan.new_files, replaced: 0, skipped_identical: plan.identical, skipped_different: plan.different, refused: plan.refused, bytes_written: plan.bytes_to_write, backup_dir: null }; },
  update_check: () => null,
  update_download: () => [],
  persist_names: () => ({ 16: 'Halcyon EQ preset' }),
  skip_tracks: () => mockSkip.map((t) => ({ rel: t.rel, dir: t.rel.split('/').slice(0, -1).join('/'), file: t.rel.split('/').pop(), secs: 200, rate: 44100, skip: t.edit?.never ? { lead_skip: 0, tail_skip: 0 } : { lead_skip: t.edit?.lead ?? t.auto[0], tail_skip: t.edit?.tail ?? t.auto[1] }, edit: t.edit })),
  skip_detail: (a) => { const t = mockSkip.find((x) => x.rel === a.rel)!; const frames = 200 * 44100; return { rate: 44100, frames, envelope: Array.from({ length: 400 }, (_, i) => (i < 40 || i > 360 ? 0 : 120 + ((i * 37) % 100))), auto: { lead_skip: t.auto[0], tail_skip: t.auto[1] }, skip: t.edit?.never ? { lead_skip: 0, tail_skip: 0 } : { lead_skip: t.edit?.lead ?? t.auto[0], tail_skip: t.edit?.tail ?? t.auto[1] }, edit: t.edit }; },
  skip_set_edit: (a) => { const t = mockSkip.find((x) => x.rel === a.rel)!; t.edit = a.edit ?? null; },
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
    lastRequest: () => lastRequest,
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
  // The mock fakes Tauri's IPC, so `__TAURI_INTERNALS__` exists; this flag tells the drag-and-drop helper to use the browser's own events.
  (window as unknown as { __TAU_MOCK__?: boolean }).__TAU_MOCK__ = true;
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
