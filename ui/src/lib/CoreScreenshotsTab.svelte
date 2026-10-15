<script lang="ts">
  // Screenshots. The Pocket keeps every screenshot in one flat folder but writes the core (and the loaded game) inside each file, so the
  // view is exact and is the only one: other cores' screenshots are not shown here. A file without that block (older firmware) is only shown as "probably" when it was taken near a save state.
  import { onMount } from 'svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { coreScreenshots, screenshotThumbnail } from './tau-api';
  import { plural } from './coresFormat';
  import type { ScreenshotView, Shot } from './types';
  export let card: string; export let coreId: string;

  let view: ScreenshotView | null = null; let error = ''; let loading = true;
  let thumbs: Record<string, string | null | undefined> = {}; const asked = new Set<string>(); let queue: string[] = []; let running = 0;
  let large: Shot | null = null; let largeUrl = '';

  onMount(async () => {
    try { view = await coreScreenshots(card, coreId); }
    catch (e) { error = errorMessage(e); }
    finally { loading = false; }
  });
  $: shown = view ? view.shots.filter((s) => s.recorded || s.likely) : [];

  function want(file: string) {
    if (thumbs[file] !== undefined || asked.has(file)) return;
    asked.add(file); queue = [...queue, file]; pump();
  }
  function pump() {
    while (running < 3 && queue.length) {
      const f = queue[0]; queue = queue.slice(1); running++;
      screenshotThumbnail(card, f, 260).then((u) => { thumbs = { ...thumbs, [f]: u }; }, () => { thumbs = { ...thumbs, [f]: null }; }).finally(() => { running--; pump(); });
    }
  }
  function visible(node: HTMLElement, file: string) {
    if (typeof IntersectionObserver === 'undefined') { want(file); return {}; }
    const io = new IntersectionObserver((e) => { if (e.some((x) => x.isIntersecting)) { want(file); io.disconnect(); } }, { rootMargin: '200px' });
    io.observe(node);
    return { destroy: () => io.disconnect() };
  }
  async function open(s: Shot) { large = s; largeUrl = thumbs[s.file] ?? ''; try { largeUrl = await screenshotThumbnail(card, s.file, 640); } catch { /* the small one stays */ } }
