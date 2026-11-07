<script lang="ts">
  // Meter presets: which presets each meter has on the card, as files (`tau-presets/<meter id>/index.tpi` and `p<id>.tpr`).
  // DEFAULT (every parameter at its default) is built into every meter and is never a file. The engine owns every rule
  // (ids, names, ranges, hash collisions, the card write); this page edits a working copy and shows what the engine says.
  // Nothing is written until the reviewed plan is confirmed, and the card keeps a backup outside it.
  import { onMount } from 'svelte';
  import { getPrefs, meterPresetsInstall, meterPresetsPlan, meterPresetsRead, errorMessage } from './tau-api';
  import { modal } from './a11y';
  import { defaultOf, isColour, loadRegistry, rangeOf, visible, type MeterDef, type ParamDef, type Registry } from './meters/registry';
  import type { MeterPresetsView, PresetInstallPlan, SetPreset } from './types';
  import { labTarget } from './meters/labBus';
  import RealMeterLab from './RealMeterLab.svelte';

  export let mediaRoot = '';
  export let cardLabel = '';

  const MAX_PRESETS = 127, MAX_ID = 126, NAME_MAX = 15;

  let registry: Registry | null = null;
  let loading = true;
  let views: MeterPresetsView[] = [];
  let work: Record<number, SetPreset[]> = {};
  let baseline: Record<number, string> = {};
  let selMeter = 0;
  let selId: number | null = null;
  let notice = ''; let problem = ''; let busy = false;
  let confirmDelete = false;
  let query = '';
  let moreOpen = false; let confirmReplace = false;

  const clone = <T,>(x: T): T => JSON.parse(JSON.stringify(x));
  const same = (a: Record<string, number>, b: Record<string, number>) => JSON.stringify(Object.entries(a).sort()) === JSON.stringify(Object.entries(b).sort());

  $: meter = registry?.meters.find((m) => m.id === selMeter) as MeterDef | undefined;
  $: view = views.find((v) => v.meter_id === selMeter);
  $: list = work[selMeter] ?? [];
  $: preset = list.find((p) => p.id === selId) as SetPreset | undefined;
  $: dirtyIds = Object.keys(work).map(Number).filter((id) => JSON.stringify(work[id]) !== baseline[id]);
  $: dirty = dirtyIds.length > 0;
  // The lab shows what is being edited (a sparse set: what the preset does not name is the meter's default, as on the Pocket).
  $: labTarget.set(meter ? { meter: meter.id, values: preset ? { ...preset.values } : {} } : null);

  async function load() {
    loading = true; problem = '';
    try {
      registry = await loadRegistry();
      if (!registry) return;
      if (!registry.meters.some((m) => m.id === selMeter) && registry.meters[0]) selMeter = registry.meters[0].id;
      if (!mediaRoot) { views = []; return; }
      views = await meterPresetsRead(registry.json, mediaRoot);
      work = {}; baseline = {};
      for (const v of views) {
        // What is on the card, then what the owner hid (kept in Omega only; one whose id the card now uses for another preset is left out).
        const onCard = clone(v.on_card?.presets ?? []);
        work[v.meter_id] = [...onCard, ...clone(v.hidden).filter((h) => !onCard.some((p) => p.id === h.id))];
        baseline[v.meter_id] = JSON.stringify(work[v.meter_id]);
      }
      if (!registry.meters.some((m) => m.id === selMeter) && registry.meters[0]) selMeter = registry.meters[0].id;
      selId = work[selMeter]?.[0]?.id ?? null;
    } catch (error) { problem = errorMessage(error); } finally { loading = false; }
  }
  onMount(load);
  let lastRoot = mediaRoot;
  $: if (mediaRoot !== lastRoot) { lastRoot = mediaRoot; load(); }

  function pickMeter(id: number) { selMeter = id; selId = work[id]?.[0]?.id ?? null; confirmDelete = false; notice = ''; query = ''; }
  // A meter holds up to 127 presets: the list is a bounded, filterable area, never a wall that pushes the editor away.
  $: needle = query.trim().toLowerCase();
  $: shownList = needle ? list.filter((p) => p.name.toLowerCase().includes(needle) || String(p.id) === needle) : list;
  // Which presets go to the card: a preset that is not "on the Pocket" is kept in Omega only (the Pocket has no hidden presets), with its id reserved.
  $: missingFactory = (view?.factory.presets ?? []).filter((f) => !list.some((p) => p.id === f.id)).length;
  $: onCount = list.filter((p) => !p.hidden).length;
  $: offCount = list.length - onCount;
  function toggleOn(p: SetPreset) { p.hidden = !p.hidden; touch(); }
  /** Puts every shown preset on the Pocket, or keeps every shown one off (what the filter shows, so a search narrows a bulk change). */
  function setShownOn(on: boolean) {
    const n = shownList.length;
    for (const p of shownList) p.hidden = !on;
    touch();
    const what = needle ? `The ${n} shown preset${n === 1 ? '' : 's'}` : `All ${n} preset${n === 1 ? '' : 's'}`;
    notice = on ? `${what} will go on the Pocket.` : `${what} will stay in Omega and will not be put on the Pocket.`;
  }
  /** Keeps the selected row in view inside the scrolling list. */
  function reveal(node: HTMLElement, selected: boolean) {
    const go = (on: boolean) => { if (on) node.scrollIntoView({ block: 'nearest' }); };
    go(selected);
    return { update: go };
  }
  /** Up and Down walk the visible rows (DEFAULT first), as in any list. */
  function walk(e: KeyboardEvent) {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    if ((e.target as HTMLElement)?.closest('input')) return;
    const ids: (number | null)[] = [null, ...shownList.map((p) => p.id)];
    const at = Math.max(0, ids.indexOf(selId));
    const next = ids[Math.min(ids.length - 1, Math.max(0, at + (e.key === 'ArrowDown' ? 1 : -1)))];
    if (next === selId) return;
    e.preventDefault(); selId = next; confirmDelete = false;
    queueMicrotask(() => document.querySelector<HTMLElement>('.mp-presets li.on .mp-pick')?.focus());
  }
  function touch() { work = work; notice = ''; confirmDelete = false; }

  const isFactory = (p: SetPreset) => {
    const f = view?.factory.presets.find((x) => x.id === p.id);
    return !!f && f.name === p.name && same(f.values, p.values);
  };
  const isEditedFactory = (p: SetPreset) => !!view?.factory.presets.some((x) => x.id === p.id) && !isFactory(p);

  /** The smallest id that is not in the list and was never used by a preset that was deleted (the Pocket remembers a choice by id). */
  function freeId(): number | null {
    const taken = new Set([...list.map((p) => p.id), ...(view?.retired ?? [])]);
    for (let i = 0; i <= MAX_ID; i++) if (!taken.has(i)) return i;
    return null;
  }
  function uniqueName(base: string) {
    let name = base.slice(0, NAME_MAX); let n = 1;
    while (list.some((p) => p.name === name)) name = `${base.slice(0, NAME_MAX - 3)} ${++n}`;
    return name;
  }
  function add(from?: SetPreset) {
    if (list.length >= MAX_PRESETS) { notice = `A meter holds up to ${MAX_PRESETS} presets.`; return; }
    const id = freeId();
    if (id === null) { notice = 'Every preset number for this meter has been used, and a number is never used twice, so no new preset can be added on this card.'; return; }
    const p: SetPreset = { id, name: uniqueName(from ? `${from.name} COPY` : 'MY PRESET'), hidden: false, values: clone(from?.values ?? {}) };
    work[selMeter] = [...list, p]; selId = id; touch();
  }
  function remove() {
    if (!preset) return;
    const at = list.findIndex((p) => p.id === preset!.id);
    work[selMeter] = list.filter((p) => p.id !== preset!.id);
    selId = work[selMeter][Math.min(at, work[selMeter].length - 1)]?.id ?? null; touch();
  }
  function move(delta: number) {
    if (!preset) return;
    const i = list.findIndex((p) => p.id === preset!.id), j = i + delta;
    if (j < 0 || j >= list.length) return;
    const next = [...list]; [next[i], next[j]] = [next[j], next[i]]; work[selMeter] = next; touch();
  }
  function setName(value: string) { if (preset) { preset.name = value.toUpperCase(); touch(); } }
  function toggleHidden() { if (preset) { preset.hidden = !preset.hidden; touch(); } }
  const nameProblem = (name: string) =>
    name.length < 1 || name.length > NAME_MAX ? `A name is 1 to ${NAME_MAX} characters.`
    : /[^\x20-\x7E]/.test(name) ? 'Use plain letters, numbers and punctuation (no accents).' : '';

  /** Adds the factory presets the list does not have; one whose id is taken by another preset is skipped and said so. */
  function addFactory() {
    if (!view) return;
    let added = 0, skipped = 0;
    const next = [...list];
    for (const f of view.factory.presets) {
      if (next.some((p) => p.id === f.id)) { if (!next.some((p) => p.id === f.id && p.name === f.name)) skipped++; continue; }
      next.push(clone(f)); added++;
    }
    work[selMeter] = next; selId = selId ?? next[0]?.id ?? null;
    notice = added ? `Added ${added} factory preset${added === 1 ? '' : 's'}.${skipped ? ` ${skipped} not added: another preset already has their number.` : ''}` : 'Every factory preset is already in the list.';
    touch(); if (added) notice = notice;
  }
  function resetToFactory() {
    if (!view) return;
    moreOpen = false; confirmReplace = false;
    work[selMeter] = clone(view.factory.presets); selId = work[selMeter][0]?.id ?? null; touch();
    notice = 'The list now has only the factory presets. Nothing is saved to the card until you press Save.';
  }
  function revert() {
    work[selMeter] = JSON.parse(baseline[selMeter] ?? '[]'); selId = work[selMeter][0]?.id ?? null; touch();
  }

  // ---- parameters ----
  const valueOf = (p: ParamDef) => preset?.values[p.key] ?? defaultOf(p);
  const isSet = (p: ParamDef) => !!preset && p.key in preset.values;
  function setValue(p: ParamDef, raw: number) {
    if (!preset || Number.isNaN(raw)) return;
    const [lo, hi] = rangeOf(p);
    preset.values[p.key] = Math.min(hi, Math.max(lo, Math.round(raw))); touch();
  }
  function unset(p: ParamDef) { if (preset) { delete preset.values[p.key]; touch(); } }
  const shown = (m: MeterDef | undefined, p: SetPreset | undefined) => (m?.params ?? []).filter((q) => p && visible(q, p.values, m!.params));
  $: groups = (() => {
    const out: { name: string; params: ParamDef[] }[] = [];
    for (const q of shown(meter, preset)) {
      const g = q.experimental ? 'Experimental' : q.group || 'general';
      let slot = out.find((x) => x.name === g);
      if (!slot) { slot = { name: g, params: [] }; out.push(slot); }
      slot.params.push(q);
    }
    return out.sort((a, b) => Number(a.name === 'Experimental') - Number(b.name === 'Experimental'));
  })();
  const hex565 = (v: number) => { const r = (v >> 11) & 31, g = (v >> 5) & 63, b = v & 31; const h = (n: number) => n.toString(16).padStart(2, '0'); return `#${h((r * 255 / 31) | 0)}${h((g * 255 / 63) | 0)}${h((b * 255 / 31) | 0)}`; };
  const from565 = (hex: string) => { const n = parseInt(hex.slice(1), 16); return (((n >> 16) & 255) >> 3 << 11) | (((n >> 8) & 255) >> 2 << 5) | ((n & 255) >> 3); };
  const label = (p: ParamDef) => p.label + (p.unit ? ` (${p.unit})` : '');
  const shownValue = (p: ParamDef) => { const v = valueOf(p) + (p.offset ?? 0); return p.type === 'enum' ? p.values?.[valueOf(p)] ?? String(v) : p.type === 'bool' ? (v ? 'on' : 'off') : String(v); };

  // ---- save to the card: plan, review, confirm ----
  let plan: PresetInstallPlan | null = null;
  let reviewing = false; let planning = false; let installing = false; let installError = ''; let backupDir = '';
  const dirtySets = () => dirtyIds.map((id) => ({ meter_id: id, presets: clone(work[id]) }));
  async function startSave() {
    if (!registry) return;
    notice = ''; installError = ''; planning = true;
    try {
      plan = await meterPresetsPlan(registry.json, mediaRoot, dirtySets());
      const prefs = await getPrefs(); backupDir = prefs.backup_dir || prefs.default_backup_dir || '';
      reviewing = true;
    } catch (error) { notice = errorMessage(error); } finally { planning = false; }
  }
  async function confirmSave() {
    if (!plan || !registry) return;
    installing = true; installError = '';
    try {
      const r = await meterPresetsInstall(registry.json, mediaRoot, dirtySets(), plan.id, backupDir || null);
      reviewing = false; plan = null;
      notice = `Saved to ${cardLabel || 'the card'}: ${r.written} file${r.written === 1 ? '' : 's'} written${r.removed ? `, ${r.removed} removed` : ''}, read back and checked.${r.backup ? ` The files it changed are saved at ${r.backup}.` : ''} Eject the card safely, then start the Tau core.`;
      await load();
    } catch (error) { installError = errorMessage(error); } finally { installing = false; }
  }
  const closeReview = () => { if (!installing) { reviewing = false; plan = null; installError = ''; } };
  const meterName = (id: number) => registry?.meters.find((m) => m.id === id)?.name ?? `meter ${id}`;
  const changedCount = (id: number, state: string) => plan?.writes.filter((w) => w.meter_id === id && w.state === state).length ?? 0;
  const hiddenCount = (ids: number[]) => ids.reduce((n, id) => n + (work[id] ?? []).filter((p) => p.hidden).length, 0);
  const removedCount = (id: number) => plan?.removals.filter(([m]) => m === id).length ?? 0;
