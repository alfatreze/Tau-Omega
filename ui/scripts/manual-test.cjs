// Behaviour checks for "Add a core manually" (zip, folder, GitHub address; preview; trust tick; reviewed install) against the dev mock.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/manual-test.cjs   (URL=... to point elsewhere)
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
  const sheet = () => p.locator('.ma');
  const addr = () => sheet().getByRole('textbox');
  const nav = (n) => p.locator('nav button', { hasText: n }).first();
  const text = () => sheet().innerText();
  const type = async (v) => { await addr().fill(v); await p.waitForTimeout(450); };
  const read = async () => { await sheet().getByRole('button', { name: /Read this core/ }).click(); await p.waitForTimeout(500); };

  // ---- entry points
  await nav('Marketplace').click(); await p.waitForTimeout(500);
  await p.getByRole('button', { name: /Add a core manually/ }).click(); await p.waitForTimeout(300);
  assert.equal(await sheet().count(), 1); assert.match(await text(), /Choose a zip file[\s\S]*Choose a folder[\s\S]*paste a GitHub address/i); ok('the Marketplace opens the sheet with three ways to add a core');
  assert.equal(await sheet().getByRole('button', { name: /Read this core/ }).isDisabled(), true); ok('nothing can be read before a source is given');

  // ---- the address box explains itself
  await type('http://github.com/o/r'); assert.match(await text(), /Use the https:\/\/ address/); assert.equal(await sheet().getByRole('button', { name: /Read this core/ }).isDisabled(), true); ok('a plain http address is refused with the reason');
  await type('https://example.org/x.zip'); assert.match(await text(), /Only github\.com addresses/); ok('another host is refused with the reason');
  await type('https://github.com/o/r/blob/main/README.md'); assert.match(await text(), /page inside the repository/); ok('a page inside a repository is refused with the reason');
  await type('https://github.com/someone/unlisted-core'); assert.match(await text(), /GitHub repository: its newest release/); assert.equal(await sheet().getByRole('checkbox', { name: /README/ }).isChecked(), true); ok('a repository address is accepted and README reading is on by default');

  // ---- a GitHub repository: preview
  await read();
  let t = await text();
  assert.match(t, /Unlisted[\s\S]*someone · version 1\.0\.0[\s\S]*Computer › Somebody Ltd\. › Unlisted Computer/); ok('the preview names the core and where it sits in the Library grouping');
  assert.match(t, /Not in the openFPGA Library\. Omega cannot tell what this bitstream does/); ok('the unlisted-core warning is always shown');
  assert.match(t, /Pocket firmware\s*2\.1 or newer[\s\S]*Dock\s*Yes[\s\S]*Link port\s*No[\s\S]*Sleep\s*Yes/); ok('requirements come from the core');
  assert.match(t, /You supply 1 file: unlisted\.rom[\s\S]*unlisted\.rom[\s\S]*you supply this[\s\S]*Assets\/unlisted\/common[\s\S]*pal\.bin[\s\S]*in the package/i); ok('files it needs are listed and the ones to supply are called out');
  assert.match(t, /Scanlines · Region · Overscan/); assert.match(t, /Ported by someone/); ok('settings and the author\'s text are shown');
  assert.match(t, /README[\s\S]*Put your BIOS in the common folder/); assert.equal(await sheet().locator('.ma-shots img').count(), 2); ok('the README and its pictures are shown');
  assert.match(t, /GitHub: someone\/unlisted-core · v1\.0[\s\S]*Matches the checksum GitHub publishes/); ok('provenance and the verified checksum are shown');
  const tick = sheet().getByRole('checkbox', { name: /I trust this source/ });
  assert.equal(await sheet().getByRole('button', { name: /Review|Install/i }).count(), 0, await sheet().getByRole('button').allInnerTexts().then((x) => x.join('|'))); ok('no install plan before the trust tick');
  await tick.check(); await p.waitForTimeout(500);
  assert.equal(await sheet().getByRole('region', { name: 'Install review' }).count(), 1); assert.match(await text(), /Install .* on Pocket/); ok('ticking the trust box shows the install review: what, one summary line, one button');
  await sheet().getByRole('button', { name: 'Cancel' }).click(); await p.waitForTimeout(200);
  assert.equal(await sheet().count(), 0); ok('Cancel closes the sheet');

  // ---- refusals and notes
  await nav('Marketplace').click(); await p.getByRole('button', { name: /Add a core manually/ }).click(); await p.waitForTimeout(200);
  await type('/mock/Tau Player.zip'); await read(); assert.match(await text(), /This is a Tau core\. Tau cores are installed and updated from Packages/); assert.equal(await sheet().getByRole('button', { name: /Read this core/ }).count(), 1); ok('a Tau core is refused with where to install it instead');
  await type('/mock/refused.zip'); await read(); assert.match(await text(), /carries user data/); ok('a package with user data is refused with the reason');
  await type('/mock/library.zip'); await read(); assert.match(await text(), /id is in the Library[\s\S]*Marketplace/); ok('a core already in the Library says so');
  await sheet().getByRole('button', { name: 'Back' }).click(); await type('/mock/installed.zip'); await read(); assert.match(await text(), /Already on this card as version 0\.9\.0/); ok('a core already installed says so');
  await sheet().getByRole('button', { name: 'Back' }).click(); await type('/mock/other.zip'); await read();
  assert.match(await text(), /added before from a different source/); assert.equal(await sheet().getByRole('checkbox', { name: /I trust/ }).isDisabled(), true); ok('the same id from a different source is blocked');
  await sheet().getByRole('button', { name: 'Back' }).click(); await type('/mock/nopic.zip'); await read(); assert.match(await text(), /No valid icon\.bin/); assert.match(await text(), /A zip file on this computer/); assert.match(await text(), /Computed here; nothing to compare it with/); ok('a local zip shows its warnings and says its checksum is only computed here');

  // ---- a folder, and a release with several zips
  await sheet().getByRole('button', { name: 'Back' }).click(); await type('/mock/Some Core Folder'); await read(); assert.match(await text(), /A folder on this computer \(BareCore\)/); ok('a folder is packed and described');
  await sheet().getByRole('button', { name: 'Back' }).click(); await type('https://github.com/someone/multi-core'); await read();
  t = await text(); assert.match(t, /has 2 releases\. Choose the one to add/); assert.match(t, /pre-release/); ok('a repository with several zips asks which one');
  await sheet().getByLabel('Release').selectOption('v1.9'); await p.waitForTimeout(150);
  assert.match(await sheet().getByLabel('Zip').innerText(), /Some\.Core_1\.9\.zip/); assert.ok(!/Some\.Core_2\.0_dock/.test(await sheet().getByLabel('Zip').innerText())); ok('the zip list follows the chosen release');
  await sheet().getByRole('button', { name: 'Use this zip' }).click(); await p.waitForTimeout(500);
  assert.match(await text(), /GitHub: someone\/unlisted-core · v1\.9[\s\S]*Some\.Core_1\.9\.zip/); ok('the chosen release and zip are the ones read');
  await p.keyboard.press('Escape'); await p.waitForTimeout(200); assert.equal(await sheet().count(), 0); ok('Escape closes the sheet');

  // ---- the Cores page: button and drop
  await p.locator('.nt-item[data-kind="platform"]').first().click(); await p.locator('.cores .crumbs button', { hasText: 'Cores' }).first().click().catch(() => {}); await p.waitForTimeout(300);
  await p.getByRole('button', { name: 'Add a core…' }).click(); await p.waitForTimeout(300); assert.equal(await sheet().count(), 1); ok('the Cores overview has an Add a core button');
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  const dt = await p.evaluateHandle(() => { const d = new DataTransfer(); d.items.add(new File(['x'], 'Dropped Core.zip')); return d; });
  await p.dispatchEvent('body', 'dragover', { dataTransfer: dt, clientX: 600, clientY: 400 }); await p.dispatchEvent('body', 'drop', { dataTransfer: dt, clientX: 600, clientY: 400 }); await p.waitForTimeout(700);
  assert.match(await text(), /Unlisted[\s\S]*Not in the openFPGA Library/); ok('dropping a zip on the Cores page reads it straight away');

  // ---- accessibility basics
  const audit = await p.evaluate(() => { const d = document.querySelector('.ma'); const name = (el) => (el.getAttribute('aria-label') || el.innerText || el.labels?.[0]?.innerText || el.getAttribute('placeholder') || '').trim();
    const bad = [...d.querySelectorAll('button,input,select')].filter((e) => !name(e)).map((e) => e.outerHTML.slice(0, 80)); return { bad, modal: d.getAttribute('aria-modal'), label: !!document.getElementById(d.getAttribute('aria-labelledby')) }; });
  assert.deepEqual(audit.bad, []); assert.equal(audit.modal, 'true'); assert.ok(audit.label); ok('every control in the sheet has a name and the dialog is labelled and modal');
  await p.setViewportSize({ width: 375, height: 800 }); await p.waitForTimeout(300);
  assert.ok(await p.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)); ok('at phone width the sheet does not scroll the page sideways');

  // ---- the core check inside the preview
  await p.setViewportSize({ width: 1440, height: 900 }); await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(1100); await nav('Marketplace').click(); await p.waitForTimeout(500);
  await p.getByRole('button', { name: /Add a core manually/ }).click(); await p.waitForTimeout(300);
  await type('https://github.com/someone/unlisted-core'); await read();
  assert.equal(await sheet().getByRole('heading', { name: 'Core check' }).count(), 0); ok('a clean core shows no core-check section');
  await sheet().getByRole('button', { name: 'Back' }).click(); await p.waitForTimeout(200);
  await type('https://github.com/someone/warnlint'); await read();
  assert.match(await text(), /Core check[\s\S]*No errors, 1 warning[\s\S]*description is 71 characters[\s\S]*Shorten the description/i); ok('a warning is listed with its fix and the rule it comes from');
  assert.equal(await sheet().getByRole('checkbox', { name: /I trust this source/ }).isDisabled(), false); ok('warnings do not block the install');
  await sheet().getByRole('button', { name: 'Back' }).click(); await p.waitForTimeout(200);
  await type('https://github.com/someone/badlint'); await read();
  assert.match(await text(), /1 error found, 1 warning[\s\S]*bitstream file bitstream\.rbf_r is missing/i); ok('an error is listed first');
  const trustBox = sheet().getByRole('checkbox', { name: /I trust this source/ });
  assert.equal(await trustBox.isDisabled(), true); ok('an error blocks the install');
  await sheet().getByRole('checkbox', { name: /Install anyway/ }).check();
  assert.equal(await trustBox.isDisabled(), false); ok('ticking Install anyway lets the developer go on');

  assert.deepEqual(errors, [], errors.join(' | ')); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
