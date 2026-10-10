<script lang="ts">
  // Halcyon EQ: the user presets the Pocket loads from `tau-assets.bin` (the `PRST` section). A preset is either six
  // control positions (edited here) or raw filters imported from an Equalizer APO / AutoEQ profile. The engine owns
  // every safety rule (names, ranges, a preamp that only turns the sound down, stable filters) because the Pocket does
  // not check them; this page shows what it says in plain words. Saving writes a file you choose; putting it on a card
  // is a separate, reviewed step. Themes and meter presets already in the file are kept untouched.
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { halcyonCurve, halcyonExport, halcyonImportApo, halcyonInstall, halcyonOpen, halcyonPlanInstall, halcyonValidate, errorMessage, getPrefs } from './tau-api';
  import { modal } from './a11y';
  import type { ApoImport, AssetsInstallPlan, HalcyonPreset } from './types';

  export let mediaRoot = '';
  export let cardLabel = '';

  const MAX = 8;
  const CONTROLS = [
    { key: 0, label: 'Warmth', min: -5, max: 5, hint: 'Low-mid body.' },
    { key: 1, label: 'Bass', min: -5, max: 5, hint: 'Low end.' },
    { key: 2, label: 'Vocal', min: -5, max: 5, hint: 'Presence of voices.' },
    { key: 3, label: 'Punch', min: -5, max: 5, hint: 'Attack and impact.' },
    { key: 4, label: 'Sibilance', min: 0, max: 5, hint: 'How much “s” sounds are tamed (0 = off).' },
    { key: 5, label: 'Air', min: -5, max: 5, hint: 'High-frequency sparkle.' },
  ];

  let presets: HalcyonPreset[] = [];
  let current = 0;
  let notice = '';
  let problem = '';
  let busy = false;
  let curve: [number, number][] | null = null;
  let lastImport: ApoImport | null = null;
  let openedPath: string | null = null;

  $: preset = presets[current] as HalcyonPreset | undefined;

  async function revalidate() {
    presets = presets; notice = '';
    if (!presets.length) { problem = ''; curve = null; return; }
    try { await halcyonValidate(presets); problem = ''; } catch (error) { problem = errorMessage(error); }
    const asked = current;
    const p = presets[asked];
    if (p) { try { const c = await halcyonCurve(p); if (asked === current) curve = c; } catch { curve = null; } } else curve = null;
  }
  function pick(i: number) { current = i; curve = null; revalidate(); }
  function uniqueName(base: string) {
    let name = base; let n = 1;
    while (presets.some((p) => p.name === name)) name = `${base.slice(0, 13)} ${++n}`;
    return name;
  }
  function addControl() {
    if (presets.length >= MAX) return;
    presets = [...presets, { kind: 'control', name: uniqueName('MY PRESET'), controls: [0, 0, 0, 0, 0, 0] }];
    pick(presets.length - 1);
  }
  function remove() {
    presets = presets.filter((_, i) => i !== current);
    pick(Math.max(0, Math.min(current, presets.length - 1)));
  }
  function setName(value: string) { if (preset) { preset.name = value.toUpperCase(); revalidate(); } }
  function setControl(i: number, value: number) { if (preset && preset.kind === 'control') { preset.controls[i] = value; revalidate(); } }

  async function openFile() {
    notice = '';
    const picked = await open({ multiple: false, filters: [{ name: 'Tau assets', extensions: ['bin'] }] });
    if (typeof picked !== 'string') return;
    busy = true;
    try {
      const loaded = await halcyonOpen(picked);
      presets = loaded; openedPath = picked; lastImport = null; pick(0);
      notice = loaded.length ? `Opened ${loaded.length} preset${loaded.length === 1 ? '' : 's'}. Saving keeps the themes and everything else in the file unchanged.` : 'That file has no Halcyon presets yet. Saving keeps everything already in it.';
    } catch (error) { notice = errorMessage(error); } finally { busy = false; }
  }

  let fileInput: HTMLInputElement;
  async function importApo(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0]; input.value = '';
    if (!file) return;
    if (presets.length >= MAX) { notice = `A file holds up to ${MAX} presets.`; return; }
    busy = true; notice = '';
    try {
      const base = file.name.replace(/\.[^.]*$/, '').toUpperCase().replace(/[^A-Z0-9 _-]/g, ' ').replace(/\s+/g, ' ').trim().slice(0, 15) || 'IMPORTED';
      const imp = await halcyonImportApo(uniqueName(base), await file.text());
      presets = [...presets, imp.preset]; lastImport = imp; pick(presets.length - 1);
      notice = `Imported ${imp.filters} filter${imp.filters === 1 ? '' : 's'}. The loudest boost is ${imp.peak_gain_db.toFixed(1)} dB, so the volume is lowered by ${(-imp.preamp_db).toFixed(1)} dB to leave headroom (the Pocket cannot turn it back up).`;
    } catch (error) { notice = errorMessage(error); } finally { busy = false; }
  }

  async function exportFile() {
    notice = '';
    const dest = await save({ defaultPath: 'tau-assets.bin', filters: [{ name: 'Tau assets', extensions: ['bin'] }] });
    if (!dest) return;
    busy = true;
    try { const bytes = await halcyonExport(presets, dest, openedPath); notice = `Saved ${bytes} bytes to ${dest}, read back and checked (themes and other sections kept).`; }
    catch (error) { notice = errorMessage(error); } finally { busy = false; }
  }

  // ---- install to the card: plan, review, confirm ----
  let plan: AssetsInstallPlan | null = null;
  let reviewing = false; let planning = false; let installing = false; let installError = '';
  async function startInstall() {
    notice = ''; installError = ''; planning = true;
    try { plan = await halcyonPlanInstall(presets, mediaRoot); reviewing = true; }
    catch (error) { notice = errorMessage(error); } finally { planning = false; }
  }
  async function confirmInstall() {
    if (!plan) return;
    installing = true; installError = '';
    try {
      const prefs = await getPrefs();
      const backup = prefs.remove_mode === 'none' ? null : (prefs.backup_dir || prefs.default_backup_dir || null);
      const r = await halcyonInstall(presets, mediaRoot, plan.id, backup);
      reviewing = false; plan = null;
      notice = `Installed on ${cardLabel || 'the card'} (${r.bytes_written} bytes), read back and checked.${r.backup ? ` The file it replaced is saved at ${r.backup}.` : ''} Eject the card safely, start the Tau core and look for your presets in the Halcyon EQ list.`;
    } catch (error) { installError = errorMessage(error); } finally { installing = false; }
  }
  const closeReview = () => { if (!installing) { reviewing = false; plan = null; installError = ''; } };

  // Response curve: 20 Hz to 20 kHz on a log axis, +-18 dB.
  const CW = 520, CH = 170, RANGE = 18;
  $: path = curve ? curve.map(([f, db], i) => `${i ? 'L' : 'M'}${(Math.log10(f / 20) / 3 * CW).toFixed(1)},${(CH / 2 - Math.max(-RANGE, Math.min(RANGE, db)) / RANGE * (CH / 2)).toFixed(1)}`).join(' ') : '';
  const ticks = [[100, '100'], [1000, '1k'], [10000, '10k']] as const;
  revalidate();