</script>

<section class="mp" aria-labelledby="mp-title" data-testid="meter-presets">
  <header class="mp-head">
    <h2 id="mp-title">{mediaRoot ? `Presets on ${cardLabel || 'the card'}` : 'Presets'}</h2>
    {#if registry && mediaRoot}<div class="mp-actions">
      <button class="primary" disabled={!dirty || planning || busy} on:click={startSave} data-testid="mp-save">{planning ? 'Checking…' : dirty ? `Save to ${cardLabel || 'the card'}…` : 'Nothing to save'}</button>
    </div>{/if}
  </header>

  {#if loading}
    <div class="mp-skeleton" data-testid="mp-loading" aria-busy="true"><span></span><span></span><span></span></div>
  {:else if !registry}
    <p class="mp-note" data-testid="mp-no-registry" role="status">The meter list is not installed yet. It comes with Tau Alpha’s lab data (a lab drop that includes <code>lab-data.json</code>); until then Omega will not guess at meters or settings.</p>
  {:else}
    {#if problem}<p class="mp-bad" role="alert">{problem}</p>{/if}
    {#if notice}<p class="mp-notice" role="status" data-testid="mp-notice">{notice}</p>{/if}
    <div class="mp-layout">
      <nav class="mp-meters" aria-label="Meters">
        {#each registry.meters as m (m.id)}
          <button class:on={selMeter === m.id} on:click={() => pickMeter(m.id)} data-testid="mp-meter-{m.key}">
            <span>{m.name}</span>
            <small title="{(work[m.id] ?? []).length} preset{(work[m.id] ?? []).length === 1 ? '' : 's'}">{(work[m.id] ?? []).length}{(work[m.id] ?? []).some((p) => p.hidden) ? ` (${(work[m.id] ?? []).filter((p) => !p.hidden).length} on Pocket)` : ''}{dirtyIds.includes(m.id) ? ' · changed' : ''}</small>
          </button>
        {/each}
      </nav>

      <div class="mp-main">
        <div class="mp-left">
          <RealMeterLab />
        </div>

        <div class="mp-right">
        {#if mediaRoot}
        <div class="mp-list-bar">
          <h3>{meter?.name} <small>{needle ? `${shownList.length} shown · ` : ''}{list.length} of {MAX_PRESETS}</small></h3>
          {#if list.length}<p class="mp-onbar" data-testid="mp-onbar"><span><b>{onCount}</b> of {list.length} will be on the Pocket{offCount ? ` · ${offCount} stay in Omega only` : ''}</span>
            <button class="quiet" on:click={() => setShownOn(true)} disabled={busy || !shownList.length || shownList.every((p) => !p.hidden)}>{needle ? 'Put shown on the Pocket' : 'Put all on the Pocket'}</button>
            <button class="quiet" on:click={() => setShownOn(false)} disabled={busy || !shownList.length || shownList.every((p) => p.hidden)}>{needle ? 'Take shown off the Pocket' : 'Take all off the Pocket'}</button></p>{/if}
          <div class="mp-actions">
            <button on:click={() => add()} disabled={busy}>New preset</button>
            <button on:click={addFactory} disabled={busy || !missingFactory} title={missingFactory ? 'Adds the factory presets this list does not have yet. Presets already in the list are not changed.' : 'All the factory presets are already in the list.'} data-testid="mp-restore-factory">Add missing factory presets{missingFactory ? ` (${missingFactory})` : ''}</button>
            {#if dirtyIds.includes(selMeter)}<button class="quiet" on:click={revert}>Undo changes</button>{/if}
            <details class="mp-more" bind:open={moreOpen}>
              <summary>More</summary>
              <div class="mp-menu">
                {#if confirmReplace}
                  <p>Start over with only the factory presets? This takes all {list.length} preset{list.length === 1 ? '' : 's'} of {meter?.name} out of the list, including your own, and puts the factory ones in. Nothing is saved to the card until you press Save, and “Undo changes” brings everything back.</p>
                  <div class="mp-actions"><button class="danger" on:click={resetToFactory} data-testid="mp-replace-yes">Yes, start over</button><button class="quiet" on:click={() => (confirmReplace = false)}>Cancel</button></div>
                {:else}
                  <button class="quiet" on:click={() => (confirmReplace = true)} disabled={busy || !view?.factory.presets.length} data-testid="mp-replace">Start over with the factory presets…</button>
                {/if}
              </div>
            </details>
          </div>
        </div>

        {#if list.length > 8}
        <label class="mp-find"><span class="mp-sr">Find a preset</span><input type="search" placeholder="Find a preset by name or id" bind:value={query} data-testid="mp-find" /></label>
        {/if}
        <!-- The arrow keys are handled for the buttons inside (they bubble to the list); the list itself is not a control. -->
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <ol class="mp-presets" aria-label="Presets of {meter?.name}" data-testid="mp-list" on:keydown={walk}>
          <li class="mp-default" class:on={selId === null} aria-label="DEFAULT, built in">
            <input type="checkbox" class="mp-on" checked disabled aria-label="DEFAULT is always on the Pocket" title="DEFAULT is built into the Pocket: it is always there" />
            <button class="mp-pick" on:click={() => { selId = null; confirmDelete = false; }} aria-current={selId === null}><span class="mp-name">DEFAULT</span><span class="mp-tags"><span class="mp-tag">built in</span></span></button>
          </li>
          {#each shownList as p (p.id)}
            <li class:on={p.id === selId} class:hid={p.hidden} use:reveal={p.id === selId}>
              <input type="checkbox" class="mp-on" checked={!p.hidden} on:change={() => toggleOn(p)} aria-label="Put {p.name || 'this preset'} on the Pocket" title={p.hidden ? 'Not on the Pocket (kept in Omega only). Tick to put it on the Pocket.' : 'On the Pocket. Untick to keep it in Omega only.'} data-testid="mp-on-{p.id}" />
              <button class="mp-pick" on:click={() => { selId = p.id; confirmDelete = false; }} aria-current={p.id === selId}>
                <span class="mp-name">{p.name || '(no name)'}</span>
                <small>no. {p.id}</small>
                <span class="mp-tags">
                  {#if isFactory(p)}<span class="mp-tag">factory</span>{:else if isEditedFactory(p)}<span class="mp-tag edit">factory, changed</span>{:else}<span class="mp-tag mine">yours</span>{/if}
                  {#if p.hidden}<span class="mp-tag hide">not on the Pocket</span>{/if}
                </span>
              </button>
            </li>
          {:else}
            <li class="mp-empty">{needle ? `No preset matches “${query.trim()}”.` : 'This meter has no presets on the card, so it uses DEFAULT only. Add the factory presets or make your own.'}</li>
          {/each}
        </ol>
        {/if}
        {#if !mediaRoot}
          <p class="mp-note" data-testid="mp-no-card" role="status">Select a Pocket card with the Tau core on the Cards page to edit presets. The meter list and the preview work without one.</p>
        {/if}
        {#if view?.problem}<p class="mp-bad" role="alert" data-testid="mp-folder-problem">The presets on the card for this meter cannot be read: {view.problem} The Pocket ignores them. Saving replaces them, after a backup.</p>{/if}
        {#if preset}
          <div class="mp-edit" data-testid="mp-edit">
            <div class="mp-edit-bar">
              <label class="mp-field">Name
                <input type="text" value={preset.name} maxlength={NAME_MAX} on:input={(e) => setName(e.currentTarget.value)} data-testid="mp-name" aria-invalid={!!nameProblem(preset.name)} />
              </label>
              <div class="mp-actions">
                <button on:click={() => move(-1)} disabled={list[0]?.id === preset.id} aria-label="Move up">Up</button>
                <button on:click={() => move(1)} disabled={list[list.length - 1]?.id === preset.id} aria-label="Move down">Down</button>
                <button on:click={toggleHidden} data-testid="mp-hide" title={preset.hidden ? 'Put it back on the Pocket' : 'Take it off the Pocket but keep it in Omega'}>{preset.hidden ? 'Put on Pocket' : 'Take off Pocket'}</button>
                <button on:click={() => add(preset)}>Duplicate</button>
                {#if confirmDelete}
                  <button class="danger" on:click={remove} data-testid="mp-delete-yes">Delete {preset.name}</button>
                  <button class="quiet" on:click={() => (confirmDelete = false)}>Keep</button>
                {:else}
                  <button class="danger" on:click={() => (confirmDelete = true)} data-testid="mp-delete">Delete</button>
                {/if}
              </div>
            </div>
            {#if nameProblem(preset.name)}<p class="mp-bad" role="alert">{nameProblem(preset.name)}</p>{/if}
            {#if preset.hidden}<p class="mp-hint" role="note" data-testid="mp-hidden-note">Not on the Pocket: this preset stays in Omega only. Put it back on the Pocket any time and it keeps the same number.</p>{/if}
            <p class="mp-hint">Number {preset.id}: the Pocket remembers your choice by this number, so it is never given to a different preset, even after you delete one.</p>

            {#each groups as g (g.name)}
              <fieldset>
                <legend>{g.name}</legend>
                {#each g.params as p (p.key)}
                  <div class="mp-param" class:set={isSet(p)}>
                    <label for="mp-{p.key}" title={p.help ?? ''}>{label(p)}</label>
                    {#if p.type === 'bool'}
                      <input id="mp-{p.key}" type="checkbox" checked={Boolean(valueOf(p))} on:change={(e) => setValue(p, e.currentTarget.checked ? 1 : 0)} />
                    {:else if p.type === 'enum'}
                      <select id="mp-{p.key}" value={valueOf(p)} on:change={(e) => setValue(p, Number(e.currentTarget.value))}>
                        {#each p.values ?? [] as option, i}<option value={i}>{option}</option>{/each}
                      </select>
                    {:else if isColour(p)}
                      <span class="mp-colour"><input id="mp-{p.key}" type="color" value={hex565(valueOf(p))} on:input={(e) => setValue(p, from565(e.currentTarget.value))} /><output>{valueOf(p)}</output></span>
                    {:else}
                      <span class="mp-range"><input id="mp-{p.key}" type="range" min={rangeOf(p)[0]} max={rangeOf(p)[1]} step={p.step ?? 1} value={valueOf(p)} on:input={(e) => setValue(p, Number(e.currentTarget.value))} /><output>{shownValue(p)}</output></span>
                    {/if}
                    <button class="quiet mp-unset" disabled={!isSet(p)} on:click={() => unset(p)} aria-label="Reset {p.label} to its default">Default</button>
                  </div>
                {/each}
              </fieldset>
            {/each}
          </div>
        {:else if mediaRoot}
          <div class="mp-defaultcard" data-testid="mp-default-card">
            <h3>DEFAULT</h3>
            <p>DEFAULT uses the standard value for every setting of {meter?.name ?? 'this meter'}. It is always on the Pocket and cannot be changed. To change settings, make a new preset: it starts from these values.</p>
            <div class="mp-actions"><button on:click={() => add()} disabled={busy}>New preset from DEFAULT</button></div>
          </div>
        {/if}
        </div>
      </div>
    </div>
  {/if}
</section>

{#if reviewing && plan}
  <div class="mp-veil" role="presentation" on:click={closeReview}></div>
  <div class="mp-modal" role="dialog" aria-labelledby="mp-rev" use:modal data-testid="mp-review">
    <h2 id="mp-rev">Save presets to {cardLabel || 'the card'}?</h2>
    <dl>
      {#each plan.meters as id}
        <div><dt>{meterName(id)}</dt><dd>{changedCount(id, 'New')} new, {changedCount(id, 'Changed')} changed, {changedCount(id, 'Unchanged')} unchanged{removedCount(id) ? `, ${removedCount(id)} removed` : ''}</dd></div>
      {/each}
      {#if hiddenCount(plan.meters) > 0}<div><dt>Not on the Pocket</dt><dd>{hiddenCount(plan.meters)} preset{hiddenCount(plan.meters) === 1 ? ' stays' : 's stay'} in Omega only and {hiddenCount(plan.meters) === 1 ? 'is' : 'are'} not put on the Pocket</dd></div>{/if}
      <div><dt>Backup</dt><dd>{plan.backup.length ? `${plan.backup.length} file${plan.backup.length === 1 ? '' : 's'} saved to ${backupDir || '(choose a backup folder in Settings)'} first` : 'nothing existing is changed, so no backup is needed'}</dd></div>
      <div><dt>Where</dt><dd>{plan.media_root}/tau-presets</dd></div>
    </dl>
    {#if plan.interrupted}<p class="mp-warn">An earlier write was interrupted. What it left is put back first.</p>{/if}
    {#each plan.warnings as w}<p class="mp-warn" role="note">{w}</p>{/each}
    {#if installError}<p class="mp-bad" role="alert">{installError}</p>{/if}
    <div class="mp-modal-actions">
      <button class="quiet" disabled={installing} on:click={closeReview}>Cancel</button>
      <button class="primary" data-autofocus disabled={installing || (plan.backup.length > 0 && !backupDir)} on:click={confirmSave} data-testid="mp-confirm">{installing ? 'Saving…' : 'Save'}</button>
    </div>
  </div>
{/if}

<style>
  .mp{display:grid;gap:14px;align-content:start;margin-top:6px;container-type:inline-size}
  .mp-head{display:flex;gap:12px;justify-content:space-between;align-items:center;flex-wrap:wrap;margin:0}
  .mp h2{margin:0;font-size:20px}.mp .mp-head h2{font-size:13px;font-weight:650;letter-spacing:0;color:#aab8b7}.mp h3{margin:0;font-size:15px}.mp h3 small{font-weight:400;color:#8f9e9d;margin-left:6px}
  .mp button{padding:8px 12px;font-size:13px;background:#202b2d;color:#e8ecec;border:1px solid #2c393a;border-radius:8px;cursor:pointer}
  .mp button.quiet{background:transparent}.mp button.primary{background:#c1f0ad;color:#142015;font-weight:700;border-color:#c1f0ad}
  .mp button.danger{border-color:#7a3d37;color:#ffb4a8}.mp button:disabled{opacity:.5;cursor:not-allowed}
  .mp-actions{display:flex;gap:6px;flex-wrap:wrap}
  .mp-list-bar .mp-actions button{padding:6px 10px;font-size:12.5px}
  .mp-note,.mp-notice{margin:0;padding:10px 12px;border:1px solid #2c393a;background:#161f20;border-radius:8px;font-size:13px;line-height:1.5}
  .mp-note code{background:#101617;padding:1px 5px;border-radius:4px}
  .mp-bad{margin:0;color:#ffb4a8;font-size:13px}.mp-warn{margin:0;color:#e5c88a;font-size:13px}
  .mp-layout{display:grid;gap:14px}
  .mp-meters{display:flex;flex-wrap:wrap;gap:6px;margin:0}
  .mp-meters button{display:flex;align-items:baseline;gap:7px;text-align:left;padding:7px 12px}
  .mp-meters button.on{border-color:#c1f0ad;background:#26352f}.mp-meters small{color:#8f9e9d;font-size:11px;white-space:nowrap}
  .mp-main{display:grid;grid-template-columns:minmax(300px,424px) minmax(0,1fr);gap:20px;align-items:start;min-width:0}
  .mp-left{display:grid;gap:12px;align-content:start;min-width:0;position:sticky;top:12px}
  .mp-right{display:grid;gap:12px;align-content:start;min-width:0}
  .mp-defaultcard{border:1px dashed #2c393a;border-radius:12px;padding:16px;display:grid;gap:8px;color:#aab8b7;font-size:13px;line-height:1.5}.mp-defaultcard p{margin:0}
  .mp-list-bar{display:grid;gap:8px}
  .mp-more{position:relative}.mp-more summary{list-style:none;cursor:pointer;padding:6px 10px;font-size:12.5px;border:1px solid #2c393a;border-radius:8px;color:#aab8b7}.mp-more summary::-webkit-details-marker{display:none}.mp-more[open] summary{color:#e8ecec;border-color:#3b4a4b}
  .mp-menu{position:absolute;z-index:5;top:calc(100% + 6px);left:0;width:300px;display:grid;gap:8px;padding:10px;background:#1a2325;border:1px solid #344244;border-radius:10px;box-shadow:0 10px 30px rgba(0,0,0,.45)}.mp-menu p{margin:0;font-size:12.5px;line-height:1.5;color:#c7d1d0}
  .mp-onbar{margin:0;display:flex;gap:8px;align-items:center;flex-wrap:wrap;font-size:12.5px;color:#aab8b7}.mp-onbar span{margin-right:auto}.mp-onbar b{color:#e8ecec}
  .mp-onbar button{padding:4px 9px!important;font-size:12px!important}
  .mp-presets{list-style:none;margin:0;padding:0 4px 0 0;display:grid;grid-template-columns:repeat(auto-fill,minmax(190px,1fr));gap:6px;max-height:min(264px,32vh);overflow-y:auto;overscroll-behavior:contain;scrollbar-width:thin;scrollbar-color:#3b4a4b transparent}
  .mp-presets .mp-default{position:sticky;top:0;z-index:1}
  .mp-find input{width:100%;box-sizing:border-box;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:8px 11px;font:inherit;font-size:13px}
  .mp-sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
  .mp-presets li{border:1px solid #2c393a;border-radius:8px;background:#1a2325;display:flex;align-items:center;min-width:0}
  .mp-on{flex:none;margin:0 0 0 10px;width:16px;height:16px;accent-color:#c1f0ad;cursor:pointer}.mp-on:disabled{cursor:default;opacity:.55}
  .mp-presets li.hid{background:#161c1d;border-style:dashed}
  .mp-presets li.on{border-color:#c1f0ad}.mp-presets li.hid .mp-name{opacity:.55;text-decoration:line-through}
  .mp-default .mp-name{color:#aab8b7}
  .mp-pick{all:unset;display:grid;grid-template-columns:minmax(0,1fr) auto;column-gap:8px;row-gap:2px;align-items:baseline;padding:7px 12px;width:100%;box-sizing:border-box;cursor:pointer;min-width:0}
  .mp-tags{grid-column:1/-1;display:flex;flex-wrap:wrap;gap:2px 8px}
  .mp-pick:focus-visible{outline:2px solid #c1f0ad;border-radius:8px}
  .mp-name{font-weight:650;font-size:13px;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .mp-tag{font-size:11px;color:#8f9e9d}
  .mp-tag.mine{color:#c1f0ad}.mp-tag.edit{color:#d9c47e}.mp-tag.hide{color:#d9c47e}
  .mp-presets small{color:#8f9e9d;font-size:11px}
  .mp-empty{grid-column:1/-1;padding:10px 12px;font-size:13px;color:#aab8b7;border-style:dashed!important}
  .mp-edit{display:grid;gap:10px;border:1px solid #2c393a;border-radius:12px;padding:16px;background:#1a2325}
  .mp-edit-bar{display:flex;gap:12px;flex-wrap:wrap;align-items:end;justify-content:space-between}
  .mp-field{display:grid;gap:4px;font-size:12px;font-weight:650;color:#acbbbb;min-width:200px}
  .mp-field input{color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:9px 11px;font:inherit;font-size:13px}
  .mp-field input[aria-invalid=true]{border-color:#c46b60}
  .mp-hint{margin:0;color:#8f9e9d;font-size:12px}
  fieldset{border:1px solid #2c393a;border-radius:10px;padding:8px 12px;display:grid;gap:8px;background:#161f20;margin:0}
  legend{padding:0 6px;color:#c1f0ad;text-transform:capitalize}
  .mp-param{display:grid;grid-template-columns:minmax(110px,1fr) minmax(120px,2fr) auto;gap:12px;align-items:center;font-size:13px}
  .mp-param label{color:#c7d1d0}.mp-param.set label{color:#e8ecec;font-weight:650}
  .mp-range,.mp-colour{display:flex;gap:10px;align-items:center}.mp-range input{flex:1;accent-color:#c1f0ad}
  .mp-range output,.mp-colour output{min-width:48px;text-align:right;color:#8f9e9d;font-size:12px;font-variant-numeric:tabular-nums}
  .mp-param select{background:#101617;color:#e8ecec;border:1px solid #3b4a4b;border-radius:8px;padding:7px 9px;font:inherit;font-size:13px}
  .mp-unset{padding:4px 8px!important;font-size:11px!important}
  .mp-skeleton{display:grid;gap:8px}.mp-skeleton span{height:38px;border-radius:8px;background:linear-gradient(90deg,#1a2325,#233033,#1a2325);background-size:200% 100%;animation:mp-shimmer 1.4s linear infinite}
  @keyframes mp-shimmer{to{background-position:-200% 0}}
  @media (prefers-reduced-motion:reduce){.mp-skeleton span{animation:none}}
  .mp-veil{position:fixed;inset:0;z-index:60;background:rgba(0,0,0,.55)}
  .mp-modal{position:fixed;z-index:70;top:50%;left:50%;transform:translate(-50%,-50%);width:min(560px,92vw);background:#1a2325;border:1px solid #344244;border-radius:16px;padding:22px;display:grid;gap:12px}
  .mp-modal h2{margin:0;font-size:18px}.mp-modal dl{margin:0;display:grid;gap:8px}.mp-modal dl div{display:grid;grid-template-columns:130px 1fr;gap:10px}
  .mp-modal dt{color:#9fb3aa;font-size:12px}.mp-modal dd{margin:0;font-size:13px;overflow-wrap:anywhere}
  .mp-modal-actions{display:flex;justify-content:flex-end;gap:8px}
  .mp-modal button{padding:8px 14px;font-size:13px;background:#202b2d;color:#e8ecec;border:1px solid #2c393a;border-radius:8px;cursor:pointer}
  .mp-modal button.quiet{background:transparent}.mp-modal button.primary{background:#c1f0ad;color:#142015;font-weight:700;border-color:#c1f0ad}
  .mp-modal button:disabled{opacity:.5;cursor:not-allowed}
  /* Side by side only when there is room for the preview and the controls; otherwise the preview sits above them. */
  @container (max-width: 880px){.mp-main{grid-template-columns:1fr}.mp-left{position:static}}
  @container (max-width: 520px){.mp-param{grid-template-columns:1fr}}
</style>
