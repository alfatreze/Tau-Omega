// Behaviour checks for the developer switch, the card role and the Real-card confirmation, against the dev mock.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/devtools-test.cjs   (URL=... to point elsewhere)
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
  const cardPage = async () => { await nav('Card').click(); await p.waitForTimeout(400); };
  const installFlow = async () => {
    await nav('Marketplace').click(); await p.waitForTimeout(500);
    await p.getByRole('button', { name: /Add a core manually/ }).click(); await p.waitForTimeout(300);
    const box = p.locator('.ma').getByRole('textbox'); await box.fill('https://github.com/someone/unlisted-core'); await p.waitForTimeout(450);
    await p.locator('.ma').getByRole('button', { name: /Read this core/ }).click(); await p.waitForTimeout(500);
    await p.locator('.ma').getByRole('checkbox', { name: /I trust this source/ }).check(); await p.waitForTimeout(400);
    await p.locator('.ma').getByRole('button', { name: 'Review update' }).click(); await p.waitForTimeout(600);
  };

  // ---- off by default
  await start(false);
  await cardPage();
  assert.equal(await p.getByText('Card role', { exact: true }).count(), 0); ok('with developer tools off there is no card role on the Card page');
  await installFlow();
  assert.equal(await p.getByLabel('Type the card name to confirm').count(), 0); ok('and installing onto the card asks for no name');
  await p.keyboard.press('Escape');
  await p.getByRole('button', { name: /Tools & settings/ }).click(); await p.getByRole('button', { name: 'Settings', exact: true }).click(); await p.waitForTimeout(500);
  assert.match(await p.locator('.ls-row', { hasText: 'Show developer tools' }).innerText(), /For people who build cores[\s\S]*Off by default/); ok('Settings has the Show developer tools row, explained');
  await p.locator('.ls-row', { hasText: 'Show developer tools' }).getByRole('switch').click(); await p.waitForTimeout(400);
  assert.equal(await p.locator('.ls-row', { hasText: 'Show developer tools' }).getByRole('switch').getAttribute('aria-checked'), 'true'); ok('turning it on saves');

  // ---- the developer tools have their own section in the menu, like Tools & settings
  const grp = p.locator('nav .nav-group', { hasText: 'Developer tools' });
  assert.equal(await grp.count(), 1); ok('with developer tools on the menu has a Developer tools section');
  assert.deepEqual(await p.locator('#dev-tools button').allInnerTexts(), ['Builds', 'Results', 'Check', 'Builder']); ok('holding Builds, Results, Check and Builder');
  assert.equal(await p.locator('nav > button', { hasText: /^Builds$/ }).count(), 0); ok('and none of them sit loose in the main menu');
  await grp.click(); await p.waitForTimeout(200);
  assert.equal(await p.locator('#dev-tools').count(), 0); assert.equal(await grp.getAttribute('aria-expanded'), 'false'); ok('the section collapses');
  await grp.click(); await p.waitForTimeout(200);
  assert.equal(await p.locator('#dev-tools').count(), 1); ok('and opens again');

  // ---- on: a Real card by default
  await cardPage();
  assert.match(await p.locator('.card-sum').innerText(), /Card role\s*Real card[\s\S]*type its name first[\s\S]*\.omega-role/i); ok('the Card page shows the role: Real by default, and says what that means');
  await installFlow();
  const name = p.getByLabel('Type the card name to confirm');
  assert.equal(await name.count(), 1); assert.equal(await p.getByRole('button', { name: 'Confirm and install' }).isDisabled(), true); ok('installing onto a Real card asks for its name and blocks the button');
  await name.fill('Wrong'); assert.equal(await p.getByRole('button', { name: 'Confirm and install' }).isDisabled(), true); ok('a wrong name keeps it blocked');
  await name.fill('pocket'); assert.equal(await p.getByRole('button', { name: 'Confirm and install' }).isDisabled(), false); ok('the right name (any case) unblocks it');
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);

  // ---- mark as a test card
  await cardPage();
  await p.getByRole('switch', { name: 'This is a test card' }).click(); await p.waitForTimeout(400);
  assert.match(await p.locator('.card-sum').innerText(), /Test card[\s\S]*usual reviewed plan[\s\S]*Marked as a test card/); ok('marking the card as a test card says so');
  await installFlow();
  assert.equal(await p.getByLabel('Type the card name to confirm').count(), 0); assert.equal(await p.getByRole('button', { name: 'Confirm and install' }).isDisabled(), false); ok('a test card needs no name');
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  await cardPage();
  await p.getByRole('switch', { name: 'This is a test card' }).click(); await p.waitForTimeout(400);
  assert.match(await p.locator('.card-sum').innerText(), /Real card[\s\S]*Real card again/); ok('and it can go back to Real');

  assert.deepEqual(errors, [], errors.join(' | ')); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
