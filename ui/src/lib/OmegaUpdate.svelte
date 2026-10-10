<script lang="ts">
  // Updates for Tau Omega itself (not the Tau core: that is UpdateBanner). `manual` shows a card with a "Check now" button for
  // the Settings page; otherwise it is a bar that appears at start-up only when a newer Tau Omega exists. Nothing is downloaded
  // or installed until the user presses Update; the installer is verified against the app's built-in key first.
  import { onMount } from 'svelte';
  import { getPrefs, omegaUpdateCheck, omegaUpdateInstall } from './tau-api';
  import { errorMessage } from './backend';
  import type { OmegaUpdate } from './types';
  export let manual = false;

  let found: OmegaUpdate | null = null;
  let checked = false; let busy = false; let installing = false; let notice = ''; let dismissed = false;

  async function check(report: boolean) {
    busy = true; notice = '';
    try { found = await omegaUpdateCheck(); checked = true; if (report && !found) notice = 'Tau Omega is up to date.'; }
    catch (error) { if (report) notice = errorMessage(error); }
    finally { busy = false; }
  }
  async function install() {
    installing = true; notice = '';
    try { await omegaUpdateInstall(); } catch (error) { notice = errorMessage(error); installing = false; }
  }
  onMount(async () => {
    if (manual) return;
    try { const prefs = await getPrefs(); if (prefs.update_notice_shown && prefs.check_updates) await check(false); } catch { /* stays quiet */ }
  });
</script>

{#if manual}
  <section class="ou-card" aria-labelledby="ou-title">
    <h3 id="ou-title">Tau Omega updates</h3>
    <p class="muted">Checks GitHub for a newer Tau Omega. Only the public update file is requested; nothing about you or your card is sent.</p>
    <div class="ou-row">
      <button class="quiet" disabled={busy || installing} on:click={() => check(true)}>{busy ? 'Checking…' : 'Check now'}</button>
      {#if found}<button class="primary" disabled={installing} on:click={install}>{installing ? 'Installing…' : `Update to ${found.version}`}</button>{/if}
    </div>
    {#if found}<p>Version {found.version} is available (you have {found.current}). Tau Omega restarts to finish.</p>{#if found.notes}<pre class="ou-notes">{found.notes}</pre>{/if}{/if}
    {#if notice}<p role="status">{notice}</p>{/if}
  </section>
{:else if found && !dismissed}
  <div class="ou-bar" role="status">
    <span>Tau Omega {found.version} is available (you have {found.current}). It downloads a signed installer and restarts the app.</span>
    <span class="ou-actions">
      <button class="primary" disabled={installing} on:click={install}>{installing ? 'Installing…' : 'Update Tau Omega'}</button>
      <button class="quiet" disabled={installing} on:click={() => (dismissed = true)}>Not now</button>
    </span>
    {#if notice}<span class="ou-error" role="alert">{notice}</span>{/if}
  </div>
{/if}

<style>
  .ou-bar{display:flex;flex-wrap:wrap;gap:10px 16px;align-items:center;justify-content:space-between;margin:12px 16px 14px;padding:12px 56px 12px 16px;border:1px solid #3b5a58;border-radius:10px;background:#16292a;color:#dce6e4;font-size:13px;line-height:1.5}
  .ou-actions,.ou-row{display:flex;gap:8px;flex-wrap:wrap}
  .ou-error{flex-basis:100%;color:#ff9b9b}
  .ou-card{display:grid;gap:8px;margin:16px 0;padding:14px 16px;border:1px solid #2c393a;border-radius:10px;background:#161f20}
  .ou-card h3,.ou-card p{margin:0}
  .ou-notes{white-space:pre-wrap;margin:0;font-size:12px;max-height:160px;overflow:auto}
  button{padding:8px 12px;font-size:13px;background:#202b2d;color:#e8ecec;border:1px solid #2c393a;border-radius:8px;cursor:pointer}
  button.quiet{background:transparent}button.primary{background:#c1f0ad;color:#142015;font-weight:700;border-color:#c1f0ad}
  button:disabled{opacity:.5;cursor:not-allowed}
</style>
