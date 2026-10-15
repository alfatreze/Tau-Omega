import { writable } from 'svelte/store';
import { cardRole, getPrefs, setCardRole } from './tau-api';

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
