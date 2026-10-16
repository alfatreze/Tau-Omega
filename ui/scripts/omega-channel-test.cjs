// Tau Omega's own updates follow a channel chosen in Settings: Automatic (the channel of the running build), Stable, or Dev (the newest test builds and releases).
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/omega-channel-test.cjs
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
  await p.getByRole('button', { name: /Tools & settings/ }).click(); await p.getByRole('button', { name: 'Settings', exact: true }).click(); await p.waitForTimeout(500);
  const card = p.locator('.ou-card');
  const sel = card.getByLabel('Update channel');
  assert.equal(await sel.inputValue(), 'auto'); assert.match(await card.innerText(), /A test build \(such as 0\.5\.1-alpha\.3\) follows Dev, a release follows Stable/); ok('the update channel is Automatic by default and says what that means');
  assert.deepEqual(await sel.locator('option').allInnerTexts(), ['Automatic (follows this build)', 'Stable: published releases only', 'Dev: the newest test builds, and releases']); ok('the choices are Automatic, Stable and Dev');
  await sel.selectOption('dev'); await p.waitForTimeout(500);
  assert.match(await card.innerText(), /Version 0\.5\.1-alpha\.2 is available on the dev channel/); assert.match(await card.innerText(), /rough edges/); ok('on Dev a newer test build is offered, with a warning that it is a test build');
  await sel.selectOption('stable'); await p.waitForTimeout(500);
  assert.match(await card.innerText(), /Tau Omega is up to date/); assert.match(await card.innerText(), /Only published releases are offered/); ok('on Stable the test build is not offered');
  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  console.log(`${passed} checks passed`); await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
