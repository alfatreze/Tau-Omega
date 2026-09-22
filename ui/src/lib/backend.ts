import { invoke as tauriInvoke, type InvokeArgs } from '@tauri-apps/api/core';
import type { ApiError } from './types';

/** Thin shell adapter; feature code should depend on this function, not Tauri directly. */
export function invoke<T>(command: string, payload: InvokeArgs = {}): Promise<T> {
  return tauriInvoke<T>(command, payload);
}

/** The English sentence from a rejected `invoke()`. The engine's error `code`
 * (see `ApiError`) is still on the object for callers that want to branch on
 * it; this only picks the part meant for display. */
export function errorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error) {
    return String((error as ApiError).message);
  }
  return String(error);
}
