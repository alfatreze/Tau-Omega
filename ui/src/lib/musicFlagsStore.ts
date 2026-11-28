import { derived, writable } from 'svelte/store';
import type { Core } from './types';

export type MusicCoreMode = 'auto' | 'on' | 'off';
export type MusicCoreFlagsConfig = {
  tauAlpha: MusicCoreMode;
  harpmudd: MusicCoreMode;
};

export type MusicFeatures = {
  tauAlpha: boolean;
  harpmudd: boolean;
  /** True when at least one supported music core (Tau Alpha or HarpMudd) is active. */
  music: boolean;
  /** Playlist editor, Meter Lab, Pocket Appearance, and Halcyon EQ belong to Tau Alpha only. */
  playlists: boolean;
  meters: boolean;
  appearance: boolean;
  halcyon: boolean;
  detectedTauAlpha: boolean;
  detectedHarpMudd: boolean;
};

const STORAGE_KEY = 'tau.musicFlags';
export const DEFAULT_MUSIC_FLAGS: MusicCoreFlagsConfig = {
  tauAlpha: 'auto',
  harpmudd: 'auto',
};

function normalizeMode(v: unknown): MusicCoreMode {
  return v === 'on' || v === 'off' || v === 'auto' ? v : 'auto';
}

function loadInitial(): MusicCoreFlagsConfig {
  if (typeof localStorage === 'undefined') return { ...DEFAULT_MUSIC_FLAGS };
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_MUSIC_FLAGS };
    const parsed = JSON.parse(raw) as Partial<MusicCoreFlagsConfig> | null;
    return {
      tauAlpha: normalizeMode(parsed?.tauAlpha),
      harpmudd: normalizeMode(parsed?.harpmudd),
    };
  } catch {
    return { ...DEFAULT_MUSIC_FLAGS };
  }
}

export const musicFlagsConfig = writable<MusicCoreFlagsConfig>(loadInitial());

musicFlagsConfig.subscribe((cfg) => {
  if (typeof localStorage === 'undefined') return;
  try {
    if (cfg.tauAlpha === 'auto' && cfg.harpmudd === 'auto') {
      localStorage.removeItem(STORAGE_KEY);
    } else {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(cfg));
    }
  } catch {
    /* ignore storage errors */
  }
});

export function setTauAlphaMode(mode: MusicCoreMode): void {
  musicFlagsConfig.update((c) => ({ ...c, tauAlpha: normalizeMode(mode) }));
}

export function setHarpMuddMode(mode: MusicCoreMode): void {
  musicFlagsConfig.update((c) => ({ ...c, harpmudd: normalizeMode(mode) }));
}

export function resetMusicFlags(): void {
  musicFlagsConfig.set({ ...DEFAULT_MUSIC_FLAGS });
}

/** Identifies Tau Alpha family music cores on a card. */
export function isTauAlphaCore(
  c: Pick<Core, 'id' | 'author' | 'platform' | 'media_platform' | 'library_capable'>,
): boolean {
  if (c.library_capable) return true;
  const id = (c.id || '').toLowerCase();
  const plat = (c.platform || '').toLowerCase();
  const mediaPlat = (c.media_platform || '').toLowerCase();
  return (
    /^(alfatreze\.)?tau($|[ ._])/.test(id) ||
    id.startsWith('tau.') ||
    plat === 'tau' ||
    mediaPlat === 'tau'
  );
}

/** Identifies HarpMudd Mp3Player music cores without matching unrelated HarpMudd arcade cores. */
export function isHarpMuddCore(
  c: Pick<Core, 'id' | 'author' | 'platform' | 'media_platform' | 'library_capable' | 'playlist'>,
): boolean {
  if (c.library_capable) return false;
  const id = (c.id || '').toLowerCase();
  const author = (c.author || '').toLowerCase();
  const plat = (c.platform || '').toLowerCase();
  const mediaPlat = (c.media_platform || '').toLowerCase();
  if (id.startsWith('harpmudd.mp3player')) return true;
  if (plat === 'mp3player' || mediaPlat === 'mp3player') return true;
  return author === 'harpmudd' && !!c.playlist;
}

export const detectedMusicCores = writable<{ tauAlpha: boolean; harpmudd: boolean }>({
  tauAlpha: false,
  harpmudd: false,
});

export function syncDetectedMusicCores(cores: Core[]): void {
  detectedMusicCores.set({
    tauAlpha: cores.some(isTauAlphaCore),
    harpmudd: cores.some(isHarpMuddCore),
  });
}

function resolveFlag(mode: MusicCoreMode, detected: boolean): boolean {
  if (mode === 'on') return true;
  if (mode === 'off') return false;
  return detected;
}

export const musicFeatures = derived(
  [musicFlagsConfig, detectedMusicCores],
  ([$cfg, $det]): MusicFeatures => {
    const tauAlpha = resolveFlag($cfg.tauAlpha, $det.tauAlpha);
    const harpmudd = resolveFlag($cfg.harpmudd, $det.harpmudd);
    return {
      tauAlpha,
      harpmudd,
      music: tauAlpha || harpmudd,
      playlists: tauAlpha,
      meters: tauAlpha,
      appearance: tauAlpha,
      halcyon: tauAlpha,
      detectedTauAlpha: $det.tauAlpha,
      detectedHarpMudd: $det.harpmudd,
    };
  },
);

export function isMusicCoreAllowed(
  c: Pick<Core, 'id' | 'author' | 'platform' | 'media_platform' | 'library_capable' | 'playlist'>,
  features: Pick<MusicFeatures, 'tauAlpha' | 'harpmudd'>,
): boolean {
  if (isTauAlphaCore(c)) return features.tauAlpha;
  if (isHarpMuddCore(c)) return features.harpmudd;
  return features.tauAlpha || features.harpmudd;
}
