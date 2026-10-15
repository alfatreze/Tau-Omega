<script lang="ts">
  // The Cores entry of the side menu: the openFPGA Library's grouping, category > manufacturer > platform > core (for example Console >
  // Nintendo > SNES > a core). Choosing a platform shows its cores in the page, choosing a core opens it. Categories and manufacturers
  // are open until the user closes them; platforms start closed. Keyboard: Up/Down move, Right opens or goes in, Left closes or goes to
  // the parent, Home/End jump.
  import Icon from './Icon.svelte';
  import { categoryIcon } from './iconFor';
  import { coresAttention, coresError, coresGroups, coresLoading, coresSelection, coresTotal, coresTreeStore, refreshCores } from './coresStore';
  /** The open card folder (empty when none). */
  export let card = '';
  /** True while the Cores page is the one showing. */
  export let active = false;
  /** Called after a selection so the app can switch to the Cores page. */
  export let show: () => void;

  let open = true; let filter = '';
  let expanded: Record<string, boolean> = {};   // platforms the user opened
  let closed: Record<string, boolean> = {};     // categories and manufacturers the user closed
  try {
    open = localStorage.getItem('tau.nav.cores') !== '0';
    expanded = JSON.parse(localStorage.getItem('tau.nav.cores.expanded') ?? '{}');
    closed = JSON.parse(localStorage.getItem('tau.nav.cores.closed') ?? '{}');
  } catch { /* per-viewer convenience */ }
  const save = () => { try { localStorage.setItem('tau.nav.cores', open ? '1' : '0'); localStorage.setItem('tau.nav.cores.expanded', JSON.stringify(expanded)); localStorage.setItem('tau.nav.cores.closed', JSON.stringify(closed)); } catch { /* ignore */ } };

  let lastCard = '\0';
  $: if (card !== lastCard) { lastCard = card; void refreshCores(card); }
  $: needle = filter.trim().toLowerCase();
  // Filtering keeps the cores that match (or every core of a platform whose name matches) and drops empty branches.
  $: groups = $coresGroups
    .map((c) => ({ ...c, makers: c.makers.map((m) => ({ ...m, platforms: m.platforms
      .map((n) => ({ ...n, cores: !needle || n.platform.name.toLowerCase().includes(needle) ? n.cores : n.cores.filter((k) => `${k.id} ${k.shortname}`.toLowerCase().includes(needle)) }))
      .filter((n) => !needle || n.cores.length) })).filter((m) => m.platforms.length) }))
    .filter((c) => c.makers.length);
  // Open when searching or not closed by the user (a function would not see the maps change).
  $: catOpen = Object.fromEntries($coresGroups.map((c) => [c.name, !!needle || !closed[`c:${c.name}`]])) as Record<string, boolean>;
  $: makerOpen = Object.fromEntries($coresGroups.flatMap((c) => c.makers.map((m) => [`${c.name}/${m.name}`, !!needle || !closed[`m:${c.name}/${m.name}`]]))) as Record<string, boolean>;
  $: platOpen = Object.fromEntries($coresTreeStore.map((n) => [n.platform.id, !!needle || !!expanded[n.platform.id]])) as Record<string, boolean>;
  // A platform that gets selected is opened once (with its parents), so the user can still collapse it.
  let lastShown = '';
  $: if (active && $coresSelection.platform && lastShown !== $coresSelection.platform) {
    lastShown = $coresSelection.platform; expanded = { ...expanded, [lastShown]: true };
    const node = $coresTreeStore.find((n) => n.platform.id === lastShown);
    if (node) { const c = $coresGroups.find((g) => g.makers.some((m) => m.platforms.includes(node))); if (c) { const m = c.makers.find((x) => x.platforms.includes(node))!; closed = { ...closed, [`c:${c.name}`]: false, [`m:${c.name}/${m.name}`]: false }; } }
    save();
  }

  function toggleGroup() { open = !open; save(); if (open) { coresSelection.set({ platform: '', core: '' }); show(); } }
  function toggleClosed(key: string) { closed = { ...closed, [key]: !closed[key] }; save(); }
  function togglePlat(id: string) { expanded = { ...expanded, [id]: !expanded[id] }; save(); }
  function pickPlatform(id: string) { expanded = { ...expanded, [id]: true }; save(); coresSelection.set({ platform: id, core: '' }); show(); }
  function pickCore(platform: string, core: string) { coresSelection.set({ platform, core }); show(); }

  const isOpen = (el: HTMLElement) => el.dataset.kind === 'category' ? !!catOpen[el.dataset.id!] : el.dataset.kind === 'maker' ? !!makerOpen[el.dataset.id!] : el.dataset.kind === 'platform' ? !!platOpen[el.dataset.id!] : false;
  function toggleEl(el: HTMLElement) {
    if (el.dataset.kind === 'category') toggleClosed(`c:${el.dataset.id}`);
    else if (el.dataset.kind === 'maker') toggleClosed(`m:${el.dataset.id}`);
    else if (el.dataset.kind === 'platform') togglePlat(el.dataset.id!);
  }
  /** Arrow-key model over the visible rows (every row is an `.nt-item` button in document order; `data-level` is its depth). */
  function key(e: KeyboardEvent) {
    const items = [...(e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('.nt-item')];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const cur = items[at]; if (!cur) return;
    const go = (el?: HTMLElement) => { if (el) { e.preventDefault(); el.focus(); } };
    const level = Number(cur.dataset.level); const branch = cur.dataset.kind !== 'core';
    if (e.key === 'ArrowDown') go(items[at + 1]);
    else if (e.key === 'ArrowUp') go(items[at - 1]);
    else if (e.key === 'Home') go(items[0]);
    else if (e.key === 'End') go(items[items.length - 1]);
    else if (e.key === 'ArrowRight' && branch) { if (!isOpen(cur)) { e.preventDefault(); toggleEl(cur); } else go(items[at + 1]); }
    else if (e.key === 'ArrowLeft') {
      if (branch && isOpen(cur) && !needle) { e.preventDefault(); toggleEl(cur); }
      else go(items.slice(0, at).reverse().find((i) => Number(i.dataset.level) < level));
    }
  }
</script>
<div class="nt">
  <button class="nav-group" class:has-active={active} aria-expanded={open} aria-controls="cores-tree" on:click={toggleGroup}>
    <span>Cores{#if $coresTotal}<span class="nt-count" aria-label={`${$coresTotal} cores`}>{$coresTotal}</span>{/if}{#if $coresAttention}<span class="nt-dot" role="img" aria-label={`${$coresAttention} need attention`} title="Some cores need a file"></span>{/if}</span>
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class:open><path d="m6 9 6 6 6-6"/></svg>
  </button>
  {#if open}
    <div class="nt-body" id="cores-tree">
      {#if !card}<p class="nt-note">Open a card to see its cores.</p>
      {:else if $coresError}<p class="nt-note nt-err" role="alert">{$coresError}</p>
      {:else if $coresLoading && !$coresTreeStore.length}<p class="nt-note" role="status">Reading the card…</p>
      {:else if !$coresTreeStore.length}<p class="nt-note">No cores on this card.</p>
      {:else}
        {#if $coresTreeStore.length > 12}
          <label class="nt-filter"><span class="sr">Filter platforms and cores</span><input type="search" placeholder="Filter" bind:value={filter} /></label>
        {/if}
        <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
        <ul class="nt-tree" role="tree" aria-label="Cores by platform" on:keydown={key}>
          {#each groups as c (c.name)}
            <li role="treeitem" aria-level="1" aria-expanded={catOpen[c.name]} aria-selected="false">
              <div class="nt-row">
                <button class="nt-chev" tabindex="-1" aria-label={`${catOpen[c.name] ? 'Collapse' : 'Expand'} ${c.name}`} on:click={() => toggleClosed(`c:${c.name}`)}><svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class:open={catOpen[c.name]}><path d="m9 6 6 6-6 6"/></svg></button>
                <button class="nt-item nt-cat" data-kind="category" data-level="1" data-id={c.name} on:click={() => toggleClosed(`c:${c.name}`)}><Icon name={categoryIcon(c.name)} size={15} /><span class="nt-name">{c.name}</span><span class="nt-count">{c.cores}</span></button>
              </div>
              {#if catOpen[c.name]}
                <ul role="group" class="nt-sub">
                  {#each c.makers as m (m.name)}
                    {@const mk = `${c.name}/${m.name}`}
                    <li role="treeitem" aria-level="2" aria-expanded={makerOpen[mk]} aria-selected="false">
                      {#if m.name}
                        <div class="nt-row">
                          <button class="nt-chev" tabindex="-1" aria-label={`${makerOpen[mk] ? 'Collapse' : 'Expand'} ${m.name}`} on:click={() => toggleClosed(`m:${mk}`)}><svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class:open={makerOpen[mk]}><path d="m9 6 6 6-6 6"/></svg></button>
                          <button class="nt-item nt-maker" data-kind="maker" data-level="2" data-id={mk} on:click={() => toggleClosed(`m:${mk}`)}><span class="nt-name">{m.name}</span><span class="nt-count">{m.cores}</span></button>
                        </div>
                      {/if}
                      {#if makerOpen[mk] || !m.name}
                        <ul role="group" class="nt-sub">
                          {#each m.platforms as n (n.platform.id)}
                            <li role="treeitem" aria-level="3" aria-expanded={platOpen[n.platform.id]} aria-selected={active && $coresSelection.platform === n.platform.id && !$coresSelection.core}>
                              <div class="nt-row">
                                <button class="nt-chev" tabindex="-1" aria-label={`${platOpen[n.platform.id] ? 'Collapse' : 'Expand'} ${n.platform.name}`} on:click={() => togglePlat(n.platform.id)}><svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class:open={platOpen[n.platform.id]}><path d="m9 6 6 6-6 6"/></svg></button>
                                <button class="nt-item" class:sel={active && $coresSelection.platform === n.platform.id && !$coresSelection.core} data-kind="platform" data-level="3" data-id={n.platform.id} on:click={() => pickPlatform(n.platform.id)}>
                                  <Icon name={categoryIcon(c.name)} size={14} /><span class="nt-name">{n.platform.name}</span><span class="nt-count">{n.cores.length}</span>
                                </button>
                              </div>
                              {#if platOpen[n.platform.id]}
                                <ul role="group" class="nt-sub">
                                  {#each n.cores as k (k.id)}
                                    <li role="treeitem" aria-level="4" aria-selected={active && $coresSelection.core === k.id}>
                                      <button class="nt-item nt-core" class:sel={active && $coresSelection.core === k.id} data-kind="core" data-level="4" data-id={k.id} on:click={() => pickCore(n.platform.id, k.id)}>
                                        <span class="nt-name">{k.shortname}</span>{#if k.missing}<span class="nt-dot" role="img" aria-label="Needs a file" title="Needs a file"></span>{/if}
                                      </button>
                                    </li>
                                  {/each}
                                </ul>
                              {/if}
                            </li>
                          {/each}
                        </ul>
                      {/if}
                    </li>
                  {/each}
                </ul>
              {/if}
            </li>
          {/each}
          {#if needle && !groups.length}<li class="nt-note">Nothing matches “{filter}”.</li>{/if}
        </ul>
      {/if}
    </div>
  {/if}
</div>
<style>
  .nt{display:block}
  .nt-body{margin:2px 0 4px 12px;padding-left:8px;border-left:1px solid #2c393a}
  .nt-note{font-size:12px;color:#8f9e9d;margin:8px 4px;line-height:1.5}.nt-err{color:#ff9b9b}
  .nt-filter input{width:100%;box-sizing:border-box;font:inherit;font-size:12px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:7px;padding:6px 9px;margin:4px 0 6px}
  .nt-tree,.nt-sub{list-style:none;margin:0;padding:0}
  .nt-tree{max-height:min(52vh,480px);overflow:auto;padding-right:2px}
  .nt-sub{margin:0 0 2px 14px}
  .nt-row{display:flex;align-items:center;gap:2px}
  .nt-chev{flex:none;width:22px;height:26px;padding:0;display:grid;place-items:center;background:transparent;color:#8f9e9d;border-radius:6px}
  .nt-chev:hover{background:#202b2d;color:#e8ecec}.nt-chev svg{transition:transform .12s}.nt-chev svg.open{transform:rotate(90deg)}
  .nt-item{flex:1;min-width:0;display:flex;align-items:center;justify-content:space-between;gap:8px;padding:6px 8px;font-size:13px;line-height:1.25;border-radius:7px;color:#b4bfbf;background:transparent;text-align:left}
  .nt-item:hover{background:#202b2d;color:#eff5f4}
  .nt-item:focus-visible,.nt-chev:focus-visible{outline:2px solid #c1f0ad;outline-offset:-2px}
  .nt-item.sel{background:#202b2d;color:#c1f0ad}
  .nt-cat{font-size:11px;font-weight:700;letter-spacing:.07em;text-transform:uppercase;color:#9fb3aa}
  .nt-maker{font-size:12px;color:#8f9e9d}
  .nt-core{width:100%}
  .nt-item :global(.ico){margin-right:7px;opacity:.85}.nt-item .nt-name{flex:1}
  .nt-name{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .nt-count{flex:none;font-size:11px;color:#8f9e9d;margin-left:6px}
  .nt-dot{flex:none;display:inline-block;width:7px;height:7px;border-radius:50%;background:#e5c88a;margin-left:6px}
  .sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
</style>
