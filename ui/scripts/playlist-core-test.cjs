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
  assert.match(note, /plays music from playlists only/); assert.match(note, /Add to playlist/); assert.match(note, /playlist\.m3u/); assert.match(note, /256 tracks/);
  assert.match(note, /12 KB per playlist/); assert.match(note, /playlists\.m3u/); ok('the note says it plays from playlists and names the default playlist and the limits per playlist');
  assert.doesNotMatch(await p.locator('main').innerText(), /playlist limit\)/); ok('the capacity bar no longer counts the whole card against one playlist limit');
  // albums added with the normal button go into the default playlist; the chooser lists it
  await p.getByRole('button', { name: 'Choose folder…' }).click(); await p.waitForTimeout(800);
  const pc = p.getByRole('region', { name: 'This computer' });
  await pc.getByLabel('Select Mingus Ah Um').check(); await p.getByRole('button', { name: /^Add 1 to Analogue Pocket/ }).click(); await p.waitForTimeout(300);
  await p.getByRole('button', { name: 'Start sync' }).click(); await p.waitForTimeout(700);
  let r = await p.evaluate(() => window.__tauMock.lastRequest());
  assert.equal(r.playlist.file, 'playlist.m3u'); assert.equal(r.playlist.list_file, 'playlists.m3u');
  assert.deepEqual(r.playlist_assign.map((x) => x.file), ['playlist.m3u']); assert.equal(r.playlist_assign[0].albums.length, 1); ok('an album added without a choice goes into playlist.m3u');
  await p.keyboard.press('Escape'); await p.waitForTimeout(300);
  await pc.getByLabel('Select Head Hunters').check();
  await pc.getByRole('button', { name: 'Add to playlist…' }).click(); await p.waitForTimeout(300);
  const dlg = p.getByRole('dialog', { name: /in a playlist/ });
  assert.match(await dlg.innerText(), /playlist[\s\S]*opens by default/); assert.match(await dlg.innerText(), /of 256/); ok('the chooser lists the default playlist and the 256-track limit');
  await dlg.getByRole('radio', { name: 'A new playlist…' }).check(); await dlg.getByLabel('New playlist name').fill('A long playlist name'); await p.waitForTimeout(100);
  assert.equal(await dlg.getByRole('button', { name: /^Add to/ }).isEnabled(), true); ok('a long name is fine because the core keeps a list of playlists');
  assert.deepEqual(errors, []); ok('no page errors');
  console.log(`${passed} checks passed`); await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
