// Behaviour & WCAG contrast checks for System/Light/Dark themes, Primary color selector,
// tabbed Settings organization, sidebar Card switcher cleanup, and shared Music library deduplication.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { chromium } = require('playwright');
const findBrowser = () => {
  for (const p of ['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', '/Applications/Chromium.app/Contents/MacOS/Chromium']) {
    if (fs.existsSync(p)) return p;
  }
  return undefined;
};

let passed = 0;
const ok = (msg) => {
  passed++;
  console.log('ok - ' + msg);
};

function parseRgb(str) {
  const m = /rgba?\((\d+),\s*(\d+),\s*(\d+)/.exec(str || '');
  return m ? [+m[1], +m[2], +m[3]] : null;
}

function relLuminance([r, g, b]) {
  const f = (v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

function contrastRatio(rgb1, rgb2) {
  const l1 = relLuminance(rgb1);
  const l2 = relLuminance(rgb2);
  return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
}

(async () => {
  const url = process.env.URL || 'http://localhost:5173';
  const executablePath = process.env.CHROMIUM_PATH || findBrowser();
  const b = await chromium.launch({ executablePath });
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: 'dark' });
  const p = await ctx.newPage();

  const errors = [];
  p.on('pageerror', (e) => errors.push(e.message));
  p.on('console', (m) => m.text().startsWith('[dev-mock] unmocked') && errors.push(m.text()));

  await p.goto(url);
  await p.evaluate(() => localStorage.clear());
  await p.goto(url);
  await p.waitForTimeout(800);

  // 1. Starts in System with <meta name="color-scheme" content="light dark"> and default Sage accent
  assert.equal(await p.evaluate(() => document.querySelector('meta[name="color-scheme"]')?.getAttribute('content')), 'light dark');
  assert.equal(await p.evaluate(() => localStorage.getItem('tau.theme')), null);
  assert.equal(await p.evaluate(() => localStorage.getItem('tau.accent')), null);
  ok('defaults to System theme and Sage primary color with no stored override');

  assert.equal(await p.evaluate(() => getComputedStyle(document.body).backgroundColor), 'rgb(19, 19, 22)');
  ok('follows OS dark preference in System mode');

  // 2. Sidebar switcher shows cards only (no Target core section)
  await p.getByRole('button', { name: 'Switch card' }).click();
  await p.waitForTimeout(150);
  const swText = await p.locator('.switcher-panel').innerText();
  assert.doesNotMatch(swText, /Target core/i);
  await p.keyboard.press('Escape');
  await p.waitForTimeout(100);
  ok('sidebar card switcher omits the redundant Target core section');

  const openSettings = async (tabName) => {
    const grp = p.locator('nav button', { hasText: 'Tools & settings' });
    if ((await grp.getAttribute('aria-expanded')) !== 'true') await grp.click();
    await p.locator('nav button', { hasText: /^Settings$/ }).click();
    await p.waitForTimeout(250);
    if (tabName) {
      await p.getByRole('tab', { name: tabName }).click();
      await p.waitForTimeout(200);
    }
  };

  // 3. Navigate to Settings and verify tabbed categories
  await openSettings();

  const tabs = await p.locator('.settings-tabs [role=tab]').allInnerTexts();
  assert.deepEqual(tabs, ['General', 'Appearance', 'Music & Sync', 'Artwork', 'Diagnostics']);
  ok('Settings is organized into General, Appearance, Music & Sync, Artwork, and Diagnostics tabs');

  // Switch to Appearance tab
  await p.getByRole('tab', { name: 'Appearance' }).click();
  await p.waitForTimeout(200);

  const appCard = p.locator('.theme-settings');
  assert.equal(await appCard.getByRole('heading', { name: 'Appearance' }).count(), 1);
  assert.equal(await appCard.getByRole('radio', { name: /System/ }).isChecked(), true);
  assert.match(await appCard.innerText(), /System \(currently dark\)/);
  ok('Settings > Appearance displays System, Light, and Dark options with live system state');

  // 4. Emulating OS light while in System updates rendered theme & live label
  await p.emulateMedia({ colorScheme: 'light' });
  await p.waitForTimeout(200);
  assert.match(await appCard.innerText(), /System \(currently light\)/);
  assert.equal(await p.evaluate(() => getComputedStyle(document.body).backgroundColor), 'rgb(247, 247, 248)');
  ok('System option live-updates when OS switches to light');

  // 5. Selecting Light pins light theme even when OS switches to dark and persists across reload
  await appCard.getByRole('radio', { name: /^Light/ }).check();
  await p.waitForTimeout(150);
  assert.equal(await p.evaluate(() => document.querySelector('meta[name="color-scheme"]')?.getAttribute('content')), 'light');
  assert.equal(await p.evaluate(() => localStorage.getItem('tau.theme')), 'light');
  await p.emulateMedia({ colorScheme: 'dark' });
  await p.waitForTimeout(200);
  assert.equal(await p.evaluate(() => getComputedStyle(document.body).backgroundColor), 'rgb(247, 247, 248)');
  await p.reload();
  await p.waitForTimeout(800);
  assert.equal(await p.evaluate(() => getComputedStyle(document.body).backgroundColor), 'rgb(247, 247, 248)');
  assert.equal(await p.evaluate(() => document.querySelector('meta[name="color-scheme"]')?.getAttribute('content')), 'light');
  ok('Light option persists across reload and stays light even when OS is dark');

  // 6. Primary color preset (Blue) in Light & Dark modes + reload persistence
  await openSettings('Appearance');

  await appCard.getByRole('radio', { name: 'Blue' }).check();
  await p.waitForTimeout(150);
  assert.equal(await p.evaluate(() => localStorage.getItem('tau.accent')), 'cobalt');
  await p.locator('nav button', { hasText: /^Card$/ }).click();
  await p.waitForTimeout(300);
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('button.primary')).backgroundColor), 'rgb(40, 93, 199)');
  await p.reload();
  await p.waitForTimeout(800);
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('button.primary')).backgroundColor), 'rgb(40, 93, 199)');
  ok('Blue primary preset applies in Light mode and persists across reload');

  // Verify Blue in Dark mode as well
  await openSettings('Appearance');
  await appCard.getByRole('radio', { name: /^Dark/ }).check();
  await p.locator('nav button', { hasText: /^Card$/ }).click();
  await p.waitForTimeout(250);
  assert.equal(await p.evaluate(() => getComputedStyle(document.querySelector('button.primary')).backgroundColor), 'rgb(158, 195, 255)');
  ok('Blue primary preset switches to accessible light tint in Dark mode');

  // 7. Challenging Custom primary color (#ffff00) maintains >= 4.5:1 button contrast in BOTH Light & Dark modes
  await openSettings('Appearance');
  await appCard.getByLabel('Custom primary color').fill('#ffff00');
  await p.waitForTimeout(150);
  assert.equal(await p.evaluate(() => localStorage.getItem('tau.accent')), 'custom:#ffff00');

  for (const modeName of ['Light', 'Dark']) {
    await appCard.getByRole('radio', { name: new RegExp('^' + modeName) }).check();
    await p.waitForTimeout(100);
    await p.locator('nav button', { hasText: /^Card$/ }).click();
    await p.waitForTimeout(200);
    const btnCheck = await p.evaluate(() => {
      const bEl = document.querySelector('button.primary');
      return { bg: getComputedStyle(bEl).backgroundColor, fg: getComputedStyle(bEl).color };
    });
    assert.ok(contrastRatio(parseRgb(btnCheck.bg), parseRgb(btnCheck.fg)) >= 4.5, `Custom color contrast failed in ${modeName} mode: ${JSON.stringify(btnCheck)}`);
    await p.locator('nav button', { hasText: /^Settings$/ }).click();
    await p.getByRole('tab', { name: 'Appearance' }).click();
  }
  ok('Custom primary color (#ffff00) derives >= 4.5:1 WCAG contrast in both Light and Dark modes');

  // Reset to Sage & switch back to Light for multi-view contrast sweep
  await p.getByRole('button', { name: 'Reset to Sage' }).click();
  await p.waitForTimeout(100);
  assert.equal(await p.evaluate(() => localStorage.getItem('tau.accent')), null);
  assert.equal(await p.evaluate(() => localStorage.getItem('tau.accentVars')), null);
  await appCard.getByRole('radio', { name: /^Light/ }).check();
  await p.waitForTimeout(150);
  ok('Reset to Sage clears stored primary color overrides');

  // 8. Check contrast across main views in Light mode
  const viewsToCheck = [
    { nav: /^Card$/ },
    { nav: 'Marketplace' },
    { nav: /^Music$/ },
    { nav: 'Appearance' },
    { expand: 'Help', nav: 'Using Tau Omega' },
    { expand: 'Tools & settings', nav: /^Settings$/ },
  ];
  for (const v of viewsToCheck) {
    if (v.expand) {
      const grp = p.locator('nav button', { hasText: v.expand });
      if ((await grp.getAttribute('aria-expanded')) !== 'true') await grp.click();
    }
    await p.locator('nav button', { hasText: v.nav }).click();
    await p.waitForTimeout(350);
    const check = await p.evaluate(() => {
      const bodyBg = getComputedStyle(document.body).backgroundColor;
      const h1 = document.querySelector('h1');
      const h1Color = h1 ? getComputedStyle(h1).color : null;
      const btn = document.querySelector('button.primary');
      const btnBg = btn ? getComputedStyle(btn).backgroundColor : null;
      const btnFg = btn ? getComputedStyle(btn).color : null;
      return { bodyBg, h1Color, btnBg, btnFg };
    });
    assert.ok(contrastRatio(parseRgb(check.bodyBg), parseRgb(check.h1Color)) >= 4.5, `Heading contrast too low in light mode: ${JSON.stringify(check)}`);
    if (check.btnBg && check.btnFg) {
      assert.ok(contrastRatio(parseRgb(check.btnBg), parseRgb(check.btnFg)) >= 4.5, `Primary button contrast too low in light mode: ${JSON.stringify(check)}`);
    }
  }
  ok('headings and primary buttons meet WCAG AA (>= 4.5:1) contrast across views in Light mode');

  // 9. Open Cores + Grid + Game Details in Light mode, and confirm no double scrollbar
  await p.locator('nav').getByText('Game Boy Advance').click();
  await p.waitForTimeout(250);
  await p.locator('.pv-open', { hasText: 'Spiritualized.GBA' }).click();
  await p.waitForTimeout(450);
  await p.getByRole('group', { name: 'View mode' }).getByRole('button', { name: 'Grid' }).click();
  await p.waitForTimeout(150);
  const gridOverflow = await p.evaluate(() => getComputedStyle(document.querySelector('.cf-grid')).overflowY);
  assert.equal(gridOverflow, 'visible', 'Expected .cf-grid not to create a nested vertical scrollbar');
  await p.getByRole('group', { name: 'View mode' }).getByRole('button', { name: 'List' }).click();
  await p.waitForTimeout(150);
  await p.getByRole('tabpanel').locator('.cf-row').first().locator('.cf-row-main').click();
  await p.waitForTimeout(250);
  assert.match(await p.locator('#core-details').innerText(), /About this game/i);
  ok('Cores Grid has no double scrollbar and Game Details renders cleanly in Light mode');

  // 9b. Verify Pocket banners invert cleanly in Light mode and stay uninverted in Dark mode
  await p.locator('nav .nav-group', { hasText: 'Cores' }).click();
  await p.waitForTimeout(300);
  const lightBannerStyle = await p.locator('.ov-art.is-pocket-art').first().evaluate((el) => {
    const img = el.querySelector('img');
    return { bg: getComputedStyle(el).backgroundColor, filter: img ? getComputedStyle(img).filter : '' };
  });
  assert.equal(lightBannerStyle.bg, 'rgb(255, 255, 255)');
  assert.match(lightBannerStyle.filter, /invert\(1\)/);

  await p.locator('nav button', { hasText: /^Settings$/ }).click();
  await p.getByRole('tab', { name: 'Appearance' }).click();
  await appCard.getByRole('radio', { name: /^Dark\b/ }).check();
  await p.waitForTimeout(150);
  await p.locator('nav .nav-group', { hasText: 'Cores' }).click();
  await p.waitForTimeout(300);
  const darkBannerStyle = await p.locator('.ov-art.is-pocket-art').first().evaluate((el) => {
    const img = el.querySelector('img');
    return { bg: getComputedStyle(el).backgroundColor, filter: img ? getComputedStyle(img).filter : '' };
  });
  assert.equal(darkBannerStyle.bg, 'rgb(0, 0, 0)');
  assert.equal(darkBannerStyle.filter, 'none');
  ok('Pocket platform banners invert onto white in Light mode and stay uninverted on black in Dark mode');

  // 10. Switch back to System
  await p.locator('nav button', { hasText: /^Settings$/ }).click();
  await p.getByRole('tab', { name: 'Appearance' }).click();
  await appCard.getByRole('radio', { name: /System/ }).check();
  await p.waitForTimeout(150);
  assert.equal(await p.evaluate(() => localStorage.getItem('tau.theme')), null);
  assert.equal(await p.evaluate(() => document.querySelector('meta[name="color-scheme"]')?.getAttribute('content')), 'light dark');
  ok('System option clears stored override and restores light dark meta');

  assert.deepEqual(errors, []);
  ok('no page errors and no unmocked commands');
  console.log(`\n${passed} checks passed`);
  await b.close();
})().catch((e) => {
  console.error('FAILED:', e.message);
  process.exit(1);
});
