// Meter presets (tau-presets/<meter id>/ files): list, edit, factory marks, reorder, hide, delete with a never-reused id, parameter
// form, plan review and save, against the dev mock (which serves the real registry fixture and keeps a stateful "card").
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/meter-presets-test.cjs
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const url = process.env.URL || 'http://localhost:5173';
let passed = 0; const ok = (n) => { passed++; console.log(`ok - ${n}`); };
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1400, height: 1100 } });
  const errors = []; p.on('pageerror', (e) => errors.push(e.message)); p.on('console', (m) => m.text().includes('unmocked') && errors.push(m.text()));
  await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(1000);
  await p.locator('nav button', { hasText: 'Meter Lab' }).first().click(); await p.waitForTimeout(1200);
  const t = (id) => p.locator(`[data-testid=${id}]`);
  const go = async (name) => { await t(`mp-tab-${name}`).click(); };   // the controls are in tabs: Settings, Presets, Common
  const items = () => p.locator('[data-testid=mp-list] li:not(.mp-default):not(.mp-empty)');
  const names = async () => (await items().locator('.mp-name').allInnerTexts());

  assert.equal(await t('mp-no-registry').count(), 0); assert.equal(await t('mp-no-card').count(), 0);
  assert.equal(await p.locator('.mp-meters button').count(), 7); ok('the meters of the registry are listed (the ones with parameters)');
  assert.deepEqual(await names(), ['FLUID', 'CLASSIC', 'BOUNCY', 'SLOW FADE', 'SNAPPY']); ok('Winamp Bars lists its five presets from the card, after the built-in DEFAULT');
  assert.equal(await p.locator('[data-testid=mp-list] .mp-default').count(), 1); ok('DEFAULT is shown as built in, not as a file');
  assert.notEqual(await p.locator('[data-testid=mp-list] .mp-default').evaluate((e) => getComputedStyle(e).position), 'sticky'); ok('DEFAULT scrolls with the list and never sits on top of the other presets');
  assert.equal(await items().locator('.mp-tag', { hasText: /^factory$/ }).count(), 5); ok('every preset equal to the factory one is marked factory');
  assert.match(await t('mp-save').innerText(), /Nothing to save/); assert.ok(await t('mp-save').isDisabled()); ok('nothing is saved until something changes');

  // rename (upper-cased), the factory mark becomes "edited"
  await t('mp-name').fill('quiet room'); assert.equal(await t('mp-name').inputValue(), 'QUIET ROOM'); ok('names are upper-cased as typed');
  assert.match(await items().first().innerText(), /QUIET ROOM[\s\S]*no\. 0[\s\S]*factory, changed/); ok('an edited factory preset says so and keeps its id');
  assert.match(await p.locator('.mp-meters button.on').innerText(), /changed/); ok('the meter shows that it has unsaved changes');
  assert.ok(await t('mp-save').isEnabled()); ok('saving becomes possible');
  await t('mp-name').fill('CAFÉ'); assert.equal(await t('mp-name').getAttribute('aria-invalid'), 'true'); assert.match(await p.locator('.mp-edit [role=alert]').innerText(), /no accents/); ok('a name the files cannot hold is explained as you type');
  await t('mp-name').fill('QUIET ROOM');

  // hide, reorder, duplicate (first free id), delete with a confirm and an id that is retired
  await go('presets');
  await t('mp-hide').click(); assert.equal(await items().first().locator('.mp-tag.hide').count(), 1); ok('Hide marks the preset hidden');
  assert.match(await t('mp-hidden-note').innerText(), /stays in Omega only[\s\S]*same number/); ok('the page says a hidden preset stays in Omega and is not on the Pocket');
  await p.getByRole('button', { name: 'Move down' }).click(); assert.deepEqual((await names()).slice(0, 2), ['CLASSIC', 'QUIET ROOM']); ok('Down moves the preset in the list order');
  await p.getByRole('button', { name: 'Duplicate' }).click(); assert.deepEqual((await names()).length, 6); assert.match(await items().last().innerText(), /QUIET ROOM COPY[\s\S]*no\. 5[\s\S]*yours/); ok('a duplicate is a new preset of yours with the first free id');
  await t('mp-delete').click(); assert.equal(await items().count(), 6); await t('mp-delete-yes').click(); assert.equal(await items().count(), 5); ok('Delete asks first, then removes');
  await p.getByRole('button', { name: 'New preset' }).click(); assert.match(await items().last().innerText(), /MY PRESET[\s\S]*no\. 5/); ok('a new preset takes the first free id');

  // parameters: change one, it is remembered as set; Default clears it
  await go('settings');
  const first = p.locator('.mp-param').first(); const unset = first.locator('.mp-unset');
  assert.ok(await unset.isDisabled()); await first.locator('input[type=range]').fill('12'); await p.waitForTimeout(100);
  assert.ok(await unset.isEnabled()); assert.equal(await first.locator('output').innerText(), '12'); ok('changing a value marks it as set on this preset');
  await unset.click(); assert.ok(await unset.isDisabled()); ok('Default puts the parameter back to the meter’s own default');

  // review and save
  await t('mp-save').click(); await p.waitForTimeout(300);
  const review = t('mp-review'); assert.equal(await review.count(), 1);
  assert.match(await review.innerText(), /WINAMP BARS[\s\S]*new[\s\S]*changed[\s\S]*removed[\s\S]*Not on the Pocket[\s\S]*1 preset stays in Omega only[\s\S]*Backup[\s\S]*saved to/); ok('the review lists new, changed and removed files, the hidden preset kept in Omega only, and the backup');
  await review.getByRole('button', { name: 'Cancel' }).click(); assert.equal(await t('mp-review').count(), 0); assert.ok(await t('mp-save').isEnabled()); ok('cancelling writes nothing and keeps the edits');
  await t('mp-save').click(); await p.waitForTimeout(300); await t('mp-confirm').click(); await p.waitForTimeout(500);
  assert.match(await t('mp-notice').innerText(), /Saved to .*files? written.*read back and checked/); ok('saving reports what was written');
  assert.match(await t('mp-save').innerText(), /Nothing to save/); ok('after saving nothing is pending');
  assert.deepEqual(await names(), ['CLASSIC', 'BOUNCY', 'SLOW FADE', 'SNAPPY', 'MY PRESET', 'QUIET ROOM']); ok('the card holds the visible presets; the hidden one is kept by Omega and listed after them');
  assert.equal(await items().last().locator('.mp-tag.hide').count(), 1); ok('after saving, the hidden preset is still marked hidden (it is not on the Pocket)');

  // an id that was on the card, deleted and saved is retired: the next new preset gets a different id
  await go('presets');
  assert.equal(await items().count(), 6); await items().filter({ hasText: 'MY PRESET' }).locator('.mp-pick').click();
  await t('mp-delete').click(); await t('mp-delete-yes').click(); assert.equal(await items().count(), 5);
  await t('mp-save').click(); await p.waitForTimeout(300); assert.match(await t('mp-review').innerText(), /1 removed/); ok('the review says what is removed');
  await t('mp-confirm').click(); await p.waitForTimeout(500);
  await p.getByRole('button', { name: 'New preset' }).click();
  assert.match(await items().last().innerText(), /no\. 6/); ok('an id that was deleted and saved is never given to another preset');

  // showing a hidden preset again puts it back (same id) in the pending list
  await items().filter({ hasText: 'QUIET ROOM' }).locator('.mp-pick').click(); await t('mp-hide').click();
  assert.equal(await items().filter({ hasText: 'QUIET ROOM' }).locator('.mp-tag.hide').count(), 0); assert.match(await items().filter({ hasText: 'QUIET ROOM' }).innerText(), /no\. 0/); ok('Show brings a hidden preset back with its own id');
  await t('mp-save').click(); await p.waitForTimeout(300); assert.match(await t('mp-review').innerText(), /new/); assert.doesNotMatch(await t('mp-review').innerText(), /Not on the Pocket/); await t('mp-review').getByRole('button', { name: 'Cancel' }).click(); ok('showing it again would write its file and nothing is hidden any more');

  // undo and factory actions
  await p.getByRole('button', { name: 'Undo changes' }).click(); assert.equal(await items().count(), 5); ok('Undo edits returns to what the card holds');
  await p.locator('.mp-more summary').click(); await t('mp-replace').click(); await t('mp-replace-yes').click(); assert.deepEqual(await names(), ['FLUID', 'CLASSIC', 'BOUNCY', 'SLOW FADE', 'SNAPPY']); ok('Replacing the list with the factory set (after a confirmation) restores the factory list (not written until saved)');
  await p.getByRole('button', { name: 'Undo changes' }).click();

  // a meter with a conditional parameter set (Layered Wave): `when` hides and shows parameters
  await t('mp-meter-layered_wave').click(); await go('presets'); assert.equal(await items().count(), 16); ok('Layered Wave lists its 16 factory presets');
  await items().first().locator('.mp-pick').click(); await go('settings');
  const labels = () => p.locator('.mp-param label').allInnerTexts();
  const before = await labels();
  await p.locator('.mp-param select').first().selectOption({ index: 1 }); await p.waitForTimeout(150);
  const after = await labels();
  assert.notDeepEqual(after, before); assert.ok(after.some((l) => !before.includes(l)) && before.some((l) => !after.includes(l))); ok('a parameter that depends on another appears or disappears with it');
  assert.ok(await p.locator('fieldset legend', { hasText: /experimental/i }).count() >= 1); ok('experimental parameters are grouped apart');
  // Factory actions: "Restore" only adds what is missing (and says how many); replacing the whole list is behind More and a confirmation.
  await t('mp-meter-winamp_bars').click(); await go('presets'); await p.waitForTimeout(250);
  assert.equal(await t('mp-restore-factory').isDisabled(), true); ok('Restore factory presets is off while every factory preset is in the list');
  assert.equal(await p.getByRole('button', { name: 'Reset to factory' }).count(), 0); ok('the destructive reset is not a toolbar button any more');
  // Many presets: the list is bounded and filterable, and the editor stays in reach.
  for (let i = 0; i < 40; i++) await p.getByRole('button', { name: 'New preset' }).click();
  const box = await t('mp-list').boundingBox(); assert.ok(box.height <= 300, `list height ${box.height}`);
  assert.equal(await items().count(), 45); assert.ok((await t('mp-selected').boundingBox()).y < 1100); ok('45 presets: the list scrolls inside a bounded area and the actions for the selected preset are still on screen');
  await t('mp-find').fill('my preset 7'); assert.equal(await items().count(), 1); assert.match(await t('mp-list').innerText(), /MY PRESET 7/); ok('a filter finds a preset by name among many');
  await t('mp-find').fill('');
  // What goes on the card is a choice per preset (a preset not on the Pocket is kept in Omega only).
  if (await p.getByRole('button', { name: 'Put all on the Pocket' }).isEnabled()) await p.getByRole('button', { name: 'Put all on the Pocket' }).click();
  const firstOn = items().first().locator('input.mp-on'); assert.equal(await firstOn.isChecked(), true);
  await firstOn.uncheck(); assert.equal(await items().first().locator('.mp-tag.hide').count(), 1); assert.match(await t('mp-onbar').innerText(), /44 of 45 will be on the Pocket · 1 stays? in Omega only/); ok('unticking a preset keeps it off the Pocket and the count says so');
  await p.getByRole('button', { name: 'Take all off the Pocket' }).click(); assert.match(await t('mp-onbar').innerText(), /0 of 45 will be on the Pocket/);
  await p.getByRole('button', { name: 'Put all on the Pocket' }).click(); assert.match(await t('mp-onbar').innerText(), /45 of 45 will be on the Pocket/); ok('bulk actions put every preset on the Pocket or keep every one off');
  await t('mp-find').fill('my preset 4'); await p.getByRole('button', { name: 'Take shown off the Pocket' }).click();
  await t('mp-find').fill(''); assert.match(await t('mp-onbar').innerText(), /(\d+) of 45 will be on the Pocket · (\d+) stay in Omega only/); ok('with a filter, a bulk change only touches the presets shown');
  await p.getByRole('button', { name: 'Undo changes' }).click();
  // Tabs: the Settings tab steps through presets (DEFAULT first) so the list tab is not needed just to change preset.
  await t('mp-meter-winamp_bars').click(); await go('presets'); const order = await names(); await items().first().locator('.mp-pick').click(); await go('settings');
  assert.ok((await t('mp-which').innerText()).includes(order[0]));
  await p.getByRole('button', { name: 'Next preset' }).click(); assert.ok((await t('mp-which').innerText()).includes(order[1]));
  await p.getByRole('button', { name: 'Previous preset' }).click(); await p.getByRole('button', { name: 'Previous preset' }).click();
  assert.match(await t('mp-which').innerText(), /DEFAULT/); assert.equal(await t('mp-default-card').count(), 1); assert.equal(await p.getByRole('button', { name: 'Previous preset' }).isDisabled(), true);
  ok('the Settings tab steps through the presets, DEFAULT first, and says DEFAULT cannot be changed');
  assert.deepEqual(await p.locator('[role=tab][data-testid^=mp-tab-]').allInnerTexts().then((x) => x.map((y) => y.replace(/\s*\d+$/, '').trim())), ['Settings', 'Presets', 'Common']); ok('the controls are three tabs: Settings, Presets, Common');
  // The meter list is a design preview only: clearly marked, nothing in it can be changed, nothing reads or writes the card.
  await t('mp-mode-list').click(); const ml = t('meter-list-preview'); assert.equal(await ml.isVisible(), true);
  assert.match(await ml.innerText(), /Preview of a coming feature[\s\S]*Nothing on this page reads your card or changes it/); ok('the Meters on the Pocket page says it is a preview and touches nothing');
  assert.equal(await ml.locator('input:not(:disabled), button:not(:disabled)').count(), 0); assert.equal(await ml.locator('li').count(), 7); ok('every control on it is switched off, and it lists the meters Omega knows');
  assert.match(await ml.innerText(), /\d of 8 slots used/); await t('mp-mode-edit').click(); assert.equal(await p.locator('.mp-layout').isVisible(), true); ok('switching back returns to the meter workspace');
  await p.screenshot({ path: process.env.SHOT || '/tmp/meter-presets.png' });
  assert.deepEqual(errors, []); ok('no page errors');
  await b.close(); console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('\nFAILED:', e.message); process.exit(1); });
