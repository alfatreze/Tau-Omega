<script lang="ts">
  // The install review: what will be installed (one line per core), one summary line, one button. Everything else (the files, where the backup goes) is
  // behind "Details". Works for one core or several: pass `zips` to review and install them all at once.
  import { createEventDispatcher } from 'svelte';
  import SettingsMigration from './SettingsMigration.svelte';
  import { planCoreUpdate, executeCoreUpdate, rollbackCoreUpdate, lintPackage } from './tau-api';
  import LintFindings from './LintFindings.svelte';
  import { plural } from './coresFormat';
  import { errorMessage } from './backend';
  import { developerTools, confirmRealCard, roleOf, refreshRole, cardLabel, markInstalled } from './devStore';
  import type { InstallPlan, InstallReport, LintReport, UpdateAssessment, UpdateVerdict } from './types';
  /** The package zip (or several) and the card folder chosen on the page above. */
  export let zipPath = ''; export let cardPath = ''; export let zips: string[] = [];
  /** Developer tools run the core check before an install and block on errors unless overridden. A caller that already shows it (manual add) turns this off. */
  export let lintGate = true;
  /** Errors the caller already showed and the developer overrode (manual add), to be recorded in the run note. */
  export let knownOverrides: string[] = [];

  type Row = { zip: string; plan: InstallPlan | null; error: string; lint: LintReport | null; lintNote: string; down: boolean; report: InstallReport | null; failed: string };
  const dispatch = createEventDispatcher<{ installed: void }>();
  let rows: Row[] = []; let busy = false; let notice = ''; let override = false; let typed = ''; let confirmUndo = false; let undone = 0; let lastKey = '';
  let migrateFrom: string[] = []; let migrateTo: string[] = [];

  $: targets = zips.length ? zips : zipPath.trim() ? [zipPath] : [];
  $: gating = $developerTools && lintGate;
  // Only when asked for (developer tools, Settings > "Ask for the card's name"), installing onto a Real card needs its name typed first.
  $: if ($developerTools && cardPath) void refreshRole(cardPath);
  $: needName = $developerTools && $confirmRealCard && !($roleOf.card === cardPath && $roleOf.role === 'test');
  $: nameOk = !needName || typed.trim().toLowerCase() === cardLabel(cardPath).toLowerCase();
  // the review starts by itself: no separate "review" step to press
  $: { const key = JSON.stringify([targets, cardPath]); if (key !== lastKey && cardPath.trim() && targets.length) { lastKey = key; void review(); } }

  const size = (b: number) => b < 1024 ? `${b} B` : b < 1024 * 1024 * 1024 ? `${(b / 1024 / 1024).toFixed(1)} MB` : `${(b / 1024 / 1024 / 1024).toFixed(2)} GB`;
  const VERDICT: Record<UpdateVerdict, string> = { new_install: 'New', same_build: 'Already installed', update: 'Update', same_date_different_build: 'Different build', older: 'Older build', mismatch: 'Cannot be installed' };
  const nameOf = (c: UpdateAssessment) => c.package.shortname || c.package.core_id;
  const bad = (pair: UpdateAssessment['pair']) => typeof pair === 'object' && ('mismatch' in pair || 'missing_feature' in pair);
  function pairProblem(pair: UpdateAssessment['pair']): string {
    if (typeof pair === 'object' && 'missing_feature' in pair) return 'its firmware needs a bitstream feature this build lacks: the Pocket would show “NO UNIT”.';
    return 'its firmware and bitstream do not match: the Pocket would show a black screen.';
  }

  async function review() {
    busy = true; notice = ''; override = false; undone = 0; confirmUndo = false;
    const keep = new Map(rows.map((r) => [r.zip, r.down]));
    const next: Row[] = targets.map((zip) => ({ zip, plan: null, error: '', lint: null, lintNote: '', down: keep.get(zip) ?? false, report: null, failed: '' }));
    rows = next;
    await Promise.all(next.map(async (r) => {
      if (gating) { try { r.lint = await lintPackage(r.zip); } catch (e) { r.lintNote = `The core check could not run: ${errorMessage(e)}`; } }
      try { r.plan = await planCoreUpdate(r.zip, cardPath, r.down); } catch (e) { r.error = errorMessage(e); }
    }));
    rows = next; busy = false;
  }
  async function toggleDowngrade(r: Row) { r.down = !r.down; rows = rows; busy = true; try { r.plan = await planCoreUpdate(r.zip, cardPath, r.down); r.error = ''; } catch (e) { r.error = errorMessage(e); } busy = false; rows = rows; }

  $: installable = rows.filter((r) => r.plan && !r.plan.refused && !r.plan.nothing_to_do && !r.report);
  $: finished = rows.some((r) => r.report || r.failed);
  $: errors = rows.reduce((n, r) => n + (r.lint?.errors ?? 0), 0);
  $: lintBlocked = gating && errors > 0 && !override;
  $: files = installable.reduce((n, r) => n + (r.plan?.files.new_files ?? 0) + (r.plan?.files.updated_files ?? 0), 0);
  $: bytes = installable.reduce((n, r) => n + (r.plan?.capacity?.bytes_needed ?? 0), 0);
  $: free = installable.find((r) => r.plan?.capacity)?.plan?.capacity?.space.available_bytes ?? null;
  $: fits = installable.every((r) => !r.plan?.capacity || r.plan.capacity.fits) && (free === null || bytes <= free);
  $: replaced = installable.reduce((n, r) => n + (r.plan?.backup.length ?? 0), 0);
  $: cautions = [...new Set(installable.flatMap((r) => r.plan?.cautions ?? []))];
  $: persist = installable.reduce((n, r) => n + (r.plan?.update.persist_changed?.length ?? 0), 0);
  $: title = rows.length === 1 && rows[0].plan?.update.cores.length === 1 ? `Install ${nameOf(rows[0].plan.update.cores[0])} on ${cardLabel(cardPath)}` : `Install ${plural(installable.length || rows.length, 'core')} on ${cardLabel(cardPath)}`;

  async function install() {
    if (!installable.length) return;
    busy = true; notice = ''; migrateFrom = []; migrateTo = []; let done = 0;
    for (const r of installable) {
      if (!r.plan) continue;
      try {
        const report = await executeCoreUpdate(r.zip, cardPath, r.down, r.plan.id);
        r.report = report; done++;
        migrateFrom = [...migrateFrom, ...r.plan.superseded_candidates.filter((id) => !/DEV|TAU_0_/.test(id))]; migrateTo = [...migrateTo, ...r.plan.update.cores.map((c) => c.package.core_id)];
        if ($developerTools) void markInstalled(cardPath, r.zip, report, override && r.lint ? r.lint.findings.filter((f) => f.severity === 'Error').map((f) => `${f.id}: ${f.message}`) : knownOverrides);
      } catch (e) { r.failed = errorMessage(e); break; }
    }
    rows = rows; busy = false;
    if (done) dispatch('installed');
    const stopped = rows.find((r) => r.failed);
    if (stopped) notice = `${done ? `${plural(done, 'core')} installed. ` : ''}Stopped at ${stopped.plan ? nameOf(stopped.plan.update.cores[0]) : stopped.zip}: ${stopped.failed} Nothing was left half-done for that core.`;
  }
  async function undo() {
    busy = true; notice = '';
    try { for (const r of [...rows].reverse()) { if (r.report && !r.report.nothing_to_do) { await rollbackCoreUpdate(cardPath, r.report.backup_dir); undone++; } } confirmUndo = false; }
    catch (e) { notice = `Nothing more was changed. ${errorMessage(e)}`; }
    finally { busy = false; }
  }
  $: reports = rows.filter((r) => r.report && !r.report.nothing_to_do);
  $: checksFailed = reports.reduce((n, r) => n + (r.report?.checks.filter((c) => c.verdict === 'fail').length ?? 0), 0);
