// What a feedback message may contain. These mirror the app's own checks (ui/src/lib/feedback.ts) and are enforced again here:
// nothing the app says is trusted.

export const KINDS = { bug: 'Something is wrong', idea: 'An idea', question: 'A question' };
export const AREAS = ['Cards', 'Cores', 'Library', 'Playlists', 'Meter Lab', 'Appearance', 'Halcyon EQ', 'Marketplace', 'Updates', 'Settings', 'Something else'];
export const LIMITS = { title: 80, text: 4000, minText: 10, files: 4, fileBytes: 5 * 1024 * 1024, totalBytes: 12 * 1024 * 1024, github: 39, version: 32, body: 18 * 1024 * 1024 };

const sig = (b, ...bytes) => bytes.every((v, i) => b[i] === v);
const ascii = (b, from, s) => [...s].every((c, i) => b[from + i] === c.charCodeAt(0));
const textOk = (b) => { if (b.includes(0)) return false; try { new TextDecoder('utf-8', { fatal: true }).decode(b); return true; } catch { return false; } };

/** The allowed types: the extension, the type the file is served as, whether a browser may show it, and the check of its own first bytes. */
export const FILE_TYPES = {
  png: { mime: 'image/png', image: true, ok: (b) => sig(b, 0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a) },
  jpg: { mime: 'image/jpeg', image: true, ok: (b) => sig(b, 0xff, 0xd8, 0xff) },
  jpeg: { mime: 'image/jpeg', image: true, ok: (b) => sig(b, 0xff, 0xd8, 0xff) },
  gif: { mime: 'image/gif', image: true, ok: (b) => ascii(b, 0, 'GIF87a') || ascii(b, 0, 'GIF89a') },
  webp: { mime: 'image/webp', image: true, ok: (b) => ascii(b, 0, 'RIFF') && ascii(b, 8, 'WEBP') },
  txt: { mime: 'text/plain; charset=utf-8', image: false, ok: textOk },
  log: { mime: 'text/plain; charset=utf-8', image: false, ok: textOk },
  json: { mime: 'application/json; charset=utf-8', image: false, ok: textOk },
  zip: { mime: 'application/zip', image: false, ok: (b) => sig(b, 0x50, 0x4b, 0x03, 0x04) || sig(b, 0x50, 0x4b, 0x05, 0x06) },
};
export const extOf = (name) => (String(name).includes('.') ? String(name).split('.').pop().toLowerCase() : '');
const GITHUB = /^[A-Za-z0-9](?:[A-Za-z0-9]|-(?=[A-Za-z0-9]))*$/;
export const githubOk = (s) => s === '' || (s.length <= LIMITS.github && GITHUB.test(s));
