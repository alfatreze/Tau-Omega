<script lang="ts">
  import type { Problem, ProblemKind } from './types';
  export let path = ''; export let problems: Problem[] | null = null; export let notice = ''; export let loading = false;
  export let choose: () => void; export let scan: () => void;

  const LABELS: Record<ProblemKind, string> = {
    duplicate: 'Duplicate files',
    missing_tag: 'Missing title or artist tags',
    unsupported_format: 'Unsupported tag format',
    path_issue: 'Path issues',
    missing_cover: 'Missing cover art',
  };
  const ORDER: ProblemKind[] = ['duplicate', 'missing_tag', 'unsupported_format', 'path_issue', 'missing_cover'];

  $: byKind = ORDER.map((kind) => ({ kind, items: (problems ?? []).filter((p) => p.kind === kind) })).filter((g) => g.items.length);
</script>
<section class="page" aria-labelledby="problems-title">
  <header><div><p class="eyebrow">LIBRARY HEALTH</p><h1 id="problems-title">Problems</h1><p class="lede">Duplicate content, missing tags, path issues a card sync would rewrite or collide on, and folders with no cover art. Nothing is changed or deleted.</p></div></header>
  <section class="settings-card"><div class="picker-row"><input bind:value={path} placeholder="/Users/me/Music"/><button class="picker" on:click={choose}>Choose</button><button class="primary" disabled={loading} on:click={scan}>{loading ? 'Scanning…' : 'Find problems'}</button></div><p class="notice" role="status">{notice}</p></section>
  {#if problems?.length}
    <section class="comparison-counts problems-counts"><span><b>{problems.length}</b> problems</span><span><b>{byKind.length}</b> categories</span></section>
    {#each byKind as group}
      <section class="settings-card">
        <h2>{LABELS[group.kind]} <span class="chip">{group.items.length}</span></h2>
        <div class="settings-values">
          {#each group.items as item}
            <div><span>{item.files.length} file{item.files.length === 1 ? '' : 's'}</span><strong title={item.files.join(' · ')}>{item.message}: {item.files.join(' · ')}</strong></div>
          {/each}
        </div>
      </section>
    {/each}
  {:else if problems}
    <section class="empty"><div class="empty-art">✓</div><h2>No problems found</h2><p>This media root has no duplicate content, tag, path, or cover issues.</p></section>
  {:else}
    <section class="empty"><div class="empty-art">!</div><h2>No folder scanned</h2><p>Choose a local media root to inspect it.</p></section>
  {/if}
</section>
<style>
  .problems-counts { grid-template-columns: repeat(2, 1fr); max-width: 360px; }
</style>
