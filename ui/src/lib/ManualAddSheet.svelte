<script lang="ts">
  // Add a core that is not in the openFPGA Library: from a zip, a folder or a GitHub address. Three steps in one dialog: Source -> Preview (what the
  // core says about itself) -> Review install (the same reviewed plan every package uses). Nothing is written to a card before the plan is confirmed.
  // docs/MARKETPLACE_SPEC.md section 7.
  import { onMount, tick } from 'svelte';
  import { open as pick } from '@tauri-apps/plugin-dialog';
  import Icon from './Icon.svelte';
  import UpdatePlanView from './UpdatePlanView.svelte';
  import { modal } from './a11y';
  import { errorMessage } from './backend';
  import { categoryIcon } from './iconFor';
  import { plural, size } from './coresFormat';
  import { manualClassify, manualInspect, manualRegister } from './tau-api';
  import type { ManualClass, ManualInspect, ManualReleaseChoice } from './types';
  export let card = ''; export let initial = '';
  export let close: () => void;

  type Ready = Extract<ManualInspect, { status: 'ready' }>;
  let step: 'source' | 'reading' | 'choose' | 'preview' = 'source';
  let input = initial; let cls: ManualClass | null = null; let error = ''; let readme = true; let installed = false;
  let choose: Extract<ManualInspect, { status: 'choose' }> | null = null; let pickTag = ''; let pickAsset = '';
  let result: Ready | null = null; let trust = false; let override = false; let timer: ReturnType<typeof setTimeout> | undefined;
  $: github = !!cls?.kind.startsWith('github');
  const cardName = (c: string) => c.split('/').filter(Boolean).pop() ?? c;

  $: { clearTimeout(timer); const v = input.trim(); if (!v) cls = null; else timer = setTimeout(async () => { try { cls = await manualClassify(v); } catch { cls = null; } }, 200); }
  onMount(() => { if (initial) void read(); });

  async function choosePath(folder: boolean) {
    const r = await pick(folder ? { directory: true, title: 'Choose a core folder' } : { multiple: false, title: 'Choose a core zip', filters: [{ name: 'Core zip', extensions: ['zip'] }] });
    if (typeof r === 'string') { input = r; await tick(); await read(); }
  }
  async function read(tag: string | null = null, asset: string | null = null) {
    const text = input.trim(); if (!text) return;
    step = 'reading'; error = '';
    try {
      const res = await manualInspect({ input: text, tag, asset, readme, card: card || null });
      if (res.status === 'choose') { choose = res; pickTag = res.tag; pickAsset = res.releases.find((r) => r.tag === res.tag)?.zips[0] ?? ''; step = 'choose'; }
      else { result = res; trust = false; override = false; step = 'preview'; }
    } catch (e) { error = errorMessage(e); step = 'source'; }
  }
  $: releaseOf = (tag: string): ManualReleaseChoice | undefined => choose?.releases.find((r) => r.tag === tag);
  $: if (choose && releaseOf(pickTag) && !releaseOf(pickTag)!.zips.includes(pickAsset)) pickAsset = releaseOf(pickTag)!.zips[0] ?? '';

  async function onInstalled() {
    installed = true;
    if (!result) return;
    const p = result.provenance;
    try { await manualRegister({ core_id: result.preview.core_id, kind: p.kind, owner: p.owner, repo: p.repo, tag: p.tag, asset: p.asset, sha256: p.sha256 || result.preview.sha256, card: cardName(card) }); } catch { /* remembering the source is advice only */ }
  }
  const yn = (v: boolean | null) => (v === true ? 'Yes' : v === false ? 'No' : 'Not stated');
  $: blocked = !result ? '' : result.different_source ? 'This core id was added before from a different source. Remove the existing core first, or add it from the same source.' : !card ? 'Open a card first: the install is reviewed against a card.' : '';
  $: lintErrors = result?.lint?.errors ?? 0;
  $: lintBlocked = lintErrors > 0 && !override;
  $: supply = result?.preview.needs.filter((n) => n.you_supply) ?? [];
  $: group = result?.preview.platforms[0];
