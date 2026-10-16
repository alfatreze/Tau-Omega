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
  assert.equal(await sel.inputValue(), 'auto'); assert.match(await card.innerText(), /A release follows Stable, a Preview build follows Preview, and a Dev build gets no offers/); ok('the update channel is Automatic by default and says what that means');
  assert.deepEqual(await sel.locator('option').allInnerTexts(), ['Automatic (follows this build)', 'Stable: published releases', 'Preview: published pre-releases', 'Dev: test builds, installed by hand']); ok('the choices are the three channels of Tau Alpha (Stable, Preview, Dev) and Automatic');
  await sel.selectOption('stable'); await p.waitForTimeout(500);
  assert.match(await card.innerText(), /Switch to stable: version 0\.5\.0 \(you have 0\.5\.1-dev\.1\)/); assert.equal(await card.getByRole('button', { name: 'Switch to 0.5.0' }).count(), 1); ok('choosing Stable on a dev build offers a switch to the newest release instead of saying it is up to date');
  await sel.selectOption('preview'); await p.waitForTimeout(500);
  assert.match(await card.innerText(), /Switch to preview: version 0\.5\.1-preview\.1/); assert.match(await card.innerText(), /rough edges/); ok('choosing Preview offers the newest preview build');
  await sel.selectOption('dev'); await p.waitForTimeout(500);
  assert.match(await card.innerText(), /updated by hand/); assert.equal(await card.getByRole('button', { name: /^(Switch|Update) to/ }).count(), 0); ok('Dev offers nothing and says test builds are installed by hand');
  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  console.log(`${passed} checks passed`); await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
