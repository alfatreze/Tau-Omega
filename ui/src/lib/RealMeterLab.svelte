<script lang="ts">
  // The preview of Tau Alpha's own meter lab (a pinned vendor drop, interop D-007): the real meter modules, themes and palette, drawn by
  // the lab page itself in a sandboxed frame (`allow-scripts` only). Tau Omega never edits the page (to change it, ask Tau Alpha for a
  // new drop: docs/interop/); the drop on disk stays verbatim. What this component adds is a host-side stylesheet in the frame's
  // document: it matches the page to the app and hides the lab's own Meter list and parameter list, because the editor beside it
  // owns both (one meter selection, one set of controls) and drives the lab by message. The lab's own options (theme, mode, source,
  // size ...) stay available behind "Preview options". A native embed mode in the lab would make this stylesheet unnecessary.
  // The page is given to the frame as `srcdoc`: Tau Alpha saw a sandboxed frame loaded by URL stay silent in one browser pane (A011/1).
  // The editor drives it by message: `tau-lab-set` with a meter and a sparse set of values; the lab answers `tau-lab-ready` (the
  // modules it built, with their hashes) and `tau-lab-applied` (what it ignored or clamped, as the firmware would).
  import { onDestroy, onMount } from 'svelte';
  import { labStatus, labTarget, type LabTarget } from './meters/labBus';
  import { checkProvenance, parseManifest, type Manifest } from './meters/provenance';
  const LAB = '/tau-lab/meter_lab.html';
  const MANIFEST = '/tau-lab/MANIFEST.json';
  let manifest: Manifest | null = null;
  let state: 'checking' | 'ready' | 'missing' = 'checking';
  let html = '';
  let frame: HTMLIFrameElement | undefined;
  let target: LabTarget = null;
  let options = false;
  const unsub = labTarget.subscribe((t) => { target = t; send(); });
  onDestroy(() => { unsub(); if (typeof window !== 'undefined') window.removeEventListener('message', onMessage); });

  // Same colours as the app's cards; the lab page reads these variables.
  const BASE = `:root{--bg:#161f20;--fg:#e8ecec;--mut:#98a5a4;--line:#2c393a;color-scheme:dark}
html,body{background:transparent!important}body{padding:0!important}
h1,#controls,.panel label:first-child{display:none!important}
.wrap{display:block!important}
.stage{max-width:none!important;position:static!important;box-shadow:none!important;padding:0!important;max-height:none!important;background:transparent!important;width:calc(400px * var(--z,1))!important}
canvas{border-radius:8px}
.infod{margin-top:8px}`;
  const PANEL_HIDDEN = '.panel{display:none!important}';
  const PANEL_SHOWN = `.panel{display:block!important;min-width:0!important;margin-top:14px}
.panel label{font-size:12.5px;color:var(--mut)}
select,input[type=text]{background:#101617;color:var(--fg);border:1px solid #3b4a4b;border-radius:6px;padding:4px 6px;font:inherit}
input[type=range]{accent-color:#c1f0ad}`;
  $: srcdoc = html ? withStyle(html, `<style id="tau-omega-embed">${BASE}${options ? PANEL_SHOWN : PANEL_HIDDEN}</style>`) : '';
  function withStyle(page: string, style: string) {
    const at = page.indexOf('</head>');
    return at < 0 ? style + page : page.slice(0, at) + style + page.slice(at);
  }

  function send() {
    if (!target || !frame?.contentWindow || !$labStatus.ready) return;
    frame.contentWindow.postMessage({ type: 'tau-lab-set', meter: target.meter, values: target.values }, '*');
  }
  function onMessage(e: MessageEvent) {
    // Only the lab frame itself (a sandboxed srcdoc frame has an opaque origin, so the source window is the check).
    if (!frame || e.source !== frame.contentWindow) return;
    const d = e.data;
    if (d?.type === 'tau-lab-ready') {
      labStatus.set({ ready: true, modules: Array.isArray(d.meters) ? d.meters : [], applied: null });
      send();
    } else if (d?.type === 'tau-lab-applied') {
      labStatus.update((s) => ({ ...s, applied: { meter: d.meter, ignored: d.ignored ?? [], clamped: d.clamped ?? {}, values: d.values, error: d.error } }));
    }
  }
  onMount(async () => {
    window.addEventListener('message', onMessage);
    try {
      const r = await fetch(LAB);
      const text = r.ok ? await r.text() : '';
      // A dev server answers an unknown path with the app's own page, so look for the lab itself.
      if (text.includes('Tau meter lab')) { html = text; state = 'ready'; } else state = 'missing';
    } catch { state = 'missing'; }
    try {
      const r = await fetch(MANIFEST);
      manifest = r.ok ? parseManifest(await r.json()) : null;
    } catch { manifest = null; }
  });
  $: applied = $labStatus.applied;
  $: modules = $labStatus.modules;
  $: held = target ? modules.find((m) => m.id === target!.meter) : undefined;
  $: prov = manifest && $labStatus.ready ? checkProvenance(modules, manifest) : null;
  $: heldChecks = prov && held ? prov.checks.filter((c) => c.name === held!.key || c.name === `${held!.key}_timed`) : [];
  $: heldBuild = heldChecks.find((c) => c.clang);
  $: bad = prov ? prov.checks.filter((c) => c.status !== 'match') : [];
