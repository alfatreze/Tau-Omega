<script lang="ts">
  // Sync history: every sync, newest first, with what changed, when, and how it
  // ended, in plain words. It is a secondary page: the Library screen offers
  // "View details" after a sync and "See what happened" after a failure, and
  // Settings > Library links here and holds the retention preferences.
  import { explainError } from './backend';
  import type { HistoryEntry } from './types';

  export let entries: HistoryEntry[] = [];
  export let loading = false;
  export let notice = '';
  /** Which entry to show: a journal path, `'latest'`, or empty for the newest. */
  export let select = '';
  export let refresh: () => void;
  export let openSettings: () => void;
  export let startSync: () => void;

  const MB = 1e6, GB = 1e9;
  const size = (n: number) => (n >= GB ? `${(n / GB).toFixed(1)} GB` : n >= MB ? `${Math.round(n / MB)} MB` : `${Math.max(1, Math.round(n / 1000))} KB`);
  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
  const STALE_RUNNING_SECS = 3600;

  type Status = 'ok' | 'failed' | 'cancelled' | 'interrupted' | 'running';
  function statusOf(e: HistoryEntry): Status {
    if (e.state === 'completed') return 'ok';
    if (e.state === 'failed') return e.error_code === 44 ? 'cancelled' : 'failed';
    // A journal still marked running long after it started means the app closed or the card went away mid-run.
    return Date.now() / 1000 - e.recorded_at_unix > STALE_RUNNING_SECS ? 'interrupted' : 'running';
  }
  const STATUS_LABEL: Record<Status, string> = { ok: 'Completed', failed: 'Didn’t finish', cancelled: 'Cancelled', interrupted: 'Interrupted', running: 'In progress' };
  const STATUS_ICON: Record<Status, string> = { ok: '✓', failed: '!', cancelled: '–', interrupted: '!', running: '…' };
  const KIND_LABEL: Record<string, string> = { library_changes: 'Library sync', sync: 'Library sync', mirror: 'Mirror sync', core_copy: 'Core copy', core_move: 'Core move' };

  /** One line saying what the run did, from the stored context when there is one. */
  function describe(e: HistoryEntry): string {
    const items = e.context?.items ?? [];
    if (items.length) {
      const n = (k: string) => items.filter((i) => i.kind === k).length;
      return [n('add') && `Added ${plural(n('add'), 'album')}`, n('remove') && `Removed ${plural(n('remove'), 'album')}`, n('edit') && `Edited ${plural(n('edit'), 'album')}`].filter(Boolean).join(' · ');
    }
    const parts = [e.copied && `Copied ${plural(e.copied, 'file')}`, e.deleted && `removed ${plural(e.deleted, 'file')}`, e.edited && `edited ${plural(e.edited, 'file')}`].filter(Boolean);
    return parts.length ? String(parts.join(', ')).replace(/^./, (c) => c.toUpperCase()) : (KIND_LABEL[e.kind] ?? e.kind);
  }
  const when = (unix: number) => new Date(unix * 1000).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });
  function ago(unix: number): string {
    const s = Math.max(0, Date.now() / 1000 - unix);
    if (s < 90) return 'just now';
    if (s < 5400) return `${Math.round(s / 60)} min ago`;
    if (s < 86400 * 1.5) return `${Math.round(s / 3600)} h ago`;
    return `${Math.round(s / 86400)} days ago`;
  }
  const took = (secs: number | null) => (secs == null ? '' : secs < 60 ? `${Math.max(1, secs)} sec` : `${Math.round(secs / 60)} min`);
  /** What is and isn't safe after a run stopped, by how far it got. */
  function reassure(e: HistoryEntry): string {
    const phase = e.phase ?? 'copy';
    if (phase === 'remove') return 'Your additions and edits were applied. Some removals may not have finished; anything that was removed is in your backup folder.';
    if (phase === 'edit') return 'Your albums were copied, but the tag edits did not finish. Nothing was removed.';
    return 'Your existing music on the Pocket is safe. The Pocket’s library list is only updated after every file is copied and checked, so it still plays what it played before. Files copied before it stopped were kept.';
  }

  let filter: 'all' | 'problems' = 'all';
  $: problems = entries.filter((e) => ['failed', 'interrupted'].includes(statusOf(e)));
  $: shown = filter === 'problems' ? problems : entries;
  let chosen = '';
  // `select` says which entry to open on. It is applied once per value, so it never overrides the user's own choice afterwards.
  let appliedSelect = '\u0000';
  function applySelect(want: string, list: HistoryEntry[]) {
    appliedSelect = want;
    const path = want && want !== 'latest' ? list.find((e) => e.path === want)?.path : undefined;
    chosen = path ?? list[0].path;
    if (want) filter = 'all';
  }
  $: if (entries.length && select !== appliedSelect) applySelect(select, entries);
  $: if (entries.length && !entries.some((e) => e.path === chosen)) chosen = entries[0].path;
  $: current = entries.find((e) => e.path === chosen) ?? null;
