import { AREAS, FILE_TYPES, KINDS, LIMITS, extOf, githubOk } from './rules.js';

export class Refused extends Error { constructor(status, code, message, extra = {}) { super(message); this.status = status; this.code = code; this.extra = extra; } }
const bad = (message) => new Refused(400, 'invalid', message);

const decode = (b64) => { try { const s = atob(b64); const out = new Uint8Array(s.length); for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i); return out; } catch { return null; } };
const str = (v) => (typeof v === 'string' ? v : '');

/** The message as the app sends it (fields and base64 files) → a checked message with its files as bytes. Throws Refused. */
export function check(raw) {
  if (!raw || typeof raw !== 'object') throw bad('That message could not be read.');
  const kind = str(raw.kind), area = str(raw.area), title = str(raw.title).trim(), text = str(raw.text).trim(), github = str(raw.github).trim();
  if (!(kind in KINDS)) throw bad('Choose what the message is about.');
  if (!AREAS.includes(area)) throw bad('Choose where in Omega it is about.');
  if (!title || title.length > LIMITS.title) throw bad(`The title needs 1 to ${LIMITS.title} characters.`);
  if (text.length < LIMITS.minText || text.length > LIMITS.text) throw bad(`The description needs ${LIMITS.minText} to ${LIMITS.text} characters.`);
  if (!githubOk(github)) throw bad('That does not look like a GitHub name.');
  let basics = null;
  if (raw.basics) {
    const b = raw.basics; const ok = (v) => typeof v === 'string' && v.length > 0 && v.length <= LIMITS.version && /^[A-Za-z0-9._+-]+$/.test(v);
    if (!ok(b.version) || !ok(b.os) || !ok(b.arch)) throw bad('The version details could not be read.');
    basics = { version: b.version, os: b.os, arch: b.arch };
  }
  const list = Array.isArray(raw.files) ? raw.files : [];
  if (list.length > LIMITS.files) throw new Refused(400, 'too_many_files', `Up to ${LIMITS.files} files can be attached.`);
  let total = 0; const files = [];
  for (const f of list) {
    const name = str(f?.name), ext = extOf(name), type = FILE_TYPES[ext];
    if (!type) throw bad(`${name || 'A file'} is not a type that can be attached.`);
    const bytes = decode(str(f.data));
    if (!bytes || bytes.length === 0) throw bad(`${name} could not be read.`);
    if (bytes.length > LIMITS.fileBytes) throw new Refused(413, 'too_large', `${name} is too large (5 MB at most).`);
    total += bytes.length;
    if (total > LIMITS.totalBytes) throw new Refused(413, 'too_large', 'The files together are too large (12 MB at most).');
    if (!type.ok(bytes)) throw bad(`${name} does not look like a real .${ext} file.`);
    files.push({ name, ext, bytes, type });
  }
  return { kind, area, title, text, github, basics, files };
}