</script>
<section class="rml" aria-label="Meter preview">
  {#if state === 'checking'}
    <p class="rml-note" role="status">Looking for the pinned lab…</p>
  {:else if state === 'ready'}
    <div class="rml-card">
      <iframe bind:this={frame} title="Tau Alpha meter lab" {srcdoc} sandbox="allow-scripts" loading="lazy" class:tall={options}></iframe>
    </div>
    <div class="rml-bar">
      <span class="rml-cap">Drawn by the Pocket's own meter code (Tau Alpha's lab) · an estimate, not measured</span>
      <button type="button" class="rml-opt" aria-expanded={options} on:click={() => (options = !options)} data-testid="rml-options">{options ? 'Hide preview options' : 'Preview options'}</button>
    </div>
    <p class="rml-status" data-testid="rml-status" role="status">
      {#if !$labStatus.ready}Starting the preview…
      {:else}
        <span data-testid="rml-built">{modules.length} meter{modules.length === 1 ? '' : 's'} ready.</span>
        {#if held}<span class="rml-dim" title="Module {held.wasm_sha256}">Showing <b>{held.key}</b>.</span>{/if}
        {#if prov}
          {#if prov.ok}<span class="ok" data-testid="rml-provenance" data-state="match" title={`All ${prov.checks.length} module hashes match the drop's manifest (Tau Alpha ${prov.commit.slice(0, 8)})${heldBuild?.clang ? `; ${held?.key}: built with ${heldBuild.clang} from ${heldBuild.sources} source files` : ''}.`}>✓ All {prov.checks.length} match what Tau Alpha published ({prov.commit.slice(0, 8)})</span>
          {:else}<span class="warn" data-testid="rml-provenance" data-state="mismatch">Some meters do not match what Tau Alpha published{#if bad.length}: {bad.map((c) => `${c.name} (${c.status === 'mismatch' ? 'different' : 'not on their list'})`).join(', ')}{/if}{#if prov.notReported.length}; the preview did not load: {prov.notReported.join(', ')}{/if}. Do not rely on the preview for those meters.</span>{/if}
        {:else}<span class="rml-dim" data-testid="rml-provenance" data-state="none">Tau Alpha's list of meters is missing, so the preview could not be checked.</span>{/if}
        {#if applied?.error}<span class="warn">The preview does not know this meter ({applied.error}).</span>{/if}
        {#if applied && Object.keys(applied.clamped).length}<span class="warn" data-testid="rml-clamped">Too far out of range, shown at the nearest allowed value: {Object.entries(applied.clamped).map(([k, v]) => `${k} = ${v}`).join(', ')}.</span>{/if}
        {#if applied && applied.ignored.length}<span class="warn" data-testid="rml-ignored">The Pocket would ignore: {applied.ignored.join(', ')}.</span>{/if}
      {/if}
    </p>
  {:else}
    <p class="rml-note" data-testid="rml-missing">The pinned lab page is not installed in this build yet. It comes from a Tau Alpha lab drop that includes the built <code>meter_lab.html</code> (see <code>vendor/tau-lab/README.md</code>); until then there is no preview; the preset editor does not need it.</p>
  {/if}
</section>
<style>
  .rml{display:grid;gap:8px;min-width:0}
  .rml-note{font-size:12.5px;color:#a6b3b2;line-height:1.55;margin:0;padding:10px 12px;border:1px dashed #2c393a;border-radius:10px}
  .rml-card{border:1px solid #2c393a;border-radius:12px;background:#161f20;padding:10px;width:fit-content;max-width:100%;box-sizing:border-box}
  .rml-card:has(iframe.tall){width:100%}
  iframe{display:block;width:400px;max-width:100%;height:398px;border:0;background:transparent}
  iframe.tall{width:100%;height:720px}
  .rml-bar{display:flex;justify-content:space-between;align-items:center;gap:8px;flex-wrap:wrap}
  .rml-cap{font-size:11.5px;color:#8f9e9d}
  .rml-opt{padding:4px 10px;font-size:12px;background:transparent;color:#aab8b7;border:1px solid #2c393a;border-radius:8px;cursor:pointer}
  .rml-opt:hover,.rml-opt[aria-expanded=true]{color:#e8ecec;border-color:#3b4a4b}
  .rml-status{font-size:11.5px;color:#8f9e9d;margin:0;line-height:1.5}
  .warn{color:#e0b36a;margin-left:6px}.ok{color:#8fcf9f;margin-left:6px}.rml-dim{margin-left:6px}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
</style>
