// Behaviour checks for hiding cores from the Pocket (dialog on card open, setting, header switch, Disabled section) against the dev mock.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/disable-test.cjs   (URL=... to point elsewhere)
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
  const start = async (mode) => { await p.goto(url); await p.evaluate((m) => { localStorage.clear(); if (m) localStorage.setItem('tau.mock.coreDisable', m); }, mode); await p.goto(url); await p.waitForTimeout(1200); };
  const dlg = () => p.getByRole('dialog', { name: /Hide .* from the Pocket/ });
  const cores = () => p.locator('nav .nav-group', { hasText: 'Cores' });
  const overview = async () => { await cores().click(); await p.waitForTimeout(200); await cores().click(); await p.waitForTimeout(300); };

  // ---- default: never suggest
  await start(null);
  assert.equal(await dlg().count(), 0); ok('with the default setting the dialog does not appear');
  await p.getByRole('button', { name: /Tools & settings/ }).click(); await p.getByRole('button', { name: 'Settings', exact: true }).click(); await p.waitForTimeout(500);
  const sel = p.getByRole('combobox', { name: 'What to do about cores the Pocket cannot use' });
  assert.equal(await sel.count(), 1); assert.equal(await sel.inputValue(), 'never'); ok('Settings has the row, set to never suggest in this mock');
  await sel.selectOption('ask'); await p.waitForTimeout(300);
  assert.equal(await sel.inputValue(), 'ask'); ok('the choice saves');
  assert.deepEqual(await sel.locator('option').allInnerTexts(), ['Ask me when I open a card', 'Hide them automatically', 'Never suggest hiding']); ok('the three choices read in plain words');

  // ---- ask: the dialog
  await start('ask');
  assert.equal(await dlg().count(), 1); ok('opening a card with unusable cores asks about them');
  const d = dlg();
  assert.match(await d.innerText(), /Hide 2 cores[\s\S]*Nothing is deleted[\s\S]*Disabled Cores/); ok('it says how many, and that nothing is deleted and where they go');
  assert.match(await d.innerText(), /GBA[\s\S]*gba_bios\.bin[\s\S]*Put gba_bios\.bin in Assets\/gba\/common[\s\S]*Thing[\s\S]*no bitstream/); ok('each core shows why, and what would fix it');
  assert.equal(await d.locator('input[type=checkbox]:checked').count(), 2); ok('every core starts ticked');
  assert.match(await d.innerText(), /needs no files, or has a quirk[\s\S]*untick/i); ok('it explains how to keep one with a quirk the checks cannot see');
  assert.equal(await d.getByRole('button', { name: /Hide 2 cores/ }).count(), 1); ok('the button counts what will be hidden');
  await d.getByRole('checkbox', { name: /Thing/ }).uncheck();
  assert.equal(await d.getByRole('button', { name: /Hide 1 core$/ }).count(), 1); ok('unticking one updates the button');
  await d.getByRole('checkbox', { name: /Don’t remind me again/ }).check();
  await d.getByRole('button', { name: /Hide 1 core$/ }).click(); await p.waitForTimeout(700);
  assert.equal(await dlg().count(), 0); ok('hiding closes the dialog');
  await overview();
  const text = await p.locator('.cores').innerText();
  assert.match(text, /Disabled[\s\S]*1 core hidden from the Pocket[\s\S]*GBA[\s\S]*missing a file|gba_bios/i); ok('the overview lists it under Disabled with the reason');
  assert.match(await p.locator('.ov-cat:not(.dis)').allInnerTexts().then((a) => a.join('\n')), /No platform\s*1 core/); ok('the core left in place is still listed');
  assert.equal(await p.locator('.ov-card', { hasText: 'Game Boy Advance' }).innerText().then((t) => /1 core/.test(t)), true); ok('the platform now counts one core (GBAlt)');

  // ---- enable again
  await p.getByRole('button', { name: 'Enable GBA' }).click(); await p.waitForTimeout(600);
  assert.match(await p.locator('.cores').innerText(), /1 core enabled again\. It will not be suggested for hiding again/); ok('enabling says it will not be suggested again');
  assert.equal(await p.locator('#ov-disabled').count(), 0); ok('the Disabled section goes when nothing is disabled');
  assert.equal(await p.locator('.ov-card', { hasText: 'Game Boy Advance' }).innerText().then((t) => /2 cores/.test(t)), true); ok('the core is back on its platform');

  // ---- the header switch
  await p.locator('.ov-card', { hasText: 'Game Boy Advance' }).click(); await p.waitForTimeout(300);
  await p.locator('.pv-open', { hasText: 'GBAlt' }).click(); await p.waitForTimeout(600);
  const sw = p.getByRole('switch', { name: 'Visible on the Pocket' });
  assert.equal(await sw.count(), 1); assert.equal(await sw.getAttribute('aria-checked'), 'true'); ok('a core page has the Visible on the Pocket switch, on');
  await sw.click(); await p.waitForTimeout(700);
  assert.match(await p.locator('.cores h1').innerText(), /Cores/); ok('turning it off leaves the core page');
  assert.match(await p.locator('.cores').innerText(), /Disabled[\s\S]*GBAlt/); ok('the core is listed under Disabled');
  await p.getByRole('button', { name: 'Enable GBAlt' }).click(); await p.waitForTimeout(500);
  assert.equal(await p.locator('#ov-disabled').count(), 0); ok('and can be enabled from there');

  // ---- not now
  await start('ask');
  await dlg().getByRole('button', { name: 'Not now' }).click(); await p.waitForTimeout(300);
  assert.equal(await dlg().count(), 0); await overview();
  assert.equal(await p.locator('#ov-disabled').count(), 0); ok('Not now closes the dialog and changes nothing');

  // ---- escape and automatic
  await start('ask'); await p.keyboard.press('Escape'); await p.waitForTimeout(300);
  assert.equal(await dlg().count(), 0); ok('Escape means not now');
  await start('auto');
  assert.equal(await dlg().count(), 0); assert.match(await p.locator('.dc-auto').innerText(), /2 cores the Pocket could not use were hidden[\s\S]*Cores › Disabled/); ok('automatic mode hides them without asking and says where to undo it');
  await overview();
  assert.equal(await p.locator('.dis-row').count(), 2); await p.getByRole('button', { name: 'Enable all' }).click(); await p.waitForTimeout(500);
  assert.equal(await p.locator('#ov-disabled').count(), 0); ok('Enable all brings every one back');

  assert.deepEqual(errors, [], errors.join(' | ')); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
