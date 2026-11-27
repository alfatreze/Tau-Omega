import { get, writable } from 'svelte/store';
import { gameArtResolveBatch, gameMetaResolveBatch } from './tau-api';
import type { GameArtResult, GameMeta } from './types';

export type CoreFilesViewMode = 'list' | 'grid';

const VIEW_MODE_KEY = 'tau.coreFiles.viewMode';

function readInitialViewMode(): CoreFilesViewMode {
  if (typeof localStorage === 'undefined') return 'list';
  return localStorage.getItem(VIEW_MODE_KEY) === 'grid' ? 'grid' : 'list';
}

/** Shared List / Grid preference across Pocket and Game Bub core file browsers (defaults to 'list'). */
export const coreFilesViewMode = writable<CoreFilesViewMode>(readInitialViewMode());
coreFilesViewMode.subscribe((mode) => {
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(VIEW_MODE_KEY, mode);
  }
});

/** Strip filename path and short alphanumeric extension for clean game card display. */
export function displayGameTitle(romName: string): string {
  const base = (romName.split(/[/\\]/).pop() ?? romName).trim();
  const dot = base.lastIndexOf('.');
  if (dot > 0) {
    const ext = base.slice(dot + 1);
    if (ext.length >= 1 && ext.length <= 8 && /^[a-z0-9]+$/i.test(ext)) {
      return base.slice(0, dot).trim();
    }
  }
  return base;
}

export function gameArtCacheKey(platformId: string, romName: string): string {
  return `${platformId.trim().toLowerCase()}::${romName.trim()}`;
}

/** Resolved per-game artwork keyed by `gameArtCacheKey(platformId, romName)`. `null` means looked up and not found. */
export const gameArtMap = writable<Record<string, GameArtResult | null>>({});

/** Resolved per-game metadata/genres keyed by `gameArtCacheKey(platformId, romName)`. */
export const gameMetaMap = writable<Record<string, GameMeta>>({});
const metaInFlight = new Set<string>();

/** Fetches genre/region metadata for all ROMs in a folder in one batched call. */
export async function requestGameGenres(platformId: string, romNames: string[]) {
  const plat = platformId.trim().toLowerCase();
  if (!plat || plat === 'tau' || plat === 'gamebub' || !romNames.length) return;
  const cur = get(gameMetaMap);
  const missing: string[] = [];
  for (const raw of romNames) {
    const rom = raw.trim();
    if (!rom) continue;
    const key = gameArtCacheKey(plat, rom);
    if (key in cur || metaInFlight.has(key)) continue;
    metaInFlight.add(key);
    missing.push(rom);
  }
  if (!missing.length) return;
  try {
    const items = await gameMetaResolveBatch(plat, missing);
    const byRom = new Map<string, GameMeta>();
    for (const item of items) byRom.set(item.rom_name, item);
    gameMetaMap.update((state) => {
      const next = { ...state };
      for (const rom of missing) {
        const key = gameArtCacheKey(plat, rom);
        next[key] = byRom.get(rom) ?? {
          rom_name: rom,
          platform_id: plat,
          genre: null,
          developer: null,
          publisher: null,
          release_year: null,
          region: null,
          matched_name: null,
        };
      }
      return next;
    });
  } catch {
    // Ignore transient network errors; next view can retry.
  } finally {
    for (const rom of missing) {
      metaInFlight.delete(gameArtCacheKey(plat, rom));
    }
  }
}

const inFlight = new Set<string>();
const pendingByPlatform = new Map<string, Set<string>>();
let flushTimer: ReturnType<typeof setTimeout> | null = null;

function isArtLookupEligible(platformId: string, romName: string): boolean {
  const p = platformId.trim().toLowerCase();
  const r = romName.trim();
  if (!p || !r || p === 'tau' || p === 'gamebub') return false;
  return true;
}

async function flushQueue() {
  flushTimer = null;
  const batches: [string, string[]][] = [];
  for (const [plat, names] of pendingByPlatform.entries()) {
    const list = [...names];
    if (!list.length) continue;
    for (let i = 0; i < list.length; i += 12) {
      batches.push([plat, list.slice(i, i + 12)]);
    }
  }
  pendingByPlatform.clear();

  for (const [plat, romNames] of batches) {
    try {
      const results = await gameArtResolveBatch(plat, romNames, 'boxart', false);
      const hitMap = new Map<string, GameArtResult>();
      for (const res of results) {
        hitMap.set(res.rom_name, res);
      }
      gameArtMap.update((cur) => {
        const next = { ...cur };
        for (const rom of romNames) {
          const k = gameArtCacheKey(plat, rom);
          next[k] = hitMap.get(rom) ?? null;
        }
        return next;
      });
    } catch {
      // Leave unpopulated on error so future visits can retry once online.
    } finally {
      for (const rom of romNames) {
        inFlight.delete(gameArtCacheKey(plat, rom));
      }
    }
  }
}

/** Queues a game ROM for lazy batched Libretro/override resolution. */
export function requestGameArt(platformId: string, romName: string, force = false) {
  if (!isArtLookupEligible(platformId, romName)) return;
  const plat = platformId.trim().toLowerCase();
  const rom = romName.trim();
  const key = gameArtCacheKey(plat, rom);
  const current = get(gameArtMap);
  if (!force && (key in current || inFlight.has(key))) return;

  inFlight.add(key);
  let set = pendingByPlatform.get(plat);
  if (!set) {
    set = new Set<string>();
    pendingByPlatform.set(plat, set);
  }
  set.add(rom);

  if (flushTimer === null) {
    flushTimer = setTimeout(() => void flushQueue(), 40);
  }
}

export function setGameArtEntry(platformId: string, romName: string, result: GameArtResult) {
  const key = gameArtCacheKey(platformId, romName);
  gameArtMap.update((cur) => ({ ...cur, [key]: result }));
  if (typeof window !== 'undefined') {
    window.dispatchEvent(
      new CustomEvent('tau-game-art-changed', { detail: { platformId, romName } }),
    );
  }
}

export function clearGameArtEntry(platformId: string, romName: string) {
  const key = gameArtCacheKey(platformId, romName);
  gameArtMap.update((cur) => {
    const next = { ...cur };
    delete next[key];
    return next;
  });
  inFlight.delete(key);
  if (typeof window !== 'undefined') {
    window.dispatchEvent(
      new CustomEvent('tau-game-art-changed', { detail: { platformId, romName, reset: true } }),
    );
  }
}

/** Svelte action that triggers `requestGameArt` when a ROM row or grid card nears the viewport. */
export function lazyGameArt(
  node: HTMLElement,
  params: { platformId: string; romName: string },
) {
  let cur = params;
  let observer: IntersectionObserver | null = null;

  const setup = () => {
    observer?.disconnect();
    observer = null;
    if (!isArtLookupEligible(cur.platformId, cur.romName)) return;
    if (typeof IntersectionObserver === 'undefined') {
      requestGameArt(cur.platformId, cur.romName);
      return;
    }
    observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          requestGameArt(cur.platformId, cur.romName);
          observer?.disconnect();
          observer = null;
        }
      },
      { rootMargin: '240px' },
    );
    observer.observe(node);
  };

  setup();
  return {
    update(next: { platformId: string; romName: string }) {
      if (next.platformId !== cur.platformId || next.romName !== cur.romName) {
        cur = next;
        setup();
      }
    },
    destroy() {
      observer?.disconnect();
    },
  };
}
