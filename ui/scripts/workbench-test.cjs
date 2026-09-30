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

  const stage = async (...titles) => { for (const t of titles) await pc().getByLabel(`Select ${t}`).check(); await p.getByRole('button', { name: /^Add \d+ to Pocket/ }).click(); };
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

  // --- slow direct connection: one dialog, not two -------------------------
  await pc().getByLabel('Select Mingus Ah Um').check(); await p.getByRole('button', { name: /^Add 1 to Pocket/ }).click();
  assert.match(await text(p.getByRole('button', { name: /Connected directly/ })), /slow/); ok('detects the direct Pocket connection');
  await p.getByRole('button', { name: 'Start sync' }).click();
  const merged = p.getByRole('alertdialog');
  assert.equal(await merged.count(), 1); assert.match(await text(merged), /Ready to sync/); assert.match(await text(merged), /connected to the Pocket directly/); ok('the slow-connection warning is inside the review sheet (one dialog, not two)');
  assert.match(await text(merged), /under 10 MB/); ok('warning mentions the 10 MB guidance');
  assert.match(await text(merged), /about \d+ (min|h)|under a minute/); assert.match(await text(merged), /rough estimate/); ok('the sheet gives a time estimate and says when it is only rough');
  assert.equal(await p.evaluate(() => document.activeElement?.textContent?.trim()), 'Back'); ok('the safe choice (Back) has focus when the warning shows');
  for (let i = 0; i < 6; i++) await p.keyboard.press('Tab');
  assert.equal(await p.evaluate(() => !!document.activeElement?.closest('[role=alertdialog]')), true); ok('Tab cannot leave the dialog');
  await p.getByLabel(/Don't ask again/).check(); await p.getByRole('button', { name: 'Sync anyway' }).click();
  // --- run, progress, result ---------------------------------------------
  await p.waitForTimeout(700);
  assert.match(await text(p.getByRole('region', { name: 'Sync progress' })), /Copying/); ok('progress shows the current step');
  assert.match(await text(p.getByRole('region', { name: 'Sync progress' })), /MB of 402 MB/); ok('progress shows bytes copied');
  assert.equal(await p.evaluate(() => document.activeElement?.textContent?.trim().startsWith('Cancel')), false); ok('focus is not on Cancel while syncing (Enter cannot cancel by accident)');
  assert.equal(await p.getByRole('dialog').count() + await p.getByRole('alertdialog').count(), 0); ok('progress is docked in the tray, not a blocking dialog');
  await p.waitForSelector('text=Sync complete', { timeout: 20000 });
  assert.match(await text(p.getByRole('dialog', { name: /Sync complete/ })), /9 tracks copied/); ok('result summarises what happened');
  assert.equal(await p.evaluate(() => document.activeElement?.textContent?.trim()), 'Done'); ok('focus lands on Done when the sync completes');
  await p.getByRole('button', { name: 'Done' }).click();
  assert.match(await text(pk()), /Mingus Ah Um/); assert.doesNotMatch(await text(pk()), /Will be added/); ok('card list refreshes after the sync');
  assert.match(await text(p.getByRole('region', { name: 'Pending changes' })), /Nothing yet/); ok('pending list is cleared after the sync');
  await stage('Moanin');
  assert.match(await text(p.getByRole('region', { name: 'Pending changes' })), /Direct connection: slow · about/); ok('after "don\'t ask again", only a small note with an estimate shows by the button');
  await p.getByRole('button', { name: 'Start sync' }).click();
  assert.equal(await p.getByRole('alertdialog').count(), 0); assert.equal(await p.getByRole('dialog', { name: /Ready to sync/ }).count(), 1); ok('and the warning no longer interrupts the review');
  await p.getByRole('button', { name: 'Back' }).click(); await p.getByRole('button', { name: 'Clear all' }).click();

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
  await pk().getByLabel('Select Blue Train').check();
  assert.equal(await p.getByRole('button', { name: 'Edit info' }).count() + await p.getByRole('button', { name: 'Change cover' }).count(), 0); ok('one Edit button replaces Rename / Edit info / Change cover');
  await p.getByRole('button', { name: 'Edit…' }).click();
  await p.getByLabel('Album title').fill('Blue Train (Remaster)');
  await p.getByRole('button', { name: 'Save (applies when you sync)' }).click();
  assert.match(await text(p.getByRole('region', { name: 'Pending changes' })), /Blue Train → Blue Train \(Remaster\)/); ok('an edit is staged with what it changes (old → new)');
  assert.match(await text(pk()), /Blue Train \(Remaster\)/); assert.match(await text(pk()), /was Blue Train/); ok('the new name shows in the list straight away, before the sync');
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

  // --- P0 review fixes ---------------------------------------------------------
  const tray = () => p.getByRole('region', { name: 'Pending changes' });

  // 1. unplugging while the Library screen is open
  await fresh('?connection=card_reader'); await openLibrary(); await chooseFolder();
  await stage('Mingus Ah Um');
  await p.evaluate(() => window.__tauMock.setMounted(false)); await p.waitForTimeout(3600);
  assert.match(await text(p.getByRole('alert').first()), /card is disconnected/i); ok('Library shows a banner when the card is unplugged');
  assert.equal(await p.getByRole('button', { name: 'Start sync' }).isDisabled(), true); ok('Start sync is disabled while disconnected');
  assert.match(await text(tray()), /Mingus Ah Um/); ok('pending changes are kept while disconnected');
  await p.evaluate(() => window.__tauMock.setMounted(true)); await p.waitForTimeout(3600);
  assert.equal(await p.getByRole('button', { name: 'Start sync' }).isDisabled(), false); ok('Start sync works again after reconnecting');

  // 2. the Pocket library's hard limits
  await fresh('?connection=card_reader'); await openLibrary(); await chooseFolder();
  assert.match(await text(p.getByRole('group', { name: 'Storage on the Pocket' })), /21 of 16,384 tracks/); ok('shows how full the Pocket library is');
  await p.evaluate(() => window.__tauMock.setMaxTracks(25)); await p.getByRole('button', { name: 'Refresh this card' }).click(); await p.waitForTimeout(900);
  await stage('Mingus Ah Um');
  assert.match(await text(p.getByRole('group', { name: 'Storage on the Pocket' })), /30 of 25 tracks/); assert.match(await text(tray()), /Over the Pocket's library limit/); ok('flags an over-limit selection');
  assert.equal(await p.getByRole('button', { name: 'Start sync' }).isDisabled(), true); ok('and blocks Start sync');
  await p.getByRole('button', { name: 'Clear all' }).click(); await stage('A Love Supreme');
  assert.equal(await p.getByRole('button', { name: 'Start sync' }).isDisabled(), false); ok('a selection that exactly fits is allowed');
  await p.getByRole('button', { name: 'Clear all' }).click();
  await p.evaluate(() => window.__tauMock.setMaxTracks(16384)); await p.getByRole('button', { name: 'Refresh this card' }).click(); await p.waitForTimeout(900);

  // 3 + 4. review lists what is changing; keep-connected messages
  await stage('Mingus Ah Um', 'Head Hunters');
  await p.getByRole('button', { name: 'Start sync' }).click();
  const review = p.getByRole('dialog', { name: /Ready to sync/ });
  
  assert.match(await text(review), /What's changing \(2\)/); assert.match(await text(review), /Mingus Ah Um/); assert.match(await text(review), /Head Hunters/); ok('the review sheet lists the albums that will change');
  await p.getByRole('button', { name: 'Confirm and start' }).click(); await p.waitForTimeout(600);
  assert.match(await text(p.getByRole('region', { name: 'Sync progress' })), /Keep the Pocket connected/); ok('progress tells you to keep the Pocket connected');
  await p.waitForSelector('text=Sync complete', { timeout: 20000 });
  assert.match(await text(p.getByRole('dialog', { name: /Sync complete/ })), /disconnect the Pocket now/); ok('completion says when it is safe to disconnect');
  await p.getByRole('button', { name: 'Done' }).click();

  // 5. plain-language errors
  await stage('Moanin');
  await p.evaluate(() => window.__tauMock.failNext(42, 'No such file or directory (os error 2)'));
  await p.getByRole('button', { name: 'Start sync' }).click(); await p.getByRole('button', { name: 'Confirm and start' }).click();
  await p.waitForSelector('text=Sync didn', { timeout: 10000 });
  const failed = await text(p.getByRole('alertdialog', { name: /Sync didn/ }));
  assert.match(failed, /couldn.t be reached/); assert.doesNotMatch(failed, /os error/); ok('a disconnected-card failure is explained in plain words');
  await p.getByRole('button', { name: 'Close' }).click();
  assert.match(await text(tray()), /Moanin/); ok('pending changes survive a failed sync');
  await p.evaluate(() => window.__tauMock.failNext(44, 'cancelled'));
  await p.getByRole('button', { name: 'Start sync' }).click(); await p.getByRole('button', { name: 'Confirm and start' }).click();
  await p.waitForSelector('text=Sync cancelled', { timeout: 10000 }); ok('a cancelled sync is reported as cancelled, not as a failure');
  await p.getByRole('button', { name: 'Close' }).click();
  await p.evaluate(() => window.__tauMock.setMaxTracks(10));
  await p.getByRole('button', { name: 'Start sync' }).click();
  assert.match(await text(p.getByRole('dialog', { name: /can.t be synced/ })), /library holds at most 10/); ok('a plan the engine refuses says why, before anything is copied');
  await p.getByRole('button', { name: 'Back' }).click();
  await p.evaluate(() => window.__tauMock.setMaxTracks(16384));
  await p.getByRole('button', { name: 'Clear all' }).click();

  // 6. Delete key and the sidebar pending badge
  await pk().getByLabel('Select Time Out').check(); await p.keyboard.press('Delete');
  assert.equal(await p.getByRole('alertdialog').count(), 1); ok('the Delete key starts removing the selected album');
  await p.getByRole('button', { name: 'Mark for removal' }).click();
  assert.match(await text(pk()), /Will be removed/);
  await p.getByRole('button', { name: 'Cards', exact: true }).click();
  assert.match(await text(p.locator('.active-context')), /1 pending/); ok('the sidebar card shows how many changes are pending');
  await p.locator('.active-context-btn').click();
  assert.match(await text(p.locator('.switcher-panel')), /1 pending/); ok('and so does the card switcher');
  await p.keyboard.press('Escape');

  // --- fit check (on selection and after queuing) ---------------------------------
  await fresh('?connection=card_reader'); await openLibrary(); await chooseFolder();
  await pc().getByLabel('Select Bitches Brew').check();
  assert.match(await text(pc()), /Fits, 2\.3 GB free afterwards/); ok('selecting an album says whether it fits, before adding it');
  for (const t of ['The Black Saint', 'Mingus Ah Um', 'Head Hunters', 'Moanin', 'Saxophone Colossus', 'A Love Supreme']) await pc().getByLabel(`Select ${t}`).check();
  assert.match(await text(pc()), /Won't fit: \d+ MB too big/); ok('and says by how much when it will not');
  for (const t of ['Bitches Brew', 'The Black Saint', 'Mingus Ah Um', 'Head Hunters', 'Saxophone Colossus', 'A Love Supreme']) await pc().getByLabel(`Select ${t}`).uncheck();
  await p.getByRole('button', { name: /^Add 1 to Pocket/ }).click();
  assert.match(await text(tray()), /Fits, 3\.4 GB free afterwards/); ok('after queuing, the tray says it fits and what is left');
  assert.equal(await p.evaluate(() => document.activeElement?.classList.contains('wb-tray')), true); ok('after adding, focus moves to Pending changes');
  await p.getByRole('link', { name: 'Skip to pending changes' }).focus(); await p.keyboard.press('Enter');
  assert.equal(await p.evaluate(() => document.activeElement?.classList.contains('wb-tray')), true); ok('a skip link jumps straight to Pending changes');
  const order = await p.evaluate(() => { const undo = [...document.querySelectorAll('.wb-chips button')][0]; const start = [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Start sync'); return !!(undo.compareDocumentPosition(start) & Node.DOCUMENT_POSITION_FOLLOWING); });
  assert.equal(order, true); ok('the pending items and their undo buttons come before Start sync in tab order');
  await p.getByRole('button', { name: 'Clear all' }).click();
  const opacity = await p.evaluate(() => Number(getComputedStyle([...document.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Start sync')).opacity));
  assert.ok(opacity < 0.7); ok('a disabled Start sync looks disabled');

  // --- sync history ------------------------------------------------------------------
  await fresh('?connection=card_reader&history=seed'); await openLibrary(); await chooseFolder();
  await stage('Moanin'); await p.getByRole('button', { name: 'Start sync' }).click(); await p.getByRole('button', { name: 'Confirm and start' }).click();
  await p.waitForSelector('text=Sync complete', { timeout: 20000 });
  await p.getByRole('button', { name: 'View details' }).click(); await p.waitForTimeout(500);
  assert.match(await text(p.locator('main')), /Sync history/); assert.match(await text(p.locator('article')), /Added 1 album/); assert.match(await text(p.locator('article')), /Moanin/); ok('"View details" after a sync opens that sync in the history, described in words');
  assert.match(await text(p.locator('main')), /4 syncs stored/); ok('the history lists earlier syncs too');
  await p.getByRole('button', { name: /^Problems \(1\)/ }).click();
  await p.locator('.hist-row').first().click();
  const detail = await text(p.locator('article'));
  assert.match(detail, /couldn.t be reached/); assert.match(detail, /Your existing music on the Pocket is safe/); ok('a failed sync explains what happened and reassures about the card');
  assert.doesNotMatch(detail, /os error/); ok('and keeps the raw error behind "Technical details"');
  await p.getByText('Technical details').click(); assert.match(await text(p.locator('article')), /os error 2/); ok('the technical details are available on demand');
  // a failure proposes the history more visibly
  await fresh('?connection=card_reader'); await openLibrary(); await chooseFolder(); await stage('Moanin');
  await p.evaluate(() => window.__tauMock.failNext(42, 'No such file or directory (os error 2)', 'copy'));
  await p.getByRole('button', { name: 'Start sync' }).click(); await p.getByRole('button', { name: 'Confirm and start' }).click();
  await p.waitForSelector('text=Sync didn', { timeout: 10000 });
  assert.match(await text(p.getByRole('alertdialog')), /Your existing music on the Pocket is safe/); ok('the failure dialog reassures based on how far the run got');
  assert.equal(await p.evaluate(() => document.activeElement?.textContent?.trim()), 'See what happened'); ok('and puts "See what happened" first');
  await p.getByRole('button', { name: 'Close' }).click();
  assert.match(await text(p.locator('.wb-banner-fail')), /Your last sync didn.t finish/); ok('the Library screen keeps offering the details after a failure');
  await p.getByRole('button', { name: 'Dismiss' }).click(); assert.equal(await p.locator('.wb-banner-fail').count(), 0); ok('the failure banner can be dismissed');
  await p.evaluate(() => window.__tauMock.failNext(42, 'No such file or directory (os error 2)', 'remove'));
  await p.getByRole('button', { name: 'Start sync' }).click(); await p.getByRole('button', { name: 'Confirm and start' }).click();
  await p.waitForSelector('text=Sync didn', { timeout: 10000 });
  assert.match(await text(p.getByRole('alertdialog')), /Some removals may not have finished/); ok('a failure late in the run says what was and was not applied');
  await p.getByRole('button', { name: 'See what happened' }).click(); await p.waitForTimeout(400);
  assert.match(await text(p.locator('article')), /What happened/); ok('"See what happened" opens the failed sync in the history');

  // --- history settings (retention) ---------------------------------------------------
  await fresh('?history=seed');
  await p.getByRole('button', { name: /Tools & settings/ }).click(); await p.getByRole('button', { name: 'Settings', exact: true }).click();
  assert.match(await text(p.locator('main')), /3 syncs stored/); ok('Settings shows how many syncs are stored');
  assert.equal(await p.getByLabel('Number of syncs to keep').inputValue(), '100'); assert.equal(await p.getByLabel('How long to keep history').inputValue(), '365'); ok('retention has reasonable defaults (100 syncs, 1 year)');
  await p.evaluate(() => window.__tauMock.addOldEntries(2, 60)); await p.getByRole('button', { name: 'Browse sync history' }).click(); await p.waitForTimeout(300);
  assert.match(await text(p.locator('main')), /5 syncs stored/);
  await p.getByRole('button', { name: 'History settings' }).click();
  await p.getByLabel('How long to keep history').selectOption('30'); await p.waitForTimeout(400);
  assert.match(await text(p.locator('main')), /Removed 2 older entries/); assert.match(await text(p.locator('main')), /3 syncs stored/); ok('shortening the retention time deletes older entries straight away');
  await p.getByLabel('Number of syncs to keep').selectOption('25'); await p.waitForTimeout(300);
  assert.equal(await p.getByLabel('Number of syncs to keep').inputValue(), '25'); ok('the number of syncs to keep can be changed');
  await p.getByRole('button', { name: 'Clear history…' }).click(); assert.match(await text(p.locator('.ls-confirm')), /Delete all 3/); ok('clearing the history asks first');
  await p.getByRole('button', { name: 'Delete', exact: true }).click(); await p.waitForTimeout(300);
  assert.match(await text(p.locator('main')), /Deleted 3 entries/); assert.match(await text(p.locator('main')), /0 syncs stored/); ok('and then clears it');
  await p.getByRole('button', { name: 'Browse sync history' }).click(); await p.waitForTimeout(300);
  assert.match(await text(p.locator('main')), /No syncs yet/); ok('an empty history says so and points back to Library');

  // --- narrow windows: a menu instead of a vanishing sidebar ---------------------------
  await fresh(); await p.setViewportSize({ width: 720, height: 450 }); await p.waitForTimeout(400);
  assert.equal(await p.locator('#sidebar').isVisible(), false); assert.equal(await p.getByRole('button', { name: 'Menu' }).isVisible(), true); ok('at 200% zoom the sidebar is replaced by a Menu button');
  await p.getByRole('button', { name: 'Menu' }).click(); await p.waitForTimeout(300);
  assert.equal(await p.locator('#sidebar').isVisible(), true); assert.equal(await p.evaluate(() => !!document.activeElement?.closest('#sidebar')), true); ok('the menu opens with focus inside it');
  for (let i = 0; i < 20; i++) await p.keyboard.press('Tab');
  assert.equal(await p.evaluate(() => !!document.activeElement?.closest('#sidebar')), true); ok('Tab stays inside the open menu');
  await p.keyboard.press('Escape'); await p.waitForTimeout(200);
  assert.equal(await p.locator('#sidebar').isVisible(), false); assert.equal(await p.evaluate(() => document.activeElement?.getAttribute('aria-label')), 'Menu'); ok('Escape closes it and returns focus to the Menu button');
  await p.getByRole('button', { name: 'Menu' }).click(); await p.locator('#sidebar').getByRole('button', { name: 'Library', exact: true }).click(); await p.waitForTimeout(400);
  assert.equal(await p.locator('#sidebar').isVisible(), false); assert.match(await text(p.locator('main')), /Choose your music folder|On Pocket/); ok('choosing a page closes the menu and shows it');
  await p.setViewportSize({ width: 1440, height: 900 });

  // --- Help panel is a proper dialog -----------------------------------------------------
  await p.getByRole('button', { name: 'Help' }).click();
  assert.equal(await p.getByRole('dialog', { name: 'Using Tau Omega' }).count(), 1); ok('Help opens as a dialog');
  for (let i = 0; i < 12; i++) await p.keyboard.press('Tab');
  assert.equal(await p.evaluate(() => !!document.activeElement?.closest('.help-panel')), true); ok('and keeps focus inside it');
  await p.keyboard.press('Escape'); assert.equal(await p.getByRole('dialog', { name: 'Using Tau Omega' }).count(), 0); ok('Escape closes it');

  // --- selection: whole-row click, ranges, select all, filters, sort ------------------------
  await fresh('?connection=card_reader'); await openLibrary(); await chooseFolder();
  const rowOf = (title) => pc().locator('.wb-row', { hasText: title }).first();
  await rowOf('Moanin').locator('strong').click();
  assert.equal(await pc().getByLabel('Select Moanin').isChecked(), true); assert.match(await text(pc()), /1 selected/); ok('clicking anywhere on a row selects it');
  await rowOf('A Love Supreme').locator('strong').click({ modifiers: ['Shift'] });
  assert.match(await text(pc()), /5 selected/); assert.equal(await pc().locator('.wb-row.sel.dim').count(), 0); ok('Shift-click selects a range (5 albums), skipping the ones already on the Pocket');
  await p.keyboard.press('Escape'); assert.equal(await pc().locator('.wb-row.sel').count(), 0); ok('Escape clears the selection');
  await pc().getByLabel('Select A Love Supreme').focus(); await p.keyboard.press('Control+a');
  assert.match(await text(pc()), /8 selected/); ok('Ctrl/Cmd+A selects everything that can be added (7 new albums and the changed one)');
  await p.keyboard.press('Escape');
  await pc().getByLabel(/Select all \d+ shown/).check();
  assert.match(await text(pc()), /8 selected/); ok('"Select all shown" selects the same set');
  await pc().getByLabel(/Select all \d+ shown/).uncheck(); assert.equal(await pc().locator('.wb-row.sel').count(), 0);
  assert.match(await text(pc()), /All \(11\)/); assert.match(await text(pc()), /New \(7\)/); assert.match(await text(pc()), /Changed \(1\)/); assert.match(await text(pc()), /On Pocket \(3\)/); ok('filter chips show how many albums are in each state');
  await pc().getByRole('button', { name: /^Changed \(1\)/ }).click();
  assert.equal(await pc().locator('.wb-row').count(), 1); assert.match(await text(pc()), /Pocket has 4 tracks/); ok('the Changed filter shows one album and says how it differs from the Pocket');
  await pc().getByRole('button', { name: /^New \(7\)/ }).click(); await pc().getByLabel(/Select all \d+ shown/).check();
  await p.getByRole('button', { name: /^Add 7 to Pocket/ }).click(); assert.match(await text(tray()), /7 albums to add/); ok('filter New, select all, add: seven albums queued in three clicks');
  await p.getByRole('button', { name: 'Clear all' }).click();
  await pc().getByRole('button', { name: /^All \(11\)/ }).click();
  await pc().getByLabel('Sort albums').selectOption('largest');
  assert.match(await pc().locator('.wb-row').first().innerText(), /Bitches Brew/); ok('sort: largest first');
  await pc().getByLabel('Sort albums').selectOption('title');
  assert.match(await pc().locator('.wb-row').first().innerText(), /A Love Supreme/); ok('sort: title A to Z');

  // --- long lists stay fast (windowing) -------------------------------------------------------
  await fresh('?connection=card_reader&big=2000'); await openLibrary(); await chooseFolder(); await p.waitForTimeout(600);
  assert.match(await text(pc()), /Select all 20\d\d shown|Select all 2011 shown|Select all 200\d shown/);
  const domRows = await pc().locator('.wb-row').count();
  assert.ok(domRows < 60, `rendered ${domRows} rows`); ok(`2,011 albums, only ${domRows} rows in the page`);
  await pc().locator('.vl').evaluate((el) => { el.scrollTop = el.scrollHeight; }); await p.waitForTimeout(500);
  assert.match(await text(pc()), /Album 19\d\d|Album 20\d\d/); ok('scrolling to the end shows the last albums');
  await pc().getByLabel(/Select all \d+ shown/).check(); assert.match(await text(pc()), /2008 selected/); ok('select all works across the whole long list');

  // --- cover thumbnails ------------------------------------------------------------------------------
  await fresh('?connection=card_reader'); await openLibrary(); await chooseFolder(); await p.waitForTimeout(700);
  assert.ok(await pc().locator('.wb-art img').count() >= 5); ok('albums on this computer show their cover pictures');
  assert.ok(await pk().locator('.wb-art img').count() >= 2); ok('and so do albums on the Pocket');
  assert.equal(await pk().locator('.wb-row', { hasText: 'Time Out' }).locator('.wb-art img').count(), 0); assert.match(await pk().locator('.wb-row', { hasText: 'Time Out' }).locator('.wb-art').innerText(), /♪/); ok('an album with no picture keeps the placeholder');
  await pk().getByLabel('Select Blue Train').check(); await p.getByRole('button', { name: 'Edit…' }).click();
  assert.equal(await p.locator('.wb-drawer .wb-cov img').count(), 1); assert.match(await text(p.locator('.wb-drawer')), /Now/); ok('the edit drawer shows the current cover');
  await p.getByRole('button', { name: /Choose image/ }).click(); await p.waitForTimeout(300);
  assert.match(await text(p.locator('.wb-drawer')), /New/); assert.equal(await p.locator('.wb-drawer .wb-cov img, .wb-drawer .wb-newcov').count() >= 2, true); ok('choosing a picture shows Now next to New');
  await p.getByRole('button', { name: 'Cancel' }).click();

  // --- progress is docked, not blocking -----------------------------------------------------------------
  await stage('Moanin');
  await p.getByRole('button', { name: 'Start sync' }).click(); await p.getByRole('button', { name: 'Confirm and start' }).click(); await p.waitForTimeout(500);
  const dock = p.getByRole('region', { name: 'Sync progress' });
  assert.match(await text(dock), /Syncing to Pocket/); assert.match(await text(dock), /keep looking around/); ok('progress sits in the tray with Cancel and a note that you can keep browsing');
  assert.equal(await p.getByRole('button', { name: /^Add \d* ?to Pocket/ }).isDisabled(), true); assert.equal(await p.getByRole('button', { name: 'Clear all' }).isDisabled(), true); ok('changing the queue is disabled while a sync runs');
  await pk().getByRole('tab', { name: 'Tracks' }).click(); assert.match(await text(pk()), /Track 1/); ok('but you can still browse the Pocket while it runs');
  await p.waitForSelector('text=Sync complete', { timeout: 20000 }); await p.getByRole('button', { name: 'Done' }).click();
  assert.equal(await p.getByRole('region', { name: 'Sync progress' }).count(), 0); ok('the dock goes away when the sync finishes');

  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  await b.close();
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
