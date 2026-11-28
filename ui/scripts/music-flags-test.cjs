// Behaviour checks for auto-detected Music Core feature flags (Tau Alpha & HarpMudd),
// Settings overrides, persistence, sidebar/tab gating, and target core filtering.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { chromium } = require('playwright');

const findBrowser = () => {
  for (const p of [
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Chromium.app/Contents/MacOS/Chromium',
  ]) {
    if (fs.existsSync(p)) return p;
  }
  return undefined;
};

let passed = 0;
const ok = (msg) => {
  passed++;
  console.log('ok - ' + msg);
};

async function navButtons(page) {
  return page.locator('#sidebar nav > button').allInnerTexts();
}

async function openSettings(page) {
  const moreBtn = page.locator('#sidebar nav button.nav-group', { hasText: /Tools & settings/ });
  if ((await moreBtn.getAttribute('aria-expanded')) !== 'true') {
    await moreBtn.click();
    await page.waitForTimeout(150);
  }
  await page.locator('#more-tools button', { hasText: 'Settings' }).click();
  await page.waitForTimeout(300);
}

(async () => {
  const baseUrl = process.env.UI || process.env.URL || 'http://localhost:5173';
  const executablePath = process.env.CHROMIUM_PATH || findBrowser();
  const browser = await chromium.launch({ executablePath });
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const p = await ctx.newPage();

  const errors = [];
  p.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
  p.on('console', (m) => {
    if (m.type() === 'error') errors.push(`console.error: ${m.text()}`);
  });

  // --- Test 1: Default card (both Tau Alpha and HarpMudd present) ---
  await p.goto(baseUrl, { waitUntil: 'networkidle' });
  await p.evaluate(() => localStorage.removeItem('tau.musicFlags'));
  await p.reload({ waitUntil: 'networkidle' });
  await p.waitForTimeout(400);

  let nav = await navButtons(p);
  for (const label of ['Music', 'Playlists', 'Meter Lab', 'Appearance', 'Halcyon EQ']) {
    assert.ok(nav.some((t) => t.trim() === label), `Expected nav item ${label}`);
  }
  await openSettings(p);
  let settingsTabs = await p.locator('.settings-tabs [role="tab"]').allInnerTexts();
  assert.ok(settingsTabs.includes('Music & Sync'));
  ok('default card with both Tau Alpha and HarpMudd shows all music features and Music & Sync tab');

  // --- Test 2: HarpMudd-only card (?card_cores=harpmudd) ---
  await p.goto(`${baseUrl}?card_cores=harpmudd`, { waitUntil: 'networkidle' });
  await p.waitForTimeout(400);
  nav = await navButtons(p);
  assert.ok(nav.some((t) => t.trim() === 'Music'), 'Music should be shown for HarpMudd');
  for (const hidden of ['Playlists', 'Meter Lab', 'Appearance', 'Halcyon EQ']) {
    assert.ok(!nav.some((t) => t.trim() === hidden), `${hidden} should be hidden for HarpMudd-only card`);
  }
  await openSettings(p);
  settingsTabs = await p.locator('.settings-tabs [role="tab"]').allInnerTexts();
  assert.ok(settingsTabs.includes('Music & Sync'), 'Music & Sync should be shown for HarpMudd-only card');

  await p.locator('#sidebar nav > button', { hasText: /^Music$/ }).click();
  await p.waitForTimeout(500);
  const hmOptions = await p.getByRole('combobox', { name: 'Target core' }).innerText();
  assert.match(hmOptions, /Mp3Player/);
  assert.doesNotMatch(hmOptions, /omega/i);
  ok('HarpMudd-only card shows Music + Music & Sync for Mp3Player while hiding Playlists, Meter Lab, Appearance, and Halcyon EQ');

  // --- Test 3: Tau Alpha-only card (?card_cores=tau) ---
  await p.goto(`${baseUrl}?card_cores=tau`, { waitUntil: 'networkidle' });
  await p.waitForTimeout(400);
  nav = await navButtons(p);
  for (const label of ['Music', 'Playlists', 'Meter Lab', 'Appearance', 'Halcyon EQ']) {
    assert.ok(nav.some((t) => t.trim() === label), `Expected nav item ${label} on Tau-only card`);
  }
  await openSettings(p);
  settingsTabs = await p.locator('.settings-tabs [role="tab"]').allInnerTexts();
  assert.ok(settingsTabs.includes('Music & Sync'));
  await p.locator('#sidebar nav > button', { hasText: /^Music$/ }).click();
  await p.waitForTimeout(500);
  const tauOptions = await p.getByRole('combobox', { name: 'Target core' }).innerText();
  assert.match(tauOptions, /omega/i);
  assert.doesNotMatch(tauOptions, /Mp3Player/);
  ok('Tau Alpha-only card shows all Tau features and lists only Tau in Target core');

  // --- Test 4: Gaming-only card (?card_cores=none) ---
  await p.goto(`${baseUrl}?card_cores=none`, { waitUntil: 'networkidle' });
  await p.waitForTimeout(400);
  nav = await navButtons(p);
  for (const hidden of ['Music', 'Playlists', 'Meter Lab', 'Appearance', 'Halcyon EQ']) {
    assert.ok(!nav.some((t) => t.trim() === hidden), `${hidden} should be hidden on gaming-only card`);
  }
  assert.equal(await p.locator('#cards').innerText().then((t) => /Music libraries/i.test(t)), false);
  await openSettings(p);
  settingsTabs = await p.locator('.settings-tabs [role="tab"]').allInnerTexts();
  assert.ok(!settingsTabs.includes('Music & Sync'), 'Music & Sync tab should be hidden when both flags are off');
  ok('gaming-only card hides Music, Playlists, Meter Lab, Appearance, Halcyon EQ, Music libraries, and Settings > Music & Sync');

  // --- Test 5: Settings overrides & persistence ---
  // Still on ?card_cores=none -> General tab
  await p.getByRole('tab', { name: 'General' }).click();
  await p.waitForTimeout(200);
  const tauSel = p.getByRole('combobox', { name: 'Tau Alpha support' });
  const hmSel = p.getByRole('combobox', { name: 'HarpMudd support' });

  await hmSel.selectOption('on');
  await p.waitForTimeout(250);
  nav = await navButtons(p);
  assert.ok(nav.some((t) => t.trim() === 'Music'));
  assert.ok(!nav.some((t) => t.trim() === 'Meter Lab'));
  settingsTabs = await p.locator('.settings-tabs [role="tab"]').allInnerTexts();
  assert.ok(settingsTabs.includes('Music & Sync'));
  ok('forcing HarpMudd support On enables Music & Music & Sync without showing Tau Alpha features');

  await tauSel.selectOption('on');
  await p.waitForTimeout(250);
  nav = await navButtons(p);
  for (const label of ['Music', 'Playlists', 'Meter Lab', 'Appearance', 'Halcyon EQ']) {
    assert.ok(nav.some((t) => t.trim() === label), `Expected ${label} when Tau Alpha forced on`);
  }
  ok('forcing Tau Alpha support On enables all Tau Alpha features even on a gaming-only card');

  // Reset to Auto and switch to default card, then test forcing Tau Alpha Off while HarpMudd stays Auto
  await tauSel.selectOption('auto');
  await hmSel.selectOption('auto');
  await p.goto(baseUrl, { waitUntil: 'networkidle' });
  await p.waitForTimeout(400);
  await openSettings(p);
  await p.getByRole('combobox', { name: 'Tau Alpha support' }).selectOption('off');
  await p.waitForTimeout(250);

  await p.reload({ waitUntil: 'networkidle' });
  await p.waitForTimeout(400);
  nav = await navButtons(p);
  assert.ok(nav.some((t) => t.trim() === 'Music'), 'Music stays visible via HarpMudd when Tau Alpha forced off');
  for (const hidden of ['Playlists', 'Meter Lab', 'Appearance', 'Halcyon EQ']) {
    assert.ok(!nav.some((t) => t.trim() === hidden), `${hidden} should stay hidden after reload when Tau Alpha forced off`);
  }
  assert.equal(await p.locator('#cards').innerText().then((t) => /Music libraries/i.test(t)), false);
  await p.locator('#sidebar nav > button', { hasText: /^Music$/ }).click();
  await p.waitForTimeout(500);
  const afterOffOpts = await p.getByRole('combobox', { name: 'Target core' }).innerText();
  assert.match(afterOffOpts, /Mp3Player/);
  assert.doesNotMatch(afterOffOpts, /omega/i);
  ok('forcing Tau Alpha support Off persists across reload, hides Tau features + Music libraries, and keeps HarpMudd Music active');

  // Now force HarpMudd Off too from Music & Sync context -> falls back cleanly
  await openSettings(p);
  await p.getByRole('tab', { name: 'Music & Sync' }).click();
  await p.waitForTimeout(150);
  await p.getByRole('tab', { name: 'General' }).click();
  await p.waitForTimeout(150);
  await p.getByRole('combobox', { name: 'HarpMudd support' }).selectOption('off');
  await p.waitForTimeout(250);

  await p.reload({ waitUntil: 'networkidle' });
  await p.waitForTimeout(400);
  nav = await navButtons(p);
  for (const hidden of ['Music', 'Playlists', 'Meter Lab', 'Appearance', 'Halcyon EQ']) {
    assert.ok(!nav.some((t) => t.trim() === hidden));
  }
  await openSettings(p);
  settingsTabs = await p.locator('.settings-tabs [role="tab"]').allInnerTexts();
  assert.ok(!settingsTabs.includes('Music & Sync'));
  ok('forcing both Tau Alpha and HarpMudd Off persists across reload and hides all Music UI cleanly');

  assert.deepEqual(errors, [], errors.join(' | '));
  ok('no page errors and no unmocked commands');

  await browser.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => {
  console.error('\nFAILED:', e.message);
  process.exit(1);
});
