// Behaviour checks for the developer Results page (decoded Check reports, run comparison, the last-install baseline) against the dev mock.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/results-test.cjs   (URL=... to point elsewhere)
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
  const nav = (n) => p.locator('nav button', { hasText: new RegExp(`^${n}$`) }).first();
  const page = () => p.locator('.results');

  await start(false);
  assert.equal(await p.locator('nav button', { hasText: /^Results$/ }).count(), 0); ok('with developer tools off there is no Results page in the menu');
  await start(true);
  await nav('Results').click(); await p.waitForTimeout(700);
  assert.match(await page().locator('.r-since').innerText(), /since the last install on Pocket, 2026-10-14 19:30:00/); ok('the page says which install it counts from');
  assert.equal(await page().locator('.r-run').count(), 1); ok('only the run since that install is shown by default');
  const run = page().locator('.r-run').first();
  assert.match(await run.innerText(), /STANDARD[\s\S]*2026-10-14 20:15:00[\s\S]*Firmware fw-0\.6\.0\+b2 · bitstream bs-5[\s\S]*some checks failed[\s\S]*Failing: Track changes/); ok('a run shows its time, build, verdict and what is failing');
  assert.match(await run.innerText(), /Compared with the previous STANDARD run, 2026-10-14 19:00:00/i); ok('it names the earlier run it is compared with');
  const lines = await run.locator('.r-diff li').allInnerTexts();
  assert.equal(lines.length, 2); assert.match(lines[0], /Changed\s*Firmware\s*fw-0\.6\.0\+b1 → fw-0\.6\.0\+b2/); assert.match(lines[1], /Worse\s*Late underruns\s*0 → 2/); ok('each difference says what changed, from what to what, and whether it is better or worse');
  assert.equal(await run.locator('.r-diff li.worse .chip.bad').count(), 1); ok('a worse result is marked');

  assert.match(await page().innerText(), /Report pages that could not be read[\s\S]*pixel grid is damaged/i); ok('a damaged report page is listed with the reason');
  assert.match(await page().innerText(), /1 other screenshot since the last install is not a Check report/); ok('other screenshots are only counted');

  const show = run.locator('.r-foot button');
  await show.click(); await p.waitForTimeout(300);
  assert.equal(await show.getAttribute('aria-expanded'), 'true'); assert.equal(await run.locator('img.r-pic').count(), 1); ok('the screenshot can be shown');
  await run.getByRole('button', { name: 'Hide the screenshot' }).click(); await p.waitForTimeout(150);
  assert.equal(await run.locator('img.r-pic').count(), 0); ok('and hidden');

  await page().getByLabel('Include older runs').check(); await p.waitForTimeout(250);
  assert.equal(await page().locator('.r-run').count(), 2); assert.match(await page().locator('.r-run').nth(1).innerText(), /FULL[\s\S]*First run of this profile[\s\S]*Nothing to compare with yet/i); ok('older runs can be included; a first run says there is nothing to compare with');
  await page().getByLabel('Include older runs').uncheck();

  await page().getByRole('button', { name: 'Start fresh from now' }).click(); await p.waitForTimeout(500);
  assert.match(await page().innerText(), /Starting fresh from now/); assert.doesNotMatch(await page().locator('.r-since').innerText(), /19:30:00/); ok('Start fresh from now moves the baseline');
  assert.match(await page().innerText(), /No Check reports since the last install[\s\S]*Run Check on the Pocket/i); ok('and an empty page says what to do');
  await page().getByRole('button', { name: 'Show everything' }).click(); await p.waitForTimeout(500);
  assert.match(await page().locator('.r-since').innerText(), /every screenshot on Pocket: no install has been recorded/); assert.ok(await page().locator('.r-run').count() >= 1); ok('Show everything clears the baseline');

  // ---- an install on the card records the baseline
  await p.locator('nav button', { hasText: 'Marketplace' }).first().click(); await p.waitForTimeout(500);
  await p.getByRole('button', { name: /Add a core manually/ }).click(); await p.waitForTimeout(300);
  await p.locator('.ma').getByRole('textbox').fill('https://github.com/someone/unlisted-core'); await p.waitForTimeout(450);
  await p.locator('.ma').getByRole('button', { name: /Read this core/ }).click(); await p.waitForTimeout(500);
  await p.locator('.ma').getByRole('checkbox', { name: /I trust this source/ }).check(); await p.waitForTimeout(400);
  await p.locator('.ma').getByRole('button', { name: 'Review update' }).click(); await p.waitForTimeout(600);
  await p.getByLabel('Type the card name to confirm').fill('Pocket');
  await p.getByRole('button', { name: 'Confirm and install' }).click(); await p.waitForTimeout(700);
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  await nav('Results').click(); await p.waitForTimeout(700);
  assert.match(await page().locator('.r-since').innerText(), /since the last install on Pocket, \d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/); ok('finishing an install records it as the new baseline');

  assert.deepEqual(errors, [], errors.join(' | ')); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
