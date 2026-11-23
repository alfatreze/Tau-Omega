// The read-only "Meters on the Pocket" page in its other states: an older Tau that ignores the meter file, a file the Pocket refuses, no meter files.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/meters-on-pocket-test.cjs   (URL=... to point elsewhere)
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const url = process.env.URL || 'http://localhost:5173';
let passed = 0; const ok = (n) => { passed++; console.log(`ok - ${n}`); };
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const errors = [];
  const open = async (mode) => {
    const p = await b.newPage({ viewport: { width: 1400, height: 1000 } });
    p.on('pageerror', (e) => errors.push(e.message)); p.on('console', (m) => m.text().startsWith('[dev-mock] unmocked') && errors.push(m.text()));
    await p.addInitScript((m) => { globalThis.__meters = m; }, mode);
    await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(900);
    await p.locator('nav button', { hasText: 'Meter Lab' }).first().click(); await p.waitForTimeout(1000);
    await p.locator('[data-testid=mp-mode-list]').click(); await p.waitForTimeout(300);
    return p;
  };
  const page = (p) => p.locator('[data-testid=meters-on-pocket]');

  let p = await open('old');
  let text = await page(p).innerText();
  assert.match(text, /alfatreze\.TAU\s*0\.7\.1[\s\S]*Uses the meters that come with it[\s\S]*does not read a meter file yet \(it needs Tau 0\.7\.5 or newer\)/); ok('an older Tau is said to ignore the meter file, with what it needs');
  assert.match(text, /From Assets\/tau\/alfatreze\.TAU\/tau-packs\.bin/); assert.ok(!/version 1\.0\.0/.test(text)); ok('it lists the meters that come with it (an old bundle has no versions)');
  await p.close();

  p = await open('refused');
  text = await page(p).innerText();
  assert.match(text, /Uses the meters that come with it[\s\S]*The Pocket ignores your meter file: The meter file has no built-in Winamp Bars entry/); ok('a meter file the Pocket would refuse says why, and the core falls back to its own meters');
  assert.equal(await page(p).locator('li').count(), 4); ok('the list shown is the one the Pocket will actually use');
  await p.close();

  p = await open('none');
  text = await page(p).innerText();
  assert.match(text, /Only the built-in Winamp Bars\. No meter file is on the card\./); assert.equal(await page(p).locator('li').count(), 0); ok('a card with no meter files says so');
  await p.close();

  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  console.log(`\n${passed} checks passed`);
  await b.close();
})().catch((e) => { console.error('FAILED:', e.message); process.exit(1); });
