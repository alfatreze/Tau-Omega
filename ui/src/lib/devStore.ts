import { writable } from 'svelte/store';
import { cardRole, getPrefs, setCardRole, setLastInstall } from './tau-api';

/** Whether the developer tools are shown (Settings > "Show developer tools"). Off until the user turns it on. */
export const developerTools = writable(false);
/** The open card's role: `test` only when the card carries the marker; every other card is `real`. */
export const roleOf = writable<{ card: string; role: 'real' | 'test' }>({ card: '', role: 'real' });

export async function loadDeveloperTools(): Promise<void> { try { developerTools.set(!!(await getPrefs()).developer_tools); } catch { developerTools.set(false); } }
export async function refreshRole(card: string): Promise<void> {
  if (!card) { roleOf.set({ card: '', role: 'real' }); return; }
  try { roleOf.set({ card, role: await cardRole(card) }); } catch { roleOf.set({ card, role: 'real' }); }
}
export async function markTestCard(card: string, test: boolean): Promise<void> { roleOf.set({ card, role: await setCardRole(card, test) }); }
export const cardLabel = (card: string) => card.split('/').filter(Boolean).pop() ?? card;

/** The host's local clock in the Pocket's own screenshot-stamp shape (`YYYY-MM-DDTHH:MM:SS`). */
export function localStamp(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}
/** Remembers "the last install was just now" for the Results page (best effort: it is advice, never a reason to fail an install). */
export async function markInstalled(card: string): Promise<void> { try { await setLastInstall(card, localStamp()); } catch { /* ignore */ } }
