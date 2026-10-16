<script lang="ts">
  // The Card page: what is about the card as a whole. Overview (known cards, updates, what needs attention, libraries) and History
  // (every sync). Cores are not listed here: the Cores section has them, grouped like the openFPGA Library.
  import { coreUpdateCount } from './marketStore';
  import { coresGroups, coresTotal, coresTreeStore } from './coresStore';
  import { checkStorageCapacity, listScreenshots } from './tau-api';
  import { plural, size } from './coresFormat';
  import type { VolumeSpace } from './types';
  import { developerTools, roleOf, refreshRole, markTestCard, cardLabel } from './devStore';
  import { errorMessage } from './backend';
  import type { Core } from './types';
  $: if ($developerTools && path) void refreshRole(path);
  $: disabledCount = $coresTreeStore.reduce((n, p) => n + p.cores.filter((c) => c.disabled).length, 0);
  let roleBusy = false; let roleNote = '';
  async function toggleRole(test: boolean) { roleBusy = true; roleNote = ''; try { await markTestCard(path, test); roleNote = test ? 'Marked as a test card.' : 'This card is a Real card again.'; } catch (e) { roleNote = `Nothing was changed. ${errorMessage(e)}`; } finally { roleBusy = false; } }
  export let cardMounted: boolean | null = null; export let ejectedSafely = ''; export let path = '';
  export let knownCards: string[] = []; export let notice = '';
  export let cores: Core[] = [];
  export let mountedCards: string[] = [];
  export let cardName: (cardPath: string) => string;
  export let chooseFolder: (which: 'card') => unknown; export let openFolder: () => unknown; export let openKnownCard: (card: string) => unknown;
  export let openCoreLibrary: (core: Core) => unknown; export let openCore: (id: string) => unknown; export let browseCores: () => unknown; export let openUpdates: () => unknown;
  /** Which tab shows; the page above can switch it (a sync's "View details" opens History). */
  export let tab: 'overview' | 'history' = 'overview';
  const TABS: ['overview' | 'history', string][] = [['overview', 'Overview'], ['history', 'History']];
  $: needing = $coresTreeStore.flatMap((n) => n.cores.filter((c) => c.missing > 0).map((c) => ({ ...c, platformName: n.platform.name })));
  $: libraries = cores.filter((c) => c.library_capable);
  // Card-wide numbers: space on the volume, what is on it, how many screenshots. Read-only; a failure just hides that figure.
  let space: VolumeSpace | null = null; let shots: number | null = null; let statsFor = '';
  $: if (path && statsFor !== path) { statsFor = path; space = null; shots = null; void loadStats(path); }
  async function loadStats(p: string) {
    try { const c = await checkStorageCapacity(p, 0); if (statsFor === p) space = c.space; } catch { /* hidden */ }
    try { const s = await listScreenshots(p); if (statsFor === p) shots = s.length; } catch { /* hidden */ }
  }
  $: usedBytes = space ? space.total_bytes - space.available_bytes : 0;
  $: usedPct = space && space.total_bytes ? Math.min(100, Math.round((usedBytes / space.total_bytes) * 100)) : 0;
  $: tracks = libraries.reduce((n, c) => n + (c.tracks ?? 0), 0);
  $: platformCount = $coresTreeStore.filter((n) => n.platform.id !== '_none').length;
  function tabKey(e: KeyboardEvent) {
    const i = TABS.findIndex(([t]) => t === tab);
    const next = e.key === 'ArrowRight' ? (i + 1) % TABS.length : e.key === 'ArrowLeft' ? (i + TABS.length - 1) % TABS.length : e.key === 'Home' ? 0 : e.key === 'End' ? TABS.length - 1 : -1;
    if (next < 0) return;
    e.preventDefault(); tab = TABS[next][0]; requestAnimationFrame(() => document.getElementById(`card-tab-${tab}`)?.focus());
  }
</script>

