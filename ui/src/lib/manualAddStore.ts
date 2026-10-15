import { writable } from 'svelte/store';
/** The "Add a core manually" sheet: open or not, and what to start with (a dropped path or a pasted address). One sheet, mounted once in the app. */
export const manualAdd = writable<{ open: boolean; initial: string }>({ open: false, initial: '' });
export const openManualAdd = (initial = '') => manualAdd.set({ open: true, initial });
