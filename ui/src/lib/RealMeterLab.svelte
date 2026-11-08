<script lang="ts">
  // The preview of Tau Alpha's own meter lab (a pinned vendor drop, interop D-007): the real meter modules, themes and palette, drawn by the
  // lab page itself in a sandboxed frame (`allow-scripts` only, given as `srcdoc`: A011/1). The page is shown exactly as the drop has it; Omega
  // adds nothing to its document. It speaks Tau Alpha's lab embed contract (version 1, `LAB_EMBED_CONTRACT.md`): Omega turns embed mode on, the
  // lab hides its own heading, panel and parameter list and crops its canvas to the current view; Omega drives the meter and the values with
  // `tau-lab-set`, the preview options with `tau-lab-option`, and reads the options, the size of the view, the cost per frame and the details
  // as data (`tau-lab-options`, `tau-lab-size`, `tau-lab-cost`, `tau-lab-info`). A page that speaks another contract number is shown as it is,
  // with a note, and not driven beyond `tau-lab-set`.
  import { onDestroy, onMount } from 'svelte';
  import { labOptions, labSend, labStatus, labTarget, previewView, type LabInfo, type LabOption, type LabTarget } from './meters/labBus';
  import { checkProvenance, parseManifest, type Manifest } from './meters/provenance';
  import { budgetFromCost, costRows, signed, type Budget } from './meters/costInfo';
  import BudgetBar from './BudgetBar.svelte';
  const LAB = '/tau-lab/meter_lab.html';
  const MANIFEST = '/tau-lab/MANIFEST.json';
  const CONTRACT = 1;
  let manifest: Manifest | null = null;
  let state: 'checking' | 'ready' | 'missing' = 'checking';
  let html = '';
  let frame: HTMLIFrameElement | undefined;
  let target: LabTarget = null;
  let frameWidth = 368;
  let frameHeight = 122;
  let sized = false;            // the lab has reported the size of its view (embed mode is on)
  let giveUp = false;           // show the frame anyway if the lab never reports
  let info: LabInfo | null = null;
  let budget: Budget | null = null;
  let detailsOpen = false;
  const unsub = labTarget.subscribe((t) => { target = t; send(); });
  onDestroy(() => { unsub(); labSend.set(null); labOptions.set([]); if (typeof window !== 'undefined') window.removeEventListener('message', onMessage); });

  const ask = (message: Record<string, unknown>) => frame?.contentWindow?.postMessage(message, '*');
  function send() {
    if (!target || !frame?.contentWindow || !$labStatus.ready) return;
    ask({ type: 'tau-lab-set', meter: target.meter, values: target.values });
  }
  function onMessage(e: MessageEvent) {
    // Only the lab frame itself (a sandboxed srcdoc frame has an opaque origin, so the source window is the check).
    if (!frame || e.source !== frame.contentWindow) return;
    const d = e.data;
    if (!d || typeof d.type !== 'string') return;
    switch (d.type) {
      case 'tau-lab-ready': {
        const contract = typeof d.contract === 'number' ? d.contract : 0;
        labStatus.set({ ready: true, modules: Array.isArray(d.meters) ? d.meters : [], applied: null, contract });
        if (contract === CONTRACT) { ask({ type: 'tau-lab-embed', on: true }); ask({ type: 'tau-lab-option', id: 'fullfig', value: $previewView === 'full' }); }
        send();
        break;
      }
      case 'tau-lab-applied':
        labStatus.update((s) => ({ ...s, applied: { meter: d.meter, ignored: d.ignored ?? [], clamped: d.clamped ?? {}, values: d.values, error: d.error } }));
        break;
      case 'tau-lab-options':
        labOptions.set(Array.isArray(d.options) ? (d.options as LabOption[]) : []);
        break;
      case 'tau-lab-size':
        if (typeof d.w === 'number' && typeof d.h === 'number' && d.w > 50 && d.h > 20) { frameWidth = d.w; frameHeight = d.h; sized = true; }
        break;
      case 'tau-lab-cost':
        // Only the meter on show: a message in flight from the last meter must not be drawn against the new meter's allowance.
        if (d.meter === target?.meter) budget = budgetFromCost(d);
        break;
      case 'tau-lab-info':
        if (d.cost && Array.isArray(d.table) && d.meter === target?.meter) info = { cost: d.cost, table: d.table };
        break;
    }
  }
  onMount(async () => {
    window.addEventListener('message', onMessage);
    labSend.set((message) => ask(message));
    setTimeout(() => { giveUp = true; }, 8000);
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
  $: ready = $labStatus.ready;
  $: embedded = $labStatus.contract === CONTRACT;
  // The small / full-screen toggle sets the lab's own full-screen option.
  $: if (ready && embedded) ask({ type: 'tau-lab-option', id: 'fullfig', value: $previewView === 'full' });
  // A different meter has a different allowance and different costs: never show the last meter's numbers against the new meter's.
  let shownMeter: number | null = null;
  $: if ((target?.meter ?? null) !== shownMeter) { shownMeter = target?.meter ?? null; budget = null; info = null; }
  $: if (ready && embedded && detailsOpen) ask({ type: 'tau-lab-info-get' });
  $: visible = embedded ? sized : ready || giveUp;
  $: applied = $labStatus.applied;
  $: modules = $labStatus.modules;
  $: rows = info ? costRows(info.cost) : [];
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
    <div class="rml-views" role="group" aria-label="View">
      <button type="button" aria-pressed={$previewView === 'small'} class:on={$previewView === 'small'} on:click={() => previewView.set('small')} data-testid="rml-view-small">Small view</button>
      <button type="button" aria-pressed={$previewView === 'full'} class:on={$previewView === 'full'} on:click={() => previewView.set('full')} data-testid="rml-view-full">Full screen</button>
    </div>
    <div class="rml-card">
      <!-- The frame is named by aria-label: a title attribute would pop up a tooltip over the meter. -->
      <!-- svelte-ignore a11y_missing_attribute -->
      <iframe bind:this={frame} aria-label="Meter preview (Tau Alpha's lab)" srcdoc={html} sandbox="allow-scripts" scrolling="no" style="width:{embedded ? frameWidth : 400}px;height:{embedded ? frameHeight : 460}px;visibility:{visible ? 'visible' : 'hidden'}" loading="lazy"></iframe>
    </div>
    <BudgetBar {budget} />
    <p class="rml-cap">Drawn by the Pocket's own meter code (Tau Alpha's lab) · an estimate, not measured</p>
    <details class="rml-details" bind:open={detailsOpen} data-testid="rml-details">
      <summary>Details: commands, processor time and cost</summary>
      {#if info}
        <table class="rml-kv" data-testid="rml-cost"><tbody>
          {#each rows as r}
            <tr class:has-note={!!r.note}><th scope="row">{r.label}</th><td class="num" class:ok={r.tone === 'ok'} class:bad={r.tone === 'bad'}>{r.value}</td></tr>
            {#if r.note}<tr class="n"><td colspan="2" class="note">{r.note}</td></tr>{/if}
          {/each}
        </tbody></table>
        {#if info.table.length}
          <h4 class="rml-sub">What each option costs</h4>
          <div class="rml-scroll"><table class="rml-opts" data-testid="rml-options-cost"><tbody>
            {#each info.table as row}
              <tr><th scope="row">{row.label}</th>{#each row.options as o}<td class:sel={o.selected}><span>{o.value}</span><span class="small">{o.cpu_low}–{o.cpu_high} % ({signed(o.delta)})</span><span class="small">{o.kops}k ops</span></td>{/each}</tr>
            {/each}
          </tbody></table></div>
          <p class="rml-cost">Each cell: the model's processor range for one core (and the change from your current setting) and the drawing engine's memory operations per frame, with demo audio and your other settings.</p>
        {/if}
        <p class="rml-cost">These figures cover the last 120 frames, about three seconds. The budget bar above shows the last second.</p>
      {:else}<p class="rml-cost">Waiting for the preview…</p>{/if}
    </details>
    <p class="rml-status" data-testid="rml-status" role="status">
      {#if !$labStatus.ready}Starting the preview…
      {:else}
        <span data-testid="rml-built">{modules.length} meter{modules.length === 1 ? '' : 's'} ready.</span>
        {#if held}<span class="rml-dim" title="Module {held.wasm_sha256}">Showing <b>{held.key}</b>.</span>{/if}
        {#if !embedded}<span class="warn" data-testid="rml-contract">This preview page speaks embed contract {$labStatus.contract}; Omega understands {CONTRACT}, so it is shown as the page has it.</span>{/if}
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
  .rml{display:grid;grid-template-columns:minmax(0,1fr);gap:8px;min-width:0}
  .rml-note{font-size:12.5px;color:#a6b3b2;line-height:1.55;margin:0;padding:10px 12px;border:1px dashed #2c393a;border-radius:10px}
  .rml-card{border:1px solid #2c393a;border-radius:12px;background:#161f20;padding:5px;width:fit-content;max-width:100%;box-sizing:border-box;overflow-x:auto}
  iframe{display:block;flex:none;border:0;background:transparent;overflow:hidden}
  .rml-views{display:inline-flex;border:1px solid #2c393a;border-radius:99px;overflow:hidden;width:fit-content}
  .rml-views button{padding:5px 14px;font-size:12.5px;background:transparent;color:#98a5a4;border:0;cursor:pointer}
  .rml-views button.on{background:#26352f;color:#e8ecec}
  .rml-details{min-width:0;overflow:hidden;border:1px solid #2c393a;border-radius:10px;background:#161f20;font-size:12.5px;color:#aab8b7}
  .rml-details summary{cursor:pointer;padding:8px 12px;list-style:none}.rml-details summary::-webkit-details-marker{display:none}.rml-details summary::before{content:'▸ ';color:#8f9e9d}.rml-details[open] summary::before{content:'▾ '}
  .rml-kv{border-collapse:collapse;width:calc(100% - 24px);margin:2px 12px 10px;font-size:12.5px}
  .rml-kv th{text-align:left;font-weight:500;color:#aab8b7;padding:5px 12px 5px 0;white-space:nowrap;vertical-align:baseline;border-top:1px solid #222d2e}
  .rml-kv td{padding:5px 0;vertical-align:baseline;border-top:1px solid #222d2e}
  .rml-kv td.num{text-align:right;font-variant-numeric:tabular-nums;font-weight:650;color:#e8ecec;white-space:nowrap;padding-right:14px}
  .rml-kv td.num.ok{color:#8fcf9f}.rml-kv td.num.bad{color:#ffb4a8}
  .rml-kv td.note{color:#8f9e9d;font-size:11.5px;line-height:1.45;padding:0 0 7px;border-top:0}
  .rml-kv tr.has-note th,.rml-kv tr.has-note td{padding-bottom:2px}
  .rml-kv tr:first-child th,.rml-kv tr:first-child td{border-top:0}
  .rml-sub{margin:4px 12px 6px;font-size:12px;font-weight:650;color:#c7d1d0;letter-spacing:.02em}
  .rml-scroll{overflow-x:auto;margin:0 12px 8px}
  .rml-opts{border-collapse:collapse;font-size:11.5px;min-width:100%}
  .rml-opts th{text-align:left;font-weight:600;color:#c7d1d0;padding:5px 12px 5px 0;white-space:nowrap;vertical-align:top;border-top:1px solid #222d2e}
  .rml-opts td{text-align:right;font-variant-numeric:tabular-nums;color:#98a5a4;padding:5px 8px;vertical-align:top;border-top:1px solid #222d2e;border-left:1px solid #222d2e;white-space:nowrap}
  .rml-opts td span{display:block}.rml-opts td span.small{font-size:10.5px;color:#7d8c8b}
  .rml-opts td.sel{color:#e8ecec;background:#1d2a2b}.rml-opts td.sel span.small{color:#98a5a4}
  .rml-cost{margin:0 12px 10px;line-height:1.5;color:#98a5a4;overflow-wrap:anywhere}
  .rml-cap{font-size:11.5px;color:#8f9e9d;margin:0}
  .rml-status{font-size:11.5px;color:#8f9e9d;margin:0;line-height:1.5}
  .warn{color:#e0b36a;margin-left:6px}.ok{color:#8fcf9f;margin-left:6px}.rml-dim{margin-left:6px}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
</style>
