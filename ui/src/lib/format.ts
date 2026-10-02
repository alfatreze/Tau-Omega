/** Shared number formatting for the Library screens (decimal units, like the card's own capacity). */
const MB = 1e6;
const GB = 1e9;
export const size = (n: number): string =>
  n >= GB ? `${(n / GB).toFixed(1)} GB` : n >= MB ? `${Math.round(n / MB)} MB` : `${Math.max(1, Math.round(n / 1000))} KB`;
export const plural = (n: number, word: string): string => `${n} ${word}${n === 1 ? '' : 's'}`;
