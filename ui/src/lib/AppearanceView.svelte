<script lang="ts">
  // Appearance: edit extra themes for the Pocket (`tau-assets.bin`). The engine checks every theme the same way the
  // firmware's own tool does (names, brightness range, contrast on every panel and every accent the device offers) and
  // shows each colour as the Pocket will draw it (RGB565 snapping). Saving writes a file you choose; putting it on a
  // card is a separate, reviewed step that is not built yet.
  import { onDestroy } from 'svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { appearanceCheck, appearanceExport, appearanceOpen, errorMessage } from './tau-api';
  import { MAX_THEMES, ROLE_GROUPS, newTheme } from './appearance';
  import type { ThemeInput, ThemeReport } from './types';

  let themes: ThemeInput[] = [newTheme('MY THEME')];
  let current = 0;
  let polarity: 'dark' | 'light' = 'dark';
  let report: ThemeReport | null = null;
  let notice = '';
  let busy = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  $: theme = themes[current];
  $: pol = theme[polarity];
  $: snapped = report ? (polarity === 'dark' ? report.dark_snapped : report.light_snapped) : {};
  $: checks = report ? report.checks.filter((c) => c.polarity === polarity) : [];
  $: bad = report ? report.problems.length : 0;

  // Re-check shortly after the last edit; the answer is quick but there is no reason to ask on every keystroke.
  function changed() {
    themes = themes; notice = '';
    clearTimeout(timer);
    timer = setTimeout(recheck, 150);
  }
  async function recheck() {
    const asked = current;
    try { const r = await appearanceCheck(themes[asked]); if (asked === current) report = r; }
    catch (error) { notice = errorMessage(error); }
  }
  onDestroy(() => clearTimeout(timer));
  recheck();

  function pick(i: number) { current = i; report = null; recheck(); }
  function add() {
    if (themes.length >= MAX_THEMES) return;
    let n = themes.length + 1; let name = `MY THEME ${n}`;
    while (themes.some((t) => t.name === name)) name = `MY THEME ${++n}`;
    themes = [...themes, newTheme(name)]; pick(themes.length - 1);
  }
  function remove() {
    if (themes.length < 2) return;
    themes = themes.filter((_, i) => i !== current); pick(Math.min(current, themes.length - 1));
  }
  function setName(value: string) { theme.name = value.toUpperCase(); changed(); }
  function setColor(key: string, value: string) { pol.colors[key] = value.toUpperCase(); changed(); }
  function setLuma(value: number) { pol.bg_luma = value; changed(); }

  async function openFile() {
    notice = '';
    const picked = await open({ multiple: false, filters: [{ name: 'Tau assets', extensions: ['bin'] }] });
    if (typeof picked !== 'string') return;
    busy = true;
    try {
      const loaded = await appearanceOpen(picked);
      if (!loaded.length) { notice = 'That file has no themes in it.'; return; }
      themes = loaded.slice(0, MAX_THEMES); pick(0);
      notice = `Opened ${loaded.length} theme${loaded.length === 1 ? '' : 's'}. Saving writes a new file; anything else in the original file (such as meter presets) is not carried over.`;
    } catch (error) { notice = errorMessage(error); } finally { busy = false; }
  }
  async function exportFile() {
    notice = '';
    const dest = await save({ defaultPath: 'tau-assets.bin', filters: [{ name: 'Tau assets', extensions: ['bin'] }] });
    if (!dest) return;
    busy = true;
    try { const bytes = await appearanceExport(themes, dest); notice = `Saved ${bytes} bytes to ${dest}, read back and checked.`; }
    catch (error) { notice = errorMessage(error); } finally { busy = false; }
  }
  const ratio = (n: number) => n.toFixed(2);
  const label = (c: { text: string; against: string }) => `${c.text.replace('_', ' ')} on ${c.against.replace('ramp', 'background').replace(/\//g, ' / ')}`;
</script>

