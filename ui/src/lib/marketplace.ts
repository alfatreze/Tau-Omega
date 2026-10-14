/** Pure helpers for the Marketplace page (kept out of the component so they can be checked on their own). */
import type { CatalogCore, CoreStatus, CoreStatusKind, MarketSource } from './types';

export type Sort = 'newest' | 'name';
export type Filters = { query: string; category: string; licence: boolean; installedOnly: boolean; sort: Sort };

/** Every word of the query must appear in the id, name, author, manufacturer or category (same rule as the engine's `Catalog::search`). */
export function matches(core: CatalogCore, f: Filters): boolean {
  const hay = `${core.id} ${core.name} ${core.author} ${core.manufacturer} ${core.category}`.toLowerCase();
  return f.query.toLowerCase().split(/\s+/).filter(Boolean).every((w) => hay.includes(w))
    && (!f.category || core.category.toLowerCase() === f.category.toLowerCase())
    && (!f.licence || core.requires_license);
}

export function filterCores(cores: CatalogCore[], f: Filters, installed: Set<string>): CatalogCore[] {
  const out = cores.filter((c) => matches(c, f) && (!f.installedOnly || installed.has(c.id.toLowerCase())));
  return f.sort === 'name'
    ? out.sort((a, b) => a.name.localeCompare(b.name))
    : out.sort((a, b) => b.release_date.localeCompare(a.release_date) || a.name.localeCompare(b.name));
}

export const categories = (cores: CatalogCore[]): string[] => [...new Set(cores.map((c) => c.category).filter(Boolean))].sort();

/** The status chip text for an installed core, or null when nothing is worth saying. Colour is never the only signal. */
export function statusLabel(s: CoreStatusKind | undefined): string | null {
  if (!s) return null;
  if (typeof s === 'object') return 'Replaces an older core';
  return ({ UpToDate: 'Installed', UpdateAvailable: 'Update available', NewerThanCatalog: 'Installed (newer)', Unknown: 'Installed', NotInCatalog: null, Excluded: null } as const)[s];
}

export function statusMap(rows: CoreStatus[]): Map<string, CoreStatus> {
  return new Map(rows.map((r) => [r.id.toLowerCase(), r]));
}

/** What the user is told about where the list came from; null when it is fresh enough to say nothing. */
export function sourceNote(source: MarketSource, fetched: number, now = Date.now() / 1000): string | null {
  const ago = (secs: number) => (secs < 90 ? 'just now' : secs < 5400 ? `${Math.round(secs / 60)} min ago` : secs < 172800 ? `${Math.round(secs / 3600)} h ago` : `${Math.round(secs / 86400)} days ago`);
  const age = ago(Math.max(0, now - fetched));
  if (typeof source === 'object') return `Showing the saved list from ${age}. ${source.Offline}`;
  return `Updated ${age}`;
}

export const size = (bytes: number) => (bytes < 1024 ? `${bytes} B` : bytes < 1048576 ? `${(bytes / 1024).toFixed(0)} KB` : bytes < 1073741824 ? `${(bytes / 1048576).toFixed(1)} MB` : `${(bytes / 1073741824).toFixed(2)} GB`);
