import { invoke as tauriInvoke, type InvokeArgs } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ApiError, ProgressEvent } from './types';

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

/** A fresh id for a cancellable job, passed to a scan/plan/execute command and
 * back to `cancelJob`. */
export function newJobId(): string {
  return typeof crypto !== 'undefined' && 'randomUUID' in crypto
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

/** Subscribes to progress events from a running scan/plan/execute command.
 * `handler` is called for every job, so callers should check `job_id`. */
export function onProgress(handler: (event: ProgressEvent) => void) {
  return listen<ProgressEvent>('tau://progress', (event) => handler(event.payload));
}

/** Asks a running job to stop. It is cooperative: the engine checks between
 * units of work, so it may finish the current file before stopping. */
export function cancelJob(jobId: string): Promise<void> {
  return invoke<void>('cancel_job', { jobId });
}
