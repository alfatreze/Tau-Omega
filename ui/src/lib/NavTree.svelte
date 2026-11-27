<script lang="ts">
  // Simplified Cores navigation in the sidebar:
  // - Analogue Pocket: 2 levels max (Platform -> Core), removing redundant Category and Manufacturer nesting
  //   so platform names stay readable in the narrow sidebar. Full category/manufacturer browsing stays on the Cores overview page.
  // - Game Bub: 1 flat level listing installed cores directly (no redundant "Game Bub > Game Bub" wrapper).
  // Keyboard: Up/Down move, Right opens or enters, Left closes or goes to parent, Home/End jump.
  import Icon from './Icon.svelte';
  import { categoryIcon } from './iconFor';
  import { coresAttention, coresError, coresLoading, coresSelection, coresTotal, coresTreeStore, refreshCores } from './coresStore';
  /** The open card folder (empty when none). */
  export let card = '';
  /** True while the Cores page is the one showing. */
  export let active = false;
  /** Called after a selection so the app can switch to the Cores page. */
  export let show: () => void;

  let open = true;
  let expanded: Record<string, boolean> = {};
  try {
    open = localStorage.getItem('tau.nav.cores') !== '0';
    expanded = JSON.parse(localStorage.getItem('tau.nav.cores.expanded') ?? '{}');
  } catch { /* per-viewer convenience */ }
  const save = () => {
    try {
      localStorage.setItem('tau.nav.cores', open ? '1' : '0');
      localStorage.setItem('tau.nav.cores.expanded', JSON.stringify(expanded));
    } catch { /* ignore */ }
  };

  let lastCard = '\0';
  $: if (card !== lastCard) { lastCard = card; void refreshCores(card); }
  $: nodes = $coresTreeStore;
  // Single synthetic Game Bub container -> list its cores directly without a wrapper row.
  $: flatGameBub = nodes.length === 1 && nodes[0].platform.id === 'gamebub';
  $: platOpen = Object.fromEntries(nodes.map((n) => [n.platform.id, !!expanded[n.platform.id]])) as Record<string, boolean>;

  // Auto-expand a platform once when one of its cores is selected externally.
  let lastShown = '';
  $: if (active && $coresSelection.platform && $coresSelection.core && lastShown !== $coresSelection.platform) {
    lastShown = $coresSelection.platform;
    if (!flatGameBub) { expanded = { ...expanded, [lastShown]: true }; save(); }
  }

  function openOverview() {
    if (!open) { open = true; save(); }
    coresSelection.set({ platform: '', core: '' });
    show();
  }
  function toggleSection() { open = !open; save(); }
  function togglePlat(id: string) { expanded = { ...expanded, [id]: !expanded[id] }; save(); }
  function pickPlatform(id: string) { coresSelection.set({ platform: id, core: '' }); show(); }
  function pickCore(platform: string, core: string) { coresSelection.set({ platform, core }); show(); }

  const isOpen = (el: HTMLElement) => el.dataset.kind === 'platform' && !!platOpen[el.dataset.id!];
  function key(e: KeyboardEvent) {
    const items = [...(e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('.nt-item')];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const cur = items[at]; if (!cur) return;
    const go = (el?: HTMLElement) => { if (el) { e.preventDefault(); el.focus(); } };
    const level = Number(cur.dataset.level);
    const branch = cur.dataset.kind === 'platform';
    if (e.key === 'ArrowDown') go(items[at + 1]);
    else if (e.key === 'ArrowUp') go(items[at - 1]);
    else if (e.key === 'Home') go(items[0]);
    else if (e.key === 'End') go(items[items.length - 1]);
    else if (e.key === 'ArrowRight' && branch) { if (!isOpen(cur)) { e.preventDefault(); togglePlat(cur.dataset.id!); } else go(items[at + 1]); }
    else if (e.key === 'ArrowLeft') {
      if (branch && isOpen(cur)) { e.preventDefault(); togglePlat(cur.dataset.id!); }
      else go(items.slice(0, at).reverse().find((i) => Number(i.dataset.level) < level));
    }
  }
</script>
<div class="nt">
  <div class="nt-head">
    <button class="nav-group nt-head-btn" class:has-active={active} on:click={openOverview}>
      <span>Cores{#if $coresTotal}<span class="nt-count" aria-label={`${$coresTotal} cores`}>{$coresTotal}</span>{/if}{#if $coresAttention}<span class="nt-dot" role="img" aria-label={`${$coresAttention} need attention`} title="Some cores need a file"></span>{/if}</span>
    </button>
    <button class="nt-chev nt-head-chev" aria-expanded={open} aria-controls="cores-tree" aria-label={open ? 'Collapse cores' : 'Expand cores'} on:click={toggleSection}>
      <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class:open><path d="m9 6 6 6-6 6"/></svg>
    </button>
  </div>
  {#if open}
    <div class="nt-body" id="cores-tree">
      {#if !card}<p class="nt-note">Open a card to see its cores.</p>
      {:else if $coresError}<p class="nt-note nt-err" role="alert">{$coresError}</p>
      {:else if $coresLoading && !nodes.length}<p class="nt-note" role="status">Reading the card…</p>
      {:else if !nodes.length}<p class="nt-note">No cores on this card.</p>
      {:else if flatGameBub}
        <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
        <ul class="nt-tree" role="tree" aria-label="Installed cores" on:keydown={key}>
          {#each nodes[0].cores as k (k.id)}
            <li role="treeitem" aria-level="1" aria-selected={active && $coresSelection.core === k.id}>
              <button class="nt-item nt-core" class:sel={active && $coresSelection.core === k.id} data-kind="core" data-level="1" data-id={k.id} on:click={() => pickCore(nodes[0].platform.id, k.id)}>
                <span class="nt-name">{k.shortname}</span>{#if k.missing}<span class="nt-dot" role="img" aria-label="Needs a file" title="Needs a file"></span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
        <ul class="nt-tree" role="tree" aria-label="Cores by platform" on:keydown={key}>
          {#each nodes as n (n.platform.id)}
            {@const multi = n.cores.length > 1}
            {@const only = !multi ? n.cores[0] : null}
            {@const hasMissing = n.cores.some((k) => k.missing > 0)}
            <li role="treeitem" aria-level="1" aria-expanded={multi ? platOpen[n.platform.id] : undefined} aria-selected={active && (only ? $coresSelection.core === only.id : $coresSelection.platform === n.platform.id && !$coresSelection.core)}>
              <div class="nt-row">
                <button
                  class="nt-item"
                  class:off={!!only?.disabled}
                  class:sel={active && (only ? $coresSelection.core === only.id : $coresSelection.platform === n.platform.id && !$coresSelection.core)}
                  data-kind={multi ? 'platform' : 'core'}
                  data-level="1"
                  data-id={only ? only.id : n.platform.id}
                  on:click={() => (only ? pickCore(n.platform.id, only.id) : pickPlatform(n.platform.id))}
                >
                  <Icon name={categoryIcon(n.platform.category || 'Other')} size={14} /><span class="nt-name">{n.platform.name}</span>{#if multi}<span class="nt-count">{n.cores.length}</span>{/if}{#if only?.disabled}<span class="nt-off" role="img" aria-label="Disabled on the Pocket" title="Disabled on the Pocket: the Pocket does not list this core">off</span>{/if}{#if hasMissing && (!multi || !platOpen[n.platform.id])}<span class="nt-dot" role="img" aria-label="Needs a file" title="Needs a file"></span>{/if}
                </button>
                {#if multi}
                  <button class="nt-chev" tabindex="-1" aria-label={`${platOpen[n.platform.id] ? 'Collapse' : 'Expand'} ${n.platform.name}`} on:click={() => togglePlat(n.platform.id)}>
                    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class:open={platOpen[n.platform.id]}><path d="m9 6 6 6-6 6"/></svg>
                  </button>
                {/if}
              </div>
              {#if multi && platOpen[n.platform.id]}
                <ul role="group" class="nt-sub">
                  {#each n.cores as k (k.id)}
                    <li role="treeitem" aria-level="2" aria-selected={active && $coresSelection.core === k.id}>
                      <button class="nt-item nt-core" class:off={k.disabled} class:sel={active && $coresSelection.core === k.id} data-kind="core" data-level="2" data-id={k.id} on:click={() => pickCore(n.platform.id, k.id)}>
                        <span class="nt-name">{k.shortname}</span>{#if k.disabled}<span class="nt-off" role="img" aria-label="Disabled on the Pocket" title="Disabled on the Pocket: the Pocket does not list this core">off</span>{/if}{#if k.missing}<span class="nt-dot" role="img" aria-label="Needs a file" title="Needs a file"></span>{/if}
                      </button>
                    </li>
                  {/each}
                </ul>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</div>
<style>
  .nt{display:block;min-width:0;max-width:100%}
  .nt-head{display:flex;align-items:center;gap:2px}
  .nt-head-btn{flex:1;min-width:0}
  .nt-head-chev{margin-top:6px}
  .nt-body{margin:2px 0 4px 6px;padding-left:6px;border-left:1px solid var(--border);min-width:0}
  .nt-note{font-size:12px;color:var(--text-muted);margin:8px 4px;line-height:1.5}.nt-err{color:var(--danger-text)}
  .nt-tree,.nt-sub{list-style:none;margin:0;padding:0}
  .nt-tree{padding-right:2px}
  .nt-sub{margin:0 0 2px 12px}
  .nt-row{display:flex;align-items:center;gap:2px}
  .nt-chev{flex:none;width:26px;height:28px;padding:0;display:grid;place-items:center;background:transparent;color:var(--text-muted);border:1px solid transparent;border-radius:7px}
  .nt-chev:hover{background:var(--bg-elevated);border-color:var(--border-strong);color:var(--accent)}.nt-chev:active{background:var(--bg-btn)}.nt-chev svg{transition:transform .12s}.nt-chev svg.open{transform:rotate(90deg)}
  .nt-item{flex:1;min-width:0;display:flex;align-items:center;justify-content:space-between;gap:6px;padding:6px 8px;font-size:13px;line-height:1.25;border-radius:7px;color:var(--text-secondary);background:transparent;text-align:left}
  .nt-item:hover{background:var(--accent-soft-bg);color:var(--accent);box-shadow:inset 3px 0 0 var(--text-faint)}
  .nt-item:focus-visible,.nt-chev:focus-visible{outline:2px solid var(--accent);outline-offset:-2px}
  .nt-item.sel{background:var(--accent-soft-bg);color:var(--accent);box-shadow:inset 3px 0 0 var(--accent-solid)}
  .nt-core{width:100%}.nt-core.off .nt-name{opacity:.6}.nt-off{flex:none;margin-left:6px;padding:0 6px;border-radius:99px;background:var(--warn-bg);color:var(--warn-text);font-size:10px;line-height:15px;letter-spacing:.04em;text-transform:uppercase}
  .nt-item::global(.ico){flex:none;margin-right:2px;opacity:.85}.nt-item .nt-name{flex:1}
  .nt-name{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .nt-count{flex:none;font-size:11px;color:var(--text-muted);margin-left:4px}
  .nt-dot{flex:none;display:inline-block;width:7px;height:7px;border-radius:50%;background:var(--warn-text);margin-left:4px}
</style>
