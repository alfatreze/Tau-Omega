<script lang="ts">
  // Game Boy colour palettes (`Assets/gb/common/Palettes/*.pal`): browse the community pack with live 160x144
  // 4-layer previews, see what is on the card, and install or remove palettes via the reviewed plan/execute flow.
  import { onMount } from 'svelte';
  import { errorMessage } from './backend';
  import { showToast } from './toastStore';
  import {
    FAMILIES,
    PALETTE_DIR,
    SCENE_LIST,
    describe,
    drawScene,
    fromHex,
    loadPalettes,
    nameOf,
    parsePal,
    rgb,
    type ParsedPal,
    type SceneId,
  } from './gbPalettes';
  import {
    gbPalettesExecuteInstall,
    gbPalettesExecuteRemove,
    gbPalettesList,
    gbPalettesPlanInstall,
    gbPalettesPlanRemove,
  } from './tau-api';
  import type { PaletteInstallPlan, PaletteRemovePlan } from './types';

  export let card = '';

  type Entry = {
    path: string;
    name: string;
    group: string;
    hex: string;
    pal: ParsedPal;
    family: string;
    search: string;
  };

  let loading = true;
  let groups: string[] = [];
  let entries: Entry[] = [];
  let onCard: string[] = [];
  let onCardSet = new Set<string>();
  let query = '';
  let group = '';
  let family = '';
  let onlyInstalled = false;
  let scene: SceneId = 'action';
  let limit = 36;

  let selected = new Set<string>();
  let installPlan: PaletteInstallPlan | null = null;
  let removePlan: PaletteRemovePlan | null = null;
  let busy = false;
  let error = '';
  let loadedCard = '';

  function paint(node: HTMLCanvasElement, params: { scene: SceneId; pal: ParsedPal }) {
    drawScene(node, params.scene, params.pal);
    return {
      update(next: { scene: SceneId; pal: ParsedPal }) {
        drawScene(node, next.scene, next.pal);
      },
    };
  }

  onMount(async () => {
    try {
      const idx = await loadPalettes();
      groups = idx.groups ?? [];
      entries = (idx.palettes ?? []).map((p) => {
        const pal = parsePal(fromHex(p.b));
        const info = describe(pal);
        const grp = groups[p.g] ?? '';
        const name = nameOf(p.p);
        return {
          path: p.p,
          name,
          group: grp,
          hex: p.b,
          pal,
          family: info.family,
          search: `${name} ${grp} ${p.p}`.toLowerCase(),
        };
      });
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  });

  async function refreshCard(c: string) {
    if (!c) { onCard = []; onCardSet = new Set(); return; }
    loadedCard = c;
    try {
      onCard = await gbPalettesList(c);
      onCardSet = new Set(onCard);
    } catch {
      onCard = [];
      onCardSet = new Set();
    }
  }

  $: if (card && card !== loadedCard) void refreshCard(card);

  $: q = query.trim().toLowerCase();
  $: filtered = entries.filter((e) => {
    if (onlyInstalled && !onCardSet.has(e.path)) return false;
    if (group && e.group !== group) return false;
    if (family && e.family !== family) return false;
    if (q && !e.search.includes(q)) return false;
    return true;
  });
  $: shown = filtered.slice(0, limit);
  $: indexPaths = new Set(entries.map((e) => e.path));
  $: customOnCard = onCard.filter((p) => !indexPaths.has(p));

  function resetLimit() { limit = 36; }

  function toggleSelect(path: string) {
    const next = new Set(selected);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    selected = next;
    installPlan = null;
  }

  function clearSelection() {
    selected = new Set();
    installPlan = null;
  }

  function selectedItems() {
    return entries
      .filter((e) => selected.has(e.path))
      .map((e) => ({ rel_path: e.path, hex: e.hex }));
  }

  async function reviewInstall() {
    if (!card || !selected.size) return;
    busy = true; error = ''; removePlan = null;
    try {
      installPlan = await gbPalettesPlanInstall(card, selectedItems());
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function runInstall() {
    if (!card || !installPlan) return;
    busy = true; error = '';
    try {
      const report = await gbPalettesExecuteInstall(card, selectedItems(), installPlan.id);
      installPlan = null;
      selected = new Set();
      await refreshCard(card);
      const msg = report.written === 0
        ? 'Those palettes were already on this card.'
        : `Installed ${report.written} ${report.written === 1 ? 'palette' : 'palettes'} to ${PALETTE_DIR}.`;
      showToast(msg, report.written === 0 ? 'info' : 'success');
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function reviewRemove(relPath: string) {
    if (!card) return;
    busy = true; error = ''; installPlan = null;
    try {
      removePlan = await gbPalettesPlanRemove(card, relPath);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function runRemove() {
    if (!card || !removePlan) return;
    busy = true; error = '';
    const rel = removePlan.rel_path;
    try {
      await gbPalettesExecuteRemove(card, rel, removePlan.id);
      removePlan = null;
      await refreshCard(card);
      showToast(`Removed ${nameOf(rel)} from this card (a backup was saved first).`);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="gb-pal" aria-label="Game Boy palettes">
  <div class="pal-head">
    <div>
      <p class="pal-lede">
        Colour palettes for Game Boy cores. Installed to <code>{PALETTE_DIR}</code> on this card — pick one in the core’s Display menu on the Pocket.
      </p>
      <p class="pal-meta">
        {onCard.length} on this card{#if !loading} · {entries.length.toLocaleString()} in pack{/if}
      </p>
    </div>
    <div class="pal-scenes" role="group" aria-label="Preview screen">
      {#each SCENE_LIST as s}
        <button class="scene-btn" class:on={scene === s.id} aria-pressed={scene === s.id} on:click={() => (scene = s.id)}>{s.label}</button>
      {/each}
    </div>
  </div>

  {#if error}<p class="pal-err" role="alert">{error}</p>{/if}

  {#if installPlan}
    <div class="pal-plan" role="region" aria-label="Install plan">
      <strong>Ready to install {installPlan.new_files.length + installPlan.updated_files.length} {installPlan.new_files.length + installPlan.updated_files.length === 1 ? 'palette' : 'palettes'}</strong>
      <p>
        {#if installPlan.new_files.length}{installPlan.new_files.length} new{/if}
        {#if installPlan.new_files.length && installPlan.updated_files.length} · {/if}
        {#if installPlan.updated_files.length}{installPlan.updated_files.length} replaced (backed up first){/if}
        {#if installPlan.unchanged} · {installPlan.unchanged} already on card{/if}
      </p>
      <div class="pal-plan-btns">
        <button class="primary" disabled={busy || (installPlan.new_files.length + installPlan.updated_files.length === 0)} on:click={runInstall}>
          {busy ? 'Installing…' : 'Install on card'}
        </button>
        <button class="quiet" disabled={busy} on:click={() => (installPlan = null)}>Cancel</button>
      </div>
    </div>
  {/if}

  {#if removePlan}
    <div class="pal-plan warn" role="region" aria-label="Remove plan">
      <strong>Remove “{nameOf(removePlan.rel_path)}”?</strong>
      <p>Deletes <code>{PALETTE_DIR}/{removePlan.rel_path}</code> after saving a verified backup outside the card.</p>
      <div class="pal-plan-btns">
        <button class="primary" disabled={busy} on:click={runRemove}>{busy ? 'Removing…' : 'Remove palette'}</button>
        <button class="quiet" disabled={busy} on:click={() => (removePlan = null)}>Cancel</button>
      </div>
    </div>
  {/if}

  {#if selected.size > 0 && !installPlan}
    <div class="pal-selbar" role="status">
      <span><strong>{selected.size}</strong> {selected.size === 1 ? 'palette' : 'palettes'} selected</span>
      <span class="pal-sel-btns">
        <button class="quiet" on:click={clearSelection}>Clear</button>
        <button class="primary" disabled={busy} on:click={reviewInstall}>Review install…</button>
      </span>
    </div>
  {/if}

  <div class="pal-controls">
    <label class="pal-search">
      <span class="sr">Search palettes</span>
      <input type="search" placeholder="Search palettes by name or collection" bind:value={query} on:input={resetLimit} />
    </label>
    <label class="pal-select">
      <span class="sr">Collection</span>
      <select bind:value={group} on:change={resetLimit}>
        <option value="">All collections</option>
        {#each groups as g}
          <option value={g}>{g}</option>
        {/each}
      </select>
    </label>
    <button class="quiet pal-toggle" class:on={onlyInstalled} aria-pressed={onlyInstalled} on:click={() => { onlyInstalled = !onlyInstalled; resetLimit(); }}>
      On this card ({onCard.length})
    </button>
  </div>

  <div class="pal-families" role="group" aria-label="Colour family">
    <button class="fam-chip" class:on={!family} aria-pressed={!family} on:click={() => { family = ''; resetLimit(); }}>All</button>
    {#each FAMILIES as f}
      <button class="fam-chip" class:on={family === f.id} aria-pressed={family === f.id} on:click={() => { family = family === f.id ? '' : f.id; resetLimit(); }}>
        <span class="fam-dot" style={`background:${f.swatch}`} aria-hidden="true"></span>{f.label}
      </button>
    {/each}
  </div>

  {#if customOnCard.length && (onlyInstalled || !q)}
    <div class="pal-custom">
      <h3>Other palettes on this card</h3>
      <ul>
        {#each customOnCard as rel}
          <li>
            <span><strong>{nameOf(rel)}</strong><small>{rel}</small></span>
            <button class="quiet" disabled={busy} on:click={() => reviewRemove(rel)}>Remove</button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if loading}
    <p class="pal-empty" role="status">Loading palettes…</p>
  {:else if !filtered.length}
    <div class="pal-empty">
      <strong>No palettes match</strong>
      <p>{onlyInstalled && !onCard.length ? 'No palettes are installed on this card yet. Turn off “On this card” to browse the pack.' : 'Try clearing the search or colour filter.'}</p>
    </div>
  {:else}
    <ul class="pal-grid" aria-label="Game Boy palettes">
      {#each shown as item (item.path)}
        {@const installed = onCardSet.has(item.path)}
        {@const picked = selected.has(item.path)}
        <li class="pal-card" class:picked class:installed>
          <button class="pal-preview-btn" type="button" on:click={() => toggleSelect(item.path)} aria-pressed={picked} aria-label={`${picked ? 'Deselect' : 'Select'} ${item.name}`}>
            <canvas class="pal-canvas" width="160" height="144" use:paint={{ scene, pal: item.pal }}></canvas>
            <span class="pal-swatches" aria-hidden="true">
              {#each item.pal.layers[0] as c}
                <span style={`background:${rgb(c)}`}></span>
              {/each}
            </span>
          </button>
          <div class="pal-body">
            <strong title={item.name}>{item.name}</strong>
            <small>{item.group}</small>
          </div>
          <div class="pal-foot">
            {#if installed}
              <span class="chip capable">On card</span>
              <button class="quiet pal-rm" disabled={busy} on:click={() => reviewRemove(item.path)}>Remove</button>
            {:else}
              <button class="quiet pal-pick" class:picked on:click={() => toggleSelect(item.path)}>
                {picked ? 'Selected' : 'Select'}
              </button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
    {#if filtered.length > shown.length}
      <div class="pal-more">
        <button class="quiet" on:click={() => (limit += 36)}>
          Show more ({filtered.length - shown.length} remaining)
        </button>
      </div>
    {/if}
  {/if}
</section>

<style>
  .gb-pal{display:grid;gap:14px}
  .pal-head{display:flex;justify-content:space-between;align-items:flex-start;gap:14px;flex-wrap:wrap}
  .pal-lede{margin:0;font-size:13px;color:#c7d1d0;line-height:1.5}
  .pal-lede code{font-size:11.5px;color:#dce6e4}
  .pal-meta{margin:4px 0 0;font-size:12px;color:#8f9e9d}
  .pal-scenes{display:inline-flex;background:#101617;border:1px solid #2c393a;border-radius:9px;padding:3px;gap:2px}
  .scene-btn{padding:5px 10px;font-size:12px;border-radius:6px;background:transparent;color:#95a5a4}
  .scene-btn:hover{color:#eff5f4}.scene-btn.on{background:#1f2e30;color:#c1f0ad}
  .pal-err{margin:0;padding:10px 14px;border-radius:10px;border:1px solid #6b3535;background:#261616;color:#f2b8b5;font-size:13px}
  .pal-plan{padding:13px 16px;border-radius:12px;border:1px solid #3d5254;background:#162022;display:grid;gap:6px}
  .pal-plan.warn{border-color:#5c5030;background:#241f12;color:#e5d9a8}
  .pal-plan p{margin:0;font-size:12.5px;color:#c7d1d0}.pal-plan.warn p{color:#e5d9a8}
  .pal-plan-btns{display:flex;gap:8px;margin-top:4px}
  .pal-selbar{display:flex;justify-content:space-between;align-items:center;gap:12px;padding:10px 14px;border-radius:10px;border:1px solid #3d5254;background:#182426;font-size:13px}
  .pal-sel-btns{display:flex;gap:8px}
  .pal-controls{display:flex;gap:10px;flex-wrap:wrap;align-items:center}
  .pal-search{flex:1;min-width:200px}
  .pal-search input,.pal-select select{width:100%;box-sizing:border-box;font:inherit;font-size:13px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:10px;padding:8px 12px}
  .pal-toggle.on{border-color:#6fa35a;color:#c1f0ad;background:#17241a}
  .pal-families{display:flex;flex-wrap:wrap;gap:6px}
  .fam-chip{display:inline-flex;align-items:center;gap:6px;padding:4px 10px;font-size:12px;border-radius:999px;background:#141d1e;border:1px solid #2c393a;color:#a6b3b2}
  .fam-chip:hover{border-color:#3d5254;color:#eff5f4}
  .fam-chip.on{border-color:#c1f0ad;color:#c1f0ad;background:#18261c}
  .fam-dot{width:9px;height:9px;border-radius:50%;flex:none}
  .pal-custom{padding:12px 14px;border:1px solid #2c393a;border-radius:12px;background:#141d1e}
  .pal-custom h3{margin:0 0 8px;font-size:12.5px;color:#9fb3aa;text-transform:uppercase;letter-spacing:.05em}
  .pal-custom ul{list-style:none;margin:0;padding:0;display:grid;gap:6px}
  .pal-custom li{display:flex;justify-content:space-between;align-items:center;gap:10px;font-size:13px}
  .pal-custom small{display:block;font-size:11.5px;color:#8f9e9d}
  .pal-grid{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fill,minmax(175px,1fr));gap:12px}
  .pal-card{display:flex;flex-direction:column;background:#161f20;border:1px solid #2c393a;border-radius:12px;overflow:hidden}
  .pal-card.picked{border-color:#c1f0ad;box-shadow:0 0 0 1px #c1f0ad}
  .pal-preview-btn{display:block;width:100%;padding:0;border:0;background:#0b1011;cursor:pointer;text-align:left}
  .pal-preview-btn:focus-visible{outline:2px solid #c1f0ad;outline-offset:-2px}
  .pal-canvas{display:block;width:100%;aspect-ratio:160/144;image-rendering:pixelated;image-rendering:crisp-edges}
  .pal-swatches{display:grid;grid-template-columns:repeat(4,1fr);height:6px}
  .pal-body{padding:9px 11px 6px;display:grid;gap:2px}
  .pal-body strong{font-size:13px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
  .pal-body small{font-size:11.5px;color:#8f9e9d;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
  .pal-foot{padding:6px 11px 10px;margin-top:auto;display:flex;align-items:center;justify-content:space-between;gap:6px}
  .pal-pick,.pal-rm{padding:4px 9px;font-size:12px}
  .pal-pick.picked{color:#c1f0ad;border-color:#6fa35a}
  .pal-more{display:flex;justify-content:center;padding:6px 0}
  .pal-empty{padding:24px;text-align:center;color:#95a5a4;font-size:13px}
  .sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
</style>
