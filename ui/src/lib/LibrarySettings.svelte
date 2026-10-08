<script lang="ts">
  // Settings -> Library: the preferences the Library screen and the sync
  // history read. Loaded and saved here directly so the screens and this page
  // can never disagree.
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { clearHistory, errorMessage, getPrefs, listHistory, pruneHistory, setPrefs, setReportsDir } from './tau-api';
  import type { Prefs, PrefsView } from './types';

  /** Opens the sync history page. */
  export let openHistory: () => void = () => {};

  let prefs: PrefsView | null = null;
  let notice = '';
  let stored = 0;
  let confirmClear = false;
  async function refreshCount() { try { stored = (await listHistory()).length; } catch { stored = 0; } }
  onMount(async () => {
    try { prefs = await getPrefs(); } catch (error) { notice = `Could not read preferences: ${errorMessage(error)}`; }
    await refreshCount();
  });

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
  /** Retention changes apply straight away, and say how many old entries that removed. */
  async function setRetention(change: Partial<Prefs>) {
    await update(change, 'Saved.');
    try { const removed = await pruneHistory(); if (removed) notice = `Saved. Removed ${removed} older ${removed === 1 ? 'entry' : 'entries'} from the history.`; } catch { /* retention is best-effort */ }
    await refreshCount();
  }
  async function chooseHistoryFolder() {
    const selected = await open({ directory: true });
    if (!selected || Array.isArray(selected)) return;
    try { await setReportsDir(selected); prefs = await getPrefs(); notice = 'History folder changed. New syncs are recorded there; the old folder is left as it is.'; window.dispatchEvent(new CustomEvent('tau-reports-dir')); await refreshCount(); }
    catch (error) { notice = `Could not change the history folder: ${errorMessage(error)}`; }
  }
  async function doClear() {
    confirmClear = false;
    try { const removed = await clearHistory(); notice = `Deleted ${removed} ${removed === 1 ? 'entry' : 'entries'} from the sync history.`; await refreshCount(); }
    catch (error) { notice = `Could not clear the history: ${errorMessage(error)}`; }
  }
  const modes: { value: Prefs['remove_mode']; title: string; help: string }[] = [
    { value: 'backup', title: 'Back up, then remove', help: 'Removed files are copied to the backup folder first. Recommended.' },
    { value: 'ask', title: 'Ask each time', help: 'The sync review lets you choose whether to back up.' },
    { value: 'none', title: 'Just remove', help: 'Files are deleted from the card with no copy kept.' },
  ];
  const KEEP_LAST = [{ v: 25, l: 'Last 25 syncs' }, { v: 50, l: 'Last 50 syncs' }, { v: 100, l: 'Last 100 syncs' }, { v: 250, l: 'Last 250 syncs' }, { v: 500, l: 'Last 500 syncs' }, { v: 0, l: 'All of them' }];
  const KEEP_DAYS = [{ v: 30, l: '30 days' }, { v: 90, l: '3 months' }, { v: 180, l: '6 months' }, { v: 365, l: '1 year' }, { v: 730, l: '2 years' }, { v: 0, l: 'Forever' }];
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

    <h3 class="ls-h">Sync history</h3>
    <p class="ls-lede">Every sync is recorded so you can see what changed and whether it finished. Old entries are removed automatically using the limits below (whichever is reached first).</p>
    <div class="ls-row">
      <div><b>Keep</b><small>Older entries beyond this many syncs are deleted.</small></div>
      <select aria-label="Number of syncs to keep" value={prefs.history_keep_last} on:change={(e) => setRetention({ history_keep_last: Number(e.currentTarget.value) })}>
        {#each KEEP_LAST as o}<option value={o.v}>{o.l}</option>{/each}
      </select>
    </div>
    <div class="ls-row">
      <div><b>Delete after</b><small>Entries older than this are deleted.</small></div>
      <select aria-label="How long to keep history" value={prefs.history_keep_days} on:change={(e) => setRetention({ history_keep_days: Number(e.currentTarget.value) })}>
        {#each KEEP_DAYS as o}<option value={o.v}>{o.l}</option>{/each}
      </select>
    </div>
    <div class="ls-row"><div><b>History folder</b><small>{prefs.reports_dir}</small></div><div><button class="picker" on:click={chooseHistoryFolder}>Choose…</button></div></div>
    <div class="ls-row"><div><b>{stored} {stored === 1 ? 'sync' : 'syncs'} stored</b><small>Browse what each one changed, or clear the list.</small></div>
      <div class="ls-btns">
        <button class="picker" on:click={openHistory}>Browse sync history</button>
        {#if !confirmClear}<button class="quiet" disabled={!stored} on:click={() => (confirmClear = true)}>Clear history…</button>
        {:else}<span class="ls-confirm" role="alert">Delete all {stored}? <button class="quiet danger" on:click={doClear}>Delete</button><button class="quiet" on:click={() => (confirmClear = false)}>Cancel</button></span>{/if}
      </div></div>

    <div class="ls-row"><div><b>Check for Tau updates</b><small>When the app starts, ask GitHub for the public release list (nothing about you or your card is sent). Installing is always your action.</small></div>
      <div><button class="quiet" on:click={() => { const on = !prefs?.check_updates; void update({ check_updates: on, update_notice_shown: true }, on ? 'Update checks are on.' : 'Update checks are off.'); }}>{prefs.check_updates ? 'Turn off' : 'Turn on'}</button></div></div>

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
  .ls-h{font-size:15px;margin:22px 0 4px}.ls-lede{margin:0 0 6px;font-size:12px;color:#8c9c9b}
  .ls-row{display:flex;justify-content:space-between;align-items:center;gap:16px;padding:10px 0;border-top:1px solid #2c393a}
  .ls-row > div:first-child{display:flex;flex-direction:column;gap:2px;min-width:0}.ls-row small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .ls-row select{background:#111617;color:#e8ecec;border:1px solid #2c393a;border-radius:8px;padding:8px 10px;font-size:13px}
  .ls-btns{display:flex;gap:8px;align-items:center;flex-wrap:wrap;justify-content:flex-end}
  .ls-confirm{display:flex;gap:6px;align-items:center;font-size:13px;color:#f4cfc4}.ls-confirm .danger{color:#ff9d8a}
  .ls-notice{min-height:1.2em;font-size:12px;color:#9cacab;margin:8px 0 0}
</style>
