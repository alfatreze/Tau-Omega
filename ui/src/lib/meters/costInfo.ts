// The lab's cost figures, turned into what the page shows: labelled rows for the Details panel (a label, a value that lines up on the right,
// an explanation) and the numbers behind the processing budget bar. The lab sends them as data (lab embed contract 1: `tau-lab-info` and
// `tau-lab-cost`), so nothing here reads text.
import type { LabCost } from './labBus';

export type CostRow = { label: string; value: string; note?: string; tone?: 'ok' | 'bad' };

const rnd = (n: number, digits = 1) => String(Math.round(n * 10 ** digits) / 10 ** digits);

/** The Details panel's summary rows. The lab's figures cover the worst and the average of its last 120 frames. */
export function costRows(c: LabCost): CostRow[] {
  const cls = c.class ?? 2;
  return [
    { label: 'Drawing commands per frame, average', value: rnd(c.mean) },
    { label: 'Drawing commands per frame, worst', value: rnd(c.max, 0) },
    { label: `Allowed for this meter (class ${cls})`, value: rnd(c.budget, 0) },
    { label: 'Compared with the bars meter', value: `${rnd(c.vs_bars, 2)}×` },
    { label: 'Within the allowance?', value: c.over ? 'No, over' : 'Yes', tone: c.over ? 'bad' : 'ok' },
    { label: 'Based on', value: c.basis },
    { label: 'Drawing cell width', value: `${c.cell_width} px`, note: c.cell_width > 1 ? 'widened by the cost guard and the Resolution setting' : 'full resolution' },
    { label: 'Processor use (estimate)', value: `${rnd(c.cpu_low)}–${rnd(c.cpu_high)} %`, note: 'of one processor core. The model: issuing a command takes 60 to 400 cycles, evaluating a column 30 to 80, at 38 Hz on a 66.7 MHz clock. Not measured on a Pocket.' },
    { label: 'Pixels drawn per frame', value: `${c.engine_kpx}k px`, note: 'by the drawing engine' }
  ];
}

/** The signed change shown beside an option's processor use, as in the lab: +0.4, -0.6, +0.0. */
export const signed = (n: number) => (n >= 0 ? '+' : '-') + Math.abs(n).toFixed(1);

/** The numbers behind the budget line: how many drawing commands a frame costs against what the meter's cost class allows. */
export type Budget = { mean: number; worst: number; allowed: number; costClass: string; over: boolean };

/** The budget from a `tau-lab-cost` message. The bar uses the lab's `recent` window (the last 30 frames, a fast-moving figure); the verdict the
 *  lab itself stands by is the worst of the last 120 frames, which the Details panel shows. */
export function budgetFromCost(m: { recent?: { n: number; mean: number; max: number }; budget?: number; class?: number | null }): Budget | null {
  if (!m.recent || typeof m.budget !== 'number' || !m.recent.n) return null;
  return { mean: Math.round(m.recent.mean * 10) / 10, worst: m.recent.max, allowed: m.budget, costClass: String(m.class ?? 2), over: m.recent.max > m.budget };
}

/** What the budget bar draws, all as percentages of the bar's width, and the sentence that goes with it. */
export type BudgetView = {
  allowedAt: number; meanAt: number; withinTo: number; overFrom: number | null; overTo: number | null;
  kind: 'under' | 'over' | 'at'; text: string;
};

/** The bar's scale is fixed by the allowance alone (it ends at 1.25 times the allowance), so the tick never moves and a change in the
 *  worst frame only moves the fill. A worst frame beyond the end of the bar is drawn to the end; the sentence still gives the exact figure. */
export const BAR_SCALE = 1.25;

export function budgetView(b: Budget): BudgetView {
  const domain = b.allowed * BAR_SCALE || 1;
  const at = (v: number) => Math.max(0, Math.min(100, (v / domain) * 100));
  const diff = Math.abs(b.allowed - b.worst);
  const pct = b.allowed ? Math.round((diff / b.allowed) * 100) : 0;
  const kind = b.worst > b.allowed ? 'over' : b.worst === b.allowed ? 'at' : 'under';
  return {
    allowedAt: at(b.allowed), meanAt: at(b.mean), withinTo: at(Math.min(b.worst, b.allowed)),
    overFrom: kind === 'over' ? at(b.allowed) : null, overTo: kind === 'over' ? at(b.worst) : null,
    kind,
    text: kind === 'over' ? `Over by ${fmt(diff)} (${pct} %)` : kind === 'at' ? 'Exactly at the allowance' : `Under by ${fmt(diff)} (${pct} %)`
  };
}
const fmt = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(1));
