// Behaviour checks for the developer Builds page (folder, series by naming schemes, card comparison, install sheet, remove older) against the dev mock.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/builds-test.cjs   (URL=... to point elsewhere)
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
  const nav = (n) => p.locator('nav button', { hasText: n }).first();
  const page = () => p.locator('.builds');

  await start(false);
  assert.equal(await p.locator('nav button', { hasText: /^Builds$/ }).count(), 0); ok('with developer tools off there is no Builds page in the menu');
  await start(true);
  assert.equal(await p.locator('nav button', { hasText: /^Builds$/ }).count(), 1); ok('with developer tools on the menu has Builds');
  await nav('Builds').click(); await p.waitForTimeout(400);
  assert.match(await page().innerText(), /Choose your build folder/); ok('with no folder chosen it asks for one');

  await p.getByLabel('Build folder').fill('/Users/dev/builds'); await p.getByRole('button', { name: 'Use this folder' }).click(); await p.waitForTimeout(600);
  const groups = await page().locator('.b-group h2').allInnerTexts();
  assert.deepEqual(groups.map((t) => t.replace(/\s+\d+ builds?$/, '')), ['tau dev', 'Not in a series', 'Could not be read']); ok('builds are grouped by series, then unplaced builds, then unreadable ones');
  assert.deepEqual(await page().locator('.b-group').first().locator('.b-main strong').allInnerTexts(), ['TAU DEV 113', 'TAU DEV 112', 'TAU DEV 110']); ok('newest first inside a series');
  const row113 = page().locator('.b-row', { hasText: 'TAU DEV 113' });
  assert.match(await row113.innerText(), /Newer than the card[\s\S]*1 warning[\s\S]*#113/); ok('a row says how it compares with the card, shows the check result and its number');
  assert.match(await page().locator('.b-row', { hasText: 'TAU DEV 112' }).innerText(), /On the card/); ok('the build that is already installed says so');
  assert.equal(await page().locator('.b-row', { hasText: 'TAU DEV 112' }).getByRole('button', { name: /Install TAU_DEV_112.zip/ }).innerText(), 'Reinstall'); ok('and its button reads Reinstall');
  assert.match(await page().locator('.b-row', { hasText: 'Weird' }).innerText(), /1 error/); ok('core-check errors show on the row');
  assert.match(await page().locator('.b-row', { hasText: 'broken.zip' }).innerText(), /This zip has no core\.json/); assert.equal(await page().locator('.b-row', { hasText: 'broken.zip' }).getByRole('button', { name: /Install/ }).count(), 0); ok('an unreadable build explains why and cannot be installed');

  // ---- install sheet
  await row113.getByRole('button', { name: /Install TAU_DEV_113.zip/ }).click(); await p.waitForTimeout(500);
  const sheet = p.getByRole('dialog', { name: /TAU DEV 113/ });
  assert.equal(await sheet.count(), 1); assert.match(await sheet.innerText(), /INSTALL A BUILD[\s\S]*Review the update/i); ok('Install opens the same reviewed update plan in a sheet');
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  assert.equal(await p.getByRole('dialog').count(), 0); ok('Escape closes it');

  // ---- remove older
  await row113.getByRole('button', { name: /Remove 2 older builds/ }).click(); await p.waitForTimeout(500);
  const rm = p.getByRole('dialog', { name: /older builds of TAU DEV 113/ });
  assert.match(await rm.innerText(), /music, library and settings are kept[\s\S]*copied to a backup folder[\s\S]*alfatreze\.TAU DEV 111[\s\S]*alfatreze\.TAU DEV 112/i); ok('removing older builds lists them and says what is kept and backed up');
  await rm.getByRole('button', { name: 'Cancel' }).click(); await p.waitForTimeout(200);
  await row113.getByRole('button', { name: /Remove 2 older builds/ }).click(); await p.waitForTimeout(400);
  await p.getByRole('dialog').getByRole('button', { name: /Remove 2 builds/ }).click(); await p.waitForTimeout(700);
  assert.match(await page().innerText(), /2 older builds removed\. Their music, library and settings were kept/); ok('confirming removes them and says so');

  // ---- naming schemes
  await page().locator('summary').click(); await p.waitForTimeout(200);
  assert.match(await page().locator('.b-schemes').innerText(), /Name and number \(TAU DEV 12\)[\s\S]*SemVer pre-release[\s\S]*SemVer \(0\.6\.0\)/); ok('the built-in schemes are listed');
  await page().getByRole('button', { name: 'Add a scheme' }).click(); await p.waitForTimeout(150);
  const tpl = page().getByLabel('Template for My scheme');
  await tpl.fill('{series}{n}'); await page().getByLabel(/Try a shortname/).fill('TAU 12'); await p.waitForTimeout(300);
  assert.match(await page().locator('.b-try').innerText(), /Two tokens touch/); ok('a bad template explains itself as you type');
  await page().getByRole('button', { name: 'Save schemes' }).click(); await p.waitForTimeout(300);
  assert.match(await page().locator('.b-snote').innerText(), /Two tokens touch/); ok('and cannot be saved');
  await page().getByLabel('Reads').selectOption('file'); await tpl.fill('{series}-nightly-{n}');
  await page().getByLabel(/Try a file name/).fill('Pocket-nightly-0042'); await p.waitForTimeout(300);
  assert.match(await page().locator('.b-try').innerText(), /Series “pocket”, number 42/); ok('a good template shows what it reads from a sample name');
  await page().getByLabel(/Try a file name/).fill('something else'); await p.waitForTimeout(300);
  assert.match(await page().locator('.b-try').innerText(), /No match/); ok('a name it does not match says so');
  await page().getByRole('button', { name: 'Save schemes' }).click(); await p.waitForTimeout(500);
  assert.match(await page().locator('.b-snote').innerText(), /Saved/); assert.match(await page().locator('summary').innerText(), /1 of yours and 3 built in/); ok('a valid scheme saves and is counted');
  await page().getByRole('button', { name: 'Remove scheme My scheme' }).click(); await page().getByRole('button', { name: 'Save schemes' }).click(); await p.waitForTimeout(400);
  assert.match(await page().locator('summary').innerText(), /^Naming schemes\s*3 built in/); ok('a scheme can be removed');

  // ---- carry media from the previous build
  await page().locator('.b-row', { hasText: 'TAU DEV 110' }).getByRole('button', { name: /Install TAU_DEV_110/ }).click(); await p.waitForTimeout(500);
  const sheet3 = p.getByRole('dialog', { name: /TAU DEV 110/ });
  assert.equal(await sheet3.getByText('Carry media from the previous build').count(), 0); ok('nothing about carrying media is offered before the install');
  await sheet3.getByRole('button', { name: 'Review update' }).click(); await p.waitForTimeout(600);
  await sheet3.getByLabel('Type the card name to confirm').fill('Pocket');
  await sheet3.getByRole('button', { name: 'Confirm and install' }).click(); await p.waitForTimeout(800);
  const carry = sheet3.getByRole('region', { name: 'Carry media from the previous build' });
  assert.equal(await carry.count(), 1); ok('after the install the sheet offers to carry media from the older builds');
  const items = carry.locator('.cm-list li');
  assert.equal(await items.count(), 3); assert.match(await carry.innerText(), /nothing already on the card is overwritten/i); ok('it lists what each older build kept, and says nothing is overwritten');
  assert.match(await items.nth(1).innerText(), /Same library[\s\S]*nothing to carry/i); assert.equal(await items.nth(1).locator('button').count(), 0); ok('a build that reads the same library has nothing to copy');
  await items.nth(0).getByRole('button', { name: /Review carrying files from TAU DEV 111/ }).click(); await p.waitForTimeout(500);
  assert.match(await items.nth(0).innerText(), /2 to copy \([\d.]+ MB\) · 1 already there · 1 different, kept as they are/); ok('reviewing a folder shows what is new, already there, and different (kept)');
  await items.nth(0).getByRole('button', { name: /Copy 2 files/ }).click(); await p.waitForTimeout(600);
  assert.match(await items.nth(0).innerText(), /Copied 2 files[\s\S]*each checked after copying[\s\S]*1 file were already there[\s\S]*1 file differ from the old build's and were left alone/); ok('copying reports what it copied and what it left alone');
  await items.nth(2).getByRole('button', { name: /Review carrying the library from TAU DEV 105/ }).click(); await p.waitForTimeout(500);
  assert.match(await items.nth(2).innerText(), /120 to copy, 0 to update[\s\S]*index is rebuilt for the new root/); ok('a library in another platform is reviewed with the index-aware copy');
  await items.nth(2).getByRole('button', { name: 'Copy the library' }).click(); await p.waitForTimeout(600);
  assert.match(await items.nth(2).innerText(), /Copied 120 files, verified, and rebuilt the index/); ok('and copying it rebuilds the index');
  await sheet3.getByRole('button', { name: 'Close', exact: true }).click(); await p.waitForTimeout(300);

  // ---- the core check gate before an install
  await page().locator('.b-row', { hasText: 'Weird' }).getByRole('button', { name: /Install weird.zip/ }).click(); await p.waitForTimeout(500);
  const sheet2 = p.getByRole('dialog', { name: /Weird/ });
  await sheet2.getByRole('button', { name: 'Review update' }).click(); await p.waitForTimeout(700);
  assert.match(await sheet2.innerText(), /Core check[\s\S]*1 error found[\s\S]*bitstream file bitstream\.rbf_r is missing/i); ok('a developer install runs the core check and shows what it found');
  const confirmBtn = sheet2.getByRole('button', { name: 'Confirm and install' });
  await sheet2.getByLabel('Type the card name to confirm').fill('Pocket');
  assert.equal(await confirmBtn.isDisabled(), true); assert.match(await sheet2.innerText(), /The core check found 1 error/); ok('an error blocks the install even with the card name typed');
  await sheet2.getByRole('checkbox', { name: /Install anyway/ }).check();
  assert.equal(await confirmBtn.isDisabled(), false); ok('ticking Install anyway lets it go ahead');
  await confirmBtn.click(); await p.waitForTimeout(700);
  await sheet2.getByRole('button', { name: 'Close', exact: true }).click(); await p.waitForTimeout(300);
  await p.locator('nav button', { hasText: /^Results$/ }).click(); await p.waitForTimeout(500);
  await p.getByRole('tab', { name: 'Run notes' }).click(); await p.waitForTimeout(500);
  assert.match(await p.locator('article.rn').first().innerText(), /Installed over 1 core check error: L09: The bitstream file bitstream\.rbf_r is missing or empty\./); ok('the override is recorded in the run note');

  await p.locator('nav button', { hasText: /^Builds$/ }).click(); await p.waitForTimeout(500); await page().locator('summary').click(); await p.waitForTimeout(150);
  // ---- accessibility basics
  const audit = await p.evaluate(() => { const name = (el) => (el.getAttribute('aria-label') || el.innerText || el.labels?.[0]?.innerText || '').trim(); const bad = []; for (const el of document.querySelectorAll('.builds button, .builds input, .builds select')) if (!name(el)) bad.push(el.outerHTML.slice(0, 80)); const ids = [...document.querySelectorAll('[id]')].map((e) => e.id); return { bad, dup: ids.filter((x, i) => ids.indexOf(x) !== i) }; });
  assert.deepEqual(audit.bad, []); assert.deepEqual(audit.dup, []); ok('every control on the page has a name and ids are unique');

  assert.deepEqual(errors, [], errors.join(' | ')); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
