<script lang="ts">
  // The preview of Tau Alpha's own meter lab (a pinned vendor drop, interop D-007): the real meter modules, themes and palette, drawn by
  // the lab page itself in a sandboxed frame (`allow-scripts` only). Tau Omega never edits the page (to change it, ask Tau Alpha for a
  // new drop: docs/interop/); the drop on disk stays verbatim. What this component adds is a host-side stylesheet in the frame's
  // document: it matches the page to the app and hides the lab's own Meter list and parameter list, because the editor beside it
  // owns both (one meter selection, one set of controls) and drives the lab by message. The lab's own options (theme, mode, source,
  // size ...) are read and set through a small host-side bridge script in the same document: it only reads the lab's own controls and
  // its detail area and sets the controls when asked (the lab's scripts are untouched), so Omega can draw them natively in its Common
  // tab and its Details panel. A native embed and options API in the lab would make the stylesheet and the bridge unnecessary.
  // The page is given to the frame as `srcdoc`: Tau Alpha saw a sandboxed frame loaded by URL stay silent in one browser pane (A011/1).
  // The editor drives it by message: `tau-lab-set` with a meter and a sparse set of values; the lab answers `tau-lab-ready` (the
  // modules it built, with their hashes) and `tau-lab-applied` (what it ignored or clamped, as the firmware would).
  import { onDestroy, onMount } from 'svelte';
    import { labOptions, labSend, labStatus, labTarget, previewView, type LabInfo, type LabOption, type LabTarget } from './meters/labBus';
  import { checkProvenance, parseManifest, type Manifest } from './meters/provenance';
  import { parseCostLines, type Budget } from './meters/costInfo';
  import BudgetBar from './BudgetBar.svelte';
  const LAB = '/tau-lab/meter_lab.html';
  const MANIFEST = '/tau-lab/MANIFEST.json';
  let manifest: Manifest | null = null;
  let state: 'checking' | 'ready' | 'missing' = 'checking';
  let html = '';
  let frame: HTMLIFrameElement | undefined;
  let target: LabTarget = null;
  let frameHeight = 124;
  let frameWidth = 368;
  let info: LabInfo | null = null;
  let detailsOpen = false;
  let infoTimer: ReturnType<typeof setInterval> | undefined;
  const unsub = labTarget.subscribe((t) => { target = t; send(); });
  onDestroy(() => { unsub(); labSend.set(null); labOptions.set([]); clearInterval(infoTimer); if (typeof window !== 'undefined') window.removeEventListener('message', onMessage); });

  // Same colours as the app's cards; the lab page reads these variables.
  const BASE = `:root{--bg:#161f20;--fg:#e8ecec;--mut:#98a5a4;--line:#2c393a;color-scheme:dark}
html,body{background:transparent!important}body{padding:0!important}
h1,#controls,.panel label:first-child{display:none!important}
.wrap{display:block!important}
.stage{position:static!important;box-shadow:none!important;padding:0!important;max-height:none!important;background:transparent!important;width:max-content!important;max-width:none!important}
canvas{border:0!important;border-radius:3px}
/* Small view: only the meter's own band of the Now Playing screen (x 16, y 152, 368 x 122). Full view: the 400 x 323 figure. Never scaled: a fractional scale makes sharp pixels uneven. */
.omega-small #cv{display:block;width:368px;height:122px;object-fit:none;object-position:-16px -152px}
.omega-full #cv{display:block;width:400px;height:323px;object-fit:none;object-position:0 0}
.infod{margin-top:8px}`;
  const HIDE = '.panel,.infod{display:none!important}';
  // The bridge: reads the lab's own controls and detail area and sets a control when asked. Nothing else in the lab is touched.
  const BRIDGE = `\x3cscript id="tau-omega-bridge">(function(){
var IDS=['theme','mode','accent','src','fullfig','barready','timedg','statsok','corr'];
function text(el){var t='';el.childNodes.forEach(function(n){if(n.nodeType===3)t+=n.textContent;});return t.replace(/\\s+/g,' ').trim();}
function describe(){var out=[];IDS.forEach(function(id){var el=document.getElementById(id);if(!el)return;var l=el.closest('label');var o={id:id,label:l?text(l)||id:id,help:l&&l.title?l.title:''};
 if(el.tagName==='SELECT'){o.type='select';o.value=el.value;o.choices=[].map.call(el.options,function(x){return{value:x.value,text:x.text};});if(id==='src')o.choices=o.choices.filter(function(c){return c.value==='demo'||c.value==='sweep'||c.value==='silence';});}
 else if(el.type==='checkbox'){o.type='bool';o.value=el.checked;}
 else if(el.type==='range'){o.type='range';o.min=+el.min;o.max=+el.max;o.value=+el.value;}
 else return;out.push(o);});return out;}
function lines(el){var out=[];if(!el)return out;el.innerHTML.split(/<br\\s*\\/?>/i).forEach(function(h){var d=document.createElement('div');d.innerHTML=h;var t=d.textContent.replace(/\\s+/g,' ').trim();if(t)out.push(t);});return out;}
function info(){var table=[];var t=document.querySelector('#costtable table');
 if(t)[].forEach.call(t.rows,function(r){var cells=[].slice.call(r.cells);var th=cells.length&&cells[0].tagName==='TH'?cells.shift():null;table.push({label:th?th.textContent.replace(/\\s+/g,' ').trim():'',cells:cells.map(function(c){return{lines:lines(c),sel:c.classList.contains('sel')};})});});
 var tn=document.querySelector('#costtable .cost'),sn=document.getElementById('splitnote');
 return{lines:lines(document.getElementById('cost')),table:table,tableNote:tn?tn.textContent.trim():'',note:sn?sn.textContent.trim():''};}
function post(m){parent.postMessage(m,'*');}
/* The lab passes its live list of per-frame costs to TauCost.summarize on every frame; keep a reference (read only) and report the last second of it, so the budget bar reacts quickly. */
var perFrame=null,costClass=null;
if(window.TauCost&&TauCost.summarize){var sum=TauCost.summarize;TauCost.summarize=function(list,cls){perFrame=list;costClass=cls;return sum.apply(this,arguments);};}
function budget(){if(!perFrame||perFrame.length<3||!window.TauCost)return null;var n=Math.min(38,perFrame.length),a=perFrame.slice(-n),mx=Math.max.apply(null,a),me=a.reduce(function(x,y){return x+y;},0)/n,c=costClass==null?2:costClass,al=TauCost.BUDGET[c];return{mean:Math.round(me*10)/10,worst:mx,allowed:al,costClass:String(c),over:mx>al};}
setInterval(function(){var b=budget();if(b)post({type:'omega-lab-budget',budget:b});},250);
function size(){var s=document.querySelector('.stage');if(!s)return;var w=document.getElementById('cv').getBoundingClientRect().width;var sw=document.getElementById('splitwrap');if(sw&&!sw.hidden)w=Math.max(w,400);post({type:'omega-lab-size',height:Math.ceil(s.getBoundingClientRect().height),width:Math.ceil(w)});}
window.addEventListener('message',function(e){var d=e.data||{};
 if(d.type==='omega-lab-get-options')post({type:'omega-lab-options',options:describe()});
 else if(d.type==='omega-lab-get-info')post({type:'omega-lab-info',info:info()});
 else if(d.type==='omega-lab-details'){var dd=document.getElementById('infod');if(dd)dd.open=!!d.open;}
 else if(d.type==='omega-lab-view'){var full=d.mode==='full',r=document.documentElement;r.classList.toggle('omega-full',full);r.classList.toggle('omega-small',!full);var ff=document.getElementById('fullfig');if(ff&&ff.checked!==full){ff.checked=full;ff.dispatchEvent(new Event('change',{bubbles:true}));}size();}
 else if(d.type==='omega-lab-option'){var el=document.getElementById(d.id);if(el&&IDS.indexOf(d.id)>=0){if(el.type==='checkbox')el.checked=!!d.value;else el.value=String(d.value);el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new Event('change',{bubbles:true}));post({type:'omega-lab-options',options:describe()});}}});
var st=document.querySelector('.stage');if(st&&window.ResizeObserver)new ResizeObserver(size).observe(st);window.addEventListener('load',size);setTimeout(size,300);
})();\x3c/script>`;
  $: srcdoc = html ? withStyle(html, `<style id="tau-omega-embed">${BASE}${HIDE}</style>`) + '' : '';
  function withStyle(page: string, style: string) {
    const at = page.indexOf('</head>');
    const styled = at < 0 ? style + page : page.slice(0, at) + style + page.slice(at);
    const end = styled.lastIndexOf('</body>');
    return end < 0 ? styled + BRIDGE : styled.slice(0, end) + BRIDGE + styled.slice(end);
  }

  const ask = (message: Record<string, unknown>) => frame?.contentWindow?.postMessage(message, '*');
  // The view (small or full screen) is chosen by the toggle above the preview; the lab is told whenever it changes.
  $: if ($labStatus.ready) sendView($previewView);
  function sendView(mode: 'small' | 'full' = $previewView) { ask({ type: 'omega-lab-view', mode }); }
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
      ask({ type: 'omega-lab-get-options' });
      sendView();
    } else if (d?.type === 'omega-lab-options') {
      labOptions.set(Array.isArray(d.options) ? (d.options as LabOption[]) : []);
    } else if (d?.type === 'omega-lab-budget') {
      if (d.budget && typeof d.budget.worst === 'number' && typeof d.budget.allowed === 'number') budget = d.budget as Budget;
    } else if (d?.type === 'omega-lab-info') {
      info = d.info ?? null;
    } else if (d?.type === 'omega-lab-size') {
      if (typeof d.height === 'number' && d.height > 40 && d.height < 1200) frameHeight = d.height;
      if (typeof d.width === 'number' && d.width > 100 && d.width < 1200) frameWidth = d.width;
    } else if (d?.type === 'tau-lab-applied') {
      labStatus.update((s) => ({ ...s, applied: { meter: d.meter, ignored: d.ignored ?? [], clamped: d.clamped ?? {}, values: d.values, error: d.error } }));
    }
  }
  onMount(async () => {
    window.addEventListener('message', onMessage);
    labSend.set((message) => ask(message));
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
  // The budget comes from the bridge (about four times a second, from the last second of frames). The Details panel asks the lab for its
  // detail text while it is open.
  let budget: Budget | null = null;
  $: ready = $labStatus.ready;
  $: if (ready) { ask({ type: 'omega-lab-details', open: detailsOpen }); }
  $: if (ready && detailsOpen) { clearInterval(infoTimer); ask({ type: 'omega-lab-get-info' }); infoTimer = setInterval(() => ask({ type: 'omega-lab-get-info' }), 1500); } else { clearInterval(infoTimer); }
  // A different meter has a different allowance: never show the last meter's numbers against the new meter's.
  let shownMeter: number | null = null;
  $: if ((target?.meter ?? null) !== shownMeter) { shownMeter = target?.meter ?? null; budget = null; }
  $: costRows = info ? parseCostLines(info.lines) : [];
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
    <div class="rml-views" role="group" aria-label="View">
      <button type="button" aria-pressed={$previewView === 'small'} class:on={$previewView === 'small'} on:click={() => previewView.set('small')} data-testid="rml-view-small">Small view</button>
      <button type="button" aria-pressed={$previewView === 'full'} class:on={$previewView === 'full'} on:click={() => previewView.set('full')} data-testid="rml-view-full">Full screen</button>
    </div>
    <div class="rml-card">
      <!-- The frame is named by aria-label: a title attribute would pop up a tooltip over the meter. -->
      <!-- svelte-ignore a11y_missing_attribute -->
      <iframe bind:this={frame} aria-label="Meter preview (Tau Alpha's lab)" {srcdoc} sandbox="allow-scripts" scrolling="no" style="width:{frameWidth}px;height:{frameHeight}px" loading="lazy"></iframe>
    </div>
    <BudgetBar {budget} />
    <p class="rml-cap">Drawn by the Pocket's own meter code (Tau Alpha's lab) · an estimate, not measured</p>
    <details class="rml-details" bind:open={detailsOpen} data-testid="rml-details">
      <summary>Details: commands, processor time and cost</summary>
      {#if info && (info.lines.length || info.table.length)}
        {#if costRows.length}
          <table class="rml-kv" data-testid="rml-cost"><tbody>
            {#each costRows as r}
              <tr class:has-note={!!r.note}><th scope="row">{r.label}</th><td class="num" class:ok={r.tone === 'ok'} class:bad={r.tone === 'bad'}>{r.value}</td></tr>
              {#if r.note}<tr class="n"><td colspan="2" class="note">{r.note}</td></tr>{/if}
            {/each}
          </tbody></table>
        {/if}
        {#if info.table.length}
          <h4 class="rml-sub">What each option costs</h4>
          <div class="rml-scroll"><table class="rml-opts" data-testid="rml-options-cost"><tbody>
            {#each info.table as row}
              <tr><th scope="row">{row.label}</th>{#each row.cells as c}<td class:sel={c.sel}>{#each c.lines as line, i}<span class:small={i > 0}>{line}</span>{/each}</td>{/each}</tr>
            {/each}
          </tbody></table></div>
          {#if info.tableNote}<p class="rml-cost">{info.tableNote}</p>{/if}
        {/if}
        {#if info.note}<p class="rml-cost">{info.note}</p>{/if}
      {:else}<p class="rml-cost">Waiting for the preview…</p>{/if}
    </details>
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
