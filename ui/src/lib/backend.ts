import { invoke as tauriInvoke, type InvokeArgs } from '@tauri-apps/api/core';

/** Thin shell adapter; feature code should depend on this function, not Tauri directly. */
export function invoke<T>(command: string, payload: InvokeArgs = {}): Promise<T> {
  return tauriInvoke<T>(command, payload);
}
