// The lab's "Info: commands, CPU, cost of each option" area, turned from running text into labelled rows (a label, a value that lines up on
// the right, and an explanation). The lab writes it as lines separated by line breaks; the bridge in RealMeterLab sends the lines apart. A line
// this does not recognise is kept as a plain note, so nothing the lab says is lost if its wording changes.

export type CostRow = { label: string; value: string; note?: string; tone?: 'ok' | 'bad' };

const num = '[\\d.]+';

/** `engine writes 0k px/frame` can trail any line (the lab appends it to the processor line). */
function takeEngine(line: string): { rest: string; px?: string } {
  const m = line.match(/\s*·\s*engine writes ([\d.]+k?) px\/frame\s*$/);
  return m ? { rest: line.slice(0, m.index), px: m[1] } : { rest: line };
}

export function parseCostLines(lines: string[]): CostRow[] {
  const rows: CostRow[] = [];
  const engineRows: CostRow[] = [];
  for (const raw of lines) {
    const { rest: line, px } = takeEngine(raw.trim());
    if (px !== undefined) engineRows.push({ label: 'Pixels drawn per frame', value: `${px} px`, note: 'by the drawing engine' });
    let m: RegExpMatchArray | null;
    if ((m = line.match(new RegExp(`^commands/frame: mean (${num}), worst (${num}) \\(class (\\S+) budget (${num})\\)(?: · (${num})× VIZ_BARS)?(?: · (within budget|OVER BUDGET))?(?: · (.+))?$`)))) {
      rows.push({ label: 'Drawing commands per frame, average', value: m[1] });
      rows.push({ label: 'Drawing commands per frame, worst', value: m[2] });
      rows.push({ label: `Allowed for this meter (class ${m[3]})`, value: m[4] });
      if (m[5]) rows.push({ label: 'Compared with the bars meter', value: `${m[5]}×` });
      if (m[6]) rows.push({ label: 'Within the allowance?', value: m[6] === 'OVER BUDGET' ? 'No, over' : 'Yes', tone: m[6] === 'OVER BUDGET' ? 'bad' : 'ok' });
      if (m[7]) rows.push({ label: 'Based on', value: m[7] });
    } else if ((m = line.match(new RegExp(`^drawing cell width: (${num}) px(?: \\((.*)\\))?$`)))) {
      rows.push({ label: 'Drawing cell width', value: `${m[1]} px`, note: m[2] });
    } else if ((m = line.match(new RegExp(`^CPU estimate: (${num})[–-](${num})% of one core(?: \\((.*)\\))?$`)))) {
      rows.push({ label: 'Processor use (estimate)', value: `${m[1]}–${m[2]} %`, note: `of one processor core${m[3] ? `. ${m[3]}` : ''}` });
    } else if (line) {
      rows.push({ label: 'Note', value: '', note: line });
    }
  }
  return [...rows, ...engineRows];
}

/** The numbers behind the budget line: how many drawing commands a frame costs against what the meter's cost class allows. */
export type Budget = { mean: number; worst: number; allowed: number; costClass: string; over: boolean };

export function parseBudget(lines: string[]): Budget | null {
  for (const raw of lines) {
    const m = raw.match(new RegExp(`^commands/frame: mean (${num}), worst (${num}) \\(class (\\S+) budget (${num})\\)`));
    if (m) {
      const [mean, worst, allowed] = [Number(m[1]), Number(m[2]), Number(m[4])];
      return { mean, worst, allowed, costClass: m[3], over: worst > allowed };   // the lab's own rule: the worst frame against the allowance
    }
  }
  return null;
}

/** What the budget bar draws, all as percentages of the bar's width, and the sentence that goes with it. */
export type BudgetView = {
  allowedAt: number; meanAt: number; withinTo: number; overFrom: number | null; overTo: number | null;
  kind: 'under' | 'over' | 'at'; text: string;
};

export function budgetView(b: Budget): BudgetView {
  // The bar is always wide enough to hold both the allowance and the worst frame, with a little room to the right.
  const domain = Math.max(b.allowed, b.worst) * 1.12 || 1;
  const at = (v: number) => Math.min(100, (v / domain) * 100);
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