</script>
<div class="ct">
  {#if loading}<p class="ct-note" role="status">Reading the screenshots…</p>
  {:else if error}<p class="ct-err" role="alert">{error}</p>
  {:else if view}
    {#if shown.length}<p class="ct-note" role="note">Each screenshot records which core took it. {plural(view.recorded, 'screenshot')} from this core{#if view.likely}, plus {plural(view.likely, 'older one')} marked “probably” (no core recorded, taken within {view.window_secs / 60} minutes of a save state){/if}.</p>{/if}
    {#if !shown.length}
      <div class="ct-empty">
        <svg viewBox="0 0 120 96" width="120" height="96" aria-hidden="true"><rect x="14" y="8" width="92" height="80" rx="14" fill="#1d2a2b" stroke="#3d5254" stroke-width="2"/><rect x="26" y="20" width="68" height="40" rx="6" fill="#101617" stroke="#2c393a"/><path d="M40 46l10-10 8 8 6-5 12 11H40z" fill="#2c4a40"/><circle cx="76" cy="31" r="4" fill="#c1f0ad"/><circle cx="46" cy="75" r="4" fill="#2c393a"/><circle cx="60" cy="75" r="4" fill="#2c393a"/><rect x="72" y="72" width="16" height="6" rx="3" fill="#c1f0ad"/></svg>
        <h2>No screenshots from this core yet</h2>
        <p>Say cheese! Take a screenshot on your Pocket while this core is running and it will show up here.</p>
      </div>
    {:else}
      <ul class="ct-grid" aria-label="Screenshots">
        {#each shown as s (s.file)}
          <li use:visible={s.file}>
            <button class="ct-shot" aria-label={`Screenshot taken ${s.taken}${s.recorded ? ', this core' : s.likely ? ', probably this core' : s.meta ? `, ${s.meta.core}` : ''}. Open`} on:click={() => open(s)}>
              {#if thumbs[s.file]}<img src={thumbs[s.file]} alt="" />{:else}<span class="ct-ph" aria-hidden="true"></span>{/if}
              <span class="ct-cap">{s.taken.slice(0, 16)}{#if s.meta} · {s.recorded ? s.meta.loaded_file : s.meta.core}{/if}</span>
              {#if s.recorded}<span class="ct-why" title="The screenshot names this core">this core</span>{:else if s.likely}<span class="ct-why soft" title={s.likely}>probably</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
      {#if view.total > view.shots.length}<p class="ct-note">Showing the newest {view.shots.length} of {view.total}.</p>{/if}
    {/if}
  {/if}
</div>
{#if large}
  <div class="ct-back" role="presentation" on:click={() => (large = null)}></div>
  <div class="ct-big" role="dialog" tabindex="-1" aria-modal="true" aria-label={`Screenshot taken ${large.taken}`} use:modal on:keydown={(e) => e.key === 'Escape' && (large = null)}>
    <header><div><strong>{large.taken}</strong>{#if large.meta}<p class="ct-note">{large.meta.core} {large.meta.version} · {large.meta.platform_name}{large.meta.loaded_file ? ` · ${large.meta.loaded_file}` : ''}</p>{:else if large.likely}<p class="ct-note">{large.likely}</p>{:else}<p class="ct-note">Cannot be tied to a core.</p>{/if}</div><button class="quiet" data-autofocus on:click={() => (large = null)}>Close</button></header>
    {#if largeUrl}<img src={largeUrl} alt={`Screenshot taken ${large.taken}`} />{/if}
  </div>
{/if}
<style>
  .ct{display:grid;gap:12px}
  .ct-note{font-size:12px;color:#95a5a4;line-height:1.55;margin:0}.ct-err{color:#ff9b9b;font-size:13px}
  .ct-grid{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fill,minmax(190px,1fr));gap:12px}
  .ct-shot{display:block;position:relative;width:100%;padding:0;background:#101617;border:1px solid #2c393a;border-radius:10px;overflow:hidden;text-align:left;color:inherit}
  .ct-shot:hover{border-color:#3d5254}.ct-shot:focus-visible{outline:2px solid #c1f0ad;outline-offset:2px}
  .ct-shot img,.ct-ph{display:block;width:100%;aspect-ratio:3/2;object-fit:cover;background:#0c1213}
  .ct-cap{display:block;padding:6px 9px;font-size:11.5px;color:#a6b3b2}
  .ct-why{position:absolute;top:7px;left:7px;font-size:10.5px;font-weight:700;color:#142015;background:#c1f0ad;border-radius:99px;padding:2px 8px}
  .ct-why.soft{background:#e5d9a8}
  .ct-empty{display:grid;justify-items:center;gap:6px;text-align:center;padding:36px 16px;border:1px dashed #2c393a;border-radius:14px;background:#141c1d}.ct-empty h2{margin:6px 0 0;font-size:16px}.ct-empty p{margin:0;max-width:340px;font-size:12.5px;color:#95a5a4;line-height:1.55}
  .ct-back{position:fixed;inset:0;background:rgba(0,0,0,.7);z-index:100}
  .ct-big{position:fixed;z-index:101;top:50%;left:50%;transform:translate(-50%,-50%);width:min(820px,calc(100vw - 32px));max-height:calc(100vh - 40px);overflow:auto;background:#1a2325;border:1px solid #2c393a;border-radius:14px;padding:16px 18px;box-sizing:border-box}
  .ct-big header{display:flex;justify-content:space-between;gap:12px;margin-bottom:10px}.ct-big img{display:block;width:100%;height:auto;border-radius:8px;image-rendering:auto}
</style>
