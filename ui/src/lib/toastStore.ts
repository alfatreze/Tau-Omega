import { writable } from 'svelte/store';

export type ToastTone = 'success' | 'info' | 'error';

export type ToastItem = {
  id: number;
  message: string;
  tone: ToastTone;
  durationMs: number;
};

export const toasts = writable<ToastItem[]>([]);

let nextId = 1;

/** Shows a bottom-right toast notification and keeps at most the 4 most recent items. */
export function showToast(
  message: string,
  tone: ToastTone = 'success',
  durationMs = 5500,
): number {
  const clean = message.trim();
  if (!clean) return 0;
  const id = nextId++;
  toasts.update((list) => [...list.slice(-3), { id, message: clean, tone, durationMs }]);
  return id;
}

export function dismissToast(id: number) {
  toasts.update((list) => list.filter((t) => t.id !== id));
}
