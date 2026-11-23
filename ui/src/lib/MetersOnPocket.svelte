<script lang="ts">
  // The meters on the card, read only: for each Tau core, which list the Pocket will use (your meter file, the core's own meters, or only Winamp Bars)
  // and the meters in it, in the order the Pocket shows them. Omega only READS here. Choosing which meters are on the Pocket (and adding a meter) is not
  // built yet; nothing on this page changes the card.
  import { metersOnCard } from './tau-api';
  import type { CoreMeters, MeterBinView, MeterRowView, MetersOnCard } from './types';
  /** The card folder currently open (empty when none). */
  export let card = '';

  let data: MetersOnCard | null = null; let loading = false; let failed = false; let askedFor = '';
  $: if (card && card !== askedFor) { askedFor = card; void load(); }
  $: if (!card) { data = null; askedFor = ''; }
  async function load() { loading = true; failed = false; try { data = await metersOnCard(card); } catch { data = null; failed = true; } finally { loading = false; } }

  const listFor = (c: CoreMeters, d: MetersOnCard): MeterBinView | null => (c.uses === 'user_file' ? d.user_file : c.uses === 'own_bundle' ? c.bundle : null);
  const usesText = (c: CoreMeters) => (c.uses === 'user_file' ? 'Uses your meter file' : c.uses === 'own_bundle' ? 'Uses the meters that come with it' : 'Only the built-in Winamp Bars');
  const shown = (l: MeterBinView | null) => (l ? l.rows.filter((r) => r.visible) : []);
  const hidden = (l: MeterBinView | null) => (l ? l.rows.filter((r) => !r.visible).length : 0);
  const note = (r: MeterRowView) => [r.fallback ? 'Built into the Pocket: always on' : '', r.unavailable ? `Greyed out: ${r.unavailable}` : '', r.tuning_lost ? 'Cannot remember tuned settings: another meter has a similar id' : ''].filter(Boolean);
</script>

<section class="ml" aria-labelledby="ml-title" data-testid="meters-on-pocket">
  <header class="ml-head">
    <h2 id="ml-title">Meters on the Pocket</h2>
    <p class="ml-sub">The meters your Pocket can show, in its order, read from your card. This page only reads: nothing here changes the card. Choosing which meters are on the Pocket comes later.</p>
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
  .ml-head{display:block;margin:0}.ml h2{margin:0 0 4px;font-size:18px}.ml-sub{margin:0;color:#aab8b7;font-size:13px;max-width:62ch;line-height:1.5}
  .ml-note{margin:0;color:#aab8b7;font-size:13px}
  .ml-core{display:grid;gap:8px;padding:14px 16px;border:1px solid #2c393a;border-radius:12px;background:#161f20}
  .ml-core h3{margin:0;font-size:15px}.ml-core h3 small{color:#95a5a4;font-weight:500;font-size:12px}
  .ml-uses{margin:0;color:#c7d1d0;font-size:13px;line-height:1.55}
  .ml-list{list-style:none;margin:0;padding:0;display:grid;gap:6px}
  .ml-list li{display:flex;align-items:center;gap:12px;padding:8px 12px;border:1px solid #2c393a;border-radius:10px;background:#1a2325}
  .ml-list li.off{opacity:.6}
  .ml-n{flex:none;width:22px;height:22px;border-radius:99px;background:#26302f;color:#c7d1d0;font-size:11.5px;display:grid;place-items:center}
  .ml-name{display:flex;flex-wrap:wrap;align-items:baseline;gap:2px 10px;min-width:0}.ml-name b{font-size:13.5px}.ml-name small{font-size:11.5px;color:#95a5a4}.ml-name .ml-flag{color:#e5c88a}
  .ml-small{margin:0;font-size:12px;color:#95a5a4}code{font-size:11.5px;overflow-wrap:anywhere}
</style>
