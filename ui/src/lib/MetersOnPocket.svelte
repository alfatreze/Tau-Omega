<script lang="ts">
  // Meters on the Pocket: shows which list each Tau core uses, and (for Tau 0.7.5+ cores that read tau-meters.bin)
  // lets the user reorder, hide/show, remove, reset, or install a .tmeter package through a reviewed plan and verified backup.
  import { open as pickFile } from '@tauri-apps/plugin-dialog';
  import {
    errorMessage,
    meterBinExecuteEdit,
    meterBinExecuteInstall,
    meterBinExecuteReset,
    meterBinPlanEdit,
    meterBinPlanInstall,
    meterBinPlanReset,
    meterPackageInspect,
    metersOnCard,
  } from './tau-api';
  import type {
    CoreMeters,
    MeterBinPlan,
    MeterBinView,
    MeterPackageSummary,
    MeterRowView,
    MetersOnCard,
  } from './types';

  /** The card folder currently open (empty when none). */
  export let card = '';

  let data: MetersOnCard | null = null;
  let loading = false;
  let failed = false;
  let askedFor = '';

  // Composer / installer state
  let editing = false;
  let draftRows: MeterRowView[] = [];
  let pkgPath = '';
  let pkgSummary: MeterPackageSummary | null = null;
  let allowSourceChange = false;
  let overwritePresets = false;
  let plan: MeterBinPlan | null = null;
  let planKind: 'edit' | 'install' | 'reset' | null = null;
  let busy = false;
  let actionError = '';
  let actionDone = '';

  $: if (card && card !== askedFor) {
    askedFor = card;
    clearWorkflow();
    void load();
  }
  $: if (!card) {
    data = null;
    askedFor = '';
    clearWorkflow();
  }

  function clearWorkflow() {
    editing = false;
    draftRows = [];
    pkgPath = '';
    pkgSummary = null;
    allowSourceChange = false;
    overwritePresets = false;
    plan = null;
    planKind = null;
    actionError = '';
  }

  async function load() {
    loading = true;
    failed = false;
    try {
      data = await metersOnCard(card);
    } catch {
      data = null;
      failed = true;
    } finally {
      loading = false;
    }
  }

  const listFor = (c: CoreMeters, d: MetersOnCard): MeterBinView | null =>
    c.uses === 'user_file' ? d.user_file : c.uses === 'own_bundle' ? c.bundle : null;
  const usesText = (c: CoreMeters) =>
    c.uses === 'user_file'
      ? 'Uses your meter file'
      : c.uses === 'own_bundle'
        ? 'Uses the meters that come with it'
        : 'Only the built-in Winamp Bars';
  const shown = (l: MeterBinView | null) => (l ? l.rows.filter((r) => r.visible) : []);
  const hidden = (l: MeterBinView | null) => (l ? l.rows.filter((r) => !r.visible).length : 0);
  const note = (r: MeterRowView) =>
    [
      r.fallback ? 'Built into the Pocket: always on' : '',
      r.unavailable ? `Greyed out: ${r.unavailable}` : '',
      r.tuning_lost ? 'Cannot remember tuned settings: another meter has a similar id' : '',
    ].filter(Boolean);

  $: editableSource = (() => {
    if (!data) return null;
    const cap = data.cores.find((c) => c.reads_user_file);
    if (!cap) return null;
    if (data.user_file && data.user_file.format === 2 && !data.user_file.problem) {
      return data.user_file;
    }
    if (cap.bundle && cap.bundle.format === 2 && !cap.bundle.problem) {
      return cap.bundle;
    }
    return null;
  })();

  function startEdit() {
    if (!editableSource) return;
    clearWorkflow();
    actionDone = '';
    draftRows = editableSource.rows.map((r) => ({ ...r }));
    editing = true;
  }

  function moveRow(index: number, delta: number) {
    const next = index + delta;
    if (next < 0 || next >= draftRows.length) return;
    const copy = [...draftRows];
    const [item] = copy.splice(index, 1);
    copy.splice(next, 0, item);
    draftRows = copy;
  }

  function toggleVisible(index: number) {
    const r = draftRows[index];
    if (!r || r.fallback) return;
    draftRows = draftRows.map((row, i) => (i === index ? { ...row, visible: !row.visible } : row));
  }

  function removeRow(index: number) {
    const r = draftRows[index];
    if (!r || r.fallback) return;
    draftRows = draftRows.filter((_, i) => i !== index);
  }

  async function reviewEdit() {
    busy = true;
    actionError = '';
    try {
      const rows = draftRows.map((r) => ({ id: r.id, visible: r.visible }));
      plan = await meterBinPlanEdit(card, rows);
      planKind = 'edit';
    } catch (e) {
      actionError = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function choosePackage() {
    actionError = '';
    actionDone = '';
    try {
      let chosen: string | null = null;
      try {
        const res = await pickFile({
          multiple: false,
          filters: [{ name: 'Tau Meter Package', extensions: ['tmeter'] }],
        });
        if (typeof res === 'string') chosen = res;
      } catch {
        // Fallback when running in browser dev mock without Tauri dialog plugin.
        chosen = 'mock-lissajous.tmeter';
      }
      if (!chosen) return;
      busy = true;
      clearWorkflow();
      pkgPath = chosen;
      pkgSummary = await meterPackageInspect(chosen);
    } catch (e) {
      actionError = errorMessage(e);
      pkgPath = '';
      pkgSummary = null;
    } finally {
      busy = false;
    }
  }

  async function reviewInstall() {
    if (!pkgPath) return;
    busy = true;
    actionError = '';
    try {
      plan = await meterBinPlanInstall(card, pkgPath, allowSourceChange, overwritePresets);
      planKind = 'install';
    } catch (e) {
      actionError = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function reviewReset() {
    clearWorkflow();
    actionDone = '';
    busy = true;
    try {
      plan = await meterBinPlanReset(card);
      planKind = 'reset';
    } catch (e) {
      actionError = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function confirmWrite() {
    if (!plan || !planKind) return;
    busy = true;
    actionError = '';
    try {
      if (planKind === 'edit') {
        const rows = draftRows.map((r) => ({ id: r.id, visible: r.visible }));
        const rep = await meterBinExecuteEdit(card, rows, plan.id);
        actionDone = `Saved ${rep.rel_path} (${rep.bytes_written.toLocaleString()} bytes).`;
      } else if (planKind === 'install') {
        const rep = await meterBinExecuteInstall(
          card,
          pkgPath,
          allowSourceChange,
          overwritePresets,
          plan.id,
        );
        actionDone =
          rep.presets_written > 0
            ? `Installed meter into ${rep.rel_path} and copied ${rep.presets_written} preset files.`
            : `Installed meter into ${rep.rel_path}.`;
      } else if (planKind === 'reset') {
        await meterBinExecuteReset(card, plan.id);
        actionDone = 'Removed your custom meter file. Your Pocket will use the core’s shipped meters.';
      }
      clearWorkflow();
      await load();
    } catch (e) {
      actionError = errorMessage(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="ml" aria-labelledby="ml-title" data-testid="meters-on-pocket">
  <header class="ml-head">
    <h2 id="ml-title">Meters on the Pocket</h2>
    <p class="ml-sub">
      The meters your Pocket can show, in its order, read from your card.
      {#if editableSource}
        You can reorder, hide, or install a <code>.tmeter</code> package; every change is reviewed and backed up first.
      {:else}
        Nothing here changes the card unless a Tau 0.7.5 or newer core is installed.
      {/if}
    </p>
  </header>

  {#if !card}
    <p class="ml-note" role="status">Open a card to see its meters.</p>
  {:else if loading && !data}
    <p class="ml-note" role="status" aria-busy="true">Reading the card…</p>
  {:else if failed || !data}
    <p class="ml-note" role="alert">The meters could not be read from this card.</p>
  {:else if !data.cores.length}
    <p class="ml-note" role="status" data-testid="mop-none">No Tau core is on this card, so there are no meters to show.</p>
  {:else}
    {#if editableSource}
      <div class="ml-actions" data-testid="mop-actions">
        <button type="button" class="btn-primary" on:click={choosePackage} disabled={busy} data-testid="mop-install-btn">
          Install a meter package…
        </button>
        <button type="button" class="btn" on:click={startEdit} disabled={busy || editing} data-testid="mop-edit-btn">
          Edit meter list
        </button>
        {#if data.user_file}
          <button type="button" class="btn" on:click={reviewReset} disabled={busy} data-testid="mop-reset-btn">
            Reset to shipped meters…
          </button>
        {/if}
      </div>
    {/if}

    {#if actionError}
      <p class="ml-err" role="alert" data-testid="mop-error">{actionError}</p>
    {/if}
    {#if actionDone}
      <p class="ml-ok" role="status" data-testid="mop-done">{actionDone}</p>
    {/if}

    {#if pkgSummary && !plan}
      <section class="ml-panel" aria-label="Meter package details" data-testid="mop-pkg-review">
        <div class="ml-panel-head">
          <h3>{pkgSummary.name} <small>version {pkgSummary.version}</small></h3>
          <span class="ml-badge">{pkgSummary.trust_label}</span>
        </div>
        <p class="ml-warn" role="note">{pkgSummary.warning_notice}</p>
        <dl class="ml-meta">
          <dt>Author</dt><dd>{pkgSummary.author}</dd>
          <dt>Licence</dt><dd>{pkgSummary.licence}</dd>
          <dt>Source</dt><dd>{pkgSummary.binary_only ? 'Binary only (no source in package)' : 'Source included'}</dd>
          <dt>Factory presets</dt><dd>{pkgSummary.preset_count}</dd>
          <dt>Package SHA-256</dt><dd><code>{pkgSummary.package_sha256}</code></dd>
          <dt>Meter pack SHA-256</dt><dd><code>{pkgSummary.pack_sha256}</code></dd>
        </dl>
        {#if pkgSummary.preset_count > 0}
          <label class="ml-check">
            <input type="checkbox" bind:checked={overwritePresets} />
            Replace existing presets in <code>tau-presets/{pkgSummary.id_hash_hex}/</code> if already present
          </label>
        {/if}
        <div class="ml-panel-actions">
          <button type="button" class="btn-primary" on:click={reviewInstall} disabled={busy} data-testid="mop-plan-install-btn">
            Review install plan…
          </button>
          <button type="button" class="btn" on:click={clearWorkflow} disabled={busy}>Cancel</button>
        </div>
      </section>
    {/if}

    {#if editing && !plan}
      <section class="ml-panel" aria-label="Edit meter list" data-testid="mop-editor">
        <h3>Edit meter order and visibility</h3>
        <div class="ml-edit-rows">
          {#each draftRows as r, i (r.id + i)}
            <div class="ml-edit-row" class:dim={!r.visible}>
              <span class="ml-n">{i + 1}</span>
              <span class="ml-name">
                <b>{r.name}</b>
                {#if r.version}<small>version {r.version}</small>{/if}
                {#if r.fallback}<small class="ml-flag">Built-in fallback</small>{/if}
                {#if !r.visible}<small class="ml-flag">Hidden</small>{/if}
              </span>
              <span class="ml-row-btns">
                <button type="button" class="btn-sm" on:click={() => moveRow(i, -1)} disabled={i === 0} aria-label="Move {r.name} up">↑</button>
                <button type="button" class="btn-sm" on:click={() => moveRow(i, 1)} disabled={i === draftRows.length - 1} aria-label="Move {r.name} down">↓</button>
                <button type="button" class="btn-sm" on:click={() => toggleVisible(i)} disabled={r.fallback}>
                  {r.visible ? 'Hide' : 'Show'}
                </button>
                <button type="button" class="btn-sm" on:click={() => removeRow(i)} disabled={r.fallback}>
                  Remove
                </button>
              </span>
            </div>
          {/each}
        </div>
        <div class="ml-panel-actions">
          <button type="button" class="btn-primary" on:click={reviewEdit} disabled={busy} data-testid="mop-review-edit-btn">
            Review changes…
          </button>
          <button type="button" class="btn" on:click={clearWorkflow} disabled={busy}>Cancel</button>
        </div>
      </section>
    {/if}

    {#if plan}
      <section class="ml-panel" aria-label="Review meter write plan" data-testid="mop-plan">
        <h3>Review before writing to card</h3>
        <p class="ml-uses">
          {#if plan.action_kind === 'reset'}
            Removes <code>{plan.rel_path}</code> so the Pocket uses each core’s shipped meters.
          {:else}
            Writes <code>{plan.rel_path}</code> ({plan.resulting_bytes.toLocaleString()} of {plan.max_bytes.toLocaleString()} bytes).
          {/if}
        </p>
        {#if plan.preset_files_to_write.length > 0 && plan.preset_dir_rel}
          <p class="ml-small">Copies {plan.preset_files_to_write.length} preset files into <code>{plan.preset_dir_rel}/</code>.</p>
        {:else if plan.preset_skipped_existing && plan.preset_dir_rel}
          <p class="ml-small">Keeps your existing presets in <code>{plan.preset_dir_rel}/</code> untouched.</p>
        {/if}
        {#if plan.backup_files.length > 0}
          <p class="ml-small">Backs up {plan.backup_files.length} existing file(s) outside the card and keeps <code>tau-meters.bin.prev</code> before swapping.</p>
        {/if}
        {#each plan.warnings as w}
          <p class="ml-warn" role="alert">{w}</p>
        {/each}
        {#if plan.same_id_different_source}
          <label class="ml-check">
            <input type="checkbox" bind:checked={allowSourceChange} />
            I understand this package has the same id from a different source and want to replace it.
          </label>
        {/if}
        <div class="ml-panel-actions">
          <button
            type="button"
            class="btn-primary"
            on:click={confirmWrite}
            disabled={busy || (plan.same_id_different_source && !allowSourceChange)}
            data-testid="mop-confirm-write-btn"
          >
            {busy ? 'Writing…' : 'Write to card'}
          </button>
          <button type="button" class="btn" on:click={clearWorkflow} disabled={busy}>Cancel</button>
        </div>
      </section>
    {/if}

    {#each data.cores as c (c.core)}
      {@const list = listFor(c, data)}
      <article class="ml-core" data-testid="mop-core">
        <h3>{c.core}{#if c.version}{' '}<small>{c.version}</small>{/if}</h3>
        <p class="ml-uses"><strong>{usesText(c)}.</strong>
          {#if data.user_file && !c.reads_user_file} This version of Tau does not read a meter file yet (it needs Tau 0.7.5 or newer), so the file on your card is not used.{/if}
          {#if data.user_file?.problem && c.reads_user_file} The Pocket ignores your meter file: {data.user_file.problem}{/if}
          {#if c.uses === 'built_in_only' && !data.user_file && !c.bundle} No meter file is on the card.{/if}
        </p>
        {#if list}
          <ol class="ml-list" aria-label="Meters of {c.core}">
            {#each shown(list) as r, i (r.name + i)}
              <li class:off={!!r.unavailable}>
                <span class="ml-n" aria-hidden="true">{i + 1}</span>
                <span class="ml-name"><b>{r.name}</b>{#if r.version}<small>version {r.version}</small>{/if}
                  {#each note(r) as n}<small class="ml-flag">{n}</small>{/each}</span>
              </li>
            {/each}
          </ol>
          {#if hidden(list)}<p class="ml-small" data-testid="mop-hidden">{hidden(list)} more {hidden(list) === 1 ? 'meter is' : 'meters are'} in the file but hidden from the Pocket's list.</p>{/if}
          <p class="ml-small">From <code>{list.rel_path}</code></p>
        {/if}
      </article>
    {/each}
  {/if}
</section>

<style>
  .ml{display:grid;gap:14px;max-width:760px}
  .ml-head{display:block;margin:0}.ml h2{margin:0 0 4px;font-size:18px}.ml-sub{margin:0;color:var(--text-secondary);font-size:13px;max-width:62ch;line-height:1.5}
  .ml-note{margin:0;color:var(--text-secondary);font-size:13px}
  .ml-actions{display:flex;flex-wrap:wrap;gap:8px}
  .ml-panel{display:grid;gap:10px;padding:14px 16px;border:1px solid var(--border-strong);border-radius:12px;background:var(--bg-card)}
  .ml-panel h3{margin:0;font-size:15px}.ml-panel h3 small{color:var(--text-muted);font-weight:500;font-size:12px}
  .ml-panel-head{display:flex;align-items:center;justify-content:space-between;gap:10px;flex-wrap:wrap}
  .ml-badge{font-size:11.5px;padding:3px 8px;border-radius:99px;border:1px solid var(--warn-border);color:var(--warn-text);background:var(--warn-bg)}
  .ml-warn{margin:0;padding:8px 10px;border-radius:8px;background:var(--warn-bg);border:1px solid var(--warn-border);color:var(--warn-text);font-size:12.5px}
  .ml-err{margin:0;padding:8px 10px;border-radius:8px;background:var(--danger-bg);border:1px solid var(--danger-border);color:var(--danger-text);font-size:12.5px}
  .ml-ok{margin:0;padding:8px 10px;border-radius:8px;background:var(--accent-soft-bg);border:1px solid var(--accent-soft-border);color:var(--accent);font-size:12.5px}
  .ml-meta{display:grid;grid-template-columns:max-content 1fr;gap:4px 12px;margin:0;font-size:12.5px}
  .ml-meta dt{color:var(--text-muted)}.ml-meta dd{margin:0;color:var(--text);overflow-wrap:anywhere}
  .ml-check{display:flex;align-items:center;gap:8px;font-size:12.5px;color:var(--text-secondary)}
  .ml-edit-rows{display:grid;gap:6px}
  .ml-edit-row{display:flex;align-items:center;gap:10px;padding:7px 10px;border:1px solid var(--border);border-radius:10px;background:var(--bg-card)}
  .ml-edit-row.dim{opacity:.65}
  .ml-row-btns{margin-left:auto;display:flex;gap:6px;flex-wrap:wrap}
  .ml-panel-actions{display:flex;gap:8px;flex-wrap:wrap}
  .btn,.btn-primary,.btn-sm{font:inherit;border-radius:8px;cursor:pointer}
  .btn{padding:6px 12px;font-size:13px;background:var(--bg-card);color:var(--text);border:1px solid var(--border-strong)}
  .btn-primary{padding:6px 12px;font-size:13px;background:var(--bg-btn);color:var(--accent);border:1px solid var(--border-strong)}
  .btn-sm{padding:3px 8px;font-size:12px;background:var(--bg-hover);color:var(--text);border:1px solid var(--border-strong)}
  button:disabled{opacity:.45;cursor:not-allowed}
  .ml-core{display:grid;gap:8px;padding:14px 16px;border:1px solid var(--border);border-radius:12px;background:var(--bg-surface)}
  .ml-core h3{margin:0;font-size:15px}.ml-core h3 small{color:var(--text-muted);font-weight:500;font-size:12px}
  .ml-uses{margin:0;color:var(--text-secondary);font-size:13px;line-height:1.55}
  .ml-list{list-style:none;margin:0;padding:0;display:grid;gap:6px}
  .ml-list li{display:flex;align-items:center;gap:12px;padding:8px 12px;border:1px solid var(--border);border-radius:10px;background:var(--bg-card)}
  .ml-list li.off{opacity:.6}
  .ml-n{flex:none;width:22px;height:22px;border-radius:99px;background:var(--bg-hover);color:var(--text-secondary);font-size:11.5px;display:grid;place-items:center}
  .ml-name{display:flex;flex-wrap:wrap;align-items:baseline;gap:2px 10px;min-width:0}.ml-name b{font-size:13.5px}.ml-name small{font-size:11.5px;color:var(--text-muted)}.ml-name .ml-flag{color:var(--warn-text)}
  .ml-small{margin:0;font-size:12px;color:var(--text-muted)}code{font-size:11.5px;overflow-wrap:anywhere}
</style>
