<script lang="ts">
  // Dedicated core view for Game Bub cards (`/cores/<id>/core.json`, `files.json`, `settings.json`).
  // Only shows features that apply to Game Bub (no Pocket-only visibility switch, saves/screenshots tabs, or Dock/Link/Sleep fields).
  import { errorMessage } from './backend';
  import { coresSelection, refreshCores } from './coresStore';
  import { platformFallbackArt } from './platformArt';
  import { gamebubExecuteRemove, gamebubInspectCard, gamebubPlanRemove } from './tau-api';
  import type { GameBubInstalledCore, GameBubRemovePlan } from './types';

  export let card = '';
  export let coreId = '';
  export let reloadCard: () => void = () => {};

  let tab: 'overview' | 'settings' = 'overview';
  let info: GameBubInstalledCore | null = null;
  let loading = true;
  let error = '';
  let loadedFor = '';

  let removePlan: GameBubRemovePlan | null = null;
  let removeBusy = false;
  let removeError = '';

  async function load(c: string, id: string) {
    loadedFor = `${c}|${id}`;
    loading = true;
    error = '';
    info = null;
    removePlan = null;
    removeError = '';
    tab = 'overview';
    try {
      const view = await gamebubInspectCard(c);
      if (loadedFor !== `${c}|${id}`) return;
      info = view.cores.find((x) => x.core.id.toLowerCase() === id.toLowerCase()) ?? null;
      if (!info) error = `Core “${id}” was not found under /cores/ on this card.`;
    } catch (e) {
      if (loadedFor === `${c}|${id}`) error = errorMessage(e);
    } finally {
      if (loadedFor === `${c}|${id}`) loading = false;
    }
  }

  $: if (card && coreId && `${card}|${coreId}` !== loadedFor) void load(card, coreId);

  $: missingCount = (info?.missing_bitstreams.length ?? 0) + (info?.missing_required_files.length ?? 0);
  $: fixedFiles = info?.files.filter((f) => !!f.filename) ?? [];
  $: romSlots = info?.files.filter((f) => f.user_selected) ?? [];
  $: dependentFiles = info?.files.filter((f) => f.dependent_on_0 && !f.filename && !f.user_selected) ?? [];

  async function startRemove() {
    if (!card || !info) return;
    removeBusy = true;
    removeError = '';
    try {
      removePlan = await gamebubPlanRemove(card, info.core.id);
    } catch (e) {
      removeError = errorMessage(e);
    } finally {
      removeBusy = false;
    }
  }

  async function confirmRemove() {
    if (!card || !info || !removePlan) return;
    removeBusy = true;
    removeError = '';
    try {
      await gamebubExecuteRemove(card, info.core.id, removePlan.id);
      removePlan = null;
      await refreshCores(card);
      reloadCard();
      coresSelection.set({ platform: '', core: '' });
    } catch (e) {
      removeError = errorMessage(e);
    } finally {
      removeBusy = false;
    }
  }
</script>

