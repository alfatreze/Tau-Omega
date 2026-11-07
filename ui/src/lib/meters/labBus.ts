// The preset editor and Tau Alpha's lab page talk through this store (interop A011/1: `tau-lab-set`, `tau-lab-ready`, `tau-lab-applied`).
// The editor says which meter and which values the lab should show; the lab frame says which modules it built and what it did with a set.
import { writable } from 'svelte/store';

export type LabTarget = { meter: number; values: Record<string, number> } | null;
export type LabModule = { id: number; key: string; wasm_sha256: string; wasm_sha256_timed?: string };
export type LabApplied = { meter: number; ignored: string[]; clamped: Record<string, number>; values?: Record<string, number>; error?: string };

/** What the editor wants shown (null: nothing selected). */
export const labTarget = writable<LabTarget>(null);
/** What the lab reported. */
export const labStatus = writable<{ ready: boolean; modules: LabModule[]; applied: LabApplied | null }>({ ready: false, modules: [], applied: null });
