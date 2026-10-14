import { writable } from 'svelte/store';
/** How many community cores on the open card have a newer release in the catalog (drives the nav badge). */
export const coreUpdateCount = writable(0);
