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
  const tab = async (name) => { await sheet().getByRole('tab', { name }).click(); await p.waitForTimeout(150); };

  await fresh();
  assert.match(await nav('Marketplace').innerText(), /4/); ok('the nav badge counts the cores with an update on the open card');
  await openMarket();
  assert.ok(await rows().count() >= 10); ok('the catalog list shows cores');
  await p.waitForTimeout(500);
  assert.ok(await p.locator('img.mk-icon').count() >= 5); assert.ok(await p.locator('.mk-art img').count() >= 5); ok('banners and author icons are fetched for the cards that are showing');
  assert.match(await p.locator('.mk-card').first().innerText(), /\S/); assert.ok(await p.locator('.mk-author-name').first().innerText() !== ''); ok('each card shows its author');
  const tags = p.getByRole('group', { name: 'Category' });
  assert.match(await tags.innerText(), /All\s*21[\s\S]*Handheld\s*\d/); ok('quick category tags show how many cores each holds');
  await tags.getByRole('button', { name: /^Handheld/ }).click(); await p.waitForTimeout(200);
  assert.equal(await tags.getByRole('button', { name: /^Handheld/ }).getAttribute('aria-pressed'), 'true');
  const handheld = await rows().count(); assert.ok(handheld >= 1 && handheld < 21);
  assert.ok((await p.locator('.mk-tag').allInnerTexts()).every((t) => t.trim() === 'Handheld')); ok('choosing a category tag filters the cards');
  await tags.getByRole('button', { name: /^All/ }).click(); await p.waitForTimeout(200);
  await p.locator('.mk-tag', { hasText: 'Console' }).first().click(); await p.waitForTimeout(200);
  assert.equal(await tags.getByRole('button', { name: /^Console/ }).getAttribute('aria-pressed'), 'true'); ok('the category tag on a card filters to that category too');
  await tags.getByRole('button', { name: /^All/ }).click(); await p.waitForTimeout(200);
  assert.match(await p.locator('.mk-note').innerText(), /of 21 cores/); ok('the list says how many cores it shows and how fresh it is');

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
  assert.equal(await sheet().getByRole('button', { name: 'Install', exact: true }).count(), 0);
  assert.match(await sheet().getByTestId('mk-have').innerText(), /Installed · v0\.9\.0[\s\S]*View on card/); ok('an up-to-date installed core shows its version and View on card instead of Install');
  await tab('Details');
  assert.match(await sheet().innerText(), /Requirements[\s\S]*Pocket firmware\s*1\.1 or newer[\s\S]*Dock\s*yes[\s\S]*Link port\s*no/); ok('requirements are shown from the catalog before anything is downloaded');
  assert.equal(await sheet().locator('img.mk-banner').count(), 1); ok('the platform banner is shown in the details');
  await tab('About');
  assert.match(await sheet().innerText(), /agg23\/openfpga-arduboy on GitHub/); ok('the detail shows where the core comes from');
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  assert.equal(await sheet().count(), 0); assert.match(await p.evaluate(() => document.activeElement?.className || ''), /mk-open/); ok('Escape closes the details and focus returns to the row');

  await search('amiga'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(600);
  await sheet().getByRole('button', { name: 'Install', exact: true }).click(); await p.waitForTimeout(600);
  assert.match(await sheet().innerText(), /no checksum was published for this release/); ok('a release without a checksum says so once it is downloaded');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);

  await search('capcom'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(600);
  assert.match(await sheet().innerText(), /needs a key file from its author/); ok('a core that needs a key file says so beside the Install button');
  await tab('Details');
  assert.match(await sheet().innerText(), /Requirements[\s\S]*Key file[\s\S]*beta\.bin[\s\S]*not in the download[\s\S]*get it from the author[\s\S]*Cores\/jotego\.jtcps1\//); ok('a core that needs a key file lists it under Requirements with where to get it and where it goes');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);

  await search('snes'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(600);
  await sheet().getByRole('button', { name: /^(Install|Update to)/ }).click(); await p.waitForTimeout(600);
  assert.match(await sheet().innerText(), /Downloaded[\s\S]*checksum verified[\s\S]*Needs Pocket firmware 2\.0/); ok('download and review shows the checks before any plan');
  assert.equal(await sheet().getByRole('region', { name: 'Install review' }).count(), 1); assert.equal(await sheet().getByRole('region', { name: 'Install result' }).count(), 0); ok('the install review is shown at once, and nothing is written yet');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);

  await search(''); await p.waitForTimeout(200);
  await rows().first().focus(); await p.keyboard.press('ArrowRight'); await p.keyboard.press('ArrowRight'); await p.keyboard.press('ArrowLeft');
  assert.equal(await p.evaluate(() => [...document.querySelectorAll('.mk-open')].indexOf(document.activeElement)), 1); ok('Left and Right move between cards');
  await p.keyboard.press('ArrowDown');
  assert.ok(await p.evaluate(() => [...document.querySelectorAll('.mk-open')].indexOf(document.activeElement)) > 1); ok('Down moves a whole row');
  await p.keyboard.press('Escape');

  // The details sheet: Install first, then About (what it is) and Details (technical), with GitHub's own facts.
  await search('arduboy'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(600);
  assert.equal(await sheet().getByRole('tab', { name: 'About' }).getAttribute('aria-selected'), 'true'); ok('the sheet opens on About');
  assert.match(await sheet().innerText(), /A mock description of agg23\.Arduboy/); ok('About says what the core is, in the author\'s words from GitHub');
  assert.match(await sheet().innerText(), /Downloads\s*12,345 in total · 678 for v1\.0\.0/); ok('About shows how often the release files were downloaded');
  assert.match(await sheet().innerText(), /Licence\s*MIT/); assert.match(await sheet().innerText(), /What’s new in v1\.0\.0[\s\S]*Fixed the menu\./); ok('About shows the licence and what is new in the newest release');
  const installBox = await sheet().getByTestId('mk-have').boundingBox(); const tabsBox = await sheet().getByRole('tablist').boundingBox();
  assert.ok(installBox.y < tabsBox.y, 'status is above the tabs'); ok('the installed status sits above the tabs');
  assert.ok(!/Omega downloads the zip|Nothing is written yet/.test(await sheet().innerText())); assert.equal(await sheet().getByRole('heading', { name: 'Install' }).count(), 0); ok('the header action has no heading or explanation around it');
  const link = sheet().getByRole('link', { name: /agg23\/openfpga-arduboy on GitHub/ });
  assert.equal(await link.getAttribute('href'), 'https://github.com/agg23/openfpga-arduboy'); assert.equal(await sheet().getByRole('button', { name: 'Copy link' }).count(), 0); ok('the source is a real link, not text to copy');
  await p.evaluate(() => { globalThis.__opened = []; }); await link.click(); await p.waitForTimeout(200);
  assert.deepEqual(await p.evaluate(() => globalThis.__opened), ['agg23.Arduboy']); assert.equal(await sheet().count(), 1); ok('clicking it opens the GitHub page in the browser and leaves the app where it is');
  await sheet().getByRole('tab', { name: 'About' }).focus(); await p.keyboard.press('ArrowRight'); await p.waitForTimeout(150);
  assert.equal(await sheet().getByRole('tab', { name: 'Details' }).getAttribute('aria-selected'), 'true'); assert.match(await sheet().innerText(), /Requirements/); ok('the arrow keys move between the tabs');
  assert.match(await sheet().innerText(), /Core id\s*agg23\.Arduboy[\s\S]*Topics\s*analogue-pocket\s*openfpga/); ok('Details holds the technical facts and requirements');
  assert.ok(!/A mock description/.test(await sheet().innerText())); ok('the description stays on About');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);
  await rows().first().click(); await p.waitForTimeout(500);
  assert.equal(await sheet().getByRole('tab', { name: 'About' }).getAttribute('aria-selected'), 'true'); ok('the next core opens on About again');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);
  await p.evaluate(() => { globalThis.__repo = 'partial'; });
  await rows().first().click(); await p.waitForTimeout(500);
  assert.match(await sheet().innerText(), /Mock description for agg23\.Arduboy/); assert.match(await sheet().innerText(), /Downloads\s*not available/); ok('when only the release counts fail, the description still shows');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);
  await p.evaluate(() => { globalThis.__repo = 'offline'; });
  await search('snes'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(500);
  assert.match(await sheet().innerText(), /description could not be loaded from GitHub/); assert.equal(await sheet().getByRole('button', { name: /^(Install|Update to)/ }).count(), 1); ok('offline: it says so, and Update still works');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);
  await p.evaluate(() => { globalThis.__repo = undefined; });
  await search(''); await p.waitForTimeout(200);

  // The sheet's header: title with action beside it, the developer under it, then category, maker and year; a round X top right.
  await search('arduboy'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(700);
  const box = async (loc) => loc.boundingBox();
  const titleB = await box(sheet().getByRole('heading', { name: 'Arduboy' })); const goB = await box(sheet().getByTestId('mk-have'));
  assert.ok(goB.x > titleB.x + 150 && Math.abs((goB.y + goB.height / 2) - (titleB.y + titleB.height / 2)) < 35); ok('Installed status is on the right, level with the core title');
  const devB = await box(sheet().locator('.mk-dev')); const metaB = await box(sheet().locator('.mk-by'));
  assert.match(await sheet().locator('.mk-dev').innerText(), /agg23/); assert.ok(devB.y > titleB.y + titleB.height - 2 && metaB.y > devB.y + devB.height - 2); ok('the developer comes under the title, then the line below it');
  assert.equal((await sheet().locator('.mk-by').innerText()).trim(), 'Handheld · Arduboy · 2016'); assert.ok(!/agg23\.Arduboy/.test(await sheet().locator('.mk-by').innerText())); ok('that line reads category, maker and year, without the core id');
  assert.equal(await sheet().locator('.mk-dev .mk-icon, .mk-dev .mk-avatar').count() >= 1, true); ok('the developer\'s icon is shown beside the name');
  const xb = sheet().getByRole('button', { name: 'Close details' }); const xB = await box(xb); const sB = await box(sheet());
  assert.ok(Math.abs(xB.width - xB.height) < 1 && xB.x + xB.width > sB.x + sB.width - 60 && xB.y < sB.y + 60); assert.equal((await xb.innerText()).trim(), ''); ok('Close is a round X button at the top right');
  assert.equal(await sheet().locator('.eyebrow').count(), 0); ok('the category is no longer a label above the title');
  await xb.click(); await p.waitForTimeout(200);
  assert.equal(await sheet().count(), 0); ok('the X closes the sheet');
  await search('amiga'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(500);
  await sheet().getByRole('button', { name: 'Install', exact: true }).click(); await p.waitForTimeout(600);
  assert.equal(await sheet().getByRole('region', { name: 'Install review' }).count(), 1); assert.equal(await sheet().locator('.mk-go').isDisabled(), true); ok('the header Install starts the download and review, and cannot be pressed twice');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);
  await rows().first().click(); await p.waitForTimeout(500);
  assert.equal(await sheet().locator('.mk-go').isDisabled(), false); assert.equal(await sheet().getByRole('region', { name: 'Install review' }).count(), 0); ok('opening a core again starts clean');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);
  await search(''); await p.waitForTimeout(200);

  // The gallery strip under the banner and its lightbox.
  await search('arduboy'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(700);
  const strip = sheet().getByRole('list', { name: /Pictures of Arduboy from GitHub/ });
  assert.equal(await strip.getByRole('button').count(), 3); ok('a strip of the pictures GitHub has sits in the sheet');
  const bannerBox = await sheet().locator('img.mk-banner').boundingBox(); const stripBox = await strip.boundingBox(); const headBox = await sheet().locator('.mk-sheet-head').boundingBox();
  assert.ok(stripBox.y >= bannerBox.y + bannerBox.height - 1 && stripBox.y < headBox.y); ok('the strip is right below the core image');
  await strip.getByRole('button').nth(1).click(); await p.waitForTimeout(200);
  const lb = p.getByRole('dialog', { name: /Pictures of Arduboy/ });
  assert.equal(await lb.count(), 1); assert.match(await lb.innerText(), /2 of 3 · The viewer/); assert.equal(await lb.locator('img').getAttribute('alt'), 'The viewer'); ok('clicking a picture opens a lightbox on it, with its place and caption');
  const im = await lb.locator('img').boundingBox(); assert.ok(im.width > 300 && im.height > 250); ok('the picture is shown large');
  await lb.getByRole('button', { name: 'Next picture' }).click(); await p.waitForTimeout(100);
  assert.match(await lb.innerText(), /3 of 3 · On the Pocket/); await p.keyboard.press('ArrowRight'); await p.waitForTimeout(100);
  assert.match(await lb.innerText(), /1 of 3 · The grid/); await p.keyboard.press('ArrowLeft'); await p.waitForTimeout(100);
  assert.match(await lb.innerText(), /3 of 3/); ok('the buttons and the arrow keys move between pictures, round the ends');
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  assert.equal(await lb.count(), 0); assert.equal(await sheet().count(), 1); assert.match(await p.evaluate(() => document.activeElement?.className || ''), /mk-thumb/); ok('Escape closes only the lightbox and focus goes back to the picture');
  await strip.getByRole('button').first().click(); await p.waitForTimeout(150);
  await p.mouse.click(1200, 500); await p.waitForTimeout(200);
  assert.equal(await lb.count(), 0); assert.equal(await sheet().count(), 1); ok('a click outside the picture closes the lightbox too');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);
  assert.equal(await sheet().count(), 0); ok('Escape again closes the sheet');
  await p.evaluate(() => { globalThis.__gallery = 'none'; });
  await rows().first().click(); await p.waitForTimeout(500);
  assert.equal(await sheet().getByRole('list', { name: /Pictures of/ }).count(), 0); ok('a core with no pictures shows no strip');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);
  await p.evaluate(() => { globalThis.__gallery = undefined; });
  await search(''); await p.waitForTimeout(200);

  // error and offline states (the mock switches modes through window.__mkt)
  await p.evaluate(() => { globalThis.__mkt = 'offline'; });
  await p.getByRole('button', { name: 'Refresh' }).click(); await p.waitForTimeout(500);
  assert.match(await p.locator('.mk-note').innerText(), /Showing the saved list from 3 days ago\. Could not reach the server/); ok('offline: the saved list is shown with its age and the reason');
  await search('chiskit'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(500);
  assert.doesNotMatch(await sheet().innerText(), /used AI tools/); ok('no AI disclaimer is shown');
  await p.keyboard.press('Escape'); await p.waitForTimeout(150);
  await search('snes'); await p.waitForTimeout(200);
  await rows().first().click(); await p.waitForTimeout(500);
  await tab('Details');
  assert.match(await sheet().innerText(), /Pocket firmware\s*2\.0 or newer/); assert.match(await sheet().innerText(), /Earlier versions \(1\)/); ok('the firmware requirement and the version history come from the catalog');
  await sheet().locator('summary').click();
  await sheet().getByRole('button', { name: 'Choose this version' }).click(); await p.waitForTimeout(200);
  assert.ok(await sheet().getByRole('button', { name: 'Install version 0.9.0' }).count() === 1); ok('an earlier version can be chosen and the install button names it');
  await sheet().getByRole('button', { name: 'Install version 0.9.0' }).click(); await p.waitForTimeout(500);
  assert.match(await sheet().innerText(), /Downloaded/); ok('the chosen older version is downloaded and goes through the same reviewed plan');
  await sheet().getByRole('button', { name: 'Use the newest instead' }).click(); await p.waitForTimeout(200);
  assert.equal(await sheet().getByRole('button', { name: /^(Install|Update to)/ }).count(), 1); ok('choosing the newest again resets the install step');
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

  // Several sources: the openFPGA Library and Tau Alpha (channels Stable / Preview / Dev).
  await openMarket();
  const sources = p.getByRole('group', { name: 'Source' });
  assert.match(await sources.innerText(), /All sources[\s\S]*openFPGA Library[\s\S]*Tau Alpha/); ok('the sources are offered as filters when more than one lists cores');
  await sources.getByRole('button', { name: 'Tau Alpha' }).click(); await p.waitForTimeout(250);
  assert.equal(await rows().count(), 1); ok('Tau Alpha lists one Tau card, not one per channel');
  await sources.getByRole('button', { name: 'openFPGA Library' }).click(); await p.waitForTimeout(250);
  assert.equal(await rows().filter({ hasText: 'Tau' }).count() >= 1, true); ok('the same card is also under the other marketplace, counted once');
  await sources.getByRole('button', { name: 'Tau Alpha' }).click(); await p.waitForTimeout(250);
  const gridText = await p.locator('.mk-grid').innerText(); for (const c of ['Stable', 'Preview', 'Dev']) assert.ok(gridText.includes(c), c); ok('the card shows the channels it offers');
  await rows().first().click(); await p.waitForTimeout(300);
  await tab('Details');
  assert.match(await sheet().innerText(), /Distributed by[\s\S]*openFPGA Library · 0\.6\.1[\s\S]*Tau Alpha · 0\.5\.1/); ok('a core listed by two marketplaces shows both as tags');
  const sel = (name) => sheet().locator('label', { hasText: name }).locator('select');
  assert.deepEqual(await sel('Channel').locator('option').allInnerTexts(), ['Stable', 'Preview', 'Dev']); ok('a Channel dropdown offers Stable, Preview and Dev');
  assert.match((await sel('Version').locator('option').allInnerTexts()).join('|'), /0\.5\.1.*newest.*0\.5\.0/); ok('the Version dropdown lists the releases of that channel, newest first');
  await sel('Build').selectOption('diag'); await p.waitForTimeout(150);
  assert.match((await sel('Version').locator('option').allInnerTexts()).join('|'), /0\.5\.1/); ok('Build switches to the Diagnostics core');
  await sel('Channel').selectOption('Preview'); await p.waitForTimeout(150);
  assert.match((await sel('Version').locator('option').allInnerTexts()).join('|'), /0\.6\.0-preview\.1/); ok('changing the channel changes the versions');
  await sel('Channel').selectOption('Dev'); await p.waitForTimeout(150);
  assert.match(await sheet().innerText(), /never published/); assert.equal(await sheet().getByRole('button', { name: /Install a local build/ }).count(), 1); ok('Dev explains it is local and offers the local install');
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  const dflt = p.getByRole('combobox', { name: 'Marketplace to check for updates' });
  assert.equal(await dflt.inputValue(), 'openfpga-library'); await dflt.selectOption('tau-alpha'); await p.waitForTimeout(500);
  assert.equal(await dflt.inputValue(), 'tau-alpha'); ok('the default marketplace for updates can be chosen and is remembered');

  // Loading: skeleton cards while the list is on its way, hidden from screen readers, replaced by the real cards.
  await p.goto(url); await p.evaluate(() => { localStorage.clear(); window.__mkt = 'slow'; });
  await nav('Marketplace').click(); await p.waitForTimeout(250);
  assert.equal(await p.locator('[data-testid=mk-skeleton] .mk-skel').count(), 9); ok('a skeleton of cards shows while the list loads');
  assert.equal(await p.locator('[data-testid=mk-skeleton]').getAttribute('aria-hidden'), 'true');
  assert.match(await p.locator('.mk-note').innerText(), /Loading the list/); ok('the skeleton is hidden from screen readers and the status says what is happening');
  await p.waitForTimeout(1800);
  assert.equal(await p.locator('[data-testid=mk-skeleton]').count(), 0); assert.ok(await rows().count() > 0); ok('the skeleton is replaced by the real cards');

  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
