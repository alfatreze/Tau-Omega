// The lab's cost text becomes labelled rows. Pure logic: no browser needed.   Usage: node scripts/cost-info-test.cjs
const assert = require('node:assert/strict');
let passed = 0; const ok = (n) => { passed++; console.log(`ok - ${n}`); };
(async () => {
  const { parseCostLines } = await import('../src/lib/meters/costInfo.ts');
  // The lines exactly as the lab wrote them for Winamp Bars (owner's screenshot, 2026-11-07).
  const rows = parseCostLines([
    'commands/frame: mean 20.8, worst 27 (class 1 budget 100) · 0.58× VIZ_BARS · within budget · model estimate',
    'drawing cell width: 1 px (full resolution)',
    'CPU estimate: 0.1–0.5% of one core (model: command issue 60–400 cycles, column evaluation 30–80, 38 Hz, 66.7 MHz; unmeasured) · engine writes 0k px/frame',
  ]);
  const get = (label) => rows.find((r) => r.label === label);
  assert.equal(get('Drawing commands per frame, average').value, '20.8'); assert.equal(get('Drawing commands per frame, worst').value, '27');
  assert.equal(get('Allowed for this meter (class 1)').value, '100'); assert.equal(get('Compared with the bars meter').value, '0.58×'); ok('the command counts become separate numbers');
  assert.deepEqual([get('Within the allowance?').value, get('Within the allowance?').tone], ['Yes', 'ok']); assert.equal(get('Based on').value, 'model estimate'); ok('the budget verdict is a plain Yes or No');
  assert.equal(get('Drawing cell width').value, '1 px'); assert.equal(get('Drawing cell width').note, 'full resolution'); ok('the cell width keeps its explanation');
  assert.equal(get('Processor use (estimate)').value, '0.1–0.5 %'); assert.match(get('Processor use (estimate)').note, /of one processor core\. model: command issue 60–400 cycles/); ok('the processor estimate is a range with its model as the note');
  assert.equal(get('Pixels drawn per frame').value, '0k px'); assert.equal(rows.at(-1).label, 'Pixels drawn per frame'); ok('the engine pixel count that trails the processor line is its own row');
  assert.equal(parseCostLines(['commands/frame: mean 9, worst 12 (class 2 budget 50) · 0.2× VIZ_BARS · OVER BUDGET · measured']).find((r) => r.label === 'Within the allowance?').tone, 'bad'); ok('over budget is marked');
  assert.deepEqual(parseCostLines(['something the lab says in the future']), [{ label: 'Note', value: '', note: 'something the lab says in the future' }]); assert.deepEqual(parseCostLines([]), []); ok('a line it does not know is kept as a note, nothing is lost');
  // The budget bar: the worst frame against the allowance, over or under.
  const { parseBudget, budgetView } = await import('../src/lib/meters/costInfo.ts');
  const bars = parseBudget(['commands/frame: mean 20.8, worst 27 (class 1 budget 100) · 0.58× VIZ_BARS · within budget · model estimate']);
  assert.deepEqual(bars, { mean: 20.8, worst: 27, allowed: 100, costClass: '1', over: false }); ok('the budget numbers are read from the lab\'s line');
  const under = budgetView(bars); assert.equal(under.kind, 'under'); assert.equal(under.text, 'Under by 73 (73 %)'); assert.equal(under.overFrom, null);
  assert.equal(under.allowedAt, 80); assert.ok(under.withinTo > 20 && under.withinTo < 23 && under.meanAt < under.withinTo); ok('under budget: the bar fills part of the allowance and says how much is left');
  const over = budgetView({ mean: 380, worst: 432, allowed: 400, costClass: '2', over: true }); assert.equal(over.kind, 'over'); assert.equal(over.text, 'Over by 32 (8 %)');
  assert.ok(over.overFrom !== null && over.overTo > over.overFrom && over.withinTo === over.allowedAt); assert.equal(over.allowedAt, 80); ok('over budget: the part beyond the allowance is its own red stretch and the text says by how much');
  assert.equal(budgetView({ mean: 380, worst: 432, allowed: 400, costClass: '2', over: true }).allowedAt, budgetView({ mean: 10, worst: 12, allowed: 400, costClass: '2', over: false }).allowedAt); assert.equal(budgetView({ mean: 900, worst: 2000, allowed: 400, costClass: '2', over: true }).overTo, 100); ok('the allowance tick never moves, whatever the worst frame is, and a huge overrun is drawn to the end of the bar'); 
  assert.equal(budgetView({ mean: 90, worst: 100, allowed: 100, costClass: '1', over: false }).kind, 'at'); assert.equal(parseBudget(['nothing the lab says']), null); assert.equal(budgetView({ mean: 0, worst: 0, allowed: 0, costClass: '0', over: false }).allowedAt, 0); ok('exactly at the allowance, no budget line and an empty meter are handled');
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
