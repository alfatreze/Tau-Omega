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
  assert.match(await p.locator('.ov-card', { hasText: 'Game Boy Advance' }).innerText(), /2 cores · 1 disabled/); ok('the platform still counts the disabled core, and says how many are disabled');

  const bar = p.locator('.dis-bar');
  assert.match(await bar.innerText(), /1 core is disabled on the Pocket[\s\S]*hidden from its menu but still listed here, tagged/i); ok('the overview opens with a bar saying how many cores are disabled');
  assert.equal(await bar.getByRole('button', { name: 'Show them' }).count(), 1); assert.equal(await bar.getByRole('button', { name: 'Enable all' }).count(), 1); ok('with Show them and Enable all');
  await p.locator('nav .nav-group', { hasText: /^Card/ }).count();
  // ---- a disabled core stays in its place, tagged, and can still be fixed
  const tree = p.getByRole('tree', { name: 'Cores by platform' });
  await tree.getByRole('button', { name: 'Expand Game Boy Advance' }).click(); await p.waitForTimeout(200);
  const gbaRow = tree.locator('.nt-core', { hasText: 'GBA' }).filter({ hasNotText: 'GBAlt' });
  assert.equal(await gbaRow.locator('.nt-off').count(), 1); assert.equal(await gbaRow.getAttribute('class').then((c) => /off/.test(c)), true); ok('in the menu the disabled core stays under its platform, tagged off and dimmed');
  assert.equal(await gbaRow.locator('.nt-dot').count(), 1); ok('and it still shows the dot for the file it needs');
  await p.locator('.ov-card', { hasText: 'Game Boy Advance' }).click(); await p.waitForTimeout(300);
  assert.match(await p.locator('.pv-open', { hasText: 'GBA' }).first().innerText(), /Disabled on the Pocket[\s\S]*Needs 1 file/); ok('the platform page tags it Disabled on the Pocket next to what it needs');
  assert.equal(await p.getByRole('button', { name: /Open GBA to add what it needs, then enable it/ }).count(), 1); ok('a disabled core that still needs a file offers Fix, then enable on its row');
  await p.locator('.pv-open', { hasText: 'GBA' }).first().click(); await p.waitForTimeout(600);
  const sw0 = p.getByRole('switch', { name: 'Visible on the Pocket' });
  assert.equal(await sw0.getAttribute('aria-checked'), 'false'); ok('its page shows the switch off');
  assert.match(await p.locator('.dis-banner').innerText(), /Disabled on the Pocket\.[\s\S]*still needs gba_bios\.bin[\s\S]*add it in Files/); ok('and says what it still needs, and where to add it');
  assert.match(await p.locator('.cores-scroll').innerText(), /gba_bios\.bin/); ok('the Files tab still works for a disabled core');
  assert.equal(await p.locator('.dis-banner').getByRole('button', { name: 'Enable on the Pocket' }).count(), 1); ok('the banner has an Enable on the Pocket button');
  await p.locator('.dis-banner').getByRole('button', { name: 'Enable on the Pocket' }).click(); await p.waitForTimeout(300);
  assert.match(await p.locator('.dis-warn').innerText(), /still lacks gba_bios\.bin/); await p.getByRole('button', { name: 'Cancel' }).click(); await p.waitForTimeout(150); ok('and it asks first while a required file is missing');

  await sw0.click(); await p.waitForTimeout(300);
  assert.match(await p.locator('.dis-warn').innerText(), /still lacks gba_bios\.bin[\s\S]*Enable it anyway/); ok('switching it on while it still lacks a file asks first');
  await p.getByRole('button', { name: 'Cancel' }).click(); await p.waitForTimeout(150);
  assert.equal(await p.locator('.dis-warn').count(), 0); assert.equal(await sw0.getAttribute('aria-checked'), 'false'); ok('Cancel leaves it disabled');

  // ---- enable again
  await p.locator('nav .nav-group', { hasText: 'Cores' }).click(); await p.waitForTimeout(150); await p.locator('nav .nav-group', { hasText: 'Cores' }).click(); await p.waitForTimeout(300);
  await p.locator('.dis-bar').getByRole('button', { name: 'Enable all' }).click(); await p.waitForTimeout(600);
  assert.match(await p.locator('.cores').innerText(), /1 core enabled again\. It will not be suggested for hiding again/); ok('enabling says it will not be suggested again');
  assert.equal(await p.locator('#ov-disabled').count(), 0); ok('the Disabled section goes when nothing is disabled');
  assert.doesNotMatch(await p.locator('.ov-card', { hasText: 'Game Boy Advance' }).innerText(), /disabled/); ok('and the platform no longer says anything is disabled');

  // ---- a disabled core with nothing missing can be enabled right from its platform row
  await p.locator('.ov-card', { hasText: 'Game Boy Advance' }).click(); await p.waitForTimeout(300);
  await p.locator('.pv-open', { hasText: 'GBAlt' }).click(); await p.waitForTimeout(500);
  await p.getByRole('switch', { name: 'Visible on the Pocket' }).click(); await p.waitForTimeout(600);
  await p.locator('nav .nav-group', { hasText: 'Cores' }).click(); await p.waitForTimeout(150); await p.locator('nav .nav-group', { hasText: 'Cores' }).click(); await p.waitForTimeout(250);
  await p.locator('.ov-card', { hasText: 'Game Boy Advance' }).click(); await p.waitForTimeout(300);
  await p.getByRole('button', { name: 'Enable GBAlt on the Pocket' }).click(); await p.waitForTimeout(600);
  assert.equal(await p.getByRole('button', { name: 'Enable GBAlt on the Pocket' }).count(), 0); assert.doesNotMatch(await p.locator('.pv-list').innerText(), /Disabled on the Pocket/); ok('the Enable button on a platform row turns the core back on');
  await p.locator('nav .nav-group', { hasText: 'Cores' }).click(); await p.waitForTimeout(150); await p.locator('nav .nav-group', { hasText: 'Cores' }).click(); await p.waitForTimeout(250);

  // ---- the header switch: it stays on the page
  await p.locator('.ov-card', { hasText: 'Game Boy Advance' }).click(); await p.waitForTimeout(300);
  await p.locator('.pv-open', { hasText: 'GBAlt' }).click(); await p.waitForTimeout(600);
  const sw = p.getByRole('switch', { name: 'Visible on the Pocket' });
  assert.equal(await sw.count(), 1); assert.equal(await sw.getAttribute('aria-checked'), 'true'); ok('a core page has the Visible on the Pocket switch, on');
  await sw.click(); await p.waitForTimeout(700);
  assert.match(await p.locator('.cores h1').innerText(), /GBAlt/); ok('turning it off keeps you on the core page');
  assert.equal(await sw.getAttribute('aria-checked'), 'false'); assert.match(await p.locator('.dis-banner').innerText(), /Switch “Visible on the Pocket” on to bring it back/); ok('the switch shows off and the page says it is disabled');
  assert.match(await p.locator('.cores-note').innerText(), /now hidden from the Pocket\. It stays here/); ok('and that it stays here');
  await sw.click(); await p.waitForTimeout(700);
  assert.equal(await sw.getAttribute('aria-checked'), 'true'); assert.equal(await p.locator('.dis-banner').count(), 0); ok('a core with nothing missing switches straight back on');

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
