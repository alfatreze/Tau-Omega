// The warning for a per-meter settings file of the wrong size (the Pocket stops at start with "Error in framework file id [11] size bad").
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/meter-settings-test.cjs   (URL=... to point elsewhere)
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const url = process.env.URL || 'http://localhost:5173';
let passed = 0; const ok = (n) => { passed++; console.log(`ok - ${n}`); };
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const errors = [];
  const open = async (mode) => {
    const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
    p.on('pageerror', (e) => errors.push(e.message));
    p.on('console', (m) => m.text().startsWith('[dev-mock] unmocked') && errors.push(m.text()));
    if (mode) await p.addInitScript((m) => { globalThis.__mset = m; }, mode);
    await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(900);
    return p;
  };
  const bar = (p) => p.getByRole('region', { name: 'Meter settings file problem' });

  let p = await open(null);
  assert.equal(await bar(p).count(), 0); ok('a card whose settings files are fine shows no warning');
  await p.close();

  p = await open('wrong');
  assert.equal(await bar(p).count(), 1);
  const text = await bar(p).innerText();
  assert.match(text, /alfatreze\.TAU will not start on the Pocket/); assert.match(text, /8,191 bytes[\s\S]*exactly 8,192[\s\S]*size bad/); ok('a wrong-sized file is explained in plain words: which core, its size, what the Pocket needs');
  assert.ok(!/DEV 230/.test(text)); ok('a settings file of the right size is not mentioned');
  assert.match(text, /Your meters then start from their defaults/); ok('it says what the fix costs');

  await bar(p).getByRole('button', { name: /Review/ }).click(); await p.waitForTimeout(300);
  const plan = await bar(p).innerText();
  assert.match(plan, /Delete Saves\/tau\/alfatreze\.TAU\/tau-meter-settings\.bin \(8,191 bytes\)\?/); assert.match(plan, /copies it to a backup folder outside your card and checks the copy[\s\S]*Nothing else on the card is touched/); ok('the plan names the one file and says a checked copy is made outside the card first');
  assert.equal(await p.evaluate(() => (globalThis.__msetDeleted ?? []).length), 0); ok('reviewing deletes nothing');
  await bar(p).getByRole('button', { name: 'Cancel' }).click(); await p.waitForTimeout(200);
  assert.equal(await bar(p).getByRole('button', { name: /Review/ }).count(), 1); assert.equal(await p.evaluate(() => (globalThis.__msetDeleted ?? []).length), 0); ok('Cancel goes back and still deletes nothing');
  await bar(p).getByRole('button', { name: /Review/ }).click(); await p.waitForTimeout(200);
  await bar(p).getByRole('button', { name: 'Delete the file' }).click(); await p.waitForTimeout(500);
  assert.deepEqual(await p.evaluate(() => globalThis.__msetDeleted), ['Saves/tau/alfatreze.TAU/tau-meter-settings.bin']); ok('confirming deletes exactly that file');
  assert.match(await bar(p).innerText(), /Deleted\. A copy is kept in \/backups\/installs\/0123456789abcdef/); ok('it says where the copy is kept');
  await bar(p).getByRole('button', { name: 'Close' }).click(); await p.waitForTimeout(200);
  assert.equal(await bar(p).count(), 0); ok('after closing, the warning is gone (the card is rescanned and is fine)');
  await p.close();

  p = await open('fail');
  await bar(p).getByRole('button', { name: /Review/ }).click(); await p.waitForTimeout(200);
  await bar(p).getByRole('button', { name: 'Delete the file' }).click(); await p.waitForTimeout(400);
  assert.match(await bar(p).getByRole('alert').innerText(), /Nothing was deleted: The backup of the file did not verify/); assert.equal(await bar(p).getByRole('button', { name: 'Delete the file' }).count(), 1); ok('a failed delete says nothing was deleted and the person can try again or cancel');
  await p.close();

  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  console.log(`\n${passed} checks passed`);
  await b.close();
})().catch((e) => { console.error('FAILED:', e.message); process.exit(1); });