</script>
<svelte:window on:keydown={(e) => e.key === 'Escape' && close()} />
<div class="ma-backdrop" role="presentation" on:click={close}></div>
<div class="ma" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="ma-title" use:modal>
  <header class="ma-head"><div><p class="eyebrow">ADD A CORE MANUALLY</p><h2 id="ma-title">{step === 'preview' && result ? result.preview.shortname || result.preview.core_id : 'Add a core that is not in the Library'}</h2>
    <ol class="ma-steps" aria-label="Steps"><li class:cur={step === 'source' || step === 'reading' || step === 'choose'}>1 Source</li><li class:cur={step === 'preview' && !trust}>2 Preview</li><li class:cur={step === 'preview' && trust}>3 Review install</li></ol></div>
    <button class="quiet" data-autofocus on:click={close}>{installed ? 'Done' : 'Cancel'}</button></header>

  {#if step === 'source' || step === 'reading'}
    <div class="ma-choices">
      <button class="ma-choice" disabled={step === 'reading'} on:click={() => choosePath(false)}><span class="ma-ci"><Icon name="file-zip" size={26} /></span><strong>Choose a zip file</strong><small>A core package as its author published it</small></button>
      <button class="ma-choice" disabled={step === 'reading'} on:click={() => choosePath(true)}><span class="ma-ci"><Icon name="folder" size={26} /></span><strong>Choose a folder</strong><small>An unzipped core, or one core's own folder</small></button>
    </div>
    <label class="ma-addr"><span>Or paste a GitHub address</span>
      <input type="text" inputmode="url" spellcheck="false" autocomplete="off" placeholder="https://github.com/author/openfpga-core" bind:value={input} aria-describedby="ma-addr-note" on:keydown={(e) => e.key === 'Enter' && cls?.ok && read()} /></label>
    <p id="ma-addr-note" class="ma-note" class:bad={cls && !cls.ok} role="status" aria-live="polite">{#if !input.trim()}A repository, a release or a release's zip. Only github.com addresses are accepted.{:else if cls?.ok}{cls.kind === 'zip' ? 'A zip file on this computer.' : cls.kind === 'folder' ? 'A folder on this computer.' : cls.kind === 'github-repo' ? 'A GitHub repository: its newest release is used (you can choose another).' : cls.kind === 'github-release' ? 'One GitHub release.' : 'One GitHub download.'}{:else if cls}{cls.reason}{/if}</p>
    {#if github}<label class="ma-check"><input type="checkbox" bind:checked={readme} /> Also read the README and its pictures from GitHub <small>(two more requests to GitHub; shown here only, never written to the card)</small></label>{/if}
    {#if error}<p class="ma-err" role="alert">{error}</p>{/if}
    <div class="ma-actions"><button class="primary" disabled={!cls?.ok || step === 'reading'} on:click={() => read()}>{step === 'reading' ? 'Reading the core…' : 'Read this core'}</button></div>
    <p class="ma-safe">Omega reads the core first and shows you what it found. Nothing is installed until you review the plan and confirm it.</p>

  {:else if step === 'choose' && choose}
    <p class="ma-note">{choose.owner}/{choose.repo} has {plural(choose.releases.length, 'release')}. Choose the one to add.</p>
    <label class="ma-row"><span>Release</span><select bind:value={pickTag}>{#each choose.releases as r}<option value={r.tag}>{r.name}{r.name !== r.tag ? ` (${r.tag})` : ''} · {r.published}{r.prerelease ? ' · pre-release' : ''}{r.zips.length ? '' : ' · no zip'}</option>{/each}</select></label>
    <label class="ma-row"><span>Zip</span><select bind:value={pickAsset} disabled={!releaseOf(pickTag)?.zips.length}>{#each releaseOf(pickTag)?.zips ?? [] as z}<option value={z}>{z}</option>{/each}</select></label>
    {#if !releaseOf(pickTag)?.zips.length}<p class="ma-note bad" role="status">This release has no zip download. Choose another release, or add the zip yourself.</p>{/if}
    <div class="ma-actions"><button class="quiet" on:click={() => (step = 'source')}>Back</button><button class="primary" disabled={!pickAsset} on:click={() => read(pickTag, pickAsset)}>Use this zip</button></div>

  {:else if step === 'preview' && result}
    {@const p = result.preview}
    {#if p.banner}<img class="ma-banner" src={p.banner} alt={`${group?.name ?? 'Platform'} artwork`} />{/if}
    <div class="ma-id">
      <span class="ma-icon" aria-hidden="true">{#if p.icon}<img src={p.icon} alt="" width="40" height="40" />{:else}{(p.shortname || p.core_id).slice(0, 2).toUpperCase()}{/if}</span>
      <div><strong>{p.shortname || p.core_id}</strong><small>{p.author} · version {p.version || 'not stated'}{p.date_release ? ` · ${p.date_release}` : ''}</small>
        {#if group}<small class="ma-where"><Icon name={categoryIcon(group.category)} size={13} /> {group.category} › {group.manufacturer} › {group.name}</small>{/if}</div>
    </div>
    {#if p.description}<p class="ma-desc">{p.description}</p>{/if}
    <div class="ma-banner-warn" role="note"><strong>Not in the openFPGA Library.</strong> Omega cannot tell what this bitstream does. Only install cores from authors you trust.</div>
    {#if result.in_library}<p class="ma-note ok" role="note">This core's id is in the Library, so it can be installed from the Marketplace and will get updates there.</p>{/if}
    {#if result.installed_version}<p class="ma-note" role="note">Already on this card as version {result.installed_version}: this will be reviewed as an update or replacement.</p>{/if}
    {#each p.warnings as w}<p class="ma-warn" role="note">{w}</p>{/each}

    {#if result.lint.findings.length}
      <h3>Core check</h3>
      <p class="ma-note" role="status">{result.lint.errors ? `${plural(result.lint.errors, 'error')} found` : 'No errors'}{result.lint.warnings ? `, ${plural(result.lint.warnings, 'warning')}` : ''}{result.lint.notes ? `, ${plural(result.lint.notes, 'note')}` : ''}. Each finding names the documentation rule it comes from.</p>
      <ul class="ma-lint" aria-label="Core check findings">{#each result.lint.findings as f}
        <li class={f.severity.toLowerCase()}><span class="chip {f.severity === 'Error' ? 'bad' : f.severity === 'Warning' ? 'warn' : ''}">{f.severity}</span><div><span>{f.message}</span><small>{f.fix} <em>({f.rule})</em></small></div></li>
      {/each}</ul>
      {#if lintErrors}<label class="ma-check ma-override"><input type="checkbox" bind:checked={override} /> Install anyway: I know what {lintErrors === 1 ? 'this error means' : 'these errors mean'} <small>(the core may not start or show in the menu)</small></label>{/if}
    {/if}

    <h3>Requirements</h3>
    <dl class="ma-facts"><div><dt>Pocket firmware</dt><dd>{p.firmware_required ? `${p.firmware_required} or newer` : 'Not stated'}</dd></div><div><dt>Dock</dt><dd>{yn(p.dock_supported)}</dd></div><div><dt>Link port</dt><dd>{yn(p.link_port)}</dd></div><div><dt>Sleep</dt><dd>{yn(p.sleep_supported)}</dd></div></dl>

    {#if p.needs.length}
      <h3>Files it uses</h3>
      {#if supply.length}<p class="ma-note you" role="note">You supply {plural(supply.length, 'file')}: {supply.map((s) => s.filename).join(', ')}. They are not downloaded or included.</p>{/if}
      <ul class="ma-needs" aria-label="Files it uses">{#each p.needs as n}
        <li><div class="ma-nm"><strong>{n.filename || n.slot}</strong><span class="chip">{n.slot}</span>{#if n.you_supply}<span class="chip warn">you supply this</span>{:else if n.in_package}<span class="chip capable">in the package</span>{:else if !n.required}<span class="chip">optional</span>{:else}<span class="chip">your files</span>{/if}</div>
          <small>{n.purpose} Goes in <code>{n.folder_rel}</code>{n.extensions.length ? ` (${n.extensions.map((e) => '.' + e).join(' ')})` : ''}.</small></li>
      {/each}</ul>
    {/if}
    {#if p.settings.length}<h3>Settings it offers</h3><p class="ma-small">{p.settings.join(' · ')}</p>{/if}
    {#if p.info_text}<h3>From the author</h3><pre class="ma-info">{p.info_text}</pre>{/if}
    {#if result.readme}
      <h3>README</h3><pre class="ma-info">{result.readme.text}</pre>
      {#if result.readme.images.length}<div class="ma-shots" aria-label="Pictures from the README">{#each result.readme.images as u}<img src={u} alt="Screenshot from the README" />{/each}</div>{/if}
    {/if}
    <h3>Where it came from</h3>
    <dl class="ma-facts">
      <div><dt>Source</dt><dd>{result.provenance.kind === 'github' ? `GitHub: ${result.provenance.owner}/${result.provenance.repo} · ${result.provenance.tag}` : result.provenance.kind === 'zip' ? 'A zip file on this computer' : `A folder on this computer${result.provenance.packed ? ` (${result.provenance.packed})` : ''}`}</dd></div>
      {#if result.provenance.asset}<div><dt>File</dt><dd>{result.provenance.asset}</dd></div>{/if}
      <div><dt>Size</dt><dd>{size(p.zip_bytes || result.provenance.bytes)} · {plural(p.files, 'file')}, {size(p.uncompressed_bytes)} unpacked</dd></div>
      <div><dt>SHA-256</dt><dd><code>{p.sha256.slice(0, 16)}…</code></dd></div>
      <div><dt>Checksum</dt><dd>{result.provenance.checksum === 'Verified' ? 'Matches the checksum GitHub publishes' : result.provenance.checksum === 'NotPublished' ? 'No checksum published for this release' : 'Computed here; nothing to compare it with'}</dd></div>
    </dl>

    {#if blocked}<p class="ma-err" role="alert">{blocked}</p>{/if}
    {#if !installed}<label class="ma-check ma-trust"><input type="checkbox" bind:checked={trust} disabled={!!blocked || lintBlocked} /> I trust this source <small>(asked every time, never remembered)</small></label>{/if}
    <div class="ma-actions"><button class="quiet" on:click={() => (step = 'source')}>Back</button></div>
    {#if trust && !blocked}
      <UpdatePlanView zipPath={result.zip_path} cardPath={card} lintGate={false} knownOverrides={override ? result.lint.findings.filter((f) => f.severity === 'Error').map((f) => `${f.id}: ${f.message}`) : []} on:installed={onInstalled} />
    {/if}
  {/if}
</div>
<style>
  .ma-backdrop{position:fixed;inset:0;background:rgba(0,0,0,.6);z-index:100}
  .ma{position:fixed;z-index:101;top:50%;left:50%;transform:translate(-50%,-50%);width:min(760px,calc(100vw - 32px));max-height:calc(100vh - 40px);overflow:auto;background:#1a2325;border:1px solid #2c393a;border-radius:14px;padding:22px 24px;box-sizing:border-box}
  .ma-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px;margin-bottom:12px}.ma-head h2{margin:0 0 8px}
  .ma-steps{display:flex;gap:6px;list-style:none;margin:0;padding:0;font-size:12px;color:#8f9e9d}.ma-steps li{padding:3px 10px;border-radius:99px;background:#141c1d}.ma-steps li.cur{background:#243628;color:#c1f0ad}
  .ma-choices{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin:6px 0 14px}
  .ma-choice{display:grid;gap:4px;justify-items:start;text-align:left;padding:16px;background:#161f20;border:1px solid #2c393a;border-radius:12px;color:inherit}.ma-choice:hover:not(:disabled){border-color:#4a6264;background:#1c282a}.ma-choice:focus-visible{outline:2px solid #c1f0ad;outline-offset:2px}
  .ma-choice small{color:#95a5a4;font-size:12px}.ma-ci{display:inline-grid;place-items:center;width:44px;height:44px;border-radius:12px;background:#1d2a2b;color:#c1f0ad;margin-bottom:4px}
  .ma-addr{display:grid;gap:6px;font-size:12.5px;color:#a6b3b2}.ma-addr input{font:inherit;font-size:13.5px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:10px;padding:11px 14px;width:100%}
  .ma-note{font-size:12px;color:#95a5a4;line-height:1.55;margin:6px 0}.ma-note.bad{color:#ff9b9b}.ma-note.ok{color:#c5f7ad}.ma-note.you{color:#e5d9a8}
  .ma-check{display:block;font-size:12.5px;color:#dce6e4;margin:10px 0}.ma-check small{color:#8f9e9d;font-size:11.5px}.ma-trust{border:1px solid #5c5030;background:#241f12;border-radius:10px;padding:10px 12px}
  .ma-actions{display:flex;justify-content:flex-end;gap:8px;margin:14px 0 4px}.ma-safe{font-size:11.5px;color:#8f9e9d;line-height:1.5;margin:8px 0}
  .ma-err{color:#ff9b9b;font-size:12.5px}.ma-row{display:grid;grid-template-columns:90px 1fr;gap:10px;align-items:center;font-size:12.5px;margin:8px 0}.ma-row select{font:inherit;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:8px;padding:8px 10px}
  .ma-banner{display:block;width:100%;height:auto;max-height:190px;object-fit:contain;border-radius:10px;border:1px solid #2c393a;background:#000;margin:0 0 12px}
  .ma-id{display:flex;gap:12px;align-items:center}.ma-id strong{font-size:16px}.ma-id small{display:block;color:#95a5a4;font-size:12px;margin-top:2px}.ma-where{display:flex!important;gap:6px;align-items:center}
  .ma-icon{flex:none;width:48px;height:48px;display:grid;place-items:center;background:#273638;border-radius:11px;color:#c1f0ad;font-weight:700}.ma-icon img{image-rendering:pixelated}
  .ma-desc{font-size:13px;color:#c7d1d0;line-height:1.55;margin:10px 0}
  .ma-banner-warn{border:1px solid #5c5030;background:#241f12;color:#e5d9a8;border-radius:10px;padding:10px 12px;font-size:12.5px;margin:10px 0;line-height:1.5}
  .ma-lint{list-style:none;margin:6px 0;padding:0;border:1px solid #2c393a;border-radius:9px}.ma-lint li{display:flex;gap:10px;align-items:flex-start;padding:9px 12px;border-bottom:1px solid #2c393a;font-size:12.5px}.ma-lint li:last-child{border:0}.ma-lint small{display:block;color:#95a5a4;font-size:11.5px;margin-top:2px}.ma-lint em{font-style:normal;color:#7f8e8d}.ma-lint .chip.bad{background:#3a1c1c;color:#ffb4b4}.ma-override{border:1px solid #5a3030;background:#241414;border-radius:10px;padding:10px 12px}
  .ma-warn{border:1px solid #3d4a2f;background:#182016;color:#cfe6bd;border-radius:9px;padding:8px 11px;font-size:12px;margin:6px 0}
  .ma h3{margin:16px 0 6px;font-size:11px;letter-spacing:.09em;color:#9fb3aa;text-transform:uppercase}
  .ma-facts{display:grid;gap:6px;margin:0}.ma-facts>div{display:grid;grid-template-columns:120px 1fr;gap:10px;font-size:12.5px}.ma-facts dt{color:#8f9e9d}.ma-facts dd{margin:0;color:#e8ecec;overflow-wrap:anywhere}
  .ma-needs{list-style:none;margin:0;padding:0;border:1px solid #2c393a;border-radius:10px;background:#141c1d}.ma-needs li{padding:10px 12px;border-bottom:1px solid #222d2f;display:grid;gap:4px}.ma-needs li:last-child{border:0}
  .ma-nm{display:flex;gap:8px;align-items:center;flex-wrap:wrap}.ma-needs small{font-size:12px;color:#a6b3b2;line-height:1.5}.chip.warn{background:#3a2e14;color:#e5d9a8}
  .ma-info{white-space:pre-wrap;font:inherit;font-size:12.5px;color:#c7d1d0;background:#141c1d;border:1px solid #222d2f;border-radius:9px;padding:10px 12px;margin:0;max-height:180px;overflow:auto}.ma-small{font-size:12.5px;color:#c7d1d0;margin:0}
  .ma-shots{display:flex;gap:8px;margin-top:8px;overflow:auto}.ma-shots img{height:120px;border-radius:8px;border:1px solid #2c393a}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px;overflow-wrap:anywhere}
  @media(max-width:720px){.ma-choices{grid-template-columns:1fr}}
</style>
