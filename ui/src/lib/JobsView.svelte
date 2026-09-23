<script lang="ts">
  import type { Job, JournalSummary } from './types';
  export let jobs: Job[] = [];
  export let journalPath = ''; export let notice = ''; export let chooseJournal: () => void; export let loadJournal: () => void;
  export let startSync: () => void;
  export let reportsDir = ''; export let historyNotice = '';
  export let history: JournalSummary[] = [];
  export let chooseReportsDir: () => void; export let refreshHistory: () => void;
  export let detail: unknown = null; export let detailNotice = '';
  export let openDetail: (path: string) => void; export let closeDetail: () => void;

  const date = (unix: number) => unix ? new Date(unix * 1000).toLocaleString() : '—';
  const KIND_LABELS: Record<string, string> = { sync: 'Library sync', mirror: 'Mirror sync', core_copy: 'Core copy', core_move: 'Core move' };
</script>
<section class="jobs-panel page" aria-labelledby="jobs-title">
  <header><div><p class="eyebrow">ACTIVITY</p><h1 id="jobs-title">Recent jobs</h1><p class="lede">Operations completed during this session, plus the durable history in your reports directory.</p></div><button class="primary" on:click={startSync}>Start a sync</button></header>

  <section class="settings-card">
    <div>
      <h2>Reports directory</h2>
      <p>Every sync, core copy, and core move writes its journal here automatically, so job history survives restarts.</p>
      <div class="picker-row"><input bind:value={reportsDir} placeholder="/Users/me/Documents/Tau Omega Reports" readonly/><button class="picker" on:click={chooseReportsDir}>Choose</button><button class="quiet" on:click={refreshHistory} disabled={!reportsDir}>Refresh</button></div>
      <p class="notice" role="status">{historyNotice}</p>
    </div>
  </section>

  {#if history.length}
    <section class="core-list" aria-label="Job history">
      {#each history as entry}
        <article><div class="core-icon">{entry.state === 'completed' ? '✓' : entry.state === 'failed' ? '!' : '…'}</div><div><h3>{KIND_LABELS[entry.kind] ?? entry.kind}</h3><p>{date(entry.recorded_at_unix)} · {entry.destination} · {entry.copied ?? 0} copied{entry.deleted ? ` · ${entry.deleted} deleted` : ''}</p></div><span class="chip" class:capable={entry.state === 'completed'}>{entry.state}</span><button class="quiet" on:click={() => openDetail(entry.path)}>Details</button></article>
      {/each}
    </section>
  {:else if reportsDir}
    <section class="empty"><div class="empty-art">◷</div><h2>No history yet</h2><p>Reviewed syncs and copies will appear here once one writes to this directory.</p></section>
  {/if}

  {#if detail}
    <section class="detail-overlay" aria-labelledby="detail-title">
      <div class="detail-card">
        <p class="eyebrow">JOURNAL DETAIL</p>
        <h2 id="detail-title">Full report</h2>
        <p class="notice" role="status">{detailNotice}</p>
        <pre class="journal-json">{JSON.stringify(detail, null, 2)}</pre>
        <div class="detail-actions"><button class="quiet" on:click={closeDetail}>Close</button></div>
      </div>
    </section>
  {/if}

  <section class="settings-card"><div><h2>Load a journal from elsewhere</h2><p>Inspect a single journal file outside your configured reports directory.</p><div class="picker-row"><input bind:value={journalPath} placeholder="/Users/me/Documents/tau-sync-report.json"/><button class="picker" on:click={chooseJournal}>Choose</button><button class="primary" on:click={loadJournal}>Load</button></div><p class="notice" role="status">{notice}</p></div></section>

  {#if jobs.length}
    <section class="core-list" aria-label="This session">{#each jobs as job}<article><div class="core-icon">{job.status === 'Completed' ? '✓' : '!'}</div><div><h3>{job.kind}</h3><p>{job.detail}</p></div><span class="chip" class:capable={job.status === 'Completed'}>{job.status}</span></article>{/each}</section>
  {/if}
</section>
<style>
  .journal-json { max-height: 360px; overflow: auto; background: #101617; border: 1px solid #344244; border-radius: 9px; padding: 14px; font-size: 12px; color: #c7d1d0; white-space: pre-wrap; word-break: break-word; }
  .detail-overlay { position: fixed; inset: 0; z-index: 10; display: grid; place-items: center; padding: 24px; background: rgb(5 9 10 / 72%); }
  .detail-card { width: min(640px, 100%); display: grid; gap: 14px; padding: 28px; border: 1px solid #455654; border-radius: 15px; background: #1a2325; box-shadow: 0 24px 80px rgb(0 0 0 / 40%); }
  .detail-card h2 { margin: 0; }
  .detail-actions { display: flex; justify-content: flex-end; }
  .detail-actions button { padding: 11px 14px; background: #314244; color: #e8ecec; }
</style>
