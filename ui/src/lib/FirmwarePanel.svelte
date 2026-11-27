<script lang="ts">
  // Per-system firmware detection, download, and reviewed install helper (`Analogue Pocket` + `Game Bub`).
  // Follows the global `activeSystem` store so switching systems here stays synced with Marketplace.
  import { save } from '@tauri-apps/plugin-dialog';
  import { errorMessage } from './backend';
  import { showToast } from './toastStore';
  import {
    firmwareDownload,
    firmwareExecuteInstall,
    firmwareLatest,
    firmwarePlanInstall,
    firmwareSaveCopy,
    firmwareStatus,
  } from './tau-api';
  import { activeSystem, activeSystemProfile, setActiveSystem, systemsStore } from './systemStore';
  import type {
    DeviceKind,
    FirmwareCardStatus,
    FirmwareDownload,
    FirmwareInstallPlan,
    FirmwareRelease,
  } from './types';

  export let card = '';

  let status: FirmwareCardStatus | null = null;
  let release: FirmwareRelease | null = null;
  let loading = false;
  let downloading = false;
  let installing = false;
  let error = '';
  let showNotes = false;
  let cachedDownload: FirmwareDownload | null = null;
  let plan: FirmwareInstallPlan | null = null;
  let targetForPlan = '';
  let loadedKey = '';

  function cmpVer(a: string, b: string): number {
    const p = (s: string) => s.replace(/^v/i, '').split(/[._-]/).map((n) => parseInt(n, 10)).filter((n) => !Number.isNaN(n));
    const x = p(a), y = p(b);
    for (let i = 0; i < Math.max(x.length, y.length); i++) {
      const d = (x[i] ?? 0) - (y[i] ?? 0);
      if (d !== 0) return d;
    }
    return 0;
  }

  $: latestOnCard = status?.card_files
    .map((f) => f.version)
    .filter((v): v is string => !!v)
    .sort((a, b) => cmpVer(b, a))[0] ?? null;
  $: pocketUpToDate = $activeSystem === 'analogue_pocket' && !!latestOnCard && !!release && cmpVer(latestOnCard, release.version) >= 0;
  $: pocketHasUpdate = $activeSystem === 'analogue_pocket' && !!latestOnCard && !!release && cmpVer(latestOnCard, release.version) < 0;
  $: firstUf2 = status?.uf2_drives[0] ?? null;

  async function refresh(device: DeviceKind, cardPath: string, force = false) {
    const key = `${device}|${cardPath}`;
    loadedKey = key;
    loading = true;
    error = '';
    if (force) { plan = null; cachedDownload = null; }
    try {
      const [st, rel] = await Promise.all([
        firmwareStatus(cardPath || null, device),
        firmwareLatest(device, force).catch((e) => { error = errorMessage(e); return null; }),
      ]);
      if (loadedKey !== key) return;
      status = st;
      if (rel) release = rel;
    } catch (e) {
      if (loadedKey === key) error = errorMessage(e);
    } finally {
      if (loadedKey === key) loading = false;
    }
  }

  $: if (`${$activeSystem}|${card}` !== loadedKey) {
    status = null;
    release = null;
    plan = null;
    cachedDownload = null;
    error = '';
    showNotes = false;
    void refresh($activeSystem, card, false);
  }

  function switchSystem(id: DeviceKind) {
    setActiveSystem(id);
  }

  async function ensureDownloaded(): Promise<FirmwareDownload | null> {
    if (!release) return null;
    if (cachedDownload && cachedDownload.file_name === release.file_name) return cachedDownload;
    downloading = true;
    error = '';
    try {
      const dl = await firmwareDownload($activeSystem, release);
      cachedDownload = dl;
      return dl;
    } catch (e) {
      error = errorMessage(e);
      return null;
    } finally {
      downloading = false;
    }
  }

  async function startInstall(targetRoot: string) {
    if (!release || !targetRoot) return;
    plan = null;
    const dl = await ensureDownloaded();
    if (!dl) return;
    try {
      targetForPlan = targetRoot;
      plan = await firmwarePlanInstall(targetRoot, $activeSystem, dl.path, release.file_name, release.md5);
    } catch (e) {
      error = errorMessage(e);
    }
  }

  async function confirmInstall() {
    if (!plan || !release || !cachedDownload || !targetForPlan) return;
    installing = true;
    error = '';
    try {
      const rep = await firmwareExecuteInstall(
        targetForPlan,
        $activeSystem,
        cachedDownload.path,
        release.file_name,
        release.md5,
        plan.id,
      );
      plan = null;
      const msg = $activeSystem === 'analogue_pocket'
        ? `Installed ${release.file_name} to the card root${rep.removed_old.length ? ` (backed up and removed ${rep.removed_old.join(', ')})` : ''}. Put the card in your Pocket and power it on to update.`
        : `Copied ${release.file_name} to ${targetForPlan}. Your Game Bub will flash and reboot automatically.`;
      showToast(msg, 'success', 7000);
      await refresh($activeSystem, card, false);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      installing = false;
    }
  }

  async function downloadAndSave() {
    if (!release) return;
    const dl = await ensureDownloaded();
    if (!dl) return;
    try {
      const picked = await save({ defaultPath: release.file_name });
      if (!picked || typeof picked !== 'string') return;
      const saved = await firmwareSaveCopy(dl.path, picked);
      showToast(`Saved verified firmware to ${saved}.`);
    } catch (e) {
      error = errorMessage(e);
    }
  }

  function formatDate(raw: string): string {
    if (!raw) return '';
    const d = new Date(raw);
    return Number.isNaN(d.getTime()) ? raw.slice(0, 10) : d.toLocaleDateString();
  }
