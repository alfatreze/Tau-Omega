import test from 'node:test';
import assert from 'node:assert/strict';
import worker from './index.js';

const PNG = Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0]);
const b64 = (u8) => Buffer.from(u8).toString('base64');
const txt = (s) => b64(Buffer.from(s));

function setup(over = {}) {
  const kv = new Map(), r2 = new Map(), filed = [];
  const env = {
    GITHUB_REPO: 'alfatreze/Tau-Omega', GITHUB_TOKEN: 'ghp_secret', IP_SALT: 'salt', PUBLIC_URL: 'https://fb.example', HOURLY: '5', DAILY: '20', GLOBAL_DAILY: '200', PAUSED: '0',
    LIMITS: { get: async (k) => kv.get(k) ?? null, put: async (k, v) => void kv.set(k, v) },
    FILES: { put: async (k, bytes, o) => void r2.set(k, { bytes, httpMetadata: o.httpMetadata }), get: async (k) => (r2.has(k) ? { body: r2.get(k).bytes, httpMetadata: r2.get(k).httpMetadata } : null) },
    ...over,
  };
  globalThis.fetch = async (url, init) => { filed.push({ url, init, body: JSON.parse(init.body) }); return new Response(JSON.stringify({ number: 123, html_url: 'https://github.com/alfatreze/Tau-Omega/issues/123' }), { status: 201 }); };
  return { env, kv, r2, filed };
}
const msg = (o = {}) => ({ kind: 'bug', area: 'Marketplace', title: 'Marketplace crash', text: 'It closes when I open a core.', basics: { version: '0.6.0', os: 'macos', arch: 'aarch64' }, github: 'octo-cat', files: [], ...o });
const post = (env, body, ip = '1.1.1.1', extra = {}) => worker.fetch(new Request('https://fb.example/v1/feedback', { method: 'POST', headers: { 'CF-Connecting-IP': ip, ...extra }, body: typeof body === 'string' ? body : JSON.stringify(body) }), env);
const refused = async (res, status, code) => { assert.equal(res.status, status); const j = await res.json(); assert.equal(j.code, code); assert.ok(j.message.length > 5 && !/Error|undefined|\.js:\d+/.test(j.message)); return j; };

test('a good message files one issue with the labels, body and links the Worker writes itself', async () => {
  const { env, r2, filed } = setup();
  const res = await post(env, msg({ files: [{ name: 'shot.png', mime: 'image/png', data: b64(PNG) }, { name: 'run.log', mime: 'text/plain', data: txt('hello') }] }));
  assert.equal(res.status, 201); assert.deepEqual(await res.json(), { number: 123, url: 'https://github.com/alfatreze/Tau-Omega/issues/123' });
  assert.equal(filed.length, 1); const g = filed[0];
  assert.equal(g.url, 'https://api.github.com/repos/alfatreze/Tau-Omega/issues'); assert.equal(g.init.headers.Authorization, 'Bearer ghp_secret');
  assert.deepEqual(g.body.labels, ['type: bug', 'area: marketplace', 'from: omega']); assert.equal(g.body.title, 'Marketplace crash');
  assert.match(g.body.body, /^It closes when I open a core\.\n\n!\[shot\.png\]\(https:\/\/fb\.example\/f\/\d{4}-\d{2}\/[0-9a-f]{32}\.png\)\n\nAttachments: \[run\.log\]\(https:\/\/fb\.example\/f\/\d{4}-\d{2}\/[0-9a-f]{32}\.log\)\n\n---\nCategory: Something is wrong · Marketplace\nSent from Tau Omega 0\.6\.0 \(macOS, aarch64\)\nReply on GitHub: @octo-cat$/);
  assert.deepEqual([...r2.keys()].map((k) => k.split('.').pop()).sort(), ['log', 'png']); assert.ok([...r2.keys()].every((k) => !/shot|run/.test(k)));
  assert.equal([...r2.values()].find((v) => v.httpMetadata.contentType.startsWith('image/')).httpMetadata.contentType, 'image/png');
});

