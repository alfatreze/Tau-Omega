// Playlist-only core (HarpMudd Mp3Player): the Library says how the core plays, shows its 256-track limit, and refuses to go over it.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/playlist-core-test.cjs
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const url = process.env.URL || 'http://localhost:5173';
let passed = 0;
const ok = (n) => { passed++; console.log(`ok - ${n}`); };
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  p.on('pageerror', (e) => errors.push(e.message));
  await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(800);
  await p.getByRole('button', { name: 'Library', exact: true }).click(); await p.waitForTimeout(400);
  assert.equal(await p.locator('.wb-info').count(), 0); ok('a library core shows no playlist note');
  await p.getByLabel('Target core').selectOption({ label: 'Mp3Player (playlist)' }); await p.waitForTimeout(500);
  const note = await p.locator('.wb-info').innerText();
  assert.match(note, /plays music from a playlist only/); assert.match(note, /playlist\.m3u/); assert.match(note, /256 tracks/);
  assert.match(note, /12 KB/); assert.match(note, /12 characters/); assert.match(note, /playlists\.m3u/); ok('the note names the file and the 256-track, 12 KB and 12-character limits');
  assert.match(await p.locator('main').innerText(), /of 256 tracks \(playlist limit\)/); ok('the capacity bar counts against the playlist limit');
  assert.deepEqual(errors, []); ok('no page errors');
  console.log(`${passed} checks passed`); await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
