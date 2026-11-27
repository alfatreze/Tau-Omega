// Behaviour checks for per-game artwork inside a core's Files tab (List/Grid toggle, lazy Libretro thumbnails, and Plex-style picker).
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/game-art-test.cjs
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

  await p.goto(url);
  await p.evaluate(() => localStorage.clear());
  await p.goto(url);
  await p.waitForTimeout(1000);

  // Navigate to Cores -> Game Boy Advance -> Spiritualized.GBA
  const tree = p.getByRole('tree', { name: 'Cores by platform' });
  await tree.getByRole('button', { name: 'Expand Game Boy Advance' }).click();
  await p.waitForTimeout(150);
  await tree.locator('.nt-core .nt-name', { hasText: /^GBA$/ }).click();
  await p.waitForTimeout(900);

  const panel = () => p.getByRole('tabpanel');

  // 1. Default view is List, with lazy Libretro thumbnails loaded onto ROM rows
  assert.equal(await panel().locator('.cf-row').count(), 6);
  const toggle = panel().getByRole('group', { name: 'View mode' });
  assert.equal(await toggle.getByRole('button', { name: 'List' }).getAttribute('aria-pressed'), 'true');
  assert.equal(await toggle.getByRole('button', { name: 'Grid' }).getAttribute('aria-pressed'), 'false');
  ok('Files tab defaults to List view with List/Grid toggle');

  await p.waitForSelector('.cf-row .cf-thumb', { timeout: 5000 });
  const thumbSrc = await panel().locator('.cf-row .cf-thumb').first().getAttribute('src');
  assert.ok(thumbSrc && thumbSrc.startsWith('data:image/'), 'Expected data URL thumbnail on ROM row');
  ok('List rows load lazy Libretro thumbnail previews');

  assert.equal(await panel().locator('input[type=checkbox]').count(), 0);
  ok('ROM selection checkboxes are hidden by default');

  // Category filter dropdown narrows games by Libretro genre
  const catSel = panel().getByLabel('Filter by category');
  await catSel.waitFor({ state: 'visible', timeout: 3000 });
  await catSel.selectOption('RPG');
  await p.waitForTimeout(200);
  assert.equal(await panel().locator('.cf-row').count(), 2);
  await catSel.selectOption('');
  await p.waitForTimeout(200);
  assert.equal(await panel().locator('.cf-row').count(), 6);
  ok('Category filter narrows games and resets cleanly');

  // 2. Switch to Grid view and verify cards + persisted preference
  await toggle.getByRole('button', { name: 'Grid' }).click();
  await p.waitForTimeout(250);
  assert.equal(await panel().locator('.cf-card').count(), 6);
  assert.equal(await panel().locator('.cf-card img').count(), 6);
  assert.equal(await p.evaluate(() => localStorage.getItem('tau.coreFiles.viewMode')), 'grid');
  ok('Grid toggle switches to boxart cover cards and persists preference');

  // Opt-in Select button enables selection checkboxes in Grid view
  await panel().getByRole('button', { name: 'Select', exact: true }).click();
  await p.waitForTimeout(150);
  await panel().getByRole('checkbox', { name: 'Select Boktai 2 (USA).gba' }).check();
  await p.waitForTimeout(150);
  assert.match(await panel().innerText(), /Remove 1 selected/);
  await panel().getByRole('checkbox', { name: 'Select Boktai 2 (USA).gba' }).uncheck();
  await panel().getByRole('button', { name: 'Done', exact: true }).click();
  await p.waitForTimeout(150);
  ok('Opt-in Select button enables ROM selection checkboxes in Grid view');

  // Switch back to List view and verify preference updates
  await toggle.getByRole('button', { name: 'List' }).click();
  await p.waitForTimeout(200);
  assert.equal(await panel().locator('.cf-row').count(), 6);
  assert.equal(await p.evaluate(() => localStorage.getItem('tau.coreFiles.viewMode')), 'list');
  ok('List toggle restores compact rows and persists preference');

  // Clicking a game opens Game Details in the right sidebar
  await panel().locator('.cf-row', { hasText: 'Boktai 2' }).locator('.cf-row-main').click();
  await p.waitForTimeout(400);
  const side = p.locator('#core-details');
  assert.match(await side.innerText(), /About this game[\s\S]*Developer[\s\S]*Images \(3\)/i);
  ok('Clicking a game opens Game Details with metadata and 3 gallery images in the right sidebar');
  await side.getByRole('button', { name: 'Core details' }).click();
  await p.waitForTimeout(200);
  assert.equal(await side.getByRole('button', { name: /Remove core/ }).count(), 1);
  ok('Sidebar switches cleanly back to Core details with Remove core');

  // 3. Open GameArtworkPickerSheet on Boktai 2
  await panel().getByRole('button', { name: 'Change artwork for Boktai 2 (USA).gba' }).click();
  const dialog = p.getByRole('dialog', { name: /Boktai 2/ });
  await dialog.waitFor({ state: 'visible', timeout: 3000 });
  await p.waitForSelector('.gart-opt', { timeout: 5000 });
  assert.equal(await dialog.locator('.gart-opt').count(), 3);
  ok('Game artwork picker opens and loads Libretro candidates');

  // Switch artwork kind to Title screen & search custom query
  await dialog.getByRole('group', { name: 'Artwork type' }).getByRole('button', { name: 'Title screen' }).click();
  await p.waitForTimeout(300);
  await dialog.getByLabel('Game title to search').fill('Boktai Solar Boy');
  await dialog.getByRole('button', { name: 'Search' }).click();
  await p.waitForTimeout(300);
  assert.match(await dialog.locator('.gart-opt').first().innerText(), /Boktai Solar Boy/);
  ok('Switching artwork kind and searching Libretro updates candidates');

  // Pick second candidate and apply override
  await dialog.locator('.gart-opt').nth(1).click();
  await dialog.getByRole('button', { name: 'Use selected artwork' }).click();
  await dialog.waitFor({ state: 'detached', timeout: 3000 });
  ok('Applying a Libretro candidate saves override and closes picker');

  // Verify the updated image reflects the override, then reopen and reset to automatic
  const boktaiThumb = await panel().locator('.cf-row', { hasText: 'Boktai 2' }).locator('.cf-thumb').getAttribute('src');
  assert.ok(decodeURIComponent(boktaiThumb || '').includes('TITLE OVERRIDE'), 'Expected updated override badge in mock SVG');

  await panel().getByRole('button', { name: 'Change artwork for Boktai 2 (USA).gba' }).click();
  await dialog.waitFor({ state: 'visible', timeout: 3000 });
  const resetBtn = dialog.getByRole('button', { name: 'Reset to automatic' });
  assert.equal(await resetBtn.isVisible(), true);
  await resetBtn.click();
  await dialog.waitFor({ state: 'detached', timeout: 3000 });
  await p.waitForTimeout(300);
  const restoredThumb = await panel().locator('.cf-row', { hasText: 'Boktai 2' }).locator('.cf-thumb').getAttribute('src');
  assert.ok(decodeURIComponent(restoredThumb || '').includes('GBA · BOXART'), 'Expected automatic artwork after reset');
  ok('Reset to automatic clears override and restores automatic Libretro art');

  assert.deepEqual(errors, [], errors.join(' | '));
  ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
