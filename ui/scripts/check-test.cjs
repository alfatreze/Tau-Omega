// Behaviour checks for the developer Check page (package, folder, installed core, every core; menu symptoms) against the dev mock.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/check-test.cjs   (URL=... to point elsewhere)
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const url = process.env.URL || 'http://localhost:5173';
let passed = 0;
const ok = (name) => { passed++; console.log(`ok - ${name}`); };
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  p.on('pageerror', (e) => errors.push(e.message));
  p.on('console', (m) => m.text().startsWith('[dev-mock] unmocked') && errors.push(m.text()));
  const start = async (dev) => { await p.goto(url); await p.evaluate((d) => { localStorage.clear(); if (d) localStorage.setItem('tau.mock.devTools', '1'); }, dev); await p.goto(url); await p.waitForTimeout(1200); };
  const page = () => p.locator('.check');

  await start(false);
  assert.equal(await p.locator('nav button', { hasText: /^Check$/ }).count(), 0); ok('with developer tools off there is no Check page in the menu');
  await start(true);
  await p.locator('nav button', { hasText: /^Check$/ }).click(); await p.waitForTimeout(500);
  assert.match(await page().innerText(), /The core checker[\s\S]*Nothing is written/); ok('the page says what it does and that it writes nothing');
  assert.equal(await page().getByRole('button', { name: 'Check it' }).first().isDisabled(), true); ok('nothing can be checked before a source is given');

  await page().getByLabel('A package zip or a core folder').fill('/builds/weird.zip'); await page().getByRole('button', { name: 'Check it' }).first().click(); await p.waitForTimeout(500);
  const res = page().locator('.ck-res');
  await res.waitFor({ timeout: 8000 });
  assert.match(await res.innerText(), /weird\.zip[\s\S]*1 error found[\s\S]*bitstream file bitstream\.rbf_r is missing[\s\S]*Add the converted bitstream/i); ok('a package is checked and the finding shows its fix');
  assert.match(await res.innerText(), /1 warning[\s\S]*description is 71 characters/i); ok('warnings are listed with the errors');
  assert.equal(await res.getByText('Why it might not be in the Pocket’s menu').count(), 0); ok('the menu checks only appear for an installed core');
  await page().getByLabel('A package zip or a core folder').fill('/builds/clean.zip'); await page().getByRole('button', { name: 'Check it' }).first().click(); await p.waitForTimeout(500);
  assert.match(await res.innerText(), /clean\.zip[\s\S]*Passes every check[\s\S]*Nothing to fix/i); ok('a clean package says so');

  await page().getByLabel('A core on the open card').selectOption('solo.Thing'); await page().getByRole('button', { name: 'Check it' }).nth(1).click(); await res.getByText('Why it might not be').waitFor({ timeout: 8000 });
  assert.match(await res.innerText(), /solo\.Thing[\s\S]*Why it might not be in the Pocket’s menu[\s\S]*folder is named solo\.Thing, but core\.json says Solo\.Thing[\s\S]*Rename the folder[\s\S]*menu cache[\s\S]*older than this core/i); ok('an installed core shows why it might be missing from the menu, each with its fix');
  assert.equal(await res.locator('.ck-sym li').count(), 2); ok('one line per cause found');
  await page().getByLabel('A core on the open card').selectOption('agg23.SNES'); await page().getByRole('button', { name: 'Check it' }).nth(1).click(); await p.waitForTimeout(500);
  assert.match(await res.innerText(), /None of the usual causes was found/i); ok('a healthy core says no usual cause was found');

  await page().getByRole('button', { name: 'Check every core on the card' }).click(); await p.waitForTimeout(600);
  const list = page().getByRole('list', { name: 'Core check for every core' });
  assert.equal(await list.locator('li').count(), 3); assert.match(await list.innerText(), /SNES[\s\S]*Passes[\s\S]*GBA[\s\S]*1 warning[\s\S]*Thing[\s\S]*1 error/); ok('every core on the card gets one line with its counts');
  assert.match(await page().innerText(), /1 core with errors\. Pick one to see the details/); ok('and a sentence that sums it up');
  await list.getByRole('button', { name: /Thing: 1 error/ }).click(); await page().locator('.ck-res').waitFor({ timeout: 8000 });
  assert.match(await page().locator('.ck-res').innerText(), /solo\.Thing[\s\S]*Why it might not be in the Pocket/i); ok('a row opens that core’s details');

  const audit = await p.evaluate(() => { const name = (el) => (el.getAttribute('aria-label') || el.innerText || el.labels?.[0]?.innerText || '').trim(); const bad = []; for (const el of document.querySelectorAll('.check button, .check input, .check select')) if (!name(el)) bad.push(el.outerHTML.slice(0, 80)); const ids = [...document.querySelectorAll('[id]')].map((e) => e.id); return { bad, dup: ids.filter((x, i) => ids.indexOf(x) !== i) }; });
  assert.deepEqual(audit.bad, []); assert.deepEqual(audit.dup, []); ok('every control has a name and ids are unique');
  assert.deepEqual(errors, [], errors.join(' | ')); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