</script>
<section class="page hist" aria-labelledby="hist-title">
  <header>
    <div><p class="eyebrow">ACTIVITY</p><h1 id="hist-title">Sync history</h1><p class="lede">Every sync, newest first: what changed, when, and whether it finished.</p></div>
    <div class="hist-actions"><button class="quiet" on:click={refresh}>Refresh</button><button class="quiet" on:click={openSettings}>History settings</button></div>
  </header>
  <p class="hist-note" role="status">{loading ? 'Loading…' : notice}</p>

  {#if entries.length}
    <div class="hist-filters" role="group" aria-label="Show">
      <button class:on={filter === 'all'} aria-pressed={filter === 'all'} on:click={() => (filter = 'all')}>All ({entries.length})</button>
      <button class:on={filter === 'problems'} aria-pressed={filter === 'problems'} on:click={() => (filter = 'problems')}>Problems ({problems.length})</button>
    </div>
    <div class="hist-grid">
      <ul class="hist-list" aria-label="Syncs">
        {#each shown as e (e.path)}
          {@const st = statusOf(e)}
          <li>
            <button class="hist-row" class:cur={e.path === chosen} aria-current={e.path === chosen ? 'true' : undefined} on:click={() => (chosen = e.path)}>
              <span class="hist-icon {st}" aria-hidden="true">{STATUS_ICON[st]}</span>
              <span class="hist-main"><strong>{describe(e)}</strong><small>{STATUS_LABEL[st]} · {ago(e.recorded_at_unix)}{e.context?.card ? ` · ${e.context.card}` : ''}</small></span>
            </button>
          </li>
        {:else}
          <li class="hist-none">No problems in this history.</li>
        {/each}
      </ul>

      {#if current}
        {@const st = statusOf(current)}
        <article class="hist-detail" aria-labelledby="hist-d-title" tabindex="-1">
          <p class="eyebrow">{KIND_LABEL[current.kind] ?? current.kind}</p>
          <h2 id="hist-d-title">{describe(current)}</h2>
          <p class="hist-status {st}"><span aria-hidden="true">{STATUS_ICON[st]}</span> {STATUS_LABEL[st]} · {when(current.recorded_at_unix)}{took(current.duration_secs) ? ` · took ${took(current.duration_secs)}` : ''}</p>

          {#if st === 'failed' || st === 'interrupted' || st === 'cancelled'}
            <div class="hist-callout" role="note">
              <b>{st === 'cancelled' ? 'You stopped this sync.' : st === 'interrupted' ? 'This sync was interrupted (the app closed or the card was removed).' : 'What happened'}</b>
              {#if st === 'failed' && current.error}<p>{explainError({ code: current.error_code ?? 0, message: current.error })}</p>{/if}
              <p class="hist-safe">{reassure(current)}</p>
              <p>To finish the job, open Library and press Start sync again; already-copied files are skipped.</p>
              <button class="primary" on:click={startSync}>Go to Library</button>
            </div>
          {/if}

          <dl class="hist-facts">
            {#if current.context?.card}<div><dt>Card</dt><dd>{current.context.card}{current.context.core ? ` · ${current.context.core}` : ''}</dd></div>{/if}
            {#if current.context?.connection}<div><dt>Connection</dt><dd>{current.context.connection === 'direct_usb' ? 'Direct to the Pocket (slow)' : current.context.connection === 'card_reader' ? 'Card reader' : 'Unknown'}</dd></div>{/if}
            {#if current.bytes_written}<div><dt>Copied</dt><dd>{size(current.bytes_written)}{current.bytes_per_sec ? ` at about ${(current.bytes_per_sec / MB).toFixed(1)} MB/s` : ''}</dd></div>{/if}
            {#if current.copied != null && st === 'ok'}<div><dt>Result</dt><dd>{[current.copied && `${plural(current.copied, 'track')} copied`, current.deleted && `${plural(current.deleted, 'track')} removed`, current.edited && `${plural(current.edited, 'track')} edited`].filter(Boolean).join(', ') || 'Already up to date'}</dd></div>{/if}
          </dl>

          {#if current.context?.items?.length}
            <h3>What changed</h3>
            <ul class="hist-items">
              {#each current.context.items as it}
                <li><span class="k {it.kind}" aria-hidden="true">{it.kind === 'add' ? '+' : it.kind === 'remove' ? '−' : '✎'}</span>
                  <span><b>{it.label ?? it.title}</b>{#if it.kind !== 'edit'} <small>{it.artist || 'Unknown artist'} · {plural(it.tracks ?? 0, 'track')}{it.bytes ? ` · ${size(it.bytes)}` : ''}</small>{/if}</span></li>
              {/each}
            </ul>
          {/if}

          <details class="hist-tech">
            <summary>Technical details</summary>
            <dl>
              <div><dt>Report file</dt><dd class="mono">{current.path}</dd></div>
              <div><dt>Plan</dt><dd class="mono">{current.plan_id.slice(0, 16)}…</dd></div>
              {#if current.phase}<div><dt>Stopped or finished at</dt><dd>{current.phase}</dd></div>{/if}
              {#if current.error}<div><dt>Error {current.error_code ?? ''}</dt><dd class="mono">{current.error}</dd></div>{/if}
            </dl>
          </details>
        </article>
      {/if}
    </div>
  {:else if !loading}
    <section class="empty"><div class="empty-art">◷</div><h2>No syncs yet</h2><p>When you sync the Pocket, a record of what changed and whether it finished will appear here.</p><button class="primary" on:click={startSync}>Go to Library</button></section>
  {/if}
</section>
<style>
  .hist-actions{display:flex;gap:8px}
  .hist-note{min-height:1.2em;font-size:12px;color:#9cacab;margin:0 0 12px}
  .hist-filters{display:flex;gap:6px;margin-bottom:14px}
  .hist-filters button{background:#1a2325;color:#a6b3b2;border:1px solid #2c393a;border-radius:99px;padding:6px 12px;font-size:12px}
  .hist-filters button.on{background:#1d2c22;color:#c1f0ad;border-color:#2f4a37}
  .hist-grid{display:grid;grid-template-columns:minmax(260px,360px) 1fr;gap:18px;align-items:start}
  @media(max-width:980px){.hist-grid{grid-template-columns:1fr}}
  .hist-list{list-style:none;margin:0;padding:0;display:grid;gap:6px}
  .hist-none{color:#8c9c9b;font-size:13px;padding:14px}
  .hist-row{display:flex;gap:12px;align-items:center;width:100%;text-align:left;background:#161f20;border:1px solid #2c393a;border-radius:11px;padding:11px 12px;color:#e8ecec}
  .hist-row:hover{border-color:#3b4a4b}.hist-row.cur{background:#1d2c22;border-color:#2f4a37}
  .hist-icon{flex-shrink:0;width:28px;height:28px;border-radius:50%;display:grid;place-items:center;font-weight:800;background:#234029;color:#c5f7ad}
  .hist-icon.failed,.hist-icon.interrupted{background:#4a2a24;color:#f4cfc4}.hist-icon.cancelled,.hist-icon.running{background:#273032;color:#b7c3c2}
  .hist-main{display:flex;flex-direction:column;gap:2px;min-width:0}.hist-main strong{font-size:14px}.hist-main small{font-size:12px;color:#8c9c9b}
  .hist-detail{background:#161f20;border:1px solid #2c393a;border-radius:14px;padding:20px 22px;min-width:0}
  .hist-detail h2{margin:2px 0 6px}.hist-detail h3{font-size:14px;margin:18px 0 8px}
  .hist-status{margin:0 0 14px;font-size:13px;color:#c5f7ad}.hist-status.failed,.hist-status.interrupted{color:#f4cfc4}.hist-status.cancelled,.hist-status.running{color:#b7c3c2}
  .hist-callout{background:#2a1d1a;border:1px solid #5a352c;border-radius:11px;padding:12px 14px;margin:0 0 14px;font-size:13px;color:#f4cfc4}
  .hist-callout p{margin:6px 0;color:#e6c4b9}.hist-callout .hist-safe{color:#c1f0ad}
  .hist-facts{margin:0;display:grid;gap:6px}.hist-facts div{display:flex;justify-content:space-between;gap:16px;font-size:13px}.hist-facts dt{color:#8c9c9b}.hist-facts dd{margin:0;text-align:right}
  .hist-items{list-style:none;margin:0;padding:0;display:grid;gap:6px;font-size:13px}.hist-items li{display:flex;gap:8px}.hist-items small{color:#8c9c9b;margin-left:8px}
  .hist-items .k{font-weight:700;width:14px;text-align:center}.hist-items .k.add{color:#c1f0ad}.hist-items .k.remove{color:#e8b59f}
  .hist-tech{margin-top:18px;font-size:12px;color:#8c9c9b}.hist-tech summary{cursor:pointer}.hist-tech dl{margin:8px 0 0;display:grid;gap:6px}.hist-tech div{display:flex;gap:12px;justify-content:space-between}.hist-tech dd{margin:0;text-align:right;word-break:break-all}
  .mono{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:11px}
</style>