<header class="gb-head">
  <div>
    <nav class="crumbs" aria-label="Where you are">
      <button class="quiet" on:click={() => coresSelection.set({ platform: '', core: '' })}>Cores</button>
    </nav>
    <h1 id="cores-title">{info?.core.name || coreId}</h1>
    <p class="lede">{coreId}{info?.core.version ? ` · version ${info.core.version}` : ''} · Game Bub</p>
  </div>
  {#if info}
    <div class="gb-actions">
      <button class="quiet danger" disabled={removeBusy || !!removePlan} on:click={startRemove}>
        {removeBusy && !removePlan ? 'Planning…' : 'Remove core…'}
      </button>
    </div>
  {/if}
</header>

{#if removeError}<p class="gb-err" role="alert">{removeError}</p>{/if}

{#if removePlan}
  <div class="gb-rm-plan" role="region" aria-label="Remove core review">
    <strong>Remove {removePlan.name || removePlan.core_id} from this card?</strong>
    <p>
      Deletes {removePlan.files_to_remove.length} {removePlan.files_to_remove.length === 1 ? 'file' : 'files'} under <code>/cores/{info?.folder}</code> after saving a verified backup outside the card. Your ROMs and saves elsewhere on the card are untouched.
    </p>
    <div class="gb-rm-btns">
      <button class="primary danger" disabled={removeBusy} on:click={confirmRemove}>
        {removeBusy ? 'Removing…' : 'Remove core'}
      </button>
      <button class="quiet" disabled={removeBusy} on:click={() => (removePlan = null)}>Cancel</button>
    </div>
  </div>
{/if}

{#if error}
  <div class="gb-empty">
    <div class="gb-empty-ic" aria-hidden="true">!</div>
    <h2>This core could not be read</h2>
    <p>{error}</p>
    <button class="primary" on:click={() => load(card, coreId)}>Try again</button>
  </div>
{:else if loading || !info}
  <p class="gb-note" role="status">Reading core metadata…</p>
{:else}
  <div class="gb-grid">
    <div class="gb-main">
      <div class="tabs" role="tablist" aria-label="Core sections">
        <button id="gb-tab-overview" role="tab" class:current={tab === 'overview'} aria-selected={tab === 'overview'} on:click={() => (tab = 'overview')}>
          Files &amp; overview{#if missingCount}<span class="tab-dot" role="img" aria-label="Needs attention"></span>{/if}
        </button>
        <button id="gb-tab-settings" role="tab" class:current={tab === 'settings'} aria-selected={tab === 'settings'} on:click={() => (tab = 'settings')}>
          Settings{#if info.settings.length}<span class="gb-tab-n">{info.settings.length}</span>{/if}
        </button>
      </div>

      <div class="gb-scroll" role="tabpanel">
        {#if tab === 'overview'}
          <section class="gb-card" class:warn={missingCount > 0} class:ok={missingCount === 0}>
            <h3>Needs attention</h3>
            {#if missingCount === 0}
              <p class="gb-ok-text">All required bitstreams and fixed files for this core are present in <code>/cores/{info.folder}/</code>.</p>
            {:else}
              {#if info.missing_bitstreams.length}
                <p class="gb-warn-text">Missing bitstream{info.missing_bitstreams.length === 1 ? '' : 's'}: <code>{info.missing_bitstreams.join(', ')}</code></p>
              {/if}
              {#if info.missing_required_files.length}
                <p class="gb-warn-text">Missing required file{info.missing_required_files.length === 1 ? '' : 's'}: <code>{info.missing_required_files.join(', ')}</code> (place in <code>/cores/{info.folder}/</code>)</p>
              {/if}
            {/if}
          </section>

          <section class="gb-card">
            <h3>Declared files</h3>
            {#if !info.files.length}
              <div class="gb-empty compact">
                <div class="gb-empty-ic" aria-hidden="true">
                  <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
                </div>
                <strong>No fixed files declared</strong>
                <p>This core doesn’t include a <code>files.json</code> manifest. Game Bub lets you browse and load compatible ROMs from any folder on your microSD card.</p>
              </div>
            {:else}
              {#if fixedFiles.length}
                <h4 class="gb-subh">Fixed files in <code>/cores/{info.folder}/</code></h4>
                <ul class="gb-list">
                  {#each fixedFiles as f}
                    {@const isMissing = !!f.filename && info.missing_required_files.includes(f.filename)}
                    <li>
                      <div>
                        <strong>{f.label || f.filename}</strong>
                        <small><code>{f.filename}</code>{f.optional ? ' · optional' : ' · required'}</small>
                      </div>
                      {#if isMissing}<span class="chip warn">Missing</span>{:else}<span class="chip capable">Present</span>{/if}
                    </li>
                  {/each}
                </ul>
              {/if}
              {#if romSlots.length}
                <h4 class="gb-subh">User-selected files (loaded from anywhere on the SD card)</h4>
                <ul class="gb-list">
                  {#each romSlots as f}
                    <li>
                      <div>
                        <strong>{f.label || 'Game ROM'}</strong>
                        <small>{f.extensions.length ? f.extensions.map((e) => (e.startsWith('.') ? e : `.${e}`)).join('  ') : 'Any file'}{f.optional ? ' · optional' : ''}</small>
                      </div>
                      <span class="chip">Browser</span>
                    </li>
                  {/each}
                </ul>
              {/if}
              {#if dependentFiles.length}
                <h4 class="gb-subh">Companion files (matched to loaded ROM)</h4>
                <ul class="gb-list">
                  {#each dependentFiles as f}
                    <li>
                      <div>
                        <strong>{f.label}</strong>
                        <small>{f.extensions.length ? f.extensions.map((e) => (e.startsWith('.') ? e : `.${e}`)).join('  ') : 'Matched by name'}{f.read_only ? ' · read-only' : ''}</small>
                      </div>
                    </li>
                  {/each}
                </ul>
              {/if}
            {/if}
          </section>
        {:else}
          <section class="gb-card">
            <h3>Core settings</h3>
            {#if !info.settings.length}
              <div class="gb-empty compact">
                <div class="gb-empty-ic" aria-hidden="true">
                  <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9c.26.604.852.997 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
                </div>
                <strong>No configurable settings</strong>
                <p>This core doesn’t declare a <code>settings.json</code> file, so it runs with its built-in defaults on Game Bub.</p>
              </div>
            {:else}
              <p class="gb-sub">Declared in <code>/cores/{info.folder}/settings.json</code>. Change these in the core’s menu on your Game Bub.</p>
              <ul class="gb-list">
                {#each info.settings as s}
                  <li>
                    <strong>{s.label}</strong>
                    <span class="chip">{s.kind || 'option'}</span>
                  </li>
                {/each}
              </ul>
            {/if}
          </section>
        {/if}
      </div>
    </div>

    <aside class="gb-side" aria-label={info.core.name || info.core.id}>
      <img class="gb-banner" src={platformFallbackArt({ id: info.core.id, name: info.core.name || info.core.id, category: 'Handheld', manufacturer: info.core.author || 'Game Bub' })} alt="" />
      <div class="gb-side-title">
        <strong>{info.core.name || info.core.id}</strong>
        <small>{info.core.id}</small>
      </div>
      {#if info.core.description}<p class="gb-desc">{info.core.description}</p>{/if}
      <dl class="gb-dl">
        <div><dt>Author</dt><dd>{info.core.author || 'Not stated'}</dd></div>
        <div><dt>Version</dt><dd>{info.core.version || 'Not stated'}</dd></div>
        {#if info.core.release_date}<div><dt>Released</dt><dd>{info.core.release_date}</dd></div>{/if}
        {#if info.core.license}<div><dt>Licence</dt><dd>{info.core.license}</dd></div>{/if}
        <div><dt>Min firmware</dt><dd>{info.core.min_firmware || 'Not stated'}</dd></div>
        <div><dt>Folder</dt><dd><code>/cores/{info.folder}</code></dd></div>
      </dl>

      <h4>Hardware targets</h4>
      {#if info.core.bitstreams.length}
        <ul class="gb-targets">
          {#each info.core.bitstreams as bs}
            {@const missing = info.missing_bitstreams.includes(bs.filename)}
            <li>
              <span><strong>{bs.target}</strong><small><code>{bs.filename}</code></small></span>
              {#if missing}<span class="chip warn">Missing</span>{:else}<span class="chip capable">Ready</span>{/if}
            </li>
          {/each}
        </ul>
      {:else}
        <p class="gb-sub">No bitstreams listed in <code>core.json</code>.</p>
      {/if}

      {#if info.core.hardware.cartridge_enable || info.core.hardware.cartridge_selectable}
        <h4>Cartridge slot</h4>
        <p class="gb-sub">
          {info.core.hardware.cartridge_enable ? `Mode: ${info.core.hardware.cartridge_enable}` : 'Supported'}
          {info.core.hardware.cartridge_selectable ? ' · selectable' : ''}
        </p>
      {/if}
    </aside>
  </div>
{/if}

<style>
  .gb-head{flex:none;display:flex;justify-content:space-between;align-items:flex-start;gap:16px;padding-right:52px}
  .crumbs{display:flex;align-items:center;gap:2px;font-size:12px;color:#8f9e9d;margin:0 0 8px -8px}.crumbs .quiet{padding:4px 8px;font-size:12px}
  .gb-actions{display:flex;gap:8px;flex:none}
  .gb-err{margin:0 0 12px;padding:10px 14px;border-radius:10px;border:1px solid #6b3535;background:#261616;color:#f2b8b5;font-size:13px}
  .gb-note{font-size:13px;color:#95a5a4}
  .gb-rm-plan{margin:0 0 14px;padding:13px 16px;border-radius:12px;border:1px solid #6b3535;background:#261616;display:grid;gap:6px}
  .gb-rm-plan p{margin:0;font-size:12.5px;color:#e6c4c2;line-height:1.5}
  .gb-rm-btns{display:flex;gap:8px;margin-top:4px}
  .gb-grid{flex:1;min-height:0;display:grid;grid-template-columns:minmax(0,1fr) 320px;gap:22px;align-items:start}
  .gb-main{display:flex;flex-direction:column;min-height:0;min-width:0}
  .tabs{display:flex;gap:4px;border-bottom:1px solid #2c393a;margin:0 0 16px}
  .tabs [role=tab]{position:relative;padding:10px 14px;font-size:13.5px;color:#a4b0b0;background:transparent;border-radius:8px 8px 0 0;display:inline-flex;align-items:center;gap:7px}
  .tabs [role=tab]:hover{color:#eff5f4;background:#1a2325}.tabs [role=tab].current{color:#c1f0ad;box-shadow:inset 0 -2px 0 #c1f0ad}
  .gb-tab-n{font-size:11px;padding:1px 6px;border-radius:999px;background:#1f2c2e;color:#a6b3b2}
  .tab-dot{display:inline-block;width:7px;height:7px;border-radius:50%;background:#e5c88a}
  .gb-scroll{display:grid;gap:14px;padding-bottom:28px}
  .gb-card{background:#161f20;border:1px solid #2c393a;border-radius:12px;padding:16px 18px;display:grid;gap:10px}
  .gb-card.warn{border-color:#5c5030;background:#1f1c13}
  .gb-card h3{margin:0;font-size:14.5px}
  .gb-subh{margin:6px 0 0;font-size:11.5px;letter-spacing:.05em;text-transform:uppercase;color:#9fb3aa}
  .gb-ok-text{margin:0;font-size:13px;color:#c5f7ad;line-height:1.5}
  .gb-warn-text{margin:0;font-size:13px;color:#e5d9a8;line-height:1.5}
  .gb-sub{margin:0;font-size:12.5px;color:#95a5a4;line-height:1.5}
  .gb-list{list-style:none;margin:0;padding:0;display:grid;gap:6px}
  .gb-list li{display:flex;justify-content:space-between;align-items:center;gap:12px;padding:9px 12px;border-radius:9px;background:#121a1b;border:1px solid #232f31;font-size:13px}
  .gb-list small{display:block;font-size:11.5px;color:#8f9e9d;margin-top:2px}
  .gb-empty{display:grid;justify-items:center;text-align:center;gap:8px;padding:28px 20px;border:1px dashed #334345;border-radius:12px;background:#121a1b}
  .gb-empty.compact{padding:22px 18px}
  .gb-empty-ic{width:40px;height:40px;border-radius:12px;background:#1b2729;border:1px solid #2c3b3d;color:#c1f0ad;display:grid;place-items:center;font-weight:700}
  .gb-empty strong{font-size:14px;color:#e8ecec}
  .gb-empty p{margin:0;max-width:420px;font-size:12.5px;color:#95a5a4;line-height:1.55}
  .gb-side{background:#161f20;border:1px solid #2c393a;border-radius:12px;padding:14px;display:grid;gap:12px}
  .gb-banner{display:block;width:100%;aspect-ratio:16/7;object-fit:cover;border-radius:9px;background:#0b1011}
  .gb-side-title strong{display:block;font-size:15px}.gb-side-title small{font-size:12px;color:#8f9e9d}
  .gb-desc{margin:0;font-size:12.5px;color:#c7d1d0;line-height:1.55}
  .gb-dl{display:grid;gap:8px;margin:0}.gb-dl>div{display:flex;justify-content:space-between;gap:12px;font-size:12.5px}
  .gb-dl dt{color:#8f9e9d}.gb-dl dd{margin:0;color:#e8ecec;text-align:right;overflow-wrap:anywhere}
  .gb-side h4{margin:4px 0 0;font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:#9fb3aa}
  .gb-targets{list-style:none;margin:0;padding:0;display:grid;gap:6px}
  .gb-targets li{display:flex;justify-content:space-between;align-items:center;gap:8px;padding:7px 10px;border-radius:8px;background:#121a1b;border:1px solid #232f31;font-size:12.5px}
  .gb-targets small{display:block;font-size:11px;color:#8f9e9d}
  @media(max-width:1100px){.gb-grid{grid-template-columns:minmax(0,1fr)}}
</style>
