// The Library page shows the last list of a music folder at once and checks the folder for changes behind it.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/saved-list-test.cjs
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const url = process.env.URL || 'http://localhost:5173';
let passed = 0;
const ok = (n) => { passed++; console.log(`ok - ${n}`); };
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  p.on('pageerror', (e) => errors.push(e.message));
  p.on('console', (m) => m.text().startsWith('[dev-mock] unmocked') && errors.push(m.text()));
  await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(800);
  await p.getByRole('button', { name: 'Library', exact: true }).click(); await p.waitForTimeout(400);
  await p.getByRole('button', { name: 'Choose folder…' }).click(); await p.waitForTimeout(800);
  const src = () => p.getByRole('region', { name: 'This computer' });
  assert.match(await src().innerText(), /Kind of Blue/); ok('the first read lists the albums');
  assert.ok(await p.evaluate(() => !!localStorage.getItem('tau.mock.saved'))); ok('the list is kept for next time');
  // next visit: the folder check is slow, the saved list is there at once
  await p.evaluate(() => localStorage.setItem('tau.mock.listDelay', '2500'));
  await p.reload(); await p.waitForTimeout(900);
  await p.getByRole('button', { name: 'Library', exact: true }).click(); await p.waitForTimeout(400);
  let text = await src().innerText();
  assert.match(text, /Kind of Blue/); assert.match(text, /Showing the saved list from/); assert.match(text, /Checking for changes/); ok('the saved list shows at once, with a note that it is being checked');
  assert.doesNotMatch(text, /Reading your library/); ok('there is no blocking "Reading your library" screen');
  await p.waitForTimeout(2800);
  text = await src().innerText();
  assert.doesNotMatch(text, /Showing the saved list/); assert.match(text, /Kind of Blue/); ok('when the check ends the note goes and the list stays');
  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  console.log(`${passed} checks passed`); await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
