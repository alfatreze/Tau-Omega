// Behaviour checks for the developer Builder (raw bitstream + template core -> package) against the dev mock.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/builder-test.cjs   (URL=... to point elsewhere)
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
  const start = async (dev) => { await p.goto(url); await p.evaluate((d) => { localStorage.clear(); if (d) localStorage.setItem('tau.mock.devTools', '1'); }, dev); await p.goto(url); await p.waitForTimeout(1200); };
  const page = () => p.locator('.builder');
  const checkBtn = () => page().getByRole('button', { name: /Check the build/ });

  await start(false);
  assert.equal(await p.locator('nav button', { hasText: /^Builder$/ }).count(), 0); ok('with developer tools off there is no Builder page in the menu');
  await start(true);
  await p.locator('nav button', { hasText: /^Builder$/ }).click(); await p.waitForTimeout(500);
  assert.match(await page().innerText(), /Nothing is written to a card[\s\S]*Bitstream[\s\S]*Template core[\s\S]*This build/i); ok('the page explains itself and shows the steps');
  assert.equal(await checkBtn().isDisabled(), true); ok('nothing can be checked before the inputs are given');

  await page().getByLabel(/The raw \.rbf Quartus wrote/).fill('/q/core.sof');
  await page().getByLabel('Core on the open card').selectOption('Spiritualized.GBA'); await p.waitForTimeout(500);
  assert.equal(await page().getByLabel('Shortname', { exact: true }).inputValue(), 'GBA DEV 1'); assert.equal(await page().getByLabel('Version', { exact: true }).inputValue(), '0.6.0-alpha.3'); ok('choosing a template fills the shortname and version from it');
  assert.match(await page().locator('.bd-info').first().innerText(), /Template: GBA DEV 1 · Spiritualized\.GBA/); ok('and says which template it is');
  await checkBtn().click(); await p.waitForTimeout(500);
  assert.match(await page().locator('.bd-err').innerText(), /JTAG programming file/); ok('a .sof is refused with the reason');
  await page().getByLabel(/The raw \.rbf Quartus wrote/).fill('/q/core.rbf_r'); await checkBtn().click(); await p.waitForTimeout(400);
  assert.match(await page().locator('.bd-err').innerText(), /already a converted bitstream[\s\S]*converting twice/i); ok('an already converted file is refused with the reason');

  await page().getByLabel(/The raw \.rbf Quartus wrote/).fill('/q/core.rbf'); await checkBtn().click(); await p.waitForTimeout(500);
  const plan = page().locator('article.bd-plan');
  assert.match(await plan.innerText(), /alfatreze\.GBA DEV 1[\s\S]*3 files[\s\S]*raw aaaaaaaaaaaa… → converted bbbbbbbbbbbb…/); ok('a good input shows the new core id, the file count and both hashes');
  const files = await plan.locator('.bd-files li').allInnerTexts();
  assert.equal(files.length, 3); assert.match(files[2], /bitstream\.rbf_r[\s\S]*bitstream \(converted\)/); assert.match(files[0], /core\.json[\s\S]*updated/); ok('the package lists each file and where it came from');
  assert.match(await plan.innerText(), /Core check[\s\S]*1 warning/i); assert.equal(await page().getByRole('button', { name: 'Build the package' }).count(), 1); ok('the core check runs, and a warning does not stop the build');

  await page().getByRole('button', { name: 'Use the next name' }).click(); await p.waitForTimeout(300);
  assert.equal(await page().getByLabel('Shortname', { exact: true }).inputValue(), 'GBA DEV 2'); assert.match(await page().locator('.bd-note').innerText(), /Next name by “Name and number/); ok('Next name counts on by the naming scheme');
  assert.equal(await page().getByRole('button', { name: 'Build the package' }).count(), 0); ok('changing a field asks for a fresh check before building');
  await page().getByRole('button', { name: 'Use the next version' }).click(); assert.equal(await page().getByLabel('Version', { exact: true }).inputValue(), '0.6.0-alpha.4'); ok('Next counts the version too');
  await page().getByLabel(/^Description/).fill('x'.repeat(71));
  assert.match(await page().locator('label', { hasText: 'Description' }).innerText(), /71\/63/); assert.equal(await page().locator('small.over').count(), 1); ok('the description counter turns red past 63 characters');
  await page().getByLabel(/^Description/).fill('A test build');

  await page().getByLabel(/Icon: a 36/).fill('/q/icon.png');
  await page().getByRole('button', { name: 'Add a file' }).click();
  await page().getByLabel('Source of extra file 1').fill('/q/boot.rom'); await page().getByLabel('Destination of extra file 1').fill('Assets/tau/{id}/boot.rom');
  await checkBtn().click(); await p.waitForTimeout(500);
  const f2 = await page().locator('article.bd-plan .bd-files li').allInnerTexts();
  assert.ok(f2.some((t) => /icon\.bin[\s\S]*new icon/.test(t)) && f2.some((t) => /Assets\/tau\/alfatreze\.GBA DEV 2\/boot\.rom[\s\S]*extra file/.test(t))); ok('an icon and an extra file appear in the package, the id filled in');

  await page().getByLabel('Shortname', { exact: true }).fill('bad one'); await checkBtn().click(); await p.waitForTimeout(500);
  assert.match(await page().locator('article.bd-plan .bd-err').innerText(), /core check found an error: audio\.json is missing/i); assert.equal(await page().getByRole('button', { name: 'Build the package' }).count(), 0); ok('a core-check error blocks the build and says what to fix');

  await page().getByLabel(/Build into/).fill('/Users/dev/builds');
  await page().getByLabel('Shortname', { exact: true }).fill('dup one'); await checkBtn().click(); await p.waitForTimeout(500);
  await page().getByRole('button', { name: 'Build the package' }).click(); await p.waitForTimeout(500);
  assert.match(await page().locator('.bd-err').first().innerText(), /already exists in that folder[\s\S]*never overwritten/i); ok('a build is never overwritten: the error says so');
  await page().getByLabel('Shortname', { exact: true }).fill('GBA DEV 5'); await checkBtn().click(); await p.waitForTimeout(500);
  await page().getByRole('button', { name: 'Build the package' }).click(); await p.waitForTimeout(600);
  const done = page().getByRole('status', { name: 'Build finished' });
  assert.match(await done.innerText(), /Built alfatreze\.GBA DEV 5[\s\S]*\/Users\/dev\/builds\/alfatreze\.GBA DEV 5\.zip[\s\S]*SHA-256[\s\S]*Build record[\s\S]*\.build\.json/); ok('building shows the package, its hash and the build record');
  await done.getByRole('button', { name: 'Show it in Builds' }).click(); await p.waitForTimeout(500);
  assert.equal(await p.locator('.builds').count(), 1); ok('and can open the Builds page');

  await p.locator('nav button', { hasText: /^Builder$/ }).click(); await p.waitForTimeout(400);
  const audit = await p.evaluate(() => { const name = (el) => (el.getAttribute('aria-label') || el.innerText || el.labels?.[0]?.innerText || '').trim(); const bad = []; for (const el of document.querySelectorAll('.builder button, .builder input, .builder select')) if (!name(el)) bad.push(el.outerHTML.slice(0, 80)); const ids = [...document.querySelectorAll('[id]')].map((e) => e.id); return { bad, dup: ids.filter((x, i) => ids.indexOf(x) !== i) }; });
  assert.deepEqual(audit.bad, []); assert.deepEqual(audit.dup, []); ok('every control has a name and ids are unique');
  assert.deepEqual(errors, [], errors.join(' | ')); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
