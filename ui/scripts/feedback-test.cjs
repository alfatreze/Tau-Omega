// Behaviour checks for the Send feedback page against the dev mock.
// Usage: (npm run dev &) && NODE_PATH=$(npm root -g) node scripts/feedback-test.cjs   (URL=... to point elsewhere)
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const url = process.env.URL || 'http://localhost:5173';
let passed = 0; const ok = (n) => { passed++; console.log(`ok - ${n}`); };
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==', 'base64');
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1440, height: 1500 } });
  const errors = [];
  p.on('pageerror', (e) => errors.push(e.message));
  p.on('console', (m) => m.text().startsWith('[dev-mock] unmocked') && errors.push(m.text()));
  const fresh = async () => { await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.goto(url); await p.waitForTimeout(900); };
  const openPage = async () => { const more = p.locator('nav button', { hasText: 'Tools & settings' }); if ((await more.getAttribute('aria-expanded')) !== 'true') await more.click(); await p.locator('nav button', { hasText: 'Send feedback' }).click(); await p.waitForTimeout(400); };
  const send = () => p.locator('form').getByRole('button', { name: /Send feedback|Sending/ });
  const title = () => p.getByLabel('Title'); const text = () => p.getByLabel('Description');
  const attach = (files) => p.locator('input[type=file]').setInputFiles(files);
  const fill = async () => { await title().fill('Marketplace crash'); await text().fill('It closes when I open a core.'); };

  await fresh(); await openPage();
  assert.equal(await p.getByRole('heading', { name: 'Send feedback' }).count(), 1); ok('Send feedback is in Tools & settings');
  assert.equal(await send().isDisabled(), true); ok('an empty message cannot be sent');
  await fill(); assert.equal(await send().isDisabled(), false); assert.equal(await p.locator('form input[type=checkbox]').count(), 1); assert.match(await p.locator('.fb-callout').last().innerText(), /This will be public/); ok('a titled and described message can be sent at once; the public notice is plain text beside Send, with no box to tick');
  await text().fill('short'); assert.equal(await send().isDisabled(), true); assert.match(await p.locator('.fb-todo').innerText(), /a little more/); await text().fill('It closes when I open a core.'); ok('a too-short description is held back with a plain reason');

  // categories
  assert.equal(await p.getByRole('radio', { name: /Something is wrong/ }).isChecked(), true); assert.match(await text().getAttribute('placeholder'), /What happened/);
  await p.getByRole('radio', { name: /An idea/ }).check(); assert.match(await text().getAttribute('placeholder'), /What would you like Omega to do/); await p.getByLabel('Where in Omega?').selectOption('Marketplace'); ok('what it is and where change the prompt');
  assert.equal(await p.locator('details').count(), 0); assert.ok(!/See exactly what will be sent/.test(await p.locator('form').innerText())); ok('there is no preview step');
  await p.getByLabel(/Your GitHub name/).fill('not ok!'); assert.equal(await send().isDisabled(), true); assert.match(await p.locator('.fb-todo').innerText(), /GitHub name/); await p.getByLabel(/Your GitHub name/).fill('octo-cat'); assert.equal(await send().isDisabled(), false); ok('a GitHub name is checked and held back with a reason when it is not one');

  // attachments
  await attach([{ name: 'shot.png', mimeType: 'image/png', buffer: PNG }, { name: 'run.log', mimeType: 'text/plain', buffer: Buffer.from('hello') }]);
  const list = p.getByRole('list', { name: 'Attached files' });
  assert.equal(await list.locator('li').count(), 2); assert.equal(await list.locator('img').count(), 1); assert.match(await list.innerText(), /shot\.png[\s\S]*run\.log/); assert.match(await p.locator('.fb-total').innerText(), /2 files/); ok('pictures and files are added; a picture is shown small, a file by its type');
  await attach([{ name: 'tool.exe', mimeType: 'application/x-msdownload', buffer: Buffer.from('MZ') }]);
  assert.match(await p.getByRole('alert').first().innerText(), /tool\.exe is not a type we can take/); assert.equal(await list.locator('li').count(), 2); ok('a type that is not allowed is refused with the allowed ones listed');
  await attach([{ name: 'huge.png', mimeType: 'image/png', buffer: Buffer.alloc(6 * 1024 * 1024) }]);
  assert.match(await p.getByRole('alert').first().innerText(), /huge\.png is 6\.0 MB\. Each file can be up to 5\.0 MB/); assert.equal(await list.locator('li').count(), 2); ok('a file over 5 MB is refused');
  await p.evaluate(() => { const ta = document.querySelector('#fb-text'); const dt = new DataTransfer(); dt.items.add(new File([new Uint8Array([137, 80, 78, 71])], 'pasted.png', { type: 'image/png' })); ta.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true })); });
  await p.waitForTimeout(200); assert.equal(await list.locator('li').count(), 3); assert.match(await list.innerText(), /pasted\.png/); ok('a picture pasted into the description is attached');
  await attach([{ name: 'a.png', mimeType: 'image/png', buffer: PNG }, { name: 'b.png', mimeType: 'image/png', buffer: Buffer.concat([PNG, Buffer.from('b')]) }]);
  assert.equal(await list.locator('li').count(), 4); assert.match(await p.getByRole('alert').first().innerText(), /up to 4 files/); assert.equal(await p.getByRole('button', { name: 'Add pictures or files…' }).isDisabled(), true); ok('at most four files; the Add button switches off at four');
  await list.getByRole('button', { name: 'Remove a.png' }).click(); assert.equal(await list.locator('li').count(), 3); assert.equal(await p.getByRole('button', { name: 'Add pictures or files…' }).isDisabled(), false); ok('a file can be removed');

  // a draft survives other pages
  await p.locator('nav button', { hasText: /^Card$/ }).click(); await p.waitForTimeout(300); await openPage();
  assert.equal(await title().inputValue(), 'Marketplace crash'); assert.equal(await list.locator('li').count(), 3); assert.equal(await p.getByRole('radio', { name: /An idea/ }).isChecked(), true); ok('what is written and attached is still there after visiting another page');

  // sending
  await p.evaluate(() => { globalThis.__sent = []; globalThis.__feedback = 'fail'; });
  await send().click(); await p.waitForTimeout(400);
  assert.match(await p.getByRole('alert').last().innerText(), /feedback service could not be reached/); assert.equal(await title().inputValue(), 'Marketplace crash'); assert.equal(await list.locator('li').count(), 3); ok('a failed send says why and keeps everything for another try');
  await p.evaluate(() => { globalThis.__feedback = 'slow'; });
  await send().click(); await p.waitForTimeout(250);
  assert.match(await send().innerText(), /Sending/); assert.equal(await send().isDisabled(), true); ok('while sending, the button says so and cannot be pressed again');
  await p.getByText('Thank you').waitFor(); const sent = await p.evaluate(() => globalThis.__sent.at(-1));
  assert.deepEqual({ kind: sent.kind, area: sent.area, title: sent.title, text: sent.text, github: sent.github, basics: sent.basics }, { kind: 'idea', area: 'Marketplace', title: 'Marketplace crash', text: 'It closes when I open a core.', github: 'octo-cat', basics: { version: '0.6.0', os: 'macos', arch: 'aarch64' } });
  assert.deepEqual(sent.files.map((f) => f.name), ['shot.png', 'run.log', 'pasted.png']); assert.equal(sent.files[1].data, Buffer.from('hello').toString('base64')); assert.equal(sent.files[1].mime, 'text/plain'); ok('what is sent is the fields (not finished text or labels) and the files as they are');
  assert.match(await p.locator('.fb-done').innerText(), /issue #123/); await p.getByRole('link', { name: 'Open it on GitHub' }).click(); await p.waitForTimeout(150);
  assert.deepEqual(await p.evaluate(() => globalThis.__opened.at(-1)), 'issue 123'); ok('it thanks the sender, names the issue and opens it on GitHub');
  await p.getByRole('button', { name: 'Send more feedback' }).click();
  assert.equal(await title().inputValue(), ''); assert.equal(await text().inputValue(), ''); assert.equal(await p.getByRole('list', { name: 'Attached files' }).count(), 0); ok('a new message starts empty');

  // not connected (the real build today)
  await p.evaluate(() => { globalThis.__feedback = 'off'; }); await p.locator('nav button', { hasText: /^Card$/ }).click(); await p.waitForTimeout(300); await openPage();
  assert.match(await p.locator('.fb-callout').first().innerText(), /Sending is not connected yet/); await fill();
  assert.equal(await send().isDisabled(), true); assert.match(await p.locator('.fb-send').innerText(), /not connected yet/); ok('when sending is not connected, the page says so and Send stays off while writing still works');

  assert.deepEqual(errors, []); ok('no page errors and no unmocked commands');
  console.log(`\n${passed} checks passed`);
  await b.close();
})().catch((e) => { console.error('FAILED:', e.message); process.exit(1); });
