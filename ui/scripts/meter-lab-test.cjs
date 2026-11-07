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
    const fr = p.frameLocator('iframe[aria-label^="Meter preview"]'); await p.waitForTimeout(2500);
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
    assert.equal(await fr.locator('#theme').isVisible(), false); ok('the lab\'s own theme, source and size options are not shown either');
    // The meter view never scrolls: the frame is exactly as tall as the meter (the lab reports its size), and Layered Wave's extra strip fits.
    const frameBox = async () => (await p.locator('iframe[aria-label^="Meter preview"]').boundingBox());
    const h1 = (await frameBox()).height; assert.ok(h1 >= 120 && h1 <= 130, `Winamp Bars frame ${h1}`); assert.equal((await frameBox()).width, 368); assert.equal(await p.locator('iframe[aria-label^="Meter preview"]').getAttribute('scrolling'), 'no');
    await p.locator('[data-testid=mp-meter-layered_wave]').click(); await p.waitForTimeout(1200);
    const h2 = (await frameBox()).height; assert.ok(h2 > h1 + 40, `Layered Wave frame ${h2} vs ${h1}`);
    assert.equal(await fr.locator('html').evaluate((e) => e.scrollHeight <= e.clientHeight + 1), true); ok('the frame grows to fit Layered Wave\'s strip, and the preview never needs to scroll');
    // The processing budget bar: the worst frame against what the meter's cost class allows, over or under, right under the preview.
    await p.locator('[data-testid=mp-meter-winamp_bars]').click(); await p.waitForTimeout(2500);
    const bud = p.locator('[data-testid=rml-budget]'); await bud.waitFor({ timeout: 8000 });
    assert.match(await p.locator('[data-testid=rml-budget-text]').innerText(), /^(Under|Over) by [\d.]+ \(\d+ %\)$/); assert.match(await bud.getAttribute('aria-label'), /Processing budget over the last second: worst frame \d+ drawing commands, 100 allowed/); ok('the budget bar says how far under or over the allowance the meter is (Winamp Bars: 100 allowed)');
    assert.equal(await bud.getAttribute('data-kind'), 'under'); const fillW = await bud.locator('.bb-fill').evaluate((e) => e.getBoundingClientRect().width), trackW = await bud.locator('.bb-track').evaluate((e) => e.getBoundingClientRect().width);
    assert.ok(fillW > 0 && fillW < trackW * 0.5); assert.equal(await bud.locator('.bb-over').evaluate((e) => e.getBoundingClientRect().width), 0); ok('under budget the bar fills part of the allowance and there is no red stretch');
    // The budget is pushed every other frame (about 19 times a second), not once in a while.
    await p.evaluate(() => { window.__budgetMsgs = 0; window.addEventListener('message', (e) => { if (e.data && e.data.type === 'omega-lab-budget') window.__budgetMsgs++; }); });
    await p.waitForTimeout(2000); const msgs = await p.evaluate(() => window.__budgetMsgs); assert.ok(msgs >= 20 && msgs <= 60, `budget messages in 2 s: ${msgs}`); ok(`the budget is sent every other frame (${msgs} updates in 2 s, about ${Math.round(msgs / 2)} a second)`);
    // It reacts within about a second to a change of setting, the allowance tick never moves, and the last meter's numbers are never shown against the next meter's allowance.
    const worstNow = async () => Number((await bud.getAttribute('aria-label')).match(/worst frame (\d+)/)?.[1] ?? NaN);
    await p.locator('[data-testid=mp-tab-settings]').click(); const bands = p.locator('#mp-bands'); const maxBands = await bands.getAttribute('max'), minBands = await bands.getAttribute('min');
    await bands.fill(maxBands); await p.waitForTimeout(2200); const wHigh = await worstNow(); const ticks = new Set();
    await bands.fill(minBands); const t0 = Date.now(); let wLow = wHigh;
    while (Date.now() - t0 < 2000 && !(wLow < wHigh)) { await p.waitForTimeout(100); wLow = await worstNow(); ticks.add(await bud.locator('.bb-allowed').getAttribute('style')); }
    assert.ok(wLow < wHigh, `worst did not fall after fewer bands: ${wHigh} -> ${wLow}`); assert.ok(Date.now() - t0 < 1900, 'the bar took longer than 1.9 s to follow a change'); assert.ok(ticks.size <= 1, `the allowance tick moved: ${[...ticks]}`);
    ok('the budget bar follows a change of setting within about a second and the allowance tick stays put');
    await bands.fill('16' >= minBands ? '16' : maxBands);
    await p.locator('[data-testid=mp-meter-layered_wave]').click(); const seen = [];
    for (let i = 0; i < 12; i++) { seen.push(await bud.getAttribute('aria-label')); await p.waitForTimeout(100); }
    assert.ok(!seen.some((l) => /100 allowed/.test(l)), 'the bar showed Winamp Bars\' numbers against Layered Wave\'s allowance'); assert.ok(seen.some((l) => /400 allowed/.test(l)), 'the bar never settled on Layered Wave\'s allowance');
    ok('after changing meter the bar never shows the old meter\'s numbers, then settles on the new allowance');
    await p.locator('[data-testid=mp-meter-winamp_bars]').click(); await p.waitForTimeout(1500);
    assert.ok((await p.locator('[data-testid=rml-budget]').boundingBox()).y > (await p.locator('iframe[aria-label^="Meter preview"]').boundingBox()).y); ok('the bar sits under the preview, outside the Details panel, so it is always visible');
    // The small view shows exactly the meter's band (368 x 122, never scaled); the toggle above the preview switches to the full-screen figure (400 x 323).
    await p.locator('[data-testid=mp-meter-winamp_bars]').click(); await p.waitForTimeout(800);
    assert.equal(await p.locator('[data-testid=rml-view-small]').getAttribute('aria-pressed'), 'true'); ok('the preview starts in the small view, with a Small view / Full screen toggle right above it');
    await p.locator('[data-testid=rml-view-full]').click(); await p.waitForTimeout(1500);
    const fb = await frameBox(); assert.ok(fb.height >= 320 && fb.height <= 330 && fb.width === 400, `full screen ${fb.width}x${fb.height}`);
    assert.equal(await fr.locator('#fullfig').isChecked(), true); ok('Full screen shows the 400 x 323 figure and sets the lab\'s own full-screen figure');
    await p.locator('[data-testid=rml-view-small]').click(); await p.waitForTimeout(1500);
    const fs2 = await frameBox(); assert.ok(fs2.height <= 130 && fs2.width === 368, `small ${fs2.width}x${fs2.height}`); assert.equal(await fr.locator('#fullfig').isChecked(), false); ok('Small view goes back to the meter\'s own band');
    await p.locator('[data-testid=mp-meter-layered_wave]').click(); await p.waitForTimeout(1000);
    // The preview options are native controls in the Common tab and drive the lab.
    await p.locator('[data-testid=mp-tab-common]').click();
    await p.waitForFunction(() => document.querySelectorAll('[data-testid=mp-common] select').length >= 3, null, { timeout: 8000 });
    assert.equal(await p.locator('[data-testid=mp-opt-theme]').count(), 1); assert.equal(await p.locator('[data-testid=mp-opt-barready]').count(), 0); ok('the Common tab shows the preview options that apply to the shown meter, in plain words');
    await p.locator('[data-testid=mp-opt-mode]').selectOption('1'); await p.waitForTimeout(500);
    assert.equal(await fr.locator('#mode').inputValue(), '1'); ok('choosing Light in the Common tab changes the lab\'s own control');
    await p.locator('[data-testid=mp-opt-mode]').selectOption('0');
    // Details are in their own expandable panel.
    const det = p.locator('[data-testid=rml-details]'); await det.locator('summary').click();
    await p.waitForFunction(() => document.querySelectorAll('[data-testid=rml-details] tr').length > 0 || (document.querySelector('[data-testid=rml-details] .rml-cost')?.textContent || '').length > 30, null, { timeout: 8000 });
    ok('the details (commands, processor time, cost) open in their own panel');
    // The panel must stay inside its column: it used to stretch the column and cover the preset list.
    const left = await p.locator('.mp-left').boundingBox(), right = await p.locator('.mp-right').boundingBox(), dbox = await det.boundingBox();
    assert.ok(dbox.x + dbox.width <= left.x + left.width + 1, `details ${dbox.x + dbox.width} wider than its column ${left.x + left.width}`); assert.ok(left.x + left.width <= right.x, 'the left column overlaps the controls');
    assert.ok(left.width <= 420, `left column ${left.width}`); ok('opening the details does not widen the column or cover the controls');
    await det.locator('summary').click();
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
