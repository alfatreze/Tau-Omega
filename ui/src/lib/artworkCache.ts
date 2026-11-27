import type { ArtworkAlternative, ArtworkSample } from './types';

// In-memory session cache so reopening the Pack Browser or per-core artwork picker is instant.
const packSamplesCache = new Map<string, ArtworkSample[]>();
const alternativesCache = new Map<string, ArtworkAlternative[]>();

export function getCachedPackSamples(packId: string, card: string): ArtworkSample[] | null {
  return packSamplesCache.get(`${packId}::${card}`) ?? null;
}

export function setCachedPackSamples(packId: string, card: string, items: ArtworkSample[]) {
  packSamplesCache.set(`${packId}::${card}`, items);
}

export function invalidatePackSamples(packId?: string) {
  if (!packId) {
    packSamplesCache.clear();
    return;
  }
  const prefix = `${packId}::`;
  for (const key of packSamplesCache.keys()) {
    if (key.startsWith(prefix)) packSamplesCache.delete(key);
  }
}

export function getCachedAlternatives(platformId: string): ArtworkAlternative[] | null {
  return alternativesCache.get(platformId.trim().toLowerCase()) ?? null;
}

export function setCachedAlternatives(platformId: string, items: ArtworkAlternative[]) {
  alternativesCache.set(platformId.trim().toLowerCase(), items);
}

export function invalidateAllArtworkCache() {
  packSamplesCache.clear();
  alternativesCache.clear();
}
