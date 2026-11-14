import { writable } from 'svelte/store';
import { emptyDraft, type Draft } from './feedback';

/** What the sender has written so far and attached, kept while they look at other pages (it is gone when Omega closes). */
export const feedbackDraft = writable<Draft>(emptyDraft());
export const feedbackFiles = writable<File[]>([]);