</script>

{#if rows.length && !finished}
  <section class="ir" aria-label="Install review">
    <h2>{title}</h2>
    {#if busy && !rows.some((r) => r.plan || r.error)}<p class="ir-sub" role="status">Checking…</p>{/if}
    <ul class="ir-cores" aria-label="Cores to install">
      {#each rows as r (r.zip)}
        <li>
          {#if r.error}<p class="ir-bad" role="alert">{r.zip.split('/').pop()}: {r.error}</p>
          {:else if r.plan}
            {#each r.plan.update.cores as c}
              <div class="ir-line"><strong>{nameOf(c)}</strong><small>{c.package_release ?? c.package.version}</small><span class="chip" class:capable={c.verdict === 'new_install' || c.verdict === 'update'} class:warn={c.verdict === 'older' || c.verdict === 'same_date_different_build'}>{c.verdict === 'update' && c.installed_release ? `Update from ${c.installed_release}` : VERDICT[c.verdict]}</span></div>
              {#if bad(c.pair)}<p class="ir-bad">{nameOf(c)}: {pairProblem(c.pair)}</p>{/if}
              {#if c.verdict === 'older'}<label class="ir-tick"><input type="checkbox" checked={r.down} on:change={() => toggleDowngrade(r)} /> Install this older build anyway</label>{/if}
            {/each}
            {#if r.plan.refused}<p class="ir-bad" role="alert">{r.plan.refused}</p>{/if}
            {#if r.lint && gating && r.lint.findings.length}<details class="ir-lint" open={r.lint.errors > 0}><summary>Core check: {r.lint.errors ? plural(r.lint.errors, 'error') : 'no errors'}{r.lint.warnings ? `, ${plural(r.lint.warnings, 'warning')}` : ''}</summary><LintFindings lint={r.lint} /></details>{/if}
            {#if r.lintNote}<p class="ir-note" role="note">{r.lintNote}</p>{/if}
          {/if}
        </li>
      {/each}
    </ul>

    {#each cautions as caution}<p class="ir-warn" role="note">{caution}</p>{/each}
    {#if persist}<p class="ir-warn">This release changes how {plural(persist, 'saved setting')} {persist === 1 ? 'is' : 'are'} stored; the core may reset {persist === 1 ? 'it' : 'them'}.</p>{/if}
    {#if gating && errors}<label class="ir-tick ir-override"><input type="checkbox" bind:checked={override} /> Install anyway: I know what {errors === 1 ? 'this error means' : 'these errors mean'} <small>(recorded in the run note)</small></label>{/if}

    {#if installable.length}
      <p class="ir-sum" role="status">{plural(files, 'file')}, {size(bytes)}{free !== null ? ` · ${fits ? `fits (${size(free)} free)` : `does not fit (${size(free)} free)`}` : ''} · {replaced ? 'what it replaces is backed up first' : 'nothing on the card is replaced'}</p>
      <div class="ir-go">
        {#if needName}<label class="ir-name">Real card: type <b>{cardLabel(cardPath)}</b> to confirm<input bind:value={typed} autocomplete="off" aria-label="Type the card name to confirm" /></label>{/if}
        <button class="primary" disabled={busy || !nameOk || lintBlocked || !fits} on:click={install}>{busy ? 'Installing…' : installable.length === 1 ? 'Install' : `Install ${plural(installable.length, 'core')}`}</button>
      </div>
      {#if lintBlocked}<p class="ir-bad" role="alert">The core check found {plural(errors, 'error')}. Fix {errors === 1 ? 'it' : 'them'}, or tick Install anyway.</p>{/if}
      <details class="ir-details"><summary>Details</summary>
        {#each installable as r}{#if r.plan}
          <h3>{r.plan.update.cores.map(nameOf).join(', ')}</h3>
          <p class="ir-sub">{r.plan.backup.length ? `${plural(r.plan.backup.length, 'existing file')} copied to a backup folder on this computer first (${size(r.plan.backup_bytes)}), each copy checked.` : 'Nothing existing is replaced, so there is nothing to back up.'} If anything goes wrong the card is put back by itself, and you can undo afterwards.</p>
          {#if r.plan.caches_to_clear.length}<p class="ir-sub">The Pocket’s menu cache is cleared so it rebuilds and lists the core.</p>{/if}
          {#if r.plan.obsolete_to_remove.length}<p class="ir-sub">Files this release no longer uses are removed: {r.plan.obsolete_to_remove.join(', ')}.</p>{/if}
          {#if r.plan.user_files_kept.length}<p class="ir-sub">Kept as they are: {r.plan.user_files_kept.join(', ')}.</p>{/if}
          {#if r.plan.superseded_candidates.length}<p class="ir-sub">Older test builds ({r.plan.superseded_candidates.join(', ')}) are left alone.</p>{/if}
          <ul class="ir-files" aria-label="Files">{#each r.plan.files.items as item}<li class={item.state}><span>{item.state === 'only_left' ? 'New' : item.state === 'different' ? 'Replace' : 'Same'}</span><span>{item.path}</span><span>{size(item.bytes)}</span></li>{/each}</ul>
        {/if}{/each}
      </details>
    {:else if !busy && rows.every((r) => r.plan?.nothing_to_do)}<p class="ir-ok" role="status">{rows.length === 1 ? 'The card already has exactly this build.' : 'The card already has all of these builds.'}</p>{/if}
    {#if notice}<p class="ir-bad" role="alert">{notice}</p>{/if}
  </section>
{/if}

{#if finished}
  <section class="ir ir-done" aria-label="Install result">
    <h2>{reports.length === 0 ? 'Nothing was installed' : checksFailed ? `Installed ${plural(reports.length, 'core')}, but ${plural(checksFailed, 'check')} failed` : `Installed ${plural(reports.length, 'core')}`}</h2>
    <ul class="ir-cores" aria-label="Installed cores">
      {#each rows.filter((r) => r.report || r.failed) as r}
        <li><div class="ir-line"><strong>{r.plan ? r.plan.update.cores.map(nameOf).join(', ') : r.zip.split('/').pop()}</strong>
          {#if r.failed}<span class="chip warn">Not installed</span>{:else if r.report?.nothing_to_do}<span class="chip">Nothing changed</span>{:else if r.report?.checks.every((c) => c.verdict !== 'fail')}<span class="chip capable">Installed and checked</span>{:else}<span class="chip warn">A check failed</span>{/if}</div>
          {#if r.failed}<p class="ir-bad">{r.failed}</p>{/if}</li>
      {/each}
    </ul>
    {#if notice}<p class="ir-bad" role="alert">{notice}</p>{/if}
    <details class="ir-details"><summary>Details</summary>
      {#each reports as r}{#if r.report}
        <h3>{r.plan?.update.cores.map(nameOf).join(', ')}</h3>
        <p class="ir-sub">{r.report.files_written} files written · {size(r.report.bytes_written)} · {r.report.files_backed_up} backed up. Backup and journal: <code>{r.report.backup_dir}</code></p>
        <ul class="ir-files" aria-label="Checks">{#each r.report.checks as check}{#each check.items as item}<li class={item.status === 'pass' ? 'identical' : item.status === 'warn' ? 'different' : 'only_right'}><span>{item.status === 'pass' ? 'Pass' : item.status === 'warn' ? 'Note' : 'Fail'}</span><span>{item.name}</span><span>{item.detail}</span></li>{/each}{/each}</ul>
      {/if}{/each}
    </details>
    {#if reports.length && !undone}
      {#if !confirmUndo}<button class="quiet" disabled={busy} on:click={() => (confirmUndo = true)}>Undo…</button>
      {:else}<span role="alert">Put the card back as it was before {reports.length === 1 ? 'this install' : 'these installs'}? <button class="danger" disabled={busy} on:click={undo}>Yes, undo</button> <button class="quiet" on:click={() => (confirmUndo = false)}>Keep {reports.length === 1 ? 'it' : 'them'}</button></span>{/if}
    {/if}
    {#if undone}<p class="ir-ok" role="status">Undone: {plural(undone, 'install')} put back.</p>{/if}
  </section>
  {#if reports.length && !undone}<SettingsMigration {cardPath} fromCores={migrateFrom} toCores={migrateTo} />{/if}
{/if}
<style>
  .ir{border:1px solid #2c393a;border-radius:12px;background:#161f20;padding:16px 18px;margin:14px 0;max-width:720px}.ir h2{margin:0 0 10px;font-size:16px}.ir h3{margin:12px 0 4px;font-size:13px}
  .ir-cores{list-style:none;margin:0 0 10px;padding:0}.ir-cores li{padding:9px 0;border-bottom:1px solid #263234}.ir-cores li:last-child{border:0}
  .ir-line{display:flex;gap:10px;align-items:baseline;flex-wrap:wrap}.ir-line strong{font-size:14px}.ir-line small{color:#8f9e9d;font-size:12px}.ir-line .chip{margin-left:auto}
  .chip.warn{background:#3a2e14;color:#e5d9a8}
  .ir-sub{font-size:12.5px;color:#a6b3b2;line-height:1.55;margin:4px 0}.ir-sum{font-size:13px;color:#dce6e4;margin:10px 0}
  .ir-warn{font-size:12.5px;color:#e5d9a8;margin:6px 0}.ir-bad{font-size:12.5px;color:#ff9b9b;margin:6px 0}.ir-ok{font-size:13px;color:#c5f7ad;margin:8px 0}.ir-note{font-size:12px;color:#95a5a4}
  .ir-tick{display:flex;gap:8px;align-items:center;font-size:12.5px;color:#dce6e4;margin:6px 0}.ir-tick small{color:#8f9e9d}.ir-override{border:1px solid #5a3030;background:#241414;border-radius:9px;padding:8px 11px}
  .ir-go{display:flex;gap:14px;align-items:flex-end;flex-wrap:wrap;margin:10px 0}.ir-name{display:grid;gap:5px;font-size:12px;color:#a6b3b2}.ir-name input{font:inherit;font-size:13px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:8px 10px;width:180px}
  .ir-details{margin-top:12px}.ir-details summary,.ir-lint summary{cursor:pointer;font-size:12.5px;color:#a6b3b2}
  .ir-files{list-style:none;margin:6px 0;padding:0;border:1px solid #2c393a;border-radius:9px;max-height:260px;overflow:auto}
  .ir-files li{display:grid;grid-template-columns:60px minmax(0,1fr) auto;gap:10px;padding:7px 11px;border-bottom:1px solid #263234;font-size:12px;color:#c7d1d0}.ir-files li:last-child{border:0}.ir-files li>span:last-child{color:#8f9e9d;text-align:right}
  .ir-files li.only_left>span:first-child{color:#b9e9a5}.ir-files li.different>span:first-child{color:#d9c47e}.ir-files li.only_right>span:first-child{color:#f29b83}
  button.danger{padding:8px 12px;border-radius:8px;background:#5a352c;color:#f4cfc4;font-weight:700}
</style>
