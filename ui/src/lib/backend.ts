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

/** A sentence for a person, not an engine: what went wrong and what to do next.
 * Branches on the engine's stable error `code` (see `ApiError`), falling back
 * to the engine's own message when it is already plain. Never hides the cause:
 * unknown failures keep the original text. */
export function explainError(error: unknown): string {
  const code = error && typeof error === 'object' && 'code' in error ? Number((error as ApiError).code) : 0;
  const message = errorMessage(error);
  const text = message.toLowerCase();
  switch (code) {
    case 44: return 'Stopped. Files already copied were kept and verified, and the index is only written at the end of a run, so the card still works.';
    case 35: return 'The plan changed since you reviewed it. Close this and press Start sync again to review the current plan.';
    case 39: return 'A file changed on your computer or the card after you reviewed the plan. Press Start sync again to review it.';
    case 38: return `A copied file did not verify, so the sync stopped without finishing. Nothing was reported complete; run it again. (${message})`;
    case 36: return 'The backup folder must be outside the card. Choose a different one in Settings → Library.';
    case 40: return 'That cover image can’t be used. Use a baseline (not progressive) JPEG under 2 MiB.';
    case 51: return `There isn’t enough room on the card for this, counting how the card stores files, so nothing was copied. Remove something or add less. (${message})`;
    case 50: return `This file’s tags can’t be rewritten safely, so it was left untouched. (${message})`;
    case 45: return `That folder can’t be found. It may have been moved, renamed or disconnected. (${message})`;
    case 42:
      if (/os error (2|3|19|5)\b|no such file|not found|no such device|input\/output error/.test(text)) return 'The card couldn’t be reached. It may have been disconnected. Nothing was reported complete; reconnect it and try again.';
      if (/os error 28|no space|disk full/.test(text)) return 'The card is full. Remove something or add less, then try again.';
      if (/os error (13|30)\b|read-only|permission denied/.test(text)) return 'The card is read-only or locked. Check the SD card’s lock switch and that the Pocket isn’t in a mode that blocks writing, then try again.';
      return `Something went wrong reading or writing the card. Nothing was reported complete. (${message})`;
    default: return message;
  }
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
