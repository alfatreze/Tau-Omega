import App from './App.svelte';
import './styles.css';
import { mount } from 'svelte';
import { installOverlayScroll } from './lib/overlayScroll';
import { initTheme } from './lib/themeStore';

// Apply stored or system theme early and watch OS/storage updates.
initTheme();

// Slim scroll thumbs that appear when useful (lib/overlayScroll.ts).
installOverlayScroll();

async function start() {
  // Plain browser (no Tauri shell) in dev: run against a mocked backend.
  if (import.meta.env.DEV && !('__TAURI_INTERNALS__' in window)) {
    (await import('./lib/dev-mock')).installDevMock();
  }
  mount(App, { target: document.getElementById('app')! });
}
start();
