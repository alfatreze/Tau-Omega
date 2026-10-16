// Skip silence is an advanced sync option: Library > Advanced > Skip silence options (on by default, the choices and their defaults, saved), the way
// into adjusting the tracks already on the card, and the same options from Settings > Syncing music > Advanced.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/trim-test.cjs
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const url = process.env.URL || 'http://localhost:5173';
let passed = 0;
const ok = (n) => { passed++; console.log(`ok - ${n}`); };
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  p.on('pageerror', (e) => errors.push(e.message));
  p.on('console', (m) => m.text().startsWith('[dev-mock] unmocked') && errors.push(m.text()));
  await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(900);
  await p.getByRole('button', { name: 'Library', exact: true }).click(); await p.waitForTimeout(500);
  const menu = async (item) => { await p.getByRole('button', { name: /Advanced/ }).click(); await p.getByRole('menuitem', { name: item }).click(); await p.waitForTimeout(500); };
  await p.getByRole('button', { name: /Advanced/ }).click();
  assert.equal(await p.getByRole('menuitem').count(), 2); ok('the Library page has an Advanced menu with the two skip-silence items');
  await p.keyboard.press('Escape'); await p.getByRole('button', { name: /Advanced/ }).click(); await p.waitForTimeout(100);
  await menu('Skip silence options…');
  const dlg = p.getByRole('dialog', { name: 'Skip silence' });
  assert.equal(await dlg.getByLabel('Silence level').count(), 1); assert.equal(await dlg.getByRole('button', { name: 'Turn off' }).count(), 1); ok('skip silence is on by default and shows its options');
  assert.equal(await dlg.getByLabel('Silence level').inputValue(), '-600'); ok('the level starts at -60 dB');
  assert.equal(await dlg.getByLabel('Silence kept at the start').inputValue(), '200');
  assert.equal(await dlg.getByLabel('Silence kept at the end').inputValue(), '500');
  assert.equal(await dlg.getByLabel('Smallest silence').inputValue(), '2000'); ok('the defaults: keep 200 ms before and 500 ms after, skip silences of at least 2 s');
  const levels = await dlg.getByLabel('Silence level').locator('option').allTextContents();
  assert.equal(levels.length, 9); assert.equal(levels[0].trim(), '-80 dB'); ok('the level is limited to -80 to -40 dB');
  await dlg.getByLabel('Smallest silence').selectOption('5000'); await p.waitForTimeout(200);
  assert.match(await dlg.innerText(), /Saved\./); ok('a change saves');
  // adjust the tracks already on the card, from the options dialog
  await dlg.getByRole('button', { name: 'Adjust tracks…' }).click(); await p.waitForTimeout(500);
  const rev = p.getByRole('dialog', { name: 'Tracks and what is skipped' });
  assert.match(await rev.innerText(), /2 of 3 tracks have silence to skip/); ok('adjust tracks opens the list of what the card holds');
  await rev.getByRole('button', { name: /01 Intro/ }).click(); await p.waitForTimeout(400);
  await rev.getByLabel('Seconds skipped at the start').fill('5'); await rev.getByLabel('Seconds skipped at the start').dispatchEvent('change');
  await rev.getByRole('button', { name: 'Save' }).click(); await p.waitForTimeout(700);
  assert.match(await rev.getByRole('button', { name: /01 Intro/ }).innerText(), /5\.0 s/); assert.match(await rev.innerText(), /edited/); ok('a typed value is saved and the track shows as edited');
  await rev.getByLabel('Never skip silence on this track').check(); await rev.getByRole('button', { name: 'Save' }).click(); await p.waitForTimeout(700);
  assert.match(await rev.getByRole('button', { name: /01 Intro/ }).innerText(), /never skip/); ok('never skip leaves the track whole');
  await rev.getByRole('button', { name: 'Back to detected' }).click(); await p.waitForTimeout(700);
  assert.match(await rev.getByRole('button', { name: /01 Intro/ }).innerText(), /9\.8 s/); ok('back to detected restores it');
  await rev.getByRole('button', { name: 'Done' }).click(); await p.waitForTimeout(200);
  await dlg.getByRole('button', { name: 'Done' }).click(); await p.waitForTimeout(200);
  // with it off, the list offers to turn it on and measure what is on the card
  await menu('Skip silence options…'); await p.getByRole('dialog', { name: 'Skip silence' }).getByRole('button', { name: 'Turn off' }).click(); await p.waitForTimeout(200);
  await p.getByRole('dialog', { name: 'Skip silence' }).getByRole('button', { name: 'Done' }).click(); await p.waitForTimeout(200);
  await menu('Adjust tracks on the card…');
  const off = p.getByRole('dialog', { name: 'Tracks and what is skipped' });
  assert.match(await off.innerText(), /Skip silence is off/); await off.getByRole('button', { name: 'Turn on and measure' }).click(); await p.waitForTimeout(700);
  assert.match(await off.innerText(), /tracks have silence to skip/); ok('with it off, the list offers to turn it on and measure the tracks already on the card');
  await off.getByRole('button', { name: 'Done' }).click(); await p.waitForTimeout(200);
  // the direct menu item opens the same list
  await menu('Adjust tracks on the card…');
  assert.equal(await p.getByRole('dialog', { name: 'Tracks and what is skipped' }).count(), 1); ok('the menu also opens the track list directly');
  await p.getByRole('dialog', { name: 'Tracks and what is skipped' }).getByRole('button', { name: 'Done' }).click(); await p.waitForTimeout(200);
  // Settings keeps a pointer to the same options
  if (!(await p.getByRole('button', { name: 'Settings', exact: true }).isVisible())) await p.getByRole('button', { name: /Tools & settings/ }).click();
  await p.getByRole('button', { name: 'Settings', exact: true }).click(); await p.waitForTimeout(500);
  await p.getByRole('button', { name: 'Skip silence…' }).click(); await p.waitForTimeout(400);
  const again = p.getByRole('dialog', { name: 'Skip silence' });
  assert.equal(await again.getByLabel('Smallest silence').inputValue(), '5000'); ok('Settings > Syncing music > Advanced opens the same options, with the saved values');
  await again.getByRole('button', { name: 'Turn off' }).click(); await p.waitForTimeout(200);
  assert.equal(await again.getByLabel('Silence level').count(), 0); ok('turning it off hides the options');
  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  console.log(`${passed} checks passed`); await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
