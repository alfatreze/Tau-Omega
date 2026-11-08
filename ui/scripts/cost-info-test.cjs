// The lab's cost figures become labelled rows and a budget bar. Pure logic: no browser needed.   Usage: node scripts/cost-info-test.cjs
const assert = require('node:assert/strict');
let passed = 0; const ok = (n) => { passed++; console.log(`ok - ${n}`); };
(async () => {
  const { costRows, signed, budgetFromCost, budgetView } = await import('../src/lib/meters/costInfo.ts');
  // The figures exactly as the lab's tau-lab-info message carries them for Winamp Bars (contract 1).
  const cost = { mean: 20.8, max: 27, budget: 100, class: 1, over: false, verdict: 'within budget', basis: 'model estimate', vs_bars: 0.58, cell_width: 1, cpu_low: 0.1, cpu_high: 0.5, engine_kpx: 0 };
  const rows = costRows(cost); const get = (label) => rows.find((r) => r.label === label);
  assert.equal(get('Drawing commands per frame, average').value, '20.8'); assert.equal(get('Drawing commands per frame, worst').value, '27');
  assert.equal(get('Allowed for this meter (class 1)').value, '100'); assert.equal(get('Compared with the bars meter').value, '0.58×'); ok('the command counts become separate numbers');
  assert.deepEqual([get('Within the allowance?').value, get('Within the allowance?').tone], ['Yes', 'ok']); assert.equal(get('Based on').value, 'model estimate'); ok('the budget verdict is a plain Yes or No');
  assert.equal(get('Drawing cell width').value, '1 px'); assert.equal(get('Drawing cell width').note, 'full resolution'); assert.match(costRows({ ...cost, cell_width: 3 }).find((r) => r.label === 'Drawing cell width').note, /widened/); ok('the cell width says whether it was widened');
  assert.equal(get('Processor use (estimate)').value, '0.1–0.5 %'); assert.match(get('Processor use (estimate)').note, /of one processor core\. The model: issuing a command takes 60 to 400 cycles/); ok('the processor estimate is a range with its model as the note');
  assert.equal(get('Pixels drawn per frame').value, '0k px'); assert.equal(rows.at(-1).label, 'Pixels drawn per frame'); ok('the engine pixel count is its own row');
  assert.equal(costRows({ ...cost, over: true, verdict: 'OVER BUDGET' }).find((r) => r.label === 'Within the allowance?').tone, 'bad'); assert.equal(costRows({ ...cost, class: null })[2].label, 'Allowed for this meter (class 2)'); ok('over budget is marked, and a meter with no class is class 2 as in the lab');
  assert.deepEqual([signed(0.4), signed(-0.6), signed(0)], ['+0.4', '-0.6', '+0.0']); ok('the change beside an option is signed as in the lab');
  // The budget bar: the last frames against the allowance, over or under.
  const bars = budgetFromCost({ recent: { n: 30, mean: 20.84, max: 27 }, budget: 100, class: 1 });
  assert.deepEqual(bars, { mean: 20.8, worst: 27, allowed: 100, costClass: '1', over: false }); assert.equal(budgetFromCost({ budget: 100 }), null); assert.equal(budgetFromCost({ recent: { n: 0, mean: 0, max: 0 }, budget: 100 }), null); ok('the budget numbers come from the lab\'s cost message (the last 30 frames)');
  const under = budgetView(bars); assert.equal(under.kind, 'under'); assert.equal(under.text, 'Under by 73 (73 %)'); assert.equal(under.overFrom, null);
  assert.equal(under.allowedAt, 80); assert.ok(under.withinTo > 20 && under.withinTo < 23 && under.meanAt < under.withinTo); ok('under budget: the bar fills part of the allowance and says how much is left');
  const over = budgetView({ mean: 380, worst: 432, allowed: 400, costClass: '2', over: true }); assert.equal(over.kind, 'over'); assert.equal(over.text, 'Over by 32 (8 %)');
  assert.ok(over.overFrom !== null && over.overTo > over.overFrom && over.withinTo === over.allowedAt); assert.equal(over.allowedAt, 80); ok('over budget: the part beyond the allowance is its own red stretch and the text says by how much');
  assert.equal(over.allowedAt, budgetView({ mean: 10, worst: 12, allowed: 400, costClass: '2', over: false }).allowedAt); assert.equal(budgetView({ mean: 900, worst: 2000, allowed: 400, costClass: '2', over: true }).overTo, 100); ok('the allowance tick never moves, whatever the worst frame is, and a huge overrun is drawn to the end of the bar');
  assert.equal(budgetView({ mean: 90, worst: 100, allowed: 100, costClass: '1', over: false }).kind, 'at'); assert.equal(budgetView({ mean: 0, worst: 0, allowed: 0, costClass: '0', over: false }).allowedAt, 0); ok('exactly at the allowance and an empty meter are handled');
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
