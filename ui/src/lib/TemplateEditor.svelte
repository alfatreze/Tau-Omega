<script lang="ts">
  // View and edit the template a build starts from, and see what it will look like: the Pocket's core list, the platform screen, and the files on the card.
  // Nothing here writes anything; the edits travel with the build and are applied when the package is made.
  import { open as pick } from '@tauri-apps/plugin-dialog';
  import { builderPreviewImage } from './tau-api';
  import { errorMessage } from './backend';
  import { size } from './coresFormat';
  import type { Fit, TemplateEdits, TemplateViewData } from './types';
  export let view: TemplateViewData | null = null;
  export let shortname = ''; export let version = '';
  /** What the editor produces: the changes only, ready for the build. `icon` is the picture for the author icon (a PNG or JPEG path). */
  export let edits: TemplateEdits; export let icon = '';
  export let changed: () => void = () => {};

  const FITS: [Fit, string][] = [['contain', 'Fit inside'], ['cover', 'Fill and crop'], ['stretch', 'Stretch']];
  const TRI: [string, string][] = [['keep', 'As the template'], ['yes', 'Yes'], ['no', 'No']];
  let author = ''; let url = ''; let fw = ''; let ids = ''; let dock = 'keep'; let link = 'keep'; let sleep = 'keep';
  let pName = ''; let pCat = ''; let pMaker = ''; let pYear = ''; let iconFit: Fit = 'contain'; let banner = ''; let bannerFit: Fit = 'contain'; let bannerPlatform = '';
  let seeded: TemplateViewData | null = null;
  $: if (view && view !== seeded) { seeded = view; author = view.author; url = view.url; fw = view.firmware_required; ids = view.platform_ids.join(', '); dock = 'keep'; link = 'keep'; sleep = 'keep';
    pName = view.platform?.name ?? ''; pCat = view.platform?.category ?? ''; pMaker = view.platform?.manufacturer ?? ''; pYear = view.platform?.year ? String(view.platform.year) : ''; icon = ''; banner = ''; iconFit = 'contain'; bannerFit = 'contain'; bannerPlatform = view.platform_ids[0] ?? ''; }
  const tri = (v: string) => (v === 'yes' ? true : v === 'no' ? false : null);
  $: idList = ids.split(',').map((s) => s.trim()).filter(Boolean);
  $: idsChanged = !!view && idList.join(',') !== view.platform_ids.join(',');
  $: pid = view ? (idList.includes(bannerPlatform) ? bannerPlatform : idList[0] ?? '') : '';
  $: platformChanged = !!view && (pName !== (view.platform?.name ?? '') || pCat !== (view.platform?.category ?? '') || pMaker !== (view.platform?.manufacturer ?? '') || pYear !== (view.platform?.year ? String(view.platform.year) : ''));
  $: edits = !view ? edits : {
    icon_fit: iconFit,
    author: author.trim() && author !== view.author ? author.trim() : null, url: url !== view.url ? url : null, firmware_required: fw !== view.firmware_required ? fw : null,
    dock_supported: tri(dock), link_port: tri(link), sleep_supported: tri(sleep),
    platform_ids: idsChanged ? idList : null,
    platform: platformChanged ? { id: view.platform?.id ?? idList[0] ?? '', name: pName, category: pCat, manufacturer: pMaker, year: pYear.trim() ? Number(pYear) || null : null } : null,
    banner: banner.trim() ? { path: banner.trim(), fit: bannerFit } : null,
  };
  $: if (view) { void edits; void icon; changed(); }

  // ---- previews: what the Pocket will actually show for each picture
  let iconPrev: string | null = null; let bannerPrev: string | null = null; let prevErr = '';
  let t1: ReturnType<typeof setTimeout> | undefined; let t2: ReturnType<typeof setTimeout> | undefined;
  $: { clearTimeout(t1); const p = icon.trim(); const f = iconFit; if (!p) iconPrev = null; else t1 = setTimeout(async () => { try { iconPrev = await builderPreviewImage(p, 'icon', f); prevErr = ''; } catch (e) { iconPrev = null; prevErr = errorMessage(e); } }, 250); }
  $: { clearTimeout(t2); const p = banner.trim(); const f = bannerFit; if (!p) bannerPrev = null; else t2 = setTimeout(async () => { try { bannerPrev = await builderPreviewImage(p, 'banner', f); prevErr = ''; } catch (e) { bannerPrev = null; prevErr = errorMessage(e); } }, 250); }
  async function choose(kind: 'icon' | 'banner') {
    const r = await pick({ multiple: false, title: kind === 'icon' ? 'Choose the author icon picture' : 'Choose the platform banner picture', filters: [{ name: 'Picture', extensions: ['png', 'jpg', 'jpeg'] }] });
    if (typeof r === 'string') { if (kind === 'icon') icon = r; else banner = r; }
  }

  $: coreId = view ? `${(edits.author ?? view.author).trim()}.${shortname.trim() || view.shortname}` : '';
  $: iconSrc = iconPrev ?? view?.icon ?? null; $: bannerSrc = bannerPrev ?? view?.banner ?? null;
  $: shownName = shortname.trim() || view?.shortname || ''; $: shownAuthor = edits.author ?? view?.author ?? '';
  // the files on the card: the template's, renamed to the new core id, plus what the edits add
  $: tree = (() => {
    if (!view) return [] as { path: string; bytes: number | null; note: string }[];
    const out = new Map<string, { path: string; bytes: number | null; note: string }>();
    for (const [n, b] of view.files) out.set(n.replace(`Cores/${view.core_id}/`, `Cores/${coreId}/`), { path: n.replace(`Cores/${view.core_id}/`, `Cores/${coreId}/`), bytes: b, note: '' });
    out.set(`Cores/${coreId}/bitstream.rbf_r`, { path: `Cores/${coreId}/bitstream.rbf_r`, bytes: null, note: 'your bitstream, converted' });
    if (icon.trim()) out.set(`Cores/${coreId}/icon.bin`, { path: `Cores/${coreId}/icon.bin`, bytes: 2592, note: 'new author icon' });
    if (edits.platform) out.set(`Platforms/${edits.platform.id}.json`, { path: `Platforms/${edits.platform.id}.json`, bytes: null, note: 'edited' });
    if (edits.banner) out.set(`Platforms/_images/${pid}.bin`, { path: `Platforms/_images/${pid}.bin`, bytes: 169620, note: 'new banner' });
    return [...out.values()].sort((a, b) => a.path.localeCompare(b.path));
  })();
  const dir = (p: string) => p.slice(0, p.lastIndexOf('/'));
  const base = (p: string) => p.slice(p.lastIndexOf('/') + 1);
