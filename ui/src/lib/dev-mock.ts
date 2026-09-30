/** Browser-only stand-in for the Tauri backend, so the UI can be run and
 * screenshotted with `npm run dev` without the desktop shell or a real card.
 * Installed by main.ts only in dev builds when Tauri is absent; never shipped. */
import { mockIPC } from '@tauri-apps/api/mocks';

const CARD = '/mock/Pocket';
const cores = [
  { id: 'tau.omega', author: 'tau', shortname: 'omega', version: '0.3.0', platform: 'tau', platform_category: 'Media Players', library_capable: true, index_status: 'ok', tracks: 7180 },
  { id: 'tau.player2', author: 'tau', shortname: 'player2', version: '0.2.1', platform: 'tau', platform_category: 'Media Players', library_capable: true, index_status: 'missing', tracks: null },
  { id: 'agg23.SNES', author: 'agg23', shortname: 'SNES', version: '1.4.0', platform: 'snes', platform_category: 'Console', library_capable: false, index_status: 'n/a', tracks: null },
];
const tracks = Array.from({ length: 60 }, (_, i) => ({ rel: `Music/Artist ${i % 6}/Album ${i % 4}/${String(i + 1).padStart(2, '0')} Track ${i + 1}.${i % 3 ? 'mp3' : 'flac'}`, title: `Track ${i + 1}`, artist: `Artist ${i % 6}`, album: `Album ${i % 4}`, secs: 150 + i * 3, format: i % 3 ? 'mp3' : 'flac' }));
const plan = { id: 'mock-plan-1', new_files: 42, updates: 5, unchanged: 7133, bytes_to_write: 412_000_000, art_sidecars: 3, art_sidecar_previews: [], warnings: [] };
const report = { plan_id: 'mock-plan-1', copied: 47, deleted: 0, verified: 47, skipped: 0, errors: [] };

const handlers: Record<string, (args: any) => unknown> = {
  get_reports_dir: () => null,
  get_recent_cards: () => [CARD],
  list_mounted_cards: () => [CARD],
  get_manual_players: () => [],
  inspect_card: () => cores,
  read_persisted_settings: () => [],
  read_check_summary: () => null,
  read_qr_report: () => null,
  list_screenshots: () => [],
  read_core_icon: () => null,
  read_platform_image: () => null,
  read_image_data_url: () => '',
  list_journals: () => [],
  scan_library: () => ({ tracks, playlists: [{ name: 'Favourites', tracks: 12 }], warnings: [] }),
  scan_media: () => ({ playlists: [{ name: 'Favourites', file: 'Favourites.m3u8', tracks: tracks.slice(0, 12).map((t) => t.rel) }], warnings: [] }),
  find_problems: () => [{ kind: 'missing_tag', files: [tracks[3].rel], message: 'No artist tag' }],
  check_storage_capacity: () => ({ free_bytes: 20e9, needed_bytes: 412e6, fits: true }),
  plan_sync: () => plan,
  execute_sync: () => report,
  compare_media: () => ({ left: '/mock/a', right: '/mock/b', only_left: 2, only_right: 1, different: 1, identical: 10, differences: [{ relative: 'Music/x.mp3', state: 'only_left', left_bytes: 4e6, right_bytes: null }] }),
  cancel_job: () => null,
  set_manual_player: () => null,
  record_recent_card: () => null,
  set_reports_dir: () => null,
};

export function installDevMock() {
  mockIPC((cmd, args) => {
    if (cmd.startsWith('plugin:dialog|')) return '/mock/picked';
    const h = handlers[cmd];
    if (!h) { console.warn(`[dev-mock] unmocked command: ${cmd}`); throw { code: 'E_MOCK', message: `No mock for ${cmd}` }; }
    return h(args);
  }, { shouldMockEvents: true });
}
