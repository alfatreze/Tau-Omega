// The Meter Lab shows Tau Alpha's pinned lab when ui/public/tau-lab/meter_lab.html is installed, and says so when it is not.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/meter-lab-test.cjs
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs'); const path = require('node:path');
const url = process.env.URL || 'http://localhost:5173';
const installed = fs.existsSync(path.join(__dirname, '../public/tau-lab/meter_lab.html'));
let passed = 0; const ok = (n) => { passed++; console.log(`ok - ${n}`); };
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1300, height: 1000 } });
  const errors = []; p.on('pageerror', (e) => errors.push(e.message));
  await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(800);
  await p.locator('nav button', { hasText: 'Meter Lab' }).first().click(); await p.waitForTimeout(1500);
  assert.match(await p.locator('h1').first().innerText(), /Meter Lab/); ok('the Meter Lab opens');
  if (installed) {
    const fr = p.frameLocator('iframe[title="Tau Alpha meter lab"]'); await p.waitForTimeout(2500);
    assert.match(await fr.locator('h1').innerText(), /Tau meter lab/); ok('the pinned lab page is shown');
    const st = p.locator('[data-testid=rml-status]');
    await p.waitForFunction(() => /meter modules? built/.test(document.querySelector('[data-testid=rml-status]')?.textContent || ''), null, { timeout: 15000 });
    assert.match(await st.innerText(), /\d+ meter modules? built/); ok('the lab reports its modules (tau-lab-ready) to the app');
    await p.waitForFunction(() => /Showing .+ \(module [0-9a-f]{12}/.test(document.querySelector('[data-testid=rml-status]')?.textContent || ''), null, { timeout: 8000 });
    ok('the lab is driven by the editor: it shows the edited meter with its module hash (tau-lab-set, tau-lab-applied)');
    assert.equal(await p.locator('[data-testid=rml-clamped], [data-testid=rml-ignored]').count(), 0); ok('a valid preset is applied with nothing ignored or clamped');
    const meters = await fr.locator('#meter option').allInnerTexts(); assert.ok(meters.includes('SOUND MAP')); ok('it offers the real meters, including the SOUND MAP wasm twin');
  } else {
    assert.equal(await p.locator('[data-testid=rml-missing]').count(), 1); assert.equal(await p.locator('iframe').count(), 0); ok('without the lab page it says so and shows no frame');
  }
  assert.equal(await p.locator('[data-testid=meter-presets]').count(), 1); ok('the preset editor is on the page');
  assert.deepEqual(errors, []); ok('no page errors');
  await b.close(); console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
