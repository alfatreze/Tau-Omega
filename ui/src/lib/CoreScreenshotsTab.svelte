<script lang="ts">
  // Screenshots. The Pocket keeps every screenshot in one flat folder and does not record which core took it, so this tab says so and
  // shows two honest views: "Likely this core" (taken within minutes of one of its save states, with the reason) and "All screenshots".
  import { onMount } from 'svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { coreScreenshots, screenshotThumbnail } from './tau-api';
  import { plural } from './coresFormat';
  import type { ScreenshotView, Shot } from './types';
  export let card: string; export let coreId: string;

  let view: ScreenshotView | null = null; let error = ''; let loading = true;
  let mode: 'likely' | 'all' = 'likely';
  let thumbs: Record<string, string | null | undefined> = {}; const asked = new Set<string>(); let queue: string[] = []; let running = 0;
  let large: Shot | null = null; let largeUrl = '';

  onMount(async () => {
    try { view = await coreScreenshots(card, coreId); if (!view.likely) mode = 'all'; }
    catch (e) { error = errorMessage(e); }
    finally { loading = false; }
  });
  $: shown = view ? (mode === 'likely' ? view.shots.filter((s) => s.likely) : view.shots) : [];

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
    <div class="ct-bar">
      <div class="ct-modes" role="group" aria-label="Which screenshots">
        <button class="chip toggle" aria-pressed={mode === 'likely'} on:click={() => (mode = 'likely')}>Likely this core <span class="ct-n">{view.likely}</span></button>
        <button class="chip toggle" aria-pressed={mode === 'all'} on:click={() => (mode = 'all')}>All screenshots <span class="ct-n">{view.total}</span></button>
      </div>
    </div>
    <p class="ct-note" role="note">The Pocket does not record which core took a screenshot. “Likely this core” are the ones taken within {view.window_secs / 60} minutes of one of this core's {plural(view.states, 'save state')}.</p>
    {#if !shown.length}
      <div class="empty"><h2>{mode === 'likely' ? 'No screenshots can be tied to this core' : 'No screenshots on this card'}</h2><p>{mode === 'likely' ? 'Switch to All screenshots to see everything on the card.' : 'Press the screenshot buttons on the Pocket and they appear here.'}</p></div>
    {:else}
      <ul class="ct-grid" aria-label="Screenshots">
        {#each shown as s (s.file)}
          <li use:visible={s.file}>
            <button class="ct-shot" aria-label={`Screenshot taken ${s.taken}${s.likely ? ', likely this core' : ''}. Open`} on:click={() => open(s)}>
              {#if thumbs[s.file]}<img src={thumbs[s.file]} alt="" />{:else}<span class="ct-ph" aria-hidden="true"></span>{/if}
              <span class="ct-cap">{s.taken.slice(0, 16)}</span>
              {#if s.likely}<span class="ct-why" title={s.likely}>likely this core</span>{/if}
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
    <header><div><strong>{large.taken}</strong>{#if large.likely}<p class="ct-note">{large.likely}</p>{:else}<p class="ct-note">Cannot be tied to a core.</p>{/if}</div><button class="quiet" data-autofocus on:click={() => (large = null)}>Close</button></header>
    {#if largeUrl}<img src={largeUrl} alt={`Screenshot taken ${large.taken}`} />{/if}
  </div>
{/if}
<style>
  .ct{display:grid;gap:12px}.ct-bar{display:flex;justify-content:space-between;gap:12px}.ct-modes{display:flex;gap:8px;flex-wrap:wrap}
  .chip.toggle{cursor:pointer;border:1px solid transparent;padding:7px 12px;font-size:12px}.chip.toggle[aria-pressed="true"]{border-color:#c1f0ad;color:#c5f7ad;background:#16292a}
  .ct-n{opacity:.65;margin-left:4px}.ct-note{font-size:12px;color:#95a5a4;line-height:1.55;margin:0}.ct-err{color:#ff9b9b;font-size:13px}
  .ct-grid{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fill,minmax(190px,1fr));gap:12px}
  .ct-shot{display:block;position:relative;width:100%;padding:0;background:#101617;border:1px solid #2c393a;border-radius:10px;overflow:hidden;text-align:left;color:inherit}
  .ct-shot:hover{border-color:#3d5254}.ct-shot:focus-visible{outline:2px solid #c1f0ad;outline-offset:2px}
  .ct-shot img,.ct-ph{display:block;width:100%;aspect-ratio:3/2;object-fit:cover;background:#0c1213}
  .ct-cap{display:block;padding:6px 9px;font-size:11.5px;color:#a6b3b2}
  .ct-why{position:absolute;top:7px;left:7px;font-size:10.5px;font-weight:700;color:#142015;background:#c1f0ad;border-radius:99px;padding:2px 8px}
  .ct-back{position:fixed;inset:0;background:rgba(0,0,0,.7);z-index:100}
  .ct-big{position:fixed;z-index:101;top:50%;left:50%;transform:translate(-50%,-50%);width:min(820px,calc(100vw - 32px));max-height:calc(100vh - 40px);overflow:auto;background:#1a2325;border:1px solid #2c393a;border-radius:14px;padding:16px 18px;box-sizing:border-box}
  .ct-big header{display:flex;justify-content:space-between;gap:12px;margin-bottom:10px}.ct-big img{display:block;width:100%;height:auto;border-radius:8px;image-rendering:auto}
</style>
