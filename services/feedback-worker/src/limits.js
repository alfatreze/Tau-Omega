import { Refused } from './validate.js';

const hex = (buf) => [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, '0')).join('');
export const sha = async (s) => hex(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(s)));
const stamp = (d, n) => d.toISOString().replace(/[-:T]/g, '').slice(0, n); // 10 = hour, 8 = day

const num = (v, d) => { const n = Number(v); return Number.isFinite(n) && n > 0 ? n : d; };

/**
 * Counts per sender (a salted hash of the address, never the address), per hour and per day, plus one count for everyone per day, in KV
 * (counts are approximate: KV is eventually consistent, which is fine for a spam brake). Throws Refused (429 or 503). Counts only when
 * the request is allowed. The same title and text twice within a day is refused as a duplicate.
 */
export async function allow(env, request, m, now = new Date()) {
  const kv = env.LIMITS;
  const who = await sha(`${env.IP_SALT ?? ''}|${request.headers.get('CF-Connecting-IP') ?? 'unknown'}`);
  const hour = `h:${who}:${stamp(now, 10)}`, day = `d:${who}:${stamp(now, 8)}`, all = `all:${stamp(now, 8)}`, dup = `dup:${await sha(m.title + '\n' + m.text)}`;
  const get = async (k) => Number((await kv.get(k)) ?? 0);
  const [h, d, a, seen] = await Promise.all([get(hour), get(day), get(all), kv.get(dup)]);
  if (a >= num(env.GLOBAL_DAILY, 200)) throw new Refused(503, 'paused', 'Feedback is paused for today. Please try again tomorrow.');
  if (h >= num(env.HOURLY, 5)) throw new Refused(429, 'rate_limited', 'You have sent a few messages already. Please try again in an hour.', { retry_after: 3600 - (now.getUTCMinutes() * 60 + now.getUTCSeconds()) });
  if (d >= num(env.DAILY, 20)) throw new Refused(429, 'rate_limited', 'You have reached today’s limit. Please try again tomorrow.', { retry_after: 86400 - (now.getUTCHours() * 3600 + now.getUTCMinutes() * 60 + now.getUTCSeconds()) });
  if (seen) throw new Refused(409, 'duplicate', 'That exact message was sent already.');
  return async () => { // called once the issue exists
    await Promise.all([kv.put(hour, String(h + 1), { expirationTtl: 7200 }), kv.put(day, String(d + 1), { expirationTtl: 172800 }), kv.put(all, String(a + 1), { expirationTtl: 172800 }), kv.put(dup, '1', { expirationTtl: 86400 })]);
  };
}
