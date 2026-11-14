// Feedback to the Tau Omega project: what a message is made of and what may be attached. Pure logic (no browser, no network).
// The service that files the issue builds the real labels and text itself from these fields (docs/FEEDBACK_SERVICE.md).

export type Kind = 'bug' | 'idea' | 'question';
export const KINDS: { id: Kind; label: string; hint: string; placeholder: string }[] = [
  { id: 'bug', label: 'Something is wrong', hint: 'A bug or something that does not work as it should', placeholder: 'What happened? What did you expect? If you can, tell us the steps to make it happen again.' },
  { id: 'idea', label: 'An idea', hint: 'Something new or something that could be better', placeholder: 'What would you like Omega to do, and why would it help?' },
  { id: 'question', label: 'A question', hint: 'Something you could not work out', placeholder: 'What would you like to know?' },
];

export const AREAS = ['Cards', 'Cores', 'Library', 'Playlists', 'Meter Lab', 'Appearance', 'Halcyon EQ', 'Marketplace', 'Updates', 'Settings', 'Something else'] as const;
export type Area = (typeof AREAS)[number];

export const LIMITS = {
  title: 80, text: 4000, minText: 10,
  files: 4, fileBytes: 5 * 1024 * 1024, totalBytes: 12 * 1024 * 1024,
  github: 39,
} as const;

/** What may be attached: pictures, plain text and logs, and a zip (Omega's own diagnostics report is one). */
export const FILE_TYPES: { ext: string; mimes: string[]; image: boolean }[] = [
  { ext: 'png', mimes: ['image/png'], image: true },
  { ext: 'jpg', mimes: ['image/jpeg'], image: true },
  { ext: 'jpeg', mimes: ['image/jpeg'], image: true },
  { ext: 'gif', mimes: ['image/gif'], image: true },
  { ext: 'webp', mimes: ['image/webp'], image: true },
  { ext: 'txt', mimes: ['text/plain'], image: false },
  { ext: 'log', mimes: ['text/plain', 'text/x-log', 'application/octet-stream'], image: false },
  { ext: 'json', mimes: ['application/json', 'text/plain'], image: false },
  { ext: 'zip', mimes: ['application/zip', 'application/x-zip-compressed', 'application/octet-stream'], image: false },
];
export const ACCEPT = FILE_TYPES.map((t) => '.' + t.ext).join(',');
export const TYPE_NAMES = 'pictures (png, jpg, gif, webp), text (txt, log, json) and zip';

export const megabytes = (n: number) => (n >= 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
export const extOf = (name: string) => (name.includes('.') ? name.split('.').pop()!.toLowerCase() : '');
export const isImage = (name: string) => FILE_TYPES.find((t) => t.ext === extOf(name))?.image === true;

export type Chosen = { name: string; type: string; size: number };

/** Why a file cannot be attached, in plain words, or null when it can. `have` are the files already chosen. */
export function checkFile(f: Chosen, have: Chosen[]): string | null {
  const kind = FILE_TYPES.find((t) => t.ext === extOf(f.name));
  if (!kind) return `${f.name} is not a type we can take. You can attach ${TYPE_NAMES}.`;
  if (f.type && !kind.mimes.includes(f.type)) return `${f.name} does not look like a .${kind.ext} file.`;
  if (f.size === 0) return `${f.name} is empty.`;
  if (f.size > LIMITS.fileBytes) return `${f.name} is ${megabytes(f.size)}. Each file can be up to ${megabytes(LIMITS.fileBytes)}.`;
  if (have.length >= LIMITS.files) return `You can attach up to ${LIMITS.files} files.`;
  if (have.some((h) => h.name === f.name && h.size === f.size)) return `${f.name} is already attached.`;
  if (have.reduce((n, h) => n + h.size, 0) + f.size > LIMITS.totalBytes) return `Adding ${f.name} would pass ${megabytes(LIMITS.totalBytes)} in total.`;
  return null;
}

export type Draft = { kind: Kind; area: Area; title: string; text: string; basics: boolean; github: string };
export const emptyDraft = (): Draft => ({ kind: 'bug', area: 'Something else', title: '', text: '', basics: true, github: '' });

/** A GitHub user name: letters, digits and single hyphens, not starting or ending with one. Empty is fine (it is optional). */
export const githubOk = (s: string) => s === '' || (s.length <= LIMITS.github && /^[A-Za-z0-9](?:[A-Za-z0-9]|-(?=[A-Za-z0-9]))*$/.test(s));

/** What is missing before it can be sent, in plain words (empty when it is ready). */
export function problems(d: Draft, attached: number): string[] {
  const out: string[] = [];
  if (!d.title.trim()) out.push('Give it a short title.');
  if (d.text.trim().length < LIMITS.minText) out.push('Say a little more in the description.');
  if (!githubOk(d.github.trim())) out.push('That does not look like a GitHub name.');
  if (attached > LIMITS.files) out.push(`Too many files (up to ${LIMITS.files}).`);
  return out;
}

export const osName = (os: string) => ({ macos: 'macOS', windows: 'Windows', linux: 'Linux' } as Record<string, string>)[os] ?? os;
