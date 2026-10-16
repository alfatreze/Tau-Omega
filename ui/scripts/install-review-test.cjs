// Behaviour checks for the simple install review (one core or several at once) against the dev mock.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/install-review-test.cjs   (URL=... to point elsewhere)
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const url = process.env.URL || 'http://localhost:5173';
let passed = 0;
const ok = (name) => { passed++; console.log(`ok - ${name}`); };
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1440, height: 1000 } });
  const errors = [];
  p.on('pageerror', (e) => errors.push(e.message));
  p.on('console', (m) => m.text().startsWith('[dev-mock] unmocked') && errors.push(m.text()));
  const start = async (dev) => { await p.goto(url); await p.evaluate((d) => { localStorage.clear(); if (d) { localStorage.setItem('tau.mock.devTools', '1'); localStorage.setItem('tau.confirmRealCard', '1'); } }, dev); await p.goto(url); await p.waitForTimeout(1200); };
  const ma = () => p.locator('.ma');
  const openManual = async () => { await p.locator('nav button', { hasText: 'Marketplace' }).first().click(); await p.waitForTimeout(500); await p.getByRole('button', { name: /Add a core manually/ }).click(); await p.waitForTimeout(300); };
  const read = async (addr) => { await ma().getByRole('textbox').fill(addr); await p.waitForTimeout(450); await ma().getByRole('button', { name: /Read this core/ }).click(); await p.waitForTimeout(500); };

  // ---- one core: the simple review
  await start(false);
  await openManual(); await read('https://github.com/someone/one-core');
  await ma().getByRole('checkbox', { name: /I trust this source/ }).check(); await p.waitForTimeout(700);
  const review = ma().getByRole('region', { name: 'Install review' });
  assert.match(await review.locator('h2').innerText(), /^Install one-core-.* on Pocket$|^Install .* on Pocket$/); ok('the review opens by itself with one heading: what is installed and where');
  assert.equal(await review.locator('.ir-cores li').count(), 1); assert.match(await review.locator('.ir-cores li').innerText(), /New/); ok('one line per core, with whether it is new or an update');
  assert.match(await review.locator('.ir-sum').innerText(), /^2 files, [\d.]+ MB · fits \([\d.]+ GB free\) · nothing on the card is replaced$/); ok('one summary line: files, size, whether it fits, what happens to existing files');
  assert.equal(await review.getByRole('button', { name: 'Install', exact: true }).count(), 1); ok('one button');
  assert.doesNotMatch(await review.innerText(), /Safety net|read back and checked|Review the update/); ok('no safety-net paragraph in the way');
  assert.equal(await review.locator('details.ir-details').getAttribute('open'), null); assert.equal(await review.locator('.ir-files').isVisible(), false); ok('the file list and backup facts are hidden behind Details');
  await review.locator('details.ir-details summary').click(); await p.waitForTimeout(200);
  assert.equal(await review.locator('.ir-files li').count(), 2); assert.match(await review.locator('.ir-details').innerText(), /Nothing existing is replaced, so there is nothing to back up/); ok('Details shows the files and the backup facts');
  const countBefore = Number(await p.locator('.nt-count').first().innerText());
  await review.getByRole('button', { name: 'Install', exact: true }).click(); await p.waitForTimeout(800);
  const done = ma().getByRole('region', { name: 'Install result' });
  assert.match(await done.locator('h2').innerText(), /^Installed 1 core$/); assert.match(await done.innerText(), /Installed and checked/); ok('the result says what happened in one heading and one chip');
  assert.equal(Number(await p.locator('.nt-count').first().innerText()), countBefore + 1); ok('after an install the Cores list in the menu shows the new core without reopening the card');
  await done.getByRole('button', { name: 'Undo…' }).click(); await p.waitForTimeout(150);
  await done.getByRole('button', { name: 'Yes, undo' }).click(); await p.waitForTimeout(500);
  assert.match(await done.innerText(), /Undone: 1 install put back/); ok('and it can be undone');

  // ---- several cores from the manual-add sheet
  await start(false);
  await openManual(); await read('https://github.com/someone/first-core');
  assert.equal(await ma().getByRole('button', { name: 'Add another core' }).isDisabled(), true); ok('Add another core waits for the trust tick, like installing does');
  await ma().getByRole('checkbox', { name: /I trust this source/ }).check();
  await ma().getByRole('button', { name: 'Add another core' }).click(); await p.waitForTimeout(300);
  const basket = ma().getByRole('region', { name: 'Cores ready to install' });
  assert.match(await basket.innerText(), /Ready to install \(1\)[\s\S]*Review and install 1 core/); ok('the first core waits on a list while the next is chosen');
  await read('https://github.com/someone/second-core');
  await ma().getByRole('checkbox', { name: /I trust this source/ }).check(); await p.waitForTimeout(700);
  const review2 = ma().getByRole('region', { name: 'Install review' });
  assert.match(await review2.locator('h2').innerText(), /^Install 2 cores on Pocket$/); assert.equal(await review2.locator('.ir-cores li').count(), 2); ok('the review covers the list and the core being looked at: one heading, one line each');
  assert.equal(await review2.getByRole('button', { name: 'Install 2 cores' }).count(), 1); ok('and one button installs them all');
  await review2.getByRole('button', { name: 'Install 2 cores' }).click(); await p.waitForTimeout(1000);
  const done2 = ma().getByRole('region', { name: 'Install result' });
  assert.match(await done2.locator('h2').innerText(), /^Installed 2 cores$/); assert.equal(await done2.locator('.ir-cores li').count(), 2); ok('both are installed and reported together');
  await done2.getByRole('button', { name: 'Undo…' }).click(); await done2.getByRole('button', { name: 'Yes, undo' }).click(); await p.waitForTimeout(500);
  assert.match(await done2.innerText(), /Undone: 2 installs put back/); ok('and undone together');

  // ---- several builds from the Builds page
  await start(true);
  await p.locator('nav button', { hasText: /^Builds$/ }).click(); await p.waitForTimeout(400);
  await p.getByLabel('Build folder').fill('/Users/dev/builds'); await p.getByRole('button', { name: 'Use this folder' }).click(); await p.waitForTimeout(600);
  await p.getByLabel('Select TAU_DEV_113.zip to install with others').check(); await p.getByLabel('Select TAU_DEV_112.zip to install with others').check(); await p.waitForTimeout(200);
  assert.match(await p.locator('.b-pickbar').innerText(), /2 builds selected[\s\S]*Install 2 builds/); ok('ticking builds shows a bar with how many and one button');
  await p.locator('.b-pickbar').getByRole('button', { name: 'Install 2 builds' }).click(); await p.waitForTimeout(800);
  const sheet = p.getByRole('dialog', { name: /2 builds/ });
  const r3 = sheet.getByRole('region', { name: 'Install review' });
  assert.equal(await r3.locator('.ir-cores li').count(), 2); assert.match(await r3.locator('h2').innerText(), /^Install 2 cores on Pocket$/); ok('they are reviewed together in one sheet');
  assert.equal(await r3.getByRole('button', { name: 'Install 2 cores' }).isDisabled(), true); await r3.getByLabel('Type the card name to confirm').fill('Pocket');
  assert.equal(await r3.getByRole('button', { name: 'Install 2 cores' }).isDisabled(), false); ok('a Real card still asks for its name, once for all of them');
  await r3.getByRole('button', { name: 'Install 2 cores' }).click(); await p.waitForTimeout(1000);
  assert.match(await sheet.getByRole('region', { name: 'Install result' }).locator('h2').innerText(), /^Installed 2 cores$/); ok('and install together');

  assert.deepEqual(errors, [], errors.join(' | ')); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
