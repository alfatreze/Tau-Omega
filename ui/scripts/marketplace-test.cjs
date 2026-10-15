// Behaviour checks for the core Marketplace and Core updates against the dev mock.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/marketplace-test.cjs   (URL=... to point elsewhere)
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
  const fresh = async () => { await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(900); };
  const nav = (name) => p.locator('nav button', { hasText: name }).first();
  const openMarket = async () => { await nav('Marketplace').click(); await p.waitForTimeout(700); };
  const search = (text) => p.getByRole('searchbox', { name: 'Search cores' }).fill(text);
  const rows = () => p.locator('.mk-open');
  const sheet = () => p.getByRole('dialog');

  await fresh();
  assert.match(await nav('Marketplace').innerText(), /4/); ok('the nav badge counts the cores with an update on the open card');
  await openMarket();
  assert.ok(await rows().count() >= 10); ok('the catalog list shows cores');
  await p.waitForTimeout(500);
  assert.ok(await p.locator('img.mk-icon').count() >= 5); assert.ok(await p.locator('.mk-art img').count() >= 5); ok('banners and author icons are fetched for the cards that are showing');
  assert.match(await p.locator('.mk-card').first().innerText(), /\S/); assert.ok(await p.locator('.mk-author-name').first().innerText() !== ''); ok('each card shows its author');
  const tags = p.getByRole('group', { name: 'Category' });
  assert.match(await tags.innerText(), /All\s*20[\s\S]*Handheld\s*\d/); ok('quick category tags show how many cores each holds');
  await tags.getByRole('button', { name: /^Handheld/ }).click(); await p.waitForTimeout(200);
  assert.equal(await tags.getByRole('button', { name: /^Handheld/ }).getAttribute('aria-pressed'), 'true');
  const handheld = await rows().count(); assert.ok(handheld >= 1 && handheld < 20);
  assert.ok((await p.locator('.mk-tag').allInnerTexts()).every((t) => t.trim() === 'Handheld')); ok('choosing a category tag filters the cards');
  await tags.getByRole('button', { name: /^All/ }).click(); await p.waitForTimeout(200);
  await p.locator('.mk-tag', { hasText: 'Console' }).first().click(); await p.waitForTimeout(200);
  assert.equal(await tags.getByRole('button', { name: /^Console/ }).getAttribute('aria-pressed'), 'true'); ok('the category tag on a card filters to that category too');
  await tags.getByRole('button', { name: /^All/ }).click(); await p.waitForTimeout(200);
  assert.match(await p.locator('.mk-note').innerText(), /of 20 cores/); ok('the list says how many cores it shows and how fresh it is');

  await search('snes'); await p.waitForTimeout(200);
  assert.equal(await rows().count(), 1); assert.match(await rows().first().getAttribute('aria-label'), /^SNES, Console/); ok('search narrows the list by name');
  await search('zzzz'); await p.waitForTimeout(200);
  assert.match(await p.locator('.empty').innerText(), /No cores match[\s\S]*zzzz/); ok('an empty result names what was searched');
  await p.getByRole('button', { name: 'Clear filters' }).click(); await p.waitForTimeout(200);
  assert.ok(await rows().count() >= 10); ok('Clear filters brings the list back');

  await search('arduboy'); await p.waitForTimeout(200);
  assert.match(await p.locator('.mk-card').first().innerText(), /Installed/); ok('a core on the card is marked Installed');
  await rows().first().click(); await p.waitForTimeout(600);
  assert.equal(await sheet().count(), 1);
  assert.match(await sheet().innerText(), /Requirements[\s\S]*Pocket firmware\s*1\.1 or newer[\s\S]*Dock\s*yes[\s\S]*Link port\s*no/); ok('requirements are shown from the catalog before anything is downloaded');
  assert.equal(await sheet().locator('img.mk-banner').count(), 1); ok('the platform banner is shown in the details');
  await sheet().getByRole('button', { name: 'Download and review' }).click(); await p.waitForTimeout(600);
  assert.match(await sheet().innerText(), /no checksum was published for this release/); ok('a release without a checksum says so once it is downloaded');
  assert.match(await sheet().innerText(), /github\.com\/agg23\/openfpga-arduboy/); ok('the detail shows where the core comes from');
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  assert.equal(await sheet().count(), 0); assert.match(await p.evaluate(() => document.activeElement?.className || ''), /mk-open/); ok('Escape closes the details and focus returns to the row');

  await search('capcom'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(600);
  assert.match(await sheet().innerText(), /Requirements[\s\S]*Key file[\s\S]*beta\.bin[\s\S]*not in the download[\s\S]*get it from the author[\s\S]*Cores\/jotego\.jtcps1\//); ok('a core that needs a key file lists it under Requirements with where to get it and where it goes');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);

  await search('snes'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(600);
  await sheet().getByRole('button', { name: 'Download and review' }).click(); await p.waitForTimeout(600);
  assert.match(await sheet().innerText(), /Downloaded[\s\S]*checksum verified[\s\S]*Needs Pocket firmware 2\.0/); ok('download and review shows the checks before any plan');
  assert.equal(await sheet().getByRole('button', { name: 'Review update' }).count(), 1); ok('the reviewed install plan is offered, nothing is written yet');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);

  await search(''); await p.waitForTimeout(200);
  await rows().first().focus(); await p.keyboard.press('ArrowRight'); await p.keyboard.press('ArrowRight'); await p.keyboard.press('ArrowLeft');
  assert.equal(await p.evaluate(() => [...document.querySelectorAll('.mk-open')].indexOf(document.activeElement)), 1); ok('Left and Right move between cards');
  await p.keyboard.press('ArrowDown');
  assert.ok(await p.evaluate(() => [...document.querySelectorAll('.mk-open')].indexOf(document.activeElement)) > 1); ok('Down moves a whole row');
  await p.keyboard.press('Escape');

  // error and offline states (the mock switches modes through window.__mkt)
  await p.evaluate(() => { globalThis.__mkt = 'offline'; });
  await p.getByRole('button', { name: 'Refresh' }).click(); await p.waitForTimeout(500);
  assert.match(await p.locator('.mk-note').innerText(), /Showing the saved list from 3 days ago\. Could not reach the server/); ok('offline: the saved list is shown with its age and the reason');
  await search('chiskit'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(500);
  assert.match(await sheet().innerText(), /author says they used AI tools[\s\S]*Claude/); ok('an author\'s AI declaration is shown as provenance');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);
  await search('snes'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(500);
  assert.match(await sheet().innerText(), /Pocket firmware\s*2\.0 or newer/); assert.match(await sheet().innerText(), /Earlier versions \(1\)/); ok('the firmware requirement and the version history come from the catalog');
  await sheet().locator('summary').click();
  await sheet().getByRole('button', { name: 'Choose this version' }).click(); await p.waitForTimeout(200);
  assert.ok(await sheet().getByRole('button', { name: 'Download version 0.9.0 and review' }).count() === 1); ok('an earlier version can be chosen and the install button names it');
  await sheet().getByRole('button', { name: 'Download version 0.9.0 and review' }).click(); await p.waitForTimeout(500);
  assert.match(await sheet().innerText(), /Downloaded/); ok('the chosen older version is downloaded and goes through the same reviewed plan');
  await sheet().getByRole('button', { name: 'Use the newest instead' }).click(); await p.waitForTimeout(200);
  assert.equal(await sheet().getByRole('button', { name: 'Download and review' }).count(), 1); ok('choosing the newest again resets the install step');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);
  await search(''); await p.waitForTimeout(300);
  await p.evaluate(() => { globalThis.__mkt = 'fail'; });
  await nav('Card').click(); await p.waitForTimeout(300); await openMarket();
  assert.match(await p.locator('.empty').innerText(), /The list could not be loaded[\s\S]*Try again/); ok('a list that cannot be loaded offers Try again');
  await p.evaluate(() => { globalThis.__mkt = undefined; });

  // Core updates: a KPI on the Card page, the list in Cores > Updates (only the cores that need one)
  await nav('Card').click(); await p.waitForTimeout(700);
  assert.equal(await p.locator('#cards').getByText('Core updates').count(), 0); ok('the Card page no longer carries the updates list');
  const kpi = p.getByRole('button', { name: /4 core updates available/ });
  assert.equal(await kpi.count(), 1); ok('the Card overview shows an Updates figure with the number available');
  await kpi.click(); await p.waitForTimeout(500);
  assert.equal(await p.getByRole('tab', { name: /Updates/ }).getAttribute('aria-selected'), 'true'); ok('it opens the Updates tab of Cores');
  const cu = p.getByRole('region', { name: 'Core updates' });
  assert.match(await cu.innerText(), /5 cores need updating/); assert.match(await cu.innerText(), /Tau cores update through the Tau update bar/); ok('Updates lists the cores that need updating and defers Tau cores to the Tau bar');
  assert.match(await cu.innerText(), /Replaced by pack\.NES/); assert.ok(!/Up to date|Not in the library/.test(await cu.innerText())); ok('a renamed core is listed, up-to-date and unknown cores are not');
  assert.match(await cu.innerText(), /someone\.Unlisted[\s\S]*1\.0\.0 → 1\.1\.0[\s\S]*added manually/); ok('a manually added core with a newer GitHub release is listed and marked as added manually');
  await cu.locator('article', { hasText: 'someone.Unlisted' }).getByRole('button', { name: 'Review update' }).click(); await p.waitForTimeout(600);
  assert.match(await p.locator('.ma').innerText(), /Unlisted[\s\S]*Not in the openFPGA Library/); ok('Review update opens that release in the manual-add sheet (with the trust tick asked again)');
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  await cu.getByRole('button', { name: /Review all 3 updates/ }).click(); await p.waitForTimeout(300);
  assert.match(await cu.innerText(), /Update 1 of 3/); ok('Review all starts a one-at-a-time queue');
  await cu.getByRole('button', { name: 'Skip this one' }).click(); await p.waitForTimeout(250);
  assert.match(await cu.innerText(), /Update 2 of 3/); ok('Skip moves to the next update without installing');

  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
