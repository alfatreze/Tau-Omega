<script lang="ts">
  // The Cores entry of the side menu: a platform tree (platform -> its cores). Choosing a platform shows all its cores in the page, choosing a core
  // opens it. Keyboard: Up/Down move, Right expands or goes in, Left collapses or goes to the parent, Enter opens.
  import { coresAttention, coresError, coresLoading, coresSelection, coresTotal, coresTreeStore, refreshCores } from './coresStore';
  /** The open card folder (empty when none). */
  export let card = '';
  /** True while the Cores page is the one showing. */
  export let active = false;
  /** Called after a selection so the app can switch to the Cores page. */
  export let show: () => void;

  let open = true; let filter = '';
  let expanded: Record<string, boolean> = {};
  try { open = localStorage.getItem('tau.nav.cores') !== '0'; expanded = JSON.parse(localStorage.getItem('tau.nav.cores.expanded') ?? '{}'); } catch { /* per-viewer convenience */ }
  const save = () => { try { localStorage.setItem('tau.nav.cores', open ? '1' : '0'); localStorage.setItem('tau.nav.cores.expanded', JSON.stringify(expanded)); } catch { /* ignore */ } };

  let lastCard = '\0';
  $: if (card !== lastCard) { lastCard = card; void refreshCores(card); }
  $: needle = filter.trim().toLowerCase();
  $: nodes = $coresTreeStore
    .map((n) => ({ ...n, cores: needle ? n.cores.filter((c) => `${c.id} ${c.shortname} ${n.platform.name}`.toLowerCase().includes(needle)) : n.cores }))
    .filter((n) => !needle || n.cores.length || n.platform.name.toLowerCase().includes(needle));
  const isOpen = (id: string) => !!needle || !!expanded[id] || ($coresSelection.platform === id && !!active);

  function toggleGroup() { open = !open; save(); if (open) { coresSelection.set({ platform: '', core: '' }); show(); } }
  function toggleNode(id: string) { expanded = { ...expanded, [id]: !expanded[id] }; save(); }
  function pickPlatform(id: string) { expanded = { ...expanded, [id]: true }; save(); coresSelection.set({ platform: id, core: '' }); show(); }
  function pickCore(platform: string, core: string) { coresSelection.set({ platform, core }); show(); }

  /** Arrow-key model over the visible rows (platform rows and core rows are all `.nt-item` buttons in document order). */
  function key(e: KeyboardEvent) {
    const items = [...(e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('.nt-item')];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const cur = items[at]; if (!cur) return;
    const go = (el?: HTMLElement) => { if (el) { e.preventDefault(); el.focus(); } };
    if (e.key === 'ArrowDown') go(items[at + 1]);
    else if (e.key === 'ArrowUp') go(items[at - 1]);
    else if (e.key === 'Home') go(items[0]);
    else if (e.key === 'End') go(items[items.length - 1]);
    else if (e.key === 'ArrowRight' && cur.dataset.kind === 'platform') {
      const id = cur.dataset.id!;
      if (!isOpen(id)) { e.preventDefault(); toggleNode(id); } else go(items[at + 1]);
    } else if (e.key === 'ArrowLeft') {
      if (cur.dataset.kind === 'platform' && isOpen(cur.dataset.id!) && !needle) { e.preventDefault(); toggleNode(cur.dataset.id!); }
      else if (cur.dataset.kind === 'core') go(items.slice(0, at).reverse().find((i) => i.dataset.kind === 'platform'));
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
          {#each nodes as n (n.platform.id)}
            <li role="treeitem" aria-expanded={isOpen(n.platform.id)} aria-selected={active && $coresSelection.platform === n.platform.id && !$coresSelection.core}>
              <div class="nt-row">
                <button class="nt-chev" tabindex="-1" aria-label={`${isOpen(n.platform.id) ? 'Collapse' : 'Expand'} ${n.platform.name}`} on:click={() => toggleNode(n.platform.id)}><svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class:open={isOpen(n.platform.id)}><path d="m9 6 6 6-6 6"/></svg></button>
                <button class="nt-item" class:sel={active && $coresSelection.platform === n.platform.id && !$coresSelection.core} data-kind="platform" data-id={n.platform.id} on:click={() => pickPlatform(n.platform.id)}>
                  <span class="nt-name">{n.platform.name}</span><span class="nt-count">{n.cores.length}</span>
                </button>
              </div>
              {#if isOpen(n.platform.id)}
                <ul role="group" class="nt-cores">
                  {#each n.cores as c (c.id)}
                    <li role="treeitem" aria-selected={active && $coresSelection.core === c.id}>
                      <button class="nt-item nt-core" class:sel={active && $coresSelection.core === c.id} data-kind="core" data-id={c.id} on:click={() => pickCore(n.platform.id, c.id)}>
                        <span class="nt-name">{c.shortname}</span>{#if c.missing}<span class="nt-dot" role="img" aria-label="Needs a file" title="Needs a file"></span>{/if}
                      </button>
                    </li>
                  {/each}
                </ul>
              {/if}
            </li>
          {/each}
          {#if needle && !nodes.length}<li class="nt-note">Nothing matches “{filter}”.</li>{/if}
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
  .nt-tree,.nt-cores{list-style:none;margin:0;padding:0}
  .nt-tree{max-height:min(46vh,420px);overflow:auto;padding-right:2px}
  .nt-cores{margin:0 0 2px 22px}
  .nt-row{display:flex;align-items:center;gap:2px}
  .nt-chev{flex:none;width:22px;height:26px;padding:0;display:grid;place-items:center;background:transparent;color:#8f9e9d;border-radius:6px}
  .nt-chev:hover{background:#202b2d;color:#e8ecec}.nt-chev svg{transition:transform .12s}.nt-chev svg.open{transform:rotate(90deg)}
  .nt-item{flex:1;min-width:0;display:flex;align-items:center;justify-content:space-between;gap:8px;padding:6px 8px;font-size:13px;line-height:1.25;border-radius:7px;color:#b4bfbf;background:transparent;text-align:left}
  .nt-item:hover{background:#202b2d;color:#eff5f4}
  .nt-item:focus-visible,.nt-chev:focus-visible{outline:2px solid #c1f0ad;outline-offset:-2px}
  .nt-item.sel{background:#202b2d;color:#c1f0ad}
  .nt-core{width:100%}
  .nt-name{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .nt-count{flex:none;font-size:11px;color:#8f9e9d;margin-left:6px}
  .nt-dot{flex:none;display:inline-block;width:7px;height:7px;border-radius:50%;background:#e5c88a;margin-left:6px}
  .sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
</style>