test('the optional parts are left out when not sent, and the sender cannot choose labels', async () => {
  const { env, filed } = setup();
  await post(env, msg({ basics: null, github: '', labels: ['admin'], files: [] }));
  assert.ok(!/Sent from|Reply on GitHub|Attachments/.test(filed[0].body.body)); assert.deepEqual(filed[0].body.labels, ['type: bug', 'area: marketplace', 'from: omega']);
});

test('every wrong or oversized message is refused in plain words and nothing is stored or filed', async () => {
  const { env, r2, filed } = setup();
  const cases = [
    [msg({ kind: 'rant' }), 400, 'invalid'], [msg({ area: 'Everything' }), 400, 'invalid'], [msg({ title: '  ' }), 400, 'invalid'], [msg({ title: 'x'.repeat(81) }), 400, 'invalid'],
    [msg({ text: 'short' }), 400, 'invalid'], [msg({ text: 'x'.repeat(4001) }), 400, 'invalid'], [msg({ github: 'no good' }), 400, 'invalid'], [msg({ basics: { version: '<b>', os: 'x', arch: 'y' } }), 400, 'invalid'],
    [msg({ files: Array.from({ length: 5 }, (_, i) => ({ name: `a${i}.png`, data: b64(PNG) })) }), 400, 'too_many_files'],
    [msg({ files: [{ name: 'tool.exe', data: txt('MZ') }] }), 400, 'invalid'], [msg({ files: [{ name: 'fake.png', data: txt('<html>not a picture</html>') }] }), 400, 'invalid'],
    [msg({ files: [{ name: 'a.zip', data: txt('not a zip') }] }), 400, 'invalid'], [msg({ files: [{ name: 'bin.txt', data: b64(Uint8Array.from([65, 0, 66])) }] }), 400, 'invalid'], [msg({ files: [{ name: 'e.png', data: '' }] }), 400, 'invalid'],
    [msg({ files: [{ name: 'big.png', data: b64(Uint8Array.from([...PNG, ...new Uint8Array(5 * 1024 * 1024)])) }] }), 413, 'too_large'],
  ];
  for (const [body, status, code] of cases) await refused(await post(env, body), status, code);
  const three = [1, 2, 3].map((i) => ({ name: `p${i}.png`, data: b64(Uint8Array.from([...PNG, ...new Uint8Array(4.5 * 1024 * 1024)])) }));
  await refused(await post(env, msg({ files: three })), 413, 'too_large');
  await refused(await post(env, 'not json'), 400, 'invalid'); await refused(await post(env, '[]'), 400, 'invalid');
  assert.equal(r2.size, 0); assert.equal(filed.length, 0);
});

