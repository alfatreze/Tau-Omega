// Library, This computer pane: a short footer (no long instruction line), two properly laid out buttons that never wrap, and albums that can be dragged
// across to the Pocket with the pointer.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/library-footer-test.cjs
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const url = process.env.URL || 'http://localhost:5173';
let passed = 0;
const ok = (n) => { passed++; console.log(`ok - ${n}`); };
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  p.on('pageerror', (e) => errors.push(e.message));
  p.on('console', (m) => m.text().startsWith('[dev-mock] unmocked') && errors.push(m.text()));
  const pc = () => p.getByRole('region', { name: 'This computer' });
  await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(900);
  await p.getByRole('button', { name: 'Library', exact: true }).click(); await p.waitForTimeout(500);
  await p.getByRole('button', { name: 'Choose folder…' }).click(); await p.waitForTimeout(800);
  const foot = pc().locator('.wb-pane-foot');
  const text = await foot.innerText();
  assert.match(text, /\d+ albums?/); assert.doesNotMatch(text, /click albums to select|drag them across/); ok('the footer is just the album count: no long instruction line');
  const geo = await foot.evaluate((f) => [...f.querySelectorAll('button')].map((b) => { const r = b.getBoundingClientRect(); const lines = Math.round(r.height / parseFloat(getComputedStyle(b).lineHeight || '18')); return { h: Math.round(r.height), w: Math.round(r.width), t: b.innerText, lines }; }));
  assert.equal(geo.length, 2); assert.ok(geo.every((g) => g.h <= 44 && g.lines <= 2), JSON.stringify(geo)); assert.ok(geo.every((g) => !g.t.includes('\n')), JSON.stringify(geo)); ok('both buttons stay on one line at a normal width');
  assert.ok(Math.abs(geo[0].h - geo[1].h) <= 1); ok('and have the same height');
  // select, then drag across
  const row = pc().getByLabel('Select Mingus Ah Um').locator('xpath=ancestor::*[contains(@class,"wb-row")][1]');
  const box = await row.boundingBox();
  const pk = await p.getByRole('region', { name: 'On Pocket' }).boundingBox();
  await p.mouse.move(box.x + 140, box.y + box.height / 2); await p.mouse.down();
  await p.mouse.move(box.x + 160, box.y + box.height / 2 + 6, { steps: 3 });
  await p.mouse.move(pk.x + pk.width / 2, pk.y + 140, { steps: 8 });
  assert.equal(await p.locator('.wb-ghost').count(), 1); assert.match(await p.locator('.wb-ghost').innerText(), /1 album/); assert.match(await p.locator('.wb-dropveil').innerText(), /Drop to add/); ok('dragging shows a chip and the Pocket says where to drop');
  await p.mouse.up(); await p.waitForTimeout(300);
  assert.equal(await p.locator('.wb-ghost').count(), 0);
  await p.getByRole('button', { name: 'Details', exact: true }).click(); await p.waitForTimeout(200);
  assert.match(await p.getByRole('dialog', { name: 'What will change' }).innerText(), /Adding[\s\S]*Mingus Ah Um/); ok('dropping it on the Pocket stages the album');
  await p.keyboard.press('Escape');
  // a click still selects, and a drag that ends elsewhere adds nothing
  const row2 = pc().getByLabel('Select Head Hunters').locator('xpath=ancestor::*[contains(@class,"wb-row")][1]');
  const b2 = await row2.boundingBox();
  await p.mouse.move(b2.x + 140, b2.y + b2.height / 2); await p.mouse.down(); await p.mouse.move(b2.x + 150, b2.y + 40, { steps: 5 }); await p.mouse.up(); await p.waitForTimeout(200);
  assert.equal(await pc().getByLabel('Select Head Hunters').isChecked(), false); ok('a drag that ends off the Pocket changes nothing, and does not tick the album');
  await row2.click({ position: { x: 140, y: 10 } }); await p.waitForTimeout(200);
  assert.equal(await pc().getByLabel('Select Head Hunters').isChecked(), true); ok('a plain click still selects an album');
  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  console.log(`${passed} checks passed`); await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
