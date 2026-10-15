import { writable, get } from 'svelte/store';
import { errorMessage } from './backend';
import { getPrefs, marketplaceCoreUpdates } from './tau-api';
import { coreUpdateCount } from './marketStore';
import type { CoreUpdates } from './types';

/** The latest update check of the open card against the openFPGA Library (shared by the Card overview, the Cores > Updates tab and the menu badge). */
export const updatesResult = writable<CoreUpdates | null>(null);
export const updatesBusy = writable(false);
export const updatesError = writable('');
/** Whether the app's update check is on (Settings): only then does opening a card check by itself. */
export const updatesAutomatic = writable(false);

let checkedFor = '';

export async function checkUpdates(card: string, force: boolean): Promise<void> {
  if (!card) return;
  updatesBusy.set(true); updatesError.set('');
  try {
    const r = await marketplaceCoreUpdates(card, force);
    updatesResult.set(r);
    coreUpdateCount.set(r.rows.filter((x) => x.status === 'UpdateAvailable').length);
  } catch (e) { updatesError.set(errorMessage(e)); }
  finally { updatesBusy.set(false); }
}

/** Called when the open card changes: forgets the old result and, when checking is on, looks once. */
export async function autoCheckUpdates(card: string): Promise<void> {
  if (card === checkedFor) return;
  checkedFor = card;
  updatesResult.set(null); updatesError.set(''); coreUpdateCount.set(0);
  if (!card) return;
  try { updatesAutomatic.set((await getPrefs()).check_updates); } catch { updatesAutomatic.set(false); }
  if (get(updatesAutomatic) && checkedFor === card) await checkUpdates(card, false);
}
