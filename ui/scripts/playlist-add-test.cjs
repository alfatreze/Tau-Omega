// Add albums to specific playlists from the Library: choose an existing playlist or name a new one, for albums on this computer or already on the card;
// a player that plays only from playlists puts albums added with the normal button into its default playlist.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/playlist-add-test.cjs
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
  p.on('console', (m) => m.text().startsWith('[dev-mock] unmocked') && errors.push(m.text()));
  const pc = () => p.getByRole('region', { name: 'This computer' });
  const pk = () => p.getByRole('region', { name: 'On Pocket' });
  const picker = () => p.getByRole('dialog', { name: /Put .* in a playlist/ });
  const lastRequest = () => p.evaluate(() => window.__tauMock.lastRequest());
  const startSync = async () => { await p.getByRole('button', { name: 'Start sync' }).click(); await p.waitForTimeout(700); const dlg = p.getByRole('dialog', { name: /Review|sync/i }); return dlg; };
  await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(900);
  await p.getByRole('button', { name: 'Library', exact: true }).click(); await p.waitForTimeout(500);
  await p.getByRole('button', { name: 'Choose folder…' }).click(); await p.waitForTimeout(800);
  // ---- the library core: a new playlist from two albums on this computer
  await pc().getByLabel('Select Mingus Ah Um').check(); await pc().getByLabel('Select Head Hunters').check();
  await pc().getByRole('button', { name: 'Add to playlist…' }).click(); await p.waitForTimeout(300);
  assert.match(await picker().innerText(), /Put 2 albums in a playlist/); assert.match(await picker().innerText(), /Favourites[\s\S]*12 tracks/); ok('the chooser lists the playlists on the card with their sizes');
  await picker().getByRole('radio', { name: 'A new playlist…' }).check();
  await picker().getByLabel('New playlist name').fill('Road trip'); await p.waitForTimeout(100);
  assert.equal(await picker().getByRole('button', { name: 'Add to Road trip' }).isEnabled(), true); ok('a new name is accepted');
  await picker().getByLabel('New playlist name').fill('bad/name'); await p.waitForTimeout(100);
  assert.equal(await picker().getByRole('button', { name: /^Add to/ }).isDisabled(), true); assert.match(await picker().innerText(), /cannot contain/); ok('a name with a slash is refused, with the reason');
  await picker().getByLabel('New playlist name').fill('Favourites'); await p.waitForTimeout(100);
  assert.match(await picker().innerText(), /exists: choose it above/); ok('a name that exists is pointed to the list');
  await picker().getByLabel('New playlist name').fill('Road trip');
  await picker().getByRole('button', { name: 'Add to Road trip' }).click(); await p.waitForTimeout(300);
  await p.getByRole('button', { name: 'Details', exact: true }).click(); await p.waitForTimeout(200);
  const det = p.getByRole('dialog', { name: 'What will change' });
  assert.match(await det.innerText(), /Adding[\s\S]*Mingus Ah Um/); assert.match(await det.innerText(), /Playlists[\s\S]*Mingus Ah Um → Road trip[\s\S]*Head Hunters → Road trip/); ok('the staged list shows the albums to add and the playlist each goes into');
  await p.keyboard.press('Escape'); await p.waitForTimeout(100);
  await startSync();
  const r = await lastRequest();
  assert.equal(r.add_albums.length, 2); assert.ok(r.add_albums.some((x) => x.endsWith('Mingus Ah Um')) && r.add_albums.some((x) => x.endsWith('Head Hunters'))); ok('both albums are copied');
  assert.equal(r.playlist_assign.length, 1); assert.equal(r.playlist_assign[0].file, 'Road trip.m3u'); assert.equal(r.playlist_assign[0].albums.length, 2); ok('and the request puts both into Road trip.m3u');
  await p.keyboard.press('Escape'); await p.waitForTimeout(300);
  // ---- albums already on the card go into a playlist without being copied
  await p.evaluate(() => localStorage.clear()); await p.reload(); await p.waitForTimeout(900);
  await p.getByRole('button', { name: 'Library', exact: true }).click(); await p.waitForTimeout(600);
  await pk().getByLabel('Select Kind of Blue').check();
  await pk().getByRole('button', { name: 'Add to playlist…' }).click(); await p.waitForTimeout(300);
  await picker().getByRole('radio', { name: /Favourites/ }).check(); await picker().getByRole('button', { name: 'Add to Favourites' }).click(); await p.waitForTimeout(300);
  await startSync();
  const r2 = await lastRequest();
  assert.equal(r2.add_albums.length, 0); assert.deepEqual(r2.playlist_assign.map((a) => a.file), ['Favourites.m3u']); ok('an album already on the card is assigned to an existing playlist with nothing copied');
  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  console.log(`${passed} checks passed`); await b.close();
})().catch((e) => { console.error(e); process.exit(1); });