<section class="ap" aria-labelledby="ap-title">
  <div class="ap-head">
    <h2 id="ap-title">Appearance</h2>
    <p class="muted">Design extra themes for the Pocket. Each has a Dark and a Light version. Colours are shown as the Pocket will draw them.</p>
  </div>

  <div class="ap-bar">
    <div role="tablist" aria-label="Themes" class="ap-tabs">
      {#each themes as t, i (i)}
        <button role="tab" aria-selected={i === current} class:on={i === current} on:click={() => pick(i)}>{t.name || '(unnamed)'}</button>
      {/each}
      <button class="quiet" disabled={themes.length >= MAX_THEMES} on:click={add} title={themes.length >= MAX_THEMES ? `A file holds up to ${MAX_THEMES} themes.` : ''}>+ New theme</button>
    </div>
    <div class="ap-actions">
      <button class="quiet" disabled={busy} on:click={openFile}>Open file…</button>
      <button class="primary" disabled={busy || bad > 0} title={bad ? 'Fix the problems below first.' : ''} on:click={exportFile}>Save tau-assets.bin…</button>
    </div>
  </div>
  {#if notice}<p class="ap-notice" role="status">{notice}</p>{/if}

  <div class="ap-grid">
    <div class="ap-edit">
      <label class="ap-field">Name
        <input type="text" value={theme.name} maxlength="15" spellcheck="false" on:input={(e) => setName(e.currentTarget.value)} />
        <span class="muted">Capital letters, digits, space, _ and - (the Pocket’s font has capitals only).</span>
      </label>
      <div class="ap-pol" role="tablist" aria-label="Version">
        <button role="tab" aria-selected={polarity === 'dark'} class:on={polarity === 'dark'} on:click={() => (polarity = 'dark')}>Dark</button>
        <button role="tab" aria-selected={polarity === 'light'} class:on={polarity === 'light'} on:click={() => (polarity = 'light')}>Light</button>
      </div>
      <label class="ap-field">Background brightness: {pol.bg_luma}
        <input type="range" min="20" max="235" value={pol.bg_luma} on:input={(e) => setLuma(+e.currentTarget.value)} />
        <span class="muted">How bright the tinted background is (20 to 235).</span>
      </label>
      {#each ROLE_GROUPS as group}
        <fieldset>
          <legend>{group.title}</legend>
          {#each group.roles as r}
            <div class="ap-role">
              <input type="color" aria-label={`${r.label} colour`} value={pol.colors[r.key]} on:input={(e) => setColor(r.key, e.currentTarget.value)} />
              <span class="ap-name"><b>{r.label}</b><span class="muted">{r.hint}</span></span>
              <code title="As the Pocket will draw it">{snapped[r.key] ?? pol.colors[r.key]}</code>
            </div>
          {/each}
        </fieldset>
      {/each}
    </div>

    <aside class="ap-side" aria-label="Preview and checks">
      <div class="ap-preview" style="background:{snapped.bg_bottom ?? pol.colors.bg_bottom}">
        <div class="ap-card" style="background:{snapped.surface ?? pol.colors.surface}; color:{snapped.text_primary ?? pol.colors.text_primary}">
          <b>Now playing</b>
          <span style="color:{snapped.text_secondary ?? pol.colors.text_secondary}">Artist · Album</span>
          <i style="background:{snapped.surface_track ?? pol.colors.surface_track}"><u style="background:{snapped.ok ?? pol.colors.ok}"></u></i>
          <small style="color:{snapped.faint ?? pol.colors.faint}">track-01.flac</small>
        </div>
        <div class="ap-chrome" style="background:{snapped.chrome ?? pol.colors.chrome}; color:{snapped.text_secondary ?? pol.colors.text_secondary}">B BACK</div>
      </div>
      <p class="muted">A rough sketch with the real colours, not the real layout. The accent colour is chosen on the Pocket.</p>

      <h3>Readability</h3>
      {#if report && !report.problems.length}<p class="ap-ok" role="status">This theme passes every readability check, so it can be saved.</p>{/if}
      {#if report && report.problems.length}
        <ul class="ap-problems" role="alert">{#each report.problems as p}<li>{p}</li>{/each}</ul>
      {/if}
      <table>
        <caption class="sr-only">Contrast for the {polarity} version</caption>
        <thead><tr><th>Check</th><th>Ratio</th><th>Needs</th><th></th></tr></thead>
        <tbody>{#each checks as c}<tr class:low={!c.ok}><td>{label(c)}</td><td>{ratio(c.worst)}</td><td>{c.needed}</td><td>{c.ok ? 'OK' : 'Too faint'}</td></tr>{/each}</tbody>
      </table>
    </aside>
  </div>
  {#if themes.length > 1}<p><button class="quiet" on:click={remove}>Delete “{theme.name}”</button></p>{/if}
</section>

<style>
  .ap-head p{margin:0}
  .ap button{padding:8px 12px;font-size:13px;background:#202b2d;color:#e8ecec;border:1px solid #2c393a}
  .ap button.quiet{background:transparent}
  .ap button.primary{background:#c1f0ad;color:#142015;font-weight:700;border-color:#c1f0ad}
  .ap button:disabled{opacity:.5;cursor:not-allowed}
  .ap input[type=text]{width:100%;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:10px 12px;font:inherit;font-size:13px;box-sizing:border-box}
  .ap input[type=range]{width:100%;accent-color:#c1f0ad}
  .ap-field{font-size:12px;font-weight:650;color:#acbbbb}
  .ap{display:grid;gap:14px;max-width:1100px;align-content:start}
  .ap h2{margin:0}
  .ap-bar{display:flex;gap:12px;flex-wrap:wrap;justify-content:space-between;align-items:center}
  .ap-tabs,.ap-pol,.ap-actions{display:flex;gap:6px;flex-wrap:wrap}
  .ap .ap-tabs button.on,.ap .ap-pol button.on{background:#c1f0ad;color:#142015;border-color:#c1f0ad;font-weight:700}
  .ap-notice{margin:0;padding:8px 12px;border:1px solid #2c393a;background:#161f20;border-radius:8px}
  .ap-grid{display:grid;grid-template-columns:minmax(0,3fr) minmax(280px,2fr);gap:18px;align-items:start}
  @media (max-width:820px){.ap-grid{grid-template-columns:1fr}}
  .ap-edit{display:grid;gap:12px}
  .ap-field{display:grid;gap:4px}
  fieldset{border:1px solid #2c393a;border-radius:10px;padding:8px 12px;display:grid;gap:8px;background:#161f20}
  legend{padding:0 6px;color:#c1f0ad}
  .ap-role{display:grid;grid-template-columns:44px 1fr auto;gap:10px;align-items:center}
  .ap-role input[type=color]{width:44px;height:32px;padding:0;border:1px solid #2c393a;background:none;border-radius:6px}
  .ap-name{display:grid;gap:1px}.ap-name .muted{font-size:12px}
  code{font-size:12px}
  .ap-side{position:sticky;top:8px;display:grid;gap:10px;align-content:start}
  .ap-preview{border:1px solid #2c393a;border-radius:10px;padding:14px;display:grid;gap:10px}
  .ap-card{border-radius:8px;padding:10px;display:grid;gap:4px}
  .ap-card i{display:block;height:6px;border-radius:3px;overflow:hidden}.ap-card u{display:block;height:100%;width:40%}
  .ap-chrome{border-radius:6px;padding:6px 10px;font-size:12px}
  .ap-ok{color:#c1f0ad;margin:0}
  .ap-problems{margin:0;padding-left:18px;color:#ffb4a8}
  table{width:100%;border-collapse:collapse;font-size:13px}
  th,td{text-align:left;padding:3px 6px;border-bottom:1px solid #2c393a}
  tr.low td{color:#ffb4a8}
  .sr-only{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0)}
</style>