<section class="page" id="cards"><header><div><p class="eyebrow">YOUR CARD</p><h1>Card</h1><p class="lede">The card as a whole: what is on it, what needs attention, and what was synced.</p></div><button class="primary" on:click={() => chooseFolder('card')}>Open folder</button></header>
    {#if cardMounted === false}<div class="ejected-banner" role="status"><span>{ejectedSafely && ejectedSafely === path ? 'Safely ejected. You can unplug it now.' : 'This card is no longer connected.'}</span><button class="quiet" on:click={() => openKnownCard(path)}>Reconnect</button></div>{/if}
    {#if knownCards.length}<section class="known-cards" aria-labelledby="known-cards-title"><p class="eyebrow">QUICK OPEN</p><h2 id="known-cards-title">Known cards</h2><div class="known-card-list">{#each knownCards as card}<button class="known-card" class:active={path === card} on:click={() => openKnownCard(card)}><span class="known-card-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none"><rect x="4" y="2" width="16" height="20" rx="3" stroke="currentColor" stroke-width="1.6"/><rect x="7" y="5" width="10" height="7" rx="1" stroke="currentColor" stroke-width="1.4"/><circle cx="9" cy="16.5" r="1.1" fill="currentColor"/><circle cx="15" cy="16.5" r="1.1" fill="currentColor"/><circle cx="12" cy="19.2" r="1.1" fill="currentColor"/></svg></span><span class="known-card-body"><strong class="known-card-name">{cardName(card)}</strong><span class="known-card-badge" class:mounted={mountedCards.includes(card)}>{mountedCards.includes(card) ? 'Available now' : 'Recently used'}</span><span class="known-card-path">{card}</span></span></button>{/each}</div></section>{/if}
    <div class="card-tabs" role="tablist" aria-label="Card sections" tabindex="-1" on:keydown={tabKey}>
      {#each TABS as [id, label]}<button id={`card-tab-${id}`} role="tab" class:current={tab === id} aria-selected={tab === id} aria-controls={`card-panel-${id}`} tabindex={tab === id ? 0 : -1} on:click={() => (tab = id)}>{label}</button>{/each}
    </div>
    <div id={`card-panel-${tab}`} role="tabpanel" aria-labelledby={`card-tab-${tab}`} tabindex="-1">
    {#if tab === 'history'}
      <slot />
    {:else}
    <p class="notice" role="status">{notice}</p>
    {#if cores.length}
      <section class="card-sum" aria-labelledby="card-sum-title">
        <div class="section-title"><div><p class="eyebrow">ON THIS CARD</p><h2 id="card-sum-title">{plural($coresTotal, 'core')} installed</h2></div><span class="sum-actions"><button class="quiet" aria-label="Refresh this card" title="Refresh" on:click={openFolder}>Refresh</button><button class="quiet" on:click={browseCores}>Browse cores</button></span></div>
        <dl class="sum-stats" aria-label="Card totals">
          <div><dt>Cores</dt><dd>{$coresTotal}</dd></div>
          <div><dt>Platforms</dt><dd>{platformCount}</dd></div>
          {#if disabledCount}<div><dt>Disabled</dt><dd><button class="sum-link" aria-label={`${disabledCount} cores disabled on the Pocket. Open the Cores page`} on:click={browseCores}>{disabledCount} <small>on the Pocket</small></button></dd></div>{/if}
          <div><dt>Categories</dt><dd>{$coresGroups.filter((g) => g.name !== 'No platform').length}</dd></div>
          {#if libraries.length}<div><dt>Library tracks</dt><dd>{tracks}</dd></div>{/if}
          <div class:ok={$coreUpdateCount === 0}><dt>Updates</dt><dd>{#if $coreUpdateCount}<button class="sum-link" aria-label={`${$coreUpdateCount} core updates available. Open the updates`} on:click={openUpdates}>{$coreUpdateCount} <small>available</small></button>{:else}0{/if}</dd></div>
          {#if shots !== null}<div><dt>Screenshots</dt><dd>{shots}</dd></div>{/if}
        </dl>
        {#if space}
          <div class="sum-space"><div class="sum-bar" role="img" aria-label={`${size(usedBytes)} used of ${size(space.total_bytes)}, ${size(space.available_bytes)} free`}><span style={`width:${usedPct}%`}></span></div>
            <p>{size(usedBytes)} used of {size(space.total_bytes)} · <strong>{size(space.available_bytes)} free</strong></p></div>
        {/if}
        {#if $developerTools}
          <h3 class="sum-h">Card role</h3>
          <div class="role-row"><p><strong>{$roleOf.role === 'test' ? 'Test card' : 'Real card'}</strong> <small>{$roleOf.role === 'test' ? 'Dev installs go through the usual reviewed plan.' : 'Installing onto it asks you to type its name first.'}</small></p>
            <button class="quiet" role="switch" aria-checked={$roleOf.role === 'test'} aria-label="This is a test card" disabled={roleBusy} on:click={() => toggleRole($roleOf.role !== 'test')}>{$roleOf.role === 'test' ? 'Make this a Real card' : 'This is a test card'}</button></div>
          <p class="sum-note">Saved as a small hidden file, <code>.omega-role</code>, at the card’s root, so the role follows the card. A card without it is always treated as Real.{roleNote ? ' ' + roleNote : ''}</p>
        {/if}
        {#if needing.length}
          <h3 class="sum-h">Needs attention</h3>
          <ul class="sum-list" aria-label="Cores that need a file">{#each needing as c (c.id)}<li><button class="quiet" on:click={() => openCore(c.id)}><strong>{c.shortname}</strong> <small>{c.platformName} · needs {plural(c.missing, 'file')}</small></button></li>{/each}</ul>
        {:else}<p class="sum-ok">Every core has the files it needs.</p>{/if}
        {#if libraries.length}
          <h3 class="sum-h">Music libraries</h3>
          <ul class="sum-list" aria-label="Music libraries">{#each libraries as c (c.id)}<li><button class="quiet" on:click={() => openCoreLibrary(c)}><strong>{c.shortname || c.id}</strong> <small>{c.tracks === null ? 'No index yet' : plural(c.tracks, 'track')} · open library</small></button></li>{/each}</ul>
        {/if}
      </section>
    {:else}<section class="empty"><div class="empty-art">◒</div><h2>No card selected</h2><p>Open a card folder to see its cores and library health.</p></section>{/if}
    {/if}
    </div>
</section>
<style>
  .card-tabs{display:flex;gap:4px;border-bottom:1px solid #2c393a;margin:6px 0 16px}
  .card-tabs [role=tab]{padding:10px 14px;font-size:13.5px;color:#a4b0b0;background:transparent;border-radius:8px 8px 0 0}
  .card-tabs [role=tab]:hover{color:#eff5f4;background:#1a2325}.card-tabs [role=tab].current{color:#c1f0ad;box-shadow:inset 0 -2px 0 #c1f0ad}
  .card-tabs [role=tab]:focus-visible{outline:2px solid #c1f0ad;outline-offset:-2px}
  .card-sum{border:1px solid #2c393a;border-radius:12px;background:#161f20;padding:14px 16px;margin-top:12px}
  .sum-stats{display:grid;grid-template-columns:repeat(auto-fill,minmax(120px,1fr));gap:10px;margin:12px 0 0}.sum-stats>div{background:#141c1d;border:1px solid #222d2f;border-radius:9px;padding:9px 12px}.sum-stats dt{font-size:11.5px;color:#8f9e9d}.sum-stats dd{margin:2px 0 0;font-size:17px;font-weight:700;color:#e8ecec}
  .sum-space{margin:12px 0 0}.sum-space p{margin:6px 0 0;font-size:12.5px;color:#a6b3b2}.sum-bar{height:8px;border-radius:99px;background:#101617;border:1px solid #2c393a;overflow:hidden}.sum-bar span{display:block;height:100%;background:#c1f0ad}
  .sum-link{background:none;border:0;padding:0;font:inherit;font-weight:700;color:#e5d9a8;cursor:pointer;text-align:left}.sum-link small{font-size:11.5px;font-weight:400;color:#c7b97f}.sum-link:hover{text-decoration:underline}.sum-link:focus-visible{outline:2px solid #c1f0ad;outline-offset:2px}
  .sum-actions{display:flex;gap:4px}
  .role-row{display:flex;justify-content:space-between;align-items:center;gap:12px}.role-row p{margin:0;font-size:13px}.role-row small{color:#8f9e9d;font-size:12px;margin-left:6px}.sum-note{font-size:11.5px;color:#8f9e9d;margin:6px 0 0}
  .sum-h{margin:14px 0 6px;font-size:11px;letter-spacing:.09em;color:#9fb3aa;text-transform:uppercase}.sum-ok{font-size:13px;color:#c5f7ad;margin:10px 0 0}
  .sum-list{list-style:none;margin:0;padding:0;display:grid;gap:2px}.sum-list .quiet{width:100%;text-align:left;display:flex;gap:8px;align-items:baseline}.sum-list small{color:#8f9e9d;font-size:12px}
</style>
