import { writable, derived } from 'svelte/store';
import { coresTree } from './tau-api';
import type { PlatformNode, CoreRow } from './types';

/** What the Cores section is showing: a platform (all its cores), or one core. */
export type CoresSelection = { platform: string; core: string };

export const coresTreeStore = writable<PlatformNode[]>([]);
export const coresSelection = writable<CoresSelection>({ platform: '', core: '' });
export const coresLoading = writable(false);
export const coresError = writable('');

let inspectorStart = true;
try { inspectorStart = localStorage.getItem('tau.cores.inspector') !== '0'; } catch { /* per-viewer convenience */ }
export const inspectorOpen = writable(inspectorStart);
inspectorOpen.subscribe((v) => { try { localStorage.setItem('tau.cores.inspector', v ? '1' : '0'); } catch { /* ignore */ } });

/** How many installed cores need something (a missing BIOS): the dot on the menu entry. */
export const coresAttention = derived(coresTreeStore, (t) => t.reduce((n, p) => n + p.cores.filter((c) => c.missing > 0).length, 0));
export const coresTotal = derived(coresTreeStore, (t) => t.reduce((n, p) => n + p.cores.length, 0));

export async function refreshCores(card: string): Promise<void> {
  if (!card) { coresTreeStore.set([]); return; }
  coresLoading.set(true); coresError.set('');
  try { coresTreeStore.set(await coresTree(card)); }
  catch (e) { coresError.set(e && typeof e === 'object' && 'message' in e ? String((e as { message: unknown }).message) : String(e)); coresTreeStore.set([]); }
  finally { coresLoading.set(false); }
}

/** The Library's grouping: category > manufacturer > platform > core. The tree from the engine is already in this order. */
export type MakerGroup = { name: string; platforms: PlatformNode[]; cores: number };
export type CategoryGroup = { name: string; makers: MakerGroup[]; platforms: number; cores: number };
export function groupTree(tree: PlatformNode[]): CategoryGroup[] {
  const out: CategoryGroup[] = [];
  for (const n of tree) {
    const cat = n.platform.id === '_none' ? 'No platform' : n.platform.category || 'Other';
    const maker = n.platform.id === '_none' ? '' : n.platform.manufacturer || 'Unknown';
    let c = out.find((x) => x.name === cat); if (!c) { c = { name: cat, makers: [], platforms: 0, cores: 0 }; out.push(c); }
    let m = c.makers.find((x) => x.name === maker); if (!m) { m = { name: maker, platforms: [], cores: 0 }; c.makers.push(m); }
    m.platforms.push(n); m.cores += n.cores.length; c.platforms++; c.cores += n.cores.length;
  }
  return out;
}
export const coresGroups = derived(coresTreeStore, groupTree);

export function findCore(tree: PlatformNode[], id: string): { node: PlatformNode; core: CoreRow } | null {
  for (const node of tree) { const core = node.cores.find((c) => c.id === id); if (core) return { node, core }; }
  return null;
}
