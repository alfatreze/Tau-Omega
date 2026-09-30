<script lang="ts">
  // Settings -> Library: the preferences the Library screen reads. Loaded and
  // saved here directly so the Library screen and this page can never disagree.
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { errorMessage, getPrefs, setPrefs } from './tau-api';
  import type { Prefs, PrefsView } from './types';

  let prefs: PrefsView | null = null;
  let notice = '';
  onMount(async () => { try { prefs = await getPrefs(); } catch (error) { notice = `Could not read preferences: ${errorMessage(error)}`; } });

  async function update(change: Partial<Prefs>, message = 'Saved.') {
    if (!prefs) return;
    try { prefs = await setPrefs({ ...prefs, ...change }); notice = message; }
    catch (error) { notice = `Could not save: ${errorMessage(error)}`; }
  }
  async function chooseBackup() {
    const selected = await open({ directory: true });
    if (!selected || Array.isArray(selected)) return;
    await update({ backup_dir: selected }, 'Backup folder saved.');
  }
  const modes: { value: Prefs['remove_mode']; title: string; help: string }[] = [
    { value: 'backup', title: 'Back up, then remove', help: 'Removed files are copied to the backup folder first. Recommended.' },
    { value: 'ask', title: 'Ask each time', help: 'The sync review lets you choose whether to back up.' },
    { value: 'none', title: 'Just remove', help: 'Files are deleted from the card with no copy kept.' },
  ];
</script>
<section class="library-settings" aria-labelledby="lib-settings-title">
  <h2 id="lib-settings-title">Library</h2>
  {#if prefs}
    <fieldset>
      <legend>Removing from the Pocket</legend>
      {#each modes as m}
        <label class="radio"><input type="radio" name="remove-mode" value={m.value} checked={prefs.remove_mode === m.value} on:change={() => update({ remove_mode: m.value })} /><span><b>{m.title}</b><small>{m.help}</small></span></label>
      {/each}
    </fieldset>
    <div class="ls-row"><div><b>Backup folder</b><small>{prefs.backup_dir || prefs.default_backup_dir}{prefs.backup_dir ? '' : ' (default)'}</small></div>
      <div><button class="picker" on:click={chooseBackup}>Choose…</button>{#if prefs.backup_dir}<button class="quiet" on:click={() => update({ backup_dir: null }, 'Using the default backup folder.')}>Use default</button>{/if}</div></div>
    <div class="ls-row"><div><b>Sync history</b><small>{prefs.reports_dir}</small></div></div>
    <div class="ls-row"><div><b>Notices</b><small>Show the one-time messages again.</small></div>
      <div><button class="quiet" disabled={!prefs.remove_explained && !prefs.slow_alert_suppressed} on:click={() => update({ remove_explained: false, slow_alert_suppressed: false }, 'Notices will show again.')}>Reset notices</button></div></div>
    <p class="ls-notice" role="status">{notice}</p>
  {:else}<p>{notice || 'Loading…'}</p>{/if}
</section>
<style>
  .library-settings{max-width:720px;margin-bottom:14px;padding:22px 24px;border:1px solid #2c393a;border-radius:12px;background:#1a2325}
  h2{margin:0 0 12px}
  fieldset{border:0;padding:0;margin:0 0 14px}legend{font-weight:700;margin-bottom:8px}
  .radio{display:flex;gap:10px;align-items:flex-start;padding:8px 0;cursor:pointer}
  .radio span{display:flex;flex-direction:column;gap:2px}.radio small,.ls-row small{color:#8c9c9b;font-size:12px}
  .ls-row{display:flex;justify-content:space-between;align-items:center;gap:16px;padding:10px 0;border-top:1px solid #2c393a}
  .ls-row div:first-child{display:flex;flex-direction:column;gap:2px;min-width:0}.ls-row small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .ls-notice{min-height:1.2em;font-size:12px;color:#9cacab;margin:8px 0 0}
</style>
