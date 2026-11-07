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
    assert.match(await fr.locator('h1').textContent(), /Tau meter lab/); ok('the pinned lab page is shown');
    const st = p.locator('[data-testid=rml-status]');
    await p.waitForFunction(() => /meters? ready/.test(document.querySelector('[data-testid=rml-status]')?.textContent || ''), null, { timeout: 15000 });
    assert.match(await st.innerText(), /\d+ meters? ready/); ok('the lab reports its modules (tau-lab-ready) to the app');
    await p.waitForFunction(() => /Showing .+\./.test(document.querySelector('[data-testid=rml-status]')?.textContent || ''), null, { timeout: 8000 });
    ok('the lab is driven by the editor: it shows the edited meter with its module hash (tau-lab-set, tau-lab-applied)');
    assert.equal(await p.locator('[data-testid=rml-clamped], [data-testid=rml-ignored]').count(), 0); ok('a valid preset is applied with nothing ignored or clamped');
    const prov = p.locator('[data-testid=rml-provenance]');
    await prov.waitFor({ timeout: 8000 });
    assert.equal(await prov.getAttribute('data-state'), 'match'); assert.match(await prov.innerText(), /All \d+ match what Tau Alpha published \([0-9a-f]{8}\)/); ok('every module the lab compiled matches the hash in the drop\'s MANIFEST.json (provenance check)');
    assert.match(await prov.getAttribute('title'), /built with .*clang.* from \d+ source files/); ok('the edited meter\'s compiler and number of sources are in the line\'s detail');
    // A manifest that disagrees about one module must be reported, naming the module.
    const p2 = await b.newPage({ viewport: { width: 1300, height: 1000 } });
    await p2.route('**/tau-lab/MANIFEST.json', async (route) => {
      const m = await (await route.fetch()).json();
      m.modules[0].wasm_sha256 = '0'.repeat(64); route.fulfill({ json: m });
    });
    await p2.goto(url); await p2.waitForTimeout(800);
    await p2.locator('nav button', { hasText: 'Meter Lab' }).first().click();
    const bad = p2.locator('[data-testid=rml-provenance]'); await bad.waitFor({ timeout: 15000 });
    assert.equal(await bad.getAttribute('data-state'), 'mismatch'); assert.match(await bad.innerText(), /chladni \(different\)/); ok('a module whose hash differs from the manifest is reported by name');
    await p2.close();
    const meters = await fr.locator('#meter option').allTextContents(); assert.ok(meters.includes('SOUND MAP')); ok('it offers the real meters, including the SOUND MAP wasm twin');
    // One meter selection: the lab's own Meter list and its parameter list are hidden; the editor's list drives the preview.
    assert.equal(await fr.locator('#meter').isVisible(), false); assert.equal(await fr.locator('#controls').isVisible(), false); ok('the lab\'s own Meter dropdown and parameter list are not shown');
    assert.equal(await fr.locator('#theme').isVisible(), false);
    await p.locator('[data-testid=rml-options]').click(); await p.waitForTimeout(2500);
    assert.equal(await fr.locator('#theme').isVisible(), true); assert.equal(await fr.locator('#meter').isVisible(), false); ok('Preview options shows the lab\'s theme and source options, still without a meter dropdown');
    await p.locator('[data-testid=rml-options]').click(); await p.waitForTimeout(1500);
    await p.locator('[data-testid=mp-meter-chladni]').click();
    await p.waitForFunction(() => /Showing .*chladni/.test(document.querySelector('[data-testid=rml-status]')?.textContent || ''), null, { timeout: 8000 });
    ok('choosing a meter in the editor\'s list is what changes the preview');
  } else {
    assert.equal(await p.locator('[data-testid=rml-missing]').count(), 1); assert.equal(await p.locator('iframe').count(), 0); ok('without the lab page it says so and shows no frame');
  }
  assert.equal(await p.locator('[data-testid=meter-presets]').count(), 1); ok('the preset editor is on the page');
  assert.deepEqual(errors, []); ok('no page errors');
  await b.close(); console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