</script>
{#if view}
<div class="te" aria-label="Template editor">
  <div class="te-form">
    <h3>The template’s details</h3>
    <label>Author <small>(the core id is author.shortname)</small><input bind:value={author} aria-label="Author" /></label>
    <label>Web address<input bind:value={url} /></label>
    <label>Pocket firmware needed<input bind:value={fw} placeholder="1.1" /></label>
    <label>Platform ids <small>(lowercase, comma separated)</small><input bind:value={ids} /></label>
    <div class="te-tri">
      <label>Dock<select bind:value={dock}>{#each TRI as [v, l]}<option value={v}>{l}{v === 'keep' ? ` (${view.dock_supported === null ? 'not stated' : view.dock_supported ? 'yes' : 'no'})` : ''}</option>{/each}</select></label>
      <label>Link port<select bind:value={link}>{#each TRI as [v, l]}<option value={v}>{l}{v === 'keep' ? ` (${view.link_port === null ? 'not stated' : view.link_port ? 'yes' : 'no'})` : ''}</option>{/each}</select></label>
      <label>Sleep<select bind:value={sleep}>{#each TRI as [v, l]}<option value={v}>{l}{v === 'keep' ? ` (${view.sleep_supported === null ? 'not stated' : view.sleep_supported ? 'yes' : 'no'})` : ''}</option>{/each}</select></label>
    </div>
    <h3>The platform</h3>
    <label>Name<input bind:value={pName} aria-label="Platform name" /></label>
    <div class="te-two"><label>Category<input bind:value={pCat} aria-label="Platform category" placeholder="Handheld" /></label><label>Manufacturer<input bind:value={pMaker} aria-label="Platform manufacturer" /></label><label>Year<input bind:value={pYear} aria-label="Platform year" inputmode="numeric" /></label></div>
    <h3>Pictures</h3>
    <div class="te-pic"><label>Author icon: a PNG or JPEG of any size<input bind:value={icon} aria-label="Author icon picture" placeholder="keep the template’s icon" /></label><select bind:value={iconFit} aria-label="How the icon is fitted">{#each FITS as [v, l]}<option value={v}>{l}</option>{/each}</select><button class="quiet" on:click={() => choose('icon')}>Choose…</button></div>
    <div class="te-pic"><label>Platform banner: a PNG or JPEG of any size<input bind:value={banner} aria-label="Platform banner picture" placeholder="keep the template’s banner" /></label><select bind:value={bannerFit} aria-label="How the banner is fitted">{#each FITS as [v, l]}<option value={v}>{l}</option>{/each}</select><button class="quiet" on:click={() => choose('banner')}>Choose…</button></div>
    {#if idList.length > 1}<label>Banner goes on platform<select bind:value={bannerPlatform} aria-label="Platform for the banner">{#each idList as i}<option value={i}>{i}</option>{/each}</select></label>{/if}
    <p class="te-hint">Pictures are turned into the Pocket’s own brightness bitmaps: white or bright is “on”, dark is “off”. The preview shows exactly what the Pocket will draw.</p>
    {#if prevErr}<p class="te-err" role="alert">{prevErr}</p>{/if}
  </div>

  <div class="te-prev" aria-label="Preview">
    <h3>On the Pocket’s core list</h3>
    <div class="pk-list" role="img" aria-label={`Core list row: ${shownName} by ${shownAuthor}`}>
      <span class="pk-icon">{#if iconSrc}<img src={iconSrc} alt="" />{/if}</span><span class="pk-txt"><strong>{shownName}</strong><small>{shownAuthor} · {version}</small></span>
    </div>
    <h3>On the platform screen</h3>
    <div class="pk-plat" role="img" aria-label={`Platform banner for ${pName || pid}`}>{#if bannerSrc}<img src={bannerSrc} alt="" />{:else}<span>No banner</span>{/if}</div>
    <p class="pk-cap">{[pName || pid, pMaker, pYear, pCat].filter(Boolean).join(' · ')}</p>
    <h3>On the card</h3>
    <ul class="pk-tree" aria-label="Files on the card">{#each tree as f (f.path)}<li class:new={!!f.note}><code>{dir(f.path)}/</code><strong>{base(f.path)}</strong><small>{f.bytes === null ? '' : size(f.bytes)}{f.note ? ` · ${f.note}` : ''}</small></li>{/each}</ul>
    <p class="te-hint">The card shows where each file would go for <code>{coreId}</code>. The Pocket’s own layout is approximated here, not captured from a device.</p>
  </div>
</div>
{/if}
<style>
  .te{display:grid;grid-template-columns:minmax(0,1.1fr) minmax(0,1fr);gap:22px;margin:12px 0;padding:16px;border:1px solid #2c393a;border-radius:12px;background:#161f20}
  .te h3{margin:14px 0 8px;font-size:11px;letter-spacing:.09em;color:#9fb3aa;text-transform:uppercase}.te h3:first-child{margin-top:0}
  .te label{display:grid;gap:5px;font-size:12px;color:#a6b3b2;margin:0 0 10px}.te label small{color:#7f8e8d}
  .te input,.te select{font:inherit;font-size:13px;color:#e8ecec;background:#101617;border:1px solid #3b4a4b;border-radius:9px;padding:8px 11px;width:100%;box-sizing:border-box}
  .te-tri,.te-two{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:10px}.te-pic{display:grid;grid-template-columns:minmax(0,1fr) 140px auto;gap:8px;align-items:end;margin:0 0 10px}.te-pic label{margin:0}
  .te-hint{font-size:12px;color:#8f9e9d;line-height:1.55}.te-err{color:#ff9b9b;font-size:12.5px}
  .pk-list{display:flex;align-items:center;gap:14px;padding:12px 16px;background:#000;border:1px solid #333;border-radius:10px}.pk-icon{flex:none;width:54px;height:54px;display:grid;place-items:center;background:#111;border-radius:6px}.pk-icon img{width:54px;height:54px;image-rendering:pixelated}
  .pk-txt{display:grid;gap:3px}.pk-txt strong{font-size:16px;color:#fff;letter-spacing:.02em}.pk-txt small{font-size:12px;color:#9aa}
  .pk-plat{background:#000;border:1px solid #333;border-radius:10px;aspect-ratio:521/165;display:grid;place-items:center;overflow:hidden}.pk-plat img{width:100%;height:100%;object-fit:contain;image-rendering:pixelated}.pk-plat span{color:#666;font-size:12px}
  .pk-cap{margin:6px 0 0;font-size:12.5px;color:#a6b3b2}
  .pk-tree{list-style:none;margin:6px 0;padding:0;border:1px solid #2c393a;border-radius:9px;max-height:260px;overflow:auto}.pk-tree li{display:flex;gap:6px;align-items:baseline;flex-wrap:wrap;padding:5px 11px;border-bottom:1px solid #222d2f;font-size:12px}.pk-tree li:last-child{border:0}.pk-tree li.new strong{color:#c1f0ad}.pk-tree code{color:#7f8e8d;font-size:11px;background:none;padding:0}.pk-tree small{color:#8f9e9d;margin-left:auto}
  @media(max-width:1000px){.te{grid-template-columns:1fr}.te-pic{grid-template-columns:1fr}}
</style>
