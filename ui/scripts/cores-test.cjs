// Behaviour checks for the Cores section (menu tree, platform and core pages, Files with drag and drop, Saves, Screenshots, Settings) against the dev mock.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/cores-test.cjs   (URL=... to point elsewhere)
// Exits non-zero on the first failed expectation.
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
  await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(1100);

  const tree = p.getByRole('tree', { name: 'Cores by platform' });
  const platform = (name) => tree.locator('.nt-item[data-kind="platform"]', { hasText: name });
  const core = (name) => tree.locator('.nt-core').filter({ has: p.locator('.nt-name', { hasText: new RegExp(`^${name}$`) }) });
  const tab = (name) => p.getByRole('tab', { name });
  const panel = () => p.getByRole('tabpanel');

  // ---- the menu tree
  assert.match(await p.locator('nav .nav-group', { hasText: 'Cores' }).innerText(), /cores\s*5/i); ok('the Cores menu entry shows how many cores the card has');
  assert.ok(await p.locator('nav .nav-group .nt-dot').count() === 1); ok('it carries a dot when a core needs a file');
  assert.deepEqual(await tree.locator('.nt-item[data-kind="platform"] .nt-name').allInnerTexts(), ['Amiga', 'Game Boy', 'Game Boy Advance', 'No platform']); ok('platforms are listed by name, with "No platform" last');
  assert.equal(await tree.locator('.nt-core').count(), 0); ok('platforms start collapsed');
  await tree.getByRole('button', { name: 'Expand Game Boy Advance' }).click(); await p.waitForTimeout(150);
  assert.deepEqual(await tree.locator('.nt-core .nt-name').allInnerTexts(), ['GBA', 'GBAlt']); ok('expanding a platform lists its cores');
  assert.ok(await core('GBA').locator('.nt-dot').count() === 1 && await core('GBAlt').locator('.nt-dot').count() === 0); ok('only the core that needs a file has the dot');

  // ---- platform page
  await platform('Game Boy Advance').click(); await p.waitForTimeout(400);
  assert.match(await p.locator('.cores h1').innerText(), /Game Boy Advance/); assert.match(await p.locator('.cores').innerText(), /2 cores on this card/); ok('choosing a platform shows all its cores');
  assert.match(await p.locator('.pv-list').innerText(), /GBA[\s\S]*Needs 1 file[\s\S]*GBAlt/); ok('the platform page marks the core that needs a file');

  // ---- core page: tabs, inspector
  await p.locator('.pv-open', { hasText: 'Spiritualized.GBA' }).click(); await p.waitForTimeout(900);
  assert.deepEqual(await p.getByRole('tab').allInnerTexts(), ['Files', 'Saves', 'Screenshots', 'Settings']); ok('a core opens with Files, Saves, Screenshots and Settings tabs');
  assert.equal(await tab('Files').getAttribute('aria-selected'), 'true'); ok('Files is the tab it opens on');
  assert.match(await p.getByRole('complementary', { name: /GBA/ }).innerText(), /requirements[\s\S]*Pocket firmware\s*1\.1 or newer[\s\S]*Dock\s*Yes[\s\S]*on this card[\s\S]*Games\s*6 files/i); ok('the details panel shows the core, its requirements and what is on the card');
  await p.getByRole('button', { name: /Hide details/ }).click(); await p.waitForTimeout(150);
  assert.equal(await p.locator('.ci').count(), 0); assert.equal(await p.evaluate(() => localStorage.getItem('tau.cores.inspector')), '0'); ok('the details panel can be hidden and the choice is remembered');
  await p.keyboard.press('i'); await p.waitForTimeout(150);
  assert.equal(await p.locator('.ci').count(), 1); ok('the I key shows it again');
  await tab('Files').focus(); await p.keyboard.press('ArrowRight');
  assert.equal(await tab('Saves').getAttribute('aria-selected'), 'true'); await p.keyboard.press('End'); assert.equal(await tab('Settings').getAttribute('aria-selected'), 'true'); await p.keyboard.press('Home'); ok('arrow keys, Home and End move between tabs');
  await tab('Files').click();

  // ---- Files
  assert.match(await panel().innerText(), /Needs attention[\s\S]*gba_bios\.bin[\s\S]*Assets\/gba\/common/); ok('a missing BIOS is listed with its exact name and folder');
  assert.match(await panel().innerText(), /Shared by every Game Boy Advance core[\s\S]*Only GBA/); ok('both asset folders are shown, shared and core-only');
  assert.equal(await panel().locator('.cf-row').count(), 6); ok('the ROMs are listed');
  await panel().getByPlaceholder('Search these files').fill('boktai'); await p.waitForTimeout(150);
  assert.equal(await panel().locator('.cf-row').count(), 1); await panel().getByPlaceholder('Search these files').fill(''); ok('search narrows a folder');
  assert.match(await panel().innerText(), /Nothing only for this core\. Drop ROMs/); ok('an empty folder invites a drop');

  // ---- drag and drop (browser mock: File objects, paths are /mock/<name>)
  const drop = async (names, zoneSelector) => {
    const dt = await p.evaluateHandle((n) => { const d = new DataTransfer(); for (const x of n) d.items.add(new File(['x'], x)); return d; }, names);
    const box = zoneSelector ? await p.locator(zoneSelector).first().boundingBox() : null;
    const pt = box ? { clientX: box.x + box.width / 2, clientY: box.y + box.height / 2 } : { clientX: 2, clientY: 2 };
    await p.dispatchEvent('body', 'dragover', { dataTransfer: dt, ...pt }); await p.waitForTimeout(80);
    return { dt, pt };
  };
  let d = await drop(['Metroid Fusion 2 (USA).gba']);
  assert.ok(await p.locator('.cf-hint').count() >= 1); ok('dragging files over the page lights up the folders');
  await p.dispatchEvent('body', 'drop', { dataTransfer: d.dt, ...d.pt }); await p.waitForTimeout(500);
  const sheet = p.locator('.as'); // the title changes from "Add to GBA" to "Added" once it has run
  assert.equal(await sheet.count(), 1); assert.match(await sheet.getByRole('heading').innerText(), /Add to GBA/); assert.match(await sheet.innerText(), /1 new file[\s\S]*Assets\/gba\/common/); ok('dropping a file opens a review of what would be added, nothing is written yet');
  assert.equal(await sheet.getByRole('radio', { name: /Shared by every/ }).isChecked(), true); ok('the shared folder is the default destination');
  await sheet.getByRole('radio', { name: /Only GBA/ }).check(); await p.waitForTimeout(300);
  assert.match(await sheet.innerText(), /Assets\/gba\/Spiritualized\.GBA/); await sheet.getByRole('radio', { name: /Shared by every/ }).check(); await p.waitForTimeout(300); ok('the destination can be switched in the review');
  await sheet.getByRole('button', { name: /^Add 1 file$/ }).click(); await p.waitForTimeout(500);
  assert.match(await sheet.innerText(), /1 file added and verified/); ok('adding copies and verifies, then says so');
  await sheet.getByRole('button', { name: 'Done' }).click(); await p.waitForTimeout(400);
  assert.equal(await panel().locator('.cf-row').count(), 7); ok('the new file shows up in the list straight away');
  assert.equal(await sheet.count(), 0);

  d = await drop(['Mother 3.gba', 'notes.xyz', 'bad:name.gba']);
  await p.dispatchEvent('body', 'drop', { dataTransfer: d.dt, ...d.pt }); await p.waitForTimeout(500);
  const text = await sheet.innerText();
  assert.match(text, /already there/); assert.match(text, /No file slot of this core reads \.xyz/); assert.match(text, /Not added: the name has a character the card cannot hold/); ok('an identical file, an unknown extension and a bad name are each explained');
  await sheet.getByRole('button', { name: 'Cancel' }).click(); await p.waitForTimeout(200);

  d = await drop(['gba_bios.bin']);
  await p.dispatchEvent('body', 'drop', { dataTransfer: d.dt, ...d.pt }); await p.waitForTimeout(500);
  await sheet.getByRole('button', { name: /^Add 1 file$/ }).click(); await p.waitForTimeout(500);
  await sheet.getByRole('button', { name: 'Done' }).click(); await p.waitForTimeout(500);
  assert.equal(await panel().getByText('Needs attention').count(), 0); assert.equal(await p.locator('.tab-dot').count(), 0); ok('dropping the BIOS clears the needs-attention list');
  assert.equal(await tree.locator('.nt-core .nt-dot').count(), 0); ok('and the dot leaves the menu');

  // ---- Saves
  await tab('Saves').click(); await p.waitForTimeout(200);
  assert.match(await panel().innerText(), /Boktai 2 \(USA\)[\s\S]*Mother 3[\s\S]*Save states[\s\S]*Boktai 2 \(USA\)/); ok('saves and save states are listed with their game names');
  assert.match(await panel().innerText(), /1 Pocket Sync save backup[\s\S]*not listed here/); ok('a Pocket Sync backup zip is counted, not mistaken for a save');

  // ---- Screenshots
  await tab('Screenshots').click(); await p.waitForTimeout(700);
  assert.match(await panel().innerText(), /does not record which core took a screenshot/); ok('the tab says the Pocket does not record which core took a screenshot');
  assert.equal(await p.getByRole('button', { name: /Likely this core/ }).getAttribute('aria-pressed'), 'true'); assert.equal(await panel().locator('.ct-shot').count(), 5); ok('it opens on the screenshots that are likely this core');
  await p.getByRole('button', { name: /All screenshots/ }).click(); await p.waitForTimeout(500);
  assert.equal(await panel().locator('.ct-shot').count(), 24); assert.ok(await panel().locator('.ct-shot img').count() >= 4); ok('all screenshots are one click away, with thumbnails');
  await panel().locator('.ct-shot').first().click(); await p.waitForTimeout(500);
  assert.equal(await p.getByRole('dialog', { name: /Screenshot taken/ }).count(), 1); await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  assert.equal(await p.getByRole('dialog', { name: /Screenshot taken/ }).count(), 0); ok('a screenshot opens large and Escape closes it');

  // ---- Settings
  await tab('Settings').click(); await p.waitForTimeout(500);
  const st = await panel().innerText();
  assert.match(st, /Needs attention[\s\S]*Nothing is missing/); assert.match(st, /High Quality Audio\s*Off\s*\(default On\)/); assert.match(st, /Rumble Strength\s*6\s*\(default 5\)/); assert.match(st, /Edge Masking\s*Auto Masking/); ok('saved settings are decoded with the core\'s own names and show the default');
  assert.match(st, /Core menu[\s\S]*Controls/); assert.match(st, /12 per-game presets in Presets\/Spiritualized\.GBA/); ok('saved files and presets are summarised');
  assert.match(st, /What this core reads[\s\S]*Cartridge[\s\S]*\.gba \.rom[\s\S]*required[\s\S]*BIOS[\s\S]*gba_bios\.bin/); ok('the core\'s data slots are listed');

  // ---- keyboard in the menu tree
  await core('GBA').focus(); await p.keyboard.press('ArrowDown'); assert.match(await p.evaluate(() => document.activeElement.textContent), /GBAlt/);
  await p.keyboard.press('ArrowLeft'); assert.match(await p.evaluate(() => document.activeElement.textContent), /Game Boy Advance/);
  await p.keyboard.press('ArrowLeft'); await p.waitForTimeout(150);
  assert.equal(await tree.locator('.nt-core').count(), 0); await p.keyboard.press('ArrowRight'); await p.waitForTimeout(150);
  assert.equal(await tree.locator('.nt-core').count(), 2); ok('arrow keys move through the tree, collapse and expand a platform');

  // ---- no card, core with no files, overview
  await p.locator('nav .nav-group', { hasText: 'Cores' }).click(); await p.waitForTimeout(200); await p.locator('nav .nav-group', { hasText: 'Cores' }).click(); await p.waitForTimeout(300);
  assert.match(await p.locator('.cores').innerText(), /Every core on this card, by platform/); assert.equal(await p.locator('.ov-card').count(), 4); ok('the Cores entry itself opens an overview of every platform');
  await platform('No platform').click(); await p.waitForTimeout(300);
  await p.locator('.pv-open').first().click(); await p.waitForTimeout(700);
  assert.match(await panel().innerText(), /No files yet/); ok('a core with no platform opens without errors');

  // ---- a scripted accessibility pass: every control is named, no duplicate ids, roles line up
  const audit = await p.evaluate(() => {
    const name = (el) => (el.getAttribute('aria-label') || el.innerText || el.getAttribute('title') || el.labels?.[0]?.innerText || '').trim();
    const bad = [];
    for (const el of document.querySelectorAll('.cores button, .cores input, .cores select, .nt button, .nt input')) if (!name(el)) bad.push(el.outerHTML.slice(0, 80));
    const ids = [...document.querySelectorAll('[id]')].map((e) => e.id);
    const tabs = [...document.querySelectorAll('[role=tab]')];
    const wired = tabs.every((t) => !t.hasAttribute('aria-controls') || document.getElementById(t.getAttribute('aria-controls')) || t.getAttribute('aria-selected') === 'false');
    return { bad, dup: ids.filter((x, i) => ids.indexOf(x) !== i), items: document.querySelectorAll('[role=treeitem]').length, groups: document.querySelectorAll('.nt [role=group]').length, wired };
  });
  assert.deepEqual(audit.bad, []); assert.deepEqual(audit.dup, []); assert.ok(audit.items >= 5 && audit.wired); ok('every control in the Cores section has a name, ids are unique and the tree and tab roles line up');
  await p.setViewportSize({ width: 375, height: 800 }); await p.waitForTimeout(300);
  assert.ok(await p.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)); ok('at phone width the page does not scroll sideways');
  await p.setViewportSize({ width: 1440, height: 900 });

  assert.deepEqual(errors, [], errors.join(' | ')); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