</script>

<section class="hx" aria-labelledby="hx-title">
  <div class="hx-head">
    <h2 id="hx-title">Halcyon EQ</h2>
    <p class="muted">Your own sound presets for the Pocket. Make one from six simple controls, or import an Equalizer APO / AutoEQ headphone profile.</p>
  </div>

  <div class="hx-bar">
    <div role="tablist" aria-label="Presets" class="hx-tabs">
      {#each presets as p, i (i)}
        <button role="tab" aria-selected={i === current} class:on={i === current} on:click={() => pick(i)}>{p.name || '(unnamed)'}</button>
      {/each}
      <button class="quiet" disabled={presets.length >= MAX} on:click={addControl} title={presets.length >= MAX ? `A file holds up to ${MAX} presets.` : ''}>+ New preset</button>
      <button class="quiet" disabled={busy || presets.length >= MAX} on:click={() => fileInput.click()}>Import APO / AutoEQ…</button>
      <input bind:this={fileInput} type="file" accept=".txt,text/plain" hidden on:change={importApo} />
    </div>
    <div class="hx-actions">
      <button class="quiet" disabled={busy} on:click={openFile}>Open file…</button>
      <button class="quiet" disabled={busy || planning || !presets.length || !!problem || !mediaRoot} title={!mediaRoot ? 'Open a card in Cards first.' : problem ? 'Fix the problem below first.' : ''} on:click={startInstall}>{planning ? 'Preparing…' : 'Install on card…'}</button>
      <button class="primary" disabled={busy || !presets.length || !!problem} title={problem ? 'Fix the problem below first.' : ''} on:click={exportFile}>Save tau-assets.bin…</button>
    </div>
  </div>
  {#if notice}<p class="hx-notice" role="status">{notice}</p>{/if}
  {#if problem}<p class="hx-bad" role="alert">{problem}</p>{/if}

  {#if !preset}
    <p class="muted">No presets yet. Start with “+ New preset”, import a profile, or open a file you saved before.</p>
  {:else}
    <label class="hx-field">Name
      <input type="text" value={preset.name} maxlength="15" spellcheck="false" on:input={(e) => setName(e.currentTarget.value)} />
      <span class="muted">Capital letters, digits, space, _ and - only; not one of the built-in names (FLAT, WARM, CLEAR, BASS, VOCAL, SPEECH, LOW VOLUME, SMOOTH).</span>
    </label>
    {#if preset.kind === 'control'}
      <fieldset>
        <legend>Controls</legend>
        {#each CONTROLS as c}
          <label class="hx-ctl">
            <span><b>{c.label}</b> <span class="muted">{c.hint}</span></span>
            <input type="range" min={c.min} max={c.max} step="1" value={preset.controls[c.key]} on:input={(e) => setControl(c.key, +e.currentTarget.value)} />
            <output>{preset.controls[c.key] > 0 && c.min < 0 ? '+' : ''}{preset.controls[c.key]}</output>
          </label>
        {/each}
      </fieldset>
      <p class="muted">The Pocket works out the filters from these six positions, so there is no curve to draw here.</p>
    {:else}
      <div class="hx-raw">
        <p><b>{preset.stages.length}</b> filter{preset.stages.length === 1 ? '' : 's'} · volume lowered by <b>{(-20 * Math.log10(preset.preamp / 4194304)).toFixed(1)} dB</b> for headroom{#if lastImport && lastImport.preset.name === preset.name} · loudest boost {lastImport.peak_gain_db.toFixed(1)} dB{/if}</p>
        {#if curve}
          <svg viewBox={`-26 -8 ${CW + 34} ${CH + 30}`} role="img" aria-label="Frequency response, 20 hertz to 20 kilohertz">
            {#each [-12, -6, 0, 6, 12] as g}<line x1="0" x2={CW} y1={CH / 2 - g / RANGE * (CH / 2)} y2={CH / 2 - g / RANGE * (CH / 2)} class:zero={g === 0} /><text x="-4" y={CH / 2 - g / RANGE * (CH / 2) + 3} text-anchor="end">{g}</text>{/each}
            {#each ticks as [f, t]}<line x1={Math.log10(f / 20) / 3 * CW} x2={Math.log10(f / 20) / 3 * CW} y1="0" y2={CH} class="v" /><text x={Math.log10(f / 20) / 3 * CW} y={CH + 14} text-anchor="middle">{t}</text>{/each}
            <path d={path} />
          </svg>
          <p class="muted">Response in dB with the volume change included. Imported filters cannot be edited here; edit the profile in your EQ tool and import it again.</p>
        {/if}
      </div>
    {/if}
    <p><button class="quiet" on:click={remove}>Delete “{preset.name}”</button></p>
  {/if}
</section>

{#if reviewing && plan}
  <div class="hx-veil" role="presentation" on:click={closeReview}></div>
  <div class="hx-modal" role="dialog" aria-labelledby="hx-rev" use:modal>
    <h2 id="hx-rev">Install on {cardLabel || 'the card'}?</h2>
    <dl>
      <div><dt>Presets</dt><dd>{plan.presets.join(', ')}</dd></div>
      <div><dt>Writes</dt><dd>{plan.destination} ({plan.bytes} bytes)</dd></div>
      <div><dt>Replaces</dt><dd>{#if plan.existing}a {plan.existing.readable ? 'Tau assets file' : 'file that is not readable'} ({plan.existing.bytes} bytes{plan.existing.themes.length ? `, themes: ${plan.existing.themes.join(', ')}` : ''}){:else}nothing (no file there yet){/if}</dd></div>
      <div><dt>Read by</dt><dd>{#if plan.readers.length}{plan.readers.map((r) => `${r.core_id} ${r.version}${r.declares_slot ? '' : ' (does not ask for it)'}`).join(', ')}{:else}no core on this card{/if}</dd></div>
    </dl>
    {#if plan.interrupted_install}<p class="hx-warn">An earlier install was interrupted. The old file is put back first.</p>{/if}
    {#each plan.warnings as w}<p class="hx-warn" role="note">{w}</p>{/each}
    {#if installError}<p class="hx-bad" role="alert">{installError}</p>{/if}
    <div class="hx-modal-actions">
      <button class="quiet" disabled={installing} on:click={closeReview}>Cancel</button>
      <button class="primary" data-autofocus disabled={installing} on:click={confirmInstall}>{installing ? 'Installing…' : 'Install'}</button>
    </div>
  </div>
{/if}

<style>
  .hx{display:grid;gap:14px;max-width:860px;align-content:start}
  .hx h2{margin:0}.hx-head p{margin:0}
  .hx button{padding:8px 12px;font-size:13px;background:#202b2d;color:#e8ecec;border:1px solid #2c393a;border-radius:8px;cursor:pointer}
  .hx button.quiet{background:transparent}
  .hx button.primary{background:#c1f0ad;color:#142015;font-weight:700;border-color:#c1f0ad}
  .hx button:disabled{opacity:.5;cursor:not-allowed}
  .hx-bar{display:flex;gap:12px;flex-wrap:wrap;justify-content:space-between;align-items:center}
  .hx-tabs,.hx-actions{display:flex;gap:6px;flex-wrap:wrap}
  .hx .hx-tabs button.on{background:#c1f0ad;color:#142015;border-color:#c1f0ad;font-weight:700}
  .hx-notice{margin:0;padding:8px 12px;border:1px solid #2c393a;background:#161f20;border-radius:8px}
  .hx-bad{margin:0;color:#ffb4a8;font-size:13px}
  .hx-field{display:grid;gap:4px;font-size:12px;font-weight:650;color:#acbbbb}
  .hx input[type=text]{width:100%;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:10px 12px;font:inherit;font-size:13px;box-sizing:border-box}
  fieldset{border:1px solid #2c393a;border-radius:10px;padding:8px 12px;display:grid;gap:10px;background:#161f20}
  legend{padding:0 6px;color:#c1f0ad}
  .hx-ctl{display:grid;grid-template-columns:minmax(160px,1fr) minmax(140px,2fr) 36px;gap:12px;align-items:center;font-size:13px}
  .hx-ctl input{accent-color:#c1f0ad}.hx-ctl output{text-align:right;font-variant-numeric:tabular-nums}
  .hx-raw{display:grid;gap:8px}.hx-raw p{margin:0}
  svg{width:100%;max-width:560px;background:#101617;border:1px solid #2c393a;border-radius:10px}
  svg line{stroke:#2c393a;stroke-width:1}svg line.zero{stroke:#566768}svg line.v{stroke:#243031}
  svg text{fill:#9fb3aa;font-size:10px}svg path{fill:none;stroke:#c1f0ad;stroke-width:2}
  .hx-veil{position:fixed;inset:0;z-index:60;background:rgba(0,0,0,.55)}
  .hx-modal{position:fixed;z-index:70;top:50%;left:50%;transform:translate(-50%,-50%);width:min(520px,92vw);background:#1a2325;border:1px solid #344244;border-radius:16px;padding:22px;box-shadow:0 20px 60px rgba(0,0,0,.5);display:grid;gap:12px}
  .hx-modal h2{margin:0;font-size:18px}.hx-modal dl{margin:0;display:grid;gap:8px}.hx-modal dl div{display:grid;grid-template-columns:90px 1fr;gap:10px}
  .hx-modal dt{color:#9fb3aa;font-size:12px}.hx-modal dd{margin:0;font-size:13px;overflow-wrap:anywhere}
  .hx-warn{margin:0;color:#e5c88a;font-size:13px}
  .hx-modal-actions{display:flex;justify-content:flex-end;gap:8px}
  .hx-modal button{padding:8px 14px;font-size:13px;background:#202b2d;color:#e8ecec;border:1px solid #2c393a;border-radius:8px;cursor:pointer}
  .hx-modal button.quiet{background:transparent}.hx-modal button.primary{background:#c1f0ad;color:#142015;font-weight:700;border-color:#c1f0ad}
  .hx-modal button:disabled{opacity:.5;cursor:not-allowed}
</style>
