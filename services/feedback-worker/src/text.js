// Turning what a stranger typed into text that is safe to put in a public issue.

const ZW = '​';
/** `@name` would notify that person or team: a zero-width space after the @ keeps the words and drops the notification. */
export const noMentions = (s) => s.replace(/@(?=[A-Za-z0-9_])/g, '@' + ZW);

/** The sender's text: no raw HTML, no pictures from other sites (they would report who reads the issue), links shown as words, no mentions. */
export function cleanText(s) {
  return noMentions(
    String(s).replace(/\r\n?/g, '\n').replace(/\u0000/g, '')
      .replace(/<(?=[A-Za-z!\/?])/g, '&lt;')
      .replace(/!\[([^\]]*)\]\([^)]*\)/g, (_, alt) => `[picture removed${alt ? ': ' + alt : ''}]`)
      .replace(/\[([^\]]+)\]\(([^)\s]+)[^)]*\)/g, (_, t, u) => `${t} (${u})`)
      .trim(),
  );
}
export const cleanTitle = (s) => noMentions(String(s).replace(/\s+/g, ' ').replace(/[<>]/g, '').trim());
/** A name for display only (the stored name is random). */
export const displayName = (s) => String(s).replace(/[^A-Za-z0-9._ -]/g, '_').replace(/^\.+/, '').slice(0, 60) || 'file';
