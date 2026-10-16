// Settings > Syncing music > Skip silence: off by default, the choices and their defaults, saved to the preferences.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/trim-test.cjs
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
  await p.getByRole('button', { name: /Tools & settings/ }).click(); await p.getByRole('button', { name: 'Settings', exact: true }).click(); await p.waitForTimeout(500);
  const fs = p.locator('fieldset', { hasText: 'Skip silence at the start and end' });
  assert.equal(await fs.count(), 1);
  assert.equal(await fs.getByLabel('Silence level').count(), 0); ok('skip silence is off by default and shows no options');
  await fs.getByRole('button', { name: 'Turn on' }).click(); await p.waitForTimeout(300);
  assert.equal(await fs.getByLabel('Silence level').inputValue(), '-600'); ok('on: the level starts at -60 dB');
  assert.equal(await fs.getByLabel('Silence kept').inputValue(), '100');
  assert.equal(await fs.getByLabel('Smallest skip').inputValue(), '2000');
  assert.equal(await fs.getByLabel('Largest skip').inputValue(), '10000'); ok('the other defaults: keep 100 ms, skip at least 2 s, at most 10 s');
  const levels = await fs.getByLabel('Silence level').locator('option').allTextContents();
  assert.equal(levels[0].trim(), '-80 dB'); assert.match(levels.join('|'), /-60 dB \(default\)/); assert.equal(levels.length, 9); ok('the level is limited to -80 to -40 dB');
  await fs.getByLabel('Smallest skip').selectOption('5000'); await p.waitForTimeout(200);
  assert.match(await p.locator('main').innerText(), /Saved\./); assert.equal(await fs.getByLabel('Smallest skip').inputValue(), '5000'); ok('a change saves');
  await fs.getByRole('button', { name: 'Turn off' }).click(); await p.waitForTimeout(200);
  assert.equal(await fs.getByLabel('Silence level').count(), 0); ok('turning it off hides the options');
  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  console.log(`${passed} checks passed`); await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
