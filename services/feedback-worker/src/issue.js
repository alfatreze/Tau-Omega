import { KINDS } from './rules.js';
import { cleanText, cleanTitle } from './text.js';

const OS = { macos: 'macOS', windows: 'Windows', linux: 'Linux' };
export const labelsFor = (kind, area) => [`type: ${kind}`, `area: ${area.toLowerCase()}`, 'from: omega'];

/** The issue as it is filed. `links` are the stored attachments: { name, url, image }. */
export function buildIssue(m, links) {
  const lines = [cleanText(m.text), ''];
  const pics = links.filter((l) => l.image), other = links.filter((l) => !l.image);
  if (pics.length) lines.push(pics.map((l) => `![${l.name}](${l.url})`).join('\n'), '');
  if (other.length) lines.push('Attachments: ' + other.map((l) => `[${l.name}](${l.url})`).join(', '), '');
  lines.push('---', `Category: ${KINDS[m.kind]} · ${m.area}`);
  if (m.basics) lines.push(`Sent from Tau Omega ${m.basics.version} (${OS[m.basics.os] ?? m.basics.os}, ${m.basics.arch})`);
  if (m.github) lines.push(`Reply on GitHub: @${m.github}`);
  return { title: cleanTitle(m.title), body: lines.join('\n'), labels: labelsFor(m.kind, m.area) };
}
