// Tau Omega feedback: receives a message from the app and files it as a public GitHub issue (docs/FEEDBACK_SERVICE.md).
//   POST /v1/feedback   JSON: { kind, area, title, text, basics, github, files: [{ name, mime, data(base64) }] }  →  201 { number, url }
//   GET  /v1/status     { ok, paused }                                                                             (the app asks before it offers Send)
//   GET  /f/<key>       a stored attachment (public, random name)
import { check } from './validate.js';
import { Refused } from './validate.js';
import { allow } from './limits.js';
import { buildIssue } from './issue.js';
import { createIssue } from './github.js';
import { displayName } from './text.js';
import { LIMITS } from './rules.js';

const json = (status, body, extra = {}) => new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store', ...extra } });
const refusal = (e) => json(e.status, { code: e.code, message: e.message, ...e.extra }, e.extra.retry_after ? { 'Retry-After': String(e.extra.retry_after) } : {});
const paused = (env) => env.PAUSED === '1' || env.PAUSED === 'true';
const random = () => [...crypto.getRandomValues(new Uint8Array(16))].map((b) => b.toString(16).padStart(2, '0')).join('');

async function submit(request, env) {
  if (paused(env)) throw new Refused(503, 'paused', 'Feedback is paused for now. Please try again later.');
  if (Number(request.headers.get('Content-Length') ?? 0) > LIMITS.body) throw new Refused(413, 'too_large', 'That message is too large.');
  const text = await request.text();
  if (text.length > LIMITS.body) throw new Refused(413, 'too_large', 'That message is too large.');
  let raw; try { raw = JSON.parse(text); } catch { throw new Refused(400, 'invalid', 'That message could not be read.'); }
  const m = check(raw);
  const done = await allow(env, request, m);
  // Attachments first (the issue links to them); a failed upload stops here, before anything is filed.
  const month = new Date().toISOString().slice(0, 7), links = [];
  for (const f of m.files) {
    const key = `${month}/${random()}.${f.ext}`;
    try { await env.FILES.put(key, f.bytes, { httpMetadata: { contentType: f.type.mime } }); }
    catch { throw new Refused(502, 'storage', 'The files could not be stored right now. Please try again later.'); }
    links.push({ name: displayName(f.name), url: `${env.PUBLIC_URL}/f/${key}`, image: f.type.image });
  }
  const filed = await createIssue(env, buildIssue(m, links));
  await done();
  return json(201, filed);
}

async function file(key, env) {
  if (!/^\d{4}-\d{2}\/[0-9a-f]{32}\.[a-z]{3,4}$/.test(key)) return json(404, { code: 'not_found', message: 'Not found.' });
  const obj = await env.FILES.get(key);
  if (!obj) return json(404, { code: 'not_found', message: 'Not found.' });
  const type = obj.httpMetadata?.contentType ?? 'application/octet-stream';
  const image = type.startsWith('image/');
  return new Response(obj.body, { headers: {
    'Content-Type': type, 'X-Content-Type-Options': 'nosniff', 'Content-Security-Policy': "default-src 'none'; sandbox",
    'Content-Disposition': image ? 'inline' : `attachment; filename="${key.split('/')[1]}"`, 'Cache-Control': 'public, max-age=31536000, immutable',
  } });
}

export default {
  async fetch(request, env) {
    const { pathname } = new URL(request.url);
    try {
      if (pathname === '/v1/status' && request.method === 'GET') return json(200, { ok: true, paused: paused(env) });
      if (pathname === '/v1/feedback') return request.method === 'POST' ? await submit(request, env) : json(405, { code: 'method', message: 'Use POST.' }, { Allow: 'POST' });
      if (pathname.startsWith('/f/') && request.method === 'GET') return await file(pathname.slice(3), env);
      return json(404, { code: 'not_found', message: 'Not found.' });
    } catch (e) {
      if (e instanceof Refused) return refusal(e);
      return json(500, { code: 'error', message: 'Something went wrong. Please try again later.' });
    }
  },
};
