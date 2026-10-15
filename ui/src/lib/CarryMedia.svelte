<script lang="ts">
  // Carry media from the previous build. A new test build is a new core id, so the files an older build kept in its own asset folder are not where the
  // new one looks. Each carry is reviewed first; files are copied and checked one by one, and a file that is already there and different is never overwritten.
  import { onMount } from 'svelte';
  import { errorMessage, newJobId } from './backend';
  import { carryCandidates, carryExecute, carryPlan, executeCoreCopy, getReportsDir, planCoreCopy } from './tau-api';
  import { plural, size } from './coresFormat';
  import type { CarryCandidate, CarryPlan, InstalledCore, Plan } from './types';
  export let card = ''; export let newId = ''; export let newName = ''; export let older: InstalledCore[] = [];

  let list: CarryCandidate[] | null = null; let error = '';
  let plans: Record<string, CarryPlan> = {}; let libPlans: Record<string, Plan> = {}; let msg: Record<string, string> = {}; let busy = '';
  const key = (c: CarryCandidate) => `${c.kind}:${c.from_id}:${c.platform}`;
  onMount(async () => { try { list = await carryCandidates(card, newId, older.map((c) => [c.id, c.shortname])); } catch (e) { error = errorMessage(e); } });
  const say = (c: CarryCandidate, m: string) => { msg[key(c)] = m; msg = msg; };

  async function review(c: CarryCandidate) {
    busy = key(c); say(c, '');
    try { plans[key(c)] = await carryPlan(card, c.from_id, newId, c.platform); plans = plans; } catch (e) { say(c, errorMessage(e)); } finally { busy = ''; }
  }
  async function carry(c: CarryCandidate) {
    const p = plans[key(c)]; if (!p) return; busy = key(c);
    try { const r = await carryExecute(card, c.from_id, newId, c.platform, p.id); delete plans[key(c)]; plans = plans; say(c, `Copied ${plural(r.copied, 'file')} (${size(r.bytes_written)}), each checked after copying.${r.identical ? ` ${plural(r.identical, 'file')} were already there.` : ''}${r.left_different ? ` ${plural(r.left_different, 'file')} differ from the old build's and were left alone.` : ''}`); }
    catch (e) { say(c, `Nothing more was copied. ${errorMessage(e)}`); } finally { busy = ''; }
  }
  const abs = (rel: string) => `${card.replace(/\/$/, '')}/${rel}`;
  async function reviewLibrary(c: CarryCandidate) {
    busy = key(c); say(c, '');
    try { libPlans[key(c)] = await planCoreCopy(abs(c.source_rel), abs(c.dest_rel)); libPlans = libPlans; } catch (e) { say(c, errorMessage(e)); } finally { busy = ''; }
  }
  async function copyLibrary(c: CarryCandidate) {
    const p = libPlans[key(c)]; if (!p) return; busy = key(c);
    try {
      const dir = await getReportsDir(); if (!dir) throw new Error('Choose a reports folder in Settings first: the copy keeps a record there.');
      const r = await executeCoreCopy(abs(c.source_rel), abs(c.dest_rel), p.id, `${dir}/${Date.now()}-core_copy.json`, newJobId());
      delete libPlans[key(c)]; libPlans = libPlans; say(c, `Copied ${plural(r.copied, 'file')}, verified, and rebuilt the index.`);
    } catch (e) { say(c, `Nothing was reported as complete. ${errorMessage(e)}`); } finally { busy = ''; }
  }
  $: actionable = (list ?? []).filter((c) => c.kind !== 'shared');
</script>
<section class="cm" aria-label="Carry media from the previous build">
  <h3>Carry media from the previous build</h3>
  {#if error}<p class="cm-err" role="alert">{error}</p>
  {:else if !list}<p class="cm-note" role="status">Looking at what {older.length === 1 ? 'the older build' : 'the older builds'} kept…</p>
  {:else if !list.length}<p class="cm-note">Nothing to carry: the older builds kept no files of their own.</p>
  {:else}
    <p class="cm-note">Files an older build kept in its own folder are not where {newName || 'the new build'} looks. Each carry is reviewed first and nothing already on the card is overwritten.</p>
    <ul class="cm-list" aria-label="What can be carried">
      {#each list as c (key(c))}
        <li class={c.kind}>
          <div class="cm-main"><strong>{c.kind === 'shared' ? 'Same library' : c.kind === 'library' ? 'Music library' : 'Files'} <small>from {c.from_name}</small></strong>
            <span>{c.note}</span>{#if c.kind !== 'shared'}<small>{plural(c.files, 'file')} · {size(c.bytes)} · <code>{c.source_rel}</code> → <code>{c.dest_rel}</code></small>{/if}</div>
          {#if c.kind === 'folder'}
            {#if plans[key(c)]}
              <div class="cm-plan" role="status"><span>{plans[key(c)].new_files} to copy ({size(plans[key(c)].bytes_to_write)}) · {plans[key(c)].identical} already there{plans[key(c)].different ? ` · ${plans[key(c)].different} different, kept as they are` : ''}</span>
                <button class="primary" disabled={!!busy || !plans[key(c)].new_files} on:click={() => carry(c)}>{busy === key(c) ? 'Copying…' : `Copy ${plural(plans[key(c)].new_files, 'file')}`}</button></div>
            {:else}<button class="quiet" disabled={!!busy} aria-label={`Review carrying files from ${c.from_name}`} on:click={() => review(c)}>Review the copy…</button>{/if}
          {:else if c.kind === 'library'}
            {#if libPlans[key(c)]}
              <div class="cm-plan" role="status"><span>{libPlans[key(c)].new_files} to copy, {libPlans[key(c)].updates} to update ({size(libPlans[key(c)].bytes_to_write)}); the index is rebuilt for the new root</span>
                <button class="primary" disabled={!!busy} on:click={() => copyLibrary(c)}>{busy === key(c) ? 'Copying…' : 'Copy the library'}</button></div>
            {:else}<button class="quiet" disabled={!!busy} aria-label={`Review carrying the library from ${c.from_name}`} on:click={() => reviewLibrary(c)}>Review the copy…</button>{/if}
          {/if}
          {#if msg[key(c)]}<p class="cm-msg" role="status">{msg[key(c)]}</p>{/if}
        </li>
      {/each}
    </ul>
    {#if !actionable.length}<p class="cm-note">Nothing needs copying.</p>{/if}
  {/if}
</section>
<style>
  .cm{margin-top:18px;border-top:1px solid #2c393a;padding-top:12px}.cm h3{margin:0 0 6px;font-size:11px;letter-spacing:.09em;color:#9fb3aa;text-transform:uppercase}
  .cm-note{font-size:12.5px;color:#95a5a4;line-height:1.55;margin:6px 0}.cm-err{color:#ff9b9b;font-size:12.5px}
  .cm-list{list-style:none;margin:8px 0;padding:0;border:1px solid #2c393a;border-radius:10px}.cm-list li{display:grid;gap:8px;padding:11px 14px;border-bottom:1px solid #2c393a}.cm-list li:last-child{border:0}
  .cm-main{display:grid;gap:3px;font-size:12.5px}.cm-main strong{font-size:13.5px}.cm-main strong small,.cm-main>small{color:#8f9e9d;font-weight:400;font-size:12px}.cm-main code{font-size:11px;background:#101617;border-radius:5px;padding:1px 5px}
  .cm-plan{display:flex;gap:12px;align-items:center;justify-content:space-between;flex-wrap:wrap;font-size:12.5px;color:#c7d1d0}.cm-msg{margin:0;font-size:12.5px;color:#c5f7ad}
  li.shared .cm-main span{color:#c5f7ad}
</style>
