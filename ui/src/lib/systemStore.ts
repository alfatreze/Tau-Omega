// Global per-system state (`Analogue Pocket`, `Game Bub`, and future handhelds).
// Shared by Marketplace, Firmware, and Card inspection so switching systems anywhere stays in sync.
import { derived, writable } from 'svelte/store';
import { cardDetectDevice, systemsList } from './tau-api';
import type { CardDeviceInfo, DeviceKind, SystemProfile } from './types';

export const DEFAULT_SYSTEMS: SystemProfile[] = [
  {
    id: 'analogue_pocket',
    name: 'Analogue Pocket',
    short_name: 'Pocket',
    beta: false,
    firmware_method: 'sd_card_root',
    supports_music_library: true,
  },
  {
    id: 'game_bub',
    name: 'Game Bub',
    short_name: 'Game Bub',
    beta: true,
    firmware_method: 'usb_uf2',
    supports_music_library: false,
  },
];

function initialSystem(): DeviceKind {
  try {
    const saved = localStorage.getItem('tau.system');
    if (saved === 'analogue_pocket' || saved === 'game_bub') return saved;
  } catch { /* ignore */ }
  return 'analogue_pocket';
}

export const systemsStore = writable<SystemProfile[]>(DEFAULT_SYSTEMS);
export const activeSystem = writable<DeviceKind>(initialSystem());
export const cardDevice = writable<CardDeviceInfo | null>(null);

export const activeSystemProfile = derived(
  [systemsStore, activeSystem],
  ([$systems, $active]) => $systems.find((s) => s.id === $active) ?? DEFAULT_SYSTEMS[0],
);

export function setActiveSystem(id: DeviceKind) {
  activeSystem.set(id);
  try { localStorage.setItem('tau.system', id); } catch { /* ignore */ }
}

let loadedSystems = false;
export function loadSystems() {
  if (loadedSystems) return;
  loadedSystems = true;
  systemsList().then((list) => {
    if (list.length) systemsStore.set(list);
  }, () => {});
}

let syncedFor = '';
export async function syncSystemForCard(card: string) {
  if (!card) { syncedFor = ''; cardDevice.set(null); return; }
  if (syncedFor === card) return;
  syncedFor = card;
  try {
    const info = await cardDetectDevice(card);
    if (syncedFor !== card) return;
    cardDevice.set(info);
    if (info.device) setActiveSystem(info.device);
  } catch {
    if (syncedFor === card) cardDevice.set(null);
  }
}
