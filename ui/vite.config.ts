import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
// `fs.allow` lets the dev server read the real fixtures in ../testdata (the dev mock uses them; production builds do not).
export default defineConfig({ plugins: [svelte()], server: { fs: { allow: ['..'] } } });
