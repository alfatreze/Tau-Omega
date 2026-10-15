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
  assert.match(await p.locator('nav .nav-group', { hasText: 'Cores' }).innerText(), /cores\s*8/i); ok('the Cores menu entry shows how many cores the card has');
  assert.ok(await p.locator('nav .nav-group .nt-dot').count() === 1); ok('it carries a dot when a core needs a file');
  assert.deepEqual((await tree.locator('.nt-item[data-kind="category"] .nt-name').allInnerTexts()).map((t) => t.toLowerCase()), ['console', 'handheld', 'computer', 'media players', 'no platform']); ok('categories follow the Library order, with "No platform" last');
  assert.deepEqual(await tree.locator('.nt-item[data-kind="maker"] .nt-name').allInnerTexts(), ['Nintendo', 'Nintendo', 'Commodore', 'alfatreze']); ok('each category lists its manufacturers');
  assert.deepEqual(await tree.locator('.nt-item[data-kind="platform"] .nt-name').allInnerTexts(), ['SNES', 'Game Boy', 'Game Boy Advance', 'Amiga', 'TAU', 'No platform']); ok('platforms sit under their manufacturer (Console > Nintendo > SNES), Tau under Media Players');
  assert.deepEqual(await tree.locator('[data-kind="platform"]').evaluateAll((els) => els.map((e) => e.closest('li[role=treeitem]').getAttribute('aria-level'))), ['3','3','3','3','3','3']); ok('the tree has four levels');
  assert.equal(await tree.locator('.nt-core').count(), 0); ok('platforms start collapsed');
  await tree.getByRole('button', { name: 'Expand Game Boy Advance' }).click(); await p.waitForTimeout(150);
  assert.deepEqual(await tree.locator('.nt-core .nt-name').allInnerTexts(), ['GBA', 'GBAlt']); ok('expanding a platform lists its cores');
  await tree.getByRole('button', { name: 'Expand TAU' }).click(); await p.waitForTimeout(150);
  assert.deepEqual(await tree.locator('.nt-core .nt-name').allInnerTexts(), ['GBA', 'GBAlt', 'omega', 'player2']); ok('every Tau core is under the one TAU platform');
  await tree.getByRole('button', { name: 'Collapse TAU' }).click(); await p.waitForTimeout(100);
  await tree.getByRole('button', { name: 'Collapse Handheld' }).click(); await p.waitForTimeout(100);
  assert.equal(await tree.locator('[data-kind="platform"]').count(), 4); ok('a category can be collapsed with everything under it');
  await tree.getByRole('button', { name: 'Expand Handheld' }).click(); await p.waitForTimeout(100);
  assert.ok(await core('GBA').locator('.nt-dot').count() === 1 && await core('GBAlt').locator('.nt-dot').count() === 0); ok('only the core that needs a file has the dot');

  // ---- platform page
  await platform('Game Boy Advance').click(); await p.waitForTimeout(400);
  assert.match(await p.locator('.cores h1').innerText(), /Game Boy Advance/); assert.match(await p.locator('.cores').innerText(), /2 cores on this card/); ok('choosing a platform shows all its cores');
  assert.match(await p.locator('.pv-list').innerText(), /GBA[\s\S]*Needs 1 file[\s\S]*GBAlt/); ok('the platform page marks the core that needs a file');

  // ---- core page: tabs, inspector
  await p.locator('.pv-open', { hasText: 'Spiritualized.GBA' }).click(); await p.waitForTimeout(900);
  assert.deepEqual(await p.getByRole('tab').allInnerTexts(), ['Files', 'Saves', 'Screenshots', 'Settings']); ok('a core opens with Files, Saves, Screenshots and Settings tabs');
  assert.equal(await tab('Files').getAttribute('aria-selected'), 'true'); ok('Files is the tab it opens on');
  assert.match(await p.getByRole('complementary', { name: /GBA/ }).innerText(), /requirements[\s\S]*Pocket firmware\s*1\.1 or newer[\s\S]*Files needed[\s\S]*supported features[\s\S]*Dock\s*Yes[\s\S]*on this card[\s\S]*Games\s*6 files/i); ok('the details panel shows the core, its requirements and what is on the card');
  await p.getByRole('button', { name: /Hide details/ }).click(); await p.waitForTimeout(150);
  assert.equal(await p.locator('.ci').count(), 0); assert.equal(await p.evaluate(() => localStorage.getItem('tau.cores.inspector')), '0'); ok('the details panel can be hidden and the choice is remembered');
  await p.keyboard.press('i'); await p.waitForTimeout(150);
  assert.equal(await p.locator('.ci').count(), 1); ok('the I key shows it again');
  await tab('Files').focus(); await p.keyboard.press('ArrowRight');
  assert.equal(await tab('Saves').getAttribute('aria-selected'), 'true'); await p.keyboard.press('End'); assert.equal(await tab('Settings').getAttribute('aria-selected'), 'true'); await p.keyboard.press('Home'); ok('arrow keys, Home and End move between tabs');
  await tab('Files').click();

  // ---- Files
  assert.match(await panel().innerText(), /Needs attention[\s\S]*gba_bios\.bin[\s\S]*Assets\/gba\/common/); ok('a missing BIOS is listed with its exact name and folder');
  assert.match(await panel().innerText(), /Shared by every Game Boy Advance core/); assert.ok(!/Only GBA/.test(await panel().innerText())); ok('Files lists the games folder; a core-only folder with only BIOS and palettes is not shown here');
  assert.equal(await panel().locator('.cf-row').count(), 6); ok('the ROMs are listed');
  await panel().getByPlaceholder('Search these files').fill('boktai'); await p.waitForTimeout(150);
  assert.equal(await panel().locator('.cf-row').count(), 1); await panel().getByPlaceholder('Search these files').fill(''); ok('search narrows a folder');
  assert.match(await panel().innerText(), /drop files anywhere on this page/i); ok('the page invites a drop anywhere');

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

  // ---- removing files
  const rows = () => panel().locator('.cf-row');
  const before = await rows().count();
  await p.getByRole('checkbox', { name: 'Select Metroid Fusion 2 (USA).gba' }).check(); await p.waitForTimeout(150);
  await panel().getByRole('button', { name: /Remove 1 selected/ }).click(); await p.waitForTimeout(400);
  const rm = p.locator('.rs');
  assert.match(await rm.innerText(), /1 file[\s\S]*Metroid Fusion 2 \(USA\)\.gba[\s\S]*also read by 1 other core[\s\S]*copied to a backup folder outside the card/); ok('removing opens a review that names the file, who else reads it and the backup');
  await rm.getByRole('button', { name: 'Cancel' }).click(); await p.waitForTimeout(200);
  assert.equal(await rows().count(), before); ok('cancelling removes nothing');
  await panel().getByRole('button', { name: /Remove 1 selected/ }).click(); await p.waitForTimeout(400);
  await rm.getByRole('button', { name: /^Remove 1 file$/ }).click(); await p.waitForTimeout(500);
  assert.match(await rm.innerText(), /1 file removed[\s\S]*Copies are kept in/); ok('removing says where the copies are kept');
  await rm.getByRole('button', { name: 'Done' }).click(); await p.waitForTimeout(400);
  assert.equal(await rows().count(), before - 1); assert.equal(await panel().getByRole('button', { name: /selected/ }).count(), 0); ok('the file leaves the list and the selection clears');

  // ---- Saves
  await tab('Saves').click(); await p.waitForTimeout(200);
  assert.match(await panel().innerText(), /Boktai 2 \(USA\)[\s\S]*Mother 3[\s\S]*Save states[\s\S]*Boktai 2 \(USA\)/); ok('saves and save states are listed with their game names');
  assert.match(await panel().innerText(), /1 Pocket Sync save backup[\s\S]*not listed here/); ok('a Pocket Sync backup zip is counted, not mistaken for a save');

  // ---- Screenshots
  await tab('Screenshots').click(); await p.waitForTimeout(700);
  assert.match(await panel().innerText(), /5 screenshots from this core, plus 2 older ones marked “probably”/); ok('the tab counts exact screenshots and the older ones marked probably');
  assert.equal(await p.getByRole('button', { name: /All screenshots/ }).count(), 0); assert.equal(await panel().locator('.ct-shot').count(), 7); assert.equal(await panel().locator('.ct-why:not(.soft)').count(), 5); assert.equal(await panel().locator('.ct-why.soft').count(), 2); ok('it opens on this core: 5 exact, 2 marked probably, none from other cores');
  await panel().locator('.ct-shot').first().click(); await p.waitForTimeout(500);
  assert.equal(await p.getByRole('dialog', { name: /Screenshot taken/ }).count(), 1); await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  assert.equal(await p.getByRole('dialog', { name: /Screenshot taken/ }).count(), 0); ok('a screenshot opens large and Escape closes it');
  // a core with none of its own shows the empty state, never other cores' screenshots
  await platform('Game Boy Advance').click(); await p.waitForTimeout(300);
  await p.locator('.pv-open', { hasText: 'other.GBAlt' }).click(); await p.waitForTimeout(600);
  await tab('Screenshots').click(); await p.waitForTimeout(500);
  assert.equal(await panel().locator('.ct-shot').count(), 0); assert.match(await panel().innerText(), /No screenshots from this core yet[\s\S]*Say cheese/); ok('a core with no screenshots of its own shows the friendly empty state');
  await platform('Game Boy Advance').click(); await p.waitForTimeout(300);
  await p.locator('.pv-open', { hasText: 'Spiritualized.GBA' }).first().click(); await p.waitForTimeout(600); await tab('Screenshots').click(); await p.waitForTimeout(300);

  // ---- Settings
  await tab('Settings').click(); await p.waitForTimeout(500);
  const st = await panel().innerText();
  assert.match(st, /Needs attention[\s\S]*Nothing is missing/); assert.match(st, /High Quality Audio\s*Off\s*\(default On\)/); assert.match(st, /Rumble Strength\s*6\s*\(default 5\)/); assert.match(st, /Edge Masking\s*Auto Masking/); ok('saved settings are decoded with the core\'s own names and show the default');
  assert.match(st, /Core menu[\s\S]*Controls/); assert.match(st, /12 per-game presets in Presets\/Spiritualized\.GBA/); ok('saved files and presets are summarised');
  assert.match(st, /What this core reads[\s\S]*Cartridge[\s\S]*\.gba \.rom[\s\S]*required[\s\S]*BIOS[\s\S]*gba_bios\.bin/); ok('the core\'s data slots are listed');
  assert.match(st, /Required and configuration files[\s\S]*BIOS and system[\s\S]*gba_bios\.bin[\s\S]*boot ROM[\s\S]*Palettes[\s\S]*C2C03\.PAL[\s\S]*Vs\. System[\s\S]*Other files[\s\S]*mpu\.bin[\s\S]*a guess/i); ok('Settings groups the required files by type and explains each one, saying where the explanation comes from');
  assert.ok(await panel().locator('.cg-fi svg').count() >= 3); ok('each file and group carries its icon');

  // ---- keyboard in the menu tree
  await core('GBA').focus(); await p.keyboard.press('ArrowDown'); assert.match(await p.evaluate(() => document.activeElement.textContent), /GBAlt/);
  await p.keyboard.press('ArrowLeft'); assert.match(await p.evaluate(() => document.activeElement.textContent), /Game Boy Advance/);
  await p.keyboard.press('ArrowLeft'); await p.waitForTimeout(150);
  assert.equal(await tree.locator('.nt-core').count(), 0); await p.keyboard.press('ArrowRight'); await p.waitForTimeout(150);
  assert.equal(await tree.locator('.nt-core').count(), 2); ok('arrow keys move through the tree, collapse and expand a platform');

  // ---- no card, core with no files, overview
  await p.locator('nav .nav-group', { hasText: 'Cores' }).click(); await p.waitForTimeout(200); await p.locator('nav .nav-group', { hasText: 'Cores' }).click(); await p.waitForTimeout(300);
  assert.match(await p.locator('.cores').innerText(), /grouped like the openFPGA Library/); assert.equal(await p.locator('.ov-card').count(), 6); assert.deepEqual((await p.locator('.ov-cat h2').allInnerTexts()).map((t) => t.split('\n')[0].replace(/\s+\d+ cores?$/, '').toLowerCase()), ['console', 'handheld', 'computer', 'media players', 'no platform']); ok('the Cores entry itself opens an overview of every platform, in category sections');
  assert.equal(await p.locator('.nt input').count(), 0); ok('the menu has no search box');
  await p.getByRole('searchbox', { name: /Search cores/ }).fill('snes'); await p.waitForTimeout(250);
  assert.match(await p.locator('.cores').innerText(), /SNES[\s\S]*Console[\s\S]*Nintendo[\s\S]*agg23\.SNES/i); assert.equal(await p.locator('.pv-list li').count(), 1); ok('searching on the Cores page lists matching cores with their category, manufacturer and platform');
  await p.locator('.pv-open').click(); await p.waitForTimeout(700);
  assert.match(await p.locator('.cores h1').innerText(), /SNES/); ok('picking a result opens that core');
  await tree.locator('.nt-item[data-kind="platform"]').first().click(); await p.waitForTimeout(250);
  await p.locator('nav .nav-group', { hasText: 'Cores' }).click().catch(() => {}); await p.waitForTimeout(150); await p.locator('nav .nav-group', { hasText: 'Cores' }).click().catch(() => {}); await p.waitForTimeout(400);
  await platform('No platform').click(); await p.waitForTimeout(300);
  await p.locator('.pv-open').first().click(); await p.waitForTimeout(700);
  assert.match(await panel().innerText(), /No files yet/); ok('a core with no platform opens without errors');

  // ---- Tau's library facts live with the core
  await tree.getByRole('button', { name: 'Expand TAU' }).click().catch(() => {}); await p.waitForTimeout(150);
  await core('omega').click(); await p.waitForTimeout(900);
  const ins = p.getByRole('complementary', { name: 'omega' });
  assert.match(await ins.innerText(), /music library[\s\S]*Library ready[\s\S]*Tracks\s*7180[\s\S]*requirements/i); assert.equal(await ins.getByRole('button', { name: 'Open library' }).count(), 1); ok('a library core shows its tracks and Open library in the details panel');
  assert.match(await ins.innerText(), /Refresh|library/i); ok('the library can be refreshed from there too');
  await core('GBA').click().catch(() => {});

  // ---- the Card page: overview and history, no core lists
  await p.getByRole('button', { name: 'Card', exact: true }).click(); await p.waitForTimeout(500);
  assert.deepEqual(await p.getByRole('tab').allInnerTexts(), ['Overview', 'History']); ok('the Card page has Overview and History');
  const card = await p.locator('#cards').innerText();
  assert.ok(!/Player cores on this card|Other cores on this card|Set as player/.test(card)); assert.match(card, /8 cores installed[\s\S]*Every core has the files it needs[\s\S]*Music libraries/i); ok('the simple core lists are gone; the overview says what needs attention (nothing, once the BIOS was added) and lists the libraries');
  assert.match(await p.locator('.sum-stats').innerText(), /Cores\s*8[\s\S]*Platforms\s*5[\s\S]*Categories\s*4[\s\S]*Library tracks\s*7180[\s\S]*Screenshots\s*0/); assert.match(await p.locator('.sum-space').innerText(), /used of[\s\S]*free/); ok('the overview shows card totals and the space used and free');
  await p.getByRole('button', { name: 'Browse cores' }).click(); await p.waitForTimeout(400);
  assert.match(await p.locator('.cores').innerText(), /grouped like the openFPGA Library/); ok('Browse cores opens the Cores section');
  await p.getByRole('button', { name: 'Card', exact: true }).click(); await p.waitForTimeout(300);
  await p.getByRole('tab', { name: 'History' }).click(); await p.waitForTimeout(500);
  assert.match(await p.locator('main').innerText(), /Sync history/); ok('History is a tab of the Card page');
  assert.equal(await p.locator('nav button', { hasText: /^Sync history$/ }).count(), 0); ok('and it is no longer a separate menu entry');

  // ---- the Library's target core is chosen from the cores that play mp3 and flac
  await p.locator('nav button', { hasText: /^Library$/ }).click(); await p.waitForTimeout(700);
  const target = p.getByRole('combobox', { name: 'Target core' });
  assert.equal(await target.count(), 1);
  const groupsText = await target.evaluate((el) => [...el.querySelectorAll('optgroup')].map((g) => g.label + ': ' + [...g.querySelectorAll('option')].map((o) => o.textContent.trim()).join(' | ')));
  assert.match(groupsText[0], /^Plays mp3 and flac: .*omega.*Mp3Player/); assert.match(groupsText[1], /^Other cores/); ok('the target list offers the cores that play mp3 and flac first, every other core by hand after');
  await tree.locator('.nt-item[data-kind="platform"]').first().click(); await p.waitForTimeout(300);

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
