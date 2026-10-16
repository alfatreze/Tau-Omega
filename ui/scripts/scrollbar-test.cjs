// Scroll bars: the side menu is the only scroller of the menu panel (the app name and card selector stay put), and the thumb is slim, only where
// content overflows, capped in length, shown while scrolling or pointing, and draggable.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/scrollbar-test.cjs
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const url = process.env.URL || 'http://localhost:5173';
let passed = 0;
const ok = (n) => { passed++; console.log(`ok - ${n}`); };
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1440, height: 560 } });
  const errors = [];
  p.on('pageerror', (e) => errors.push(e.message));
  p.on('console', (m) => m.text().startsWith('[dev-mock] unmocked') && errors.push(m.text()));
  await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(900);
  const info = () => p.evaluate(() => {
    const a = document.querySelector('aside'), n = a.querySelector('nav[aria-label="Main"]'), t = document.querySelector('.nt-tree');
    return { asideOverflows: a.scrollHeight > a.clientHeight + 1, navOverflows: n.scrollHeight > n.clientHeight + 1, treeScrolls: t ? getComputedStyle(t).overflowY !== 'visible' : false,
      brandTop: a.querySelector('.brand').getBoundingClientRect().top, ctxTop: a.querySelector('.active-context').getBoundingClientRect().top, navTop: n.scrollTop };
  });
  const before = await info();
  assert.equal(before.asideOverflows, false); assert.equal(before.treeScrolls, false); ok('the panel itself does not scroll, and the cores tree does not scroll inside it');
  assert.equal(before.navOverflows, true); ok('the menu below the fixed header is the one scrolling area');
  // scroll the menu: the header stays, a thumb appears
  await p.evaluate(() => { document.querySelector('aside nav[aria-label="Main"]').scrollTop = 40; });
  await p.waitForTimeout(150);
  const after = await info();
  assert.equal(after.brandTop, before.brandTop); assert.equal(after.ctxTop, before.ctxTop); assert.ok(after.navTop > 0); ok('the app name and card selector stay fixed while the menu scrolls');
  const th = () => p.evaluate(() => { const t = document.querySelector('.os-thumb'); const r = t.getBoundingClientRect(); return { on: t.classList.contains('on'), h: r.height, w: r.width, x: r.left, y: r.top, opacity: getComputedStyle(t).opacity }; });
  let t = await th();
  assert.equal(t.on, true); assert.ok(t.h >= 28 && t.h <= 96.1); assert.ok(t.w <= 5); ok('a slim thumb shows while scrolling, no longer than 96 px');
  const navRect = await p.evaluate(() => { const r = document.querySelector('aside nav[aria-label="Main"]').getBoundingClientRect(); return { top: r.top, bottom: r.bottom, right: r.right }; });
  assert.ok(t.y >= navRect.top - 1 && t.y + t.h <= navRect.bottom + 1 && t.x < navRect.right); ok('the thumb sits inside the area it belongs to');
  // drag it
  const startTop = after.navTop;
  await p.mouse.move(t.x + 2, t.y + t.h / 2); await p.mouse.down(); await p.mouse.move(t.x + 2, t.y + t.h / 2 + 12, { steps: 4 }); await p.mouse.up(); await p.waitForTimeout(100);
  const dragged = await info();
  assert.ok(dragged.navTop !== startTop); ok('dragging the thumb scrolls the menu');
  // it fades when idle
  await p.mouse.move(700, 300); await p.waitForTimeout(1600);
  t = await th(); assert.equal(t.on, false); ok('it goes away when nothing is scrolling or pointed at');
  // a tall page: the thumb stays capped however long the page is
  await p.getByRole('button', { name: 'Library', exact: true }).click(); await p.waitForTimeout(500);
  await p.evaluate(() => { const d = document.createElement('div'); d.style.cssText = 'height:6000px'; d.id = 'tall'; document.querySelector('main').appendChild(d); window.scrollTo(0, 300); });
  await p.waitForTimeout(200);
  t = await th(); assert.ok(t.h <= 96.1 && t.h >= 28); ok('a very tall page still gets a capped thumb');
  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  console.log(`${passed} checks passed`); await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
