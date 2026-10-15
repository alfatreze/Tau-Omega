<script lang="ts">
  // Offered once per card per session: cores the Pocket cannot use (a file they need is missing, or the core itself is broken) can be hidden from its
  // menu. Nothing is deleted; each core is moved to "Disabled Cores" and can be enabled again from the Cores page. The setting decides whether we ask.
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { coreKeep, coreSuggestions } from './tau-api';
  import { setCoresEnabled } from './coreStateStore';
  import { plural } from './coresFormat';
  import type { CoreHealth } from './types';
  /** The open card, and whether its cores have been read (so a half-typed path is never checked). */
  export let card = ''; export let ready = false; export let changed: () => void = () => {};

  let asked = ''; let open = false; let list: CoreHealth[] = []; let picked: Record<string, boolean> = {};
  let remember = false; let busy = false; let error = ''; let autoNote = '';
  $: if (card && ready && card !== asked) { asked = card; void check(); }
  async function check() {
    autoNote = '';
    try {
      const s = await coreSuggestions(card);
      if (!s.cores.length || s.mode === 'never') return;
      if (s.mode === 'auto') { await disable(s.cores); autoNote = `${plural(s.cores.length, 'core')} the Pocket could not use ${s.cores.length === 1 ? 'was' : 'were'} hidden from its menu. Enable ${s.cores.length === 1 ? 'it' : 'them'} again under Cores › Disabled.`; return; }
      list = s.cores; picked = Object.fromEntries(s.cores.map((c) => [c.id, true])); remember = false; error = ''; open = true;
    } catch { /* a card with nothing to suggest, or unreadable: stay quiet */ }
  }
  $: chosen = list.filter((c) => picked[c.id]);
  async function disable(cores: CoreHealth[]) {
    await setCoresEnabled(card, cores.map((c) => c.id), false, cores.map((c) => [c.id, c.reasons.map((r) => r.message)] as [string, string[]]));
  }
  async function confirm() {
    busy = true; error = '';
    try {
      if (chosen.length) await disable(chosen);
      if (remember) { const left = list.filter((c) => !picked[c.id]).map((c) => c.id); if (left.length) await coreKeep(card, left, true); }
      open = false; changed();
    } catch (e) { error = `Nothing more was changed. ${errorMessage(e)}`; }
    finally { busy = false; }
  }
  async function later() {
    busy = true; error = '';
    try { if (remember) await coreKeep(card, list.map((c) => c.id), true); open = false; }
    catch (e) { error = errorMessage(e); }
    finally { busy = false; }
  }
</script>
{#if autoNote}<p class="dc-auto" role="status">{autoNote} <button class="quiet" on:click={() => (autoNote = '')}>Dismiss</button></p>{/if}
{#if open}
  <div class="dc-backdrop" role="presentation"></div>
  <div class="dc" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="dc-title" use:modal on:keydown={(e) => e.key === 'Escape' && !busy && later()}>
    <header><p class="eyebrow">THE POCKET CANNOT USE THESE</p><h2 id="dc-title">Hide {plural(list.length, 'core')} from the Pocket’s menu?</h2></header>
    <p class="dc-lede">{list.length === 1 ? 'This core' : 'These cores'} cannot start or has nothing to play on this card. Hiding keeps the menu short. <strong>Nothing is deleted</strong>: each core is moved to <code>Disabled Cores</code> with its saves and settings in place, and you can enable it again from Cores any time.</p>
    {#if error}<p class="dc-err" role="alert">{error}</p>{/if}
    <ul class="dc-list" aria-label="Cores to hide">
      {#each list as c (c.id)}
        <li><label><input type="checkbox" bind:checked={picked[c.id]} /><span class="dc-main"><strong>{c.shortname || c.id}</strong><small>{c.id} · {c.verdict === 'Broken' ? 'Broken' : 'Missing files'}</small>
          {#each c.reasons as r}<span class="dc-why">{r.message} <em>{r.fix}</em></span>{/each}</span></label></li>
      {/each}
    </ul>
    <p class="dc-note">A core that needs no files, or has a quirk these checks cannot see, can be left enabled: untick it.</p>
    <label class="dc-remember"><input type="checkbox" bind:checked={remember} /> Don’t remind me again about the cores I leave enabled</label>
    <div class="dc-actions"><button class="quiet" data-autofocus disabled={busy} on:click={later}>Not now</button>
      <button class="primary" disabled={busy || !chosen.length} on:click={confirm}>{busy ? 'Working…' : `Hide ${plural(chosen.length, 'core')}`}</button></div>
  </div>
{/if}
<style>
  .dc-backdrop{position:fixed;inset:0;background:rgba(0,0,0,.55);z-index:100}
  .dc{position:fixed;z-index:101;top:50%;left:50%;transform:translate(-50%,-50%);width:min(660px,calc(100vw - 32px));max-height:calc(100vh - 48px);overflow:auto;background:#1a2325;border:1px solid #2c393a;border-radius:14px;padding:22px 24px;box-sizing:border-box}
  .dc h2{margin:0 0 8px}.dc-lede{font-size:13px;line-height:1.6;color:#dce6e4;margin:0 0 12px}.dc-err{color:#ff9b9b;font-size:12.5px}
  .dc-list{list-style:none;margin:0 0 10px;padding:0;border:1px solid #2c393a;border-radius:10px;max-height:300px;overflow:auto}
  .dc-list li{border-bottom:1px solid #2c393a}.dc-list li:last-child{border:0}
  .dc-list label{display:flex;gap:12px;align-items:flex-start;padding:11px 14px;cursor:pointer}.dc-list label:hover{background:#1c282a}
  .dc-main{display:grid;gap:3px;min-width:0}.dc-main strong{font-size:14px}.dc-main small{font-size:12px;color:#95a5a4}
  .dc-why{font-size:12px;color:#e5d9a8;line-height:1.5}.dc-why em{font-style:normal;color:#9fb3aa}
  .dc-note{font-size:12px;color:#8f9e9d;margin:6px 0}.dc-remember{display:flex;gap:9px;align-items:center;font-size:12.5px;color:#c7d1d0;margin:8px 0 14px}
  .dc-actions{display:flex;justify-content:flex-end;gap:10px}
  .dc-auto{margin:12px 16px;padding:10px 14px;border:1px solid #3b5a58;border-radius:10px;background:#16292a;color:#dce6e4;font-size:13px}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
</style>
