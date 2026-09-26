<script lang="ts">
  import type { Core } from './types';
  import type { ConnectionKind } from './tau-api';
  import CardIcon from './CardIcon.svelte';
  export let path = '';
  export let connectionKinds: Record<string, ConnectionKind> = {};
  export let cardMounted: boolean | null = null;
  export let openKnownCard: (card: string) => void;
  export let knownCards: string[] = [];
  export let mountedCards: string[] = [];
  export let cardName: (path: string) => string;
  export let notice = '';
  export let cores: Core[] = [];
  export let openFolder: () => void;
  export let choose: () => void;
  export let playerCores: Core[] = [];
  export let otherCores: Core[] = [];
  export let platformImages: Record<string, string | null> = {};
  export let coreIcons: Record<string, string | null> = {};
  export let openCoreLibrary: (core: Core) => void;
  export let showCoreDetail: (core: Core) => void;
  export let showOtherCores = false;
  export let toggleManualPlayer: (core: Core) => void;
</script>
<section class="page" id="cards">
  <header><div><p class="eyebrow">CARD LIBRARY</p><h1>Start with a card</h1><p class="lede">Inspect a Pocket card or a staging folder. Your music stays untouched.</p></div><button class="primary" on:click={choose}>Open folder</button></header>
  {#if cardMounted === false}<div class="ejected-banner" role="status"><span>This card is no longer connected.</span><button class="quiet" on:click={() => openKnownCard(path)}>Reconnect</button></div>{/if}
  {#if knownCards.length}<section class="known-cards" aria-labelledby="known-cards-title"><p class="eyebrow">QUICK OPEN</p><h2 id="known-cards-title">Known cards</h2><div class="known-card-list">{#each knownCards as card}<button class="known-card" class:active={path === card} on:click={() => openKnownCard(card)}><span class="known-card-icon" aria-hidden="true"><CardIcon kind={connectionKinds[card]} /></span><span class="known-card-body"><strong class="known-card-name">{cardName(card)}</strong><span class="known-card-badge" class:mounted={mountedCards.includes(card)}>{mountedCards.includes(card) ? 'Available now' : 'Recently used'}</span><span class="known-card-path">{card}</span></span></button>{/each}</div></section>{/if}
  <p class="notice" role="status">{notice}</p>
  {#if cores.length}
    <section aria-labelledby="player-title">
      <div class="section-title"><div><p class="eyebrow">MEDIA PLAYERS</p><h2 id="player-title">Player cores on this card</h2></div><button class="quiet refresh-btn" aria-label="Refresh this card" title="Refresh" on:click={openFolder}><svg viewBox="0 0 24 24" fill="none" width="16" height="16"><path d="M20 12a8 8 0 1 1-2.34-5.66M20 4v5h-5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg></button></div>
      {#if playerCores.length}
        <div class="player-grid">
          {#each playerCores as core}
            <article class="player-card">
              <button class="player-card-main" on:click={() => openCoreLibrary(core)}>
                <div class="player-card-art" aria-hidden="true">{#if platformImages[core.platform]}<img src={platformImages[core.platform]} alt="" />{:else}<span>{(core.shortname || core.id).slice(0, 2).toUpperCase()}</span>{/if}</div>
                <h3>{core.shortname || core.id}</h3>
                <div class="player-card-dev">{#if coreIcons[core.id]}<img class="player-card-dev-icon" src={coreIcons[core.id]} alt="" />{:else}<svg viewBox="0 0 24 24" fill="none" width="13" height="13" aria-hidden="true"><path d="M8 6 3 12l5 6M16 6l5 6-5 6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>{/if}<span>{core.author || 'Unknown developer'}</span></div>
                <p class="player-card-meta">{core.tracks === null ? 'No index yet' : `${core.tracks} tracks`}</p>
              </button>
              <div class="player-card-footer">
                <span class:capable={core.library_capable} class="chip">{core.library_capable ? 'Library ready' : 'Legacy core'}</span>
                <button class="quiet" aria-label={`Details for ${core.id}`} title="Details" on:click={() => showCoreDetail(core)}>
                  <svg viewBox="0 0 24 24" fill="none" width="16" height="16"><circle cx="12" cy="12" r="9" stroke="currentColor" stroke-width="1.6"/><path d="M12 11v5.5M12 8v.01" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>
                </button>
              </div>
            </article>
          {/each}
        </div>
      {:else}<section class="empty"><div class="empty-art">τ</div><h2>No player cores found</h2><p>This card has {cores.length} other core{cores.length === 1 ? '' : 's'} installed. Show them below to set one as a player.</p></section>{/if}
    </section>
    {#if otherCores.length}
      <section aria-labelledby="other-core-title">
        <div class="section-title"><div><p class="eyebrow">EVERYTHING ELSE</p><h2 id="other-core-title">Other cores on this card</h2></div><label class="show-all"><input type="checkbox" bind:checked={showOtherCores}/> Show {otherCores.length} other core{otherCores.length === 1 ? '' : 's'}</label></div>
        {#if showOtherCores}<div class="core-list">{#each otherCores as core}<article><div class="core-icon">{#if coreIcons[core.id]}<img src={coreIcons[core.id]} alt="" />{:else}{core.id.split('.').at(-1)?.[0] ?? '?'}{/if}</div><div><h3>{core.id}</h3><p>{core.author || 'Unknown author'} · {core.version || 'Version unknown'} · {core.platform || 'No platform declared'}</p></div><button class="quiet" on:click={() => toggleManualPlayer(core)}>Set as player</button></article>{/each}</div>{/if}
      </section>
    {/if}
  {:else}<section class="empty"><div class="empty-art">◒</div><h2>No card selected</h2><p>Open a card folder to see its cores and library health.</p></section>{/if}
</section>
