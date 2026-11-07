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

/** One of the lab's own preview options (theme, colours, source ...), read from the page by the host-side bridge (RealMeterLab). */
export type LabOption =
  | { id: string; label: string; help: string; type: 'select'; value: string; choices: { value: string; text: string }[] }
  | { id: string; label: string; help: string; type: 'bool'; value: boolean }
  | { id: string; label: string; help: string; type: 'range'; value: number; min: number; max: number };
/** The lab's detail area (commands, CPU, cost of each option), as text lines and table rows. */
export type LabInfo = {
  /** The lines of the cost summary, as the lab wrote them (see costInfo.ts). */
  lines: string[];
  /** What each option of the meter costs: one row per option, one cell per choice (three short lines each). */
  table: { label: string; cells: { lines: string[]; sel: boolean }[] }[];
  tableNote: string;
  note: string;
};

/** The options the lab offers for the meter it shows; empty until the lab is ready. */
export const labOptions = writable<LabOption[]>([]);
/** Sends a message to the lab frame (set by the preview component; null while there is no frame). */
export const labSend = writable<((message: Record<string, unknown>) => void) | null>(null);
/** Which view of the meter the preview shows: the small view (the meter's band of the Now Playing screen) or the full-screen figure. */
export type PreviewView = 'small' | 'full';
export const previewView = writable<PreviewView>('small');
