import { writable } from 'svelte/store';
import { disabledCores, executeCoreState, planCoreState } from './tau-api';
import { refreshCores } from './coresStore';
import type { DisabledCore, StateReport } from './types';

/** Cores moved out of `Cores/` on the open card, so the Pocket does not list them. */
export const disabledList = writable<DisabledCore[]>([]);

export async function refreshDisabled(card: string): Promise<void> {
  if (!card) { disabledList.set([]); return; }
  try { disabledList.set(await disabledCores(card)); } catch { disabledList.set([]); }
}

/** Disables or enables cores: plan, then the confirmed move (one card write), then the menu and the Disabled list are re-read. Throws the engine's error. */
export async function setCoresEnabled(card: string, ids: string[], enable: boolean, reasons: [string, string[]][] = []): Promise<StateReport> {
  const plan = await planCoreState(card, ids, enable);
  if (plan.blocked) throw new Error(plan.blocked);
  const report = await executeCoreState(card, ids, enable, plan.token, reasons);
  await Promise.all([refreshCores(card), refreshDisabled(card)]);
  return report;
}