</script>

<section class="fw-card" aria-labelledby="fw-title" data-testid="firmware-panel">
  <div class="fw-head">
    <div>
      <p class="eyebrow">SYSTEM FIRMWARE</p>
      <h2 id="fw-title">{$activeSystemProfile.name} firmware</h2>
    </div>
    <div class="fw-head-right">
      <div class="fw-switch" role="group" aria-label="System">
        {#each $systemsStore as sys (sys.id)}
          <button
            class="chip toggle"
            aria-pressed={$activeSystem === sys.id}
            on:click={() => switchSystem(sys.id)}
          >{sys.short_name}{#if sys.beta} <small>Beta</small>{/if}</button>
        {/each}
      </div>
      <button class="quiet" disabled={loading} on:click={() => refresh($activeSystem, card, true)}>
        {loading ? 'Checking…' : 'Check'}
      </button>
    </div>
  </div>

  {#if error}<p class="fw-err" role="alert">{error}</p>{/if}

  <div class="fw-grid">
    <div class="fw-col">
      <h3>{#if $activeSystem === 'analogue_pocket'}On this card{:else}On this device{/if}</h3>
      {#if $activeSystem === 'analogue_pocket'}
        {#if !card}
          <p class="fw-muted">Open a card to check for a firmware file at its root.</p>
        {:else if status?.card_files.length}
          <p class="fw-val">
            <strong>{latestOnCard ? `Version ${latestOnCard}` : status.card_files[0].file_name}</strong>
            {#if pocketUpToDate}<span class="chip capable">Up to date</span>
            {:else if pocketHasUpdate}<span class="chip warn">Update available</span>{/if}
          </p>
          <p class="fw-sub"><code>{status.card_files.map((f) => f.file_name).join(', ')}</code></p>
        {:else}
          <p class="fw-val"><strong>No firmware file on card</strong></p>
          <p class="fw-sub">Place <code>pocket_firmware_*.bin</code> on the card root to update your Pocket on next boot.</p>
        {/if}
        {#each status?.warnings ?? [] as w}
          <p class="fw-warn">{w}</p>
        {/each}
      {:else}
        {#if firstUf2}
          <p class="fw-val">
            <strong>{firstUf2.label} connected</strong>
            <span class="chip capable">Ready to flash</span>
          </p>
          {#if firstUf2.info}<p class="fw-sub">{firstUf2.info}</p>{/if}
        {:else}
          <p class="fw-val"><strong>USB update drive not connected</strong></p>
          <p class="fw-sub">
            Game Bub firmware lives on the handheld (not on the SD card). To flash over USB: power off, hold <strong>Vol −</strong> + <strong>Power</strong>, connect USB-C, then press Check.
          </p>
        {/if}
        {#if status?.required_min_firmware}
          <p class="fw-sub">Cores on this card require firmware <strong>{status.required_min_firmware}+</strong>.</p>
        {/if}
      {/if}
    </div>

    <div class="fw-col">
      <h3>Latest official release</h3>
      {#if loading && !release}
        <p class="fw-muted" role="status">Checking official release…</p>
      {:else if release}
        <p class="fw-val">
          <strong>Version {release.version}</strong>
          {#if release.size_label}<span class="fw-meta">{release.size_label}</span>{/if}
          {#if release.published_at}<span class="fw-meta">· {formatDate(release.published_at)}</span>{/if}
        </p>
        <p class="fw-sub"><code>{release.file_name}</code>{#if release.md5} · MD5 verified{/if}</p>
        <div class="fw-actions">
          {#if $activeSystem === 'analogue_pocket'}
            {#if card}
              <button class="primary" disabled={downloading || installing} on:click={() => startInstall(card)}>
                {downloading ? 'Downloading…' : pocketUpToDate ? 'Reinstall on card…' : 'Download & install to card…'}
              </button>
            {/if}
            <button class="quiet" disabled={downloading || installing} on:click={downloadAndSave}>
              {downloading ? 'Downloading…' : 'Save .bin…'}
            </button>
          {:else}
            {#if firstUf2}
              <button class="primary" disabled={downloading || installing} on:click={() => startInstall(firstUf2.path)}>
                {downloading ? 'Downloading…' : `Flash to ${firstUf2.label}…`}
              </button>
            {/if}
            <button class="quiet" disabled={downloading || installing} on:click={downloadAndSave}>
              {downloading ? 'Downloading…' : 'Download .uf2…'}
            </button>
          {/if}
          {#if release.release_notes}
            <button class="quiet" on:click={() => (showNotes = !showNotes)}>
              {showNotes ? 'Hide notes' : 'Release notes'}
            </button>
          {/if}
        </div>
      {:else}
        <p class="fw-muted">Could not fetch the latest release right now.</p>
      {/if}
    </div>
  </div>

  {#if showNotes && release?.release_notes}
    <pre class="fw-notes">{release.release_notes}</pre>
  {/if}

  {#if plan}
    <div class="fw-plan" role="region" aria-label="Firmware install review">
      <strong>Ready to write <code>{plan.file_name}</code></strong>
      <p>
        Destination: <code>{plan.target_path}</code>
        {#if plan.md5_verified} · MD5 verified{/if}
      </p>
      {#if plan.replaces.length}
        <p>Older firmware backed up and removed: <code>{plan.replaces.join(', ')}</code></p>
      {/if}
      {#each plan.warnings as w}
        <p class="fw-warn">{w}</p>
      {/each}
      <div class="fw-actions">
        <button class="primary" disabled={installing} on:click={confirmInstall}>
          {installing ? 'Writing…' : $activeSystem === 'analogue_pocket' ? 'Install on card' : 'Flash firmware now'}
        </button>
        <button class="quiet" disabled={installing} on:click={() => (plan = null)}>Cancel</button>
      </div>
    </div>
  {/if}
</section>

<style>
  .fw-card{border:1px solid var(--border);border-radius:12px;background:var(--bg-surface);padding:14px 16px;margin-top:14px;display:grid;gap:12px}
  .fw-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px;flex-wrap:wrap}
  .fw-head h2{margin:2px 0 0;font-size:16px}
  .fw-head-right{display:flex;align-items:center;gap:8px;flex-wrap:wrap}
  .fw-switch{display:inline-flex;gap:6px}
  .fw-switch small{opacity:.75;font-size:10.5px}
  .fw-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:14px}
  .fw-col{background:var(--bg-subtle);border:1px solid var(--bg-hover);border-radius:10px;padding:12px 14px;display:grid;gap:6px;align-content:start}
  .fw-col h3{margin:0;font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:var(--text-secondary)}
  .fw-val{margin:0;font-size:14px;display:flex;align-items:center;gap:8px;flex-wrap:wrap}
  .fw-meta{font-size:12px;color:var(--text-muted)}
  .fw-sub,.fw-muted{margin:0;font-size:12.5px;color:var(--text-muted);line-height:1.5}
  .fw-sub code{font-size:11.5px;color:var(--text-secondary)}
  .fw-actions{display:flex;gap:8px;flex-wrap:wrap;margin-top:4px}
  .fw-err{margin:0;padding:9px 12px;border-radius:9px;border:1px solid var(--danger-border);background:var(--danger-bg);color:var(--danger-text);font-size:12.5px}
  .fw-warn{margin:2px 0 0;font-size:12px;color:var(--warn-text)}
  .fw-notes{margin:0;padding:11px 13px;border-radius:9px;background:var(--bg-input);border:1px solid var(--border);font-size:12px;color:var(--text-secondary);white-space:pre-wrap;max-height:180px;overflow:auto;font-family:inherit;line-height:1.5}
  .fw-plan{padding:12px 14px;border-radius:10px;border:1px solid var(--border-strong);background:var(--bg-surface);display:grid;gap:6px}
  .fw-plan p{margin:0;font-size:12.5px;color:var(--text-secondary)}
</style>