test('what a stranger typed cannot notify people, show pictures from other sites or inject markup', async () => {
  const { env, filed } = setup();
  await post(env, msg({ title: 'Hello @octocat\nsecond line <b>', text: 'Ping @acme/everyone and @bob.\n![track](https://evil.example/p.gif)\n<img src="https://evil.example/x"> <script>alert(1)</script>\n[click](https://example.com/a)' }));
  const { title, body } = filed[0].body;
  assert.ok(!/@[A-Za-z]/.test(title.replace('@​', '')) && !title.includes('\n') && !/[<>]/.test(title));
  const text = body.split('\n---\n')[0];
  assert.ok(!/@(?=[A-Za-z])/.test(text)); assert.ok(text.includes('@​acme')); assert.ok(!/!\[/.test(text) && text.includes('[picture removed: track]'));
  assert.ok(!/<(img|script)/i.test(text) && text.includes('&lt;script>')); assert.ok(text.includes('click (https://example.com/a)') && !text.includes('](https'));
  assert.match(body, /Reply on GitHub: @octo-cat/); // the one @ the Worker writes itself
});

test('limits: per sender per hour, per day, for everyone, and the same message twice; only filed messages count', async () => {
  const { env } = setup({ HOURLY: '2', DAILY: '3', GLOBAL_DAILY: '5' });
  const n = (i) => msg({ title: `Message ${i}`, text: `Something different number ${i}` });
  assert.equal((await post(env, n(1))).status, 201); assert.equal((await post(env, n(2))).status, 201);
  const j = await refused(await post(env, n(3)), 429, 'rate_limited'); assert.ok(j.retry_after > 0 && j.retry_after <= 3600);
  assert.equal((await post(env, n(3), '2.2.2.2')).status, 201); // another sender
  await refused(await post(env, n(1), '2.2.2.2'), 409, 'duplicate'); // the same message again
  assert.equal((await post(env, n(4), '3.3.3.3')).status, 201); assert.equal((await post(env, n(5), '4.4.4.4')).status, 201);
  await refused(await post(env, n(6), '5.5.5.5'), 503, 'paused'); // the day's total for everyone is used up
});

test('a refused or failed message does not use up the sender’s allowance, and GitHub’s answers are never passed on', async () => {
  const { env } = setup({ HOURLY: '1' });
  await post(env, msg({ title: '' })); // refused before it counts
  globalThis.fetch = async () => new Response('{"message":"Bad credentials for token ghp_secret on alfatreze/Tau-Omega"}', { status: 401 });
  const j = await refused(await post(env, msg()), 502, 'github_refused'); assert.ok(!/ghp_|alfatreze|credentials/i.test(j.message));
  globalThis.fetch = async () => { throw new TypeError('network down'); }; await refused(await post(env, msg()), 502, 'github_unreachable');
  setup(); // restore a working fetch, same env counters
  globalThis.fetch = async () => new Response(JSON.stringify({ number: 7, html_url: 'https://github.com/alfatreze/Tau-Omega/issues/7' }), { status: 201 });
  assert.equal((await post(env, msg())).status, 201); await refused(await post(env, msg({ title: 'Another one', text: 'A different message entirely' })), 429, 'rate_limited');
});

test('the kill switch pauses everything, and status says so', async () => {
  const { env, filed } = setup({ PAUSED: '1' });
  await refused(await post(env, msg()), 503, 'paused'); assert.equal(filed.length, 0);
  assert.deepEqual(await (await worker.fetch(new Request('https://fb.example/v1/status'), env)).json(), { ok: true, paused: true });
  assert.deepEqual(await (await worker.fetch(new Request('https://fb.example/v1/status'), setup().env)).json(), { ok: true, paused: false });
});

test('attachments are served with a random name, never run as a page, and non-pictures are downloads', async () => {
  const { env, r2 } = setup();
  await post(env, msg({ files: [{ name: 'shot.png', data: b64(PNG) }, { name: 'run.log', data: txt('hello') }] }));
  const [png, log] = [...r2.keys()].sort((a) => (a.endsWith('png') ? -1 : 1));
  const a = await worker.fetch(new Request(`https://fb.example/f/${png}`), env);
  assert.equal(a.status, 200); assert.equal(a.headers.get('Content-Type'), 'image/png'); assert.equal(a.headers.get('Content-Disposition'), 'inline'); assert.equal(a.headers.get('X-Content-Type-Options'), 'nosniff'); assert.match(a.headers.get('Content-Security-Policy'), /sandbox/);
  const b = await worker.fetch(new Request(`https://fb.example/f/${log}`), env);
  assert.match(b.headers.get('Content-Disposition'), /^attachment; filename="[0-9a-f]{32}\.log"$/);
  for (const bad of ['../secret', '2026-11/short.png', '2026-11/' + 'a'.repeat(32) + '.exe', '2026-11/' + 'a'.repeat(32) + '.png']) assert.equal((await worker.fetch(new Request(`https://fb.example/f/${bad}`), env)).status, 404);
});

test('other requests: wrong method, unknown address, oversized body', async () => {
  const { env } = setup();
  assert.equal((await worker.fetch(new Request('https://fb.example/v1/feedback'), env)).status, 405);
  assert.equal((await worker.fetch(new Request('https://fb.example/'), env)).status, 404);
  await refused(await post(env, msg(), '1.1.1.1', { 'Content-Length': String(19 * 1024 * 1024) }), 413, 'too_large');
});
