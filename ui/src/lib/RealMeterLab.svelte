<script lang="ts">
  // Tau Alpha's own meter lab (a pinned vendor drop, interop D-007): ONE self-contained page with the real meter modules, themes,
  // palette and registry. It is shown as it is, in a sandboxed frame (`allow-scripts` only); Tau Omega never edits it (to change it,
  // ask Tau Alpha for a new drop: docs/interop/). The page is given to the frame as `srcdoc`: Tau Alpha saw a sandboxed frame
  // loaded by URL stay silent in one browser pane, and `srcdoc` works everywhere (A011/1).
  // The preset editor drives it by message: `tau-lab-set` with a meter and a sparse set of values; the lab answers `tau-lab-ready`
  // (the modules it built, with their hashes) and `tau-lab-applied` (what it ignored or clamped, as the firmware would).
  import { onDestroy, onMount } from 'svelte';
  import { labStatus, labTarget, type LabTarget } from './meters/labBus';
  import { checkProvenance, parseManifest, type Manifest } from './meters/provenance';
  const LAB = '/tau-lab/meter_lab.html';
  const MANIFEST = '/tau-lab/MANIFEST.json';
  // The drop's manifest: the hash of every module Tau Alpha recorded. The modules the lab compiled are checked against it (Q-035).
  let manifest: Manifest | null = null;
  let state: 'checking' | 'ready' | 'missing' = 'checking';
  let html = '';
  let frame: HTMLIFrameElement | undefined;
  let target: LabTarget = null;
  const unsub = labTarget.subscribe((t) => { target = t; send(); });
  onDestroy(() => { unsub(); if (typeof window !== 'undefined') window.removeEventListener('message', onMessage); });

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
<section class="rml" aria-labelledby="rml-title">
  <h2 id="rml-title">Tau Alpha's meter lab</h2>
  {#if state === 'checking'}
    <p class="rml-note" role="status">Looking for the pinned lab…</p>
  {:else if state === 'ready'}
    <p class="rml-note">The real meters, themes and registry from Tau Alpha's own preview stack (the same drawing code as the Pocket; estimates, not measured). Pinned and never edited here. The lab shows the preset you are editing below.</p>
    <iframe bind:this={frame} title="Tau Alpha meter lab" srcdoc={html} sandbox="allow-scripts" loading="lazy"></iframe>
    <p class="rml-status" data-testid="rml-status" role="status">
      {#if !$labStatus.ready}Starting the lab…
      {:else}
        {modules.length} meter module{modules.length === 1 ? '' : 's'} built.
        {#if held}Showing <b>{held.key}</b> (module {held.wasm_sha256.slice(0, 12)}…).{/if}
        {#if prov}
          {#if prov.ok}<span class="ok" data-testid="rml-provenance" data-state="match">All {prov.checks.length} module hashes match the drop's manifest (Tau Alpha {prov.commit.slice(0, 8)}){#if heldBuild?.clang}; {held?.key}: built with {heldBuild.clang} from {heldBuild.sources} source files{/if}.</span>
          {:else}<span class="warn" data-testid="rml-provenance" data-state="mismatch">Module provenance differs from the drop's manifest{#if bad.length}: {bad.map((c) => `${c.name} (${c.status === 'mismatch' ? 'hash differs' : 'not in the manifest'})`).join(', ')}{/if}{#if prov.notReported.length}; not reported by the lab: {prov.notReported.join(', ')}{/if}. This is not the build Tau Alpha recorded; do not rely on the preview for those meters.</span>{/if}
        {:else if $labStatus.ready}<span class="rml-dim" data-testid="rml-provenance" data-state="none">No manifest beside the lab page, so the module hashes are not checked.</span>{/if}
        {#if applied?.error}<span class="warn">The lab does not know this meter ({applied.error}).</span>{/if}
        {#if applied && Object.keys(applied.clamped).length}<span class="warn" data-testid="rml-clamped">Out of range, shown clamped: {Object.entries(applied.clamped).map(([k, v]) => `${k} = ${v}`).join(', ')}.</span>{/if}
        {#if applied && applied.ignored.length}<span class="warn" data-testid="rml-ignored">Ignored by the lab: {applied.ignored.join(', ')}.</span>{/if}
      {/if}
    </p>
  {:else}
    <p class="rml-note" data-testid="rml-missing">The pinned lab page is not installed in this build yet. It comes from a Tau Alpha lab drop that includes the built <code>meter_lab.html</code> (see <code>vendor/tau-lab/README.md</code>); until then the lab preview is not available; the preset editor below does not need it.</p>
  {/if}
</section>
<style>
  .rml{margin:0 0 22px}.rml h2{font-size:16px;margin:0 0 8px}
  .rml-note{font-size:12.5px;color:#a6b3b2;line-height:1.55;margin:0 0 10px;max-width:760px}
  .rml-status{font-size:12px;color:#a6b3b2;margin:8px 0 0}.warn{color:#e0b36a;margin-left:8px}.ok{color:#8fcf9f;margin-left:8px}.rml-dim{margin-left:8px}
  iframe{display:block;width:100%;height:min(900px,82vh);border:1px solid #2c393a;border-radius:10px;background:#14161a}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
</style>
