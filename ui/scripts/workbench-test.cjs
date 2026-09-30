// Behaviour checks for the Library workbench against the dev mock.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/workbench-test.cjs
// Exits non-zero on the first failed expectation.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const url = process.env.URL || 'http://localhost:5173';
let passed = 0;
const ok = (name) => { passed++; console.log(`ok - ${name}`); };
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  p.on('pageerror', (e) => errors.push(e.message));
  p.on('console', (m) => m.text().startsWith('[dev-mock] unmocked') && errors.push(m.text()));
  const fresh = async (query = '') => { await p.goto(url + query); await p.evaluate(() => localStorage.clear()); await p.goto(url + query); await p.waitForTimeout(800); };
  const pk = () => p.getByRole('region', { name: 'On Pocket' });
  const pc = () => p.getByRole('region', { name: 'This computer' });
  const text = (loc) => loc.innerText();
  const openLibrary = async () => { await p.getByRole('button', { name: 'Library', exact: true }).click(); await p.waitForTimeout(400); };
  const chooseFolder = async () => { await p.getByRole('button', { name: 'Choose folder…' }).click(); await p.waitForTimeout(400); };

  // --- navigation ---------------------------------------------------------
  await fresh();
  assert.equal(await p.getByRole('button', { name: 'Sync library' }).count(), 0); ok('old Sync library screen is gone from the menu');
  assert.equal(await p.getByRole('button', { name: 'Backup', exact: true }).count(), 0); ok('tools are collapsed by default');
  await p.getByRole('button', { name: /Tools & settings/ }).click();
  assert.equal(await p.getByRole('button', { name: 'Backup', exact: true }).count(), 1); ok('tools & settings expands');
  await p.reload(); await p.waitForTimeout(600);
  assert.equal(await p.getByRole('button', { name: 'Backup', exact: true }).count(), 1); ok('expanded state is remembered');

  // --- first run ----------------------------------------------------------
  await fresh(); await openLibrary();
  assert.match(await text(pk()), /Kind of Blue/); ok('shows what is on the selected card');
  assert.match(await text(p.getByRole('group', { name: 'Storage on the Pocket' })), /free of 8\.0 GB/); ok('capacity bar shows real totals');
  assert.match(await text(pc()), /Choose your music folder/); ok('asks for a music folder first');
  await chooseFolder();
  assert.match(await text(pc()), /Mingus Ah Um/); ok('lists the local library');
  assert.match(await text(pc()), /On Pocket/); assert.match(await text(pc()), /Changed/); ok('badges albums as On Pocket / Changed / New');

  // --- staging ------------------------------------------------------------
  await pc().getByLabel('Select Mingus Ah Um').check(); await pc().getByLabel('Select Head Hunters').check();
  await p.getByRole('button', { name: /^Add 2 to Pocket/ }).click();
  assert.match(await text(p.getByRole('group', { name: 'Storage on the Pocket' })), /\+ 782 MB queued/); ok('capacity shows queued bytes');
  assert.match(await text(p.getByRole('region', { name: 'Pending changes' })), /2 albums to add \(13 tracks, 782 MB\)/); ok('pending list counts albums, tracks and size');
  await p.getByRole('button', { name: 'Remove Head Hunters from pending changes' }).click();
  assert.match(await text(p.getByRole('region', { name: 'Pending changes' })), /1 album to add/); ok('a pending item can be removed');
  await p.getByRole('button', { name: 'Clear all' }).click();
  assert.match(await text(p.getByRole('region', { name: 'Pending changes' })), /Nothing yet/); ok('Clear all empties the list');
  await p.reload(); await p.waitForTimeout(700); await openLibrary();
  await pc().getByLabel('Select Moanin').check(); await p.getByRole('button', { name: /^Add 1 to Pocket/ }).click();
  await p.reload(); await p.waitForTimeout(700); await openLibrary();
  assert.match(await text(p.getByRole('region', { name: 'Pending changes' })), /Moanin/); ok('pending changes survive a reload (per card and core)');
  await p.getByRole('button', { name: 'Clear all' }).click();

  // --- over capacity ------------------------------------------------------
  for (const t of ['Bitches Brew', 'The Black Saint', 'Moanin', 'Saxophone Colossus', 'A Love Supreme', 'Mingus Ah Um', 'Head Hunters']) await pc().getByLabel(`Select ${t}`).check();
  await p.getByRole('button', { name: /^Add 7 to Pocket/ }).click();
  assert.equal(await p.getByRole('button', { name: 'Start sync' }).isDisabled(), true); ok('Start sync is disabled when it will not fit');
  assert.match(await text(p.getByRole('region', { name: 'Pending changes' })), /Won't fit/); ok('and says why');
  await p.getByRole('button', { name: 'Clear all' }).click();

  // --- slow direct connection --------------------------------------------
  await pc().getByLabel('Select Mingus Ah Um').check(); await p.getByRole('button', { name: /^Add 1 to Pocket/ }).click();
  assert.match(await text(p.getByRole('button', { name: /Direct USB/ })), /slow/); ok('detects the direct Pocket connection');
  await p.getByRole('button', { name: 'Start sync' }).click();
  assert.equal(await p.getByRole('alertdialog').count(), 1); assert.match(await text(p.getByRole('alertdialog')), /connected to the Pocket directly/); ok('first sync over 10 MB shows the slow alert');
  assert.match(await text(p.getByRole('alertdialog')), /under 10 MB/); ok('alert mentions the 10 MB guidance');
  assert.equal(await p.evaluate(() => !!document.activeElement?.closest('[role=alertdialog]')), true); ok('focus moves into the dialog when it opens');
  for (let i = 0; i < 5; i++) await p.keyboard.press('Tab');
  assert.equal(await p.evaluate(() => !!document.activeElement?.closest('[role=alertdialog]')), true); ok('Tab cannot leave the dialog');
  await p.getByLabel(/Don't ask again/).check(); await p.getByRole('button', { name: 'Sync anyway' }).click();
  assert.equal(await p.getByRole('dialog', { name: /Ready to sync/ }).count(), 1); ok('continuing goes to the review sheet');
  await p.getByRole('button', { name: 'Back' }).click();
  assert.match(await text(p.getByRole('region', { name: 'Pending changes' })), /Direct connection: slow/); ok('after "don\'t ask again", only a small note shows by the button');
  await p.getByRole('button', { name: 'Start sync' }).click();
  assert.equal(await p.getByRole('alertdialog').count(), 0); ok('and the alert no longer interrupts');
  await p.getByRole('button', { name: 'Back' }).click();
  // --- run, progress, result ---------------------------------------------
  await p.getByRole('button', { name: 'Start sync' }).click();
  await p.getByRole('button', { name: 'Confirm and start' }).click();
  await p.waitForTimeout(700);
  assert.match(await text(p.getByRole('dialog', { name: /Syncing/ })), /Copying/); ok('progress shows the current step');
  assert.match(await text(p.getByRole('dialog', { name: /Syncing/ })), /MB of 402 MB/); ok('progress shows bytes copied');
  await p.waitForSelector('text=Sync complete', { timeout: 20000 });
  assert.match(await text(p.getByRole('dialog', { name: /Sync complete/ })), /9 files copied/); ok('result summarises what happened');
  await p.getByRole('button', { name: 'Done' }).click();
  assert.match(await text(pk()), /Mingus Ah Um/); assert.doesNotMatch(await text(pk()), /Will be added/); ok('card list refreshes after the sync');
  assert.match(await text(p.getByRole('region', { name: 'Pending changes' })), /Nothing yet/); ok('pending list is cleared after the sync');

  // --- removal ------------------------------------------------------------
  await pk().getByLabel('Select Time Out').check(); await p.getByRole('button', { name: 'Remove', exact: true }).click();
  assert.match(await text(p.getByRole('alertdialog')), /Settings → Library/); ok('first removal explains where to change the preference');
  await p.getByRole('button', { name: 'Mark for removal' }).click();
  assert.match(await text(pk()), /Will be removed/); ok('removal is only marked, not applied');
  await p.getByRole('button', { name: 'Undo removing Time Out' }).click();
  assert.doesNotMatch(await text(pk()), /Will be removed/); ok('a marked removal can be undone');
  await pk().getByLabel('Select Time Out').check(); await p.getByRole('button', { name: 'Remove', exact: true }).click();
  assert.equal(await p.getByRole('alertdialog').count(), 0); ok('the explanation is shown only once');
  await p.getByRole('button', { name: 'Start sync' }).click();
  assert.match(await text(p.getByRole('dialog', { name: /Ready to sync/ })), /copied to your backup folder first/); ok('review states that removals are backed up first');
  await p.getByRole('button', { name: 'Confirm and start' }).click();
  await p.waitForSelector('text=Sync complete', { timeout: 20000 }); await p.getByRole('button', { name: 'Done' }).click();
  assert.doesNotMatch(await text(pk()), /Time Out/); ok('removed album is gone after the sync');

  // --- editing ------------------------------------------------------------
  await pk().getByLabel('Select Blue Train').check(); await p.getByRole('button', { name: 'Edit info' }).click();
  await p.getByLabel('Album title').fill('Blue Train (Remaster)');
  await p.getByRole('button', { name: 'Add to Pending changes' }).click();
  assert.match(await text(p.getByRole('region', { name: 'Pending changes' })), /Blue Train \(title\)/); ok('an edit is staged with what it changes');
  await p.getByRole('button', { name: 'Start sync' }).click(); await p.getByRole('button', { name: 'Confirm and start' }).click();
  await p.waitForSelector('text=Sync complete', { timeout: 20000 }); await p.getByRole('button', { name: 'Done' }).click();
  assert.match(await text(pk()), /Blue Train \(Remaster\)/); ok('the edited title shows on the card after the sync');

  // --- refresh and auto-detect ---------------------------------------------
  await p.evaluate(() => window.__tauMock.addCardAlbum('Added Behind Your Back'));
  assert.doesNotMatch(await text(pk()), /Added Behind Your Back/);
  await p.getByRole('button', { name: 'Refresh this card' }).click(); await p.waitForTimeout(900);
  assert.match(await text(pk()), /Added Behind Your Back/); ok('the refresh button re-reads the card');
  await p.getByRole('button', { name: 'Cards', exact: true }).click();
  await p.evaluate(() => window.__tauMock.setMounted(false)); await p.waitForTimeout(3600);
  assert.match(await text(p.locator('main')), /no longer connected/); ok('unplugging is detected automatically');
  await p.evaluate(() => { window.__tauMock.setMounted(true); window.__tauMock.addCardAlbum('Appeared On Reconnect'); });
  await p.waitForTimeout(3600); await openLibrary();
  assert.match(await text(pk()), /Appeared On Reconnect/); ok('reconnecting is detected and the card is re-read');

  // --- connection kinds -----------------------------------------------------
  await fresh('?connection=card_reader'); await openLibrary(); await chooseFolder();
  await pc().getByLabel('Select Mingus Ah Um').check(); await p.getByRole('button', { name: /^Add 1 to Pocket/ }).click();
  assert.match(await text(p.getByRole('button', { name: /Card reader/ })), /fast/);
  await p.getByRole('button', { name: 'Start sync' }).click();
  assert.equal(await p.getByRole('alertdialog').count(), 0); ok('no slow alert over a card reader');
  await p.getByRole('button', { name: 'Back' }).click();
  await fresh('?connection=unknown'); await openLibrary(); await chooseFolder();
  assert.match(await text(p.getByRole('button', { name: /Connection unknown/ })), /unknown/); ok('an unrecognised connection is reported as unknown, not guessed');

  // --- settings --------------------------------------------------------------
  await p.getByRole('button', { name: /Tools & settings/ }).click(); await p.getByRole('button', { name: 'Settings', exact: true }).click();
  await p.getByLabel(/Just remove/).check(); await p.waitForTimeout(300);
  assert.match(await text(p.locator('main')), /Saved\./); ok('removal preference saves from Settings → Library');

  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
