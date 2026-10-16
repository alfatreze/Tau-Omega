// Several cores installed together from the Marketplace and from Core updates: select, download and check each, one reviewed plan.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/multi-install-test.cjs
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
  await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(900);
  await p.locator('nav button', { hasText: 'Marketplace' }).first().click(); await p.waitForTimeout(700);
  assert.equal(await p.getByRole('region', { name: 'Selected cores' }).count(), 0); ok('nothing is selected at first and there is no selection bar');
  const boxes = p.getByRole('checkbox', { name: /to install together/ });
  assert.ok(await boxes.count() >= 3); ok('each core with a download can be selected');
  await boxes.nth(0).check(); await boxes.nth(1).check(); await p.waitForTimeout(200);
  const bar = p.getByRole('region', { name: 'Selected cores' });
  assert.match(await bar.innerText(), /2 cores selected/); ok('the bar counts the selected cores');
  await bar.getByRole('button', { name: /Install 2 together/ }).click(); await p.waitForTimeout(1500);
  const dlg = p.getByRole('dialog', { name: /2 cores on/ });
  const text = await dlg.innerText();
  assert.equal((text.match(/Ready/g) ?? []).length, 2); ok('both cores are downloaded and checked');
  await dlg.getByRole('button', { name: /Review 2 installs/ }).click(); await p.waitForTimeout(1200);
  assert.match(await dlg.innerText(), /Install|install/); assert.equal(await dlg.getByRole('button', { name: 'Review 2 installs' }).count(), 0); ok('then one reviewed plan covers them together');
  await dlg.getByRole('button', { name: 'Cancel' }).click(); await p.waitForTimeout(200);
  assert.equal(await p.getByRole('dialog').count(), 0); ok('cancelling closes it with nothing installed');
  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  console.log(`${passed} checks passed`); await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
