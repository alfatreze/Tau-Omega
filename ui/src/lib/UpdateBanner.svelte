<script lang="ts">
  import { onMount } from 'svelte';
  import { getPrefs, setPrefs, updateCheck, updateDownload } from './tau-api';
  import { errorMessage } from './backend';
  import type { PrefsView, UpdateCheck } from './types';
  /** The card folder currently open, so the check can compare it with the newest release. */
  export let card = '';
  /** Called with the verified zip path when the user presses Update; the host opens the package page on it. */
  export let openPackage: (zipPath: string) => void;

  let prefs: PrefsView | null = null;
  let found: UpdateCheck | null = null;
  let dismissed = false; let busy = false; let notice = '';
  let checkedFor: string | null = null;

  onMount(async () => { try { prefs = await getPrefs(); } catch { /* the check simply stays off */ } });
  // One check per card, only once the one-time note has been shown, and only if the user has not turned it off.
  $: if (prefs && prefs.update_notice_shown && prefs.check_updates && checkedFor !== card) { checkedFor = card; void run(); }

  async function run() {
    try { found = await updateCheck(card || null); } catch { found = null; /* offline or limited: stay quiet, the Settings page can retry */ }
  }
  async function acknowledge(keepOn: boolean) {
    if (!prefs) return;
    try { prefs = await setPrefs({ ...prefs, update_notice_shown: true, check_updates: keepOn }); } catch { /* the note shows again next time */ }
  }
  async function install() {
    if (!found || !found.zips.length) return;
    busy = true; notice = '';
    try {
      const [zip] = await updateDownload(found.latest.tag, [found.zips[0].name]);
      openPackage(zip.path);
    } catch (error) { notice = errorMessage(error); } finally { busy = false; }
  }
</script>
{#if prefs && !prefs.update_notice_shown}
  <div class="update-bar" role="region" aria-label="Update check">
    <span>Tau Omega can check GitHub for new Tau releases when it starts. It only asks for the public release list: nothing about you, your card or your music is sent. Nothing is downloaded or installed unless you press Update.</span>
    <span class="update-actions"><button class="primary" on:click={() => acknowledge(true)}>OK, check for updates</button><button class="quiet" on:click={() => acknowledge(false)}>Don't check</button></span>
  </div>
{:else if found && (found.newer === true || found.others.length) && !dismissed}
  <div class="update-bar" role="status">
    <span>{#if found.newer === true}{found.message}{#if !found.zips.length} The release has no verifiable download yet.{/if}{/if}{#each found.others as other}<span class="update-other">{other}</span>{/each}</span>
    <span class="update-actions">
      {#if found.newer === true && found.zips.length}<button class="primary" disabled={busy} on:click={install}>{busy ? 'Downloading…' : 'Update'}</button>{/if}
      <button class="quiet" on:click={() => (dismissed = true)}>Not now</button>
    </span>
    {#if notice}<span class="update-error" role="alert">{notice}</span>{/if}
  </div>
{/if}
<style>
  .update-bar{display:flex;flex-wrap:wrap;gap:10px 16px;align-items:center;justify-content:space-between;margin:12px 16px 14px;padding:12px 56px 12px 16px;border:1px solid #3b5a58;border-radius:10px;background:#16292a;color:#dce6e4;font-size:13px;line-height:1.5}
  .update-other{display:block;opacity:.8}
  .update-actions{display:flex;gap:8px}
  .update-error{flex-basis:100%;color:#ff9b9b}
</style>
