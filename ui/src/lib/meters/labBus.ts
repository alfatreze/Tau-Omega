// The preset editor and Tau Alpha's lab page talk through these stores (lab embed contract 1: docs/interop/ and the pinned drop's
// LAB_EMBED_CONTRACT.md). The editor says which meter and which values the lab should show; the lab says which modules it built, what it did
// with a set, which preview options apply to the meter, what the meter costs and how big its view is.
import { writable } from 'svelte/store';

export type LabTarget = { meter: number; values: Record<string, number> } | null;
export type LabModule = { id: number; key: string; wasm_sha256: string; wasm_sha256_timed?: string };
export type LabApplied = { meter: number; ignored: string[]; clamped: Record<string, number>; values?: Record<string, number>; error?: string };

/** What the editor wants shown (null: nothing selected). */
export const labTarget = writable<LabTarget>(null);
/** What the lab reported. `contract` is the embed contract number the lab page speaks (null until it is ready). */
export const labStatus = writable<{ ready: boolean; modules: LabModule[]; applied: LabApplied | null; contract: number | null }>({ ready: false, modules: [], applied: null, contract: null });

/** One of the lab's preview options for the meter on show (`tau-lab-options`). An enum's value is an index into its choices. */
export type LabOption =
  | { id: string; label: string; type: 'enum'; value: number; choices: string[] }
  | { id: string; label: string; type: 'bool'; value: boolean }
  | { id: string; label: string; type: 'range'; value: number; min: number; max: number };

/** The lab's cost summary for the meter on show (`tau-lab-info`): the figures cover the last 120 frames, about three seconds. */
export type LabCost = {
  mean: number; max: number; budget: number; class: number | null; over: boolean; verdict: string; basis: string; vs_bars: number;
  cell_width: number; cpu_low: number; cpu_high: number; engine_kpx: number;
};
export type LabInfo = {
  cost: LabCost;
  /** What each option of the meter costs: one row per option, one cell per choice. */
  table: { key: string; label: string; options: { value: string | number; selected: boolean; cpu_low: number; cpu_high: number; delta: number; kops: number }[] }[];
};

/** The options the lab offers for the meter it shows; empty until the lab is ready. */
export const labOptions = writable<LabOption[]>([]);
/** Sends a message to the lab frame (set by the preview component; null while there is no frame). */
export const labSend = writable<((message: Record<string, unknown>) => void) | null>(null);

/** Which view of the meter the preview shows: the small view (the meter's band of the Now Playing screen) or the full-screen view. */
export type PreviewView = 'small' | 'full';
export const previewView = writable<PreviewView>('small');
